//! What a declared trait is read into, and every refusal it can earn.
//!
//! The refusals are read off `parse_service` rather than off rendered `compile_error!` tokens, so
//! an assertion compares the text the compiler shows against the text the design specifies,
//! character for character, with no token-rendering escapes in between.

#![cfg(feature = "serde")]

use super::parse::{OperationDef, OperationInputs, OperationOutcome, ServiceDef, parse_service};
use super::{emitted_trait, exec_service_schema};
use crate::model_schema::exec_model_schema;
use core::mem::take;
use proc_macro2::{Delimiter, Group, Span, TokenStream, TokenTree};
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

/// A service declaring no operation at all, whose dispatcher has only the arm that answers a name
/// nothing recognises.
const BARE_SERVICE: &str = "
    pub trait BareService<Ctx> {}
";

/// A service whose every operation expects no reply, so nothing it emits ever reads an answer back.
const ONE_WAY_SERVICE: &str = "
    pub trait NoteService<Ctx> {
        #[service_schema_op(one_way)]
        async fn apply_bundle(&self, ctx: &Ctx, req: ApplyBundleRequest);

        #[service_schema_op(one_way)]
        async fn note(&self, ctx: &Ctx, slug: String);
    }
";

/// One item as either macro body emits it: what its doc attributes said, everything ahead of the
/// block it opens, the keyword that opened it, and that block.
struct EmittedItem {
    block: TokenStream,
    docs: String,
    head: String,
    keyword: String,
}

fn declared(source: &str) -> ItemTrait {
    syn::parse_str::<ItemTrait>(source).unwrap()
}

fn expanded(source: &str) -> String {
    exec_service_schema(TokenStream::new(), declared(source).to_token_stream()).to_string()
}

/// The same expansion for a service that asked for the one transport this version has, which is
/// what carries both a dispatcher and a client.
fn expanded_over_amqp_rpc(source: &str) -> String {
    expansion_over_amqp_rpc(source).to_string()
}

fn expansion_over_amqp_rpc(source: &str) -> TokenStream {
    exec_service_schema(
        quote! { transports = ["amqp_rpc"] },
        declared(source).to_token_stream(),
    )
}

/// Whether every mention of `named` in the macro body is written under `qualifier`. A mention that
/// is part of a longer identifier — `serde_named_field` carrying `named_field` — is not one.
fn every_mention_is_qualified(body: &str, named: &str, qualifier: &str) -> bool {
    body.match_indices(named)
        .map(|(at, _)| &body[..at])
        .filter(|before| !before.ends_with(|last: char| last.is_alphanumeric() || last == '_'))
        .all(|before| before.ends_with(qualifier))
}

/// One published macro's stored tokens and nothing beside them, with every literal blanked so a
/// name a doc comment mentions is not read as a path.
fn macro_body(source: &str, named: &str) -> String {
    macro_rules_stream(without_literals(expansion_over_amqp_rpc(source)), named).to_string()
}

/// The same tokens with their literals left as written, for a test reading what the body says
/// rather than which names it reaches.
fn published_macro(source: &str, named: &str) -> String {
    macro_rules_stream(expansion_over_amqp_rpc(source), named).to_string()
}

/// Every item a token stream declares, in the order it declares them.
///
/// Read off tokens rather than through `syn`, a body carrying `$crate` being no parseable item. An
/// item runs to the first brace-delimited group after it, and its keyword is the first one that
/// opens an item - which is why the `impl` in `-> impl Future` is never read as one, `fn` having
/// opened that item several tokens earlier.
fn emitted_items(items: TokenStream) -> Vec<EmittedItem> {
    let mut declared = Vec::new();
    let mut docs = String::new();
    let mut head = String::new();
    let mut keyword = String::new();
    let mut attribute = false;
    for token in items {
        match token {
            TokenTree::Group(carried) if carried.delimiter() == Delimiter::Brace => {
                declared.push(EmittedItem {
                    block: carried.stream(),
                    docs: take(&mut docs),
                    head: take(&mut head),
                    keyword: take(&mut keyword),
                });
                attribute = false;
            }
            TokenTree::Group(carried) if attribute && carried.delimiter() == Delimiter::Bracket => {
                attribute = false;
                let written = carried.stream().to_string();
                if let Some(said) = written.strip_prefix("doc = ") {
                    docs.push_str(said);
                    docs.push('\n');
                }
            }
            // An attribute's `#` is left out of the head, so a documented item's head opens on
            // whatever the item itself opens on.
            TokenTree::Punct(punct) if punct.as_char() == '#' => attribute = true,
            other @ (TokenTree::Group(_)
            | TokenTree::Ident(_)
            | TokenTree::Punct(_)
            | TokenTree::Literal(_)) => {
                attribute = false;
                if let TokenTree::Ident(spelled) = &other {
                    let named = spelled.to_string();
                    if keyword.is_empty() && opens_an_item(&named) {
                        keyword = named;
                    }
                }
                head.push_str(&other.to_string());
                head.push(' ');
            }
        }
    }
    declared
}

/// Whether a keyword is one that opens an item this walk classifies. `impl` is among them, so an
/// `impl` block is an item; the `impl` of an `impl Trait` return type is not, its item having been
/// opened by the `fn` ahead of it.
fn opens_an_item(keyword: &str) -> bool {
    matches!(
        keyword,
        "enum" | "fn" | "impl" | "struct" | "trait" | "type" | "union"
    )
}

/// Every `pub fn` a body publishes, free or inherent, as its documentation beside its signature.
/// `impl` blocks are walked into for the same reason the lint reaches through them: an inherent
/// method answering a `Result` is as public as a free function answering one.
fn published_functions(items: TokenStream) -> Vec<(String, String)> {
    let mut published = Vec::new();
    for item in emitted_items(items) {
        match item.keyword.as_str() {
            "impl" => published.extend(published_functions(item.block)),
            "fn" if item.head.starts_with("pub ") => published.push((item.docs, item.head)),
            _ => (),
        }
    }
    published
}

/// The items one published macro stores: the transcriber of its one rule, with the matcher and the
/// arrow ahead of it left behind, so a walk sees items and nothing else.
fn macro_rules_items(source: &str, named: &str) -> TokenStream {
    macro_rules_stream(expansion_over_amqp_rpc(source), named)
        .into_iter()
        .find_map(|token| match token {
            TokenTree::Group(carried) if carried.delimiter() == Delimiter::Brace => {
                Some(carried.stream())
            }
            TokenTree::Group(_)
            | TokenTree::Ident(_)
            | TokenTree::Punct(_)
            | TokenTree::Literal(_) => None,
        })
        .unwrap()
}

/// The tokens the service's own module holds, read off the expansion the same way a macro's rules
/// are: the module runs to a brace, and it is the one place the root anchors sit.
fn module_body(expansion: TokenStream, named: &str) -> TokenStream {
    let mut reached_the_keyword = false;
    let mut reached_the_name = false;
    let mut held = None;
    for token in expansion {
        match token {
            TokenTree::Ident(spelled) if spelled == "mod" => reached_the_keyword = true,
            TokenTree::Ident(spelled) => {
                reached_the_name = reached_the_keyword && spelled == named;
                reached_the_keyword = false;
            }
            TokenTree::Group(carried) if reached_the_name => {
                held = Some(carried.stream());
                reached_the_name = false;
            }
            TokenTree::Group(_) | TokenTree::Punct(_) | TokenTree::Literal(_) => (),
        }
    }
    held.unwrap()
}

/// Every span carried by an ident spelled `named` anywhere in `tokens`, groups walked into.
fn spans_of_idents_named(tokens: TokenStream, named: &str) -> Vec<Span> {
    let mut found = Vec::new();
    for token in tokens {
        match token {
            TokenTree::Group(carried) => {
                found.extend(spans_of_idents_named(carried.stream(), named));
            }
            TokenTree::Ident(spelled) if spelled == named => found.push(spelled.span()),
            TokenTree::Ident(_) | TokenTree::Punct(_) | TokenTree::Literal(_) => (),
        }
    }
    found
}

/// The bound written on `S` at the first `opening`, up to whatever ends it. The path's argument
/// list is part of what comes back, which is what makes two of them comparable.
fn bound_on_s(text: &str, opening: &str) -> String {
    let at = text.find(opening).unwrap() + opening.len();
    let rest = &text[at..];
    rest[..rest.find(['+', ',']).unwrap()].trim().to_owned()
}

/// The rules a `macro_rules!` published under `named` stores, read off the expansion's own tokens
/// rather than off a slice of its rendering: the expansion publishes more than one macro, and each
/// runs to a brace a search over text has to match for itself.
fn macro_rules_stream(expansion: TokenStream, named: &str) -> TokenStream {
    let mut reached_the_keyword = false;
    let mut reached_the_name = false;
    let mut rules = None;
    for token in expansion {
        match token {
            TokenTree::Ident(spelled) if spelled == "macro_rules" => reached_the_keyword = true,
            TokenTree::Ident(spelled) => {
                reached_the_name = reached_the_keyword && spelled == named;
                reached_the_keyword = false;
            }
            TokenTree::Group(carried) if reached_the_name => {
                rules = Some(carried.stream());
                reached_the_name = false;
            }
            TokenTree::Group(_) | TokenTree::Punct(_) | TokenTree::Literal(_) => (),
        }
    }
    rules.unwrap()
}

fn without_literals(tokens: TokenStream) -> TokenStream {
    tokens
        .into_iter()
        .map(|token| match token {
            TokenTree::Group(group) => TokenTree::Group(Group::new(
                group.delimiter(),
                without_literals(group.stream()),
            )),
            TokenTree::Literal(_) => TokenTree::Ident(Ident::new("literal", Span::call_site())),
            other @ (TokenTree::Ident(_) | TokenTree::Punct(_)) => other,
        })
        .collect()
}

/// Whether `declaration` carries `#[model_schema()]`, which is what publishes its `ts_definition()`,
/// `zod_schema()` and `json_schema()` under their respective features. The attribute is the item's
/// own only where nothing else is declared between the two.
fn is_described(emitted: &str, declaration: &str) -> bool {
    let at = emitted.find(declaration).unwrap();
    let before = &emitted[..at];
    before
        .rfind("# [:: tixschema :: model_schema ()]")
        .is_some_and(|annotated| !before[annotated..].contains("pub "))
}

/// Whether `declaration` carries a doc comment of its own, which is the `#[doc = "…"]` the tokens
/// immediately ahead of it spell. A `#[derive(…)]` or a `#[serde(…)]` closes on `)]` rather than on
/// a string, so neither is read as one.
fn is_documented(body: &str, declaration: &str) -> bool {
    let at = body.find(declaration).unwrap();
    body[..at].trim_end().ends_with("\"]")
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

/// The one import that decides where a service may be declared.
///
/// The generated module reaches the trait and every message type the author declared beside it
/// through `super`, so a declaration written inside a function body resolves none of them: a module
/// nested in a function body has the enclosing module as its parent, not the function. The macro
/// cannot refuse that placement — an attribute macro is handed the annotated item's tokens and
/// nothing about the scope around them — so this reads back the mechanism instead, and the doctest
/// pair on `support::emit` reads the four errors a function-scoped declaration earns.
#[test]
fn the_generated_module_reaches_the_author_s_declarations_through_super() {
    let emitted = expanded(MIXED_SERVICE);
    assert!(
        emitted.contains("pub mod usage_service_schema { use super :: * ;"),
        "the module opens on `use super::*;`, which is the whole of how it reaches what the author \
         declared beside the trait. Got: {emitted}"
    );
    assert!(
        !emitted.contains("use self :: * ;") && !emitted.contains("use crate :: * ;"),
        "no other import reaches the author's scope, so `super` is the only path there. \
         Got: {emitted}"
    );
    // The README states the requirement where it documents the construct, so the two cannot drift.
    let readme = include_str!("../../README.md");
    assert!(
        readme.contains("**A service is declared at module scope, never inside a function body.**")
            && readme.contains("error[E0405]: cannot find trait `UsageService` in module `super`"),
        "the README no longer states where a service may be declared, or what a function-scoped \
         one earns"
    );
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
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
    assert!(
        emitted.contains(
            "pub fn dispatch < S , Ctx , R > (svc : & S , ctx : & Ctx , message : & \
             IncomingMessage , reply : & R ,) -> impl :: core :: future :: Future < Output = () > \
             + Send where S : $ crate :: UsageService < Ctx > + Sync , Ctx : Sync , \
             R : Reply + Sync"
        ),
        "it returns nothing, and a trait with `async fn` has no `dyn` form to offer. Got: \
         {emitted}"
    );
    assert!(
        !emitted.contains("& dyn"),
        "no `&dyn` form exists to offer, so none is emitted. Got: {emitted}"
    );
}

/// The dispatcher is a stored token sequence, not compiled items: nothing of it is built where the
/// service is declared, and the only `dispatch` in the expansion is the one inside the macro.
#[test]
fn the_dispatcher_is_emitted_inside_the_macro_and_nowhere_at_the_trait_s_own_scope() {
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
    let macro_at = emitted
        .find("macro_rules ! usage_service_amqp_rpc_dispatcher")
        .unwrap();
    assert_eq!(
        emitted.matches("pub fn dispatch").count(),
        1,
        "got: {emitted}"
    );
    assert!(
        emitted.find("pub fn dispatch").unwrap() > macro_at,
        "a dispatcher outside the macro is compiled where the service is declared, which is the \
         whole of what this moves. Got: {emitted}"
    );
    assert!(
        emitted.contains("# [macro_export]"),
        "the macro is reached from another crate, so it is exported. Got: {emitted}"
    );
}

/// The list is read as the service wrote it, duplicates and all, and the emission walks the
/// registry instead — one `#[macro_export]` name cannot be defined twice in one crate.
#[test]
fn a_transport_named_twice_contributes_one_macro() {
    let emitted = exec_service_schema(
        quote! { transports = ["amqp_rpc", "amqp_rpc"] },
        declared(MIXED_SERVICE).to_token_stream(),
    )
    .to_string();
    assert_eq!(
        emitted
            .matches("macro_rules ! usage_service_amqp_rpc_dispatcher")
            .count(),
        1,
        "got: {emitted}"
    );
}

/// A service that asked for no transport is emitted its contract and nothing else.
///
/// Both halves are read here rather than the absences alone: a regression that stopped emitting the
/// contract would pass a test that only asked what is missing, and the contract is what a
/// hand-written dispatcher is written against.
#[test]
fn a_service_asking_for_no_transport_is_emitted_the_contract_and_nothing_else() {
    let emitted = expanded(MIXED_SERVICE);
    for absent in [
        "macro_rules",
        "pub fn dispatch",
        "IncomingMessage",
        "pub trait Reply",
        "pub trait Transport",
        "pub struct UsageServiceClient",
        "serde_json",
        "tracing",
    ] {
        assert!(
            !emitted.contains(absent),
            "`{absent}` belongs to a transport, and this service asked for none. Got: {emitted}"
        );
    }
    for present in [
        "pub trait UsageService < Ctx >",
        "pub struct ExpireCreditRequest",
        "pub struct SweepRequest",
        "pub struct UsageServiceFaultFields",
        "pub enum UsageServiceFaultKind",
        "pub type ServiceFault = UsageServiceFaultFields",
        "pub type ServiceFaultKind = UsageServiceFaultKind",
        "pub fn failed_validation",
        "pub fn handler_panic",
        "pub fn transport_failure",
        "pub fn undeserializable_payload",
        "pub fn unknown_operation",
        "pub trait MessageValidation",
        "fn validate (& self)",
        "pub enum CallError",
    ] {
        assert!(
            emitted.contains(present),
            "`{present}` is contract rather than transport, and a service is no use without it. \
             Got: {emitted}"
        );
    }
    for described in [
        "pub struct ExpireCreditRequest",
        "pub struct SweepRequest",
        "pub struct UsageServiceFaultFields",
        "pub enum UsageServiceFaultKind",
    ] {
        assert!(
            is_described(&emitted, described),
            "`{described}` crosses the wire, so it is described on every surface this build \
             writes. Got: {emitted}"
        );
    }
    // The service's own TypeScript is the one artifact a feature decides the existence of.
    #[cfg(feature = "typescript")]
    assert!(
        emitted.contains("pub fn ts_definition"),
        "a build that writes TypeScript publishes the service's types. Got: {emitted}"
    );
    #[cfg(not(feature = "typescript"))]
    assert!(
        !emitted.contains("pub fn ts_definition"),
        "a build that writes no TypeScript publishes none of it. Got: {emitted}"
    );
    #[cfg(all(feature = "typescript", feature = "zod"))]
    assert!(
        emitted.contains("ExpireCreditRequest :: zod_schema"),
        "and a build with a schema to publish brings each message's along with its type. \
         Got: {emitted}"
    );
}

/// The macro takes no arguments and opens no module: the caller supplies the module, which is what
/// keeps two transports in one crate from colliding.
#[test]
fn the_dispatcher_macro_takes_no_arguments_and_opens_no_module_of_its_own() {
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
    assert!(
        emitted.contains("macro_rules ! usage_service_amqp_rpc_dispatcher { () => {"),
        "the one rule matches an empty invocation. Got: {emitted}"
    );
    let body = macro_body(MIXED_SERVICE, "usage_service_amqp_rpc_dispatcher");
    assert!(
        !body.contains(" mod "),
        "the items are bare, so the caller names the module they land in. Got: {body}"
    );
}

/// Every runtime crate the macro body calls is written with a leading `::`, so it resolves in the
/// crate that invoked the macro — which is the crate that names it in its own manifest, and the
/// whole reason the tokens travel rather than being compiled where the service is declared.
#[test]
fn every_runtime_crate_the_macro_body_calls_is_written_with_a_leading_colon_pair() {
    let body = macro_body(MIXED_SERVICE, "usage_service_amqp_rpc_dispatcher");
    for called in ["serde :: Serialize", "serde_json", "tracing", "core", "std"] {
        assert!(
            every_mention_is_qualified(&body, called, ":: "),
            "`{called}` is reached without a leading `::` somewhere in the macro body, so it \
             resolves against whatever the invoking crate happens to have in scope. Got: {body}"
        );
    }
}

/// Every name the macro body reaches in the declaring crate is written through `$crate`, because a
/// path in a `macro_rules!` body resolves where the macro was *invoked*. An unqualified one
/// compiles only while the caller happens to share the declaring crate's scope.
#[test]
fn every_generated_name_the_macro_body_reaches_is_written_through_crate() {
    let body = macro_body(MIXED_SERVICE, "usage_service_amqp_rpc_dispatcher");
    assert!(
        every_mention_is_qualified(&body, "UsageService", "$ crate :: "),
        "the trait is reached unqualified somewhere in the macro body, which resolves in whichever \
         crate invoked the macro. Got: {body}"
    );
    for reached in [
        "ServiceFault",
        "Answered",
        "named_field",
        "violated_field",
        "violation_detail",
        "GetAvailableBalanceMessage",
        "ExpireCreditMessage",
        "SweepMessage",
        "CanGenerateMessage",
        "ApplyBundleMessage",
        "validated_get_available_balance",
        "validated_apply_bundle",
    ] {
        assert!(
            every_mention_is_qualified(&body, reached, "$ crate :: usage_service_schema :: "),
            "`{reached}` is reached unqualified somewhere in the macro body, which resolves in \
             whichever crate invoked the macro. Got: {body}"
        );
    }
}

/// A transport's macro reaches the trait and the service's own module through `$crate`, which is
/// the declaring crate's *root* however far below it the service was written. The module carries
/// one anchor per name, so the crate that owes the re-exports is the crate that stops compiling
/// without them.
#[test]
fn the_module_anchors_both_root_names_a_transport_macro_reaches() {
    let held = module_body(
        expansion_over_amqp_rpc(MIXED_SERVICE),
        "usage_service_schema",
    )
    .to_string();
    assert!(
        held.contains(
            "const _ : :: core :: marker :: PhantomData < crate :: usage_service_schema :: \
             ServiceFault > = :: core :: marker :: PhantomData ;"
        ),
        "the module is not anchored at the crate root, so a service below the root publishes a \
         macro whose `$crate::{{service}}_schema` resolves nowhere. Got: {held}"
    );
    assert!(
        held.contains("S : crate :: UsageService < Ctx >"),
        "the trait is not anchored at the crate root, so a service below the root publishes a \
         macro whose `$crate::{{Trait}}` resolves nowhere. Got: {held}"
    );
}

/// A service that named no transport publishes no macro, reaches no root and owes none, so the
/// bare-service surface does not grow.
#[test]
fn a_service_that_asked_for_no_transport_is_anchored_at_no_root() {
    let held = module_body(
        exec_service_schema(
            TokenStream::new(),
            declared(MIXED_SERVICE).to_token_stream(),
        ),
        "usage_service_schema",
    )
    .to_string();
    for anchored in [
        "crate :: usage_service_schema",
        "RootAnchor",
        "crate :: UsageService",
    ] {
        assert!(
            !held.contains(anchored),
            "`{anchored}` is anchored for a service that asked for no transport, so a declaration \
             below the crate root is refused a re-export nothing reaches. Got: {held}"
        );
    }
}

/// The anchor stands for the dispatcher's own `where` clause, so it is written the way that clause
/// is: one type argument, whatever the trait declares. Both are read off one expansion, so a trait
/// the dispatcher could never bind is refused at the declaration rather than at every consumer.
#[test]
fn the_trait_anchor_binds_what_the_dispatcher_s_where_clause_binds() {
    let expansion = expansion_over_amqp_rpc(MIXED_SERVICE);
    let dispatched = macro_rules_body(expansion.clone(), "usage_service_amqp_rpc_dispatcher");
    let held = module_body(expansion, "usage_service_schema").to_string();
    assert_eq!(
        bound_on_s(&held, "S : "),
        bound_on_s(&dispatched, "S : ").replace("$ crate", "crate"),
        "the anchor and the dispatcher bind `S` differently, so one of them can pass while the \
         other cannot compile. Anchored in: {held}"
    );
}

/// Both anchors are located on the trait's own ident, so the caret a missing re-export earns sits
/// on the declaration rather than on tokens with no source of their own.
#[test]
fn both_root_anchors_are_spanned_on_the_trait_s_ident() {
    let held = module_body(
        expansion_over_amqp_rpc(MIXED_SERVICE),
        "usage_service_schema",
    );
    for anchored in ["usage_service_schema", "RootAnchor"] {
        let spans = spans_of_idents_named(held.clone(), anchored);
        assert!(
            !spans.is_empty(),
            "`{anchored}` is not in the module at all"
        );
        for span in spans {
            assert_eq!(
                span.source_text().as_deref(),
                Some("UsageService"),
                "`{anchored}` is spanned somewhere other than the trait's ident"
            );
        }
    }
}

#[test]
fn every_arm_is_keyed_on_the_wire_name_and_never_on_anything_in_the_payload() {
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
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
        emitted.contains("match message . operation ()"),
        "the operation is the one the transport read off the wire, read through the accessor \
         rather than off a public field. Got: {emitted}"
    );
}

#[test]
fn an_arm_validates_before_it_calls_and_faults_on_both_ways_the_message_can_be_wrong() {
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
    let deserialized = emitted
        .find(
            "serde_json :: from_slice :: < $ crate :: usage_service_schema :: \
             GetAvailableBalanceMessage >",
        )
        .unwrap();
    let validated = emitted
        .find("$ crate :: usage_service_schema :: validated_get_available_balance (& received)")
        .unwrap();
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
fn every_kind_the_fault_publishes_is_one_the_generated_code_has_a_caller_for() {
    // Read with a transport asked for: `transport_failure` reports a call that never landed, which
    // only the client half is in a position to say.
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
    // Both halves, so the comparison cannot be satisfied by a kind that was quietly dropped: every
    // variant the enum declares, and a constructor call for each of them. A kind with no caller is
    // a shape a TypeScript consumer narrows on and nothing ever produces.
    for (variant, built) in [
        ("FailedValidation", "ServiceFault :: failed_validation"),
        ("HandlerPanic", "ServiceFault :: handler_panic"),
        ("TransportFailure", "ServiceFault :: transport_failure"),
        (
            "UndeserializablePayload",
            "ServiceFault :: undeserializable_payload",
        ),
        ("UnknownOperation", "ServiceFault :: unknown_operation"),
    ] {
        assert!(
            emitted.contains(variant),
            "`{variant}` is declared on the kind and this expansion does not carry it. Got: \
             {emitted}"
        );
        assert!(
            emitted.contains(built),
            "`{variant}` is published as a kind a receiver can be handed, and `{built}` is called \
             from nowhere, so nothing ever produces one. Got: {emitted}"
        );
    }
}

/// A dispatcher is what turns a defect into a fault, and one written by hand — or expanded from a
/// transport's own macro — sits outside the module the fault is declared in.
#[test]
fn every_constructor_the_fault_carries_is_published() {
    let emitted = expanded(MIXED_SERVICE);
    for published in [
        "pub fn failed_validation",
        "pub fn handler_panic",
        "pub fn transport_failure",
        "pub fn undeserializable_payload",
        "pub fn unknown_operation",
    ] {
        assert!(
            emitted.contains(published),
            "`{published}` left bare is a kind of defect nothing outside this module can report. \
             Got: {emitted}"
        );
    }
}

#[test]
fn every_arm_calls_its_implementation_behind_the_panic_guard() {
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
    for method in [
        "get_available_balance",
        "expire_credit",
        "sweep",
        "can_generate",
        "apply_bundle",
    ] {
        assert!(
            emitted.contains(&format!("caught (move || svc . {method} (")),
            "the transport acknowledges after `dispatch` returns, so a panic in `{method}` that \
             unwound past it would leave the delivery unacknowledged on a bus with no `nack`, no \
             dead-letter exchange, no message TTL and no timeout. Got: {emitted}"
        );
    }
    assert!(
        !emitted.contains("Answered :: answering (svc ."),
        "a call that is not behind the guard is one whose panic escapes. Got: {emitted}"
    );
}

#[test]
fn a_request_and_reply_arm_answers_a_caught_panic_with_a_fault_naming_its_own_wire_name() {
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
    assert!(
        emitted.contains(
            "Err (panicked) => { record_panic (\"usage-generation-request\" , & panicked) ; \
             reply . fault ($ crate :: usage_service_schema :: ServiceFault :: handler_panic \
             (\"usage-generation-request\" , & panicked)) . await }"
        ),
        "the arm that answered to the name is the one that reports the defect, and it reports it \
         under the wire name rather than under the Rust ident. Got: {emitted}"
    );
}

#[test]
fn every_arm_writes_a_caught_panic_down_before_it_answers() {
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
    assert!(
        emitted.contains(
            "if let Err (panicked) = caught (move || svc . apply_bundle (ctx , received)) . await \
             { record_panic (\"apply-bundle\" , & panicked) ; }"
        ),
        "a one-way arm answers nobody, so the record is the whole account of the panic there is. \
         Got: {emitted}"
    );
    let recorded = emitted
        .find("record_panic (\"usage-generation-request\" , & panicked)")
        .unwrap();
    let answered = emitted
        .find("ServiceFault :: handler_panic (\"usage-generation-request\" , & panicked)")
        .unwrap();
    assert!(
        recorded < answered,
        "the operator's record is written before the caller is answered, so a `Reply` that comes \
         apart in turn cannot take the account of the first panic with it. Got: {emitted}"
    );
}

#[test]
fn the_dispatcher_macro_names_tracing_and_a_declared_type_does_not() {
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
    assert!(
        emitted.contains(":: tracing :: error !"),
        "the crate that invokes a transport's dispatcher macro names `tracing` in its own \
         manifest, beside `serde_json`, because the dispatcher calls it. Got: {emitted}"
    );
    let described = exec_model_schema(
        TokenStream::new(),
        quote! {
            pub struct AvailableBalanceRequest {
                pub organization_id: String,
            }
        },
    )
    .to_string();
    assert!(
        !described.contains("tracing"),
        "only a declared service emits a dispatcher, so describing a type reaches no logger and \
         adds nothing to a crate's manifest. Got: {described}"
    );
}

#[test]
fn a_refusal_and_a_violation_are_read_for_a_field_name_by_the_same_reader() {
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
    assert!(
        emitted.contains("fn named_field (reported : & str)"),
        "got: {emitted}"
    );
    assert!(
        emitted.contains(
            "fn violated_field (reported : & [String]) -> Option < & str > { \
                          named_field (reported . first () ?) }"
        ),
        "a violation report's field is the first line's, read by the one reader. Got: {emitted}"
    );
    assert!(
        emitted.contains(
            "let named = $ crate :: usage_service_schema :: named_field (said) . or_else \
             (|| serde_named_field (said))"
        ),
        "a serde refusal is read for a field by the reader a violation is read by — a hook hands \
         serde a validator's message verbatim — and, failing that, by serde's own naming. \
         Got: {emitted}"
    );
    assert!(
        emitted
            .contains("if matches ! (refusal . classify () , :: serde_json :: error :: Category"),
        "which fault a refusal is comes off serde_json's own classification of it rather than off \
         the shape of the sentence it wrote. Got: {emitted}"
    );
}

#[test]
fn a_one_way_arm_calls_the_implementation_and_then_touches_the_handle_with_nothing() {
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
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
        "nothing about replying belongs on a path that never replies, a caught panic included: \
         the operation declared no reply and the delivery carries no queue for one to go to. \
         Acknowledgement is the transport adapter's, after `dispatch` returns. Got: {emitted}"
    );
}

#[test]
fn the_client_carries_one_method_per_operation_under_the_operation_s_own_wire_name() {
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
    assert!(
        emitted.contains("pub struct UsageServiceClient < T : Transport >"),
        "got: {emitted}"
    );
    for named in [
        "pub fn get_available_balance (& self , req : AvailableBalanceRequest)",
        "pub fn expire_credit (& self , organization_id : OrganizationId , credit_id : CreditId)",
        "pub fn sweep (& self)",
        "pub fn can_generate (& self , req : GenerationRequest)",
        "pub fn apply_bundle (& self , req : ApplyBundleRequest)",
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
fn a_client_method_takes_the_message_and_no_context() {
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
    let client = published_macro(MIXED_SERVICE, "usage_service_amqp_rpc_client");
    assert!(
        !client.contains("Ctx") && !client.contains("_ctx"),
        "the context is what an implementation needs and a caller has nothing to hand one to, so \
         no part of the client names it. Got: {client}"
    );
    // Every parameter list in full, so an argument creeping back in on either side of the message
    // fails here rather than slipping past a `contains` on the message type alone.
    for taken in [
        "pub fn get_available_balance (& self , req : AvailableBalanceRequest)",
        "pub fn expire_credit (& self , organization_id : OrganizationId , credit_id : CreditId)",
        "pub fn sweep (& self)",
        "pub fn can_generate (& self , req : GenerationRequest)",
        "pub fn apply_bundle (& self , req : ApplyBundleRequest)",
    ] {
        assert!(
            client.contains(taken),
            "a client method takes what the operation takes after its context, and nothing else. \
             Got: {client}"
        );
    }
    assert!(
        emitted.contains(
            "fn get_available_balance (& self , ctx : & Ctx , req : AvailableBalanceRequest ,)"
        ),
        "the trait keeps its context: it is the half that has an implementation to hand one to. \
         Got: {emitted}"
    );
}

#[test]
fn a_client_method_validates_before_it_reaches_the_transport() {
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
    let validated = emitted
        .find("$ crate :: usage_service_schema :: validated_get_available_balance (& sending)")
        .unwrap();
    let sent = emitted.find("self . transport .").unwrap();
    assert!(
        validated < sent,
        "a message the client refuses never becomes a remote error a round trip later. Got: \
         {emitted}"
    );
    assert!(
        emitted.contains(
            "return Err ($ crate :: usage_service_schema :: CallError :: Fault ($ crate :: \
             usage_service_schema :: ServiceFault :: failed_validation ("
        ),
        "the operation never ran, so it is not one of its declared errors. Got: {emitted}"
    );
}

#[test]
fn the_transport_seam_gives_a_call_that_never_landed_somewhere_to_be_reported() {
    let client = published_macro(MIXED_SERVICE, "usage_service_amqp_rpc_client");
    for answered in [
        "Output = Result < () , String >",
        "Output = Result < Vec < u8 > , String >",
    ] {
        assert!(
            client.contains(answered),
            "a transport that hit its own deadline can only panic or hang without a failure arm \
             to answer in, and the caller is left holding a call that never completes. Got: \
             {client}"
        );
    }
    assert!(
        !client.contains("Output = Vec < u8 > "),
        "the reply position is the failure arm's `Ok`, not the whole answer. Got: {client}"
    );
    // Both directions, so a seam that grew the arm and a client that ignored it fails here. Each
    // method names its own wire name, which is what tells the five apart from the mirror's own
    // reading of a fault that arrived carrying the kind.
    assert_eq!(
        client
            .matches("ServiceFault :: transport_failure (\"")
            .count(),
        5,
        "every method turns what the transport reported into a fault, one-way operations \
         included: a send that did not go out is still something the caller is owed. Got: {client}"
    );
    assert!(
        client.contains(
            "$ crate :: usage_service_schema :: CallError :: Fault ($ crate :: \
             usage_service_schema :: ServiceFault :: transport_failure"
        ),
        "a replying operation carries it in the arm a caller already matches defects on, rather \
         than in a third one. Got: {client}"
    );
}

#[test]
fn a_transport_failure_is_a_kind_the_fault_publishes_and_the_mirror_reads_back() {
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
    assert!(
        emitted.contains(
            "FaultKindOnTheWire :: TransportFailure => { $ crate :: usage_service_schema :: \
             ServiceFault :: transport_failure"
        ),
        "the mirror is what reads a fault back off the wire, so a kind it does not spell is a \
         fault that arrives and will not deserialize. Got: {emitted}"
    );
    assert!(
        emitted.contains("Self :: TransportFailure => \"transport failure\""),
        "a fault is meant to page a human, so every kind renders to a line. Got: {emitted}"
    );
}

#[test]
fn a_fault_is_read_back_through_a_private_mirror_rather_than_by_widening_the_fault() {
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
    assert!(
        emitted.contains("struct FaultOnTheWire") && !emitted.contains("pub struct FaultOnTheWire"),
        "the mirror is the seam, and it is private to wherever the client was placed. Got: \
         {emitted}"
    );
    assert!(
        emitted.contains("fn into_fault (self) -> $ crate :: usage_service_schema :: ServiceFault"),
        "got: {emitted}"
    );
    for minted in [
        "ServiceFault :: failed_validation (& self . operation",
        "ServiceFault :: handler_panic (& self . operation",
        "ServiceFault :: transport_failure (& self . operation",
        "ServiceFault :: undeserializable_payload (& self . operation",
        "ServiceFault :: unknown_operation (& self . operation)",
    ] {
        assert!(
            emitted.contains(minted),
            "the fault's fields are private to the module it is declared in and the mirror is \
             not in it, so every kind is minted through the constructor that module publishes. \
             Got: {emitted}"
        );
    }
    // Declared under the name its TypeScript is published as, `ServiceFault` being the alias the
    // module's own generated code writes.
    let fault = emitted.find("pub struct UsageServiceFaultFields").unwrap();
    let derives = &emitted[fault.saturating_sub(200)..fault];
    assert!(
        !derives.contains("Deserialize"),
        "a public `Deserialize` on the fault is a public constructor by another name. Got: \
         {derives}"
    );
    assert!(
        emitted.contains("pub type ServiceFault = UsageServiceFaultFields ;"),
        "the module keeps the unstuttering spelling; only TypeScript needs the prefix. Got: \
         {emitted}"
    );
}

/// The ident the fault is declared under, which is also the name it publishes to TypeScript.
///
/// It carries `Fields` because in TypeScript `UsageServiceFault` is taken by the sealed type
/// written over these members — the same members plus a brand a hand-written object cannot spell.
/// Rust needs no such pair, the fields here being private whatever the constructors publish, so
/// the one declaration answers to both names.
#[test]
fn the_fault_is_declared_under_the_name_its_fields_publish_as() {
    let emitted = expanded(MIXED_SERVICE);
    assert!(
        emitted.contains("pub struct UsageServiceFaultFields"),
        "the sealed TypeScript type takes `UsageServiceFault`, so the fields publish beside it. \
         Got: {emitted}"
    );
    assert!(
        !emitted.contains("pub struct UsageServiceFault {")
            && !emitted.contains("pub struct UsageServiceFault ("),
        "two declarations under one flat name is what a bundle cannot compile. Got: {emitted}"
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

/// The name is the service snake-cased, the transport, and `client`, so `UsageService` over
/// `amqp_rpc` publishes `usage_service_amqp_rpc_client`.
#[test]
fn a_service_asking_for_a_transport_publishes_its_client_as_a_macro_and_not_as_a_type() {
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
    let at = emitted
        .find("# [macro_export] macro_rules ! usage_service_amqp_rpc_client { () => {")
        .unwrap();
    for inside in ["pub trait Transport", "pub struct UsageServiceClient < T"] {
        assert!(
            !emitted[..at].contains(inside),
            "the two halves of a service usually live in different crates, so `{inside}` is \
             emitted inert and placed by whoever wants it. Got: {emitted}"
        );
    }
}

#[test]
fn a_transport_named_twice_publishes_its_client_once() {
    let emitted = exec_service_schema(
        quote! { transports = ["amqp_rpc", "amqp_rpc"] },
        declared(MIXED_SERVICE).to_token_stream(),
    )
    .to_string();
    assert_eq!(
        emitted
            .matches("macro_rules ! usage_service_amqp_rpc_client")
            .count(),
        1,
        "`#[macro_export]` puts the name at the declaring crate's root, where one name can be \
         defined once. Got: {emitted}"
    );
}

#[test]
fn the_client_macro_takes_no_arguments_and_wraps_its_items_in_no_module() {
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
    assert!(
        emitted.contains("macro_rules ! usage_service_amqp_rpc_client { () => {"),
        "one rule, matching an empty invocation. Got: {emitted}"
    );
    let body = macro_body(MIXED_SERVICE, "usage_service_amqp_rpc_client");
    assert!(
        !body.contains(" mod "),
        "the items are bare, so the module they land in is the invoking crate's to name. Got: \
         {body}"
    );
}

/// Every name the declaring crate generated is reached through `$crate`, because the body is
/// expanded in whatever module of whatever crate wanted a client. A bare one would resolve there,
/// and resolve to nothing.
#[test]
fn every_generated_name_the_client_writes_is_reached_through_the_declaring_crate() {
    let code = macro_body(MIXED_SERVICE, "usage_service_amqp_rpc_client");
    assert_eq!(
        code.matches("usage_service_schema").count(),
        code.matches("$ crate :: usage_service_schema").count(),
        "the service's own module is the declaring crate's. Got: {code}"
    );
    for named in ["CallError", "ServiceFault"] {
        assert!(code.contains(named), "got: {code}");
        assert_eq!(
            code.matches(named).count(),
            code.matches(&format!("usage_service_schema :: {named}"))
                .count(),
            "`{named}` is declared inside the service's module and reached nowhere else. Got: \
             {code}"
        );
    }
    for message in ["ExpireCreditRequest", "SweepRequest"] {
        assert!(code.contains(message), "got: {code}");
        assert_eq!(
            code.matches(message).count(),
            code.matches(&format!("$ crate :: {message}")).count(),
            "a message the macro declared sits beside the trait, in the declaring crate. Got: \
             {code}"
        );
    }
    assert!(
        code.contains("req : AvailableBalanceRequest"),
        "a type the *author* wrote is spelled as they wrote it: no one prefix is true of both \
         `AvailableBalanceRequest` and `String`, so the module the macro is invoked in supplies \
         it, exactly as the generated module used to supply it. Got: {code}"
    );
}

/// Every runtime crate carries a leading `::`, because it resolves in the invoking crate and has
/// to be named in that crate's manifest. `tracing` is not among them: nothing here catches a
/// panic, so nothing here has anything to write down.
#[test]
fn every_runtime_crate_the_client_reaches_is_written_from_the_root_and_tracing_is_not_one() {
    let code = macro_body(MIXED_SERVICE, "usage_service_amqp_rpc_client");
    for reached in [
        ":: core :: future :: Future",
        ":: serde :: Serialize",
        ":: serde :: de :: DeserializeOwned",
        ":: serde_json :: from_slice",
    ] {
        assert!(code.contains(reached), "got: {code}");
    }
    for crate_root in ["core :: ", "serde :: ", "serde_json :: "] {
        assert_eq!(
            code.matches(crate_root).count(),
            code.matches(&format!(":: {crate_root}")).count(),
            "`{crate_root}` resolves in the invoking crate, which is not the one that declared \
             the service. Got: {code}"
        );
    }
    assert!(
        !code.contains("tracing"),
        "a caller that only wants to make calls names one crate fewer than a crate that answers \
         them: catching a panic is the dispatcher's, and so is writing one down. Got: {code}"
    );
}

/// Which of the two answers a message's check gives depends on the message's *concrete* type — an
/// inherent `validate()` beats the fallback trait's — so the check is a function in the module that
/// declared the message, and both halves call that one function rather than a copy each.
#[test]
fn both_halves_ask_one_operation_s_check_rather_than_a_copy_each() {
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
    for published in [
        "pub fn validated_get_available_balance (received : & GetAvailableBalanceMessage)",
        "pub fn validated_expire_credit (received : & ExpireCreditMessage)",
        "pub fn validated_sweep (received : & SweepMessage)",
        "pub fn validated_can_generate (received : & CanGenerateMessage)",
        "pub fn validated_apply_bundle (received : & ApplyBundleMessage)",
    ] {
        assert!(emitted.contains(published), "got: {emitted}");
    }
    for half in [
        "usage_service_amqp_rpc_dispatcher",
        "usage_service_amqp_rpc_client",
    ] {
        let body = published_macro(MIXED_SERVICE, half);
        assert!(
            body.contains("$ crate :: usage_service_schema :: validated_get_available_balance ("),
            "`{half}` asks the question rather than answering it, the answer depending on a type \
             it reaches only through the declaring crate. Got: {body}"
        );
        assert!(
            !body.contains("message_validation") && !body.contains(". validate ()"),
            "a copy of the check inside `{half}` answers at whatever type is in scope where the \
             macro was placed. Got: {body}"
        );
    }
}

/// The struct is generated, so a consumer publishing the module it lands in cannot answer for its
/// shape: a struct whose every field is public earns them `clippy::exhaustive_structs`, and the
/// only fix from where they stand is an `#[allow]` over an attribute they did not write. The
/// constructor is what keeps a transport adapter in another crate able to build one, which
/// `#[non_exhaustive]` would have taken away from exactly the crate that needs it.
#[test]
fn the_incoming_message_publishes_a_constructor_and_two_readers_rather_than_its_fields() {
    let body = published_macro(MIXED_SERVICE, "usage_service_amqp_rpc_dispatcher");
    assert!(
        body.contains("pub struct IncomingMessage { operation : String , payload : Vec < u8 > , }"),
        "neither field is published, so nothing outside reads or writes one directly. Got: {body}"
    );
    for published in [
        "pub const fn new (operation : String , payload : Vec < u8 >) -> Self",
        "pub fn operation (& self) -> & str",
        "pub fn payload (& self) -> & [u8]",
    ] {
        assert!(
            body.contains(published),
            "`{published}` is missing. Got: {body}"
        );
        assert!(
            is_documented(&body, published),
            "`{published}` is what a consumer builds and reads one with, and it is published \
             undocumented. Got: {body}"
        );
    }
}

/// The two readers are what the dispatcher itself goes through, which is what keeps the fields
/// private rather than merely spelled private.
#[test]
fn the_dispatcher_reads_an_incoming_message_through_its_accessors() {
    let body = published_macro(MIXED_SERVICE, "usage_service_amqp_rpc_dispatcher");
    assert!(
        body.contains("match message . operation ()"),
        "the operation is matched through the reader. Got: {body}"
    );
    assert!(
        body.contains(
            "from_slice :: < $ crate :: usage_service_schema :: GetAvailableBalanceMessage > \
             (message . payload () ,)"
        ),
        "the payload is handed to serde through the reader. Got: {body}"
    );
    for (reached, through) in [
        ("message . operation", "message . operation ()"),
        ("message . payload", "message . payload ()"),
    ] {
        assert_eq!(
            body.matches(reached).count(),
            body.matches(through).count(),
            "`{reached}` is reached somewhere as a field rather than through the reader. \
             Got: {body}"
        );
    }
}

/// `clippy::missing_errors_doc` reaches a `pub fn` - free or inherent - answering `Result<…>` or
/// `impl Future<Output = Result<…>>`, and a consumer cannot write the section: the doc comment is
/// generated. Walked rather than listed, so a method added later is covered without touching this.
#[test]
fn every_published_function_answering_a_result_says_under_errors_what_the_failure_arm_holds() {
    let mut reached = 0_usize;
    for half in [
        "usage_service_amqp_rpc_dispatcher",
        "usage_service_amqp_rpc_client",
    ] {
        for (docs, head) in published_functions(macro_rules_items(MIXED_SERVICE, half)) {
            if !head.contains("-> Result <") && !head.contains("Output = Result <") {
                continue;
            }
            reached += 1;
            assert!(
                docs.contains("# Errors"),
                "`{head}` answers a `Result` and says nothing under an `# Errors` heading about \
                 what its failure arm holds. Got: {docs}"
            );
        }
    }
    assert!(
        reached >= 5,
        "the client publishes one method per operation and every one of them answers a `Result`, \
         so a walk reaching {reached} of them is not seeing them"
    );
}

/// A fault and an answer both arrive in a reply. A service that declares none has no reply to read
/// either out of, so emitting the mirror and the reader anyway leaves seven items dead in whatever
/// module the consumer placed - and `dead_code` is an error in plenty of consumers' builds.
#[test]
fn the_fault_mirror_and_the_answer_reader_are_emitted_only_where_an_operation_answers() {
    let reading = [
        "struct FaultOnTheWire",
        "enum FaultKindOnTheWire",
        "struct TaggedFault",
        "enum ReportedError",
        "fn into_fault",
        "fn reported",
        "fn read_answer",
    ];
    let answering = published_macro(MIXED_SERVICE, "usage_service_amqp_rpc_client");
    for emitted in reading {
        assert!(
            answering.contains(emitted),
            "`{emitted}` reads a reply back, and this service declares operations that reply. \
             Got: {answering}"
        );
    }
    let one_way = published_macro(ONE_WAY_SERVICE, "note_service_amqp_rpc_client");
    for absent in reading {
        assert!(
            !one_way.contains(absent),
            "`{absent}` is reached only from an operation that answers, and this service declares \
             none. Got: {one_way}"
        );
    }
    for present in [
        "pub trait Transport",
        "pub struct NoteServiceClient",
        "pub fn apply_bundle",
        "pub fn note",
    ] {
        assert!(
            one_way.contains(present),
            "`{present}` is what a caller of a one-way service reaches. Got: {one_way}"
        );
    }
}

/// A service with no operation has no arm, so the guard an arm calls its implementation behind and
/// the reader that classifies an arm's own deserialization refusal are reached from nowhere - and
/// neither is the implementation nor the context `dispatch` would hand one.
#[test]
fn a_service_declaring_no_operation_is_emitted_no_item_nothing_reaches() {
    let dispatcher = published_macro(BARE_SERVICE, "bare_service_amqp_rpc_dispatcher");
    for absent in [
        "fn caught",
        "fn panic_detail",
        "fn record_panic",
        "fn refused_payload",
        "fn serde_named_field",
        "tracing",
    ] {
        assert!(
            !dispatcher.contains(absent),
            "`{absent}` is an arm's, and this service declares no arm. Got: {dispatcher}"
        );
    }
    assert!(
        dispatcher.contains(
            "pub fn dispatch < S , Ctx , R > (_ : & S , _ : & Ctx , message : & IncomingMessage , \
             reply : & R ,)"
        ),
        "the fallback arm reads the operation name and answers through the handle, so neither the \
         implementation nor the context is bound. Got: {dispatcher}"
    );
    for present in [
        "pub struct IncomingMessage",
        "pub trait Reply",
        "unknown_operation",
    ] {
        assert!(
            dispatcher.contains(present),
            "`{present}` is what a delivery naming nothing is still settled through. Got: \
             {dispatcher}"
        );
    }
    let client = published_macro(BARE_SERVICE, "bare_service_amqp_rpc_client");
    for absent in [
        "FaultOnTheWire",
        "TaggedFault",
        "ReportedError",
        "read_answer",
    ] {
        assert!(
            !client.contains(absent),
            "`{absent}` reads a reply, and this service declares no operation to have one. Got: \
             {client}"
        );
    }
    assert!(
        client.contains("pub trait Transport") && client.contains("pub struct BareServiceClient"),
        "the seam and the client itself are what a service says nothing about. Got: {client}"
    );
}

/// Clippy's default grouping puts every type ahead of every function, so a function emitted above a
/// type is a diagnostic in every strict consumer's build at once. Within a group nothing is
/// ordered, but the three-way grouping costs nothing to hold and is asserted whole.
#[test]
fn each_macro_body_emits_its_types_then_its_impls_then_its_functions() {
    for (source, half) in [
        (MIXED_SERVICE, "usage_service_amqp_rpc_dispatcher"),
        (MIXED_SERVICE, "usage_service_amqp_rpc_client"),
        (ONE_WAY_SERVICE, "note_service_amqp_rpc_dispatcher"),
        (ONE_WAY_SERVICE, "note_service_amqp_rpc_client"),
        (BARE_SERVICE, "bare_service_amqp_rpc_dispatcher"),
        (BARE_SERVICE, "bare_service_amqp_rpc_client"),
    ] {
        let emitted = emitted_items(macro_rules_items(source, half));
        assert!(!emitted.is_empty(), "`{half}` emitted nothing to group");
        let grouped: Vec<(usize, &str)> = emitted
            .iter()
            .map(|item| {
                let rank = match item.keyword.as_str() {
                    "enum" | "struct" | "trait" | "type" | "union" => 0_usize,
                    "impl" => 1,
                    "fn" => 2,
                    // `opens_an_item` admits nothing else, so a rank of three is an item kind the
                    // walk learned to see and this test never learned to place.
                    _ => 3,
                };
                (rank, item.head.as_str())
            })
            .collect();
        assert!(
            grouped.iter().all(|(rank, _)| *rank < 3),
            "`{half}` emits an item kind nothing here groups: {grouped:?}"
        );
        assert!(
            grouped.iter().map(|(rank, _)| *rank).is_sorted(),
            "`{half}` emits its items out of the grouping clippy asks for: {grouped:?}"
        );
    }
}

/// An `#[allow]` written into a consumer's expansion silences a check they chose, in their build,
/// with no line of their source to explain it. `#[doc(hidden)]` reached for to the same end mutes
/// the one lint that exists to make a fallible method documented, while hiding it from every
/// consumer who publishes it. Neither is emitted, and neither is anything else that quiets a lint.
#[test]
fn neither_macro_body_carries_an_attribute_that_quiets_a_lint() {
    for (source, half) in [
        (MIXED_SERVICE, "usage_service_amqp_rpc_dispatcher"),
        (MIXED_SERVICE, "usage_service_amqp_rpc_client"),
        (ONE_WAY_SERVICE, "note_service_amqp_rpc_dispatcher"),
        (ONE_WAY_SERVICE, "note_service_amqp_rpc_client"),
        (BARE_SERVICE, "bare_service_amqp_rpc_dispatcher"),
        (BARE_SERVICE, "bare_service_amqp_rpc_client"),
    ] {
        // Literals blanked, so a doc comment saying the word `allow` is not read as an attribute.
        let body = macro_body(source, half);
        for quieting in ["allow", "expect", "doc (hidden)", "automatically_derived"] {
            assert!(
                !body.contains(quieting),
                "`{half}` writes `{quieting}` into a consumer's build, where nothing in their own \
                 source explains it. Got: {body}"
            );
        }
    }
}

/// The placement a consumer chooses decides three of the lints they see, and nothing but the
/// documentation tells them which one measures clean. Both macros carry it, and both quote the
/// refusal a path earns verbatim so a consumer who hits it recognises what they are reading.
#[test]
fn both_macro_docs_prescribe_a_file_placement_and_quote_what_a_path_is_refused_with() {
    let emitted = expanded_over_amqp_rpc(MIXED_SERVICE);
    for placed in [
        "// src/amqp_transport.rs\\nthe_contract_crate::usage_service_amqp_rpc_dispatcher!();",
        "// src/amqp_client.rs\\nuse the_contract_crate::{AvailableBalanceRequest, \
         AvailableBalanceResponse};\\n\\nthe_contract_crate::usage_service_amqp_rpc_client!();",
    ] {
        assert!(
            emitted.contains(placed),
            "the example placement is a module of its own file, with the author's types named one \
             by one. Got: {emitted}"
        );
    }
    for quoted in [
        "error: macro-expanded `macro_export` macros from the current crate cannot be referred to \
         by absolute paths",
        "`#[deny(macro_expanded_macro_exports_accessed_by_absolute_paths)]` (part of \
         `#[deny(future_incompatible)]`) on by default",
    ] {
        assert_eq!(
            emitted.matches(quoted).count(),
            2,
            "both macros quote what `crate::…!()` and `use crate::…;` are refused with, since \
             either is what a declaring crate reaches for first. Got: {emitted}"
        );
    }
    assert_eq!(
        emitted.matches("# Where to put it").count(),
        2,
        "each macro carries the placement, a consumer placing one half having no reason to read \
         the other's documentation. Got: {emitted}"
    );
}

/// The module header is what a reader of this crate sees, and it prescribed a placement that
/// measures dirty until this changed: an inline module and a glob import, one guaranteed
/// `clippy::inline_modules` and one guaranteed `clippy::wildcard_imports`.
#[test]
fn the_transport_module_header_prescribes_the_same_placement_the_macros_do() {
    let header = include_str!("transport/amqp_rpc.rs");
    for prescribed in [
        "//! // src/amqp_transport.rs",
        "//! // src/amqp_client.rs",
        "//! use declaring_crate::{AvailableBalanceRequest, AvailableBalanceResponse};",
        "//! error: macro-expanded `macro_export` macros from the current crate cannot be \
         referred to by absolute paths",
    ] {
        assert!(
            header.contains(prescribed),
            "the module header no longer says `{prescribed}`"
        );
    }
    for shown in [
        "//!     declaring_crate::usage_service",
        "//!     use declaring_crate::*;",
    ] {
        assert!(
            !header.contains(shown),
            "`{shown}` is the inside of an inline module, which is the placement that measures \
             dirty"
        );
    }
}
