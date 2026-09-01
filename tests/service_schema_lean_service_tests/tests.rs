//! The two lean services, the backends that answer them, and the reply handle and transport the
//! calls below are driven through.

#![cfg(feature = "serde")]

use crate::{bare_amqp_client, bare_amqp_transport, note_amqp_client, note_amqp_transport};
use core::future::{Future, ready};
use core::pin::pin;
use core::task::{Context as PollContext, Poll, Waker};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tixschema::{model_schema, service_schema};

/// A message the author declared, so the macro generates none for the operation that names it.
#[model_schema()]
#[derive(Deserialize, Serialize)]
pub struct AnnotationRequest {
    pub organization_id: String,
}

/// A service declaring no operation at all. Its dispatcher has one arm, and that arm answers a
/// name nothing recognises.
#[service_schema(transports = ["amqp_rpc"])]
pub trait BareService<Ctx> {}

/// A service whose every operation expects no reply, so nothing it emits ever reads an answer
/// back.
#[service_schema(transports = ["amqp_rpc"])]
pub trait NoteService<Ctx> {
    /// A message the author declared.
    #[service_schema_op(one_way)]
    async fn annotate(&self, ctx: &Ctx, req: AnnotationRequest);

    /// Several arguments after the context, so the macro declares `NoteRequest` for it and the
    /// client builds one through `$crate`.
    #[service_schema_op(one_way)]
    async fn note(&self, ctx: &Ctx, slug: String, detail: String);
}

/// Something to implement the empty contract with: a service that declares no operation still
/// declares a trait, and `dispatch` is generic over whatever implements it.
pub struct BareBackEnd;

/// A backend writing down what reached it, which is the whole of what a one-way call can be read
/// back by.
#[derive(Default)]
pub struct NoteBackEnd {
    reached: Mutex<Vec<String>>,
}

/// A reply handle keeping every fault it was handed, as the JSON a caller would have read.
#[derive(Default)]
pub struct Recorder {
    faults: Mutex<Vec<serde_json::Value>>,
}

/// A transport that carries nothing and says so, which is the one failure a one-way call has left
/// once its message has passed its own validator.
#[derive(Debug, Eq, PartialEq)]
pub struct RefusingTransport;

impl BareService<()> for BareBackEnd {}

impl NoteService<()> for NoteBackEnd {
    async fn annotate(&self, _ctx: &(), req: AnnotationRequest) {
        ready(()).await;
        self.reached.lock().unwrap().push(req.organization_id);
    }

    async fn note(&self, _ctx: &(), slug: String, detail: String) {
        ready(()).await;
        self.reached
            .lock()
            .unwrap()
            .push(format!("{slug}/{detail}"));
    }
}

impl bare_amqp_client::Transport for RefusingTransport {
    async fn notify<T>(&self, _operation: &str, _payload: T) -> Result<(), String>
    where
        T: Serialize + Send,
    {
        ready(()).await;
        Err("the message never went out".to_owned())
    }

    async fn request<T>(&self, _operation: &str, _payload: T) -> Result<Vec<u8>, String>
    where
        T: Serialize + Send,
    {
        ready(()).await;
        Err("the call never landed".to_owned())
    }
}

impl bare_amqp_transport::Reply for Recorder {
    async fn fault(&self, fault: bare_service_schema::ServiceFault) {
        ready(()).await;
        self.record(&fault);
    }

    async fn send<T>(&self, value: T)
    where
        T: Serialize + Send,
    {
        ready(()).await;
        self.record(&value);
    }
}

impl note_amqp_client::Transport for RefusingTransport {
    async fn notify<T>(&self, _operation: &str, _payload: T) -> Result<(), String>
    where
        T: Serialize + Send,
    {
        ready(()).await;
        Err("the message never went out".to_owned())
    }

    async fn request<T>(&self, _operation: &str, _payload: T) -> Result<Vec<u8>, String>
    where
        T: Serialize + Send,
    {
        ready(()).await;
        Err("the call never landed".to_owned())
    }
}

impl note_amqp_transport::Reply for Recorder {
    async fn fault(&self, fault: note_service_schema::ServiceFault) {
        ready(()).await;
        self.record(&fault);
    }

    async fn send<T>(&self, value: T)
    where
        T: Serialize + Send,
    {
        ready(()).await;
        self.record(&value);
    }
}

impl NoteBackEnd {
    fn reached(&self) -> Vec<String> {
        self.reached.lock().unwrap().clone()
    }
}

impl Recorder {
    fn record<T>(&self, written: &T)
    where
        T: Serialize,
    {
        self.faults
            .lock()
            .unwrap()
            .push(serde_json::to_value(written).unwrap());
    }

    fn settled(&self) -> Vec<serde_json::Value> {
        self.faults.lock().unwrap().clone()
    }
}

/// Nothing below suspends, so one poll answers it; `None` says an assumption about the bodies
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

/// A one-way arm reaches its implementation and answers nobody, with no answer reader and no fault
/// mirror anywhere in the module it was placed in.
#[test]
fn a_one_way_service_dispatches_into_its_implementation_and_settles_nothing() {
    let backend = NoteBackEnd::default();
    let reply = Recorder::default();
    poll_once(note_amqp_transport::dispatch(
        &backend,
        &(),
        &note_amqp_transport::IncomingMessage::new(
            "note".to_owned(),
            br#"{"slug":"acme","detail":"read"}"#.to_vec(),
        ),
        &reply,
    ))
    .unwrap();
    assert_eq!(backend.reached(), vec!["acme/read".to_owned()]);
    assert!(
        reply.settled().is_empty(),
        "the operation declared no reply, so the arm touches the handle with nothing"
    );
}

/// The one failure a one-way call still owes its caller: a transport that could not put the
/// message out. It is reported without any of the machinery that reads a reply back.
#[test]
fn a_one_way_client_reports_a_transport_that_could_not_put_the_message_out() {
    let client = note_amqp_client::NoteServiceClient::new(RefusingTransport);
    let answered = poll_once(client.annotate(AnnotationRequest {
        organization_id: "acme".to_owned(),
    }))
    .unwrap();
    let fault = answered.unwrap_err();
    assert_eq!(
        serde_json::to_value(&fault).unwrap(),
        serde_json::json!({
            "detail": "the message never went out",
            "kind": "transport-failure",
            "operation": "annotate",
        })
    );
    let built = poll_once(client.note("acme".to_owned(), "read".to_owned())).unwrap();
    assert!(
        built.is_err(),
        "the message the macro declared is built and handed to the same transport"
    );
    assert_eq!(client.transport(), &RefusingTransport);
}

/// A dispatcher for a service with no operation is the fallback arm and nothing else: it reads the
/// name off the message and answers, reaching neither an implementation nor a context.
#[test]
fn a_service_declaring_no_operation_still_settles_a_delivery_that_names_one() {
    let reply = Recorder::default();
    poll_once(bare_amqp_transport::dispatch(
        &BareBackEnd,
        &(),
        &bare_amqp_transport::IncomingMessage::new("anything".to_owned(), b"{}".to_vec()),
        &reply,
    ))
    .unwrap();
    assert_eq!(
        reply.settled(),
        vec![serde_json::json!({
            "detail": "the service answers to no operation by that name",
            "kind": "unknown-operation",
            "operation": "anything",
        })]
    );
}

/// A client for a service with no operation publishes the binding and the transport it was bound
/// to, and no method for anything to call.
#[test]
fn a_client_for_a_service_declaring_no_operation_still_binds_a_transport() {
    let client = bare_amqp_client::BareServiceClient::new(RefusingTransport);
    assert_eq!(client.transport(), &RefusingTransport);
}

/// The handle carries an answer arm whether or not a service ever reaches it: a one-way arm
/// answers nobody, a bare service has no arm at all, and a transport implementing `Reply` writes
/// both arms either way.
#[test]
fn the_reply_handle_carries_an_answer_arm_neither_lean_service_ever_reaches() {
    let bare = Recorder::default();
    poll_once(bare_amqp_transport::Reply::send(&bare, "answered")).unwrap();
    let note = Recorder::default();
    poll_once(note_amqp_transport::Reply::send(&note, "answered")).unwrap();
    for settled in [bare.settled(), note.settled()] {
        assert_eq!(settled, vec![serde_json::json!("answered")]);
    }
}

/// The seam carries both arms for the same reason: a one-way client only ever notifies, and a
/// bare client reaches neither arm at all.
#[test]
fn the_transport_seam_carries_the_arms_neither_lean_client_ever_reaches() {
    for called in [
        poll_once(bare_amqp_client::Transport::request(
            &RefusingTransport,
            "anything",
            (),
        ))
        .unwrap(),
        poll_once(note_amqp_client::Transport::request(
            &RefusingTransport,
            "anything",
            (),
        ))
        .unwrap(),
    ] {
        assert_eq!(called, Err("the call never landed".to_owned()));
    }
    let sent = poll_once(bare_amqp_client::Transport::notify(
        &RefusingTransport,
        "anything",
        (),
    ))
    .unwrap();
    assert_eq!(sent, Err("the message never went out".to_owned()));
}

/// A delivery keeps its bytes and hands them back through the reader, in a module whose dispatcher
/// has no arm to read them.
#[test]
fn a_delivery_hands_its_bytes_back_where_no_arm_reads_them() {
    let delivered =
        bare_amqp_transport::IncomingMessage::new("anything".to_owned(), b"{}".to_vec());
    assert_eq!(delivered.operation(), "anything");
    assert_eq!(delivered.payload(), b"{}");
}
