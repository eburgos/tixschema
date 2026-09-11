//! `DocumentSession`: a `watch`/`unwatch` pair covering the success, declared-error and
//! unit-success shapes; `touch`, one-way, for the request/notify split and the default-success
//! fallback; `read_range`, a `header_in`/`header_out` pair, for the headers round trip.
//!
//! `answer` is driven directly against text frames — no socket, no adapter — the same way
//! `dispatch` is driven directly against an `IncomingMessage` elsewhere in this crate.

#![cfg(feature = "serde")]

use crate::ws_transport::{self, Frame};
use core::future::{Future, ready};
use core::pin::pin;
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
#[derive(Deserialize, Serialize)]
pub struct RangeResult {
    pub content: String,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
pub struct WatchResult {
    pub accepted: bool,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum RangeError {
    NotFound,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum UnwatchError {
    NotFound,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
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

/// `ping_frame` is the client macro's, landing later — this dispatcher only answers a ping, it
/// never sends one — but the two share one codec, so the dispatcher publishes it too.
#[test]
fn ping_frame_encodes_the_kind_ping_frame() {
    assert_eq!(ws_transport::ping_frame(), r#"{"kind":"ping"}"#);
}

/// `Frame::Reply` is the client macro's own frame kind, landing later — this dispatcher never
/// answers with one, it only decodes one, which `answer` itself has no reason to do.
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
