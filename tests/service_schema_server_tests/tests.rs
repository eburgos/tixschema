//! A service with one request-and-reply operation and one one-way operation, driven through the
//! server macro's own `dispatch` — the same items the dispatcher macro emits, reused unchanged —
//! and `serve_until` named at a concrete type, since a real `lapin::Channel` cannot be built
//! without a connection.

#![cfg(feature = "serde")]

use crate::amqp_server;
use core::future::{Future, Ready, ready};
use core::mem::size_of_val;
use core::pin::pin;
use core::task::{Context as PollContext, Poll, Waker};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tixschema::{model_schema, service_schema};

#[model_schema()]
#[derive(Deserialize, Serialize)]
pub struct PingRequest {
    pub organization_id: String,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
pub struct PingResponse {
    pub credits: u32,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum PingError {
    DbError,
}

#[service_schema(transports = ["amqp_rpc"])]
pub trait PingService<Ctx> {
    /// No reply, so the arm still has to settle the delivery without publishing anything.
    #[service_schema_op(one_way)]
    async fn note(&self, ctx: &Ctx, req: PingRequest);

    /// A request-and-reply operation, dispatched with the span the consumer loop opens entered.
    async fn ping(&self, ctx: &Ctx, req: PingRequest) -> Result<PingResponse, PingError>;
}

/// Answers the contract against the server macro's own `Context`, the same struct
/// [`amqp_server::serve_until`] builds one of per delivery.
pub struct PingBackEnd;

impl PingService<amqp_server::Context> for PingBackEnd {
    async fn note(&self, ctx: &amqp_server::Context, req: PingRequest) {
        let _entered = ctx.logger.enter();
        let _read = ready(req.organization_id.len()).await;
    }

    async fn ping(
        &self,
        ctx: &amqp_server::Context,
        req: PingRequest,
    ) -> Result<PingResponse, PingError> {
        let _entered = ctx.logger.enter();
        ready(()).await;
        if req.organization_id == "unlucky" {
            Err(PingError::DbError)
        } else {
            Ok(PingResponse { credits: 7 })
        }
    }
}

/// One of the two ways an arm answers. Exactly one lands per request-and-reply dispatch, and none
/// at all where the one-way operation reached its implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Settled {
    Fault(ping_service_schema::ServiceFault),
    Sent(String),
}

/// Records how each message was settled instead of publishing anything, standing in for the
/// `ReplyHandle` a real delivery would be answered through.
pub struct ProbeReply {
    settled: Mutex<Vec<Settled>>,
}

impl amqp_server::Reply for ProbeReply {
    async fn fault(&self, fault: ping_service_schema::ServiceFault) {
        ready(()).await;
        self.record(Settled::Fault(fault));
    }

    async fn send<T>(&self, value: T)
    where
        T: Serialize + Send,
    {
        ready(()).await;
        self.record(Settled::Sent(serde_json::to_string(&value).unwrap()));
    }
}

impl ProbeReply {
    fn new() -> Self {
        Self {
            settled: Mutex::new(Vec::new()),
        }
    }

    fn record(&self, what: Settled) {
        self.settled.lock().unwrap().push(what);
    }

    fn settled(&self) -> Vec<Settled> {
        self.settled.lock().unwrap().clone()
    }
}

/// The probe never suspends, so one poll answers it; `None` says an assumption about the bodies
/// above stopped holding rather than that the runtime is missing.
fn poll_once<Answered>(answering: Answered) -> Option<Answered::Output>
where
    Answered: Future,
{
    let mut pinned = pin!(answering);
    let mut polling = PollContext::from_waker(Waker::noop());
    match pinned.as_mut().poll(&mut polling) {
        Poll::Ready(answer) => Some(answer),
        Poll::Pending => None,
    }
}

fn dispatched(operation: &str, payload: &str) -> Vec<Settled> {
    let service = PingBackEnd;
    let reply = ProbeReply::new();
    poll_once(amqp_server::dispatch(
        &service,
        &amqp_server::Context {
            logger: tracing::Span::none(),
        },
        &amqp_server::IncomingMessage::new(operation.to_owned(), payload.as_bytes().to_vec()),
        &reply,
    ))
    .unwrap();
    reply.settled()
}

#[test]
fn the_server_macro_s_dispatch_answers_the_same_way_the_dispatcher_macro_s_does() {
    let settled = dispatched("ping", r#"{"organization_id":"acme"}"#);
    assert_eq!(
        settled,
        vec![Settled::Sent(
            r#"{"ok":true,"value":{"credits":7}}"#.to_owned()
        )],
        "got: {settled:?}"
    );
}

#[test]
fn the_operation_s_own_error_rides_in_the_failure_arm_rather_than_becoming_a_fault() {
    let settled = dispatched("ping", r#"{"organization_id":"unlucky"}"#);
    assert_eq!(
        settled,
        vec![Settled::Sent(
            r#"{"error":{"errorCode":"db-error"},"ok":false}"#.to_owned()
        )],
        "got: {settled:?}"
    );
}

#[test]
fn a_one_way_operation_settles_without_publishing_anything() {
    let settled = dispatched("note", r#"{"organization_id":"acme"}"#);
    assert!(settled.is_empty(), "got: {settled:?}");
}

#[test]
fn a_name_nothing_answers_to_becomes_a_fault_through_the_same_handle() {
    let settled = dispatched("get-the-balance", r#"{"organization_id":"acme"}"#);
    assert_eq!(settled.len(), 1, "got: {settled:?}");
    assert!(
        matches!(&settled[0], Settled::Fault(reported)
            if reported.kind() == ping_service_schema::ServiceFaultKind::UnknownOperation),
        "got: {settled:?}"
    );
}

/// A real `lapin::Channel` cannot be built without a connection, so `serve_until` is named at a
/// concrete service and shutdown future rather than run. Compiling this line is what proves every
/// `::lapin`, `::tokio` and `::futures` path the consumer loop names resolves, and what keeps the
/// loop and the framing it calls from being dead code in this binary.
#[test]
fn serve_until_is_reachable_at_a_concrete_service_and_shutdown_future() {
    let named = amqp_server::serve_until::<PingBackEnd, Ready<()>>;
    assert_eq!(
        size_of_val(&named),
        0,
        "a bare fn item names nothing to store"
    );
}
