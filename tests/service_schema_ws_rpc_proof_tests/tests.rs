//! `DocumentSession` (a `header_in`/`header_out` reply, two plain replies and a one-way) and
//! `SessionEvents` (one one-way, pushed rather than called) mirror the shapes
//! `service_schema_ws_rpc_tests` already drives by hand — this harness instead joins the generated
//! `DocumentSession` client to the generated `DocumentSession` dispatcher, and the generated
//! `SessionEvents` client to the generated `SessionEvents` dispatcher, through one in-memory
//! [`Wire`], so every frame either side sends is actually read by the other.
//!
//! `DocumentBackEnd` answers `DocumentSession` on the server side; `Screen` answers
//! `SessionEvents` on the client side, receiving whatever `Session::events` — the server's own
//! generated client, over a `FrameWriter` into the wire's own client-bound queue — pushes.
//! [`Harness`] bundles one of each, and [`drive`] polls a call, pumping the wire between polls,
//! until it settles.

#![cfg(feature = "serde")]

use crate::document_session_schema::{CallError, ServiceFaultKind};
use crate::events_client;
use crate::events_transport;
use crate::ws_client::{self, DocumentSessionClient};
use crate::ws_transport;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::future::{Future, ready};
use core::pin::{Pin, pin};
use core::task::{Context as PollContext, Poll, Waker};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tixschema::{model_schema, service_schema};

#[model_schema()]
#[derive(Deserialize, Serialize)]
pub struct ReadRangeRequest {
    pub document_id: String,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
pub struct TouchRequest {
    pub document_id: String,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
pub struct UnwatchRequest {
    pub document_id: String,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
pub struct WatchRequest {
    pub document_id: String,
}

#[model_schema()]
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RangeResult {
    pub content: String,
}

#[model_schema()]
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WatchResult {
    pub accepted: bool,
}

#[model_schema()]
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum RangeError {
    NotFound,
}

#[model_schema()]
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum UnwatchError {
    NotFound,
}

#[model_schema()]
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum WatchError {
    NotFound,
}

#[service_schema(transports = ["ws_rpc"])]
pub trait DocumentSession<Ctx> {
    #[service_schema_op(http(
        method = "GET",
        path = "/documents/{document_id}/range",
        header_in("range" = byte_range),
        header_out("etag"),
        error_status(NotFound = 404),
    ))]
    async fn read_range(
        &self,
        ctx: &Ctx,
        req: ReadRangeRequest,
        byte_range: Option<String>,
    ) -> Result<(RangeResult, String), RangeError>;

    #[service_schema_op(one_way)]
    async fn touch(&self, ctx: &Ctx, req: TouchRequest);

    async fn unwatch(&self, ctx: &Ctx, req: UnwatchRequest) -> Result<(), UnwatchError>;

    async fn watch(&self, ctx: &Ctx, req: WatchRequest) -> Result<WatchResult, WatchError>;
}

#[model_schema()]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DocumentChangedRequest {
    pub document_id: String,
    pub version: u32,
}

/// A service the browser implements: a server holds this client over a `FrameWriter` to push an
/// event, and never a `FrameSession` — there is no reply to wait on.
#[service_schema(transports = ["ws_rpc"])]
pub trait SessionEvents<Ctx> {
    #[service_schema_op(one_way)]
    async fn document_changed(&self, ctx: &Ctx, req: DocumentChangedRequest);
}

/// The server side of `DocumentSession`. Every call is reached through `Session`, the same `Ctx`
/// every operation takes, though only the push scenario actually calls through it.
struct DocumentBackEnd {
    reached: Mutex<Vec<String>>,
}

impl DocumentBackEnd {
    fn new() -> Self {
        Self {
            reached: Mutex::new(Vec::new()),
        }
    }

    fn reach(&self, marker: String) {
        self.reached.lock().unwrap().push(marker);
    }

    fn reached(&self) -> Vec<String> {
        self.reached.lock().unwrap().clone()
    }
}

impl DocumentSession<Session> for DocumentBackEnd {
    async fn read_range(
        &self,
        _ctx: &Session,
        req: ReadRangeRequest,
        byte_range: Option<String>,
    ) -> Result<(RangeResult, String), RangeError> {
        ready(()).await;
        self.reach(format!("read_range {} {byte_range:?}", req.document_id));
        if req.document_id == "missing" {
            return Err(RangeError::NotFound);
        }
        Ok((
            RangeResult {
                content: format!("range-of-{}", req.document_id),
            },
            "etag-1".to_owned(),
        ))
    }

    async fn touch(&self, _ctx: &Session, req: TouchRequest) {
        ready(()).await;
        self.reach(format!("touch {}", req.document_id));
    }

    async fn unwatch(&self, _ctx: &Session, req: UnwatchRequest) -> Result<(), UnwatchError> {
        ready(()).await;
        self.reach(format!("unwatch {}", req.document_id));
        if req.document_id == "missing" {
            return Err(UnwatchError::NotFound);
        }
        Ok(())
    }

    async fn watch(&self, _ctx: &Session, req: WatchRequest) -> Result<WatchResult, WatchError> {
        ready(()).await;
        self.reach(format!("watch {}", req.document_id));
        if req.document_id == "missing" {
            return Err(WatchError::NotFound);
        }
        Ok(WatchResult { accepted: true })
    }
}

/// The browser side of `SessionEvents`: records every event pushed to it, in the order it
/// arrived.
struct Screen {
    received: Mutex<Vec<DocumentChangedRequest>>,
}

impl Screen {
    fn new() -> Self {
        Self {
            received: Mutex::new(Vec::new()),
        }
    }

    fn received(&self) -> Vec<DocumentChangedRequest> {
        self.received.lock().unwrap().clone()
    }
}

impl SessionEvents<()> for Screen {
    async fn document_changed(&self, _ctx: &(), req: DocumentChangedRequest) {
        ready(()).await;
        self.received.lock().unwrap().push(req);
    }
}

/// The context every `DocumentSession` operation receives: the server's own means of pushing to
/// the browser, a `SessionEventsClient` over a `FrameWriter` that writes into the wire's own
/// client-bound queue.
struct Session {
    events: events_client::SessionEventsClient<events_client::FrameWriter>,
}

/// The two queues a text frame crosses between client and server, and every frame either side
/// sent, in the order it crossed.
#[derive(Default)]
struct Wire {
    crossed: Mutex<Vec<String>>,
    to_client: Mutex<VecDeque<String>>,
    to_server: Mutex<VecDeque<String>>,
}

/// One `DocumentBackEnd`/`Screen` pair, joined through one [`Wire`]: `client` is what a test calls
/// operations on, `session_client` is the same transport `deliver`ed into as replies arrive, and
/// `session` is the server's own means of pushing to `screen`.
struct Harness {
    back_end: DocumentBackEnd,
    client: DocumentSessionClient<ws_client::FrameSession>,
    screen: Screen,
    session: Session,
    session_client: ws_client::FrameSession,
    wire: Arc<Wire>,
}

impl Harness {
    fn new() -> Self {
        let wire = Arc::new(Wire::default());

        let server_bound = Arc::clone(&wire);
        let session_client = ws_client::FrameSession::new(move |text| {
            let queue = Arc::clone(&server_bound);
            async move {
                queue.to_server.lock().unwrap().push_back(text);
                Ok(())
            }
        });

        let client_bound = Arc::clone(&wire);
        let events =
            events_client::SessionEventsClient::new(events_client::FrameWriter::new(move |text| {
                let queue = Arc::clone(&client_bound);
                async move {
                    queue.to_client.lock().unwrap().push_back(text);
                    Ok(())
                }
            }));

        Self {
            back_end: DocumentBackEnd::new(),
            client: DocumentSessionClient::new(session_client.clone()),
            screen: Screen::new(),
            session: Session { events },
            session_client,
            wire,
        }
    }
}

/// Hands every frame queued in one direction to the other side, in order: a frame bound for the
/// server is answered — or merely dispatched — by both services registered on it, the first
/// `Some` reply winning; a frame bound for the client updates `client`'s own correlation map and,
/// where it names `SessionEvents`, `screen`.
async fn pump(
    wire: &Wire,
    back_end: &DocumentBackEnd,
    session: &Session,
    client: &ws_client::FrameSession,
    screen: &Screen,
) {
    loop {
        let queued = wire.to_server.lock().unwrap().pop_front();
        let Some(text) = queued else { break };
        wire.crossed.lock().unwrap().push(text.clone());
        let answered = ws_transport::answer(&text, back_end, session)
            .await
            .or(events_transport::answer(&text, screen, &()).await);
        if let Some(reply) = answered {
            wire.to_client.lock().unwrap().push_back(reply);
        }
    }
    loop {
        let queued = wire.to_client.lock().unwrap().pop_front();
        let Some(text) = queued else { break };
        wire.crossed.lock().unwrap().push(text.clone());
        client.deliver(&text).await;
        events_transport::answer(&text, screen, &()).await;
    }
}

/// One poll, by hand — no executor, no waker that does anything but satisfy the signature.
fn poll_by_hand<Answered>(pinned: Pin<&mut Answered>) -> Poll<Answered::Output>
where
    Answered: Future,
{
    let mut polling = PollContext::from_waker(Waker::noop());
    pinned.poll(&mut polling)
}

/// Runs [`pump`] to completion once. Every send and dispatch in this harness resolves on its
/// first poll, so a pump that were still `Pending` would mean the assumption stopped holding.
fn pump_once(harness: &Harness) {
    let progressed = poll_by_hand(
        pin!(pump(
            &harness.wire,
            &harness.back_end,
            &harness.session,
            &harness.session_client,
            &harness.screen,
        ))
        .as_mut(),
    );
    assert_eq!(progressed, Poll::Ready(()));
}

/// Polls `call`, pumping the wire between polls, until it settles.
fn drive<Answered>(harness: &Harness, mut call: Pin<&mut Answered>) -> Answered::Output
where
    Answered: Future,
{
    loop {
        if let Poll::Ready(output) = poll_by_hand(call.as_mut()) {
            return output;
        }
        pump_once(harness);
    }
}

/// Every frame the wire has carried so far, parsed — in the order it crossed.
fn crossed_frames(harness: &Harness) -> Vec<serde_json::Value> {
    harness
        .wire
        .crossed
        .lock()
        .unwrap()
        .iter()
        .map(|raw| serde_json::from_str(raw).unwrap())
        .collect()
}

#[test]
fn a_successful_call_round_trips_through_the_wire() {
    let harness = Harness::new();
    let mut call = pin!(harness.client.watch(WatchRequest {
        document_id: "doc-1".to_owned(),
    }));
    let answered = drive(&harness, call.as_mut());
    assert_eq!(answered, Ok(WatchResult { accepted: true }));
    assert_eq!(harness.back_end.reached(), vec!["watch doc-1".to_owned()]);
    assert_eq!(
        crossed_frames(&harness),
        vec![
            serde_json::json!({
                "id": "1",
                "kind": "request",
                "operation": "watch",
                "payload": { "document_id": "doc-1" },
                "service": "DocumentSession",
            }),
            serde_json::json!({
                "id": "1",
                "kind": "reply",
                "ok": true,
                "service": "DocumentSession",
                "value": { "accepted": true },
            }),
        ],
    );
}

#[test]
fn a_declared_error_round_trips_through_the_wire() {
    let harness = Harness::new();
    let mut call = pin!(harness.client.watch(WatchRequest {
        document_id: "missing".to_owned(),
    }));
    let answered = drive(&harness, call.as_mut());
    assert_eq!(answered, Err(CallError::Operation(WatchError::NotFound)));
}

#[test]
fn header_in_and_header_out_round_trip_through_the_wire() {
    let harness = Harness::new();
    let mut call = pin!(harness.client.read_range(
        ReadRangeRequest {
            document_id: "doc-1".to_owned(),
        },
        Some("bytes=0-10".to_owned()),
    ));
    let answered = drive(&harness, call.as_mut());
    assert_eq!(
        answered,
        Ok((
            RangeResult {
                content: "range-of-doc-1".to_owned(),
            },
            "etag-1".to_owned(),
        )),
    );
    assert_eq!(
        harness.back_end.reached(),
        vec!["read_range doc-1 Some(\"bytes=0-10\")".to_owned()],
    );
    assert_eq!(
        crossed_frames(&harness),
        vec![
            serde_json::json!({
                "id": "1",
                "kind": "request",
                "operation": "read-range",
                "payload": { "document_id": "doc-1" },
                "headers": { "range": "bytes=0-10" },
                "service": "DocumentSession",
            }),
            serde_json::json!({
                "id": "1",
                "kind": "reply",
                "ok": true,
                "service": "DocumentSession",
                "headers": { "etag": "etag-1" },
                "value": { "content": "range-of-doc-1" },
            }),
        ],
    );
}

#[test]
fn an_unknown_operation_becomes_a_fault_through_the_wire() {
    let harness = Harness::new();
    harness.wire.to_server.lock().unwrap().push_back(
        r#"{"kind":"request","id":"1","service":"DocumentSession","operation":"nope","payload":{}}"#
            .to_owned(),
    );
    pump_once(&harness);
    let reply = crossed_frames(&harness).into_iter().nth(1).unwrap();
    assert_eq!(
        reply,
        serde_json::json!({
            "error": {
                "fault": {
                    "detail": "the service answers to no operation by that name",
                    "kind": "unknown-operation",
                    "operation": "nope",
                },
                "isServiceFault": true,
            },
            "id": "1",
            "kind": "reply",
            "ok": false,
            "service": "DocumentSession",
        }),
    );
}

#[test]
fn a_failed_validation_becomes_a_fault_through_the_wire() {
    let harness = Harness::new();
    harness.wire.to_server.lock().unwrap().push_back(
        r#"{"kind":"request","id":"1","service":"DocumentSession","operation":"watch","payload":{"document_id":7}}"#
            .to_owned(),
    );
    pump_once(&harness);
    let reply = crossed_frames(&harness).into_iter().nth(1).unwrap();
    assert_eq!(
        reply,
        serde_json::json!({
            "error": {
                "fault": {
                    "detail": "invalid type: integer `7`, expected a string",
                    "kind": "failed-validation",
                    "operation": "watch",
                },
                "isServiceFault": true,
            },
            "id": "1",
            "kind": "reply",
            "ok": false,
            "service": "DocumentSession",
        }),
    );
    assert!(harness.back_end.reached().is_empty());
}

#[test]
fn a_one_way_operation_is_dispatched_and_answers_nothing_through_the_wire() {
    let harness = Harness::new();
    let sent = poll_by_hand(
        pin!(harness.client.touch(TouchRequest {
            document_id: "doc-1".to_owned(),
        }))
        .as_mut(),
    );
    assert_eq!(sent, Poll::Ready(Ok(())));
    pump_once(&harness);
    assert_eq!(harness.back_end.reached(), vec!["touch doc-1".to_owned()]);
    assert!(harness.wire.to_client.lock().unwrap().is_empty());
}

#[test]
fn a_request_naming_a_one_way_operation_gets_the_default_success_through_the_wire() {
    let harness = Harness::new();
    harness.wire.to_server.lock().unwrap().push_back(
        r#"{"kind":"request","id":"1","service":"DocumentSession","operation":"touch","payload":{"document_id":"doc-1"}}"#
            .to_owned(),
    );
    pump_once(&harness);
    let reply = crossed_frames(&harness).into_iter().nth(1).unwrap();
    assert_eq!(
        reply,
        serde_json::json!({
            "id": "1",
            "kind": "reply",
            "ok": true,
            "service": "DocumentSession",
            "value": null,
        }),
    );
    assert_eq!(harness.back_end.reached(), vec!["touch doc-1".to_owned()]);
}

#[test]
fn a_unit_success_round_trips_through_the_wire() {
    let harness = Harness::new();
    let mut call = pin!(harness.client.unwatch(UnwatchRequest {
        document_id: "doc-1".to_owned(),
    }));
    let answered = drive(&harness, call.as_mut());
    assert_eq!(answered, Ok(()));
    assert_eq!(harness.back_end.reached(), vec!["unwatch doc-1".to_owned()]);
}

#[test]
fn a_push_from_the_servers_events_client_reaches_screen() {
    let harness = Harness::new();
    let pushed = poll_by_hand(
        pin!(
            harness
                .session
                .events
                .document_changed(DocumentChangedRequest {
                    document_id: "doc-1".to_owned(),
                    version: 4,
                })
        )
        .as_mut(),
    );
    assert_eq!(pushed, Poll::Ready(Ok(())));
    pump_once(&harness);
    assert_eq!(
        harness.screen.received(),
        vec![DocumentChangedRequest {
            document_id: "doc-1".to_owned(),
            version: 4,
        }],
    );
}

#[test]
fn a_ping_is_answered_once_even_with_two_services_registered() {
    let harness = Harness::new();
    harness
        .wire
        .to_server
        .lock()
        .unwrap()
        .push_back(ws_client::ping_frame());
    pump_once(&harness);
    assert_eq!(
        crossed_frames(&harness),
        vec![
            serde_json::json!({ "kind": "ping" }),
            serde_json::json!({ "kind": "pong" }),
        ],
    );
}

#[test]
fn a_stray_reply_is_ignored_and_state_stays_usable() {
    let harness = Harness::new();
    let stray = r#"{"kind":"reply","id":"999","service":"DocumentSession","ok":true,"value":{"accepted":true}}"#;

    let decoded = ws_transport::Frame::decode(stray).unwrap();
    assert!(matches!(decoded, ws_transport::Frame::Reply { .. }));
    if let ws_transport::Frame::Reply {
        id,
        service,
        envelope,
    } = decoded
    {
        assert_eq!(id, "999");
        assert_eq!(service, "DocumentSession");
        assert_eq!(
            envelope.get("value"),
            Some(&serde_json::json!({ "accepted": true })),
        );
    }

    harness
        .wire
        .to_client
        .lock()
        .unwrap()
        .push_back(stray.to_owned());
    pump_once(&harness);
    assert!(harness.wire.to_client.lock().unwrap().is_empty());

    let mut call = pin!(harness.client.watch(WatchRequest {
        document_id: "doc-1".to_owned(),
    }));
    assert_eq!(
        drive(&harness, call.as_mut()),
        Ok(WatchResult { accepted: true })
    );
}

#[test]
fn close_settles_a_waiting_call_with_a_transport_failure_fault() {
    let harness = Harness::new();
    let mut call = pin!(harness.client.watch(WatchRequest {
        document_id: "doc-1".to_owned(),
    }));
    assert_eq!(poll_by_hand(call.as_mut()), Poll::Pending);
    harness
        .session_client
        .close("the socket closed before the reply arrived");
    let fault = match poll_by_hand(call.as_mut()) {
        Poll::Ready(Err(CallError::Fault(fault))) => Some(fault),
        Poll::Pending | Poll::Ready(Ok(_) | Err(CallError::Operation(_))) => None,
    }
    .unwrap();
    assert_eq!(fault.kind(), ServiceFaultKind::TransportFailure);
    assert_eq!(fault.detail(), "the socket closed before the reply arrived");
}

// The scenarios above never reach every corner of four independently generated macro
// expansions — a dispatcher's own copy of a reply it never answers with, a client's own copy of
// a request it only ever sends, `SessionEvents`'s unused browser-side `FrameSession` half. What
// follows exercises what the wire never happened to.

#[test]
fn every_modules_ping_frame_encodes_the_same_liveness_probe() {
    let ping = serde_json::json!({ "kind": "ping" });
    for encoded in [
        ws_transport::ping_frame(),
        ws_client::ping_frame(),
        events_client::ping_frame(),
        events_transport::ping_frame(),
    ] {
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&encoded).unwrap(),
            ping
        );
    }
}

#[test]
fn ws_client_frame_decode_reads_request_and_notify_frames() {
    let decoded = ws_client::Frame::decode(
        r#"{"kind":"request","id":"9","service":"DocumentSession","operation":"watch","payload":{"document_id":"doc-1"},"headers":{"range":"bytes=0-1"}}"#,
    )
    .unwrap();
    assert!(matches!(decoded, ws_client::Frame::Request { .. }));
    if let ws_client::Frame::Request {
        id,
        service,
        message,
    } = decoded
    {
        assert_eq!(id, "9");
        assert_eq!(service, "DocumentSession");
        assert_eq!(message.operation(), "watch");
        assert_eq!(message.payload(), br#"{"document_id":"doc-1"}"#);
        assert_eq!(
            message.headers(),
            &[("range".to_owned(), "\"bytes=0-1\"".to_owned())]
        );
    }

    let notified = ws_client::Frame::decode(
        r#"{"kind":"notify","service":"DocumentSession","operation":"touch","payload":{"document_id":"doc-1"}}"#,
    )
    .unwrap();
    assert!(matches!(notified, ws_client::Frame::Notify { .. }));
    if let ws_client::Frame::Notify { service, message } = notified {
        assert_eq!(service, "DocumentSession");
        assert_eq!(message.operation(), "touch");
    }
}

/// `FrameWriter` keeps no correlation map, so a call expecting an answer is refused outright,
/// naming the operation — proven directly since no scenario above asks a `FrameWriter` for one.
#[test]
fn a_frame_writer_asked_for_request_answers_err_naming_the_operation() {
    let writer = ws_client::FrameWriter::new(|_text| async { Ok(()) });
    let asked = WatchRequest {
        document_id: "doc-1".to_owned(),
    };
    let refused = match poll_by_hand(
        pin!(ws_client::Transport::request(
            &writer,
            "watch",
            &asked,
            Vec::new()
        ))
        .as_mut(),
    ) {
        Poll::Ready(Err(refused)) => Some(refused),
        Poll::Pending | Poll::Ready(Ok(_)) => None,
    }
    .unwrap();
    assert!(
        refused.contains("watch"),
        "the refusal names the operation. Got: {refused}"
    );
}

#[test]
fn a_document_session_client_exposes_the_transport_it_was_bound_to() {
    let harness = Harness::new();
    let pushed = poll_by_hand(
        pin!(ws_client::Transport::notify(
            harness.client.transport(),
            "probe",
            &(),
            Vec::new(),
        ))
        .as_mut(),
    );
    assert_eq!(pushed, Poll::Ready(Ok(())));
    let popped = harness.wire.to_server.lock().unwrap().pop_back().unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&popped).unwrap(),
        serde_json::json!({
            "kind": "notify",
            "operation": "probe",
            "payload": null,
            "service": "DocumentSession",
        }),
    );
}

/// `SessionEvents` never places a `FrameSession` in production — the browser only ever replies
/// through the `FrameWriter` half `Session::events` already exercises — but the correlation half
/// is still published, so it is proven directly here the way `ws_client`'s own is proven through
/// the scenarios above.
#[test]
fn events_client_frame_session_completes_a_request_and_reply_round_trip_and_answers_a_ping() {
    let sent = Arc::new(Mutex::new(Vec::<String>::new()));
    let mailbox = Arc::clone(&sent);
    let session = events_client::FrameSession::new(move |text| {
        let inbox = Arc::clone(&mailbox);
        async move {
            inbox.lock().unwrap().push(text);
            Ok(())
        }
    });

    let mut call = pin!(events_client::Transport::request(
        &session,
        "probe",
        &(),
        Vec::new(),
    ));
    assert_eq!(poll_by_hand(call.as_mut()), Poll::Pending);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(sent.lock().unwrap().last().unwrap()).unwrap(),
        serde_json::json!({
            "id": "1",
            "kind": "request",
            "operation": "probe",
            "payload": null,
            "service": "SessionEvents",
        }),
    );

    let delivered = poll_by_hand(
        pin!(session.deliver(r#"{"kind":"reply","id":"1","service":"SessionEvents","ok":true}"#))
            .as_mut(),
    );
    assert_eq!(delivered, Poll::Ready(()));
    let (encoded, headers) = match poll_by_hand(call.as_mut()) {
        Poll::Ready(Ok(answered)) => Some(answered),
        Poll::Pending | Poll::Ready(Err(_)) => None,
    }
    .unwrap();
    assert_eq!(headers, Vec::new());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&encoded).unwrap(),
        serde_json::json!({ "ok": true }),
    );

    let pinged = poll_by_hand(pin!(session.deliver(r#"{"kind":"ping"}"#)).as_mut());
    assert_eq!(pinged, Poll::Ready(()));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(sent.lock().unwrap().last().unwrap()).unwrap(),
        serde_json::json!({ "kind": "pong" }),
    );

    session.close("done");
}

#[test]
fn events_client_frame_decode_reads_request_and_notify_frames() {
    let decoded = events_client::Frame::decode(
        r#"{"kind":"request","id":"9","service":"SessionEvents","operation":"probe","payload":{"n":1}}"#,
    )
    .unwrap();
    assert!(matches!(decoded, events_client::Frame::Request { .. }));
    if let events_client::Frame::Request {
        id,
        service,
        message,
    } = decoded
    {
        assert_eq!(id, "9");
        assert_eq!(service, "SessionEvents");
        assert_eq!(message.operation(), "probe");
        assert_eq!(message.payload(), br#"{"n":1}"#);
    }

    let notified = events_client::Frame::decode(
        r#"{"kind":"notify","service":"SessionEvents","operation":"probe","payload":{"n":1}}"#,
    )
    .unwrap();
    assert!(matches!(notified, events_client::Frame::Notify { .. }));
    if let events_client::Frame::Notify { service, message } = notified {
        assert_eq!(service, "SessionEvents");
        assert_eq!(message.operation(), "probe");
    }
}

#[test]
fn a_session_events_client_exposes_the_transport_it_was_bound_to() {
    let harness = Harness::new();
    let refused = match poll_by_hand(
        pin!(events_client::Transport::request(
            harness.session.events.transport(),
            "probe",
            &(),
            Vec::new(),
        ))
        .as_mut(),
    ) {
        Poll::Ready(Err(refused)) => Some(refused),
        Poll::Pending | Poll::Ready(Ok(_)) => None,
    }
    .unwrap();
    assert!(refused.contains("probe"), "got: {refused}");
}

/// `SessionEvents` declares one operation and it is one-way, so nothing this crate dispatches
/// ever answers through `Reply::send` — a real reply-shaped operation would, exactly as
/// `DocumentSession`'s own scenarios above already prove. Proven directly here instead, alongside
/// the reply frame this dispatcher only ever decodes and never answers with.
#[test]
fn events_transport_frame_reply_writes_a_send_envelope_and_decodes_a_reply_frame() {
    use events_transport::Reply as _;

    let reply = events_transport::FrameReply::new("7");
    let answered =
        session_events_schema::Answered::answering(Ok::<_, ()>(DocumentChangedRequest {
            document_id: "doc-9".to_owned(),
            version: 1,
        }));
    let written = poll_by_hand(pin!(reply.send(answered, Vec::new())).as_mut());
    assert_eq!(written, Poll::Ready(()));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&reply.into_text()).unwrap(),
        serde_json::json!({
            "id": "7",
            "kind": "reply",
            "ok": true,
            "service": "SessionEvents",
            "value": { "document_id": "doc-9", "version": 1_u32 },
        }),
    );

    let decoded = events_transport::Frame::decode(
        r#"{"kind":"reply","id":"1","service":"SessionEvents","ok":true,"value":null}"#,
    )
    .unwrap();
    assert!(matches!(decoded, events_transport::Frame::Reply { .. }));
    if let events_transport::Frame::Reply {
        id,
        service,
        envelope,
    } = decoded
    {
        assert_eq!(id, "1");
        assert_eq!(service, "SessionEvents");
        assert_eq!(envelope.get("value"), Some(&serde_json::Value::Null));
    }
}
