//! A `body = "stream"` operation called through the `http_rest` client with no dispatcher in
//! sight - a hand-written `Transport` answering with a chunked reader, proving the client reads a
//! response body incrementally through the same seam a dispatcher answers one through, and a
//! `206` range answer coming back as `StreamedAnswer::Partial` with its own `content-range`.

#![cfg(feature = "serde")]

use crate::stream_http_rest_client;
use core::future::{Future, ready};
use core::pin::pin;
use core::task::{Context as PollContext, Poll, Waker};
use serde::{Deserialize, Serialize};
use std::io::{self, Read};
use std::sync::Mutex;
use tixschema::{model_schema, service_schema};

/// The body the "full" queued response streams, and the "partial" one's own range slices from.
const CONTENT: &[u8] = b"the quick brown fox jumps over the lazy dog";

/// The most [`ChunkedSlice::read`] ever returns in one call - what makes draining a queued stream
/// genuinely incremental rather than one buffered copy.
const CHUNK_CAP: usize = 5;

#[model_schema()]
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum ContentError {
    NotFound,
}

#[service_schema(transports = ["http_rest"])]
pub trait ContentClientService<Ctx> {
    #[service_schema_op(http(
        method = "GET",
        path = "/documents/{document_id}/content",
        header_in("range" = byte_range),
        body = "stream",
        error_status(NotFound = 404),
    ))]
    async fn get_content(
        &self,
        ctx: &Ctx,
        document_id: String,
        byte_range: Option<String>,
    ) -> Result<content_client_service_schema::StreamedAnswer, ContentError>;

    /// The same content, with a declared `header_out` composed onto the streamed answer - proving
    /// the client reads it back off both `StreamedAnswer` arms, exactly as the dispatcher writes
    /// it out.
    #[service_schema_op(http(
        method = "GET",
        path = "/documents/{document_id}/content-with-etag",
        header_in("range" = byte_range),
        header_out("etag"),
        body = "stream",
        error_status(NotFound = 404),
    ))]
    async fn get_content_with_etag(
        &self,
        ctx: &Ctx,
        document_id: String,
        byte_range: Option<String>,
    ) -> Result<(content_client_service_schema::StreamedAnswer, String), ContentError>;
}

/// A chunked [`Read`] source: every call answers at most [`CHUNK_CAP`] bytes. Blanket-implemented
/// as [`BodySource`](content_client_service_schema::BodySource) for free.
struct ChunkedSlice {
    remaining: Vec<u8>,
}

/// One queued answer: status, headers, and the body the seam hands back - already-buffered bytes
/// or a genuinely chunked source, either satisfying `BodySource`.
enum QueuedBody {
    Bytes(Vec<u8>),
    Stream(ChunkedSlice),
}

/// One queued response: status, headers and body, in that order - the same order
/// `IncomingResponse::new` takes them.
type QueuedResponse = (u16, Vec<(String, String)>, QueuedBody);

/// One request the seam recorded, read back through the generated `OutgoingRequest`'s own
/// accessors rather than by destructuring it - there is nothing else the seam implementation could
/// reach for either.
#[derive(Clone, Debug, Eq, PartialEq)]
struct RecordedRequest {
    body: Vec<u8>,
    headers: Vec<(String, String)>,
    method: String,
    path: String,
    query: String,
}

/// A hand-written seam answering whichever response was queued for it, in order, and recording
/// every request it was handed - the client's own consuming side of the seam is what turns a
/// queued answer into a `StreamedAnswer`.
struct StreamingTransport {
    requests: Mutex<Vec<RecordedRequest>>,
    responses: Mutex<Vec<QueuedResponse>>,
}

/// The contract stands on its own: implementing it takes the trait and nothing else, and nothing
/// in this binary placed a dispatcher for it.
pub struct ContentClientBackEnd;

impl ChunkedSlice {
    fn new(content: &[u8]) -> Self {
        Self {
            remaining: content.to_vec(),
        }
    }
}

impl Read for ChunkedSlice {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let take = buf.len().min(self.remaining.len()).min(CHUNK_CAP);
        let rest = self.remaining.split_off(take);
        buf[..take].copy_from_slice(&self.remaining);
        self.remaining = rest;
        Ok(take)
    }
}

impl StreamingTransport {
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

impl stream_http_rest_client::Transport for StreamingTransport {
    async fn send(
        &self,
        request: stream_http_rest_client::OutgoingRequest,
    ) -> Result<stream_http_rest_client::IncomingResponse, String> {
        ready(()).await;
        self.requests.lock().unwrap().push(RecordedRequest {
            method: request.method().to_owned(),
            path: request.path().to_owned(),
            query: request.query().to_owned(),
            headers: request.headers().to_vec(),
            body: request.body().to_vec(),
        });
        let (status, headers, queued_body) = self.responses.lock().unwrap().remove(0);
        let incoming_body = match queued_body {
            QueuedBody::Bytes(bytes) => stream_http_rest_client::IncomingBody::Bytes(bytes),
            QueuedBody::Stream(source) => {
                stream_http_rest_client::IncomingBody::Stream(Box::new(source))
            }
        };
        Ok(stream_http_rest_client::IncomingResponse::new(
            status,
            headers,
            incoming_body,
        ))
    }
}

impl ContentClientService<()> for ContentClientBackEnd {
    async fn get_content(
        &self,
        _ctx: &(),
        document_id: String,
        _byte_range: Option<String>,
    ) -> Result<content_client_service_schema::StreamedAnswer, ContentError> {
        ready(()).await;
        if document_id == "missing" {
            return Err(ContentError::NotFound);
        }
        Ok(content_client_service_schema::StreamedAnswer::Full(
            Box::new(ChunkedSlice::new(CONTENT)),
        ))
    }

    async fn get_content_with_etag(
        &self,
        ctx: &(),
        document_id: String,
        byte_range: Option<String>,
    ) -> Result<(content_client_service_schema::StreamedAnswer, String), ContentError> {
        let answered = self.get_content(ctx, document_id, byte_range).await?;
        Ok((answered, "v9".to_owned()))
    }
}

/// The `Full` source, or `None` for `Partial` - extracted through `Option` rather than a match arm
/// that panics, so a wrong variant fails a test through the same `.unwrap()` every other assertion
/// here does.
fn full_source(
    answered: content_client_service_schema::StreamedAnswer,
) -> Option<Box<dyn content_client_service_schema::BodySource + Send>> {
    match answered {
        content_client_service_schema::StreamedAnswer::Full(source) => Some(source),
        content_client_service_schema::StreamedAnswer::Partial {
            source: _source,
            content_range: _content_range,
        } => None,
    }
}

/// The `Partial` source and its `content-range`, or `None` for `Full`.
fn partial_source(
    answered: content_client_service_schema::StreamedAnswer,
) -> Option<(
    Box<dyn content_client_service_schema::BodySource + Send>,
    String,
)> {
    match answered {
        content_client_service_schema::StreamedAnswer::Partial {
            source,
            content_range,
        } => Some((source, content_range)),
        content_client_service_schema::StreamedAnswer::Full(_) => None,
    }
}

/// Drains a `BodySource` fully, standing in for whatever a real caller does with the stream the
/// client handed back. Answers the bytes and how many `pull()` calls it took.
fn drain(mut source: Box<dyn content_client_service_schema::BodySource + Send>) -> (Vec<u8>, u32) {
    let mut drained = Vec::new();
    let mut pulls = 0_u32;
    let mut buf = [0_u8; 64];
    loop {
        let read = source.pull(&mut buf).unwrap();
        if read == 0 {
            break;
        }
        pulls += 1;
        drained.extend_from_slice(&buf[..read]);
    }
    (drained, pulls)
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

/// A full `200` answer comes back as `StreamedAnswer::Full`, its source read incrementally through
/// the same `BodySource` seam a dispatcher's own handler answers through - and the request the
/// client sent is recorded exactly as `header_in`/the path template built it.
#[test]
fn a_full_answer_streams_back_through_the_seam_in_more_than_one_pull() {
    let transport = StreamingTransport::queued(vec![(
        200,
        Vec::new(),
        QueuedBody::Stream(ChunkedSlice::new(CONTENT)),
    )]);
    let client = stream_http_rest_client::ContentClientServiceClient::new(transport);
    let answered = poll_once(client.get_content("present".to_owned(), None))
        .unwrap()
        .unwrap();
    let source = full_source(answered).unwrap();
    let (body, pulls) = drain(source);
    assert_eq!(body, CONTENT);
    assert!(pulls > 1, "got {pulls} pulls");
    assert_eq!(
        client.transport().requests(),
        vec![RecordedRequest {
            method: "GET".to_owned(),
            path: "/documents/present/content".to_owned(),
            query: String::new(),
            // A `header_in` bound to `None` sends no header at all.
            headers: Vec::new(),
            body: Vec::new(),
        }]
    );
}

/// A `206` answer comes back as `StreamedAnswer::Partial`, `content-range` read off the response
/// header before the body is taken, and the `Range` request header travelling out through
/// `header_in` exactly like it does for any other operation.
#[test]
fn a_206_answer_carries_its_content_range_into_the_partial_variant() {
    let transport = StreamingTransport::queued(vec![(
        206,
        vec![("content-range".to_owned(), "bytes 4-8/44".to_owned())],
        QueuedBody::Stream(ChunkedSlice::new(b"quick")),
    )]);
    let client = stream_http_rest_client::ContentClientServiceClient::new(transport);
    let answered =
        poll_once(client.get_content("present".to_owned(), Some("bytes=4-8".to_owned())))
            .unwrap()
            .unwrap();
    let (source, content_range) = partial_source(answered).unwrap();
    assert_eq!(content_range, "bytes 4-8/44");
    let (body, _pulls) = drain(source);
    assert_eq!(body, b"quick");
    assert_eq!(
        client.transport().requests()[0].headers,
        vec![("range".to_owned(), "bytes=4-8".to_owned())]
    );
}

/// A declared `header_out` reads back off the `Full` arm alongside the whole streamed body.
#[test]
fn a_header_out_entry_reads_back_off_the_full_streamed_answer() {
    let transport = StreamingTransport::queued(vec![(
        200,
        vec![("etag".to_owned(), "v9".to_owned())],
        QueuedBody::Stream(ChunkedSlice::new(CONTENT)),
    )]);
    let client = stream_http_rest_client::ContentClientServiceClient::new(transport);
    let (answered, etag) = poll_once(client.get_content_with_etag("present".to_owned(), None))
        .unwrap()
        .unwrap();
    assert_eq!(etag, "v9");
    let (drained, _pulls) = drain(full_source(answered).unwrap());
    assert_eq!(drained, CONTENT);
}

/// A declared `header_out` reads back off the `206` `Partial` arm, composed alongside the
/// `content-range` that arm already carries.
#[test]
fn a_header_out_entry_reads_back_off_the_partial_streamed_answer() {
    let transport = StreamingTransport::queued(vec![(
        206,
        vec![
            ("content-range".to_owned(), "bytes 4-8/44".to_owned()),
            ("etag".to_owned(), "v9".to_owned()),
        ],
        QueuedBody::Stream(ChunkedSlice::new(b"quick")),
    )]);
    let client = stream_http_rest_client::ContentClientServiceClient::new(transport);
    let (answered, etag) =
        poll_once(client.get_content_with_etag("present".to_owned(), Some("bytes=4-8".to_owned())))
            .unwrap()
            .unwrap();
    assert_eq!(etag, "v9");
    let (source, content_range) = partial_source(answered).unwrap();
    assert_eq!(content_range, "bytes 4-8/44");
    let (drained, _pulls) = drain(source);
    assert_eq!(drained, b"quick");
}

/// A response the seam already buffered still satisfies `BodySource` once wrapped in a `Cursor` -
/// the client answers a body source either way, streamed or not.
#[test]
fn an_already_buffered_response_still_reads_back_as_a_body_source() {
    let transport =
        StreamingTransport::queued(vec![(200, Vec::new(), QueuedBody::Bytes(CONTENT.to_vec()))]);
    let client = stream_http_rest_client::ContentClientServiceClient::new(transport);
    let answered = poll_once(client.get_content("present".to_owned(), None))
        .unwrap()
        .unwrap();
    let source = full_source(answered).unwrap();
    let (body, _pulls) = drain(source);
    assert_eq!(body, CONTENT);
}

/// A mapped error status still decodes into the declared error, exactly like any other operation's.
#[test]
fn a_streamed_operations_mapped_status_still_decodes_into_the_declared_error() {
    let transport = StreamingTransport::queued(vec![(
        404,
        Vec::new(),
        QueuedBody::Bytes(br#"{"errorCode":"not-found"}"#.to_vec()),
    )]);
    let client = stream_http_rest_client::ContentClientServiceClient::new(transport);
    let answered = poll_once(client.get_content("missing".to_owned(), None)).unwrap();
    // `StreamedAnswer` holds a boxed trait object and derives neither `Debug` nor `PartialEq`, so
    // the error arm is checked with `matches!` rather than compared with `assert_eq!`.
    assert!(matches!(
        answered,
        Err(content_client_service_schema::CallError::Operation(
            ContentError::NotFound
        ))
    ));
}

/// The contract stands on its own: implementing it takes the trait and nothing else, and nothing
/// in this binary placed a dispatcher for it.
#[test]
fn the_contract_is_implementable_where_no_dispatcher_was_placed() {
    let answered = poll_once(ContentClientBackEnd.get_content(&(), "present".to_owned(), None))
        .unwrap()
        .unwrap();
    let (body, pulls) = drain(full_source(answered).unwrap());
    assert_eq!(body, CONTENT);
    assert!(pulls > 1, "got {pulls} pulls");
    assert!(matches!(
        poll_once(ContentClientBackEnd.get_content(&(), "missing".to_owned(), None)).unwrap(),
        Err(ContentError::NotFound)
    ));
    let (with_etag, etag) =
        poll_once(ContentClientBackEnd.get_content_with_etag(&(), "present".to_owned(), None))
            .unwrap()
            .unwrap();
    assert_eq!(etag, "v9");
    let (drained, _pulls) = drain(full_source(with_etag).unwrap());
    assert_eq!(drained, CONTENT);
}
