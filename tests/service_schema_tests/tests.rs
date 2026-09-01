//! A service declared through the macro, implemented at a chosen context, and driven far enough to
//! prove the emitted trait is the contract the design says it is: the context reaches every
//! operation, `async fn` is gone in favour of a returned future, a one-way operation answers with
//! nothing, and a wire-name override changes nothing about the Rust the author writes.
//!
//! The same declaration is then reached from the other two directions — dispatched under the wire
//! name it answers to, and called through the generated client over a transport that loops back
//! into that dispatcher — so the three spellings of one operation are read off one declaration
//! rather than three fixtures.

#![cfg(feature = "serde")]

use core::future::{Future, ready};
use core::pin::pin;
use core::task::{Context as PollContext, Poll, Waker};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tixschema::service_schema;

#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct BalanceRequest {
    pub organization_id: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct BalanceResponse {
    pub credits: u32,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum ProbeError {
    DbError,
    InsufficientBalance,
}

/// What an implementation needs and no caller may see. Nothing here crosses the wire.
pub struct ProbeContext {
    pub logger_name: String,
}

pub struct ProbeBackEnd {
    pub granted_credits: u32,
}

#[service_schema(transports = ["amqp_rpc"])]
pub trait ProbeService<Ctx> {
    /// No reply, so no return type.
    #[service_schema_op(one_way)]
    async fn apply_bundle(&self, ctx: &Ctx, req: BalanceRequest);

    /// A wire name the method name would never yield, and Rust is untouched by it.
    #[service_schema_op(message = "usage-generation-request")]
    async fn can_generate(
        &self,
        ctx: &Ctx,
        req: BalanceRequest,
    ) -> Result<BalanceResponse, ProbeError>;

    /// Several arguments after the context.
    async fn expire_credit(
        &self,
        ctx: &Ctx,
        organization_id: String,
        credit_id: String,
    ) -> Result<BalanceResponse, ProbeError>;

    /// One argument after the context: the argument already is the message.
    async fn get_balance(
        &self,
        ctx: &Ctx,
        req: BalanceRequest,
    ) -> Result<BalanceResponse, ProbeError>;

    /// None at all.
    async fn sweep(&self, ctx: &Ctx) -> Result<BalanceResponse, ProbeError>;
}

impl ProbeService<ProbeContext> for ProbeBackEnd {
    async fn apply_bundle(&self, ctx: &ProbeContext, req: BalanceRequest) {
        let _settled = ready(ctx.logger_name.len() + req.organization_id.len()).await;
    }

    async fn can_generate(
        &self,
        ctx: &ProbeContext,
        req: BalanceRequest,
    ) -> Result<BalanceResponse, ProbeError> {
        self.get_balance(ctx, req).await
    }

    async fn expire_credit(
        &self,
        ctx: &ProbeContext,
        organization_id: String,
        credit_id: String,
    ) -> Result<BalanceResponse, ProbeError> {
        let seen = ready(organization_id.len() + credit_id.len()).await;
        if ctx.logger_name.is_empty() {
            Err(ProbeError::InsufficientBalance)
        } else {
            Ok(BalanceResponse {
                credits: u32::try_from(seen).unwrap_or(0),
            })
        }
    }

    async fn get_balance(
        &self,
        ctx: &ProbeContext,
        req: BalanceRequest,
    ) -> Result<BalanceResponse, ProbeError> {
        let seen = ready(req.organization_id.len()).await;
        if ctx.logger_name.is_empty() {
            Err(ProbeError::DbError)
        } else {
            Ok(BalanceResponse {
                credits: self.granted_credits + u32::try_from(seen).unwrap_or(0),
            })
        }
    }

    async fn sweep(&self, ctx: &ProbeContext) -> Result<BalanceResponse, ProbeError> {
        let seen = ready(ctx.logger_name.len()).await;
        Ok(BalanceResponse {
            credits: u32::try_from(seen).unwrap_or(0),
        })
    }
}

/// The client, placed: the macro takes no arguments and emits bare items, so the module is this
/// crate's to name. It sits below the declaration it came from, that being the only scope a
/// `macro_export` macro another macro expanded is reachable from inside the declaring crate.
#[cfg(test)]
pub mod amqp_client {
    use super::{BalanceRequest, BalanceResponse, ProbeError};

    probe_service_amqp_rpc_client!();
}

/// A transport that answers by dispatching straight back into the service, so a client call and
/// the arm that serves it are the two ends of one seam rather than two fixtures.
pub struct Loopback {
    service: ProbeBackEnd,
}

/// A reply handle that keeps the bytes a transport would have published.
pub struct Capture {
    answered: Mutex<Vec<Vec<u8>>>,
}

impl probe_service_schema::Reply for Capture {
    async fn fault(&self, fault: probe_service_schema::ServiceFault) {
        ready(()).await;
        // The framing is the transport's business: a fault rides tagged inside the failure arm,
        // which is the shape a caller in either language narrows on.
        let framed = serde_json::json!({
            "ok": false,
            "error": { "isServiceFault": true, "fault": fault },
        });
        self.answered
            .lock()
            .unwrap()
            .push(serde_json::to_vec(&framed).unwrap());
    }

    async fn send<T>(&self, value: T)
    where
        T: Serialize + Send,
    {
        ready(()).await;
        self.answered
            .lock()
            .unwrap()
            .push(serde_json::to_vec(&value).unwrap());
    }
}

impl amqp_client::Transport for Loopback {
    async fn notify<T>(&self, operation: &str, payload: T) -> Result<(), String>
    where
        T: Serialize + Send,
    {
        let capture = Capture::new();
        probe_service_schema::dispatch(
            &self.service,
            &ProbeContext {
                logger_name: "probe".to_owned(),
            },
            &incoming(operation, &payload),
            &capture,
        )
        .await;
        Ok(())
    }

    async fn request<T>(&self, operation: &str, payload: T) -> Result<Vec<u8>, String>
    where
        T: Serialize + Send,
    {
        let capture = Capture::new();
        probe_service_schema::dispatch(
            &self.service,
            &ProbeContext {
                logger_name: "probe".to_owned(),
            },
            &incoming(operation, &payload),
            &capture,
        )
        .await;
        Ok(capture.answered())
    }
}

impl Capture {
    fn answered(&self) -> Vec<u8> {
        self.answered.lock().unwrap().pop().unwrap()
    }

    fn new() -> Self {
        Self {
            answered: Mutex::new(Vec::new()),
        }
    }
}

/// What the dispatcher settled one message with, as the bytes a transport would have put on the
/// wire.
fn answered(operation: &str, payload: &str) -> String {
    let capture = Capture::new();
    poll_once(probe_service_schema::dispatch(
        &ProbeBackEnd { granted_credits: 5 },
        &ProbeContext {
            logger_name: "probe".to_owned(),
        },
        &probe_service_schema::IncomingMessage {
            operation: operation.to_owned(),
            payload: payload.as_bytes().to_vec(),
        },
        &capture,
    ))
    .unwrap();
    String::from_utf8(capture.answered()).unwrap()
}

/// One message as the dispatcher on the far side reads it.
fn incoming<T>(operation: &str, payload: &T) -> probe_service_schema::IncomingMessage
where
    T: Serialize,
{
    probe_service_schema::IncomingMessage {
        operation: operation.to_owned(),
        payload: serde_json::to_vec(payload).unwrap(),
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

#[test]
fn a_one_way_operation_answers_with_nothing_at_all() {
    let service = ProbeBackEnd { granted_credits: 5 };
    let ctx = ProbeContext {
        logger_name: "probe".to_owned(),
    };
    let settled = poll_once(service.apply_bundle(
        &ctx,
        BalanceRequest {
            organization_id: "acme".to_owned(),
        },
    ));
    assert_eq!(settled, Some(()), "a one-way operation produces no reply");
}

#[test]
fn an_operation_reads_the_context_rather_than_any_message_field() {
    let service = ProbeBackEnd { granted_credits: 5 };
    let silent = ProbeContext {
        logger_name: String::new(),
    };
    let answered =
        poll_once(service.expire_credit(&silent, "acme".to_owned(), "cr-1".to_owned())).unwrap();
    assert_eq!(
        answered,
        Err(ProbeError::InsufficientBalance),
        "the operation answered off the context, which no message carries"
    );
}

#[test]
fn an_operation_taking_nothing_but_the_context_is_still_callable() {
    let service = ProbeBackEnd { granted_credits: 5 };
    let ctx = ProbeContext {
        logger_name: "probe".to_owned(),
    };
    let answered = poll_once(service.sweep(&ctx)).unwrap();
    assert_eq!(
        answered,
        Ok(BalanceResponse { credits: 5 }),
        "an operation with no arguments after the context still answers"
    );
}

#[test]
fn every_operation_returns_a_future_rather_than_being_declared_async() {
    let service = ProbeBackEnd { granted_credits: 5 };
    let ctx = ProbeContext {
        logger_name: "probe".to_owned(),
    };
    // Binding the call without `.await` only compiles because the emitted signature returns a
    // future, which is exactly what the `async fn` desugaring produces.
    let answering = service.get_balance(
        &ctx,
        BalanceRequest {
            organization_id: "acme".to_owned(),
        },
    );
    let answered = poll_once(answering).unwrap();
    assert_eq!(
        answered,
        Ok(BalanceResponse { credits: 9 }),
        "the emitted trait is implementable at a chosen context"
    );
}

#[test]
fn a_wire_name_override_leaves_the_rust_method_name_alone() {
    let service = ProbeBackEnd { granted_credits: 5 };
    let ctx = ProbeContext {
        logger_name: "probe".to_owned(),
    };
    let answered = poll_once(service.can_generate(
        &ctx,
        BalanceRequest {
            organization_id: "acme".to_owned(),
        },
    ))
    .unwrap();
    assert_eq!(
        answered,
        Ok(BalanceResponse { credits: 9 }),
        "Rust calls it `can_generate` whatever the wire carries"
    );
}

#[test]
fn an_operation_answers_on_the_wire_to_the_kebab_case_of_the_name_it_was_declared_under() {
    assert_eq!(
        answered("get-balance", r#"{"organization_id":"acme"}"#),
        r#"{"ok":true,"value":{"credits":9}}"#,
        "one declaration, and the wire name is derived from it with no attribute written"
    );
}

#[test]
fn a_wire_name_override_moves_the_wire_name_and_leaves_no_derived_one_behind() {
    assert_eq!(
        answered("usage-generation-request", r#"{"organization_id":"acme"}"#),
        r#"{"ok":true,"value":{"credits":9}}"#,
        "the override is what an existing service already puts on the wire, so it has to be the \
         name the dispatcher answers to"
    );
    let derived = answered("can-generate", r#"{"organization_id":"acme"}"#);
    assert!(
        derived.contains(r#""kind":"unknown-operation""#),
        "the override moves the wire name rather than adding a second one. Got: {derived}"
    );
}

// Read off the published client, which only a build with both surfaces writes.
#[cfg(all(feature = "typescript", feature = "zod"))]
#[test]
fn the_same_declaration_spells_the_operation_in_typescript_the_way_typescript_would() {
    let published = ProbeServiceSchema::ts_client();
    assert!(
        published.contains("canGenerate(req: BalanceRequest)"),
        "a TypeScript caller types the name a TypeScript developer would write. Got: {published}"
    );
    assert!(
        published.contains(r#""usage-generation-request""#),
        "and hands the transport the wire name, which is the only one of the three that moved. \
         Got: {published}"
    );
    assert!(
        !published.contains("can_generate"),
        "the Rust spelling is Rust's alone. Got: {published}"
    );
}

#[test]
fn a_client_call_answers_the_declared_success_type_or_a_call_error_over_the_declared_error() {
    let client = amqp_client::ProbeServiceClient::new(Loopback {
        service: ProbeBackEnd { granted_credits: 5 },
    });
    // Annotated rather than inferred: the failure arm being `CallError<ProbeError>` rather than
    // `ProbeError` is what makes room for a fault the operation never declared, and an inferred
    // binding would not say so.
    let answering: Result<BalanceResponse, probe_service_schema::CallError<ProbeError>> =
        poll_once(client.get_balance(BalanceRequest {
            organization_id: "acme".to_owned(),
        }))
        .unwrap();
    assert_eq!(answering, Ok(BalanceResponse { credits: 9 }));
}

/// Every operation, over the same loop back into the dispatcher: the client publishes one method
/// per declared operation, each under the wire name its own arm answers to, and each carrying what
/// the implementation returned.
#[test]
fn every_operation_the_service_declares_is_reachable_through_the_client_it_publishes() {
    let client = amqp_client::ProbeServiceClient::new(Loopback {
        service: ProbeBackEnd { granted_credits: 5 },
    });
    let request = || BalanceRequest {
        organization_id: "acme".to_owned(),
    };
    assert_eq!(
        poll_once(client.can_generate(request())).unwrap(),
        Ok(BalanceResponse { credits: 9 }),
        "a wire-name override moves the name the transport carries and nothing about the method"
    );
    assert_eq!(
        poll_once(client.expire_credit("acme".to_owned(), "cr-1".to_owned())).unwrap(),
        Ok(BalanceResponse { credits: 8 }),
        "the arguments after the context are packed into the message the macro declared"
    );
    assert_eq!(
        poll_once(client.sweep()).unwrap(),
        Ok(BalanceResponse { credits: 5 }),
        "an operation that takes nothing still sends the empty message declared for it"
    );
    assert!(
        poll_once(client.apply_bundle(request())).unwrap().is_ok(),
        "a one-way send answers nothing beyond the send itself"
    );
    assert_eq!(
        client.transport().service.granted_credits,
        5,
        "the client keeps the transport it was bound to rather than consuming it"
    );
}
