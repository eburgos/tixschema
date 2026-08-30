//! A service whose operations cover every input shape and every outcome, read off the TypeScript
//! it publishes and off a bundle written to a file the way a consuming codebase writes one.

#[cfg(feature = "typescript")]
mod the_bundle_one_registration_line_produces {
    use super::{
        ApplyBundleReceipt, BalanceRequest, BalanceResponse, CreditWriteError, ProbeError,
        ProbeServiceSchema,
    };
    use std::env::temp_dir;
    use std::fs;
    use std::path::PathBuf;

    /// The bundle a consuming codebase writes: its own types named by hand, one line each, and the
    /// service named once. Nothing here names a message the macro declared — that is the point.
    fn bundle() -> String {
        [
            BalanceRequest::ts_definition(),
            BalanceResponse::ts_definition(),
            ApplyBundleReceipt::ts_definition(),
            ProbeError::ts_definition(),
            CreditWriteError::ts_definition(),
            ProbeServiceSchema::ts_definition(),
            ProbeServiceSchema::ts_client(),
            ProbeServiceSchema::ts_service(),
        ]
        .join("\n\n")
    }

    /// Every name the published result envelopes refer to, read off the two arms themselves rather
    /// than from a list written here, so a type the envelope starts naming is checked without this
    /// test being edited.
    fn referenced_types(written: &str) -> Vec<String> {
        let mut reached = Vec::new();
        for line in written.lines().map(str::trim) {
            if let Some(rest) = line.strip_prefix("| { ok: true; value: ") {
                reached.push(rest.trim_end_matches(" }").to_owned());
            }
            if let Some(rest) = line.strip_prefix("| { ok: false; error: ") {
                if let Some((declared, _)) = rest.split_once(" | {") {
                    reached.push(declared.to_owned());
                }
                reached.push("ServiceFault".to_owned());
            }
        }
        reached
    }

    fn written_bundle(named: &str) -> (PathBuf, String) {
        let path = temp_dir().join(named);
        fs::write(&path, bundle()).unwrap();
        let read_back = fs::read_to_string(&path).unwrap();
        fs::remove_file(&path).unwrap();
        (path, read_back)
    }

    #[test]
    fn a_bundle_written_to_a_file_declares_every_type_it_refers_to() {
        let (path, written) = written_bundle("tixschema_service_bundle_complete.ts");
        assert!(!written.is_empty(), "wrote nothing to {}", path.display());
        let reached = referenced_types(&written);
        assert!(reached.len() >= 8, "got: {reached:?}");
        for named in reached {
            assert!(
                written.contains(&format!("export type {named} =")),
                "a bundle carrying one line per author type and one line for the service leaves \
                 `{named}` undeclared. Got: {written}"
            );
        }
    }

    #[test]
    fn a_message_the_macro_declared_reaches_the_bundle_without_a_line_of_its_own() {
        let (_, written) = written_bundle("tixschema_service_bundle_declared_messages.ts");
        for declared in ["ExpireCreditRequest", "SweepRequest", "ApplyBundleRequest"] {
            assert!(
                written.contains(&format!("export type {declared} =")),
                "nobody wrote `{declared}`, so nobody could have written its registration. \
                 Got: {written}"
            );
        }
    }

    #[test]
    fn the_envelope_adds_no_field_to_the_message_it_carries() {
        let (_, written) = written_bundle("tixschema_service_bundle_untouched_messages.ts");
        let found = written
            .split("export type BalanceResponse =")
            .nth(1)
            .and_then(|rest| rest.split_once("};"))
            .map(|(body, _)| body.to_owned());
        assert!(found.is_some(), "got: {written}");
        let declared = found.unwrap();
        for injected in ["ok:", "value:", "isServiceFault", "fault:", "error:"] {
            assert!(
                !declared.contains(injected),
                "the envelope is added around the message, never into it. Got: {declared}"
            );
        }
        assert!(declared.contains("credits: number;"), "got: {declared}");
    }

    #[test]
    fn the_fault_is_declared_once_per_service_and_reachable_from_every_failure_arm() {
        let (_, written) = written_bundle("tixschema_service_bundle_fault.ts");
        assert_eq!(
            written.matches("export type ServiceFault =").count(),
            1,
            "got: {written}"
        );
        assert_eq!(
            written
                .matches("| { isServiceFault: true; fault: ServiceFault } };")
                .count(),
            4,
            "every operation that answers can answer with a fault. Got: {written}"
        );
    }

    #[test]
    fn the_result_keeps_ok_a_two_value_discriminant() {
        let written = ProbeServiceSchema::ts_definition();
        for arm in written
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("| { ok:"))
        {
            assert!(
                arm.starts_with("| { ok: true; value: ")
                    || arm.starts_with("| { ok: false; error: "),
                "a third arm would stop `ok` discriminating anything. Got: {arm}"
            );
        }
        assert_eq!(
            written.matches("| { ok: true; value: ").count(),
            written.matches("| { ok: false; error: ").count(),
            "got: {written}"
        );
    }

    #[test]
    fn the_stubs_the_client_and_the_service_emitters_fill_are_empty_and_not_missing() {
        assert!(ProbeServiceSchema::ts_client().is_empty());
        assert!(ProbeServiceSchema::ts_service().is_empty());
    }
}

#[cfg(all(feature = "typescript", feature = "zod"))]
mod the_schema_that_rides_with_the_type {
    use super::ProbeServiceSchema;

    #[test]
    fn a_declared_message_publishes_its_schema_through_the_same_line() {
        let written = ProbeServiceSchema::ts_definition();
        for declared in ["ExpireCreditRequest", "SweepRequest", "ApplyBundleRequest"] {
            assert!(
                written.contains(&format!("{declared}$Schema")),
                "a client on the far side validates what it sends. Got: {written}"
            );
        }
    }
}

use core::future::{Future, ready};
use core::pin::pin;
use core::task::{Context as PollContext, Poll, Waker};
use tixschema::{model_schema, service_schema};

#[model_schema()]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ApplyBundleReceipt {
    pub applied: bool,
}

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

/// A second error type, so the published results are read for keeping each operation's declared
/// error to that operation rather than folding them into one service-wide union.
#[model_schema()]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "errorCode", rename_all = "kebab-case")]
pub enum CreditWriteError {
    Conflict,
    NotFound,
}

#[model_schema()]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "errorCode", rename_all = "kebab-case")]
pub enum ProbeError {
    DbError,
    InsufficientBalance,
}

pub struct ProbeContext {
    pub logger_name: String,
}

#[service_schema()]
pub trait ProbeService<Ctx> {
    /// Answers nothing, and still receives a message a caller has to construct.
    #[service_schema_op(one_way)]
    async fn apply_bundle(&self, ctx: &Ctx, organization_id: String, bundle_id: String);

    /// Two arguments after the context: the message is declared from the argument list, and the
    /// operation names an error unrelated to the others'.
    async fn expire_credit(
        &self,
        ctx: &Ctx,
        organization_id: String,
        credit_id: String,
    ) -> Result<BalanceResponse, CreditWriteError>;

    /// One argument after the context: the argument already is the message.
    async fn get_balance(
        &self,
        ctx: &Ctx,
        req: BalanceRequest,
    ) -> Result<BalanceResponse, ProbeError>;

    /// A fourth operation that answers, so the fault reaches four failure arms rather than three.
    async fn settle(
        &self,
        ctx: &Ctx,
        req: BalanceRequest,
    ) -> Result<ApplyBundleReceipt, ProbeError>;

    /// None at all: an empty message is declared for it.
    async fn sweep(&self, ctx: &Ctx) -> Result<BalanceResponse, ProbeError>;
}

pub struct ProbeBackEnd {
    pub granted_credits: u32,
}

impl ProbeService<ProbeContext> for ProbeBackEnd {
    async fn apply_bundle(&self, ctx: &ProbeContext, organization_id: String, bundle_id: String) {
        let _settled = ready(ctx.logger_name.len() + organization_id.len() + bundle_id.len()).await;
    }

    async fn expire_credit(
        &self,
        ctx: &ProbeContext,
        organization_id: String,
        credit_id: String,
    ) -> Result<BalanceResponse, CreditWriteError> {
        let seen = ready(organization_id.len() + credit_id.len()).await;
        if ctx.logger_name.is_empty() {
            Err(CreditWriteError::Conflict)
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
        let seen = ready(req.organization_id.len() + ctx.logger_name.len()).await;
        Ok(BalanceResponse {
            credits: self.granted_credits + u32::try_from(seen).unwrap_or(0),
        })
    }

    async fn settle(
        &self,
        ctx: &ProbeContext,
        req: BalanceRequest,
    ) -> Result<ApplyBundleReceipt, ProbeError> {
        let seen = ready(req.organization_id.len()).await;
        if ctx.logger_name.is_empty() {
            Err(ProbeError::DbError)
        } else {
            Ok(ApplyBundleReceipt { applied: seen > 0 })
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

/// Read in every feature combination: the TypeScript emission is additive, so the trait the macro
/// emits is still the trait an implementation satisfies and a caller calls.
#[test]
fn the_service_is_still_implementable_and_callable_alongside_its_published_typescript() {
    let service = ProbeBackEnd { granted_credits: 5 };
    let ctx = ProbeContext {
        logger_name: "probe".to_owned(),
    };

    let answered = poll_once(service.get_balance(
        &ctx,
        BalanceRequest {
            organization_id: "acme".to_owned(),
        },
    ))
    .unwrap();
    assert_eq!(answered.unwrap().credits, 14);

    let refused = poll_once(service.expire_credit(
        &ProbeContext {
            logger_name: String::new(),
        },
        "acme".to_owned(),
        "cr-1".to_owned(),
    ))
    .unwrap();
    assert!(matches!(refused, Err(CreditWriteError::Conflict)));

    let swept = poll_once(service.sweep(&ctx)).unwrap();
    assert_eq!(swept.unwrap().credits, 5);

    let settled = poll_once(service.settle(
        &ctx,
        BalanceRequest {
            organization_id: "acme".to_owned(),
        },
    ))
    .unwrap();
    assert!(settled.unwrap().applied);

    assert!(poll_once(service.apply_bundle(&ctx, "acme".to_owned(), "b-1".to_owned())).is_some());
}
