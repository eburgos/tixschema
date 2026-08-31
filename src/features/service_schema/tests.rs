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

#[cfg(feature = "zod")]
mod client_tests;
#[cfg(feature = "zod")]
mod service_tests;

#[cfg(feature = "zod")]
use super::client;
#[cfg(feature = "zod")]
use super::service;
use super::{emit, result};
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

#[cfg(feature = "zod")]
fn client_of(source: &str) -> String {
    client::emit(&parsed(source)).join("\n\n")
}

fn parsed(source: &str) -> ServiceDef {
    parse_service(&syn::parse_str::<ItemTrait>(source).unwrap()).unwrap()
}

fn registration(source: &str) -> String {
    emit(&parsed(source)).to_token_stream().to_string()
}

#[cfg(feature = "zod")]
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
fn the_fault_s_fields_are_asked_for_rather_than_written_here() {
    let rendered = registration(MIXED_SERVICE);
    for asked in [
        "usage_service_schema :: UsageServiceFaultFields :: ts_definition",
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
    // The seal is written here and the fields are not, so the sealed alias names the asked-for
    // type and spells no member of its own. A field written here is a field that can drift.
    for member in ["detail:", "field:", "kind:", "operation:"] {
        assert!(
            !rendered.contains(&format!("export type UsageServiceFault = {{\\n  {member}")),
            "the seal adds a brand and nothing else; the members stay the Rust declaration's. \
             Got: {rendered}"
        );
    }
}

/// The two declarations the seal is: a symbol the bundle exports nowhere, and the fault a caller
/// names, declared as the asked-for fields plus one property keyed on that symbol.
///
/// This is what TypeScript is given in place of the read-without-construct split Rust has. The Rust
/// fault refuses both routes an implementation would take — `E0451` on the private fields, `E0624`
/// on the private constructors — and a plain structural object type refuses neither.
#[test]
fn the_published_fault_is_the_asked_for_fields_under_a_brand_the_bundle_exports_nowhere() {
    let rendered = registration(MIXED_SERVICE);
    assert!(
        rendered.contains("declare const usageServiceFaultSeal: unique symbol;"),
        "a brand keyed on an exported name is a brand anyone can write. Got: {rendered}"
    );
    assert!(
        !rendered.contains("export declare const usageServiceFaultSeal"),
        "an exported symbol is one an implementation can name, and a property it can write. \
         Got: {rendered}"
    );
    assert!(
        rendered.contains(
            "export type UsageServiceFault = UsageServiceFaultFields & {\\n  readonly \
             [usageServiceFaultSeal]: true;\\n};"
        ),
        "the fault a caller names is the fields the Rust declaration published, plus the brand. \
         Got: {rendered}"
    );
    // The README states both halves where it documents the fault — what the seal stops, and the
    // assertion it does not — so the two cannot drift.
    let readme = include_str!("../../../README.md");
    assert!(
        readme.contains("**And the fault type itself refuses to be written.**")
            && readme.contains("declare const usageServiceFaultSeal: unique symbol;")
            && readme.contains("`built as UsageServiceFault` compiles"),
        "the README no longer says what the seal is, or that a type assertion still gets past it"
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

/// The pair that says a client and a dispatcher are published exactly where their check can be.
/// This is the half that runs in a build with the Zod surface; the one below it is the same
/// registration read in a build without it, and neither could pass alone.
#[cfg(feature = "zod")]
#[test]
fn a_build_that_publishes_a_schema_publishes_the_client_and_the_dispatcher_that_parse_it() {
    let rendered = registration(MIXED_SERVICE);
    for published in [
        "pub fn ts_client",
        "pub fn ts_service",
        "pub fn ts_definition",
    ] {
        assert!(
            rendered.contains(published),
            "a build with a schema to parse against publishes all three artifacts. \
             Got: {rendered}"
        );
    }
}

/// A build with `typescript` on and `zod` off publishes the service's types and neither seam
/// artifact.
///
/// Both of them parse a message against the `<Message>$Schema` const `#[model_schema()]` writes,
/// and this build writes none. Emitting them without the parse is what this replaced: a client that
/// forwarded whatever it was handed, and a dispatcher that narrowed an unread payload with `as` and
/// gave it to an implementation entitled to assume it was valid. Both compiled, both read like the
/// checked ones, and the Rust half of the same service went on validating — so the two halves
/// disagreed about what they accept and nothing said so.
#[cfg(not(feature = "zod"))]
#[test]
fn a_build_that_publishes_no_schema_publishes_no_client_and_no_dispatcher() {
    let rendered = registration(MIXED_SERVICE);
    for withheld in ["pub fn ts_client", "pub fn ts_service"] {
        assert!(
            !rendered.contains(withheld),
            "an artifact that cannot hold the guarantee its callers are written against is not \
             published at all. Got: {rendered}"
        );
    }
    assert!(
        rendered.contains("pub fn ts_definition"),
        "the types describe what the Rust half puts on the wire and are true either way. \
         Got: {rendered}"
    );
    // The README states the consequence where it documents the requirement, so the two cannot
    // drift.
    let readme = include_str!("../../../README.md");
    assert!(
        readme.contains("**A service that publishes TypeScript needs the `zod` feature too.**")
            && readme.contains(
                "no `<Service>Schema::ts_client()` and no `<Service>Schema::ts_service()`"
            ),
        "the README no longer says what a build without the Zod surface publishes"
    );
}

/// The missing methods are the one thing a reader of this build's registry goes looking for, so
/// the reason they are missing is written on the registry itself rather than left to an
/// `E0599` naming the method and nothing else.
#[cfg(not(feature = "zod"))]
#[test]
fn a_build_that_publishes_no_client_says_on_the_registry_why_not() {
    let rendered = registration(MIXED_SERVICE);
    for said in [
        "This build publishes no `UsageServiceSchema::ts_client()` and no \
         `UsageServiceSchema::ts_service()`.",
        "only a build with tixschema's `zod` feature writes one",
        "Add `features = [\\\"zod\\\"]` to the tixschema dependency to get them.",
    ] {
        assert!(
            rendered.contains(said),
            "the registry's own rustdoc names the feature and what to add. Got: {rendered}"
        );
    }
}

/// What the Zod-less build still publishes, and therefore why it is not refused outright: the
/// message types and the result envelopes describe what the *Rust* dispatcher and client put on the
/// wire, and that half validates in this build exactly as it does in any other.
#[cfg(not(feature = "zod"))]
#[test]
fn a_build_that_publishes_no_client_still_publishes_every_type_the_wire_carries() {
    let rendered = registration(MIXED_SERVICE);
    for asked in [
        "ExpireCreditRequest :: ts_definition",
        "usage_service_schema :: UsageServiceFaultFields :: ts_definition",
        "declare const usageServiceFaultSeal: unique symbol;",
        "export type UsageServiceGetAvailableBalanceResult =",
    ] {
        assert!(
            rendered.contains(asked),
            "the types are what a hand-written caller of this service reads, and nothing about \
             them depends on the Zod surface. Got: {rendered}"
        );
    }
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
