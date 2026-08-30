//! The shape a `#[service_schema]` trait is read into, and the only place it is read.
//!
//! # The contract
//!
//! `parse_service` turns a declared trait into a [`ServiceDef`]. Every emitter downstream —
//! messages, supporting types, the dispatcher, the Rust client, the TypeScript artifacts — reads
//! that value and re-reads the trait for nothing. Two consequences follow and both are the point:
//! a rule about what a service may say is written once, here, and the emitters can be built in
//! parallel without touching each other's files.
//!
//! What the representation answers, per operation:
//!
//! - **What it is called.** Three spellings of one declaration, all derived: the Rust ident as
//!   written, [`OperationDef::ts_name`] camelCased for TypeScript callers, and
//!   [`OperationDef::wire_name`] kebab-cased for the wire. Only the wire name is overridable, with
//!   `#[service_schema_op(message = "...")]`, because services already ship names nobody would
//!   derive.
//! - **What it receives.** One message, always. [`OperationInputs`] records which of the three
//!   ways it was declared: the argument that already is the message, the argument list a message
//!   is declared from, or nothing, which gets an empty message.
//! - **What it answers with.** [`OperationOutcome::Reply`] carries the two declared arms, or
//!   [`OperationOutcome::OneWay`] says there is no reply to carry.
//!
//! The context is on [`ServiceDef`], not on any operation: every operation takes it, it is the
//! same type for all of them, and it reaches no message and no schema.
//!
//! # What is deliberately not here
//!
//! The **name of a message the macro declares** is not a field. `<Operation>Request` is the
//! convention, and the task that first emits one derives it here so the messages, the dispatcher
//! and the client cannot disagree about what the type is called.
//!
//! **Nothing about how a message is annotated** is here either. Every ident and every type below
//! is the author's own, carried verbatim, so an emitter is free to write whatever derives and
//! serde attributes a generated message needs onto them.

use crate::rename_rule::RenameRule;
use proc_macro2::TokenTree;
use quote::ToTokens as _;
use std::collections::HashMap;
use syn::spanned::Spanned as _;
use syn::{
    Attribute, FnArg, GenericArgument, GenericParam, Ident, ItemTrait, Pat, PathArguments,
    ReturnType, TraitItem, TraitItemFn, Type,
};

/// The per-operation directive, read and then stripped before the trait is emitted.
pub const OPERATION_DIRECTIVE: &str = "service_schema_op";

const UNKNOWN_DIRECTIVE_MESSAGE: &str = concat!(
    "service_schema: unknown `service_schema_op` directive\n",
    "       the directives are `message = \"<wire name>\"` and `one_way`"
);

/// One service, read once.
pub struct ServiceDef {
    /// The trait's type parameter, which every operation takes and no message carries.
    pub context_param: Ident,
    /// The trait as declared: `UsageService`.
    pub ident: Ident,
    pub operations: Vec<OperationDef>,
}

/// One operation: a name in three spellings, a message in, and either a reply or nothing.
pub struct OperationDef {
    /// The trait method as declared: `get_available_balance`.
    pub ident: Ident,
    pub inputs: OperationInputs,
    pub outcome: OperationOutcome,
    /// How a TypeScript caller spells it: `getAvailableBalance`.
    pub ts_name: String,
    /// What the wire carries: `get-available-balance`, or the `message = "..."` override.
    pub wire_name: String,
}

/// How the incoming message was declared. Which one it is decides who declares the message, never
/// whether there is one — every operation receives exactly one.
pub enum OperationInputs {
    /// No arguments after the context. An empty message is declared for it, so an operation that
    /// later gains a field does not change from carrying no payload to carrying one.
    Empty,
    /// More than one argument after the context, in declaration order. The message is declared
    /// from the list and each argument's name becomes a field on it.
    Generated(Vec<(Ident, Type)>),
    /// Exactly one argument after the context. That argument's type already is the message and
    /// nothing is declared for it. Boxed only because a bare `syn::Type` is 688 bytes and would
    /// make every `Empty` cost the same; `quote!` interpolates through the box unchanged.
    Named(Box<Type>),
}

/// What the operation answers with.
pub enum OperationOutcome {
    /// Marked `#[service_schema_op(one_way)]`: no reply, and therefore no error arm either. An
    /// operation that has to report failure is a request-and-reply operation declared wrong.
    OneWay,
    /// The two arms of the declared `Result<Success, Error>`, separately rendered on every
    /// surface. Boxed for the same reason [`OperationInputs::Named`] is.
    Reply {
        error: Box<Type>,
        success: Box<Type>,
    },
}

/// What one `#[service_schema_op(...)]` said, before anything is derived from it.
struct OperationDirective {
    message: Option<String>,
    one_way: bool,
}

impl OperationDirective {
    fn read(attrs: &[Attribute]) -> Result<Self, syn::Error> {
        let mut directive = Self {
            message: None,
            one_way: false,
        };
        for attribute in attrs
            .iter()
            .filter(|carried| carried.path().is_ident(OPERATION_DIRECTIVE))
        {
            attribute.parse_nested_meta(|meta| {
                if meta.path.is_ident("one_way") {
                    directive.one_way = true;
                    return Ok(());
                }
                if meta.path.is_ident("message") {
                    directive.message = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                    return Ok(());
                }
                Err(meta.error(UNKNOWN_DIRECTIVE_MESSAGE))
            })?;
        }
        Ok(directive)
    }
}

/// Reads and validates a declared trait into the representation every emitter consumes.
///
/// Refusals are accumulated rather than reported one per build, so an author fixing a service sees
/// everything wrong with it at once.
pub fn parse_service(declared: &ItemTrait) -> Result<ServiceDef, syn::Error> {
    let context_param = context_parameter(declared)?;
    let mut operations = Vec::new();
    let mut refusals: Option<syn::Error> = None;
    for member in &declared.items {
        let TraitItem::Fn(operation) = member else {
            continue;
        };
        match parse_operation(operation, &context_param) {
            Ok(parsed) => operations.push(parsed),
            Err(refusal) => refusals = Some(combined(refusals.take(), refusal)),
        }
    }
    let service = ServiceDef {
        context_param,
        ident: declared.ident.clone(),
        operations,
    };
    if let Some(across) = service_refusals(&service) {
        refusals = Some(combined(refusals.take(), across));
    }
    refusals.map_or(Ok(service), Err)
}

fn combined(collected: Option<syn::Error>, refusal: syn::Error) -> syn::Error {
    match collected {
        Some(mut existing) => {
            existing.combine(refusal);
            existing
        }
        None => refusal,
    }
}

/// The context is the trait's first type parameter. A trait without one has nothing to hand an
/// implementation that is not also on the wire.
fn context_parameter(declared: &ItemTrait) -> Result<Ident, syn::Error> {
    declared
        .generics
        .params
        .iter()
        .find_map(|parameter| match parameter {
            GenericParam::Type(named) => Some(named.ident.clone()),
            GenericParam::Const(_) | GenericParam::Lifetime(_) => None,
        })
        .ok_or_else(|| {
            syn::Error::new(
                declared.ident.span(),
                missing_context_parameter_message(&declared.ident),
            )
        })
}

fn is_context_argument(declared: &Type, context: &Ident) -> bool {
    let Type::Reference(borrowed) = declared else {
        return false;
    };
    let Type::Path(named) = borrowed.elem.as_ref() else {
        return false;
    };
    named.qself.is_none() && named.path.is_ident(context)
}

fn missing_context_argument_message(operation: &Ident, context: &Ident) -> String {
    format!(
        "service_schema: operation `{operation}` does not take the context\n       \
         every operation takes `ctx: &{context}` as its first argument after `&self`"
    )
}

fn missing_context_parameter_message(service: &Ident) -> String {
    format!(
        "service_schema: trait `{service}` declares no context type parameter\n       \
         give it one, as in `trait {service}<Ctx>`, and take it in every operation"
    )
}

fn missing_receiver_message(operation: &Ident) -> String {
    format!(
        "service_schema: operation `{operation}` does not take `&self`\n       \
         an operation is called on the service value, so `&self` comes first"
    )
}

fn missing_return_type_message(operation: &Ident) -> String {
    format!(
        "service_schema: operation `{operation}` has no return type\n       \
         add `#[service_schema_op(one_way)]` if it expects no reply,\n       \
         or give it a `Result<Success, Error>` return"
    )
}

fn non_result_return_message(operation: &Ident) -> String {
    format!(
        "service_schema: operation `{operation}` must return `Result<Success, Error>`\n       \
         an operation declares its success type and its error type in one signature"
    )
}

fn one_way_returns_a_value_message(operation: &Ident) -> String {
    format!(
        "service_schema: operation `{operation}` is marked `one_way` but returns a value\n       \
         a one-way operation produces no reply"
    )
}

/// Everything the operation takes after `&self` and the context, in declaration order.
fn operation_inputs(
    operation: &TraitItemFn,
    context: &Ident,
) -> Result<OperationInputs, syn::Error> {
    let named = &operation.sig.ident;
    let mut positional = operation.sig.inputs.iter();
    let Some(FnArg::Receiver(_)) = positional.next() else {
        return Err(syn::Error::new(
            operation.sig.span(),
            missing_receiver_message(named),
        ));
    };
    let Some(FnArg::Typed(first)) = positional.next() else {
        return Err(syn::Error::new(
            operation.sig.span(),
            missing_context_argument_message(named, context),
        ));
    };
    if !is_context_argument(first.ty.as_ref(), context) {
        return Err(syn::Error::new(
            first.ty.span(),
            missing_context_argument_message(named, context),
        ));
    }

    let mut carried = Vec::new();
    for typed in positional.filter_map(|input| match input {
        FnArg::Typed(typed) => Some(typed),
        FnArg::Receiver(_) => None,
    }) {
        let Pat::Ident(argument) = typed.pat.as_ref() else {
            return Err(syn::Error::new(
                typed.pat.span(),
                plain_argument_name_message(named),
            ));
        };
        carried.push((argument.ident.clone(), typed.ty.as_ref().clone()));
    }

    if carried.len() > 1 {
        Ok(OperationInputs::Generated(carried))
    } else if let Some((_, only)) = carried.pop() {
        Ok(OperationInputs::Named(Box::new(only)))
    } else {
        Ok(OperationInputs::Empty)
    }
}

/// The `one_way` flag and the return type have to agree, and the check runs in both directions:
/// a forgotten `Result` is a build failure naming both choices rather than a silent
/// fire-and-forget, and a `one_way` operation that returns something is refused just as loudly.
fn operation_outcome(
    operation: &TraitItemFn,
    one_way: bool,
) -> Result<OperationOutcome, syn::Error> {
    let named = &operation.sig.ident;
    match (&operation.sig.output, one_way) {
        (ReturnType::Default, true) => Ok(OperationOutcome::OneWay),
        (ReturnType::Default, false) => Err(syn::Error::new(
            operation.sig.span(),
            missing_return_type_message(named),
        )),
        (ReturnType::Type(_, answered), true) => Err(syn::Error::new(
            answered.span(),
            one_way_returns_a_value_message(named),
        )),
        (ReturnType::Type(_, answered), false) => result_arms(answered)
            .map(|(success, error)| OperationOutcome::Reply {
                error: Box::new(error),
                success: Box::new(success),
            })
            .ok_or_else(|| syn::Error::new(answered.span(), non_result_return_message(named))),
    }
}

fn parse_operation(operation: &TraitItemFn, context: &Ident) -> Result<OperationDef, syn::Error> {
    let directive = OperationDirective::read(&operation.attrs)?;
    let inputs = operation_inputs(operation, context)?;
    let outcome = operation_outcome(operation, directive.one_way)?;
    let declared = operation.sig.ident.to_string();
    Ok(OperationDef {
        ident: operation.sig.ident.clone(),
        inputs,
        outcome,
        ts_name: RenameRule::CamelCase.apply_to_field(&declared),
        wire_name: directive
            .message
            .unwrap_or_else(|| RenameRule::KebabCase.apply_to_field(&declared)),
    })
}

fn context_on_the_wire_message(operation: &Ident, context: &Ident) -> String {
    format!(
        "service_schema: operation `{operation}` puts the context type `{context}` on the wire\n       \
         the context reaches no message and no schema, so it belongs in neither the arguments nor \
         either result arm"
    )
}

fn duplicate_ts_name_message(
    service: &Ident,
    spelling: &str,
    taken: &Ident,
    second: &Ident,
) -> String {
    format!(
        "service_schema: trait `{service}` spells two operations `{spelling}` in TypeScript\n       \
         `{taken}` and `{second}` differ in Rust and collide once camelCased"
    )
}

fn duplicate_wire_name_message(
    service: &Ident,
    carried: &str,
    taken: &Ident,
    second: &Ident,
) -> String {
    format!(
        "service_schema: trait `{service}` carries the wire name `{carried}` on two operations\n       \
         `{taken}` and `{second}` would be indistinguishable on the wire; move one with \
         `#[service_schema_op(message = \"...\")]`"
    )
}

/// Whether a type names the context anywhere inside it, `Ctx` and `Vec<Ctx>` alike. The context is
/// a type parameter of the trait, so an occurrence of its name in a message or a result arm is the
/// context itself rather than a coincidence.
fn names_the_context(declared: &Type, context: &Ident) -> bool {
    fn mentions(tree: &TokenTree, context: &Ident) -> bool {
        match tree {
            TokenTree::Group(group) => group
                .stream()
                .into_iter()
                .any(|inner| mentions(&inner, context)),
            TokenTree::Ident(named) => named == context,
            TokenTree::Literal(_) | TokenTree::Punct(_) => false,
        }
    }
    declared
        .to_token_stream()
        .into_iter()
        .any(|tree| mentions(&tree, context))
}

fn plain_argument_name_message(operation: &Ident) -> String {
    format!(
        "service_schema: operation `{operation}` takes an argument that is not a plain name\n       \
         an argument's name becomes a field on the message declared from the argument list"
    )
}

/// The two rules that can only be checked once every operation has been read: no wire name and no
/// TypeScript spelling is carried by two operations, and the context reaches neither the message
/// nor either result arm.
fn service_refusals(service: &ServiceDef) -> Option<syn::Error> {
    let mut refusals: Option<syn::Error> = None;
    let mut wire_names: HashMap<&str, &Ident> = HashMap::new();
    let mut ts_names: HashMap<&str, &Ident> = HashMap::new();
    for operation in &service.operations {
        if let Some(taken) = wire_names.insert(operation.wire_name.as_str(), &operation.ident) {
            refusals = Some(combined(
                refusals.take(),
                syn::Error::new(
                    operation.ident.span(),
                    duplicate_wire_name_message(
                        &service.ident,
                        &operation.wire_name,
                        taken,
                        &operation.ident,
                    ),
                ),
            ));
        }
        if let Some(taken) = ts_names.insert(operation.ts_name.as_str(), &operation.ident) {
            refusals = Some(combined(
                refusals.take(),
                syn::Error::new(
                    operation.ident.span(),
                    duplicate_ts_name_message(
                        &service.ident,
                        &operation.ts_name,
                        taken,
                        &operation.ident,
                    ),
                ),
            ));
        }
        for declared in wire_types(operation) {
            if names_the_context(declared, &service.context_param) {
                refusals = Some(combined(
                    refusals.take(),
                    syn::Error::new(
                        declared.span(),
                        context_on_the_wire_message(&operation.ident, &service.context_param),
                    ),
                ));
            }
        }
    }
    refusals
}

/// Every type an operation puts on the wire: the message it receives, and both arms of the reply
/// it answers with.
fn wire_types(operation: &OperationDef) -> Vec<&Type> {
    let mut carried: Vec<&Type> = match &operation.inputs {
        OperationInputs::Empty => Vec::new(),
        OperationInputs::Generated(arguments) => {
            arguments.iter().map(|(_, declared)| declared).collect()
        }
        OperationInputs::Named(declared) => vec![declared.as_ref()],
    };
    match &operation.outcome {
        OperationOutcome::OneWay => (),
        OperationOutcome::Reply { error, success } => {
            carried.push(success.as_ref());
            carried.push(error.as_ref());
        }
    }
    carried
}

/// The success and error arms of a `Result<Success, Error>`, or `None` for anything else — a bare
/// value, a unit, or a `Result` that names only one arm.
fn result_arms(answered: &Type) -> Option<(Type, Type)> {
    let Type::Path(named) = answered else {
        return None;
    };
    let last = named.path.segments.last()?;
    if last.ident != "Result" {
        return None;
    }
    let PathArguments::AngleBracketed(declared) = &last.arguments else {
        return None;
    };
    let mut arms = Vec::new();
    for argument in &declared.args {
        let GenericArgument::Type(arm) = argument else {
            continue;
        };
        arms.push(arm.clone());
    }
    let [success, error] = arms.as_slice() else {
        return None;
    };
    Some((success.clone(), error.clone()))
}
