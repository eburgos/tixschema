//! What the service publishes to TypeScript, read off the strings themselves.
//!
//! The rendered TypeScript is asserted as text rather than as tokens, because text is what a bundle
//! writes to a `.ts` file and what a TypeScript compiler then reads.
//!
//! **What text assertions do and do not prove.** No TypeScript toolchain is reachable from this
//! repository — no `tsc`, no `package.json`, no `node_modules` — so nothing here type-checks the
//! bundle. These tests read structure: that a member is required rather than optional, that a name
//! carries the service, that the transport is named only on the far side of the validation check.
//! They cannot prove the emitted file compiles, and they cannot prove that an implementation
//! missing a method is rejected where it reaches the factory — only a compiler can, and the gap
//! itself is tracked separately.

mod the_client;
mod the_implementable_service;

use super::{client, emit, result, service};
use crate::service_schema::parse::{ServiceDef, parse_service};
use quote::ToTokens as _;
use syn::ItemTrait;

/// A service with one of every input shape and one of every outcome: a named message, an argument
/// list, no arguments at all, and a one-way operation that answers nothing.
const MIXED_SERVICE: &str = "
    pub trait UsageService<Ctx> {
        async fn get_available_balance(
            &self,
            ctx: &Ctx,
            req: AvailableBalanceRequest,
        ) -> Result<AvailableBalanceResponse, BalanceError>;

        async fn expire_credit(
            &self,
            ctx: &Ctx,
            organization_id: OrganizationId,
            credit_id: CreditId,
        ) -> Result<ExpiredCredit, CreditWriteError>;

        async fn sweep(&self, ctx: &Ctx) -> Result<SweepReport, BalanceError>;

        #[service_schema_op(one_way)]
        async fn apply_bundle(&self, ctx: &Ctx, req: ApplyBundleRequest);
    }
";

fn client_of(source: &str) -> String {
    client::emit(&parsed(source)).join("\n\n")
}

fn parsed(source: &str) -> ServiceDef {
    parse_service(&syn::parse_str::<ItemTrait>(source).unwrap()).unwrap()
}

fn registration(source: &str) -> String {
    emit(&parsed(source)).to_token_stream().to_string()
}

fn service_of(source: &str) -> String {
    service::emit(&parsed(source)).join("\n\n")
}

#[test]
fn a_one_way_operation_gets_no_result_type() {
    let published = result::emit(&parsed(MIXED_SERVICE));
    assert_eq!(published.len(), 3, "got: {published:?}");
    assert!(
        !published
            .iter()
            .any(|ts| ts.contains("UsageServiceApplyBundleResult")),
        "an operation that declared no reply has no arms to join. Got: {published:?}"
    );
}

#[test]
fn every_declared_message_is_registered_with_the_service() {
    let rendered = registration(MIXED_SERVICE);
    for declared in ["ExpireCreditRequest", "SweepRequest"] {
        assert!(
            rendered.contains(&format!("{declared} :: ts_definition")),
            "a message the macro declared reaches the bundle through the service's own line. \
             Got: {rendered}"
        );
    }
    assert!(
        !rendered.contains("AvailableBalanceRequest :: ts_definition"),
        "the message the author declared is registered by the author, not here. Got: {rendered}"
    );
}

#[test]
fn the_bundle_line_hangs_off_a_struct_named_for_the_service() {
    let rendered = registration(MIXED_SERVICE);
    assert!(
        rendered.contains("pub struct UsageServiceSchema"),
        "got: {rendered}"
    );
    assert!(
        rendered.contains("impl UsageServiceSchema"),
        "got: {rendered}"
    );
}

#[test]
fn the_fault_is_asked_for_rather_than_written_here() {
    let rendered = registration(MIXED_SERVICE);
    for asked in [
        "usage_service_schema :: UsageServiceFault :: ts_definition",
        "usage_service_schema :: UsageServiceFaultKind :: ts_definition",
    ] {
        assert!(
            rendered.contains(asked),
            "the fault's TypeScript comes from the same declaration the Rust dispatcher builds \
             faults from, never from a literal beside it. Got: {rendered}"
        );
    }
    assert!(
        !rendered.contains("export type ServiceFault ="),
        "a hand-maintained literal beside a generated type is how the two drift. Got: {rendered}"
    );
}

#[test]
fn the_result_joins_the_two_declared_arms_and_adds_nothing_to_either() {
    let published = result::emit(&parsed(MIXED_SERVICE));
    let found = published
        .iter()
        .find(|ts| ts.contains("export type UsageServiceGetAvailableBalanceResult ="));
    assert!(found.is_some(), "got: {published:?}");
    let balance = found.unwrap();
    assert!(
        balance.contains("| { ok: true; value: AvailableBalanceResponse }"),
        "got: {balance}"
    );
    assert!(
        balance.contains(
            "| { ok: false; error: BalanceError | { isServiceFault: true; fault: \
             UsageServiceFault } };"
        ),
        "got: {balance}"
    );
}

#[test]
fn the_result_takes_its_name_from_the_service_and_the_operation() {
    let published = result::emit(&parsed(MIXED_SERVICE));
    for named in [
        "UsageServiceGetAvailableBalanceResult",
        "UsageServiceExpireCreditResult",
        "UsageServiceSweepResult",
    ] {
        assert!(
            published
                .iter()
                .any(|ts| ts.contains(&format!("export type {named} ="))),
            "a bundle carrying ten services is one flat file, so every name carries the service. \
             Got: {published:?}"
        );
    }
    assert!(
        !published
            .iter()
            .any(|ts| ts.contains("export type SweepResult =")),
        "an unprefixed result collides with any other service declaring the same operation. \
         Got: {published:?}"
    );
}

#[test]
fn two_operations_naming_unrelated_errors_keep_them_apart() {
    let published = result::emit(&parsed(MIXED_SERVICE));
    let found = published
        .iter()
        .find(|ts| ts.contains("export type UsageServiceExpireCreditResult ="));
    assert!(found.is_some(), "got: {published:?}");
    let expire = found.unwrap();
    assert!(
        expire.contains("error: CreditWriteError |"),
        "an operation's failure arm carries the error that operation declared, not the service's. \
         Got: {expire}"
    );
}

#[cfg(feature = "zod")]
#[test]
fn a_declared_message_brings_its_schema_along_with_its_type() {
    let rendered = registration(MIXED_SERVICE);
    assert!(
        rendered.contains("ExpireCreditRequest :: zod_schema"),
        "the schema has no registration line of its own either. Got: {rendered}"
    );
}
