//! What a declared trait is read into, and every refusal it can earn.
//!
//! The refusals are read off `parse_service` rather than off rendered `compile_error!` tokens, so
//! an assertion compares the text the compiler shows against the text the design specifies,
//! character for character, with no token-rendering escapes in between.

use super::parse::{OperationDef, OperationInputs, OperationOutcome, ServiceDef, parse_service};
use super::{emitted_trait, exec_service_schema};
use proc_macro2::TokenStream;
use quote::{ToTokens as _, quote};
use syn::{Ident, ItemTrait, Type};

/// A service with one of every input shape, one of every outcome, and an overridden wire name.
const MIXED_SERVICE: &str = r#"
    pub trait UsageService<Ctx> {
        async fn get_available_balance(
            &self,
            ctx: &Ctx,
            req: AvailableBalanceRequest,
        ) -> Result<AvailableBalanceResponse, UsageError>;

        async fn expire_credit(
            &self,
            ctx: &Ctx,
            organization_id: OrganizationId,
            credit_id: CreditId,
        ) -> Result<ExpiredCredit, UsageError>;

        async fn sweep(&self, ctx: &Ctx) -> Result<SweepReport, UsageError>;

        #[service_schema_op(message = "usage-generation-request")]
        async fn can_generate(
            &self,
            ctx: &Ctx,
            req: GenerationRequest,
        ) -> Result<GenerationVerdict, UsageError>;

        #[service_schema_op(one_way)]
        async fn apply_bundle(&self, ctx: &Ctx, req: ApplyBundleRequest);
    }
"#;

fn declared(source: &str) -> ItemTrait {
    syn::parse_str::<ItemTrait>(source).unwrap()
}

fn generated_inputs(operation: &OperationDef) -> Option<&[(Ident, Type)]> {
    match &operation.inputs {
        OperationInputs::Generated(carried) => Some(carried.as_slice()),
        OperationInputs::Empty | OperationInputs::Named(_) => None,
    }
}

fn named_input(operation: &OperationDef) -> Option<&Type> {
    match &operation.inputs {
        OperationInputs::Named(declared_type) => Some(declared_type.as_ref()),
        OperationInputs::Empty | OperationInputs::Generated(_) => None,
    }
}

fn refusals(source: &str) -> Vec<String> {
    parse_service(&declared(source))
        .err()
        .map(|refusal| refusal.into_iter().map(|one| one.to_string()).collect())
        .unwrap_or_default()
}

fn rendered(source: &str) -> String {
    emitted_trait(&declared(source))
        .to_token_stream()
        .to_string()
}

fn reply_arms(operation: &OperationDef) -> Option<(&Type, &Type)> {
    match &operation.outcome {
        OperationOutcome::Reply { error, success } => Some((success, error)),
        OperationOutcome::OneWay => None,
    }
}

fn service(source: &str) -> ServiceDef {
    parse_service(&declared(source)).unwrap()
}

fn spelled(declared_type: &Type) -> String {
    declared_type.to_token_stream().to_string()
}

#[test]
fn a_trait_with_no_type_parameter_names_the_context_requirement() {
    assert_eq!(
        refusals("pub trait UsageService { }"),
        vec![
            "service_schema: trait `UsageService` declares no context type parameter\n       \
             give it one, as in `trait UsageService<Ctx>`, and take it in every operation"
        ],
        "a trait with nothing to hand an implementation has to say so"
    );
}

#[test]
fn an_operation_marked_one_way_that_returns_a_value_is_refused() {
    assert_eq!(
        refusals(
            "pub trait OrganizationService<Ctx> {
                #[service_schema_op(one_way)]
                async fn apply_bundle(&self, ctx: &Ctx, req: ApplyBundleRequest) -> Result<Ack, E>;
            }"
        ),
        vec![
            "service_schema: operation `apply_bundle` is marked `one_way` but returns a value\n       \
             a one-way operation produces no reply"
        ],
        "the flag and the return type have to agree in this direction too"
    );
}

#[test]
fn an_operation_not_taking_self_is_refused() {
    assert_eq!(
        refusals(
            "pub trait UsageService<Ctx> {
                async fn sweep(ctx: &Ctx) -> Result<SweepReport, UsageError>;
            }"
        ),
        vec![
            "service_schema: operation `sweep` does not take `&self`\n       \
             an operation is called on the service value, so `&self` comes first"
        ],
        "the dispatcher calls the operation on a service value"
    );
}

#[test]
fn an_operation_not_taking_the_context_is_refused_naming_the_context_type() {
    assert_eq!(
        refusals(
            "pub trait UsageService<Ctx> {
                async fn sweep(&self, req: SweepRequest) -> Result<SweepReport, UsageError>;
            }"
        ),
        vec![
            "service_schema: operation `sweep` does not take the context\n       \
             every operation takes `ctx: &Ctx` as its first argument after `&self`"
        ],
        "the refusal names the context type the trait actually declared"
    );
}

#[test]
fn an_operation_returning_something_other_than_a_result_is_refused() {
    assert_eq!(
        refusals(
            "pub trait UsageService<Ctx> {
                async fn sweep(&self, ctx: &Ctx) -> SweepReport;
            }"
        ),
        vec![
            "service_schema: operation `sweep` must return `Result<Success, Error>`\n       \
             an operation declares its success type and its error type in one signature"
        ],
        "a success arm with no error arm is not a service operation"
    );
}

#[test]
fn an_operation_taking_no_arguments_after_the_context_receives_an_empty_message() {
    let read = service(MIXED_SERVICE);
    let sweep = &read.operations[2];
    assert!(
        matches!(sweep.inputs, OperationInputs::Empty),
        "got: {}",
        sweep.wire_name
    );
    assert!(
        generated_inputs(sweep).is_none() && named_input(sweep).is_none(),
        "an operation with no arguments declares no message of its own"
    );
}

#[test]
fn an_operation_taking_one_argument_after_the_context_is_already_a_message() {
    let read = service(MIXED_SERVICE);
    let balance = &read.operations[0];
    assert_eq!(
        spelled(named_input(balance).unwrap()),
        "AvailableBalanceRequest",
        "the one argument is the message, as declared"
    );
    assert!(
        generated_inputs(balance).is_none(),
        "nothing is declared for an operation that already named its message"
    );
}

#[test]
fn an_operation_taking_several_arguments_carries_them_in_declaration_order() {
    let read = service(MIXED_SERVICE);
    let expire = &read.operations[1];
    let carried: Vec<(String, String)> = generated_inputs(expire)
        .unwrap()
        .iter()
        .map(|(name, declared_type)| (name.to_string(), spelled(declared_type)))
        .collect();
    assert_eq!(
        carried,
        vec![
            ("organization_id".to_owned(), "OrganizationId".to_owned()),
            ("credit_id".to_owned(), "CreditId".to_owned()),
        ],
        "each argument's name becomes a field on the declared message, so the order is the wire's"
    );
    assert!(
        named_input(expire).is_none(),
        "the argument list is the declaration, so no single argument is the message"
    );
}

#[test]
fn an_unknown_directive_is_refused_naming_the_ones_that_exist() {
    let reported = refusals(
        "pub trait UsageService<Ctx> {
            #[service_schema_op(fire_and_forget)]
            async fn apply_bundle(&self, ctx: &Ctx, req: ApplyBundleRequest);
        }",
    );
    assert_eq!(reported.len(), 1, "got: {reported:?}");
    assert!(
        reported[0].contains("unknown `service_schema_op` directive"),
        "got: {}",
        reported[0]
    );
}

#[test]
fn both_result_arms_are_carried_separately() {
    let read = service(MIXED_SERVICE);
    let (success, error) = reply_arms(&read.operations[0]).unwrap();
    assert_eq!(
        spelled(success),
        "AvailableBalanceResponse",
        "the success arm is declared, not inferred"
    );
    assert_eq!(
        spelled(error),
        "UsageError",
        "the error arm is declared, not inferred"
    );
}

#[test]
fn every_refusal_a_service_earns_is_reported_in_one_build() {
    assert_eq!(
        refusals(
            "pub trait UsageService<Ctx> {
                async fn apply_bundle(&self, ctx: &Ctx, req: ApplyBundleRequest);
                async fn sweep(&self, req: SweepRequest) -> Result<SweepReport, UsageError>;
            }"
        )
        .len(),
        2,
        "an author fixing a service sees everything wrong with it at once"
    );
}

#[test]
fn the_context_type_parameter_is_read_off_the_trait() {
    let read = service(MIXED_SERVICE);
    assert_eq!(
        read.ident.to_string(),
        "UsageService",
        "the trait as declared"
    );
    assert_eq!(
        read.context_param.to_string(),
        "Ctx",
        "the context parameter"
    );
    assert_eq!(read.operations.len(), 5, "every operation is read");
}

#[test]
fn the_emitted_trait_carries_the_context_and_desugars_every_async_operation() {
    let emitted = rendered(MIXED_SERVICE);
    assert!(!emitted.contains("async fn"), "got: {emitted}");
    assert!(
        emitted.contains("trait UsageService < Ctx >"),
        "got: {emitted}"
    );
    assert!(
        emitted.contains(
            "-> impl :: core :: future :: Future < Output = Result < AvailableBalanceResponse , UsageError > > + Send"
        ),
        "got: {emitted}"
    );
}

#[test]
fn the_emitted_trait_desugars_a_one_way_operation_to_an_empty_output() {
    let emitted = rendered(MIXED_SERVICE);
    assert!(
        emitted.contains("-> impl :: core :: future :: Future < Output = () > + Send"),
        "got: {emitted}"
    );
}

#[test]
fn the_emitted_trait_no_longer_carries_the_per_operation_directives() {
    let emitted = rendered(MIXED_SERVICE);
    assert!(!emitted.contains("service_schema_op"), "got: {emitted}");
}

#[test]
fn the_expansion_emits_the_trait_beside_the_refusal_so_the_refusal_is_what_gets_reported() {
    let expanded = exec_service_schema(
        TokenStream::new(),
        quote! {
            pub trait UsageService<Ctx> {
                async fn apply_bundle(&self, ctx: &Ctx, req: ApplyBundleRequest);
            }
        },
    )
    .to_string();
    assert!(expanded.contains("compile_error"), "got: {expanded}");
    assert!(expanded.contains("has no return type"), "got: {expanded}");
    assert!(
        expanded.contains("trait UsageService < Ctx >"),
        "got: {expanded}"
    );
}

#[test]
fn the_message_override_moves_the_wire_name_and_nothing_else() {
    let read = service(MIXED_SERVICE);
    let can_generate = &read.operations[3];
    assert_eq!(
        can_generate.ident.to_string(),
        "can_generate",
        "Rust still calls it by the method name"
    );
    assert_eq!(
        can_generate.ts_name, "canGenerate",
        "TypeScript still calls it by the camelCased name"
    );
    assert_eq!(
        can_generate.wire_name, "usage-generation-request",
        "only the wire name moves"
    );
}

#[test]
fn the_missing_return_type_refusal_names_both_choices() {
    assert_eq!(
        refusals(
            "pub trait OrganizationService<Ctx> {
                async fn apply_bundle(&self, ctx: &Ctx, req: ApplyBundleRequest);
            }"
        ),
        vec![
            "service_schema: operation `apply_bundle` has no return type\n       \
             add `#[service_schema_op(one_way)]` if it expects no reply,\n       \
             or give it a `Result<Success, Error>` return"
        ],
        "a forgotten Result must not become a silent fire-and-forget"
    );
}

#[test]
fn the_one_way_flag_is_recognised_and_leaves_no_reply_to_carry() {
    let read = service(MIXED_SERVICE);
    let apply_bundle = &read.operations[4];
    assert!(
        matches!(apply_bundle.outcome, OperationOutcome::OneWay),
        "got: {}",
        apply_bundle.wire_name
    );
    assert_eq!(
        apply_bundle.wire_name, "apply-bundle",
        "a greenfield operation writes no attribute and gets the kebab-cased name"
    );
    assert_eq!(
        apply_bundle.ts_name, "applyBundle",
        "and the camelCased one"
    );
}

#[test]
fn the_three_spellings_of_an_operation_name_are_all_derived_from_one_declaration() {
    let read = service(MIXED_SERVICE);
    let balance = &read.operations[0];
    assert_eq!(
        balance.ident.to_string(),
        "get_available_balance",
        "the Rust spelling"
    );
    assert_eq!(balance.ts_name, "getAvailableBalance", "the TypeScript one");
    assert_eq!(balance.wire_name, "get-available-balance", "the wire one");
}

#[test]
fn two_operations_carrying_one_wire_name_are_refused() {
    let reported = refusals(
        "pub trait UsageService<Ctx> {
            async fn sweep(&self, ctx: &Ctx) -> Result<SweepReport, UsageError>;
            #[service_schema_op(message = \"sweep\")]
            async fn can_generate(&self, ctx: &Ctx) -> Result<GenerationVerdict, UsageError>;
        }",
    );
    assert_eq!(reported.len(), 1, "got: {reported:?}");
    assert_eq!(
        reported[0],
        "service_schema: trait `UsageService` carries the wire name `sweep` on two operations\n       \
         `sweep` and `can_generate` would be indistinguishable on the wire; move one with \
         `#[service_schema_op(message = \"...\")]`",
        "an override can collide with a name another operation derived"
    );
}

#[test]
fn two_operations_spelled_the_same_in_typescript_are_refused() {
    let reported = refusals(
        "pub trait UsageService<Ctx> {
            async fn get_balance(&self, ctx: &Ctx) -> Result<BalanceResponse, UsageError>;
            async fn getBalance(&self, ctx: &Ctx) -> Result<BalanceResponse, UsageError>;
        }",
    );
    assert_eq!(reported.len(), 1, "got: {reported:?}");
    assert!(
        reported[0].contains("spells two operations `getBalance` in TypeScript"),
        "got: {}",
        reported[0]
    );
}

#[test]
fn an_operation_putting_the_context_on_the_wire_is_refused() {
    let reported = refusals(
        "pub trait UsageService<Ctx> {
            async fn sweep(&self, ctx: &Ctx, carried: Vec<Ctx>) -> Result<SweepReport, UsageError>;
        }",
    );
    assert_eq!(reported.len(), 1, "got: {reported:?}");
    assert_eq!(
        reported[0],
        "service_schema: operation `sweep` puts the context type `Ctx` on the wire\n       \
         the context reaches no message and no schema, so it belongs in neither the arguments nor \
         either result arm",
        "the context never crosses the wire, in an argument or in a result arm"
    );
}

#[test]
fn a_result_arm_naming_the_context_is_refused_too() {
    let reported = refusals(
        "pub trait UsageService<Ctx> {
            async fn sweep(&self, ctx: &Ctx, req: SweepRequest) -> Result<Ctx, UsageError>;
        }",
    );
    assert_eq!(reported.len(), 1, "got: {reported:?}");
    assert!(
        reported[0].contains("puts the context type `Ctx` on the wire"),
        "got: {}",
        reported[0]
    );
}
