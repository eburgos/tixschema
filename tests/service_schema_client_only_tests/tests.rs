//! A service, the client placed out of its own macro, and a transport written by hand.
//!
//! Nothing here places a dispatcher. What the calls below read back is the whole of what a client
//! needs to work: the envelope a remote wrote, the error the operation declared, and the fault a
//! transport that could not carry the call reports.

#![cfg(feature = "serde")]

use crate::amqp_client;
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

/// A backend answering the contract, with no dispatcher in sight: the trait is the contract, and a
/// crate that placed only the client can still name and implement it.
pub struct CallBackEnd;

/// A transport that refuses everything it is handed, in the words a bus reports a call that never
/// landed in.
pub struct RefusingTransport;

/// A transport that hands out prepared answers, one per call, and records the operation name it
/// was asked to carry beside the payload.
pub struct ProbeTransport {
    answers: Mutex<Vec<Vec<u8>>>,
    calls: Mutex<Vec<String>>,
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

impl amqp_client::Transport for ProbeTransport {
    async fn notify<T>(&self, operation: &str, payload: T) -> Result<(), String>
    where
        T: Serialize + Send,
    {
        ready(()).await;
        self.record(operation, &payload);
        self.answer();
        Ok(())
    }

    async fn request<T>(&self, operation: &str, payload: T) -> Result<Vec<u8>, String>
    where
        T: Serialize + Send,
    {
        ready(()).await;
        self.record(operation, &payload);
        Ok(self.answer())
    }
}

impl amqp_client::Transport for RefusingTransport {
    async fn notify<T>(&self, _operation: &str, _payload: T) -> Result<(), String>
    where
        T: Serialize + Send,
    {
        ready(()).await;
        Err("the deadline passed with no reply".to_owned())
    }

    async fn request<T>(&self, _operation: &str, _payload: T) -> Result<Vec<u8>, String>
    where
        T: Serialize + Send,
    {
        ready(()).await;
        Err("the deadline passed with no reply".to_owned())
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
