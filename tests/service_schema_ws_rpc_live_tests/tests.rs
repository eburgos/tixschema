//! `Ledger`: a read, a write and a one-way notice, covering the success, declared-error and
//! push-on-write shapes. `LedgerEvents`: the one-way, browser-implemented service a server pushes
//! to over a `FrameWriter` rather than calls.
//!
//! The axum server loop in [`ledger_socket`] and the tokio-tungstenite client loop in [`connect`]
//! are the reference adapters a developer copies around the generated pieces — `answer`,
//! `FrameWriter::new`, `FrameSession::new`, `deliver`, `close`, split, the outbound `mpsc`
//! channel. Every test drives them over a real socket bound to `127.0.0.1:0`.

#![cfg(feature = "serde")]

use crate::ledger_client_ws::{FrameSession, LedgerClient};
use crate::ledger_events_ws::LedgerEventsClient;
use crate::ledger_schema::{CallError, ServiceFaultKind};
use crate::{ledger_client_ws, ledger_events_ws, ledger_ws, screen_ws};
use alloc::sync::Arc;
use axum::Router;
use axum::extract::State;
use axum::extract::ws::WebSocketUpgrade;
use axum::response::IntoResponse;
use axum::routing::any;
use core::future::{Future, ready};
use core::net::SocketAddr;
use core::sync::atomic::{AtomicU64, Ordering};
use futures::{SinkExt as _, StreamExt as _};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tixschema::{model_schema, service_schema};
use tokio::net::TcpListener;
use tokio::runtime::{Builder, Runtime};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::{Bytes, Message};

#[model_schema()]
#[derive(Debug, Serialize, Deserialize)]
pub struct ListTransactionsRequest {
    pub account_id: String,
}

#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub account_id: String,
    pub amount_cents: i64,
    pub id: String,
    pub memo: String,
}

#[model_schema()]
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionList {
    pub items: Vec<Transaction>,
}

#[model_schema()]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum ListError {
    AccountNotFound,
}

#[model_schema()]
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTransactionRequest {
    pub account_id: String,
    pub amount_cents: i64,
    pub memo: String,
}

#[model_schema()]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum CreateError {
    AccountNotFound,
    InsufficientFunds,
}

#[model_schema()]
#[derive(Debug, Serialize, Deserialize)]
pub struct MarkSeen {
    pub transaction_id: String,
}

#[model_schema()]
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionPosted {
    pub transaction: Transaction,
}

/// What the browser or the Flutter app calls on the server.
#[service_schema(transports = ["ws_rpc"])]
pub trait Ledger<Ctx> {
    async fn create_transaction(
        &self,
        ctx: &Ctx,
        req: CreateTransactionRequest,
    ) -> Result<Transaction, CreateError>;

    async fn list_transactions(
        &self,
        ctx: &Ctx,
        req: ListTransactionsRequest,
    ) -> Result<TransactionList, ListError>;

    /// The app tells the server it showed a transaction; nothing comes back.
    #[service_schema_op(one_way)]
    async fn mark_seen(&self, ctx: &Ctx, req: MarkSeen);
}

/// What the server calls on the browser or the Flutter app.
#[service_schema(transports = ["ws_rpc"])]
pub trait LedgerEvents<Ctx> {
    #[service_schema_op(one_way)]
    async fn transaction_posted(&self, ctx: &Ctx, ev: TransactionPosted);
}

/// An in-memory `Ledger` backend: one user's one account seeded with two transactions, an id
/// counter and the set of transaction ids a `mark_seen` notify has recorded. Accounts are keyed
/// by `(user_id, account_id)`, the pair a real multi-tenant store would scope a lookup by.
struct LedgerBackEnd {
    accounts: Mutex<HashMap<(String, String), Vec<Transaction>>>,
    next_id: AtomicU64,
    seen: Mutex<Vec<String>>,
}

impl LedgerBackEnd {
    fn has_seen(&self, transaction_id: &str) -> bool {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .any(|marked| marked == transaction_id)
    }

    fn insert(
        &self,
        user_id: &str,
        req: &CreateTransactionRequest,
    ) -> Result<Transaction, CreateError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let transaction = Transaction {
            account_id: req.account_id.clone(),
            amount_cents: req.amount_cents,
            id: format!("tx-{id}"),
            memo: req.memo.clone(),
        };
        if self.record(user_id, transaction.clone()) {
            Ok(transaction)
        } else {
            Err(CreateError::AccountNotFound)
        }
    }

    /// Appends `transaction` to its own user's account list, and reports whether that account
    /// exists.
    fn record(&self, user_id: &str, transaction: Transaction) -> bool {
        let mut accounts = self.accounts.lock().unwrap();
        let key = (user_id.to_owned(), transaction.account_id.clone());
        let Some(transactions) = accounts.get_mut(&key) else {
            return false;
        };
        transactions.push(transaction);
        drop(accounts);
        true
    }

    fn seeded() -> Self {
        let seeded = vec![
            Transaction {
                account_id: "acc-1".to_owned(),
                amount_cents: 500,
                id: "tx-1".to_owned(),
                memo: "seed one".to_owned(),
            },
            Transaction {
                account_id: "acc-1".to_owned(),
                amount_cents: 1_200,
                id: "tx-2".to_owned(),
                memo: "seed two".to_owned(),
            },
        ];
        let key = ("u-1".to_owned(), "acc-1".to_owned());
        Self {
            accounts: Mutex::new(HashMap::from([(key, seeded)])),
            next_id: AtomicU64::new(3),
            seen: Mutex::new(Vec::new()),
        }
    }

    fn transactions(&self, user_id: &str, account_id: &str) -> Option<Vec<Transaction>> {
        let key = (user_id.to_owned(), account_id.to_owned());
        self.accounts.lock().unwrap().get(&key).cloned()
    }
}

/// What a connection's `Ledger` implementation is handed as context: who it is, and the client it
/// pushes `LedgerEvents` to on this same connection.
struct Session {
    events: LedgerEventsClient<ledger_events_ws::FrameWriter>,
    user_id: String,
}

impl Ledger<Session> for LedgerBackEnd {
    async fn create_transaction(
        &self,
        ctx: &Session,
        req: CreateTransactionRequest,
    ) -> Result<Transaction, CreateError> {
        let transaction = self.insert(&ctx.user_id, &req)?;
        let _: Result<(), _> = ctx
            .events
            .transaction_posted(TransactionPosted {
                transaction: transaction.clone(),
            })
            .await; // push to this connection
        Ok(transaction)
    }

    fn list_transactions(
        &self,
        ctx: &Session,
        req: ListTransactionsRequest,
    ) -> impl Future<Output = Result<TransactionList, ListError>> {
        let outcome = self
            .transactions(&ctx.user_id, &req.account_id)
            .ok_or(ListError::AccountNotFound)
            .map(|items| TransactionList { items });
        ready(outcome)
    }

    fn mark_seen(&self, _ctx: &Session, req: MarkSeen) -> impl Future<Output = ()> {
        self.seen.lock().unwrap().push(req.transaction_id);
        ready(())
    }
}

/// The browser or Flutter side of `LedgerEvents`: every push it receives is forwarded onto a
/// channel a test can read back from, standing in for rendering it.
struct Screen(mpsc::Sender<TransactionPosted>);

impl LedgerEvents<()> for Screen {
    async fn transaction_posted(&self, _ctx: &(), ev: TransactionPosted) {
        let _: Result<(), _> = self.0.send(ev).await;
    }
}

/// Starts the server on an ephemeral port.
async fn serve(back_end: Arc<LedgerBackEnd>) -> (SocketAddr, JoinHandle<()>) {
    let app = Router::new()
        .route("/ledger", any(ledger_socket))
        .with_state(back_end);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    (
        addr,
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        }),
    )
}

/// The reference axum server loop: upgrade, split, an outbound `mpsc` channel forwarding to the
/// sink, and `ledger_ws::answer` per inbound text frame.
async fn ledger_socket(
    ws: WebSocketUpgrade,
    State(back_end): State<Arc<LedgerBackEnd>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        use axum::extract::ws::Message;

        let (mut sink, mut stream) = socket.split();
        let (out, mut outbox) = mpsc::channel::<String>(64);
        tokio::spawn(async move {
            while let Some(text) = outbox.recv().await {
                let _: Result<(), _> = sink.send(Message::Text(text.into())).await;
            }
        });
        let push = out.clone();
        let session = Session {
            user_id: "u-1".to_owned(),
            events: LedgerEventsClient::new(ledger_events_ws::FrameWriter::new(move |text| {
                let sender = push.clone();
                async move {
                    sender
                        .send(text)
                        .await
                        .map_err(|refused| refused.to_string())
                }
            })),
        };
        while let Some(Ok(message)) = stream.next().await {
            let Message::Text(text) = message else {
                continue;
            };
            if let Some(reply) = ledger_ws::answer(&text, &*back_end, &session).await {
                let _: Result<(), _> = out.send(reply).await;
            }
        }
    })
}

/// A `LedgerBackEnd` bound to its own dedicated runtime, so the runtime itself — not just the
/// accept loop — can be dropped to simulate the server disappearing out from under a live
/// connection. `axum::serve`'s accept loop hands each connection to its own detached task, so
/// aborting only that loop's `JoinHandle` leaves an already-upgraded socket running; tearing down
/// the runtime it was spawned on drops every task and every socket bound to it instead.
fn spawn_droppable_server(
    back_end: Arc<LedgerBackEnd>,
) -> (oneshot::Receiver<SocketAddr>, Runtime) {
    let runtime = Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let (addr_tx, addr_rx) = oneshot::channel();
    runtime.spawn(async move {
        let app = Router::new()
            .route("/ledger", any(ledger_socket))
            .with_state(back_end);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let _: Result<(), _> = addr_tx.send(listener.local_addr().unwrap());
        axum::serve(listener, app).await.unwrap();
    });
    (addr_rx, runtime)
}

/// The reference tokio-tungstenite client loop: connect, split, an outbound `mpsc` channel
/// forwarding to the sink, `reader.deliver` for replies and pongs, `screen_ws::answer` for
/// pushes, and `reader.close` once the stream ends.
async fn connect(
    url: &str,
) -> (
    LedgerClient<FrameSession>,
    mpsc::Receiver<TransactionPosted>,
) {
    let (socket, _response) = connect_async(url).await.unwrap();
    let (mut sink, mut stream) = socket.split();
    let (out, mut outbox) = mpsc::channel::<String>(64);
    tokio::spawn(async move {
        while let Some(text) = outbox.recv().await {
            let _: Result<(), _> = sink.send(Message::Text(text.into())).await;
        }
    });

    let session = FrameSession::new(move |text| {
        let sender = out.clone();
        async move {
            sender
                .send(text)
                .await
                .map_err(|refused| refused.to_string())
        }
    });
    let ledger = LedgerClient::new(session.clone());
    let reader = session;
    let (posted_out, posted_in) = mpsc::channel(16);
    let screen = Screen(posted_out);
    tokio::spawn(async move {
        while let Some(Ok(message)) = stream.next().await {
            let Message::Text(text) = message else {
                continue;
            };
            reader.deliver(&text).await; // replies to my requests, and pongs
            screen_ws::answer(&text, &screen, &()).await; // pushes from the server
        }
        reader.close("the socket closed");
    });

    (ledger, posted_in)
}

#[tokio::test]
async fn list_transactions_answers_the_seeded_list_and_a_created_transaction_reaches_screen() {
    let (addr, server) = serve(Arc::new(LedgerBackEnd::seeded())).await;
    let (ledger, mut posted) = connect(&format!("ws://{addr}/ledger")).await;

    let listed = ledger
        .list_transactions(ListTransactionsRequest {
            account_id: "acc-1".to_owned(),
        })
        .await
        .unwrap();
    assert_eq!(listed.items.len(), 2);

    let created = ledger
        .create_transaction(CreateTransactionRequest {
            account_id: "acc-1".to_owned(),
            amount_cents: 1_250,
            memo: "coffee".to_owned(),
        })
        .await
        .unwrap();
    let pushed = posted.recv().await.unwrap();
    assert_eq!(pushed.transaction.id, created.id);

    server.abort();
}

#[tokio::test]
async fn mark_seen_is_one_way_and_the_store_records_it() {
    let back_end = Arc::new(LedgerBackEnd::seeded());
    let (addr, server) = serve(Arc::clone(&back_end)).await;
    let (ledger, _posted) = connect(&format!("ws://{addr}/ledger")).await;

    ledger
        .mark_seen(MarkSeen {
            transaction_id: "tx-1".to_owned(),
        })
        .await
        .unwrap();
    // Frames arrive in the order they were sent and the server answers one at a time, so this
    // reply cannot land until the one-way notify ahead of it has already been dispatched.
    ledger
        .list_transactions(ListTransactionsRequest {
            account_id: "acc-1".to_owned(),
        })
        .await
        .unwrap();

    assert!(back_end.has_seen("tx-1"));
    server.abort();
}

#[tokio::test]
async fn create_transaction_for_an_unknown_account_answers_the_declared_error() {
    let (addr, server) = serve(Arc::new(LedgerBackEnd::seeded())).await;
    let (ledger, _posted) = connect(&format!("ws://{addr}/ledger")).await;

    let refused = ledger
        .create_transaction(CreateTransactionRequest {
            account_id: "acc-missing".to_owned(),
            amount_cents: 100,
            memo: "n/a".to_owned(),
        })
        .await;
    assert!(matches!(
        refused,
        Err(CallError::Operation(CreateError::AccountNotFound))
    ));

    server.abort();
}

#[tokio::test]
async fn a_client_ping_is_answered_with_a_pong() {
    let (addr, server) = serve(Arc::new(LedgerBackEnd::seeded())).await;
    let (mut socket, _response) = connect_async(format!("ws://{addr}/ledger")).await.unwrap();

    socket
        .send(Message::Ping(Bytes::from_static(b"ping")))
        .await
        .unwrap();
    let answered = socket.next().await.unwrap().unwrap();
    assert_eq!(answered, Message::Pong(Bytes::from_static(b"ping")));

    server.abort();
}

#[tokio::test]
async fn dropping_the_server_settles_a_waiting_call_with_a_transport_failure_fault() {
    let (addr_rx, server) = spawn_droppable_server(Arc::new(LedgerBackEnd::seeded()));
    let addr = addr_rx.await.unwrap();
    let (ledger, _posted) = connect(&format!("ws://{addr}/ledger")).await;

    // Spawned but not yet polled: nothing here yields before `shutdown_background` runs, so the
    // call is still unsent when the server's runtime — sockets included — is torn down under it.
    let waiting = tokio::spawn(async move {
        ledger
            .list_transactions(ListTransactionsRequest {
                account_id: "acc-1".to_owned(),
            })
            .await
    });
    server.shutdown_background();

    let fault = match waiting.await.unwrap() {
        Err(CallError::Fault(fault)) => Ok(fault),
        other => Err(format!("expected a transport-failure fault, got {other:?}")),
    }
    .unwrap();
    assert_eq!(fault.kind(), ServiceFaultKind::TransportFailure);
}

// The six tests above drive every module through the one path a live socket actually takes.
// `Frame` no longer means one shared codec: each macro now reads only the frame kinds its own
// side receives, so a dispatcher's `Frame` cannot name a reply and a client's cannot name a
// request or a notify — there is nothing to hand-decode on either side that the type system does
// not already refuse to construct. A one-way-only service's `Reply` carries no `send` either, for
// the same reason: `LedgerEvents` never answers, so its dispatcher never publishes a method to
// answer with. What follows drives only what still cannot be reached through the live socket
// above: a session's own `ping_frame`, a `FrameWriter`'s construction and its refusal of
// `request`, the `.transport()` accessor every generated client publishes, and — since
// `LedgerEvents`'s own client is only ever bound to a `FrameWriter` on the live path — a
// `FrameSession` round trip and its own ping/pong handling for `ledger_events_ws`.

/// `ledger_client_ws`'s own copy of `ping_frame`, published beside `FrameSession`'s liveness
/// probe — a dispatcher never sends one.
#[tokio::test]
async fn ledger_client_ws_ping_frame_encodes_the_kind_ping_frame() {
    assert_eq!(ledger_client_ws::ping_frame(), r#"{"kind":"ping"}"#);
}

/// A `FrameWriter` carries no correlation map, so `request` is refused outright, naming the
/// operation that asked for an answer it cannot wait on.
#[tokio::test]
async fn a_ledger_client_frame_writer_asked_for_a_request_answers_err_naming_the_operation() {
    let writer = ledger_client_ws::FrameWriter::new(|_text| async { Ok(()) });
    let refused =
        ledger_client_ws::Transport::request(&writer, "list-transactions", &(), Vec::new())
            .await
            .err()
            .unwrap();
    assert!(
        refused.contains("list-transactions"),
        "the refusal names the operation. Got: {refused}"
    );
}

/// `LedgerClient::transport` reaches the transport a client was bound to, the same as every
/// other generated client.
#[tokio::test]
async fn a_ledger_client_exposes_the_transport_it_was_bound_to() {
    let (sent_out, mut sent_in) = mpsc::channel::<String>(4);
    let writer = ledger_client_ws::FrameWriter::new(move |text| {
        let sender = sent_out.clone();
        async move {
            sender
                .send(text)
                .await
                .map_err(|refused| refused.to_string())
        }
    });
    let ledger = LedgerClient::new(writer);
    ledger_client_ws::Transport::notify(ledger.transport(), "mark-seen", &(), Vec::new())
        .await
        .unwrap();
    let sent = sent_in.recv().await.unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&sent).unwrap(),
        serde_json::json!({
            "kind": "notify",
            "operation": "mark-seen",
            "payload": null,
            "service": "Ledger",
        }),
    );
}

/// `ledger_events_ws`'s own copy of `ping_frame` — a session sends one, a dispatcher only ever
/// answers one.
#[tokio::test]
async fn ledger_events_ws_ping_frame_encodes_the_kind_ping_frame() {
    assert_eq!(ledger_events_ws::ping_frame(), r#"{"kind":"ping"}"#);
}

/// `deliver` answers a ping with a pong on `ledger_events_ws`'s own `FrameSession` too — nothing
/// in the live test above ever sends one over this connection, since it carries only pushes.
#[tokio::test]
async fn the_ledger_events_frame_session_answers_a_ping_with_a_pong() {
    let (sent_out, mut sent_in) = mpsc::channel::<String>(4);
    let session = ledger_events_ws::FrameSession::new(move |text| {
        let sender = sent_out.clone();
        async move {
            sender
                .send(text)
                .await
                .map_err(|refused| refused.to_string())
        }
    });
    session.deliver(r#"{"kind":"ping"}"#).await;
    let sent = sent_in.recv().await.unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&sent).unwrap(),
        serde_json::json!({ "kind": "pong" }),
    );
}

/// The `ledger_events_ws` module's own `FrameSession` completes a request-and-reply round trip
/// exactly like `ledger_client_ws`'s, even though `LedgerEvents` declares no reply operation of
/// its own to exercise it through a generated method — the transport machinery does not read the
/// operation's own shape.
#[tokio::test]
async fn the_ledger_events_frame_session_completes_a_request_and_reply_round_trip() {
    let (sent_out, mut sent_in) = mpsc::channel::<String>(4);
    let session = ledger_events_ws::FrameSession::new(move |text| {
        let sender = sent_out.clone();
        async move {
            sender
                .send(text)
                .await
                .map_err(|refused| refused.to_string())
        }
    });
    let waiting = tokio::spawn({
        let requester = session.clone();
        async move { ledger_events_ws::Transport::request(&requester, "probe", &(), Vec::new()).await }
    });
    let sent = sent_in.recv().await.unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&sent).unwrap(),
        serde_json::json!({
            "id": "1",
            "kind": "request",
            "operation": "probe",
            "payload": null,
            "service": "LedgerEvents",
        }),
    );
    session
        .deliver(r#"{"kind":"reply","id":"1","service":"LedgerEvents","ok":true}"#)
        .await;
    let (encoded, headers) = waiting.await.unwrap().unwrap();
    assert_eq!(headers, Vec::new());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&encoded).unwrap(),
        serde_json::json!({ "ok": true }),
    );
    session.close("done");
}

/// `LedgerEventsClient::transport` reaches the transport a client was bound to, the same as
/// every other generated client.
#[tokio::test]
async fn a_ledger_events_client_exposes_the_transport_it_was_bound_to() {
    let (sent_out, mut sent_in) = mpsc::channel::<String>(4);
    let writer = ledger_events_ws::FrameWriter::new(move |text| {
        let sender = sent_out.clone();
        async move {
            sender
                .send(text)
                .await
                .map_err(|refused| refused.to_string())
        }
    });
    let events = LedgerEventsClient::new(writer);
    ledger_events_ws::Transport::notify(events.transport(), "probe", &(), Vec::new())
        .await
        .unwrap();
    let sent = sent_in.recv().await.unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&sent).unwrap(),
        serde_json::json!({
            "kind": "notify",
            "operation": "probe",
            "payload": null,
            "service": "LedgerEvents",
        }),
    );
}
