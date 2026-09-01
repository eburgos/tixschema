//! A dispatcher written by hand against the contract half alone.
//!
//! Nothing here is expanded from a tixschema macro. The service next door asked for no transport,
//! so this is what a consumer writes when the shape a generated dispatcher imposes — operation-name
//! routing over opaque bytes, one reply per message — is not their bus's. Every name it reaches
//! is one the service's own module publishes: the trait, the message types, the fault and its
//! constructors, the validation fallback, the readers that turn a violation report into a field and
//! a detail, and the envelope an answer travels in.

use crate::declarations::probe_service_schema::message_validation::MessageValidation as _;
use crate::declarations::probe_service_schema::{
    Answered, ServiceFault, ServiceFaultKind, violated_field, violation_detail,
};
use crate::declarations::{
    BalanceRequest, BalanceResponse, ProbeError, ProbeService, SweepRequest,
};
use core::future::{Future, ready};
use core::pin::pin;
use core::task::{Context as PollContext, Poll, Waker};
use serde::de::DeserializeOwned;

/// What an implementation is handed and no caller sees.
pub struct ProbeContext {
    pub logger_name: String,
}

pub struct ProbeBackEnd {
    pub granted_credits: u32,
}

impl ProbeService<ProbeContext> for ProbeBackEnd {
    async fn get_balance(
        &self,
        ctx: &ProbeContext,
        req: BalanceRequest,
    ) -> Result<BalanceResponse, ProbeError> {
        let seen = ready(ctx.logger_name.len() + req.organization_id.len()).await;
        Ok(BalanceResponse {
            credits: self.granted_credits + u32::try_from(seen).unwrap_or(0),
        })
    }

    async fn sweep(
        &self,
        ctx: &ProbeContext,
        req: SweepRequest,
    ) -> Result<BalanceResponse, ProbeError> {
        let seen = ready(ctx.logger_name.len()).await;
        Ok(BalanceResponse {
            credits: req.since + u32::try_from(seen).unwrap_or(0),
        })
    }
}

/// One delivery, answered: decode, check, call, encode. The answer comes back as bytes rather than
/// through a handle, this transport having one reply channel per call and no use for the other
/// shape.
async fn dispatched<S, Ctx>(
    svc: &S,
    ctx: &Ctx,
    operation: &str,
    body: &[u8],
) -> Result<Vec<u8>, ServiceFault>
where
    S: ProbeService<Ctx> + Sync,
    Ctx: Sync,
{
    match operation {
        "get-balance" => {
            let received: BalanceRequest = decoded(operation, body)?;
            checked(operation, received.validate())?;
            encoded(svc.get_balance(ctx, received).await)
        }
        "sweep" => {
            let received: SweepRequest = decoded(operation, body)?;
            checked(operation, received.validate())?;
            encoded(svc.sweep(ctx, received).await)
        }
        other => Err(ServiceFault::unknown_operation(other)),
    }
}

/// What a violation report becomes, read through the same two readers the generated half reads it
/// through.
fn checked(operation: &str, report: Result<(), Vec<String>>) -> Result<(), ServiceFault> {
    match report {
        Ok(()) => Ok(()),
        Err(violations) => Err(ServiceFault::failed_validation(
            operation,
            violated_field(&violations),
            &violation_detail(&violations),
        )),
    }
}

fn decoded<T>(operation: &str, body: &[u8]) -> Result<T, ServiceFault>
where
    T: DeserializeOwned,
{
    serde_json::from_slice::<T>(body)
        .map_err(|refusal| ServiceFault::undeserializable_payload(operation, &refusal.to_string()))
}

/// The outcome in the envelope the service publishes, which is what a caller of this service — in
/// either language — narrows on.
fn encoded(outcome: Result<BalanceResponse, ProbeError>) -> Result<Vec<u8>, ServiceFault> {
    serde_json::to_vec(&Answered::answering(outcome))
        .map_err(|refusal| ServiceFault::handler_panic("encode", &refusal.to_string()))
}

/// The probe never suspends, so one poll answers it; `None` says an assumption about the bodies
/// above stopped holding rather than that the runtime is missing.
fn poll_once<Answering>(answering: Answering) -> Option<Answering::Output>
where
    Answering: Future,
{
    let mut pinned = pin!(answering);
    let mut polling = PollContext::from_waker(Waker::noop());
    match pinned.as_mut().poll(&mut polling) {
        Poll::Ready(answer) => Some(answer),
        Poll::Pending => None,
    }
}

fn settled(operation: &str, body: &str) -> Result<Vec<u8>, ServiceFault> {
    poll_once(dispatched(
        &ProbeBackEnd { granted_credits: 5 },
        &ProbeContext {
            logger_name: "probe".to_owned(),
        },
        operation,
        body.as_bytes(),
    ))
    .unwrap()
}

#[test]
fn a_good_message_reaches_the_implementation_and_answers_in_the_published_envelope() {
    let answered = settled("get-balance", r#"{"organization_id":"acme"}"#).unwrap();
    assert_eq!(
        String::from_utf8(answered).unwrap(),
        r#"{"ok":true,"value":{"credits":14}}"#,
        "the whole round trip is written against the contract half and nothing else"
    );
}

#[test]
fn a_message_failing_its_own_check_never_reaches_the_implementation() {
    let refused = settled("get-balance", r#"{"organization_id":""}"#).unwrap_err();
    assert_eq!(refused.kind(), ServiceFaultKind::FailedValidation);
    assert_eq!(refused.operation(), "get-balance");
    assert_eq!(
        refused.field(),
        Some("organization_id"),
        "the published reader names the field off the report the check wrote"
    );
    assert_eq!(
        refused.detail(),
        "'organization_id': too short: minimum length is 1, got 0"
    );
}

#[test]
fn a_message_that_declares_no_check_of_its_own_passes_the_published_fallback() {
    let answered = settled("sweep", r#"{"since":3}"#).unwrap();
    assert_eq!(
        String::from_utf8(answered).unwrap(),
        r#"{"ok":true,"value":{"credits":8}}"#,
        "the fallback answers `Ok(())` for a message that declared nothing to check"
    );
}

#[test]
fn an_operation_the_service_answers_to_by_no_name_is_a_fault_naming_what_arrived() {
    let unrecognised = settled("rebuild", "{}").unwrap_err();
    assert_eq!(unrecognised.kind(), ServiceFaultKind::UnknownOperation);
    assert_eq!(unrecognised.operation(), "rebuild");
    assert_eq!(unrecognised.field(), None);
    assert_eq!(
        unrecognised.detail(),
        "the service answers to no operation by that name"
    );
}
