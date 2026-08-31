//! The TypeScript client: a factory over an injected transport, with one camelCase method per
//! operation, shaped so a call site reads exactly as the hand-written one it replaces.
//!
//! # What a call site sees
//!
//! ```typescript
//! const usage = createUsageServiceClient(transport);
//! const result = await usage.getBalance({ organizationId });
//! if (result.ok) {
//!   render(result.value.credits);
//! } else if ("isServiceFault" in result.error) {
//!   reportUnexpected(result.error.fault);
//! } else {
//!   switch (result.error.errorCode) { … }
//! }
//! ```
//!
//! # Outbound validation comes before the transport, not after it
//!
//! Every method parses its message against the message's own generated schema first. A failure
//! answers with a fault **without reaching the transport**, naming the key that failed: the
//! operation never ran, so what came back is not one of the errors it declared, and a caller's
//! code is identical whether the fault came from here or from the far end.
//!
//! # Where the schemas come from, and why this module exists only beside them
//!
//! The schema a message validates against is the one `#[model_schema()]` publishes for it, which
//! only a build with the Zod surface on writes at all. This module is gated with it: a client whose
//! check was dropped would forward whatever it was handed while reading exactly like the checked
//! one, so a build that publishes no schema publishes no client either.
//!
//! # A one-way method has nowhere to return a fault
//!
//! A one-way operation's method answers `Promise<void>`, so a refused message cannot come back as
//! a value. It is thrown instead — the transport is still never reached, and a defect that would
//! otherwise vanish stays visible.
//!
//! What is thrown is part of the published surface rather than something to be discovered from the
//! emitted body, so it is named: `<Service>Refusal`, an `Error` carrying the fault on a `fault`
//! property. The method's own `JSDoc` names it too, `Promise<void>` having no room to say it.

use super::fault;
use super::message;
use super::result::result_name;
use crate::rename_rule::RenameRule;
use crate::service_schema::parse::{OperationDef, OperationOutcome, ServiceDef};

pub fn emit(service: &ServiceDef) -> Vec<String> {
    let mut published = vec![
        transport_type(&service.ident.to_string()),
        client_type(service),
    ];
    // The readers land ahead of the factory that calls them, a bundle being read top to bottom.
    published.extend(fault_helpers(service));
    published.push(factory(service));
    published
}

/// The client type: one method per operation, named the way a TypeScript caller expects to type it
/// and answering the operation's own result type.
fn client_type(service: &ServiceDef) -> String {
    let named = service.ident.to_string();
    let methods = service
        .operations
        .iter()
        .map(|operation| {
            format!(
                "{}\n  {}(req: {}): Promise<{}>;",
                method_doc(&named, operation),
                operation.ts_name,
                message::typename(operation),
                answers(&named, operation)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "/**\n \
         * A `{named}` caller, over any transport that can send an operation name beside a \
         payload.\n \
         *\n \
         * Every operation the service declares has a method here. A request-and-reply operation \
         answers\n \
         * its own result type; a one-way operation answers nothing beyond the send.\n \
         */\n\
         export type {named}Client = {{\n\
         {methods}\n\
         }};"
    )
}

/// The factory: it binds a transport and answers with the client, every method built the same way
/// — validate, then send.
fn factory(service: &ServiceDef) -> String {
    let named = service.ident.to_string();
    let methods = service
        .operations
        .iter()
        .map(|operation| method(service, operation))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "/**\n \
         * Binds a `{named}` client to a transport.\n \
         *\n \
         * The operation name is handed to the transport beside the payload, never inside it, so \
         no\n \
         * message type has to reserve a key for routing.\n \
         */\n\
         export function create{named}Client(transport: {named}Transport): {named}Client {{\n  \
         return {{\n\
         {methods}\n  \
         }};\n\
         }}"
    )
}

/// The two readers a validation failure goes through, emitted only where there is a schema to fail
/// against — and the thrower only where a one-way operation needs somewhere to put a fault.
///
/// Their names carry the service for the same reason every published type's does: a bundle is one
/// flat file, and ten services would otherwise declare one of each ten times over.
fn fault_helpers(service: &ServiceDef) -> Vec<String> {
    let named = service.ident.to_string();
    let prefix = RenameRule::CamelCase.apply_to_variant(&named);
    let mut helpers = vec![format!(
        "/**\n \
         * The fault a `{named}` client answers with when the message it was about to send failed \
         its\n \
         * own schema. The operation never ran, so this is not one of the errors it declared, and \
         the\n \
         * transport was never reached.\n \
         */\n\
         function {prefix}OutboundFault(\n  \
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
             kind: \"failed-validation\",\n    \
             operation,"
        )
    )];
    if service
        .operations
        .iter()
        .any(|operation| matches!(operation.outcome, OperationOutcome::OneWay))
    {
        helpers.push(format!(
            "/**\n \
             * What a one-way `{named}` method throws when it refuses the message it was \
             handed.\n \
             *\n \
             * `Promise<void>` has no failure arm and no value position, so the fault rides on \
             the\n \
             * thrown error rather than on something returned. Narrow a caught error with \
             `\"fault\" in\n \
             * caught` to read it.\n \
             */\n\
             export type {named}Refusal = Error & {{ fault: {named}Fault }};"
        ));
        helpers.push(format!(
            "/**\n \
             * How a one-way `{named}` method reports a message it refused. It answers \
             `Promise<void>`,\n \
             * so there is no failure arm to put a fault in and it is thrown instead — the \
             transport\n \
             * still never reached, the defect still visible.\n \
             */\n\
             function {prefix}Refused(fault: {named}Fault): {named}Refusal {{\n  \
             return Object.assign(\n    \
             new Error(`${{fault.kind}} in operation \\`${{fault.operation}}\\`: \
             ${{fault.detail}}`),\n    \
             {{ fault }},\n  \
             );\n\
             }}"
        ));
    }
    helpers
}

/// What one operation's method answers: its own result type, or nothing for a one-way operation.
fn answers(service: &str, operation: &OperationDef) -> String {
    result_name(service, operation).unwrap_or_else(|| "void".to_owned())
}

/// One method on the factory's returned object: parse the message, then reach the transport. The
/// transport is named only on the far side of the check, which is what makes "the transport was
/// never touched" something a test can observe.
fn method(service: &ServiceDef, operation: &OperationDef) -> String {
    let named = service.ident.to_string();
    let wire = &operation.wire_name;
    let call = &operation.ts_name;
    let checked = validation(service, operation);
    let sending = match operation.outcome {
        OperationOutcome::OneWay => {
            format!("      await transport.notify(\"{wire}\", validated.data);")
        }
        OperationOutcome::Reply { .. } => format!(
            "      return transport.request<{}>(\"{wire}\", validated.data);",
            answers(&named, operation)
        ),
    };
    format!("    async {call}(req) {{\n{checked}{sending}\n    }},")
}

/// The method's own `JSDoc`: one line where the signature already says everything, a block where
/// it does not. A one-way method's `Promise<void>` cannot say what the method throws, so the
/// `JSDoc` is the only place it can be said.
fn method_doc(service: &str, operation: &OperationDef) -> String {
    let summary = method_summary(service, operation);
    let thrown = throws_clause(service, operation);
    if thrown.is_empty() {
        format!("  /** {summary} */")
    } else {
        format!("  /**\n   * {summary}\n   *\n{thrown}\n   */")
    }
}

/// A one-line summary for the method's own `JSDoc`, so a bundle reader learns what a method sends
/// without opening the trait.
fn method_summary(service: &str, operation: &OperationDef) -> String {
    let wire = &operation.wire_name;
    match operation.outcome {
        OperationOutcome::OneWay => {
            format!("Sends `{wire}` on `{service}`, which expects no reply.")
        }
        OperationOutcome::Reply { .. } => {
            format!("Calls `{wire}` on `{service}` and waits for the answer.")
        }
    }
}

/// The outbound check and the refusal it leads to. It runs on every method, this module being
/// emitted only where there is a schema for a message to be checked against.
fn validation(service: &ServiceDef, operation: &OperationDef) -> String {
    let named = service.ident.to_string();
    let prefix = RenameRule::CamelCase.apply_to_variant(&named);
    let wire = &operation.wire_name;
    let schema = message::schema(operation);
    let refusal = match operation.outcome {
        OperationOutcome::OneWay => format!(
            "        throw {prefix}Refused({prefix}OutboundFault(\"{wire}\", \
             validated.error.issues));"
        ),
        OperationOutcome::Reply { .. } => format!(
            "        return {{\n          \
             ok: false,\n          \
             error: {{\n            \
             isServiceFault: true,\n            \
             fault: {prefix}OutboundFault(\"{wire}\", validated.error.issues),\n          \
             }},\n        \
             }};"
        ),
    };
    format!(
        "      const validated = {schema}.safeParse(req);\n      \
         if (!validated.success) {{\n{refusal}\n      \
         }}\n"
    )
}

/// What a method's `JSDoc` says it throws. Only a one-way method throws at all — a replying one
/// answers its refusal into the failure arm it already has.
fn throws_clause(service: &str, operation: &OperationDef) -> String {
    match operation.outcome {
        OperationOutcome::OneWay => format!(
            "   * @throws {{{service}Refusal}} when the message fails its own schema. The \
             operation\n   \
             * answers `Promise<void>`, so there is no failure arm to put the fault in; the \
             transport\n   \
             * is still never reached."
        ),
        OperationOutcome::Reply { .. } => String::new(),
    }
}

/// The transport seam: an operation name, a payload, and an answer. Emitted per service for the
/// same reason the Rust side declares one `Transport` trait per service module — TypeScript has no
/// per-service scope to keep two of them apart.
fn transport_type(service: &str) -> String {
    format!(
        "/**\n \
         * What binds a `{service}` client to a bus.\n \
         *\n \
         * The operation name travels beside the payload rather than inside it, so no message type \
         has\n \
         * to reserve a key for routing. The payload is handed over as a value rather than as \
         bytes:\n \
         * a transport merges its own fields — a correlation id, an error flag — into the object \
         before\n \
         * serializing it, and neither is reachable behind an encoded buffer.\n \
         */\n\
         export type {service}Transport = {{\n  \
         /** Sends a message no reply is expected for. */\n  \
         notify(operation: string, payload: unknown): Promise<void>;\n  \
         /** Sends a message and answers with the reply the far side wrote. */\n  \
         request<Answered>(operation: string, payload: unknown): Promise<Answered>;\n\
         }};"
    )
}
