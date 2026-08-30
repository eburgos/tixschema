//! A service whose three operations cover the three input shapes, and the messages the macro
//! declares for two of them: their TypeScript, their Zod schema, their JSON Schema, the keys they
//! write on the wire, and the author's own message left exactly as they wrote it.
//!
//! The context type, the type bound to it and that type's own fields are read for too, negatively:
//! an implementation receives the context in every operation and none of the three surfaces
//! mentions any of it.

#[cfg(feature = "jsonschema")]
mod the_generated_message_on_json_schema {
    use super::{CREDIT_ID_KEY, ORGANIZATION_ID_KEY};
    use super::{ExpireCreditRequest, SweepRequest};

    #[test]
    fn the_declared_message_requires_every_argument() {
        let document = ExpireCreditRequest::json_schema();
        let rendered = document.to_string();
        assert_eq!(document["type"], "object", "got: {rendered}");
        assert_eq!(
            document["properties"][ORGANIZATION_ID_KEY]["type"], "string",
            "got: {rendered}"
        );
        assert_eq!(
            document["properties"][CREDIT_ID_KEY]["type"], "string",
            "got: {rendered}"
        );
        let required = document["required"].as_array().cloned().unwrap_or_default();
        assert!(
            required.iter().any(|key| key == ORGANIZATION_ID_KEY),
            "got: {rendered}"
        );
        assert!(
            required.iter().any(|key| key == CREDIT_ID_KEY),
            "got: {rendered}"
        );
    }

    #[test]
    fn the_empty_message_is_a_document_of_its_own_rather_than_no_document() {
        let document = SweepRequest::json_schema();
        let rendered = document.to_string();
        assert_eq!(document["type"], "object", "got: {rendered}");
        assert_eq!(
            document["properties"].as_object().map(serde_json::Map::len),
            Some(0),
            "got: {rendered}"
        );
        assert_eq!(
            document["required"].as_array().map(Vec::len),
            Some(0),
            "got: {rendered}"
        );
    }
}

#[cfg(feature = "typescript")]
mod the_generated_message_on_typescript {
    use super::{BalanceRequest, ExpireCreditRequest, SweepRequest};
    use super::{CREDIT_ID_KEY, ORGANIZATION_ID_KEY};

    #[test]
    fn the_declared_message_publishes_a_type_naming_every_argument() {
        let ts = ExpireCreditRequest::ts_definition();
        assert!(!ts.is_empty(), "got: {ts}");
        assert!(
            ts.contains("export type ExpireCreditRequest = {"),
            "got: {ts}"
        );
        assert!(
            ts.contains(&format!("{ORGANIZATION_ID_KEY}: string;")),
            "got: {ts}"
        );
        assert!(
            ts.contains(&format!("{CREDIT_ID_KEY}: string;")),
            "got: {ts}"
        );
    }

    #[test]
    fn the_declared_message_says_what_its_field_names_cost() {
        let ts = ExpireCreditRequest::ts_definition();
        assert!(
            ts.contains("field names are the operation's parameter names"),
            "the cost an author has to weigh reaches the published type too. Got: {ts}"
        );
    }

    #[test]
    fn the_empty_message_publishes_a_type_of_its_own() {
        let ts = SweepRequest::ts_definition();
        assert!(!ts.is_empty(), "got: {ts}");
        assert!(ts.contains("export type SweepRequest ="), "got: {ts}");
    }

    #[test]
    fn the_message_the_author_declared_is_untouched_by_the_service_macro() {
        let ts = BalanceRequest::ts_definition();
        assert!(ts.contains("export type BalanceRequest = {"), "got: {ts}");
        assert!(
            ts.contains("organization_id: string;"),
            "the author annotated their own type and the macro added nothing to it. Got: {ts}"
        );
    }
}

#[cfg(feature = "zod")]
mod the_generated_message_on_zod {
    use super::{CREDIT_ID_KEY, ORGANIZATION_ID_KEY};
    use super::{ExpireCreditRequest, SweepRequest};

    #[test]
    fn the_declared_message_publishes_a_schema_naming_every_argument() {
        let zod = ExpireCreditRequest::zod_schema();
        assert!(!zod.is_empty(), "got: {zod}");
        assert!(zod.contains("ExpireCreditRequest$Schema"), "got: {zod}");
        assert!(
            zod.contains(&format!("{ORGANIZATION_ID_KEY}: z.string()")),
            "got: {zod}"
        );
        assert!(
            zod.contains(&format!("{CREDIT_ID_KEY}: z.string()")),
            "got: {zod}"
        );
    }

    #[test]
    fn the_empty_message_publishes_a_schema_of_its_own() {
        let zod = SweepRequest::zod_schema();
        assert!(!zod.is_empty(), "got: {zod}");
        assert!(zod.contains("SweepRequest$Schema"), "got: {zod}");
        assert!(zod.contains("z.strictObject({"), "got: {zod}");
    }
}

mod the_context_the_trait_declares {
    use super::BalanceRequest;
    use super::{ProbeBackEnd, ProbeContext, ProbeError, ProbeService as _, poll_once};

    #[test]
    fn an_operation_reads_the_context_rather_than_any_message_field() {
        let service = ProbeBackEnd { granted_credits: 5 };
        let silent = ProbeContext {
            logger_name: String::new(),
        };
        let answered =
            poll_once(service.expire_credit(&silent, "acme".to_owned(), "cr-1".to_owned()))
                .unwrap();
        assert!(
            matches!(answered, Err(ProbeError::InsufficientBalance)),
            "the operation answered off the context, which no message carries"
        );
    }

    #[cfg(feature = "jsonschema")]
    #[test]
    fn the_context_reaches_no_json_schema_surface() {
        use super::{BalanceResponse, ExpireCreditRequest, SweepRequest};

        for surface in [
            ExpireCreditRequest::json_schema().to_string(),
            SweepRequest::json_schema().to_string(),
            BalanceRequest::json_schema().to_string(),
            BalanceResponse::json_schema().to_string(),
        ] {
            assert!(!surface.contains("Ctx"), "got: {surface}");
            assert!(!surface.contains("ProbeContext"), "got: {surface}");
            assert!(!surface.contains("logger_name"), "got: {surface}");
        }
    }

    #[cfg(feature = "typescript")]
    #[test]
    fn the_context_reaches_no_typescript_surface() {
        use super::{BalanceResponse, ExpireCreditRequest, SweepRequest};

        for surface in [
            ExpireCreditRequest::ts_definition(),
            SweepRequest::ts_definition(),
            BalanceRequest::ts_definition(),
            BalanceResponse::ts_definition(),
        ] {
            assert!(!surface.contains("Ctx"), "got: {surface}");
            assert!(!surface.contains("ProbeContext"), "got: {surface}");
            assert!(!surface.contains("logger_name"), "got: {surface}");
        }
    }

    #[cfg(feature = "zod")]
    #[test]
    fn the_context_reaches_no_zod_surface() {
        use super::{BalanceResponse, ExpireCreditRequest, SweepRequest};

        for surface in [
            ExpireCreditRequest::zod_schema(),
            SweepRequest::zod_schema(),
            BalanceRequest::zod_schema(),
            BalanceResponse::zod_schema(),
        ] {
            assert!(!surface.contains("Ctx"), "got: {surface}");
            assert!(!surface.contains("ProbeContext"), "got: {surface}");
            assert!(!surface.contains("logger_name"), "got: {surface}");
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

        let empty = poll_once(service.sweep(&ctx)).unwrap();
        assert_eq!(empty.unwrap().credits, 5);
    }
}

use core::future::{Future, ready};
use core::pin::pin;
use core::task::{Context as PollContext, Poll, Waker};
use tixschema::{model_schema, service_schema};

/// The key an argument becomes on the wire. Every declared message carries
/// `#[serde(rename_all = "camelCase")]`, which is what serde writes; the `serde` feature is what
/// makes the describing surfaces read that attribute, and a build without it describes the Rust
/// spelling instead.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
const CREDIT_ID_KEY: &str = if cfg!(feature = "serde") {
    "creditId"
} else {
    "credit_id"
};

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
const ORGANIZATION_ID_KEY: &str = if cfg!(feature = "serde") {
    "organizationId"
} else {
    "organization_id"
};

/// The message the author declared, carrying their own annotations and no `rename_all` of any
/// kind, so a key the macro moved would show up here as plainly as anywhere.
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

/// What an implementation needs and no caller may see. Nothing here crosses the wire.
pub struct ProbeContext {
    pub logger_name: String,
}

pub struct ProbeBackEnd {
    pub granted_credits: u32,
}

#[model_schema()]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "errorCode", rename_all = "kebab-case")]
pub enum ProbeError {
    DbError,
    InsufficientBalance,
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

    /// None at all: an empty message is declared rather than no message.
    async fn sweep(&self, ctx: &Ctx) -> Result<BalanceResponse, ProbeError>;
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

    async fn sweep(&self, ctx: &ProbeContext) -> Result<BalanceResponse, ProbeError> {
        let _settled = ready(ctx.logger_name.len()).await;
        Ok(BalanceResponse {
            credits: self.granted_credits,
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

/// The wire itself, read in every feature combination: the serde derives and the `rename_all` are
/// written onto the declared message by the macro, so what serde puts on the wire does not depend
/// on which describing feature is on.
#[test]
fn a_declared_message_writes_its_arguments_as_camel_case_keys() {
    let written = serde_json::to_string(&ExpireCreditRequest {
        organization_id: "acme".to_owned(),
        credit_id: "cr-1".to_owned(),
    })
    .unwrap();
    assert_eq!(
        written, r#"{"organizationId":"acme","creditId":"cr-1"}"#,
        "an argument is snake_case in Rust and camelCase on the wire, as a hand-written field is"
    );
}

#[test]
fn an_empty_message_is_still_a_payload_on_the_wire() {
    let written = serde_json::to_string(&SweepRequest {}).unwrap();
    assert_eq!(
        written, "{}",
        "an operation that later gains a field must not change from no payload to a payload"
    );
    assert!(
        serde_json::from_str::<SweepRequest>("{}").is_ok(),
        "and reads back as the message it is"
    );
}
