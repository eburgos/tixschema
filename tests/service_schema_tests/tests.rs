//! A service declared through the macro, implemented at a chosen context, and driven far enough to
//! prove the emitted trait is the contract the design says it is: the context reaches every
//! operation, `async fn` is gone in favour of a returned future, a one-way operation answers with
//! nothing, and a wire-name override changes nothing about the Rust the author writes.

use core::future::{Future, ready};
use core::pin::pin;
use core::task::{Context as PollContext, Poll, Waker};
use tixschema::service_schema;

#[derive(Debug, PartialEq, Eq)]
pub struct BalanceRequest {
    pub organization_id: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BalanceResponse {
    pub credits: u32,
}

#[derive(Debug, PartialEq, Eq)]
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

#[service_schema()]
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
