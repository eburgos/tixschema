//! The TypeScript a service implements: an interface it satisfies in full or does not compile, the
//! outcomes its operations answer with, and the factory that turns an implementation into a
//! dispatcher.
//!
//! # Why an interface and not a table of handlers
//!
//! This is the piece the whole construct exists for. An operation declared and implemented by
//! nobody is only prevented if the compiler refuses the incomplete implementation, so the emitted
//! interface has one required member per operation — no optional members, no index signature,
//! nothing a partial implementation slips through. Adding an operation breaks every implementation
//! of the service, which is the point.
//!
//! # An implementation cannot fabricate a fault
//!
//! An operation publishes two types, not one. `<Service><Operation>Result` is what a *caller*
//! reads: the value, the declared error, or a fault. `<Service><Operation>Outcome` is what an
//! *implementation* returns, and its failure arm is the declared error alone. A fault reports a
//! failure the operation never declared, and the two places entitled to build one are both
//! generated — this dispatcher and the client.
//!
//! # The dispatcher exists only beside a schema
//!
//! An arm parses the payload before it calls, which is what entitles an implementation to assume
//! its message is valid. The parse is against the `<Message>$Schema` const `#[model_schema()]`
//! publishes, so this module is gated with the Zod surface that writes one: a build without it
//! publishes no dispatcher rather than one that narrows an unread payload with `as` and hands it to
//! an implementation written against a guarantee nothing checked.
//!
//! # The context is explicit and generic
//!
//! The interface carries a context type parameter and every method takes it, mirroring the Rust
//! trait. The code owning the transport constructs one per message and hands it to the dispatcher.
//! It appears in no message and no schema.

use super::fault;
use super::message;
use super::result::result_name;
use crate::field_type::get_field_def;
use crate::rename_rule::RenameRule;
use crate::service_schema::parse::{OperationDef, OperationOutcome, ServiceDef};

pub fn emit(service: &ServiceDef) -> Vec<String> {
    let mut published = outcome_types(service);
    published.push(interface(service));
    published.extend(fault_helpers(service));
    published.push(dispatcher(service));
    published
}

/// One arm of the dispatcher's `switch`: parse the payload, then call. The order is the point — an
/// implementation may assume its message is valid, because an invalid one never reaches it.
fn arm(service: &ServiceDef, operation: &OperationDef) -> String {
    let wire = &operation.wire_name;
    let call = &operation.ts_name;
    let received = payload_check(service, operation);
    let answering = match operation.outcome {
        OperationOutcome::OneWay => {
            format!("        await impl.{call}(ctx, received.data);\n        return undefined;")
        }
        OperationOutcome::Reply { .. } => {
            format!("        return impl.{call}(ctx, received.data);")
        }
    };
    format!("      case \"{wire}\": {{\n{received}{answering}\n      }}")
}

/// The factory: an implementation in, a dispatch function out. It answers with what the transport
/// puts on the wire — the operation's envelope, or a fault framed inside a failure arm — and with
/// nothing at all for a one-way operation that ran.
fn dispatcher(service: &ServiceDef) -> String {
    let named = service.ident.to_string();
    let prefix = RenameRule::CamelCase.apply_to_variant(&named);
    let arms = service
        .operations
        .iter()
        .map(|operation| arm(service, operation))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "/**\n \
         * Turns a `{named}` implementation into the function a transport drives it with.\n \
         *\n \
         * The operation is read from the argument the transport passed beside the payload, never \
         out\n \
         * of the payload itself. What comes back is what goes on the wire: the operation's own \
         result\n \
         * envelope, a fault framed inside a failure arm, or nothing at all where the operation \
         expects\n \
         * no reply.\n \
         */\n\
         export function create{named}Dispatcher<Ctx>(\n  \
         impl: {named}Impl<Ctx>,\n\
         ): (ctx: Ctx, operation: string, payload: unknown) => Promise<unknown> {{\n  \
         return async (ctx, operation, payload) => {{\n    \
         switch (operation) {{\n\
         {arms}\n      \
         default:\n        \
         return {prefix}Framed({prefix}UnknownOperation(operation));\n    \
         }}\n  \
         }};\n\
         }}"
    )
}

/// The three readers the dispatcher answers through: the framing a fault crosses in, the fault an
/// unrecognised operation gets, and the one a payload that failed its schema gets.
fn fault_helpers(service: &ServiceDef) -> Vec<String> {
    let named = service.ident.to_string();
    let prefix = RenameRule::CamelCase.apply_to_variant(&named);
    let mut helpers = vec![
        format!(
            "/**\n \
             * How a fault reaches a caller: inside the failure arm, behind the literal a caller \
             in\n \
             * either language narrows on. It is the shape every `{named}` result type declares, \
             and\n \
             * the shape the Rust dispatcher's transport frames.\n \
             */\n\
             function {prefix}Framed(\n  \
             fault: {named}Fault,\n\
             ): {{ ok: false; error: {{ isServiceFault: true; fault: {named}Fault }} }} {{\n  \
             return {{ ok: false, error: {{ isServiceFault: true, fault }} }};\n\
             }}"
        ),
        format!(
            "/**\n \
             * The fault an operation name nothing on `{named}` answers to produces. The name is \
             the\n \
             * one that arrived, not one the service declares.\n \
             */\n\
             function {prefix}UnknownOperation(operation: string): {named}Fault {{\n\
             {minted}\n\
             }}",
            minted = fault::minted(
                &named,
                "    detail: \"the service answers to no operation by that name\",\n    \
                 field: undefined,\n    \
                 kind: \"unknown-operation\",\n    \
                 operation,"
            )
        ),
    ];
    helpers.extend(inbound_fault(service));
    helpers
}

/// The fault a payload that will not become the operation's message produces, split the way the
/// Rust dispatcher splits it: a payload that failed at no key at all was never the message, and one
/// that failed at a key failed the message's own validation.
fn inbound_fault(service: &ServiceDef) -> Vec<String> {
    let named = service.ident.to_string();
    let prefix = RenameRule::CamelCase.apply_to_variant(&named);
    vec![format!(
        "/**\n \
         * The fault a payload produces when it will not become the operation's message.\n \
         *\n \
         * A failure that names no key at all is a payload that was never this message, which is \
         what\n \
         * the Rust side reports when the bytes would not deserialize. A failure at a key is \
         the\n \
         * message's own validation, and the fault names the key.\n \
         */\n\
         function {prefix}InboundFault(\n  \
         operation: string,\n  \
         issues: ReadonlyArray<{{ path: ReadonlyArray<PropertyKey>; message: string }}>,\n\
         ): {named}Fault {{\n  \
         const [first] = issues;\n  \
         const failedAt = first === undefined ? \"\" : first.path.join(\".\");\n\
         {minted}\n\
         }}",
        minted = fault::minted(
            &named,
            "    detail: issues\n      \
             .map((issue) =>\n        \
             issue.path.length === 0 ? issue.message : `'${issue.path.join(\".\")}': \
             ${issue.message}`,\n      \
             )\n      \
             .join(\"; \"),\n    \
             field: failedAt === \"\" ? undefined : failedAt,\n    \
             kind: failedAt === \"\" ? \"undeserializable-payload\" : \"failed-validation\",\n    \
             operation,"
        )
    )]
}

/// The interface an implementation satisfies. Every member is required and none is optional, so an
/// implementation missing one is refused where it reaches the factory.
fn interface(service: &ServiceDef) -> String {
    let named = service.ident.to_string();
    let methods = service
        .operations
        .iter()
        .map(|operation| {
            format!(
                "  /** {} */\n  {}(ctx: Ctx, req: {}): Promise<{}>;",
                method_summary(&named, operation),
                operation.ts_name,
                message::typename(operation),
                implementation_answers(&named, operation)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "/**\n \
         * What a `{named}` implementation satisfies, in full or not at all.\n \
         *\n \
         * Every operation the service declares is a required member, so an implementation missing \
         one\n \
         * is refused where it reaches `create{named}Dispatcher`. Adding an operation breaks every\n \
         * implementation until each one handles it, which is what the interface is for.\n \
         *\n \
         * The context is the implementation's own type, constructed per message by whatever owns \
         the\n \
         * transport. It reaches no message and no schema.\n \
         */\n\
         export interface {named}Impl<Ctx> {{\n\
         {methods}\n\
         }}"
    )
}

/// What an implementation's method answers: the two arms the operation declared, and never a
/// fault.
fn implementation_answers(service: &str, operation: &OperationDef) -> String {
    outcome_name(service, operation).unwrap_or_else(|| "void".to_owned())
}

/// A one-line summary for the member's own `JSDoc`.
fn method_summary(service: &str, operation: &OperationDef) -> String {
    let wire = &operation.wire_name;
    match operation.outcome {
        OperationOutcome::OneWay => {
            format!("Handles `{wire}` on `{service}`, which expects no reply.")
        }
        OperationOutcome::Reply { .. } => {
            format!("Handles `{wire}` on `{service}` and answers it.")
        }
    }
}

/// What one operation's implementation-side type is called. It sits beside the caller-side result
/// type and differs from it in exactly one way: no fault.
fn outcome_name(service: &str, operation: &OperationDef) -> Option<String> {
    result_name(service, operation)
        .map(|published| format!("{}Outcome", published.trim_end_matches("Result")))
}

/// The outcome type per operation that answers, written out in full rather than derived from the
/// caller-side result: the two are read side by side, and a reader should be able to see that one
/// admits a fault and the other does not.
fn outcome_types(service: &ServiceDef) -> Vec<String> {
    let named = service.ident.to_string();
    service
        .operations
        .iter()
        .filter_map(|operation| outcome_type(&named, operation))
        .collect()
}

fn outcome_type(service: &str, operation: &OperationDef) -> Option<String> {
    let OperationOutcome::Reply { error, success } = &operation.outcome else {
        return None;
    };
    let published = outcome_name(service, operation)?;
    let value = get_field_def("value", success, "").typescript_typename();
    let failure = get_field_def("error", error, "").typescript_typename();
    let called = &operation.ts_name;
    Some(format!(
        "/**\n \
         * What an implementation of `{called}` on `{service}` answers with: the value it \
         declared, or\n \
         * the error it declared.\n \
         *\n \
         * A fault is not among them. It reports a failure the operation never declared, and the \
         two\n \
         * places entitled to build one are both generated — the dispatcher and the client.\n \
         */\n\
         export type {published} =\n  \
         | {{ ok: true; value: {value} }}\n  \
         | {{ ok: false; error: {failure} }};"
    ))
}

/// The parse that runs before the implementation is called. It runs in every arm, this module
/// being emitted only where there is a schema to parse against.
fn payload_check(service: &ServiceDef, operation: &OperationDef) -> String {
    let named = service.ident.to_string();
    let prefix = RenameRule::CamelCase.apply_to_variant(&named);
    let wire = &operation.wire_name;
    let schema = message::schema(operation);
    format!(
        "        const received = {schema}.safeParse(payload);\n        \
         if (!received.success) {{\n          \
         return {prefix}Framed({prefix}InboundFault(\"{wire}\", received.error.issues));\n        \
         }}\n"
    )
}
