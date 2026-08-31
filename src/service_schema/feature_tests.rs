//! The one thing this module says in a build that has no `serde` feature: a service is refused,
//! and the refusal names the feature and what to add.
//!
//! Ungated on purpose, and the only part of `service_schema` that is. Everything else the construct
//! emits is gated on the feature, so the crate's other unit tests cannot speak for the build where
//! it is missing — which is precisely the build this decision exists for.

use super::exec_service_schema;
use proc_macro2::TokenStream;
use quote::ToTokens as _;
use syn::ItemTrait;

/// A service with one of every input shape, one of every outcome, and an overridden wire name —
/// the same declaration the feature-gated unit tests read, so the pair below is about the feature
/// and nothing else.
const MIXED_SERVICE: &str = "
    pub trait UsageService<Ctx> {
        async fn get_available_balance(
            &self,
            ctx: &Ctx,
            req: AvailableBalanceRequest,
        ) -> Result<AvailableBalanceResponse, UsageError>;

        async fn sweep(&self, ctx: &Ctx) -> Result<SweepReport, UsageError>;

        #[service_schema_op(one_way)]
        async fn apply_bundle(&self, ctx: &Ctx, req: ApplyBundleRequest);
    }
";

fn expanded(source: &str) -> String {
    let declared = syn::parse_str::<ItemTrait>(source).unwrap();
    exec_service_schema(TokenStream::new(), declared.to_token_stream()).to_string()
}

/// A build without the `serde` feature refuses a service, and says in one sentence what to add.
///
/// The refusal is asserted whole, because the message *is* the decision as a user meets it. Which
/// error a build earns was read off a standalone compile of the same declaration rather than off
/// this assertion — token matching is as blind to that as a `compile_fail` doctest is — and it was
/// the only error the file earned:
///
/// ```text
/// error: service_schema: a service needs tixschema's `serde` feature, and this build does not have it
///               without it the TypeScript a service publishes names Rust fields rather than the camelCase
///               and kebab-case keys its own dispatcher writes, so the two halves disagree about the wire
///               add `features = ["serde"]` to the tixschema dependency in Cargo.toml
///   --> tests/zz_probe.rs:15:11
///    |
/// 15 | pub trait UsageService<Ctx> {
///    |           ^^^^^^^^^^^^
///
/// error: could not compile `tixschema` (test "zz_probe") due to 1 previous error
/// ```
///
/// That same file under `--features serde,typescript` compiles, so the refusal is the feature's
/// absence and nothing else about the program. The companion below is the token-level half of that
/// pair, and it runs in every combination that does carry the feature.
#[cfg(not(feature = "serde"))]
#[test]
fn a_build_without_the_serde_feature_refuses_a_service_and_says_what_to_add() {
    let emitted = expanded(MIXED_SERVICE);
    // The README shows this message where it documents the requirement, so the two cannot drift.
    let readme = include_str!("../../README.md");
    let said = "service_schema: a service needs tixschema's `serde` feature, and this build does \
                not have it\n       without it the TypeScript a service publishes names Rust \
                fields rather than the camelCase\n       and kebab-case keys its own dispatcher \
                writes, so the two halves disagree about the wire\n       add `features = \
                [\"serde\"]` to the tixschema dependency in Cargo.toml";
    // The token stream renders a string literal escaped, which is what `{:?}` writes.
    assert!(
        emitted.contains(&format!("compile_error ! {{ {said:?} }}")),
        "the refusal a user reads is the whole of this decision. Got: {emitted}"
    );
    assert!(
        readme.contains(said),
        "the README no longer shows this refusal verbatim:\n{said}"
    );
}

/// The other half of the pair: the same declaration, in a build that has the feature, earns no
/// refusal at all. Without this, the assertion above would still pass if the macro refused every
/// service in every combination.
#[cfg(feature = "serde")]
#[test]
fn a_build_with_the_serde_feature_earns_no_refusal_for_the_same_declaration() {
    let emitted = expanded(MIXED_SERVICE);
    assert!(
        !emitted.contains("compile_error !"),
        "a service is refused only for the feature's absence, and this build has it. Got: {emitted}"
    );
}
