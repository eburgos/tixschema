//! `DocumentSession`: a `watch`/`unwatch` pair covering the success, declared-error and
//! unit-success shapes; `touch`, one-way, for the request/notify split and the default-success
//! fallback; `read_range`, a `header_in`/`header_out` pair, for the headers round trip.
//! `SessionEvents`: the one-way, browser-implemented service a server pushes to over a
//! `FrameWriter` rather than calls.
//!
//! `answer` is driven directly against text frames — no socket, no adapter — the same way
//! `dispatch` is driven directly against an `IncomingMessage` elsewhere in this crate. The client
//! tests at the foot of this file drive the generated `DocumentSessionClient` over a `FrameSession`
//! polled by hand, and `SessionEventsClient` over a `FrameWriter`.

#![cfg(feature = "serde")]

use crate::document_session_schema::{CallError, ServiceFaultKind};
use crate::events_client::{self, SessionEventsClient};
use crate::ws_client::{self, DocumentSessionClient, FrameSession, Transport as _};
use crate::ws_transport::{self, Frame};
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

/// Writes down every call that reached it, so a test can say what the dispatcher let through.
pub struct DocumentBackEnd {
    reached: Mutex<Vec<String>>,
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
#[derive(Deserialize, Serialize)]
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

/// The browser side of `SessionEvents`, which no Rust program actually implements in production —
/// this one exists only so the trait a server's client calls into is not itself unreachable in a
/// harness that places no dispatcher for it.
struct SessionEventsBrowser;

/// Every text frame a mock send function captured, in the order it was sent.
#[derive(Clone, Default)]
struct Sent(Arc<Mutex<Vec<String>>>);

// Every method below is synchronous work wrapped in an already-ready `Future`: nothing here
// waits on anything, so `ready` is the whole of what implementing the trait's async signature
// takes, with no `async fn` sugar over a body that never awaits.
impl DocumentSession<()> for DocumentBackEnd {
    fn read_range(
        &self,
        _ctx: &(),
        req: ReadRangeRequest,
        byte_range: Option<String>,
    ) -> impl Future<Output = Result<(RangeResult, String), RangeError>> {
        self.reach(format!("read_range {} {byte_range:?}", req.document_id));
        let outcome = if req.document_id == "missing" {
            Err(RangeError::NotFound)
        } else {
            Ok((
                RangeResult {
                    content: format!("range-of-{}", req.document_id),
                },
                "etag-1".to_owned(),
            ))
        };
        ready(outcome)
    }

    fn touch(&self, _ctx: &(), req: TouchRequest) -> impl Future<Output = ()> {
        self.reach(format!("touch {}", req.document_id));
        ready(())
    }

    fn unwatch(
        &self,
        _ctx: &(),
        req: UnwatchRequest,
    ) -> impl Future<Output = Result<(), UnwatchError>> {
        self.reach(format!("unwatch {}", req.document_id));
        let outcome = if req.document_id == "missing" {
            Err(UnwatchError::NotFound)
        } else {
            Ok(())
        };
        ready(outcome)
    }

    fn watch(
        &self,
        _ctx: &(),
        req: WatchRequest,
    ) -> impl Future<Output = Result<WatchResult, WatchError>> {
        self.reach(format!("watch {}", req.document_id));
        let outcome = if req.document_id == "missing" {
            Err(WatchError::NotFound)
        } else {
            Ok(WatchResult { accepted: true })
        };
        ready(outcome)
    }
}

impl SessionEvents<()> for SessionEventsBrowser {
    async fn document_changed(&self, _ctx: &(), req: DocumentChangedRequest) {
        let _read = ready(req.document_id.len()).await;
    }
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

impl Sent {
    /// The most recently sent frame, parsed — key order is never significant.
    fn last(&self) -> serde_json::Value {
        let raw = self.0.lock().unwrap().last().cloned().unwrap();
        serde_json::from_str(&raw).unwrap()
    }

    fn push(&self, text: String) {
        self.0.lock().unwrap().push(text);
    }
}

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

/// Drives `answer` against one text frame. Every implementation here answers on its first poll,
/// so `flatten` losing the pending case costs nothing real — a genuinely pending result would
/// read back as `None`, the same as a frame this transport does not answer.
fn answer(service: &DocumentBackEnd, text: &str) -> Option<String> {
    poll_once(ws_transport::answer(text, service, &())).flatten()
}

/// `raw`, parsed as JSON, equals `expected` — key order is never significant.
fn assert_reply(raw: &str, expected: &serde_json::Value) {
    let parsed: serde_json::Value = serde_json::from_str(raw).unwrap();
    assert_eq!(&parsed, expected, "raw reply: {raw}");
}

/// A `FrameSession` over a send function that records every frame rather than putting it on a
/// real socket.
fn frame_session() -> (FrameSession, Sent) {
    let sent = Sent::default();
    let mailbox = sent.clone();
    let session = FrameSession::new(move |text| {
        let inbox = mailbox.clone();
        async move {
            inbox.push(text);
            Ok(())
        }
    });
    (session, sent)
}

/// A `FrameWriter` over the same kind of recording send function. `ws_client`'s own, for the
/// `DocumentSession` client — `FrameWriter` is a private nominal type per macro invocation, so a
/// `SessionEvents` push needs [`events_frame_writer`] instead.
fn frame_writer() -> (ws_client::FrameWriter, Sent) {
    let sent = Sent::default();
    let mailbox = sent.clone();
    let writer = ws_client::FrameWriter::new(move |text| {
        let inbox = mailbox.clone();
        async move {
            inbox.push(text);
            Ok(())
        }
    });
    (writer, sent)
}

/// The `events_client` module's own `FrameSession`, for the round trip its own service never
/// declares a reply operation to exercise otherwise.
fn events_frame_session() -> (events_client::FrameSession, Sent) {
    let sent = Sent::default();
    let mailbox = sent.clone();
    let session = events_client::FrameSession::new(move |text| {
        let inbox = mailbox.clone();
        async move {
            inbox.push(text);
            Ok(())
        }
    });
    (session, sent)
}

/// The `events_client` module's own `FrameWriter`, for a `SessionEventsClient` push.
fn events_frame_writer() -> (events_client::FrameWriter, Sent) {
    let sent = Sent::default();
    let mailbox = sent.clone();
    let writer = events_client::FrameWriter::new(move |text| {
        let inbox = mailbox.clone();
        async move {
            inbox.push(text);
            Ok(())
        }
    });
    (writer, sent)
}

/// One poll, by hand — no executor, no waker that does anything but satisfy the signature.
fn poll_by_hand<Answered>(pinned: Pin<&mut Answered>) -> Poll<Answered::Output>
where
    Answered: Future,
{
    let mut polling = PollContext::from_waker(Waker::noop());
    pinned.poll(&mut polling)
}

#[test]
fn a_successful_call_replies_with_the_declared_value() {
    let service = DocumentBackEnd::new();
    let reply = answer(
        &service,
        r#"{"kind":"request","id":"1","service":"DocumentSession","operation":"watch","payload":{"document_id":"doc-1"}}"#,
    )
    .unwrap();
    assert_reply(
        &reply,
        &serde_json::json!({
            "id": "1",
            "kind": "reply",
            "ok": true,
            "service": "DocumentSession",
            "value": { "accepted": true },
        }),
    );
    assert_eq!(service.reached(), vec!["watch doc-1".to_owned()]);
}

#[test]
fn a_declared_error_replies_with_the_operations_own_error_shape() {
    let service = DocumentBackEnd::new();
    let reply = answer(
        &service,
        r#"{"kind":"request","id":"2","service":"DocumentSession","operation":"watch","payload":{"document_id":"missing"}}"#,
    )
    .unwrap();
    assert_reply(
        &reply,
        &serde_json::json!({
            "error": { "errorCode": "not-found" },
            "id": "2",
            "kind": "reply",
            "ok": false,
            "service": "DocumentSession",
        }),
    );
}

#[test]
fn an_operation_nothing_answers_to_becomes_an_unknown_operation_fault() {
    let service = DocumentBackEnd::new();
    let reply = answer(
        &service,
        r#"{"kind":"request","id":"3","service":"DocumentSession","operation":"nope","payload":{}}"#,
    )
    .unwrap();
    assert_reply(
        &reply,
        &serde_json::json!({
            "error": {
                "fault": {
                    "detail": "the service answers to no operation by that name",
                    "kind": "unknown-operation",
                    "operation": "nope",
                },
                "isServiceFault": true,
            },
            "id": "3",
            "kind": "reply",
            "ok": false,
            "service": "DocumentSession",
        }),
    );
}

#[test]
fn a_payload_that_is_not_the_message_becomes_a_failed_validation_fault() {
    let service = DocumentBackEnd::new();
    let reply = answer(
        &service,
        r#"{"kind":"request","id":"4","service":"DocumentSession","operation":"watch","payload":{"document_id":7}}"#,
    )
    .unwrap();
    assert_reply(
        &reply,
        &serde_json::json!({
            "error": {
                "fault": {
                    "detail": "invalid type: integer `7`, expected a string",
                    "kind": "failed-validation",
                    "operation": "watch",
                },
                "isServiceFault": true,
            },
            "id": "4",
            "kind": "reply",
            "ok": false,
            "service": "DocumentSession",
        }),
    );
}

#[test]
fn a_unit_success_replies_ok_true_value_null() {
    let service = DocumentBackEnd::new();
    let reply = answer(
        &service,
        r#"{"kind":"request","id":"5","service":"DocumentSession","operation":"unwatch","payload":{"document_id":"doc-1"}}"#,
    )
    .unwrap();
    assert_reply(
        &reply,
        &serde_json::json!({
            "id": "5",
            "kind": "reply",
            "ok": true,
            "service": "DocumentSession",
            "value": null,
        }),
    );
}

#[test]
fn a_ping_is_answered_with_a_pong() {
    let service = DocumentBackEnd::new();
    let reply = answer(&service, r#"{"kind":"ping"}"#).unwrap();
    assert_reply(&reply, &serde_json::json!({ "kind": "pong" }));
}

#[test]
fn a_notify_is_dispatched_and_answers_nothing() {
    let service = DocumentBackEnd::new();
    let reply = answer(
        &service,
        r#"{"kind":"notify","service":"DocumentSession","operation":"touch","payload":{"document_id":"doc-9"}}"#,
    );
    assert_eq!(reply, None);
    assert_eq!(service.reached(), vec!["touch doc-9".to_owned()]);
}

#[test]
fn a_frame_for_another_service_answers_nothing() {
    let service = DocumentBackEnd::new();
    let reply = answer(
        &service,
        r#"{"kind":"request","id":"9","service":"OtherService","operation":"watch","payload":{}}"#,
    );
    assert_eq!(reply, None);
    assert!(service.reached().is_empty());
}

#[test]
fn text_that_is_not_json_answers_nothing() {
    let service = DocumentBackEnd::new();
    assert_eq!(answer(&service, "not json at all"), None);
}

/// A `request` frame naming a one-way operation is a mismatch: the arm never calls `send` or
/// `fault`, so the reply falls back to the default success rather than leaving the caller with
/// nothing.
#[test]
fn a_request_naming_a_one_way_operation_gets_the_default_success() {
    let service = DocumentBackEnd::new();
    let reply = answer(
        &service,
        r#"{"kind":"request","id":"10","service":"DocumentSession","operation":"touch","payload":{"document_id":"doc-1"}}"#,
    )
    .unwrap();
    assert_reply(
        &reply,
        &serde_json::json!({
            "id": "10",
            "kind": "reply",
            "ok": true,
            "service": "DocumentSession",
            "value": null,
        }),
    );
    assert_eq!(service.reached(), vec!["touch doc-1".to_owned()]);
}

#[test]
fn headers_round_trip_through_a_request_frame_and_its_reply() {
    let service = DocumentBackEnd::new();
    let reply = answer(
        &service,
        r#"{"kind":"request","id":"11","service":"DocumentSession","operation":"read-range","payload":{"document_id":"doc-1"},"headers":{"range":"bytes=0-10"}}"#,
    )
    .unwrap();
    assert_reply(
        &reply,
        &serde_json::json!({
            "headers": { "etag": "etag-1" },
            "id": "11",
            "kind": "reply",
            "ok": true,
            "service": "DocumentSession",
            "value": { "content": "range-of-doc-1" },
        }),
    );
    assert_eq!(
        service.reached(),
        vec!["read_range doc-1 Some(\"bytes=0-10\")".to_owned()]
    );
}

/// A header nothing carried decodes as the argument's own absent value, exactly like `amqp_rpc`'s
/// own `header_in` binding.
#[test]
fn a_header_in_binding_nothing_carried_decodes_as_the_arguments_own_absent_value() {
    let service = DocumentBackEnd::new();
    let reply = answer(
        &service,
        r#"{"kind":"request","id":"12","service":"DocumentSession","operation":"read-range","payload":{"document_id":"doc-2"}}"#,
    )
    .unwrap();
    assert_reply(
        &reply,
        &serde_json::json!({
            "headers": { "etag": "etag-1" },
            "id": "12",
            "kind": "reply",
            "ok": true,
            "service": "DocumentSession",
            "value": { "content": "range-of-doc-2" },
        }),
    );
    assert_eq!(service.reached(), vec!["read_range doc-2 None".to_owned()]);
}

/// `ping_frame` is the client macro's — a session sends one, a dispatcher only ever answers one —
/// but the two share one codec, so the dispatcher publishes it too.
#[test]
fn ping_frame_encodes_the_kind_ping_frame() {
    assert_eq!(ws_transport::ping_frame(), r#"{"kind":"ping"}"#);
}

/// `Frame::Reply` is the client macro's own frame kind — this dispatcher never answers with one,
/// it only decodes one, which `answer` itself has no reason to do.
#[test]
fn frame_decode_reads_a_reply_frame_into_its_id_service_and_envelope() {
    let decoded = Frame::decode(
        r#"{"kind":"reply","id":"1","service":"DocumentSession","ok":true,"value":{"accepted":true}}"#,
    )
    .unwrap();
    assert!(matches!(decoded, Frame::Reply { .. }));
    if let Frame::Reply {
        id,
        service,
        envelope,
    } = decoded
    {
        assert_eq!(id, "1");
        assert_eq!(service, "DocumentSession");
        assert_eq!(
            envelope.get("value"),
            Some(&serde_json::json!({ "accepted": true }))
        );
    }
}

/// A request-and-reply call is `Pending` until its reply is delivered, and reads back the
/// declared value once it is — the frame it sent along the way carries the id `FrameSession`
/// drew from its own counter.
#[test]
fn a_request_is_pending_until_delivered_then_ready_with_the_declared_value() {
    let (session, sent) = frame_session();
    let client = DocumentSessionClient::new(session.clone());
    let mut call = pin!(client.watch(WatchRequest {
        document_id: "doc-1".to_owned(),
    }));
    assert_eq!(poll_by_hand(call.as_mut()), Poll::Pending);
    assert_eq!(
        sent.last(),
        serde_json::json!({
            "id": "1",
            "kind": "request",
            "operation": "watch",
            "payload": { "document_id": "doc-1" },
            "service": "DocumentSession",
        }),
    );
    let delivered = poll_by_hand(
        pin!(session.deliver(
            r#"{"kind":"reply","id":"1","service":"DocumentSession","ok":true,"value":{"accepted":true}}"#
        ))
        .as_mut(),
    );
    assert_eq!(delivered, Poll::Ready(()));
    assert_eq!(
        poll_by_hand(call.as_mut()),
        Poll::Ready(Ok(WatchResult { accepted: true })),
    );
}

/// A reply carrying the operation's own declared error reads back as `CallError::Operation`
/// rather than a fault.
#[test]
fn a_declared_error_reply_reads_back_as_the_operations_own_error() {
    let (session, _sent) = frame_session();
    let client = DocumentSessionClient::new(session.clone());
    let mut call = pin!(client.watch(WatchRequest {
        document_id: "missing".to_owned(),
    }));
    assert_eq!(poll_by_hand(call.as_mut()), Poll::Pending);
    let delivered = poll_by_hand(
        pin!(session.deliver(
            r#"{"kind":"reply","id":"1","service":"DocumentSession","ok":false,"error":{"errorCode":"not-found"}}"#
        ))
        .as_mut(),
    );
    assert_eq!(delivered, Poll::Ready(()));
    assert_eq!(
        poll_by_hand(call.as_mut()),
        Poll::Ready(Err(CallError::Operation(WatchError::NotFound))),
    );
}

/// `close` fails every request still waiting with the words it was given, which the generated
/// client turns into a `transport-failure` fault.
#[test]
fn close_fails_a_waiting_request_with_a_transport_failure_fault() {
    let (session, _sent) = frame_session();
    let client = DocumentSessionClient::new(session.clone());
    let mut call = pin!(client.watch(WatchRequest {
        document_id: "doc-1".to_owned(),
    }));
    assert_eq!(poll_by_hand(call.as_mut()), Poll::Pending);
    session.close("the socket closed before the reply arrived");
    let fault = match poll_by_hand(call.as_mut()) {
        Poll::Ready(Err(CallError::Fault(fault))) => Some(fault),
        Poll::Pending | Poll::Ready(Ok(_) | Err(CallError::Operation(_))) => None,
    }
    .unwrap();
    assert_eq!(fault.kind(), ServiceFaultKind::TransportFailure);
    assert_eq!(fault.detail(), "the socket closed before the reply arrived");
}

/// `deliver` answers a ping with a pong through the very send function a request would have used.
#[test]
fn deliver_answers_a_ping_with_a_pong() {
    let (session, sent) = frame_session();
    let delivered = poll_by_hand(pin!(session.deliver(r#"{"kind":"ping"}"#)).as_mut());
    assert_eq!(delivered, Poll::Ready(()));
    assert_eq!(sent.last(), serde_json::json!({ "kind": "pong" }));
}

/// A server pushing to a browser holds `SessionEventsClient` over a `FrameWriter`: the one-way
/// call writes a notify frame and nothing else.
#[test]
fn a_frame_writer_push_writes_a_notify_frame() {
    let (writer, sent) = events_frame_writer();
    let client = SessionEventsClient::new(writer);
    let outcome = poll_by_hand(
        pin!(client.document_changed(DocumentChangedRequest {
            document_id: "doc-1".to_owned(),
            version: 3_u32,
        }))
        .as_mut(),
    );
    assert_eq!(outcome, Poll::Ready(Ok(())));
    assert_eq!(
        sent.last(),
        serde_json::json!({
            "kind": "notify",
            "operation": "document-changed",
            "payload": { "document_id": "doc-1", "version": 3_u32 },
            "service": "SessionEvents",
        }),
    );
}

/// A `FrameWriter` carries no correlation map, so `request` is refused outright, naming the
/// operation that asked for an answer it cannot wait on.
#[test]
fn a_frame_writer_asked_for_request_answers_err_naming_the_operation() {
    let (writer, _sent) = frame_writer();
    let asked = WatchRequest {
        document_id: "doc-1".to_owned(),
    };
    let refused = match poll_by_hand(pin!(writer.request("watch", &asked, Vec::new())).as_mut()) {
        Poll::Ready(Err(refused)) => Some(refused),
        Poll::Pending | Poll::Ready(Ok(_)) => None,
    }
    .unwrap();
    assert!(
        refused.contains("watch"),
        "the refusal names the operation. Got: {refused}"
    );
}

/// The `events_client` module's own `FrameSession` completes a request-and-reply round trip
/// exactly like `ws_client`'s, even though `SessionEvents` declares no reply operation of its own
/// to exercise it through a generated method — the transport machinery does not read the
/// operation's own shape.
#[test]
fn the_events_client_frame_session_completes_a_request_and_reply_round_trip() {
    let (session, sent) = events_frame_session();
    let mut call = pin!(events_client::Transport::request(
        &session,
        "probe",
        &(),
        Vec::new(),
    ));
    assert_eq!(poll_by_hand(call.as_mut()), Poll::Pending);
    assert_eq!(
        sent.last(),
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
    session.close("done");
}

/// `deliver` answers a ping with a pong on `events_client`'s own `FrameSession` too, and
/// `ping_frame` itself — the client's own, a dispatcher never sends one — encodes the frame a
/// session would send to probe liveness the other way.
#[test]
fn the_events_client_frame_session_answers_a_ping_and_publishes_ping_frame() {
    let (session, sent) = events_frame_session();
    let delivered = poll_by_hand(pin!(session.deliver(r#"{"kind":"ping"}"#)).as_mut());
    assert_eq!(delivered, Poll::Ready(()));
    assert_eq!(sent.last(), serde_json::json!({ "kind": "pong" }));
    assert_eq!(events_client::ping_frame(), r#"{"kind":"ping"}"#);
}

/// `events_client::Frame::decode` reads a request frame into an `IncomingMessage` the same way
/// `ws_client`'s own copy does — nothing in `SessionEvents` ever receives one, but the codec is one
/// copy shared with every frame kind, request and notify included.
#[test]
fn the_events_client_frame_decode_reads_a_request_frame_into_an_incoming_message() {
    let decoded = events_client::Frame::decode(
        r#"{"kind":"request","id":"9","service":"SessionEvents","operation":"probe","payload":{"n":1}}"#,
    )
    .unwrap();
    let (id, service, message) = match decoded {
        events_client::Frame::Request {
            id,
            service,
            message,
        } => Some((id, service, message)),
        events_client::Frame::Notify { .. }
        | events_client::Frame::Reply { .. }
        | events_client::Frame::Ping
        | events_client::Frame::Pong => None,
    }
    .unwrap();
    assert_eq!(id, "9");
    assert_eq!(service, "SessionEvents");
    assert_eq!(message.operation(), "probe");
    assert_eq!(message.payload(), br#"{"n":1}"#);
    let notified = events_client::Frame::decode(
        r#"{"kind":"notify","service":"SessionEvents","operation":"probe","payload":{"n":1}}"#,
    )
    .unwrap();
    let (notify_service, notify_message) = match notified {
        events_client::Frame::Notify {
            service: notify_service,
            message: notify_message,
        } => Some((notify_service, notify_message)),
        events_client::Frame::Request { .. }
        | events_client::Frame::Reply { .. }
        | events_client::Frame::Ping
        | events_client::Frame::Pong => None,
    }
    .unwrap();
    assert_eq!(notify_service, "SessionEvents");
    assert_eq!(notify_message.operation(), "probe");
}

/// `SessionEventsClient::transport` reaches the transport a client was bound to, the same as
/// every other generated client.
#[test]
fn a_session_events_client_exposes_the_transport_it_was_bound_to() {
    let (writer, sent) = events_frame_writer();
    let client = SessionEventsClient::new(writer);
    let pushed = poll_by_hand(
        pin!(events_client::Transport::notify(
            client.transport(),
            "probe",
            &(),
            Vec::new(),
        ))
        .as_mut(),
    );
    assert_eq!(pushed, Poll::Ready(Ok(())));
    assert_eq!(
        sent.last(),
        serde_json::json!({
            "kind": "notify",
            "operation": "probe",
            "payload": null,
            "service": "SessionEvents",
        }),
    );
}

/// `touch` is one-way: it writes a notify frame and answers as soon as the send function does.
#[test]
fn touch_is_one_way_and_writes_a_notify_frame() {
    let (session, sent) = frame_session();
    let client = DocumentSessionClient::new(session);
    let outcome = poll_by_hand(
        pin!(client.touch(TouchRequest {
            document_id: "doc-1".to_owned(),
        }))
        .as_mut(),
    );
    assert_eq!(outcome, Poll::Ready(Ok(())));
    assert_eq!(
        sent.last(),
        serde_json::json!({
            "kind": "notify",
            "operation": "touch",
            "payload": { "document_id": "doc-1" },
            "service": "DocumentSession",
        }),
    );
}

/// `unwatch` answers with the unit-success reader: `ok` alone, no `value` to read.
#[test]
fn unwatch_reads_the_unit_success_reply() {
    let (session, sent) = frame_session();
    let client = DocumentSessionClient::new(session.clone());
    let mut call = pin!(client.unwatch(UnwatchRequest {
        document_id: "doc-1".to_owned(),
    }));
    assert_eq!(poll_by_hand(call.as_mut()), Poll::Pending);
    assert_eq!(
        sent.last(),
        serde_json::json!({
            "id": "1",
            "kind": "request",
            "operation": "unwatch",
            "payload": { "document_id": "doc-1" },
            "service": "DocumentSession",
        }),
    );
    let delivered = poll_by_hand(
        pin!(session.deliver(r#"{"kind":"reply","id":"1","service":"DocumentSession","ok":true}"#))
            .as_mut(),
    );
    assert_eq!(delivered, Poll::Ready(()));
    assert_eq!(poll_by_hand(call.as_mut()), Poll::Ready(Ok(())));
}

/// `read_range` round-trips a `header_in` value out and a `header_out` value back, rejoining it
/// with the response into the tuple the trait declared.
#[test]
fn read_range_round_trips_header_in_and_header_out() {
    let (session, sent) = frame_session();
    let client = DocumentSessionClient::new(session.clone());
    let mut call = pin!(client.read_range(
        ReadRangeRequest {
            document_id: "doc-1".to_owned(),
        },
        Some("bytes=0-10".to_owned()),
    ));
    assert_eq!(poll_by_hand(call.as_mut()), Poll::Pending);
    assert_eq!(
        sent.last(),
        serde_json::json!({
            "id": "1",
            "kind": "request",
            "operation": "read-range",
            "payload": { "document_id": "doc-1" },
            "headers": { "range": "bytes=0-10" },
            "service": "DocumentSession",
        }),
    );
    let delivered = poll_by_hand(
        pin!(session.deliver(
            r#"{"kind":"reply","id":"1","service":"DocumentSession","ok":true,"value":{"content":"range-of-doc-1"},"headers":{"etag":"etag-1"}}"#
        ))
        .as_mut(),
    );
    assert_eq!(delivered, Poll::Ready(()));
    assert_eq!(
        poll_by_hand(call.as_mut()),
        Poll::Ready(Ok((
            RangeResult {
                content: "range-of-doc-1".to_owned(),
            },
            "etag-1".to_owned(),
        ))),
    );
}

/// `ws_client`'s own copy of `ping_frame`, published beside `FrameSession`'s liveness probe —
/// `deliver_answers_a_ping_with_a_pong` exercises `pong_frame` already, this exercises the other.
#[test]
fn ws_client_ping_frame_encodes_the_kind_ping_frame() {
    assert_eq!(ws_client::ping_frame(), r#"{"kind":"ping"}"#);
}

/// `ws_client::Frame::decode` reads a request frame into an `IncomingMessage`, headers included —
/// `deliver` only ever decodes a reply or a ping, so nothing else in this file reaches the other
/// two branches `answer`'s own dispatcher-side test already covers from that side.
#[test]
fn ws_client_frame_decode_reads_a_request_frame_into_an_incoming_message() {
    let decoded = ws_client::Frame::decode(
        r#"{"kind":"request","id":"9","service":"DocumentSession","operation":"watch","payload":{"document_id":"doc-1"},"headers":{"range":"bytes=0-1"}}"#,
    )
    .unwrap();
    let (id, service, message) = match decoded {
        ws_client::Frame::Request {
            id,
            service,
            message,
        } => Some((id, service, message)),
        ws_client::Frame::Notify { .. }
        | ws_client::Frame::Reply { .. }
        | ws_client::Frame::Ping
        | ws_client::Frame::Pong => None,
    }
    .unwrap();
    assert_eq!(id, "9");
    assert_eq!(service, "DocumentSession");
    assert_eq!(message.operation(), "watch");
    assert_eq!(message.payload(), br#"{"document_id":"doc-1"}"#);
    assert_eq!(
        message.headers(),
        &[("range".to_owned(), "\"bytes=0-1\"".to_owned())]
    );
    let notified = ws_client::Frame::decode(
        r#"{"kind":"notify","service":"DocumentSession","operation":"touch","payload":{"document_id":"doc-1"}}"#,
    )
    .unwrap();
    let (notify_service, notify_message) = match notified {
        ws_client::Frame::Notify {
            service: notify_service,
            message: notify_message,
        } => Some((notify_service, notify_message)),
        ws_client::Frame::Request { .. }
        | ws_client::Frame::Reply { .. }
        | ws_client::Frame::Ping
        | ws_client::Frame::Pong => None,
    }
    .unwrap();
    assert_eq!(notify_service, "DocumentSession");
    assert_eq!(notify_message.operation(), "touch");
}

/// `DocumentSessionClient::transport` reaches the transport a client was bound to.
#[test]
fn a_document_session_client_exposes_the_transport_it_was_bound_to() {
    let (session, sent) = frame_session();
    let client = DocumentSessionClient::new(session);
    let pushed = poll_by_hand(
        pin!(ws_client::Transport::notify(
            client.transport(),
            "probe",
            &(),
            Vec::new(),
        ))
        .as_mut(),
    );
    assert_eq!(pushed, Poll::Ready(Ok(())));
    assert_eq!(
        sent.last(),
        serde_json::json!({
            "kind": "notify",
            "operation": "probe",
            "payload": null,
            "service": "DocumentSession",
        }),
    );
}

/// `SessionEventsBrowser` proves the trait a server's `SessionEventsClient` calls into is
/// implementable the way the browser side actually would — nothing else in this crate places a
/// dispatcher for it, so nothing else constructs one.
#[test]
fn the_session_events_trait_is_implementable_the_way_a_browser_would() {
    let browser = SessionEventsBrowser;
    poll_once(browser.document_changed(
        &(),
        DocumentChangedRequest {
            document_id: "doc-1".to_owned(),
            version: 1,
        },
    ));
}
