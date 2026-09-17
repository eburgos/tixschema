//! The TypeScript `http_rest` client: one service-agnostic transport seam, and one method per
//! operation that builds a plain-terms request from the operation's own message and decodes the
//! answer by status.
//!
//! # Mirrors the Rust client's own shapes
//!
//! The seam here is the same seam `{service}_http_rest_client!` emits in Rust: a request record
//! in (method, path, query, headers, body), a response record out (status, headers, body), and
//! one method that sends the one and answers with the other. Nothing in either language names the
//! library that finally carries the call — that adapter is hand-written where it is used, against
//! this plain-terms surface, exactly as the Rust seam's own doc comments describe.
//!
//! # One seam type, not one per service
//!
//! Every service's alias of the seam carries the exact same two members, request in and response
//! out, with nothing service-specific in either shape. A hand-written implementation built once
//! therefore satisfies every service's own `{Service}HttpTransport` alias — TypeScript's structural
//! typing does the sharing, so the type itself still carries the service prefix every other
//! published name here does, since a bundle is one flat file with no scope of its own.
//!
//! # No envelope on the wire, an envelope at the call site
//!
//! A REST answer carries no `{ ok, value, error }` envelope on the wire (see
//! [`crate::service_schema::transport::http_rest`]'s own module doc). The method here still
//! answers the same `{Service}{Operation}Result` shape [`super::result`] already publishes for
//! every operation, caller-shaped rather than wire-shaped: `{ ok: true, value }` on the declared
//! `ok_status`, `{ ok: false, error: declared }` on a mapped status, and `{ ok: false, error: {
//! isServiceFault: true, fault } }` everywhere else. A one-way operation has no failure arm to
//! answer through — `Promise<void>` — so a refusal is thrown instead, exactly as the AMQP client's
//! one-way methods throw.
//!
//! # Outbound validation, transport failure, and the wire's own fixed faults
//!
//! Every method validates its message before it builds anything, the same as the AMQP client. A
//! transport that cannot carry the call, a status this client did not expect, and the wire's own
//! fixed fault statuses (400 validation, 404 unmatched route, 500 panic) all become the same
//! [`crate::service_schema::support`]-published fault type every other surface answers faults
//! through.
//!
//! # `body = "stream"` reads two success statuses, not one
//!
//! `reply_decode_stmt` peels `BodyKind::Stream` off first into its own status ladder (`206` reads
//! `content-range` back before the body is ever named; the declared `ok_status` leaves it
//! `undefined`; either way the answer carries `response.bodyStream` — the seam's own
//! `ReadableStream<Uint8Array>`, present only where the service declares a streamed operation) —
//! mirroring the Rust and Dart clients' own split. What is left (`success_decode_block`) only ever
//! sees `Bytes`, `Json` and `Multipart`.

use super::fault;
use super::message;
use super::result::result_name;
use crate::field_type::get_field_def;
use crate::rename_rule::RenameRule;
use crate::service_schema::parse::{
    BodyKind, DEFAULT_BINDING_ERROR_STATUS, HttpShape, OperationDef, OperationInputs,
    OperationOutcome, PathSegment, ScalarKind, ServiceDef, is_scalar_named_type, is_unit_type,
    option_inner, scalar_kind, service_declares_a_stream, service_declares_multipart,
    tuple_elements, vec_inner, wire_key,
};
use core::fmt::Write as _;
use syn::Type;

pub fn emit(service: &ServiceDef) -> Vec<String> {
    let has_stream = service_declares_a_stream(service);
    let has_multipart = service_declares_multipart(service);
    let mut published = vec![
        transport_type(&service.ident.to_string(), has_stream, has_multipart),
        client_type(service),
    ];
    published.extend(fault_helpers(service));
    published.push(factory(service));
    published
}

// ---------------------------------------------------------------------------------------------
// The transport seam and the client type
// ---------------------------------------------------------------------------------------------

/// The one seam type a `{service}` client sends through: a request record in, a response record
/// out, nothing service-specific in either and nothing here naming the library that finally
/// carries the call. The request record carries `parts` only where the service declares a
/// multipart operation - every other body kind still carries its content as `body`. The response
/// record carries `bodyStream` only where the service declares a streamed operation - a real
/// implementation can fill it a chunk at a time rather than buffering the whole answer first, while
/// `body` keeps answering eagerly for every other operation. `ReadableStream` is the platform's own
/// type, not a naming of `fetch`: the seam stays library-agnostic either way.
fn transport_type(service: &str, has_stream: bool, has_multipart: bool) -> String {
    let parts_field = if has_multipart {
        "\n    parts: ReadonlyArray<readonly [string, unknown]>;"
    } else {
        ""
    };
    let stream_field = if has_stream {
        "\n    bodyStream: ReadableStream<Uint8Array>;"
    } else {
        ""
    };
    format!(
        "/**\n \
         * What binds a `{service}` client to a real HTTP stack.\n \
         *\n \
         * One method, sending a whole request and answering with a whole response. The shapes \
         carry\n \
         * nothing service-specific, so one hand-written implementation (built once, over any HTTP \
         stack)\n \
         * satisfies every service's own alias of this type.\n \
         */\n\
         export type {service}HttpTransport = {{\n  \
         send(request: {{\n    \
         method: string;\n    \
         path: string;\n    \
         query: string;\n    \
         headers: ReadonlyArray<readonly [string, string]>;\n    \
         body: string;{parts_field}\n  \
         }}): Promise<{{\n    \
         status: number;\n    \
         headers: ReadonlyArray<readonly [string, string]>;\n    \
         body: string;{stream_field}\n  \
         }}>;\n\
         }};"
    )
}

/// The client type: one method per operation, taking that operation's own message plus one extra
/// argument per `header_in` binding.
fn client_type(service: &ServiceDef) -> String {
    let named = service.ident.to_string();
    let methods = service
        .operations
        .iter()
        .map(|operation| {
            let shape = HttpShape::of(operation);
            format!(
                "{}\n  {}({}): Promise<{}>;",
                method_doc(operation, &shape),
                operation.ts_name,
                method_params(operation, &shape),
                answers(&named, operation)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "/**\n \
         * A `{named}` caller over `http_rest`.\n \
         *\n \
         * Every operation the service declares has a method here, taking that operation's own \
         message\n \
         * plus one extra argument per `header_in` binding. A request-and-reply operation answers \
         its\n \
         * own result type; a one-way operation answers nothing beyond the send and throws \
         instead\n \
         * of returning a fault it has nowhere to put.\n \
         */\n\
         export type {named}HttpClient = {{\n\
         {methods}\n\
         }};"
    )
}

/// The factory: binds a transport, and answers with the client, every method built the same way —
/// validate, build the request, send, decode by status.
fn factory(service: &ServiceDef) -> String {
    let named = service.ident.to_string();
    let has_multipart = service_declares_multipart(service);
    let methods = service
        .operations
        .iter()
        .map(|operation| method(service, operation, has_multipart))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "/**\n \
         * Binds a `{named}` client to an `http_rest` transport.\n \
         */\n\
         export function create{named}HttpClient(\n  \
         transport: {named}HttpTransport,\n\
         ): {named}HttpClient {{\n  \
         return {{\n\
         {methods}\n  \
         }};\n\
         }}"
    )
}

/// What one operation's method answers: its own result type, or nothing for a one-way operation.
fn answers(service: &str, operation: &OperationDef) -> String {
    result_name(service, operation).unwrap_or_else(|| "void".to_owned())
}

/// The parameter list a method (and the client type's own member) takes: the message first, then
/// one argument per `header_in` binding, then one per `part` binding, in declaration order.
fn method_params(operation: &OperationDef, shape: &HttpShape) -> String {
    let mut params = vec![format!("req: {}", message::typename(operation))];
    for header in &shape.header_in {
        let name = RenameRule::CamelCase.apply_to_field(&header.parameter.to_string());
        let ty = get_field_def(&name, &header.ty, "").typescript_typename();
        params.push(format!("{name}: {ty}"));
    }
    for part in &shape.multipart_parts {
        let name = RenameRule::CamelCase.apply_to_field(&part.parameter.to_string());
        let ty = get_field_def(&name, &part.ty, "").typescript_typename();
        params.push(format!("{name}: {ty}"));
    }
    params.join(", ")
}

fn method_doc(operation: &OperationDef, shape: &HttpShape) -> String {
    format!(
        "  /** Calls `{}` over `{} {}`. */",
        operation.wire_name,
        shape.method.name(),
        shape.path_template()
    )
}

// ---------------------------------------------------------------------------------------------
// One operation's method
// ---------------------------------------------------------------------------------------------

/// One method on the factory's returned object: validate, build the request from the validated
/// message, send it, decode the answer by status.
fn method(service: &ServiceDef, operation: &OperationDef, has_multipart: bool) -> String {
    let named = service.ident.to_string();
    let prefix = RenameRule::CamelCase.apply_to_variant(&named);
    let wire = &operation.wire_name;
    let call = &operation.ts_name;
    let shape = HttpShape::of(operation);
    let params = method_params(operation, &shape);
    let validated = validation_stmt(&prefix, operation, wire);
    let path_build = path_build_stmt(operation, &shape);
    let query_build = query_build_stmt(operation, &shape);
    let headers_build = header_in_build_stmt(&shape);
    let body_build = body_build_stmt(&shape);
    let parts_build = multipart_parts_build_stmt(operation, &shape, has_multipart);
    let send = send_stmt(&prefix, operation, wire, shape.method.name(), has_multipart);
    let decode = decode_stmt(&prefix, operation, &shape, wire);
    format!(
        "    async {call}({params}) {{\n\
{validated}      \
         const sending = validated.data;\n\
{path_build}\
{query_build}\
{headers_build}\
{body_build}\
{parts_build}\
{send}\
{decode}\
    }},"
    )
}

/// The outbound check and the refusal it leads to: a failure answers before the transport is ever
/// named, exactly as the AMQP client's own outbound check does.
fn validation_stmt(prefix: &str, operation: &OperationDef, wire: &str) -> String {
    let schema = message::schema(operation);
    let refusal = match &operation.outcome {
        OperationOutcome::OneWay => format!(
            "        throw {prefix}HttpRefused(\n          \
             {prefix}HttpOutboundFault(\"{wire}\", validated.error.issues),\n        \
             );\n"
        ),
        OperationOutcome::Reply {
            error: _error,
            success: _success,
        } => format!(
            "        return {{\n          \
             ok: false,\n          \
             error: {{\n            \
             isServiceFault: true,\n            \
             fault: {prefix}HttpOutboundFault(\"{wire}\", validated.error.issues),\n          \
             }},\n        \
             }};\n"
        ),
    };
    format!(
        "      const validated = {schema}.safeParse(req);\n      \
         if (!validated.success) {{\n{refusal}      \
         }}\n"
    )
}

/// The value one path placeholder reads off `sending`: a generated field under its camelCase wire
/// key; the whole message where a `Named` type is itself a wire scalar; a `Named` type's own
/// field, spelled exactly as the placeholder names it, otherwise.
fn placeholder_value_expr(
    operation: &OperationDef,
    shape: &HttpShape,
    placeholder: &str,
) -> String {
    match &operation.inputs {
        OperationInputs::Empty => "sending".to_owned(),
        OperationInputs::Generated(_) => {
            format!(
                "sending.{}",
                RenameRule::CamelCase.apply_to_field(placeholder)
            )
        }
        OperationInputs::Named(declared) => {
            if shape.placeholder_names().len() == 1 && is_scalar_named_type(declared) {
                "sending".to_owned()
            } else {
                format!("sending.{placeholder}")
            }
        }
    }
}

fn path_build_stmt(operation: &OperationDef, shape: &HttpShape) -> String {
    let mut stmt = String::from("      let path = \"\";\n");
    for segment in &shape.path {
        match segment {
            PathSegment::Literal(text) => {
                let _ = writeln!(stmt, "      path += \"{text}\";");
            }
            PathSegment::Placeholder(name) => {
                let value = placeholder_value_expr(operation, shape, name);
                let _ = writeln!(stmt, "      path += encodeURIComponent(String({value}));");
            }
        }
    }
    stmt
}

fn named_query_build_stmt(shape: &HttpShape) -> String {
    let bound = shape
        .placeholder_names()
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<String>>()
        .join(", ");
    format!(
        "      const queryParts: Array<string> = [];\n      \
         const pathBound: ReadonlyArray<string> = [{bound}];\n      \
         for (const [key, value] of Object.entries(sending)) {{\n        \
         if (pathBound.includes(key) || value === undefined || value === null) {{\n          \
         continue;\n        \
         }}\n        \
         const rendered = Array.isArray(value)\n          \
         ? value.map((element) => String(element)).join(\",\")\n          \
         : String(value);\n        \
         queryParts.push(`${{key}}=${{encodeURIComponent(rendered)}}`);\n      \
         }}\n      \
         const query = queryParts.join(\"&\");\n"
    )
}

fn query_build_stmt(operation: &OperationDef, shape: &HttpShape) -> String {
    if shape.method.carries_a_body() {
        return "      const query = \"\";\n".to_owned();
    }
    let fields = match &operation.inputs {
        OperationInputs::Empty => return "      const query = \"\";\n".to_owned(),
        OperationInputs::Named(declared) => {
            return if is_scalar_named_type(declared) {
                "      const query = \"\";\n".to_owned()
            } else {
                named_query_build_stmt(shape)
            };
        }
        OperationInputs::Generated(fields) => fields,
    };
    let placeholders = shape.placeholder_names();
    let mut pushes = String::new();
    for (field, ty) in fields {
        let field_name = field.to_string();
        if placeholders.contains(&field_name) {
            continue;
        }
        let key = wire_key(field);
        let inner = option_inner(ty).unwrap_or(ty);
        if vec_inner(inner).is_some() {
            let _ = write!(
                pushes,
                "      if (sending.{key} !== undefined) {{\n        \
                 const joined = sending.{key}.map((element) => String(element)).join(\",\");\n        \
                 queryParts.push(`{key}=${{encodeURIComponent(joined)}}`);\n      \
                 }}\n"
            );
        } else {
            let _ = write!(
                pushes,
                "      if (sending.{key} !== undefined) {{\n        \
                 queryParts.push(`{key}=${{encodeURIComponent(String(sending.{key}))}}`);\n      \
                 }}\n"
            );
        }
    }
    if pushes.is_empty() {
        return "      const query = \"\";\n".to_owned();
    }
    format!(
        "      const queryParts: Array<string> = [];\n\
{pushes}      \
         const query = queryParts.join(\"&\");\n"
    )
}

/// Builds the outgoing header array, one entry per `header_in` binding — except an optional
/// binding holding `undefined`, which is pushed nowhere rather than as the text `String(undefined)`
/// would otherwise render (`"undefined"`, a header the request never meant to carry). Mirrors the
/// Rust client's own `header_in_build_stmts`.
fn header_in_build_stmt(shape: &HttpShape) -> String {
    let mut stmt = String::from("      const headers: Array<[string, string]> = [];\n");
    for header in &shape.header_in {
        let name = &header.name;
        let parameter = RenameRule::CamelCase.apply_to_field(&header.parameter.to_string());
        if option_inner(&header.ty).is_some() {
            let _ = write!(
                stmt,
                "      if ({parameter} !== undefined) {{\n        \
                 headers.push([\"{name}\", String({parameter})]);\n      \
                 }}\n"
            );
        } else {
            let _ = writeln!(
                stmt,
                "      headers.push([\"{name}\", String({parameter})]);"
            );
        }
    }
    stmt
}

fn body_build_stmt(shape: &HttpShape) -> String {
    if matches!(shape.body_kind, BodyKind::Multipart) {
        "      const body = \"\";\n".to_owned()
    } else if shape.method.carries_a_body() {
        "      const body = JSON.stringify(sending);\n".to_owned()
    } else {
        "      const body = \"\";\n".to_owned()
    }
}

/// The `parts` a `body = "multipart"` method sends: one text entry per carried `Generated` field
/// not otherwise placeholder-bound (under its own wire key), then one entry per declared `part`
/// binding (under its own declared name, its value the client's own extra argument, passed
/// through untouched) - mirrors the Rust client's own `multipart_parts_build_stmt`. Every other
/// body kind on a service that declares multipart still builds an empty `parts` so the request
/// literal has a value for the field; a service with no multipart operation at all builds nothing.
fn multipart_parts_build_stmt(
    operation: &OperationDef,
    shape: &HttpShape,
    has_multipart: bool,
) -> String {
    if !has_multipart {
        return String::new();
    }
    if !matches!(shape.body_kind, BodyKind::Multipart) {
        return "      const parts: Array<[string, unknown]> = [];\n".to_owned();
    }
    let placeholders = shape.placeholder_names();
    let mut stmt = String::from("      const parts: Array<[string, unknown]> = [];\n");
    if let OperationInputs::Generated(fields) = &operation.inputs {
        for (field, ty) in fields {
            let field_name = field.to_string();
            if placeholders.contains(&field_name) {
                continue;
            }
            let key = wire_key(field);
            let field_key = RenameRule::CamelCase.apply_to_field(&field_name);
            if option_inner(ty).is_some() {
                let _ = write!(
                    stmt,
                    "      if (sending.{field_key} !== undefined) {{\n        \
                     parts.push([\"{key}\", String(sending.{field_key})]);\n      \
                     }}\n"
                );
            } else {
                let _ = writeln!(
                    stmt,
                    "      parts.push([\"{key}\", String(sending.{field_key})]);"
                );
            }
        }
    }
    for part in &shape.multipart_parts {
        let name = &part.name;
        let parameter = RenameRule::CamelCase.apply_to_field(&part.parameter.to_string());
        let _ = writeln!(stmt, "      parts.push([\"{name}\", {parameter}]);");
    }
    stmt
}

fn send_stmt(
    prefix: &str,
    operation: &OperationDef,
    wire: &str,
    method_str: &str,
    has_multipart: bool,
) -> String {
    let failure = match &operation.outcome {
        OperationOutcome::OneWay => format!(
            "        throw {prefix}HttpRefused(\n          \
             {prefix}HttpTransportFailure(\"{wire}\", String(uncarried)),\n        \
             );\n"
        ),
        OperationOutcome::Reply {
            error: _error,
            success: _success,
        } => format!(
            "        return {{\n          \
             ok: false,\n          \
             error: {{\n            \
             isServiceFault: true,\n            \
             fault: {prefix}HttpTransportFailure(\"{wire}\", String(uncarried)),\n          \
             }},\n        \
             }};\n"
        ),
    };
    let parts_field = if has_multipart {
        "\n          parts,"
    } else {
        ""
    };
    format!(
        "      let response;\n      \
         try {{\n        \
         response = await transport.send({{\n          \
         method: \"{method_str}\",\n          \
         path,\n          \
         query,\n          \
         headers,\n          \
         body,{parts_field}\n        \
         }});\n      \
         }} catch (uncarried) {{\n{failure}      \
         }}\n"
    )
}

fn decode_stmt(prefix: &str, operation: &OperationDef, shape: &HttpShape, wire: &str) -> String {
    match &operation.outcome {
        OperationOutcome::OneWay => one_way_decode_stmt(prefix, shape, wire),
        OperationOutcome::Reply { error, success } => {
            reply_decode_stmt(prefix, shape, wire, error, success)
        }
    }
}

/// A one-way operation's decode: nothing to answer on the declared status, a thrown refusal
/// everywhere else — mirroring the Rust client's own `one_way_decode`.
fn one_way_decode_stmt(prefix: &str, shape: &HttpShape, wire: &str) -> String {
    let ok_status = shape.ok_status;
    format!(
        "      const status = response.status;\n      \
         if (status === {ok_status}) {{\n        \
         return;\n      \
         }}\n      \
         if (status === 400 || status === 404 || status === 500) {{\n        \
         throw {prefix}HttpRefused({prefix}HttpFaultFromBody(\"{wire}\", response.body));\n      \
         }}\n      \
         throw {prefix}HttpRefused(\n        \
         {prefix}HttpUndeserializablePayload(\n          \
         \"{wire}\",\n          \
         `an unexpected status (${{status}}) answered`,\n        \
         ),\n      \
         );\n"
    )
}

/// The condition that says a status is one of the operation's own declared errors — every
/// `error_status` code, or the fixed [`DEFAULT_BINDING_ERROR_STATUS`] where the operation declared
/// none.
fn error_condition_expr(shape: &HttpShape) -> String {
    if shape.error_status.is_empty() {
        format!("status === {DEFAULT_BINDING_ERROR_STATUS}")
    } else {
        shape
            .error_status
            .iter()
            .map(|(_, code)| format!("status === {code}"))
            .collect::<Vec<_>>()
            .join(" || ")
    }
}

/// A request-and-reply operation's decode: the declared status into the success type (or the
/// success tuple, `header_out` elements read back from response headers), a mapped status into
/// the declared error, a fixed fault status into a decoded fault, anything else into a fault
/// naming the status this client did not expect — mirroring the Rust client's own `reply_decode`.
fn reply_decode_stmt(
    prefix: &str,
    shape: &HttpShape,
    wire: &str,
    error: &Type,
    success: &Type,
) -> String {
    if matches!(shape.body_kind, BodyKind::Stream) {
        return stream_reply_decode_stmt(prefix, shape, wire, error, success);
    }
    let ok_status = shape.ok_status;
    let error_condition = error_condition_expr(shape);
    let error_ty = get_field_def("error", error, "").typescript_typename();
    let success_block = success_decode_block(prefix, wire, shape, success);
    format!(
        "      const status = response.status;\n      \
         if (status === {ok_status}) {{\n\
{success_block}      \
         }}\n      \
         if ({error_condition}) {{\n        \
         let declared: {error_ty};\n        \
         try {{\n          \
         declared = JSON.parse(response.body) as {error_ty};\n        \
         }} catch (rejected) {{\n          \
         return {{\n            \
         ok: false,\n            \
         error: {{\n              \
         isServiceFault: true,\n              \
         fault: {prefix}HttpUndeserializablePayload(\"{wire}\", String(rejected)),\n            \
         }},\n          \
         }};\n        \
         }}\n        \
         return {{ ok: false, error: declared }};\n      \
         }}\n      \
         if (status === 400 || status === 404 || status === 500) {{\n        \
         return {{\n          \
         ok: false,\n          \
         error: {{\n            \
         isServiceFault: true,\n            \
         fault: {prefix}HttpFaultFromBody(\"{wire}\", response.body),\n          \
         }},\n        \
         }};\n      \
         }}\n      \
         return {{\n        \
         ok: false,\n        \
         error: {{\n          \
         isServiceFault: true,\n          \
         fault: {prefix}HttpUndeserializablePayload(\n            \
         \"{wire}\",\n            \
         `an unexpected status (${{status}}) answered`,\n          \
         ),\n        \
         }},\n      \
         }};\n"
    )
}

/// The statements that read a success answer off the response, once its status has already
/// matched: nothing at all for a unit reply, the body alone for an ordinary reply, the response
/// bytes and their content type for a `body = "bytes"` operation, or the body plus every
/// `header_out` element read back from the response's own headers. `body = "stream"` never reaches
/// here — [`reply_decode_stmt`] answers it through [`stream_reply_decode_stmt`] instead, since a
/// streamed answer has two success statuses (`200` and `206`), not one.
fn success_decode_block(prefix: &str, wire: &str, shape: &HttpShape, success: &Type) -> String {
    if matches!(shape.body_kind, BodyKind::Bytes) {
        return bytes_success_decode_block(prefix, wire, shape, success);
    }
    if shape.header_out.is_empty() {
        if is_unit_type(success) {
            return "        return { ok: true, value: undefined };\n".to_owned();
        }
        let success_ty = get_field_def("value", success, "").typescript_typename();
        return format!(
            "        let value: {success_ty};\n        \
             try {{\n          \
             value = JSON.parse(response.body) as {success_ty};\n        \
             }} catch (rejected) {{\n          \
             return {{\n            \
             ok: false,\n            \
             error: {{\n              \
             isServiceFault: true,\n              \
             fault: {prefix}HttpUndeserializablePayload(\"{wire}\", String(rejected)),\n            \
             }},\n          \
             }};\n        \
             }}\n        \
             return {{ ok: true, value }};\n"
        );
    }
    let elements: Vec<&Type> = tuple_elements(success).into_iter().flatten().collect();
    let body_ty = elements.first().map_or_else(
        || get_field_def("value", success, "").typescript_typename(),
        |ty| get_field_def("value", ty, "").typescript_typename(),
    );
    let mut stmt = format!(
        "        let value: {body_ty};\n        \
         try {{\n          \
         value = JSON.parse(response.body) as {body_ty};\n        \
         }} catch (rejected) {{\n          \
         return {{\n            \
         ok: false,\n            \
         error: {{\n              \
         isServiceFault: true,\n              \
         fault: {prefix}HttpUndeserializablePayload(\"{wire}\", String(rejected)),\n            \
         }},\n          \
         }};\n        \
         }}\n"
    );
    let (header_stmts, header_idents) = header_out_read_stmts(prefix, wire, shape, &elements, 1);
    stmt.push_str(&header_stmts);
    let _ = writeln!(
        stmt,
        "        return {{ ok: true, value: [value, {}] }};",
        header_idents.join(", ")
    );
    stmt
}

/// A `body = "bytes"` operation's own success decode: the raw response body stands in for the
/// bytes - the seam already carries it as a plain `string`, so there is no `JSON.parse` on the
/// success path at all, mirroring the Rust client's own bytes decode, which reaches for no
/// `serde_json` either - `content-type` read back from the response headers, then any declared
/// `header_out` element after it, the same composition the JSON path performs, shifted one slot
/// for the content type.
fn bytes_success_decode_block(
    prefix: &str,
    wire: &str,
    shape: &HttpShape,
    success: &Type,
) -> String {
    let mut stmt = String::from(
        "        const contentType = response.headers.find(\n          \
         ([name]) => name.toLowerCase() === \"content-type\",\n        \
         )?.[1] ?? \"\";\n",
    );
    if shape.header_out.is_empty() {
        stmt.push_str("        return { ok: true, value: [response.body, contentType] };\n");
        return stmt;
    }
    let elements: Vec<&Type> = tuple_elements(success).into_iter().flatten().collect();
    let (header_stmts, header_idents) = header_out_read_stmts(prefix, wire, shape, &elements, 2);
    stmt.push_str(&header_stmts);
    let _ = writeln!(
        stmt,
        "        return {{ ok: true, value: [response.body, contentType, {}] }};",
        header_idents.join(", ")
    );
    stmt
}

/// A `body = "stream"` operation's own decode: `206` answers the streamed record with its own
/// `contentRange` read back off the response ahead of the body; the declared `ok_status` answers
/// the same record with `contentRange` left `undefined`; everything else falls through the same
/// declared-error, fixed-fault and unexpected-status ladder every other kind answers through —
/// mirrors the Rust and Dart clients' own stream decode.
fn stream_reply_decode_stmt(
    prefix: &str,
    shape: &HttpShape,
    wire: &str,
    error: &Type,
    success: &Type,
) -> String {
    let ok_status = shape.ok_status;
    let error_condition = error_condition_expr(shape);
    let error_ty = get_field_def("error", error, "").typescript_typename();
    let partial = stream_success_arm(prefix, wire, shape, success, true);
    let full = stream_success_arm(prefix, wire, shape, success, false);
    format!(
        "      const status = response.status;\n      \
         if (status === 206) {{\n\
{partial}      \
         }}\n      \
         if (status === {ok_status}) {{\n\
{full}      \
         }}\n      \
         if ({error_condition}) {{\n        \
         let declared: {error_ty};\n        \
         try {{\n          \
         declared = JSON.parse(response.body) as {error_ty};\n        \
         }} catch (rejected) {{\n          \
         return {{\n            \
         ok: false,\n            \
         error: {{\n              \
         isServiceFault: true,\n              \
         fault: {prefix}HttpUndeserializablePayload(\"{wire}\", String(rejected)),\n            \
         }},\n          \
         }};\n        \
         }}\n        \
         return {{ ok: false, error: declared }};\n      \
         }}\n      \
         if (status === 400 || status === 404 || status === 500) {{\n        \
         return {{\n          \
         ok: false,\n          \
         error: {{\n            \
         isServiceFault: true,\n            \
         fault: {prefix}HttpFaultFromBody(\"{wire}\", response.body),\n          \
         }},\n        \
         }};\n      \
         }}\n      \
         return {{\n        \
         ok: false,\n        \
         error: {{\n          \
         isServiceFault: true,\n          \
         fault: {prefix}HttpUndeserializablePayload(\n            \
         \"{wire}\",\n            \
         `an unexpected status (${{status}}) answered`,\n          \
         ),\n        \
         }},\n      \
         }};\n"
    )
}

/// One status arm of [`stream_reply_decode_stmt`]: `contentRange` read back off the response for a
/// `206` partial answer, left `undefined` for the declared `ok_status`'s whole-body answer — both
/// paired with `response.bodyStream`, the seam's own lazily-pulled source — then every declared
/// `header_out` element read back exactly as the bytes and JSON paths do.
fn stream_success_arm(
    prefix: &str,
    wire: &str,
    shape: &HttpShape,
    success: &Type,
    partial: bool,
) -> String {
    let mut stmt = if partial {
        "        const contentRange =\n          \
         response.headers.find(\n            \
         ([name]) => name.toLowerCase() === \"content-range\",\n          \
         )?.[1] ?? \"\";\n"
            .to_owned()
    } else {
        "        const contentRange: string | undefined = undefined;\n".to_owned()
    };
    stmt.push_str("        const answer = { contentRange, body: response.bodyStream };\n");
    if shape.header_out.is_empty() {
        stmt.push_str("        return { ok: true, value: answer };\n");
        return stmt;
    }
    let elements: Vec<&Type> = tuple_elements(success).into_iter().flatten().collect();
    let (header_stmts, header_idents) = header_out_read_stmts(prefix, wire, shape, &elements, 1);
    stmt.push_str(&header_stmts);
    let _ = writeln!(
        stmt,
        "        return {{ ok: true, value: [answer, {}] }};",
        header_idents.join(", ")
    );
    stmt
}

/// Reads every declared `header_out` element back off the response headers, in declaration order,
/// skipping `body_elements` positions in `elements` to reach the first header slot — 1 for the
/// ordinary JSON and streamed shapes (the value or answer alone), 2 for `body = "bytes"` (the bytes
/// and their content type). Shared by every success-decoding arm so the copies cannot drift.
fn header_out_read_stmts(
    prefix: &str,
    wire: &str,
    shape: &HttpShape,
    elements: &[&Type],
    body_elements: usize,
) -> (String, Vec<String>) {
    let mut stmt = String::new();
    let mut idents = Vec::new();
    for (index, (name, element_ty)) in shape
        .header_out
        .iter()
        .zip(elements.iter().skip(body_elements))
        .enumerate()
    {
        let raw_ident = format!("rawHeaderOut{index}");
        let ident = format!("headerOut{index}");
        let element_typename = get_field_def("value", element_ty, "").typescript_typename();
        let _ = write!(
            stmt,
            "        const {raw_ident} = response.headers.find(\n          \
             ([name]) => name.toLowerCase() === \"{name}\",\n        \
             );\n        \
             if ({raw_ident} === undefined) {{\n          \
             return {{\n            \
             ok: false,\n            \
             error: {{\n              \
             isServiceFault: true,\n              \
             fault: {prefix}HttpUndeserializablePayload(\n                \
             \"{wire}\",\n                \
             \"a declared response header was missing\",\n              \
             ),\n            \
             }},\n          \
             }};\n        \
             }}\n"
        );
        let value_expr = header_out_value_expr(element_ty, &format!("{raw_ident}[1]"));
        let _ = writeln!(
            stmt,
            "        const {ident} = {value_expr} as {element_typename};"
        );
        idents.push(ident);
    }
    (stmt, idents)
}

/// The expression that reads a `header_out` element's declared type back off `raw` — a `string`
/// expression holding the raw header text. Mirrors the coercion the Rust client's own
/// `decode_expr` performs on the way back from a response header, minus the fallible middle step:
/// a value already trusted enough to publish under a declared type is read directly rather than
/// re-validated a second time.
fn header_out_value_expr(ty: &Type, raw: &str) -> String {
    let base = option_inner(ty).unwrap_or(ty);
    if let Some(inner) = vec_inner(base) {
        let element = header_out_value_expr(inner, "piece");
        return format!("({raw}).split(\",\").map((piece: string) => {element})");
    }
    match scalar_kind(base) {
        ScalarKind::Bool => format!("(({raw}) === \"true\")"),
        ScalarKind::Number => format!("Number({raw})"),
        ScalarKind::Text => raw.to_owned(),
    }
}

// ---------------------------------------------------------------------------------------------
// The fault helpers every method reaches for
// ---------------------------------------------------------------------------------------------

/// The four fault-building readers every method reaches for, plus the one-way throw pair where the
/// service declares at least one one-way operation.
fn fault_helpers(service: &ServiceDef) -> Vec<String> {
    let named = service.ident.to_string();
    let prefix = RenameRule::CamelCase.apply_to_variant(&named);
    let mut helpers = vec![
        outbound_fault_fn(&named, &prefix),
        transport_failure_fn(&named, &prefix),
        undeserializable_payload_fn(&named, &prefix),
        fault_from_body_fn(&named, &prefix),
    ];
    if service
        .operations
        .iter()
        .any(|operation| matches!(operation.outcome, OperationOutcome::OneWay))
    {
        helpers.push(refusal_type(&named));
        helpers.push(refused_fn(&named, &prefix));
    }
    helpers
}

fn outbound_fault_fn(named: &str, prefix: &str) -> String {
    format!(
        "/**\n \
         * The fault a `{named}` HTTP client answers with when the message it was about to send \
         failed\n \
         * its own schema. The operation never ran, so this is not one of the errors it declared, \
         and\n \
         * the transport was never reached.\n \
         */\n\
         function {prefix}HttpOutboundFault(\n  \
         operation: string,\n  \
         issues: ReadonlyArray<{{ path: ReadonlyArray<PropertyKey>; message: string }}>,\n\
         ): {named}Fault {{\n  \
         const [first] = issues;\n  \
         const failedAt = first === undefined ? \"\" : first.path.join(\".\");\n\
         {minted}\n\
         }}",
        minted = fault::minted(
            named,
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
    )
}

fn transport_failure_fn(named: &str, prefix: &str) -> String {
    format!(
        "/**\n \
         * The fault a `{named}` HTTP client answers with when the transport could not carry a \
         call:\n \
         * the request never went out, or the response never came back.\n \
         */\n\
         function {prefix}HttpTransportFailure(operation: string, detail: string): {named}Fault {{\n\
         {minted}\n\
         }}",
        minted = fault::minted(
            named,
            "    detail,\n    field: undefined,\n    kind: \"transport-failure\",\n    operation,"
        )
    )
}

fn undeserializable_payload_fn(named: &str, prefix: &str) -> String {
    format!(
        "/**\n \
         * The fault a `{named}` HTTP client answers with when a response will not become the \
         answer\n \
         * its status promised: a body that will not parse, a declared header that never \
         arrived, or\n \
         * a status this client did not expect.\n \
         */\n\
         function {prefix}HttpUndeserializablePayload(\n  \
         operation: string,\n  \
         detail: string,\n\
         ): {named}Fault {{\n\
         {minted}\n\
         }}",
        minted = fault::minted(
            named,
            "    detail,\n    field: undefined,\n    kind: \"undeserializable-payload\",\n    \
             operation,"
        )
    )
}

fn fault_from_body_fn(named: &str, prefix: &str) -> String {
    let members = format!(
        "      detail: parsed.detail,\n      \
         field: parsed.field as string | undefined,\n      \
         kind: parsed.kind as {named}FaultKind,\n      \
         operation: parsed.operation,"
    );
    format!(
        "/**\n \
         * Reads a fixed fault status's own body back into a `{named}Fault`, from whatever it \
         holds.\n \
         * A body that does not read as the fixed fault shape is itself a defect answered as \
         one.\n \
         */\n\
         function {prefix}HttpFaultFromBody(operation: string, bodyText: string): {named}Fault {{\n  \
         try {{\n    \
         const parsed = JSON.parse(bodyText) as {{\n      \
         detail?: unknown;\n      \
         field?: unknown;\n      \
         kind?: unknown;\n      \
         operation?: unknown;\n    \
         }};\n    \
         const knownKinds: ReadonlyArray<unknown> = [\n      \
         \"failed-validation\",\n      \
         \"handler-panic\",\n      \
         \"transport-failure\",\n      \
         \"undeserializable-payload\",\n      \
         \"unknown-operation\",\n    \
         ];\n    \
         if (\n      \
         typeof parsed.detail === \"string\" &&\n      \
         (parsed.field === undefined || typeof parsed.field === \"string\") &&\n      \
         typeof parsed.kind === \"string\" &&\n      \
         knownKinds.includes(parsed.kind) &&\n      \
         typeof parsed.operation === \"string\"\n    \
         ) {{\n\
         {minted}\n    \
         }}\n  \
         }} catch {{\n    \
         // Falls through to the payload below.\n  \
         }}\n  \
         return {prefix}HttpUndeserializablePayload(\n    \
         operation,\n    \
         \"a fault body did not match the expected shape\",\n  \
         );\n\
         }}",
        minted = fault::minted(named, &members)
    )
}

fn refusal_type(named: &str) -> String {
    format!(
        "/**\n \
         * What a one-way `{named}` HTTP method throws when it cannot answer its declared \
         status.\n \
         *\n \
         * `Promise<void>` has no failure arm and no value position, so the fault rides on the \
         thrown\n \
         * error rather than on something returned. Narrow a caught error with `\"fault\" in \
         caught`\n \
         * to read it.\n \
         */\n\
         export type {named}HttpRefusal = Error & {{ fault: {named}Fault }};"
    )
}

fn refused_fn(named: &str, prefix: &str) -> String {
    format!(
        "/**\n \
         * How a one-way `{named}` HTTP method reports a call it could not carry out. It answers \
         `Promise<void>`, so there is no failure arm to put a fault in, and it is thrown instead.\n \
         */\n\
         function {prefix}HttpRefused(fault: {named}Fault): {named}HttpRefusal {{\n  \
         return Object.assign(\n    \
         new Error(`${{fault.kind}} in operation \\`${{fault.operation}}\\`: ${{fault.detail}}`),\n    \
         {{ fault }},\n  \
         );\n\
         }}"
    )
}
