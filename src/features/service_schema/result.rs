//! The `<Operation>Result` type: the two arms an operation declared, joined into one return type.
//!
//! The envelope is all this adds. Whatever the operation named as its success type and as its
//! error type crosses unchanged — no field is added to either, none removed, none renamed — so a
//! message that happens to carry a key called `type` keeps it and one that does not never gains
//! one.
//!
//! The failure arm holds the declared error *or* a fault, rather than the union growing a third
//! member: two members sharing `ok: false` would stop `ok` being a discriminant at all, and
//! narrowing on the envelope would then tell a caller only that the call failed. Two arms, with the
//! fault behind the literal `isServiceFault: true`, leave the compiler something to narrow on at
//! both levels.
//!
//! A one-way operation gets no result type. It declared no reply and therefore no error, and a
//! type joining arms it does not have would be a type nothing can be assigned from.
//!
//! Both names carry the service: `UsageServiceGetBalanceResult`, and the fault it can hold is
//! `UsageServiceFault`. Two services declaring a `get_balance` each would otherwise publish one
//! `GetBalanceResult` twice into the one flat file a bundle is.
//!
//! A `body = "stream"` operation's own success is declared in Rust as `StreamedAnswer`, a type with
//! no `#[model_schema()]` of its own — the ordinary `value` rendering would publish that bare name
//! as a TypeScript reference nothing declares. [`stream_success_ts_type`] stands in for it instead,
//! mirroring the Rust and Dart clients' own streamed record.

use crate::field_type::get_field_def;
use crate::rename_rule::RenameRule;
use crate::service_schema::parse::{
    BodyKind, HttpShape, OperationDef, OperationOutcome, ServiceDef, tuple_elements,
};
use syn::Type;

/// The TypeScript record a `body = "stream"` operation's own success answers with: a `contentRange`
/// left `undefined` at the operation's own `ok_status`, set to the range text at `206`, paired with
/// the body as the platform's own `ReadableStream<Uint8Array>` — mirrors the Rust client's own
/// `StreamedAnswer::Full`/`Partial` and the Dart client's own streamed record.
const STREAMED_ANSWER_TS_TYPE: &str =
    "{ contentRange: string | undefined; body: ReadableStream<Uint8Array> }";

pub fn emit(service: &ServiceDef) -> Vec<String> {
    let named = service.ident.to_string();
    service
        .operations
        .iter()
        .filter_map(|operation| result_type(&named, operation))
        .collect()
}

/// What one operation's result type is called: `get_balance` on `UsageService` answers a
/// `UsageServiceGetBalanceResult`. `None` for a one-way operation, which answers nothing.
pub fn result_name(service: &str, operation: &OperationDef) -> Option<String> {
    match &operation.outcome {
        OperationOutcome::OneWay => None,
        OperationOutcome::Reply {
            error: _error,
            success: _success,
        } => Some(format!(
            "{service}{}Result",
            RenameRule::PascalCase.apply_to_field(&operation.ident.to_string())
        )),
    }
}

fn result_type(service: &str, operation: &OperationDef) -> Option<String> {
    let OperationOutcome::Reply { error, success } = &operation.outcome else {
        return None;
    };
    let published = result_name(service, operation)?;
    let shape = HttpShape::of(operation);
    let value = if matches!(shape.body_kind, BodyKind::Stream) {
        stream_success_ts_type(&shape, success)
    } else {
        get_field_def("value", success, "").typescript_typename()
    };
    let failure = get_field_def("error", error, "").typescript_typename();
    let called = &operation.ts_name;
    Some(format!(
        "/**\n \
         * What `{called}` on `{service}` answers with: the value it declared, the error it \
         declared,\n \
         * or a fault it never declared.\n \
         */\n\
         export type {published} =\n  \
         | {{ ok: true; value: {value} }}\n  \
         | {{ ok: false; error: {failure} | {{ isServiceFault: true; fault: {service}Fault }} }};"
    ))
}

/// [`STREAMED_ANSWER_TS_TYPE`], wrapped in a tuple with one more element per declared `header_out`
/// entry — mirrors the JSON and bytes paths' own composition, and the Dart client's own
/// `stream_success_dart_type`. `success` is read only for the header types after the first slot;
/// the first slot is always the fixed streamed record; `StreamedAnswer` carries no
/// `#[model_schema()]` to resolve a TypeScript type from.
fn stream_success_ts_type(shape: &HttpShape, success: &Type) -> String {
    if shape.header_out.is_empty() {
        return STREAMED_ANSWER_TS_TYPE.to_owned();
    }
    let elements: Vec<&Type> = tuple_elements(success).into_iter().flatten().collect();
    let mut parts = vec![STREAMED_ANSWER_TS_TYPE.to_owned()];
    parts.extend(
        elements
            .iter()
            .skip(1)
            .map(|ty| get_field_def("value", ty, "").typescript_typename()),
    );
    format!("[{}]", parts.join(", "))
}
