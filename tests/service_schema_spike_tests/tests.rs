//! The probe trait, the message the macro declares for its two-argument operation, and the four
//! readings that settle the spike: the generated message's TypeScript, its Zod schema, its JSON
//! Schema, and the context reaching an implementation without appearing on any of them.

/// Question one: a struct emitted by `#[service_schema]`, carrying `#[model_schema()]`, re-expands.
#[cfg(feature = "typescript")]
mod nested_expansion_typescript {
    use super::{BalanceRequest, ExpireCreditRequest};

    #[test]
    fn the_generated_message_publishes_a_typescript_type_naming_both_arguments() {
        let ts = ExpireCreditRequest::ts_definition();
        assert!(!ts.is_empty(), "Got: {ts}");
        assert!(
            ts.contains("export type ExpireCreditRequest = {"),
            "Got: {ts}"
        );
        assert!(ts.contains("organization_id: string;"), "Got: {ts}");
        assert!(ts.contains("credit_id: string;"), "Got: {ts}");
    }

    #[test]
    fn the_message_the_author_declared_is_untouched_by_the_service_macro() {
        let ts = BalanceRequest::ts_definition();
        assert!(ts.contains("export type BalanceRequest = {"), "Got: {ts}");
        assert!(ts.contains("organization_id: string;"), "Got: {ts}");
    }
}

#[cfg(feature = "zod")]
mod nested_expansion_zod {
    use super::ExpireCreditRequest;

    #[test]
    fn the_generated_message_publishes_a_zod_schema_naming_both_arguments() {
        let zod = ExpireCreditRequest::zod_schema();
        assert!(!zod.is_empty(), "Got: {zod}");
        assert!(zod.contains("ExpireCreditRequest$Schema"), "Got: {zod}");
        assert!(zod.contains("organization_id: z.string()"), "Got: {zod}");
        assert!(zod.contains("credit_id: z.string()"), "Got: {zod}");
    }
}

#[cfg(feature = "jsonschema")]
mod nested_expansion_jsonschema {
    use super::ExpireCreditRequest;

    #[test]
    fn the_generated_message_publishes_a_json_schema_requiring_both_arguments() {
        let document = ExpireCreditRequest::json_schema();
        let rendered = document.to_string();
        assert_eq!(document["type"], "object", "Got: {rendered}");
        assert_eq!(
            document["properties"]["organization_id"]["type"], "string",
            "Got: {rendered}"
        );
        assert_eq!(
            document["properties"]["credit_id"]["type"], "string",
            "Got: {rendered}"
        );
        let required = document["required"].as_array().cloned().unwrap_or_default();
        assert!(
            required.iter().any(|key| key == "organization_id"),
            "Got: {rendered}"
        );
        assert!(
            required.iter().any(|key| key == "credit_id"),
            "Got: {rendered}"
        );
    }
}

/// Question two: the trait carries the context, and no surface the macro wrote mentions it.
mod context_type_parameter {
    use super::{
        BalanceRequest, ProbeBackEnd, ProbeContext, ProbeError, ProbeService as _, poll_once,
    };

    #[test]
    fn an_operation_reads_the_context_rather_than_any_message_field() {
        let service = ProbeBackEnd { granted_credits: 5 };
        let silent = ProbeContext {
            logger_name: String::new(),
        };
        let answered =
            poll_once(service.expire_credit(&silent, "acme".to_owned(), "cr-1".to_owned()))
                .unwrap();
        assert!(matches!(answered, Err(ProbeError::InsufficientBalance)));
    }

    #[cfg(feature = "jsonschema")]
    #[test]
    fn the_context_reaches_no_json_schema_surface() {
        use super::{BalanceResponse, ExpireCreditRequest};

        for surface in [
            ExpireCreditRequest::json_schema().to_string(),
            BalanceRequest::json_schema().to_string(),
            BalanceResponse::json_schema().to_string(),
        ] {
            assert!(!surface.contains("Ctx"), "Got: {surface}");
            assert!(!surface.contains("ProbeContext"), "Got: {surface}");
            assert!(!surface.contains("logger_name"), "Got: {surface}");
        }
    }

    #[cfg(feature = "typescript")]
    #[test]
    fn the_context_reaches_no_typescript_surface() {
        use super::{BalanceResponse, ExpireCreditRequest};

        for surface in [
            ExpireCreditRequest::ts_definition(),
            BalanceRequest::ts_definition(),
            BalanceResponse::ts_definition(),
        ] {
            assert!(!surface.contains("Ctx"), "Got: {surface}");
            assert!(!surface.contains("ProbeContext"), "Got: {surface}");
            assert!(!surface.contains("logger_name"), "Got: {surface}");
        }
    }

    #[cfg(feature = "zod")]
    #[test]
    fn the_context_reaches_no_zod_surface() {
        use super::{BalanceResponse, ExpireCreditRequest};

        for surface in [
            ExpireCreditRequest::zod_schema(),
            BalanceRequest::zod_schema(),
            BalanceResponse::zod_schema(),
        ] {
            assert!(!surface.contains("Ctx"), "Got: {surface}");
            assert!(!surface.contains("ProbeContext"), "Got: {surface}");
            assert!(!surface.contains("logger_name"), "Got: {surface}");
        }
    }

    #[test]
    fn the_emitted_trait_is_implementable_at_a_chosen_context_and_every_operation_receives_it() {
        let service = ProbeBackEnd { granted_credits: 5 };
        let ctx = ProbeContext {
            logger_name: "probe".to_owned(),
        };

        let named = poll_once(service.get_balance(
            &ctx,
            BalanceRequest {
                organization_id: "acme".to_owned(),
            },
        ))
        .unwrap();
        assert_eq!(named.unwrap().credits, 9);

        let generated =
            poll_once(service.expire_credit(&ctx, "acme".to_owned(), "cr-1".to_owned())).unwrap();
        assert_eq!(generated.unwrap().credits, 8);
    }
}

use core::future::{Future, ready};
use core::pin::pin;
use core::task::{Context as PollContext, Poll, Waker};
use tixschema::{model_schema, service_schema};

#[model_schema()]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct BalanceRequest {
    pub organization_id: String,
}

#[model_schema()]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct BalanceResponse {
    pub credits: u32,
}

#[model_schema()]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "errorCode", rename_all = "kebab-case")]
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
    /// Two arguments after the context: the message is declared from the argument list.
    async fn expire_credit(
        &self,
        ctx: &Ctx,
        organization_id: String,
        credit_id: String,
    ) -> Result<BalanceResponse, ProbeError>;

    /// One argument after the context: the argument already is the message, so nothing is declared.
    async fn get_balance(
        &self,
        ctx: &Ctx,
        req: BalanceRequest,
    ) -> Result<BalanceResponse, ProbeError>;
}

impl ProbeService<ProbeContext> for ProbeBackEnd {
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
