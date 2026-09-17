//! A service, the client placed out of its own macro, and a transport written by hand.
//!
//! Nothing here places a dispatcher. What the calls below read back is the whole of what a client
//! needs to work: the envelope a remote wrote, the error the operation declared, and the fault a
//! transport that could not carry the call reports.

#![cfg(feature = "serde")]

use crate::amqp_client;
use crate::http_rest_client;
use core::future::{Future, ready};
use core::pin::pin;
use core::task::{Context as PollContext, Poll, Waker};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tixschema::{model_schema, service_schema};

#[model_schema()]
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CallAnswer {
    pub credits: u32,
}

#[model_schema()]
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum CallFailure {
    DbError,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
pub struct CallRequest {
    pub organization_id: String,
}

#[model_schema()]
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CreateDocumentResponse {
    pub document_id: String,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
pub struct CreateDocumentRequest {
    pub title: String,
}

#[model_schema()]
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum CreateDocumentError {
    TitleTaken,
}

#[model_schema()]
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum GetVersionError {
    NotFound,
    VersionGone,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
pub struct GetVersionRequest {
    pub document_id: String,
    pub version_id: String,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
pub struct ReadWindowRequest {
    pub document_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_version: Option<String>,
}

#[model_schema()]
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VersionResponse {
    pub content: String,
}

#[model_schema()]
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum ThumbnailError {
    NotFound,
}

#[service_schema(transports = ["amqp_rpc"])]
pub trait CallService<Ctx> {
    /// No reply, so the send half of the seam is placed beside the call half.
    #[service_schema_op(one_way)]
    async fn note(&self, ctx: &Ctx, req: CallRequest);

    /// One argument, which already is the message.
    async fn read_balance(&self, ctx: &Ctx, req: CallRequest) -> Result<CallAnswer, CallFailure>;

    /// None at all, so the macro declares the message and the client builds one through `$crate`.
    async fn sweep(&self, ctx: &Ctx) -> Result<CallAnswer, CallFailure>;
}

#[service_schema(transports = ["http_rest"])]
pub trait DocumentClientService<Ctx> {
    #[service_schema_op(http(
        method = "POST",
        path = "/documents",
        error_status(TitleTaken = 409)
    ))]
    async fn create_document(
        &self,
        ctx: &Ctx,
        req: CreateDocumentRequest,
    ) -> Result<CreateDocumentResponse, CreateDocumentError>;

    #[service_schema_op(http(
        method = "GET",
        path = "/documents/{document_id}/thumbnail",
        error_status(NotFound = 404),
        body = "bytes",
        header_out("x-document-id"),
    ))]
    async fn get_thumbnail(
        &self,
        ctx: &Ctx,
        document_id: String,
    ) -> Result<(Vec<u8>, String, String), ThumbnailError>;

    #[service_schema_op(http(
        method = "GET",
        path = "/documents/{document_id}/versions/{version_id}",
        ok_status = 200,
        header_in("range" = byte_range),
        header_out("etag"),
        error_status(NotFound = 404, VersionGone = 410),
    ))]
    async fn get_version(
        &self,
        ctx: &Ctx,
        req: GetVersionRequest,
        byte_range: Option<String>,
    ) -> Result<(VersionResponse, String), GetVersionError>;

    #[service_schema_op(one_way, http(method = "DELETE", path = "/documents/{document_id}"))]
    async fn purge_document(&self, ctx: &Ctx, document_id: String);

    #[service_schema_op(http(
        method = "GET",
        path = "/documents/{document_id}/window",
        error_status(NotFound = 404, VersionGone = 410),
    ))]
    async fn read_window(
        &self,
        ctx: &Ctx,
        req: ReadWindowRequest,
    ) -> Result<VersionResponse, GetVersionError>;
}

/// A backend answering the contract, with no dispatcher in sight: the trait is the contract, and a
/// crate that placed only the client can still name and implement it.
pub struct CallBackEnd;

/// The contract stands on its own here too: implementing it takes the trait and nothing else, and
/// nothing in this binary placed a dispatcher for it.
pub struct DocumentClientBackEnd;

/// A transport that refuses everything it is handed, in the words a bus reports a call that never
/// landed in.
pub struct RefusingTransport;

/// A transport that hands out prepared answers, one per call, and records the operation name it
/// was asked to carry beside the payload.
pub struct ProbeTransport {
    answers: Mutex<Vec<Vec<u8>>>,
    calls: Mutex<Vec<String>>,
}

/// One queued response: status, headers and body, in that order — the same order
/// `IncomingResponse::new` takes them.
type QueuedResponse = (u16, Vec<(String, String)>, Vec<u8>);

/// One request the seam recorded, read back through the generated `OutgoingRequest`'s own
/// accessors rather than by destructuring it — there is nothing else the seam implementation could
/// reach for either.
#[derive(Clone, Debug, Eq, PartialEq)]
struct RecordedRequest {
    body: Vec<u8>,
    headers: Vec<(String, String)>,
    method: String,
    path: String,
    query: String,
}

/// A hand-written in-memory seam: records every request it is handed and hands back whichever
/// response was queued for it, in order.
pub struct RecordingTransport {
    requests: Mutex<Vec<RecordedRequest>>,
    responses: Mutex<Vec<QueuedResponse>>,
}

impl CallService<()> for CallBackEnd {
    async fn note(&self, _ctx: &(), req: CallRequest) {
        let _read = ready(req.organization_id.len()).await;
    }

    async fn read_balance(&self, _ctx: &(), req: CallRequest) -> Result<CallAnswer, CallFailure> {
        ready(()).await;
        Ok(CallAnswer {
            credits: u32::try_from(req.organization_id.len()).unwrap(),
        })
    }

    async fn sweep(&self, _ctx: &()) -> Result<CallAnswer, CallFailure> {
        ready(()).await;
        Ok(CallAnswer { credits: 0 })
    }
}

impl DocumentClientService<()> for DocumentClientBackEnd {
    async fn create_document(
        &self,
        _ctx: &(),
        req: CreateDocumentRequest,
    ) -> Result<CreateDocumentResponse, CreateDocumentError> {
        ready(()).await;
        Ok(CreateDocumentResponse {
            document_id: format!("doc-{}", req.title),
        })
    }

    async fn get_thumbnail(
        &self,
        _ctx: &(),
        document_id: String,
    ) -> Result<(Vec<u8>, String, String), ThumbnailError> {
        ready(()).await;
        Ok((
            vec![0x89, 0x50, 0x4e, 0x47],
            "image/png".to_owned(),
            format!("doc-{document_id}"),
        ))
    }

    async fn get_version(
        &self,
        _ctx: &(),
        req: GetVersionRequest,
        _byte_range: Option<String>,
    ) -> Result<(VersionResponse, String), GetVersionError> {
        ready(()).await;
        Ok((
            VersionResponse {
                content: format!("{}@{}", req.document_id, req.version_id),
            },
            "v1".to_owned(),
        ))
    }

    async fn purge_document(&self, _ctx: &(), _document_id: String) {
        ready(()).await;
    }

    async fn read_window(
        &self,
        _ctx: &(),
        req: ReadWindowRequest,
    ) -> Result<VersionResponse, GetVersionError> {
        ready(()).await;
        Ok(VersionResponse {
            content: req.document_id,
        })
    }
}

impl amqp_client::Transport for ProbeTransport {
    async fn notify<T>(
        &self,
        operation: &str,
        payload: T,
        _headers: Vec<(String, String)>,
    ) -> Result<(), String>
    where
        T: Serialize + Send,
    {
        ready(()).await;
        self.record(operation, &payload);
        self.answer();
        Ok(())
    }

    async fn request<T>(
        &self,
        operation: &str,
        payload: T,
        _headers: Vec<(String, String)>,
    ) -> Result<(Vec<u8>, Vec<(String, String)>), String>
    where
        T: Serialize + Send,
    {
        ready(()).await;
        self.record(operation, &payload);
        Ok((self.answer(), Vec::new()))
    }
}

impl amqp_client::Transport for RefusingTransport {
    async fn notify<T>(
        &self,
        _operation: &str,
        _payload: T,
        _headers: Vec<(String, String)>,
    ) -> Result<(), String>
    where
        T: Serialize + Send,
    {
        ready(()).await;
        Err("the deadline passed with no reply".to_owned())
    }

    async fn request<T>(
        &self,
        _operation: &str,
        _payload: T,
        _headers: Vec<(String, String)>,
    ) -> Result<(Vec<u8>, Vec<(String, String)>), String>
    where
        T: Serialize + Send,
    {
        ready(()).await;
        Err("the deadline passed with no reply".to_owned())
    }
}

impl http_rest_client::Transport for RecordingTransport {
    async fn send(
        &self,
        request: http_rest_client::OutgoingRequest,
    ) -> Result<http_rest_client::IncomingResponse, String> {
        ready(()).await;
        self.requests.lock().unwrap().push(RecordedRequest {
            method: request.method().to_owned(),
            path: request.path().to_owned(),
            query: request.query().to_owned(),
            headers: request.headers().to_vec(),
            body: request.body().to_vec(),
        });
        let (status, headers, body) = self.responses.lock().unwrap().remove(0);
        Ok(http_rest_client::IncomingResponse::new(
            status, headers, body,
        ))
    }
}

impl ProbeTransport {
    /// The next prepared answer. A transport built with none never returns from here, which is the
    /// failure a test that reaches the transport is meant to report.
    fn answer(&self) -> Vec<u8> {
        self.answers.lock().unwrap().pop().unwrap()
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    /// The answers this transport will give, in the order it will give them.
    fn new(answers: &[&str]) -> Self {
        Self {
            answers: Mutex::new(
                answers
                    .iter()
                    .rev()
                    .map(|answer| answer.as_bytes().to_vec())
                    .collect(),
            ),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn record<T>(&self, operation: &str, payload: &T)
    where
        T: Serialize,
    {
        let _written = serde_json::to_string(payload).unwrap();
        self.calls.lock().unwrap().push(operation.to_owned());
    }
}

impl RecordingTransport {
    /// The responses this transport will give, in the order it will give them.
    fn queued(responses: Vec<QueuedResponse>) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            responses: Mutex::new(responses),
        }
    }

    fn requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().unwrap().clone()
    }
}

/// The transports never suspend, so one poll answers them; `None` says an assumption about the
/// bodies above stopped holding rather than that the runtime is missing.
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

#[test]
fn a_call_placed_without_a_dispatcher_reads_the_answer_the_remote_wrote() {
    let transport = ProbeTransport::new(&[r#"{"ok":true,"value":{"credits":7}}"#]);
    let client = amqp_client::CallServiceClient::new(transport);
    let answered = poll_once(client.read_balance(CallRequest {
        organization_id: "acme".to_owned(),
    }))
    .unwrap();
    assert_eq!(answered, Ok(CallAnswer { credits: 7 }));
    assert_eq!(client.transport().calls(), vec!["read-balance".to_owned()]);
}

#[test]
fn the_error_the_operation_declared_comes_back_in_the_operation_arm() {
    let transport = ProbeTransport::new(&[r#"{"ok":false,"error":{"errorCode":"db-error"}}"#]);
    let client = amqp_client::CallServiceClient::new(transport);
    let answered = poll_once(client.sweep()).unwrap();
    assert_eq!(
        answered,
        Err(call_service_schema::CallError::Operation(
            CallFailure::DbError
        ))
    );
}

#[test]
fn a_transport_that_could_not_carry_the_call_is_reported_as_a_transport_failure() {
    let client = amqp_client::CallServiceClient::new(RefusingTransport);
    let answered = poll_once(client.read_balance(CallRequest {
        organization_id: "acme".to_owned(),
    }))
    .unwrap();
    let reported = match &answered {
        Err(call_service_schema::CallError::Fault(reported)) => Some(reported),
        Ok(_) | Err(call_service_schema::CallError::Operation(_)) => None,
    }
    .unwrap();
    assert_eq!(
        reported.kind(),
        call_service_schema::ServiceFaultKind::TransportFailure
    );
    assert_eq!(reported.operation(), "read-balance");
    assert_eq!(reported.detail(), "the deadline passed with no reply");
}

#[test]
fn a_one_way_send_answers_the_transport_s_own_refusal_as_a_fault() {
    let client = amqp_client::CallServiceClient::new(RefusingTransport);
    let reported = poll_once(client.note(CallRequest {
        organization_id: "acme".to_owned(),
    }))
    .unwrap()
    .unwrap_err();
    assert_eq!(
        reported.kind(),
        call_service_schema::ServiceFaultKind::TransportFailure
    );
    assert_eq!(reported.operation(), "note");
}

/// The contract stands on its own: implementing it takes a trait and nothing else, and nothing in
/// this binary placed a dispatcher for it.
#[test]
fn the_contract_is_implementable_where_no_dispatcher_was_placed() {
    let answered = poll_once(CallBackEnd.read_balance(
        &(),
        CallRequest {
            organization_id: "acme".to_owned(),
        },
    ))
    .unwrap();
    assert_eq!(answered, Ok(CallAnswer { credits: 4 }));
    assert_eq!(
        poll_once(CallBackEnd.sweep(&())).unwrap(),
        Ok(CallAnswer { credits: 0 })
    );
    poll_once(CallBackEnd.note(
        &(),
        CallRequest {
            organization_id: "acme".to_owned(),
        },
    ))
    .unwrap();
}

#[test]
fn a_call_records_the_exact_request_method_path_query_headers_and_body() {
    let transport = RecordingTransport::queued(vec![(
        200,
        vec![("etag".to_owned(), "v9".to_owned())],
        br#"{"content":"d1@v1"}"#.to_vec(),
    )]);
    let client = http_rest_client::DocumentClientServiceClient::new(transport);
    let answered = poll_once(client.get_version(
        GetVersionRequest {
            document_id: "d1".to_owned(),
            version_id: "v1".to_owned(),
        },
        Some("bytes=0-1".to_owned()),
    ))
    .unwrap();
    assert_eq!(
        answered,
        Ok((
            VersionResponse {
                content: "d1@v1".to_owned()
            },
            "v9".to_owned()
        ))
    );
    assert_eq!(
        client.transport().requests(),
        vec![RecordedRequest {
            method: "GET".to_owned(),
            path: "/documents/d1/versions/v1".to_owned(),
            query: String::new(),
            headers: vec![("range".to_owned(), "bytes=0-1".to_owned())],
            body: Vec::new(),
        }],
        "a placeholder is filled by exact segment substitution, never by splitting a shared prefix"
    );
}

#[test]
fn an_absent_option_header_in_omits_the_header_entirely_rather_than_sending_null() {
    let transport = RecordingTransport::queued(vec![(
        200,
        vec![("etag".to_owned(), "v9".to_owned())],
        br#"{"content":"d1@v1"}"#.to_vec(),
    )]);
    let client = http_rest_client::DocumentClientServiceClient::new(transport);
    poll_once(client.get_version(
        GetVersionRequest {
            document_id: "d1".to_owned(),
            version_id: "v1".to_owned(),
        },
        None,
    ))
    .unwrap()
    .unwrap();
    assert_eq!(
        client.transport().requests(),
        vec![RecordedRequest {
            method: "GET".to_owned(),
            path: "/documents/d1/versions/v1".to_owned(),
            query: String::new(),
            headers: Vec::new(),
            body: Vec::new(),
        }],
        "a `None` header_in argument carries no header at all - not one holding the text \"null\""
    );
}

#[test]
fn a_bodied_call_carries_the_message_as_the_json_body() {
    let transport = RecordingTransport::queued(vec![(
        200,
        Vec::new(),
        br#"{"document_id":"doc-report"}"#.to_vec(),
    )]);
    let client = http_rest_client::DocumentClientServiceClient::new(transport);
    let answered = poll_once(client.create_document(CreateDocumentRequest {
        title: "report".to_owned(),
    }))
    .unwrap();
    assert_eq!(
        answered,
        Ok(CreateDocumentResponse {
            document_id: "doc-report".to_owned()
        })
    );
    let requests = client.transport().requests();
    assert_eq!(requests.len(), 1, "got: {requests:?}");
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/documents");
    assert_eq!(requests[0].body, br#"{"title":"report"}"#);
}

#[test]
fn a_mapped_status_decodes_into_the_declared_error() {
    let transport = RecordingTransport::queued(vec![(
        404,
        Vec::new(),
        br#"{"errorCode":"not-found"}"#.to_vec(),
    )]);
    let client = http_rest_client::DocumentClientServiceClient::new(transport);
    let answered = poll_once(client.get_version(
        GetVersionRequest {
            document_id: "missing".to_owned(),
            version_id: "v1".to_owned(),
        },
        None,
    ))
    .unwrap();
    assert_eq!(
        answered,
        Err(document_client_service_schema::CallError::Operation(
            GetVersionError::NotFound
        ))
    );
}

#[test]
fn a_fixed_fault_status_decodes_through_the_private_mirror() {
    let transport = RecordingTransport::queued(vec![(
        500,
        Vec::new(),
        br#"{"detail":"the handler came apart","kind":"handler-panic","operation":"get-version"}"#
            .to_vec(),
    )]);
    let client = http_rest_client::DocumentClientServiceClient::new(transport);
    let answered = poll_once(client.get_version(
        GetVersionRequest {
            document_id: "d1".to_owned(),
            version_id: "v1".to_owned(),
        },
        None,
    ))
    .unwrap();
    let reported = match &answered {
        Err(document_client_service_schema::CallError::Fault(reported)) => Some(reported),
        Ok(_) | Err(document_client_service_schema::CallError::Operation(_)) => None,
    }
    .unwrap();
    assert_eq!(
        reported.kind(),
        document_client_service_schema::ServiceFaultKind::HandlerPanic
    );
    assert_eq!(reported.detail(), "the handler came apart");
}

#[test]
fn a_status_naming_neither_a_declared_error_nor_a_fixed_fault_is_an_undeserializable_payload_fault()
{
    let transport = RecordingTransport::queued(vec![(599, Vec::new(), b"{}".to_vec())]);
    let client = http_rest_client::DocumentClientServiceClient::new(transport);
    let answered = poll_once(client.get_version(
        GetVersionRequest {
            document_id: "d1".to_owned(),
            version_id: "v1".to_owned(),
        },
        None,
    ))
    .unwrap();
    let reported = match &answered {
        Err(document_client_service_schema::CallError::Fault(reported)) => Some(reported),
        Ok(_) | Err(document_client_service_schema::CallError::Operation(_)) => None,
    }
    .unwrap();
    assert_eq!(
        reported.kind(),
        document_client_service_schema::ServiceFaultKind::UndeserializablePayload
    );
    assert!(
        reported.detail().contains("599"),
        "got: {}",
        reported.detail()
    );
}

#[test]
fn a_no_payload_operation_resolves_on_its_declared_status_without_reading_a_body() {
    let transport = RecordingTransport::queued(vec![(204, Vec::new(), Vec::new())]);
    let client = http_rest_client::DocumentClientServiceClient::new(transport);
    poll_once(client.purge_document("d1".to_owned()))
        .unwrap()
        .unwrap();
    assert_eq!(client.transport().requests()[0].method, "DELETE");
    assert_eq!(client.transport().requests()[0].path, "/documents/d1");
}

#[test]
fn a_lone_placeholder_on_an_author_s_own_message_sends_the_field_it_names() {
    let transport =
        RecordingTransport::queued(vec![(200, Vec::new(), br#"{"content":"d1"}"#.to_vec())]);
    let client = http_rest_client::DocumentClientServiceClient::new(transport);
    let answered = poll_once(client.read_window(ReadWindowRequest {
        document_id: "d1".to_owned(),
        from_version: Some("v1".to_owned()),
    }))
    .unwrap();
    assert_eq!(
        answered,
        Ok(VersionResponse {
            content: "d1".to_owned()
        })
    );
    assert_eq!(
        client.transport().requests()[0].path,
        "/documents/d1/window"
    );
}

/// A `body = "bytes"` operation reads the response body bare, the `content-type` header back into
/// the tuple's second element, and its declared `header_out` entry back into the third - no
/// `serde_json` decode anywhere on the success path.
#[test]
fn a_bytes_operation_reads_the_body_content_type_and_header_out_back() {
    let transport = RecordingTransport::queued(vec![(
        200,
        vec![
            ("content-type".to_owned(), "image/png".to_owned()),
            ("x-document-id".to_owned(), "doc-d1".to_owned()),
        ],
        vec![0x89, 0x50, 0x4e, 0x47],
    )]);
    let client = http_rest_client::DocumentClientServiceClient::new(transport);
    let answered = poll_once(client.get_thumbnail("d1".to_owned())).unwrap();
    assert_eq!(
        answered,
        Ok((
            vec![0x89, 0x50, 0x4e, 0x47],
            "image/png".to_owned(),
            "doc-d1".to_owned()
        ))
    );
    assert_eq!(client.transport().requests()[0].method, "GET");
    assert_eq!(
        client.transport().requests()[0].path,
        "/documents/d1/thumbnail"
    );
}

/// A `header_out` entry a bytes response never carries is a defect, not a silently absent value -
/// exactly like the JSON path's own missing-header refusal.
#[test]
fn a_bytes_operation_missing_its_declared_header_out_answers_a_fault() {
    let transport = RecordingTransport::queued(vec![(
        200,
        vec![("content-type".to_owned(), "image/png".to_owned())],
        vec![0x89, 0x50, 0x4e, 0x47],
    )]);
    let client = http_rest_client::DocumentClientServiceClient::new(transport);
    let answered = poll_once(client.get_thumbnail("d1".to_owned())).unwrap();
    assert!(
        matches!(
            &answered,
            Err(document_client_service_schema::CallError::Fault(reported))
                if reported.kind()
                    == document_client_service_schema::ServiceFaultKind::UndeserializablePayload
        ),
        "expected a fault, got: {answered:?}"
    );
}

/// A bytes operation's mapped error status still decodes into the declared error, exactly like any
/// other operation's.
#[test]
fn a_bytes_operations_mapped_status_still_decodes_into_the_declared_error() {
    let transport = RecordingTransport::queued(vec![(
        404,
        Vec::new(),
        br#"{"errorCode":"not-found"}"#.to_vec(),
    )]);
    let client = http_rest_client::DocumentClientServiceClient::new(transport);
    let answered = poll_once(client.get_thumbnail("missing".to_owned())).unwrap();
    assert_eq!(
        answered,
        Err(document_client_service_schema::CallError::Operation(
            ThumbnailError::NotFound
        ))
    );
}

/// The contract stands on its own: implementing it takes a trait and nothing else, and nothing in
/// this binary placed a dispatcher for it.
#[test]
fn the_document_contract_is_implementable_where_no_dispatcher_was_placed() {
    let answered = poll_once(DocumentClientBackEnd.get_version(
        &(),
        GetVersionRequest {
            document_id: "d1".to_owned(),
            version_id: "v1".to_owned(),
        },
        None,
    ))
    .unwrap();
    assert_eq!(
        answered,
        Ok((
            VersionResponse {
                content: "d1@v1".to_owned()
            },
            "v1".to_owned()
        ))
    );
    poll_once(DocumentClientBackEnd.purge_document(&(), "d1".to_owned())).unwrap();
    assert_eq!(
        poll_once(DocumentClientBackEnd.create_document(
            &(),
            CreateDocumentRequest {
                title: "report".to_owned()
            }
        ))
        .unwrap(),
        Ok(CreateDocumentResponse {
            document_id: "doc-report".to_owned()
        })
    );
    assert_eq!(
        poll_once(DocumentClientBackEnd.get_thumbnail(&(), "d1".to_owned())).unwrap(),
        Ok((
            vec![0x89, 0x50, 0x4e, 0x47],
            "image/png".to_owned(),
            "doc-d1".to_owned()
        ))
    );
    assert_eq!(
        poll_once(DocumentClientBackEnd.read_window(
            &(),
            ReadWindowRequest {
                document_id: "d1".to_owned(),
                from_version: None,
            }
        ))
        .unwrap(),
        Ok(VersionResponse {
            content: "d1".to_owned()
        })
    );
}
