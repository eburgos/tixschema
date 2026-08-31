//! What a declared trait is read into, and every refusal it can earn.
//!
//! The refusals are read off `parse_service` rather than off rendered `compile_error!` tokens, so
//! an assertion compares the text the compiler shows against the text the design specifies,
//! character for character, with no token-rendering escapes in between.

#![cfg(feature = "serde")]

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

fn expanded(source: &str) -> String {
    exec_service_schema(TokenStream::new(), declared(source).to_token_stream()).to_string()
}

fn generated_inputs(operation: &OperationDef) -> Option<&[(Ident, Type)]> {
    match &operation.inputs {
        OperationInputs::Generated(carried) => Some(carried.as_slice()),
        OperationInputs::Empty | OperationInputs::Named(_) => None,
    }
}

fn message_names(service: &ServiceDef) -> Vec<String> {
    service
        .generated_messages
        .iter()
        .map(|declared| declared.ident.to_string())
        .collect()
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

#[test]
fn a_message_is_declared_for_every_operation_that_named_none_and_for_no_other() {
    assert_eq!(
        message_names(&service(MIXED_SERVICE)),
        vec!["ExpireCreditRequest", "SweepRequest"],
        "the argument-list operation and the zero-argument one, and neither of the three that \
         named a message of their own"
    );
}

#[test]
fn a_declared_message_records_the_arguments_in_declaration_order() {
    let read = service(MIXED_SERVICE);
    let declared_message = &read.generated_messages[0];
    assert_eq!(
        declared_message.declared_for.to_string(),
        "expire_credit",
        "the message knows the operation it was declared for, which its documentation names"
    );
    let carried: Vec<(String, String)> = declared_message
        .fields
        .iter()
        .map(|(name, declared_type)| (name.to_string(), spelled(declared_type)))
        .collect();
    assert_eq!(
        carried,
        vec![
            ("organization_id".to_owned(), "OrganizationId".to_owned()),
            ("credit_id".to_owned(), "CreditId".to_owned()),
        ],
        "the emitter writes the fields off this list rather than reading the operation again"
    );
}

#[test]
fn a_message_declared_for_an_operation_taking_nothing_carries_no_fields() {
    let read = service(MIXED_SERVICE);
    let declared_message = &read.generated_messages[1];
    assert_eq!(declared_message.ident.to_string(), "SweepRequest");
    assert!(
        declared_message.fields.is_empty(),
        "an empty message, not the absence of one"
    );
}

#[test]
fn a_declared_message_is_emitted_with_everything_a_hand_written_type_carries() {
    let emitted = expanded(MIXED_SERVICE);
    assert!(
        emitted.contains("pub struct ExpireCreditRequest"),
        "got: {emitted}"
    );
    assert!(
        emitted.contains("pub organization_id : OrganizationId"),
        "got: {emitted}"
    );
    assert!(
        emitted.contains("pub credit_id : CreditId"),
        "got: {emitted}"
    );
    assert!(
        emitted.contains(":: tixschema :: model_schema ()"),
        "a client on the far side has to construct one, so it gets every schema a declared type \
         gets. Got: {emitted}"
    );
    assert!(
        emitted.contains(":: serde :: Serialize") && emitted.contains(":: serde :: Deserialize"),
        "the author never wrote the type and has nowhere to put a derive. Got: {emitted}"
    );
    assert!(
        emitted.contains("rename_all = \"camelCase\""),
        "an argument is snake_case in Rust and camelCase on the wire. Got: {emitted}"
    );
}

#[test]
fn an_operation_taking_nothing_is_emitted_an_empty_message_rather_than_none() {
    let emitted = expanded(MIXED_SERVICE);
    assert!(
        emitted.contains("pub struct SweepRequest { }"),
        "got: {emitted}"
    );
}

#[test]
fn nothing_is_emitted_for_the_operation_whose_argument_already_is_the_message() {
    let emitted = expanded(MIXED_SERVICE);
    assert!(
        !emitted.contains("GetAvailableBalanceRequest"),
        "the argument is the author's own type, reusable and versionable, and a second declaration \
         over it would take that away. Got: {emitted}"
    );
    assert!(
        !emitted.contains("CanGenerateRequest") && !emitted.contains("ApplyBundleRequest {"),
        "got: {emitted}"
    );
}

#[test]
fn a_declared_message_says_in_its_own_documentation_what_its_field_names_cost() {
    let emitted = expanded(MIXED_SERVICE);
    assert!(
        emitted.contains("field names are the operation's parameter names"),
        "renaming a parameter moves a key on the wire, and the rustdoc is where an author meets \
         that before choosing the form. Got: {emitted}"
    );
    assert!(
        emitted.contains("no compiler will flag it"),
        "got: {emitted}"
    );
}

#[test]
fn a_declared_message_colliding_with_a_type_the_service_names_is_refused() {
    let reported = refusals(
        "pub trait UsageService<Ctx> {
            async fn sweep(&self, ctx: &Ctx) -> Result<SweepReport, UsageError>;
            async fn replay(&self, ctx: &Ctx, req: SweepRequest) -> Result<SweepReport, UsageError>;
        }",
    );
    assert_eq!(reported.len(), 1, "got: {reported:?}");
    assert_eq!(
        reported[0],
        "service_schema: operation `sweep` names no message, so `SweepRequest` is declared for \
         it, and operation `replay` already names a type spelled `SweepRequest`\n       \
         one name cannot carry two declarations; rename the operation, or have it take the \
         existing `SweepRequest` as its one argument",
        "the refusal names both declarations, rather than leaving the compiler to report a \
         duplicate definition against a type the author never wrote"
    );
}

#[test]
fn a_declared_message_sharing_a_name_with_a_type_written_elsewhere_is_not_refused() {
    assert!(
        refusals(
            "pub trait UsageService<Ctx> {
                async fn sweep(&self, ctx: &Ctx) -> Result<SweepReport, UsageError>;
                async fn replay(
                    &self,
                    ctx: &Ctx,
                    req: crate::messages::SweepRequest,
                ) -> Result<SweepReport, UsageError>;
            }"
        )
        .is_empty(),
        "a qualified spelling names a type in another module, which a declaration beside the \
         trait does not collide with"
    );
}

#[test]
fn dispatch_is_generic_over_the_implementing_type_and_answers_through_the_handle() {
    let emitted = expanded(MIXED_SERVICE);
    assert!(
        emitted.contains(
            "pub fn dispatch < S , Ctx , R > (svc : & S , ctx : & Ctx , message : & \
             IncomingMessage , reply : & R ,) -> impl :: core :: future :: Future < Output = () > \
             + Send where S : super :: UsageService < Ctx > + Sync , Ctx : Sync , R : Reply + Sync"
        ),
        "it returns nothing, and a trait with `async fn` has no `dyn` form to offer. Got: \
         {emitted}"
    );
    assert!(
        !emitted.contains("& dyn"),
        "no `&dyn` form exists to offer, so none is emitted. Got: {emitted}"
    );
}

#[test]
fn the_dispatcher_and_the_client_are_emitted_inside_the_module_the_constructors_are_private_to() {
    let emitted = expanded(MIXED_SERVICE);
    let module = emitted.find("pub mod usage_service_schema").unwrap();
    let contract = emitted.find("pub trait UsageService").unwrap();
    for inside in ["pub fn dispatch", "pub struct UsageServiceClient"] {
        let at = emitted.find(inside).unwrap();
        assert!(
            at > module && at < contract,
            "`{inside}` has to sit between the module opening and the trait that follows it, or \
             it is not inside the module at all. Got: {emitted}"
        );
    }
}

#[test]
fn every_arm_is_keyed_on_the_wire_name_and_never_on_anything_in_the_payload() {
    let emitted = expanded(MIXED_SERVICE);
    for carried in [
        "\"get-available-balance\" =>",
        "\"expire-credit\" =>",
        "\"sweep\" =>",
        "\"usage-generation-request\" =>",
        "\"apply-bundle\" =>",
    ] {
        assert!(emitted.contains(carried), "got: {emitted}");
    }
    assert!(
        emitted.contains("match message . operation . as_str ()"),
        "the operation is the one the transport read off the wire. Got: {emitted}"
    );
}

#[test]
fn an_arm_validates_before_it_calls_and_faults_on_both_ways_the_message_can_be_wrong() {
    let emitted = expanded(MIXED_SERVICE);
    let deserialized = emitted
        .find("serde_json :: from_slice :: < AvailableBalanceRequest >")
        .unwrap();
    let validated = emitted.find("received . validate ()").unwrap();
    let called = emitted.find("svc . get_available_balance").unwrap();
    assert!(
        deserialized < validated && validated < called,
        "an implementation may assume its incoming message is valid, which only holds if the \
         validator runs before it is entered. Got: {emitted}"
    );
    assert!(
        emitted.contains("ServiceFault :: undeserializable_payload")
            && emitted.contains("ServiceFault :: failed_validation")
            && emitted.contains("ServiceFault :: unknown_operation"),
        "got: {emitted}"
    );
}

#[test]
fn a_one_way_arm_calls_the_implementation_and_then_touches_the_handle_with_nothing() {
    let emitted = expanded(MIXED_SERVICE);
    // From the call to the end of the arm: the two fault guards sit above the call, so anything
    // naming the handle below it would be an answer on a path the operation declared no reply for.
    let called = emitted.find("svc . apply_bundle").unwrap();
    let rest = &emitted[called..];
    let next_arm = rest.find("=>").unwrap_or(rest.len());
    let tail = &rest[..next_arm];
    assert!(
        tail.contains('}'),
        "the slice has to reach the end of the arm or it proves nothing. Got: {tail}"
    );
    assert!(
        !tail.contains("reply ."),
        "nothing about replying belongs on a path that never replies; acknowledgement is the \
         transport adapter's, after `dispatch` returns. Got: {emitted}"
    );
}

#[test]
fn the_client_carries_one_method_per_operation_under_the_operation_s_own_wire_name() {
    let emitted = expanded(MIXED_SERVICE);
    assert!(
        emitted.contains("pub struct UsageServiceClient < T : Transport >"),
        "got: {emitted}"
    );
    for named in [
        "pub fn get_available_balance < Ctx >",
        "pub fn expire_credit < Ctx >",
        "pub fn sweep < Ctx >",
        "pub fn can_generate < Ctx >",
        "pub fn apply_bundle < Ctx >",
    ] {
        assert!(emitted.contains(named), "got: {emitted}");
    }
    assert!(
        emitted.contains("self . transport . request (\"usage-generation-request\" , sending)"),
        "the name the wire carries is the one the transport is handed, beside the payload. Got: \
         {emitted}"
    );
    assert!(
        emitted.contains("self . transport . notify (\"apply-bundle\" , sending)"),
        "a one-way operation is sent rather than called. Got: {emitted}"
    );
}

#[test]
fn a_client_method_validates_before_it_reaches_the_transport() {
    let emitted = expanded(MIXED_SERVICE);
    let validated = emitted.find("sending . validate ()").unwrap();
    let sent = emitted.find("self . transport .").unwrap();
    assert!(
        validated < sent,
        "a message the client refuses never becomes a remote error a round trip later. Got: \
         {emitted}"
    );
    assert!(
        emitted.contains("return Err (CallError :: Fault (ServiceFault :: failed_validation ("),
        "the operation never ran, so it is not one of its declared errors. Got: {emitted}"
    );
}

#[test]
fn a_fault_is_read_back_through_a_private_mirror_rather_than_by_widening_the_fault() {
    let emitted = expanded(MIXED_SERVICE);
    assert!(
        emitted.contains("struct FaultOnTheWire") && !emitted.contains("pub struct FaultOnTheWire"),
        "the mirror is the seam, and it is private to the module. Got: {emitted}"
    );
    assert!(
        emitted.contains("fn into_fault (self) -> ServiceFault"),
        "got: {emitted}"
    );
    // Declared under the name its TypeScript is published as, `ServiceFault` being the alias the
    // module's own generated code writes.
    let fault = emitted.find("pub struct UsageServiceFault").unwrap();
    let derives = &emitted[fault.saturating_sub(200)..fault];
    assert!(
        !derives.contains("Deserialize"),
        "a public `Deserialize` on the fault is a public constructor by another name. Got: \
         {derives}"
    );
    assert!(
        emitted.contains("pub type ServiceFault = UsageServiceFault ;"),
        "the module keeps the unstuttering spelling; only TypeScript needs the prefix. Got: \
         {emitted}"
    );
}

#[test]
fn the_emitted_trait_names_the_operation_a_missing_implementation_is_refused_for() {
    let emitted = rendered(MIXED_SERVICE);
    // rustc's `E0046` names the trait item an implementation left out, so the name a reader is
    // sent to look for is whatever ident the emitted trait declares the operation under. The
    // desugaring rewrites the return type and nothing about the name.
    for declared in [
        "fn apply_bundle",
        "fn can_generate",
        "fn expire_credit",
        "fn get_available_balance",
        "fn sweep",
    ] {
        assert!(
            emitted.contains(declared),
            "an operation rustc cannot name is one a missing implementation is refused for \
             silently. Got: {emitted}"
        );
    }
}

#[test]
fn the_readme_shows_both_one_way_refusals_the_way_the_macro_writes_them() {
    let readme = include_str!("../../README.md");
    for (source, shown) in [
        (
            "pub trait OrganizationService<Ctx> {
                async fn apply_bundle(&self, ctx: &Ctx, req: ApplyBundleRequest);
            }",
            "service_schema: operation `apply_bundle` has no return type\n       \
             add `#[service_schema_op(one_way)]` if it expects no reply,\n       \
             or give it a `Result<Success, Error>` return",
        ),
        (
            "pub trait OrganizationService<Ctx> {
                #[service_schema_op(one_way)]
                async fn apply_bundle(&self, ctx: &Ctx, req: ApplyBundleRequest) -> Result<Ack, E>;
            }",
            "service_schema: operation `apply_bundle` is marked `one_way` but returns a value\n       \
             a one-way operation produces no reply",
        ),
    ] {
        assert_eq!(refusals(source), vec![shown.to_owned()]);
        assert!(
            readme.contains(shown),
            "the README no longer shows this refusal verbatim:\n{shown}"
        );
    }
}
