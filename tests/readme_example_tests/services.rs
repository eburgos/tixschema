//! The README's `Services` section: the service it declares, expanded here exactly as the README
//! declares it, and every emission shown beside it read back off the generator.
//!
//! Nothing in the section is written by hand. A sample that stops matching what the generator
//! writes fails here rather than in the editor of whoever pasted it.
//!
//! Zod is read in its own tests, a build without it publishing no schema to compare against.

use super::readme;
use core::future::{Future, ready};
use core::pin::pin;
use core::task::{Context as PollContext, Poll, Waker};
use serde::{Deserialize, Serialize};
use tixschema::{model_schema, service_schema};

/// The context the README declares beside the service, character for character. The rest of that
/// block is elided prose -- `// ... and the other four` -- and is not a declaration to pin.
const DECLARED_CONTEXT: &str = "pub struct UsageContext {
    pub logger_name: String,
}";

/// The trait the README declares, character for character. It is the whole of the section's Rust
/// input: the shape, both message forms, the wire-name override and the one-way flag.
const DECLARED_SERVICE: &str = r#"#[service_schema()]
pub trait UsageService<Ctx> {
    /// No reply, so no return type and no error arm.
    #[service_schema_op(one_way)]
    async fn apply_bundle(&self, ctx: &Ctx, req: AvailableBalanceRequest);

    /// Carried on the wire as `usage-generation-request` rather than `can-generate`.
    #[service_schema_op(message = "usage-generation-request")]
    async fn can_generate(
        &self,
        ctx: &Ctx,
        req: AvailableBalanceRequest,
    ) -> Result<AvailableBalanceResponse, BalanceError>;

    /// Several arguments after the context: the macro declares the message.
    async fn expire_credit(
        &self,
        ctx: &Ctx,
        organization_id: String,
        credit_id: String,
    ) -> Result<AvailableBalanceResponse, BalanceError>;

    /// One argument after the context: that argument already is the message.
    async fn get_available_balance(
        &self,
        ctx: &Ctx,
        req: AvailableBalanceRequest,
    ) -> Result<AvailableBalanceResponse, BalanceError>;

    /// None at all, so the macro declares an empty message.
    async fn sweep(&self, ctx: &Ctx) -> Result<AvailableBalanceResponse, BalanceError>;
}"#;

/// The three types the operations answer through, as the README declares them.
const DECLARED_TYPES: &str = r#"#[model_schema()]
#[derive(Deserialize, Serialize)]
pub struct AvailableBalanceRequest {
    pub organization_id: String,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
pub struct AvailableBalanceResponse {
    pub available_credits: u32,
    pub is_post_paid: bool,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum BalanceError {
    DbError,
    InsufficientBalance { shortfall: u32 },
}"#;

#[model_schema()]
#[derive(Deserialize, Serialize)]
pub struct AvailableBalanceRequest {
    pub organization_id: String,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
pub struct AvailableBalanceResponse {
    pub available_credits: u32,
    pub is_post_paid: bool,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum BalanceError {
    DbError,
    InsufficientBalance { shortfall: u32 },
}

#[service_schema()]
pub trait UsageService<Ctx> {
    /// No reply, so no return type and no error arm.
    #[service_schema_op(one_way)]
    async fn apply_bundle(&self, ctx: &Ctx, req: AvailableBalanceRequest);

    /// Carried on the wire as `usage-generation-request` rather than `can-generate`.
    #[service_schema_op(message = "usage-generation-request")]
    async fn can_generate(
        &self,
        ctx: &Ctx,
        req: AvailableBalanceRequest,
    ) -> Result<AvailableBalanceResponse, BalanceError>;

    /// Several arguments after the context: the macro declares the message.
    async fn expire_credit(
        &self,
        ctx: &Ctx,
        organization_id: String,
        credit_id: String,
    ) -> Result<AvailableBalanceResponse, BalanceError>;

    /// One argument after the context: that argument already is the message.
    async fn get_available_balance(
        &self,
        ctx: &Ctx,
        req: AvailableBalanceRequest,
    ) -> Result<AvailableBalanceResponse, BalanceError>;

    /// None at all, so the macro declares an empty message.
    async fn sweep(&self, ctx: &Ctx) -> Result<AvailableBalanceResponse, BalanceError>;
}

pub struct UsageContext {
    pub logger_name: String,
}

pub struct UsageBackEnd;

impl UsageService<UsageContext> for UsageBackEnd {
    async fn apply_bundle(&self, ctx: &UsageContext, req: AvailableBalanceRequest) {
        let _settled = ready(ctx.logger_name.len() + req.organization_id.len()).await;
    }

    async fn can_generate(
        &self,
        ctx: &UsageContext,
        req: AvailableBalanceRequest,
    ) -> Result<AvailableBalanceResponse, BalanceError> {
        self.get_available_balance(ctx, req).await
    }

    async fn expire_credit(
        &self,
        _ctx: &UsageContext,
        organization_id: String,
        credit_id: String,
    ) -> Result<AvailableBalanceResponse, BalanceError> {
        let seen = ready(organization_id.len() + credit_id.len()).await;
        Ok(AvailableBalanceResponse {
            available_credits: u32::try_from(seen).unwrap_or(0),
            is_post_paid: false,
        })
    }

    async fn get_available_balance(
        &self,
        ctx: &UsageContext,
        req: AvailableBalanceRequest,
    ) -> Result<AvailableBalanceResponse, BalanceError> {
        let seen = ready(req.organization_id.len()).await;
        if ctx.logger_name.is_empty() {
            Err(BalanceError::InsufficientBalance { shortfall: 1 })
        } else {
            Ok(AvailableBalanceResponse {
                available_credits: u32::try_from(seen).unwrap_or(0),
                is_post_paid: true,
            })
        }
    }

    async fn sweep(&self, _ctx: &UsageContext) -> Result<AvailableBalanceResponse, BalanceError> {
        ready(()).await;
        Err(BalanceError::DbError)
    }
}

/// The generator writes this, and the README shows it verbatim.
fn assert_shown(emission: &str, shown: &str) {
    assert!(
        emission.contains(shown),
        "the generator no longer writes this:\n{shown}\nGot: {emission}"
    );
    assert!(
        readme().contains(shown),
        "the README no longer shows this verbatim:\n{shown}"
    );
}

/// A member matched as a whole line on both sides, so a spelling that is merely a prefix of the
/// emitted one cannot pass for it. What the surrounding `JSDoc` says is the generator's business and
/// not the README's, which is why a block is read line by line rather than whole.
fn assert_shown_members(emission: &str, members: &[&str]) {
    for member in members {
        assert!(
            emission.lines().any(|line| line == *member),
            "the generator no longer writes this member: {member}\nGot: {emission}"
        );
        assert!(
            readme().lines().any(|line| line == *member),
            "the README no longer shows this member verbatim: {member}"
        );
    }
}

/// A declaration the README shows and this file expands, held to being one text rather than two.
///
/// The pin appears twice in this file: once as the constant the README is searched for, and once as
/// the declaration the compiler reads. Anything that moves one and not the other -- a rustfmt pass,
/// an edit to either side -- lands here.
fn assert_declared_and_documented(pinned: &str) {
    assert_eq!(
        source().matches(pinned).count(),
        2,
        "this is pinned but no longer declared here character for character:\n{pinned}"
    );
    assert!(
        readme().contains(pinned),
        "the README no longer declares this verbatim:\n{pinned}"
    );
}

/// This file, read back so a pinned declaration can be held against the one that compiles.
fn source() -> &'static str {
    include_str!("services.rs")
}

#[test]
fn the_readme_declares_the_service_the_section_is_written_around() {
    assert_declared_and_documented(DECLARED_TYPES);
    assert_declared_and_documented(DECLARED_SERVICE);
}

#[test]
fn a_message_declared_from_an_argument_list_is_shown_as_the_generator_writes_it() {
    assert_shown_members(
        &UsageServiceSchema::ts_definition(),
        &[
            "export type ExpireCreditRequest = {",
            "  organizationId: string;",
            "  creditId: string;",
            "};",
        ],
    );
}

#[test]
fn the_message_declared_for_an_operation_taking_nothing_is_shown_as_a_document_of_its_own() {
    assert_shown(
        &UsageServiceSchema::ts_definition(),
        "export type SweepRequest = Record<string, never>;",
    );
}

#[test]
fn the_error_an_operation_declares_is_shown_as_the_union_it_publishes() {
    assert_shown_members(
        &BalanceError::ts_definition(),
        &[
            "export type BalanceError = {",
            "  errorCode: \"db-error\";",
            "} | {",
            "  errorCode: \"insufficient-balance\";",
            "  shortfall: number;",
            "};",
        ],
    );
}

#[test]
fn the_result_a_caller_narrows_on_is_shown_with_the_fault_inside_its_failure_arm() {
    assert_shown(
        &UsageServiceSchema::ts_definition(),
        "export type UsageServiceGetAvailableBalanceResult =\n  \
         | { ok: true; value: AvailableBalanceResponse }\n  \
         | { ok: false; error: BalanceError | { isServiceFault: true; fault: UsageServiceFault } };",
    );
}

#[test]
fn the_outcome_an_implementation_answers_is_shown_with_no_fault_in_it() {
    assert_shown(
        &UsageServiceSchema::ts_service(),
        "export type UsageServiceGetAvailableBalanceOutcome =\n  \
         | { ok: true; value: AvailableBalanceResponse }\n  \
         | { ok: false; error: BalanceError };",
    );
}

#[test]
fn the_client_type_is_shown_with_every_method_the_service_declares() {
    assert_shown(
        &UsageServiceSchema::ts_client(),
        "export type UsageServiceClient = {\n  \
         /** Sends `apply-bundle` on `UsageService`, which expects no reply. */\n  \
         applyBundle(req: AvailableBalanceRequest): Promise<void>;\n  \
         /** Calls `usage-generation-request` on `UsageService` and waits for the answer. */\n  \
         canGenerate(req: AvailableBalanceRequest): Promise<UsageServiceCanGenerateResult>;\n  \
         /** Calls `expire-credit` on `UsageService` and waits for the answer. */\n  \
         expireCredit(req: ExpireCreditRequest): Promise<UsageServiceExpireCreditResult>;\n  \
         /** Calls `get-available-balance` on `UsageService` and waits for the answer. */\n  \
         getAvailableBalance(req: AvailableBalanceRequest): \
         Promise<UsageServiceGetAvailableBalanceResult>;\n  \
         /** Calls `sweep` on `UsageService` and waits for the answer. */\n  \
         sweep(req: SweepRequest): Promise<UsageServiceSweepResult>;\n};",
    );
}

#[test]
fn the_implementable_interface_is_shown_with_every_member_required() {
    assert_shown(
        &UsageServiceSchema::ts_service(),
        "export interface UsageServiceImpl<Ctx> {\n  \
         /** Handles `apply-bundle` on `UsageService`, which expects no reply. */\n  \
         applyBundle(ctx: Ctx, req: AvailableBalanceRequest): Promise<void>;\n  \
         /** Handles `usage-generation-request` on `UsageService` and answers it. */\n  \
         canGenerate(ctx: Ctx, req: AvailableBalanceRequest): \
         Promise<UsageServiceCanGenerateOutcome>;\n  \
         /** Handles `expire-credit` on `UsageService` and answers it. */\n  \
         expireCredit(ctx: Ctx, req: ExpireCreditRequest): \
         Promise<UsageServiceExpireCreditOutcome>;\n  \
         /** Handles `get-available-balance` on `UsageService` and answers it. */\n  \
         getAvailableBalance(ctx: Ctx, req: AvailableBalanceRequest): \
         Promise<UsageServiceGetAvailableBalanceOutcome>;\n  \
         /** Handles `sweep` on `UsageService` and answers it. */\n  \
         sweep(ctx: Ctx, req: SweepRequest): Promise<UsageServiceSweepOutcome>;\n}",
    );
}

#[test]
fn the_dispatcher_factory_is_shown_with_the_signature_it_is_emitted_under() {
    assert_shown(
        &UsageServiceSchema::ts_service(),
        "export function createUsageServiceDispatcher<Ctx>(\n  \
         impl: UsageServiceImpl<Ctx>,\n\
         ): (ctx: Ctx, operation: string, payload: unknown) => Promise<unknown> {",
    );
}

#[cfg(feature = "zod")]
#[test]
fn the_schema_a_declared_message_publishes_is_shown_as_the_generator_writes_it() {
    let published = UsageServiceSchema::ts_definition();
    assert_shown(
        &published,
        "const ExpireCreditRequest$RawSchema = z.strictObject({\n  \
         organizationId: z.string(),\n  \
         creditId: z.string(),\n});",
    );
    assert_shown(
        &published,
        "export const ExpireCreditRequest$Schema: ZodType<ExpireCreditRequest> = \
         ExpireCreditRequest$RawSchema;",
    );
}

#[cfg(feature = "zod")]
#[test]
fn the_schema_an_empty_message_publishes_is_shown_too() {
    let published = UsageServiceSchema::ts_definition();
    assert_shown(
        &published,
        "const SweepRequest$RawSchema = z.strictObject({\n});",
    );
    assert_shown(
        &published,
        "export const SweepRequest$Schema: ZodType<SweepRequest> = SweepRequest$RawSchema;",
    );
}

#[cfg(feature = "zod")]
#[test]
fn the_outbound_check_the_client_runs_before_the_transport_is_shown_as_emitted() {
    assert_shown(
        &UsageServiceSchema::ts_client(),
        "    async expireCredit(req) {\n      \
         const validated = ExpireCreditRequest$Schema.safeParse(req);\n      \
         if (!validated.success) {\n        \
         return {\n          \
         ok: false,\n          \
         error: {\n            \
         isServiceFault: true,\n            \
         fault: usageServiceOutboundFault(\"expire-credit\", validated.error.issues),\n          \
         },\n        \
         };\n      \
         }\n      \
         return transport.request<UsageServiceExpireCreditResult>(\"expire-credit\", \
         validated.data);\n    },",
    );
}

/// The service never suspends, so one poll answers it; `None` says an assumption about the bodies
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

/// The organization the sample calls are made for.
fn asking() -> AvailableBalanceRequest {
    AvailableBalanceRequest {
        organization_id: "acme".to_owned(),
    }
}

/// The credits an operation answered with, and `u32::MAX` where it answered its declared error.
fn credits(answered: Result<AvailableBalanceResponse, BalanceError>) -> u32 {
    answered.map_or(u32::MAX, |balance| balance.available_credits)
}

#[test]
fn the_service_the_readme_declares_is_implementable_at_the_context_it_shows() {
    assert_declared_and_documented(DECLARED_CONTEXT);
    let ctx = UsageContext {
        logger_name: "readme".to_owned(),
    };
    assert_eq!(
        credits(poll_once(UsageBackEnd.get_available_balance(&ctx, asking())).unwrap()),
        4,
        "the README's sample is a service that compiles and answers, not a sketch"
    );
    assert_eq!(
        credits(poll_once(UsageBackEnd.can_generate(&ctx, asking())).unwrap()),
        4,
        "an overridden wire name changes nothing about the Rust that calls it"
    );
    assert_eq!(
        credits(
            poll_once(UsageBackEnd.expire_credit(&ctx, "acme".to_owned(), "cr-1".to_owned()))
                .unwrap()
        ),
        8,
        "a multi-argument operation still takes its arguments separately"
    );
    assert_eq!(
        credits(poll_once(UsageBackEnd.sweep(&ctx)).unwrap()),
        u32::MAX,
        "the error arm is the operation's own, and it is the only failure it can name"
    );
    assert_eq!(
        poll_once(UsageBackEnd.apply_bundle(&ctx, asking())),
        Some(()),
        "a one-way operation answers with nothing at all"
    );
}
