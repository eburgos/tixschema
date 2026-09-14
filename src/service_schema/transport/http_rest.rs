//! The HTTP/REST transport: two inert macros, `{service}_http_rest_dispatcher!` and
//! `{service}_http_rest_client!`, and nothing compiled where the service is declared.
//!
//! This version covers JSON, bytes and streamed bodies (the last with 206/`content-range` range
//! answers), declared statuses, header bindings, no-payload operations, a panic guard, and an
//! owner-overridable [`FaultHandler`] seam whose provided default answers the fixed behavior this
//! dispatcher always answered (400 validation, 404 unmatched route, 500 panic). Multipart and the
//! TypeScript client's own streamed-body coverage are separate, later work — nothing here stands
//! in their way.
//!
//! # Why the flat AMQP seam cannot carry this
//!
//! `amqp_rpc`'s dispatcher matches on an operation name a transport read beside the payload.
//! `http_rest` matches on a method and a path template, decodes path segments, a query string and
//! request headers into the operation's own arguments, and answers with a status, response headers
//! and a bare JSON body — none of which the flat `(operation, payload)` seam can express. So
//! `http_rest` is its own emitter, built the "Adding one" way the transport registry's own module
//! documentation prescribes.
//!
//! # No envelope
//!
//! Unlike `amqp_rpc`'s `{ ok, value, error }`, a REST answer carries no envelope: a success is the
//! declared `ok_status` with the success type serialized directly as the body; a declared error is
//! its mapped `error_status` with the error type itself serialized as the body; a fault answers
//! through the installed [`FaultHandler`], whose provided default is a small fixed JSON naming it.
//!
//! # An operation naming no `http(...)` group
//!
//! `OperationDef::http` is `None` for such an operation — "a transport defaults it on its own", per
//! [`crate::service_schema::parse`]'s own module documentation — and [`HttpShape::of`] is where this
//! module answers that: `POST /{wire-name}`, the declared (or default) `ok_status`, no header
//! bindings, and every declared error answered at the fixed status [`DEFAULT_BINDING_ERROR_STATUS`]
//! rather than a per-variant table, there being no annotation to read one from.
//!
//! # Path, query and header values
//!
//! A path placeholder and a query parameter both travel as text and both have to become the right
//! shape of JSON before the operation's own message can be built from them: `true`/`false` become a
//! JSON boolean, anything that parses as a number becomes a JSON number, anything else stays a JSON
//! string, and a comma-separated value behind a `Vec<T>` becomes a JSON array of that same coercion
//! applied to each piece. [`decode_expr`] is the one place that judgement is made, from the
//! argument's own declared type, and it is used identically for a path placeholder, a query
//! parameter and a `header_in` binding — the three read the same way here that the client writes
//! them.
//!
//! A path placeholder on a *generated* message (declared from the operation's own argument list, or
//! empty) binds a same-named field, exactly as a placeholder is checked against one. A path
//! placeholder on a message the operation already names (`OperationInputs::Named`) has no field for
//! this macro to see — the type is the author's own — so it is read back under its own written
//! spelling instead: `{document_id}` becomes the JSON key `"document_id"`, or, where there is
//! exactly one placeholder and the named type is one of the primitive shapes this macro recognises,
//! the whole message *is* that one coerced value. Either way the author's type must expose the wire
//! shape the placeholder implies — this macro cannot check that any more than it can check an error
//! enum's variants against `error_status`.

use super::Transport;
use super::amqp_rpc::{
    Generated, answers, call_arguments, call_message, outbound_refusal, panic_guard, placement_doc,
    refusal_reader,
};
use crate::service_schema::parse::{
    BodyKind, DEFAULT_BINDING_ERROR_STATUS, HeaderIn, HttpShape, MultipartPart, OperationDef,
    OperationInputs, OperationOutcome, PathSegment, ScalarKind, ServiceDef, is_scalar_named_type,
    is_unit_type, option_inner, scalar_kind, service_declares_a_stream, service_declares_multipart,
    tuple_elements, vec_inner, wire_key,
};
use crate::service_schema::support::{message_alias_ident, message_validator_ident, module_ident};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, Type};

pub fn emit(service: &ServiceDef, transport: Transport) -> TokenStream {
    let dispatcher = dispatcher_macro(service, transport);
    let client = client_macro(service, transport);
    quote! {
        #dispatcher
        #client
    }
}

// ---------------------------------------------------------------------------------------------
// Reading a Rust type as a wire scalar
// ---------------------------------------------------------------------------------------------

/// The runtime expression that turns `raw` — a `&str` expression — into the `serde_json::Value` a
/// field of type `ty` deserializes from: `Option<...>` and `Vec<...>` are read through to the
/// scalar they wrap, a `Vec` splitting `raw` on `,` and coercing each piece the same way. Used
/// identically for a path placeholder, a query parameter, a `header_in` binding and (via
/// [`encode_expr`]'s inverse reading) a `header_out` element — the two sides of one wire agreeing by
/// construction rather than by two authors agreeing.
fn decode_expr(ty: &Type, raw: &TokenStream) -> TokenStream {
    let base = option_inner(ty).unwrap_or(ty);
    if let Some(inner) = vec_inner(base) {
        let element = decode_expr(inner, &quote! { piece });
        return quote! {
            ::serde_json::Value::Array(
                (#raw).split(',').map(|piece: &str| #element).collect(),
            )
        };
    }
    match scalar_kind(base) {
        ScalarKind::Bool => quote! {
            match #raw {
                "true" => ::serde_json::Value::Bool(true),
                "false" => ::serde_json::Value::Bool(false),
                other => ::serde_json::Value::String(other.to_owned()),
            }
        },
        ScalarKind::Number => quote! {
            {
                let raw_text = #raw;
                if let Ok(as_integer) = raw_text.parse::<i64>() {
                    ::serde_json::Value::from(as_integer)
                } else if let Ok(as_float) = raw_text.parse::<f64>() {
                    match ::serde_json::Number::from_f64(as_float) {
                        Some(number) => ::serde_json::Value::Number(number),
                        None => ::serde_json::Value::String(raw_text.to_owned()),
                    }
                } else {
                    ::serde_json::Value::String(raw_text.to_owned())
                }
            }
        },
        ScalarKind::Text => quote! { ::serde_json::Value::String((#raw).to_owned()) },
    }
}

/// The runtime expression that renders `value` — an expression of any `Serialize` type — as the
/// text a path segment or a header carries. The inverse of [`decode_expr`]'s scalar case, read
/// through `serde_json::to_value` rather than a second per-type judgement, since encoding never has
/// to guess a shape the way decoding a bare string does.
fn encode_expr(value: &TokenStream) -> TokenStream {
    quote! {
        match ::serde_json::to_value(&(#value)) {
            Ok(::serde_json::Value::String(rendered)) => rendered,
            Ok(::serde_json::Value::Bool(rendered)) => rendered.to_string(),
            Ok(::serde_json::Value::Number(rendered)) => rendered.to_string(),
            Ok(rendered) => rendered.to_string(),
            Err(_unserializable) => ::std::string::String::new(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The route table
// ---------------------------------------------------------------------------------------------

/// One row an adapter iterates to register a handler: the method and path template an operation
/// answers to, and the statuses it can answer with.
fn route_type() -> TokenStream {
    quote! {
        /// One operation's method, path template and status table, for an adapter to iterate when
        /// it registers a handler per route. The path template is written exactly as `http(...)`
        /// declared it (or defaulted it) — `{field}` placeholders included — since the adapter is
        /// the one that knows its own router's own placeholder syntax.
        pub struct Route {
            error_statuses: &'static [u16],
            method: &'static str,
            ok_status: u16,
            operation: &'static str,
            path: &'static str,
        }

        impl Route {
            /// Every status a declared error can answer with.
            pub const fn error_statuses(&self) -> &'static [u16] {
                self.error_statuses
            }

            /// The HTTP method this route answers to.
            pub const fn method(&self) -> &'static str {
                self.method
            }

            /// The status a success answers with.
            pub const fn ok_status(&self) -> u16 {
                self.ok_status
            }

            /// The operation's own wire name.
            pub const fn operation(&self) -> &'static str {
                self.operation
            }

            /// The path template, `{field}` placeholders and all.
            pub const fn path(&self) -> &'static str {
                self.path
            }
        }
    }
}

fn route_table(service: &ServiceDef) -> TokenStream {
    let rows = service.operations.iter().map(|operation| {
        let shape = HttpShape::of(operation);
        let method = shape.method.name();
        let path = shape.path_template();
        let wire = &operation.wire_name;
        let ok_status = shape.ok_status;
        let error_statuses: Vec<u16> = if shape.error_status.is_empty() {
            match &operation.outcome {
                OperationOutcome::OneWay => Vec::new(),
                OperationOutcome::Reply {
                    error: _error,
                    success: _success,
                } => {
                    vec![DEFAULT_BINDING_ERROR_STATUS]
                }
            }
        } else {
            shape.error_status.iter().map(|(_, code)| *code).collect()
        };
        quote! {
            Route {
                method: #method,
                path: #path,
                operation: #wire,
                ok_status: #ok_status,
                error_statuses: &[#(#error_statuses),*],
            }
        }
    });
    quote! {
        /// Every route this service answers to, in declaration order.
        pub const ROUTES: &[Route] = &[#(#rows),*];
    }
}

// ---------------------------------------------------------------------------------------------
// The dispatcher
// ---------------------------------------------------------------------------------------------

fn dispatcher_macro(service: &ServiceDef, transport: Transport) -> TokenStream {
    let contract = &service.ident;
    let macro_name = super::dispatcher_macro_ident(service, transport);
    let placement = placement_doc(&macro_name, "http_transport", "the_contract_crate::");
    let macro_doc = format!(
        "The `{contract}` dispatcher for the `{}` transport, held as tokens rather than compiled \
         here.\n\n\
         It takes no arguments and emits bare items - `IncomingRequest`, `OutgoingResponse`, the \
         route table, `FaultHandler` and `dispatch` - so the caller supplies the module they land \
         in and two transports in one crate cannot collide. `dispatch` matches the method and \
         path itself (no server sits behind it), decodes the path, the query string, request \
         headers and the JSON body into the operation's own arguments, runs the message's own \
         `validate()`, calls the implementation behind a panic guard, and answers a status, \
         response headers and a body - the declared `ok_status` with the success type as bare \
         JSON (or as raw bytes with its content type, for an operation declaring `body = \
         \"bytes\"`; or, for `body = \"stream\"`, an undrained body source, `200` for the whole \
         body or `206` with `content-range` for a range slice), a declared error at its mapped \
         status with the error type itself as the body, or a fault the caller's own installed \
         `FaultHandler` decides the answer for. The \
         invoking crate names `serde`, `serde_json` and `tracing` in its own manifest, because \
         the items below call them.\n\n\
         {placement}",
        transport.name()
    );
    let items = dispatcher_items(service);
    quote! {
        #[doc = #macro_doc]
        #[macro_export]
        macro_rules! #macro_name {
            () => {
                #items
            };
        }
    }
}

fn dispatcher_items(service: &ServiceDef) -> TokenStream {
    let module = module_ident(service);
    // Whether `OutgoingResponse` carries a plain `Vec<u8>` body (every service until this one, and
    // every service with no streamed operation from here on — unchanged tokens either way) or the
    // `OutgoingBody` enum a streamed operation's undrained body needs: `dispatch` is one function
    // with one return type, so the choice is the whole service's rather than one operation's.
    let has_stream = service_declares_a_stream(service);
    // Whether `dispatch` takes an extra `parts` argument at all: a whole-service decision exactly
    // like `has_stream`, so a service with no multipart operation reaches for none of this.
    let has_multipart = service_declares_multipart(service);
    let incoming = incoming_request_items();
    let incoming_part = if has_multipart {
        incoming_part_type(&module)
    } else {
        TokenStream::new()
    };
    let outgoing = outgoing_response_items(has_stream, &module);
    let route = route_type();
    let routes = route_table(service);
    let fault_handler = fault_handler_trait(&module);
    // `match_path` is an arm's, and a service declaring no operation has no arm to call it from.
    let path_token = if service.operations.is_empty() {
        TokenStream::new()
    } else {
        path_token_type()
    };
    // The query reader is one bodyless field's, unbound by any path placeholder; a service with
    // none never reaches for it.
    let query_helpers = if service
        .operations
        .iter()
        .any(|operation| has_query_fields(operation, &HttpShape::of(operation)))
    {
        query_parsing_helpers()
    } else {
        TokenStream::new()
    };
    let dispatch = dispatch_fn(service, has_stream, has_multipart, &module);
    // Every type, trait and impl ahead of every function: `fault_handler` declares a trait and an
    // impl, so it sits with `incoming`/`outgoing`/`route` rather than beside `path_token` and
    // `query_helpers`, both of which are functions a strict consumer's lints would otherwise see
    // declared ahead of a type.
    quote! {
        #routes
        #incoming
        #incoming_part
        #outgoing
        #route
        #fault_handler
        #path_token
        #query_helpers
        #dispatch
    }
}

fn incoming_request_items() -> TokenStream {
    quote! {
        /// One HTTP request in plain terms: nothing here names a framework.
        ///
        /// The path and the query string arrive separately - an adapter splits them off whatever
        /// its own request type carries them as (`Uri::path()` and `Uri::query()`, for one) - and
        /// the query string is unparsed, `dispatch` reading it itself.
        pub struct IncomingRequest {
            body: Vec<u8>,
            headers: Vec<(String, String)>,
            method: String,
            path: String,
            query: String,
        }

        impl IncomingRequest {
            /// The request body, undecoded.
            pub fn body(&self) -> &[u8] {
                &self.body
            }

            /// One request header, read case-insensitively as HTTP headers are.
            pub fn header(&self, name: &str) -> Option<&str> {
                self.headers
                    .iter()
                    .find(|(carried, _)| carried.eq_ignore_ascii_case(name))
                    .map(|(_, value)| value.as_str())
            }

            /// Every header this request carried, in the order it carried them.
            pub fn headers(&self) -> &[(String, String)] {
                &self.headers
            }

            /// The method exactly as the request line carried it.
            pub fn method(&self) -> &str {
                &self.method
            }

            /// Binds one request as an adapter read it off the wire.
            pub const fn new(
                method: String,
                path: String,
                query: String,
                headers: Vec<(String, String)>,
                body: Vec<u8>,
            ) -> Self {
                Self {
                    method,
                    path,
                    query,
                    headers,
                    body,
                }
            }

            /// The path, with no query string attached.
            pub fn path(&self) -> &str {
                &self.path
            }

            /// The query string, with no leading `?` and not yet parsed.
            pub fn query(&self) -> &str {
                &self.query
            }
        }
    }
}

/// One multipart request part, already split off the wire by an adapter: text content, or a file
/// part's undrained bytes through the same seam a streamed response pulls from. Handed to
/// `dispatch` as its own argument rather than a field on `IncomingRequest` — a file part's own
/// handle has to be owned by whichever call argument it fills, and `IncomingRequest` is read by
/// shared reference throughout the rest of dispatch, path and header decoding included.
fn incoming_part_type(module: &Ident) -> TokenStream {
    quote! {
        /// One multipart request part: text content, or a file part's undrained bytes.
        pub enum IncomingPart {
            File(::std::boxed::Box<dyn $crate::#module::BodySource + Send>),
            Text(::std::string::String),
        }
    }
}

/// `OutgoingResponse` and, only where a streamed operation needs it, the `OutgoingBody` its body
/// is carried in. A service with no streamed operation keeps the plain `Vec<u8>` body — the exact
/// tokens every service answered before this kind existed — since materializing every answer into
/// bytes is exactly what a streamed body must not do.
fn outgoing_response_items(has_stream: bool, module: &Ident) -> TokenStream {
    if !has_stream {
        return quote! {
            /// One HTTP response in plain terms, for an adapter to write back however its own
            /// framework answers a request.
            pub struct OutgoingResponse {
                body: Vec<u8>,
                headers: Vec<(String, String)>,
                status: u16,
            }

            impl OutgoingResponse {
                /// The response body.
                pub fn body(&self) -> &[u8] {
                    &self.body
                }

                /// Every header this response carries, in the order they were written.
                pub fn headers(&self) -> &[(String, String)] {
                    &self.headers
                }

                /// Binds one response - what a `FaultHandler` an owner installed builds its answer
                /// with, there being no other way to fill this type's private fields from outside
                /// the module `dispatch` was placed in.
                pub const fn new(status: u16, headers: Vec<(String, String)>, body: Vec<u8>) -> Self {
                    Self {
                        status,
                        headers,
                        body,
                    }
                }

                /// The status this response answers with.
                pub const fn status(&self) -> u16 {
                    self.status
                }
            }
        };
    }
    quote! {
        /// One HTTP response body, undrained: bytes already in hand, or a source an adapter pulls
        /// from a chunk at a time on the way to the wire.
        pub enum OutgoingBody {
            Bytes(Vec<u8>),
            Stream(::std::boxed::Box<dyn $crate::#module::BodySource + Send>),
        }

        /// One HTTP response in plain terms, for an adapter to write back however its own
        /// framework answers a request.
        pub struct OutgoingResponse {
            body: OutgoingBody,
            headers: Vec<(String, String)>,
            status: u16,
        }

        impl OutgoingResponse {
            /// Every header this response carries, in the order they were written.
            pub fn headers(&self) -> &[(String, String)] {
                &self.headers
            }

            /// Takes the body, for an adapter to drain onto the wire - by value, since pulling
            /// from a boxed body source needs to own it.
            pub fn into_body(self) -> OutgoingBody {
                self.body
            }

            /// Binds one response - what a `FaultHandler` an owner installed builds its answer
            /// with, there being no other way to fill this type's private fields from outside the
            /// module `dispatch` was placed in.
            pub const fn new(status: u16, headers: Vec<(String, String)>, body: OutgoingBody) -> Self {
                Self {
                    status,
                    headers,
                    body,
                }
            }

            /// The status this response answers with.
            pub const fn status(&self) -> u16 {
                self.status
            }
        }
    }
}

/// One piece of a path template at runtime: text written exactly as declared, or a `{field}`
/// placeholder capturing up to the next `/`. Emitted only where at least one route exists to be
/// matched against — a service declaring no operation has no arm to call `match_path` from.
fn path_token_type() -> TokenStream {
    quote! {
        enum PathToken {
            Literal(&'static str),
            Placeholder,
        }

        /// Matches `path` against `template` left to right, and answers what each placeholder
        /// captured, in order - or `None` where a literal disagrees, a placeholder captures
        /// nothing, or text remains once the template is exhausted.
        fn match_path(template: &[PathToken], path: &str) -> Option<::std::vec::Vec<String>> {
            let mut rest = path;
            let mut captured = ::std::vec::Vec::new();
            for token in template {
                match token {
                    PathToken::Literal(text) => rest = rest.strip_prefix(text)?,
                    PathToken::Placeholder => {
                        let end = rest.find('/').unwrap_or(rest.len());
                        let (value, remainder) = rest.split_at(end);
                        if value.is_empty() {
                            return None;
                        }
                        captured.push(value.to_owned());
                        rest = remainder;
                    }
                }
            }
            rest.is_empty().then_some(captured)
        }
    }
}

/// The query-string reader: emitted only where at least one operation reads a query parameter, a
/// bodyless method's field the path left unbound. A service with no such field never calls either
/// function.
fn query_parsing_helpers() -> TokenStream {
    quote! {
        /// A query string, read into its keys and values. Repeated keys keep the last value; a
        /// `%XX` escape decodes to the byte it names and anything else is read verbatim.
        fn parse_query(raw: &str) -> ::std::collections::HashMap<String, String> {
            let mut parsed = ::std::collections::HashMap::new();
            for pair in raw.split('&') {
                if pair.is_empty() {
                    continue;
                }
                let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
                parsed.insert(percent_decoded(key), percent_decoded(value));
            }
            parsed
        }

        /// `%XX` escapes decoded back to bytes and reassembled as text, lossily where the result is
        /// not valid UTF-8 - a query value is a caller's own words, not this crate's to reject on
        /// that alone.
        fn percent_decoded(raw: &str) -> String {
            let bytes = raw.as_bytes();
            let mut decoded = ::std::vec::Vec::with_capacity(bytes.len());
            let mut index = 0;
            while index < bytes.len() {
                let byte = bytes[index];
                if byte == b'%' && index + 3 <= bytes.len() {
                    let hex = ::core::str::from_utf8(&bytes[index + 1..index + 3])
                        .ok()
                        .and_then(|hex| u8::from_str_radix(hex, 16).ok());
                    if let Some(value) = hex {
                        decoded.push(value);
                        index += 3;
                        continue;
                    }
                }
                decoded.push(byte);
                index += 1;
            }
            String::from_utf8_lossy(&decoded).into_owned()
        }
    }
}

/// Whether `operation` reads at least one query parameter: a bodyless method with a generated
/// field the path left unbound. Read identically by the dispatcher (to decide whether
/// [`query_parsing_helpers`] is reachable) and the client (to decide whether it ever builds one).
fn has_query_fields(operation: &OperationDef, shape: &HttpShape) -> bool {
    if shape.method.carries_a_body() {
        return false;
    }
    let OperationInputs::Generated(fields) = &operation.inputs else {
        return false;
    };
    let placeholders = shape.placeholder_names();
    fields
        .iter()
        .any(|(field, _)| !placeholders.contains(&field.to_string()))
}

fn path_token_tokens(path: &[PathSegment]) -> Vec<TokenStream> {
    path.iter()
        .map(|segment| match segment {
            PathSegment::Literal(text) => quote! { PathToken::Literal(#text) },
            PathSegment::Placeholder(_) => quote! { PathToken::Placeholder },
        })
        .collect()
}

/// The `body: <expr>` field initializer for an `OutgoingResponse` (or `OutgoingResponse`-shaped)
/// literal: wrapped in `OutgoingBody::Bytes(...)` where the service carries the enum (a streamed
/// operation is somewhere in it), or, where it does not, written as `body: <expr>` - except when
/// `body_expr` is itself the bare identifier `body`, in which case it is written as the field-init
/// shorthand `body` instead, exactly as every one of these call sites wrote it before the streamed
/// kind existed. The one seam every JSON- and bytes-body-writing call site reads through, so the
/// two shapes never drift apart by a forgotten call site, and the unwrapped path never regresses
/// into a `clippy::redundant_field_names` a byte-for-byte-unchanged expansion cannot carry.
fn body_field(has_stream: bool, body_expr: &TokenStream) -> TokenStream {
    if has_stream {
        quote! { body: OutgoingBody::Bytes(#body_expr) }
    } else if body_expr.to_string() == "body" {
        quote! { body }
    } else {
        quote! { body: #body_expr }
    }
}

/// `json_response`, the one way `dispatch` and the default `FaultHandler` both write a body.
fn response_builders(has_stream: bool) -> TokenStream {
    let ok_body = body_field(has_stream, &quote! { body });
    let err_body = body_field(has_stream, &quote! { ::std::vec::Vec::new() });
    quote! {
        /// Serializes `value` as the body, under `status` and whatever `headers` the caller
        /// already built. A value that will not serialize is answered as a fault instead - the
        /// dispatcher's own defect, not the caller's.
        fn json_response<T>(status: u16, mut headers: Vec<(String, String)>, value: &T) -> OutgoingResponse
        where
            T: ::serde::Serialize,
        {
            match ::serde_json::to_vec(value) {
                Ok(body) => {
                    headers.push(("content-type".to_owned(), "application/json".to_owned()));
                    OutgoingResponse {
                        status,
                        headers,
                        #ok_body,
                    }
                }
                Err(unserializable) => {
                    ::tracing::error!(
                        error = %unserializable,
                        "an answer would not serialize; the caller is told a defect answered instead",
                    );
                    OutgoingResponse {
                        status: 500,
                        headers: ::std::vec::Vec::new(),
                        #err_body,
                    }
                }
            }
        }
    }
}

/// The seam every fault `dispatch` meets answers through: `on_fault` decides its status, headers
/// and body. The provided default matches the fixed behavior this dispatcher answered before this
/// seam existed - 400 for a payload that failed to decode or to `validate()`, 404 for a request no
/// route answers to, 500 for a handler that panicked - each as the fault itself under
/// `json_response`.
///
/// An owner installs their own handler by implementing this trait on a type of their own and
/// passing it to `dispatch`; overriding `on_fault` replaces the default for every kind at once,
/// `fault.kind()` being how an override tells one kind from another.
fn fault_handler_trait(module: &Ident) -> TokenStream {
    quote! {
        /// Decides what one fault answers with, for a `dispatch` it was installed on.
        pub trait FaultHandler {
            /// Turns `fault` into the response `dispatch` answers a request with.
            fn on_fault(&self, fault: &$crate::#module::ServiceFault) -> OutgoingResponse {
                let status = match fault.kind() {
                    $crate::#module::ServiceFaultKind::UnknownOperation => 404,
                    $crate::#module::ServiceFaultKind::HandlerPanic => 500,
                    _ => 400,
                };
                json_response(status, ::std::vec::Vec::new(), fault)
            }
        }

        /// A `FaultHandler` that overrides nothing: every fault answers the fixed default.
        pub struct DefaultFaultHandler;

        impl FaultHandler for DefaultFaultHandler {}
    }
}

fn dispatch_fn(
    service: &ServiceDef,
    has_stream: bool,
    has_multipart: bool,
    module: &Ident,
) -> TokenStream {
    let contract = &service.ident;
    let builders = response_builders(has_stream);
    let arms = service
        .operations
        .iter()
        .map(|operation| dispatch_arm(module, operation, has_stream));
    let (guard, refusal, implementation, context) = if service.operations.is_empty() {
        (TokenStream::new(), TokenStream::new(), quote!(_), quote!(_))
    } else {
        (
            panic_guard(),
            refusal_reader(module),
            quote!(svc),
            quote!(ctx),
        )
    };
    // A `body = "multipart"` operation reads its parts out of this extra argument rather than off
    // `IncomingRequest` - a file part's own handle has to be owned by whichever call argument it
    // fills, where `request` is read by shared reference throughout the rest of dispatch, and
    // `mut` is what lets the one matching arm remove entries from it as it claims them.
    let parts_param = if has_multipart {
        quote! { mut parts: ::std::vec::Vec<(::std::string::String, IncomingPart)>, }
    } else {
        TokenStream::new()
    };
    let dispatch_doc = format!(
        "Matches `request` against every route `{contract}` declares and answers it: a declared \
         status with the success or error type as bare JSON, or a fault the installed \
         `FaultHandler` decides the answer for - by default the fixed behavior this dispatcher \
         always answered: 400 for a payload that fails to decode or to validate, 404 for a \
         request no route answers to, 500 for a handler that panicked.\n\n\
         Generic over the implementing type rather than taking `&dyn {contract}`: a trait whose \
         methods are `async` is not dyn compatible."
    );
    quote! {
        #builders
        #refusal
        #guard

        #[doc = #dispatch_doc]
        pub fn dispatch<S, Ctx, H>(
            #implementation: &S,
            #context: &Ctx,
            request: &IncomingRequest,
            #parts_param
            handler: &H,
        ) -> impl ::core::future::Future<Output = OutgoingResponse> + Send
        where
            S: $crate::#contract<Ctx> + Sync,
            Ctx: Sync,
            H: FaultHandler + Sync,
        {
            async move {
                let method = request.method();
                let path = request.path();
                #(#arms)*
                handler.on_fault(&$crate::#module::ServiceFault::unknown_operation(&format!(
                    "{method} {path}"
                )))
            }
        }
    }
}

/// One operation's arm: match the method and the path template, decode path, query, headers and
/// body into the operation's own arguments, validate, call the implementation behind the panic
/// guard, and answer.
fn dispatch_arm(module: &Ident, operation: &OperationDef, has_stream: bool) -> TokenStream {
    let shape = HttpShape::of(operation);
    let wire = &operation.wire_name;
    let method_str = shape.method.name();
    let path_tokens = path_token_tokens(&shape.path);
    let placeholder_names = shape.placeholder_names();
    let placeholder_idents: Vec<Ident> = placeholder_names
        .iter()
        .map(|name| format_ident!("{name}"))
        .collect();

    let placeholder_lets = if placeholder_idents.is_empty() {
        TokenStream::new()
    } else {
        quote! {
            let mut placeholders = captured.into_iter();
            #(let #placeholder_idents = placeholders.next().unwrap();)*
        }
    };

    let header_in_lets: TokenStream = shape
        .header_in
        .iter()
        .map(|header| header_in_let(wire, header))
        .collect();

    let multipart_part_lets: TokenStream = shape
        .multipart_parts
        .iter()
        .map(|part| multipart_part_let(module, wire, part))
        .collect();

    let (value_stmts, value_expr) = message_value(
        wire,
        operation,
        &shape,
        &placeholder_idents,
        &placeholder_names,
    );

    let message_alias = message_alias_ident(operation);
    let validator = message_validator_ident(operation);
    let decode_and_validate = quote! {
        let received: $crate::#module::#message_alias = match ::serde_json::from_value(#value_expr) {
            Ok(received) => received,
            Err(rejected) => return handler.on_fault(&refused_payload(#wire, &rejected)),
        };
        if let Err(violations) = $crate::#module::#validator(&received) {
            return handler.on_fault(&$crate::#module::ServiceFault::failed_validation(
                #wire,
                $crate::#module::violated_field(&violations),
                &$crate::#module::violation_detail(&violations),
            ));
        }
    };

    let call_args = call_arguments(operation);
    let method_ident = &operation.ident;
    let called = quote! { caught(move || svc.#method_ident(ctx #(, #call_args)*)).await };

    let answer = answer_block(wire, operation, &shape, &called, module, has_stream);
    let captured_binding = if placeholder_idents.is_empty() {
        quote! { _captured }
    } else {
        quote! { captured }
    };

    quote! {
        if method == #method_str {
            if let Some(#captured_binding) = match_path(&[#(#path_tokens),*], path) {
                #placeholder_lets
                #header_in_lets
                #multipart_part_lets
                #value_stmts
                #decode_and_validate
                #answer
            }
        }
    }
}

/// Removes and returns the multipart part named `name` from `parts` (the dispatcher's own extra
/// argument), or answers `None` where the request never carried one by that name. Each name is
/// claimed at most once - a scalar field by its own wire key, a `part` binding by its declared
/// name - so nothing here reads the same part twice.
fn take_part_expr(name: &str) -> TokenStream {
    quote! {
        parts
            .iter()
            .position(|(carried, _)| carried == #name)
            .map(|index| parts.remove(index).1)
    }
}

/// One `part("name" = parameter)` binding's own local: the named part's file handle, handed
/// through undecoded before `validate()` runs - a message field never carries one, `BodySource`
/// publishing no `Deserialize` for it to be read back through.
fn multipart_part_let(module: &Ident, wire: &str, part: &MultipartPart) -> TokenStream {
    let name = &part.name;
    let parameter = &part.parameter;
    let declared_type = &part.ty;
    let take = take_part_expr(name);
    quote! {
        let #parameter: #declared_type = match #take {
            Some(IncomingPart::File(source)) => source,
            Some(IncomingPart::Text(_)) => {
                return handler.on_fault(&$crate::#module::ServiceFault::failed_validation(
                    #wire,
                    Some(#name),
                    "expected a file part, found a text part",
                ));
            }
            None => {
                return handler.on_fault(&$crate::#module::ServiceFault::failed_validation(
                    #wire,
                    Some(#name),
                    "a required multipart part was not carried",
                ));
            }
        };
    }
}

fn header_in_let(wire: &str, header: &HeaderIn) -> TokenStream {
    let name = &header.name;
    let parameter = &header.parameter;
    let declared_type = &header.ty;
    let decode = decode_expr(declared_type, &quote! { text });
    quote! {
        let #parameter: #declared_type = {
            let source = match request.header(#name) {
                Some(text) => #decode,
                None => ::serde_json::Value::Null,
            };
            match ::serde_json::from_value(source) {
                Ok(value) => value,
                Err(rejected) => return handler.on_fault(&refused_payload(#wire, &rejected)),
            }
        };
    }
}

/// The statements and the final expression that build one operation's message as a
/// `serde_json::Value`, ready for `serde_json::from_value` into the operation's own message type.
/// The statements and final expression that build one operation's message as a
/// `serde_json::Value`, ready for `serde_json::from_value` into the operation's own message type.
/// Split by input shape into [`message_value_for_named`] and [`message_value_for_generated`],
/// which is also where each shape's own rules are documented.
fn message_value(
    wire: &str,
    operation: &OperationDef,
    shape: &HttpShape,
    placeholder_idents: &[Ident],
    placeholder_names: &[String],
) -> (TokenStream, TokenStream) {
    let bodied = shape.method.carries_a_body();
    match &operation.inputs {
        OperationInputs::Empty => {
            if bodied && !matches!(shape.body_kind, BodyKind::Multipart) {
                (TokenStream::new(), from_body_expr(wire))
            } else {
                // A multipart request's body is not itself the message - every field, if there
                // were any, would be read off a named part instead - and an operation with no
                // carried field at all (every argument claimed by `part`) has nothing to read.
                (
                    TokenStream::new(),
                    quote! { ::serde_json::Value::Object(::serde_json::Map::new()) },
                )
            }
        }
        OperationInputs::Named(named_type) => message_value_for_named(
            wire,
            named_type,
            bodied,
            placeholder_idents,
            placeholder_names,
        ),
        OperationInputs::Generated(fields) => {
            message_value_for_generated(fields, shape, placeholder_names)
        }
    }
}

/// The message value the raw request body parses to, or the fault a body that will not parse
/// answers through the installed `FaultHandler` instead.
fn from_body_expr(wire: &str) -> TokenStream {
    quote! {
        match ::serde_json::from_slice(request.body()) {
            Ok(value) => value,
            Err(rejected) => return handler.on_fault(&refused_payload(#wire, &rejected)),
        }
    }
}

/// A `OperationInputs::Named` message: the whole body where there is one and no placeholder binds
/// it; the one placeholder's own coerced value where the named type is a recognised scalar and
/// there is exactly one; otherwise an object keyed under each placeholder's own written spelling,
/// merged onto the body where the method carries one — the opaque-struct case [`message_value`]'s
/// own documentation covers.
fn message_value_for_named(
    wire: &str,
    named_type: &Type,
    bodied: bool,
    placeholder_idents: &[Ident],
    placeholder_names: &[String],
) -> (TokenStream, TokenStream) {
    if placeholder_idents.is_empty() {
        return if bodied {
            (TokenStream::new(), from_body_expr(wire))
        } else {
            (TokenStream::new(), quote! { ::serde_json::Value::Null })
        };
    }
    if placeholder_idents.len() == 1 && is_scalar_named_type(named_type) {
        // The whole message *is* this one placeholder — a body, if the method carries one at all,
        // has no second field to contribute and is left unread.
        let only = &placeholder_idents[0];
        let decode = decode_expr(named_type, &quote! { #only.as_str() });
        return (TokenStream::new(), decode);
    }
    let base = object_base(bodied);
    let inserts: TokenStream = placeholder_names
        .iter()
        .zip(placeholder_idents)
        .map(|(name, ident)| {
            quote! { object.insert(#name.to_owned(), ::serde_json::Value::String(#ident.clone())); }
        })
        .collect();
    (
        quote! { #base #inserts },
        quote! { ::serde_json::Value::Object(object) },
    )
}

/// A `OperationInputs::Generated` message: each field is path-bound (its own coerced value,
/// inserted under its wire key), query-bound (a bodyless method's field the path left unbound,
/// read out of the parsed query string), part-bound (a `body = "multipart"` method's field, read
/// out of the same-named part) or left exactly as the parsed JSON body already has it.
fn message_value_for_generated(
    fields: &[(Ident, Type)],
    shape: &HttpShape,
    placeholder_names: &[String],
) -> (TokenStream, TokenStream) {
    let bodied = shape.method.carries_a_body();
    let multipart = matches!(shape.body_kind, BodyKind::Multipart);
    let base = if multipart {
        quote! { let mut object = ::serde_json::Map::new(); }
    } else if bodied {
        object_base(true)
    } else {
        quote! {
            let query_map = parse_query(request.query());
            let mut object = ::serde_json::Map::new();
        }
    };
    let inserts: TokenStream = fields
        .iter()
        .map(|(field, ty)| {
            let field_name = field.to_string();
            let key = wire_key(field);
            if placeholder_names.contains(&field_name) {
                let ident = format_ident!("{field_name}");
                let decode = decode_expr(ty, &quote! { #ident.as_str() });
                quote! { object.insert(#key.to_owned(), #decode); }
            } else if multipart {
                let take = take_part_expr(&key);
                let decode = decode_expr(ty, &quote! { text });
                quote! {
                    object.insert(#key.to_owned(), match #take {
                        Some(IncomingPart::Text(text)) => {
                            let text = text.as_str();
                            #decode
                        }
                        Some(IncomingPart::File(_)) | None => ::serde_json::Value::Null,
                    });
                }
            } else if bodied {
                TokenStream::new()
            } else {
                let decode = decode_expr(ty, &quote! { text });
                quote! {
                    object.insert(#key.to_owned(), match query_map.get(#key).map(::std::string::String::as_str) {
                        Some(text) => #decode,
                        None => ::serde_json::Value::Null,
                    });
                }
            }
        })
        .collect();
    (
        quote! { #base #inserts },
        quote! { ::serde_json::Value::Object(object) },
    )
}

/// The object an operation's fields are inserted into: the parsed body where the method carries
/// one (falling back to an empty object where the body is not itself an object), or a fresh empty
/// object where it does not.
fn object_base(bodied: bool) -> TokenStream {
    if bodied {
        quote! {
            let mut object = match ::serde_json::from_slice(request.body()) {
                Ok(::serde_json::Value::Object(map)) => map,
                Ok(_) | Err(_) => ::serde_json::Map::new(),
            };
        }
    } else {
        quote! { let mut object = ::serde_json::Map::new(); }
    }
}

/// The status a declared error answers at, read off `error_status` where the operation named an
/// `http(...)` group, or the fixed [`DEFAULT_BINDING_ERROR_STATUS`] where it named none. The match
/// is exhaustive by construction wherever it is written at all: `error_status`'s own completeness
/// against the error type is checked unconditionally, in every build, by
/// [`crate::service_schema::support`]'s own probe.
fn error_status_expr(shape: &HttpShape, error_type: &Type) -> TokenStream {
    if shape.error_status.is_empty() {
        let fallback = DEFAULT_BINDING_ERROR_STATUS;
        quote! { #fallback }
    } else {
        let arms = shape
            .error_status
            .iter()
            .map(|(variant, code)| quote! { #error_type::#variant => #code, });
        quote! { match &declared_error { #(#arms)* } }
    }
}

/// What one operation's arm answers with, once its message has been decoded and validated: the
/// declared status and body on success, the mapped status and body on a declared error, or the
/// fixed panic fault.
fn answer_block(
    wire: &str,
    operation: &OperationDef,
    shape: &HttpShape,
    called: &TokenStream,
    module: &Ident,
    has_stream: bool,
) -> TokenStream {
    let ok_status = shape.ok_status;
    let panic_fault = quote! {
        record_panic(#wire, &panicked);
        return handler.on_fault(&$crate::#module::ServiceFault::handler_panic(#wire, &panicked));
    };
    match &operation.outcome {
        OperationOutcome::OneWay => {
            let empty_body = body_field(has_stream, &quote! { ::std::vec::Vec::new() });
            quote! {
                match #called {
                    Ok(()) => return OutgoingResponse {
                        status: #ok_status,
                        headers: ::std::vec::Vec::new(),
                        #empty_body,
                    },
                    Err(panicked) => { #panic_fault }
                }
            }
        }
        OperationOutcome::Reply { error, success } => {
            let status_expr = error_status_expr(shape, error);
            if matches!(shape.body_kind, BodyKind::Stream) {
                stream_answer_block(module, shape, called, &status_expr, &panic_fault)
            } else if matches!(shape.body_kind, BodyKind::Bytes) {
                bytes_answer_block(shape, called, &status_expr, &panic_fault, has_stream)
            } else if shape.header_out.is_empty() {
                if is_unit_type(success) {
                    let empty_body = body_field(has_stream, &quote! { ::std::vec::Vec::new() });
                    quote! {
                        match #called {
                            Ok(Ok(())) => return OutgoingResponse {
                                status: #ok_status,
                                headers: ::std::vec::Vec::new(),
                                #empty_body,
                            },
                            Ok(Err(declared_error)) => {
                                let status = #status_expr;
                                return json_response(status, ::std::vec::Vec::new(), &declared_error);
                            }
                            Err(panicked) => { #panic_fault }
                        }
                    }
                } else {
                    quote! {
                        match #called {
                            Ok(Ok(value)) => {
                                return json_response(#ok_status, ::std::vec::Vec::new(), &value);
                            }
                            Ok(Err(declared_error)) => {
                                let status = #status_expr;
                                return json_response(status, ::std::vec::Vec::new(), &declared_error);
                            }
                            Err(panicked) => { #panic_fault }
                        }
                    }
                }
            } else {
                let header_idents = header_out_idents(shape.header_out.len());
                let header_entries = header_out_entries(shape, &header_idents);
                quote! {
                    match #called {
                        Ok(Ok((value, #(#header_idents),*))) => {
                            let headers: Vec<(String, String)> = ::std::vec![#header_entries];
                            return json_response(#ok_status, headers, &value);
                        }
                        Ok(Err(declared_error)) => {
                            let status = #status_expr;
                            return json_response(status, ::std::vec::Vec::new(), &declared_error);
                        }
                        Err(panicked) => { #panic_fault }
                    }
                }
            }
        }
    }
}

/// Fresh local identifiers, one per `header_out` entry, in declaration order - the tuple pattern
/// an arm destructures a bound success into, and a client rebuilds one from.
fn header_out_idents(count: usize) -> Vec<Ident> {
    (0..count)
        .map(|index| format_ident!("header_out_{index}"))
        .collect()
}

/// One `(name, rendered)` push-ready entry per `header_out` binding, each already bound to that
/// entry's own local. The one seam every response-header-writing arm - JSON, bytes, stream - builds
/// its extra headers through, so the three cannot render one differently from another.
fn header_out_entries(shape: &HttpShape, idents: &[Ident]) -> TokenStream {
    shape
        .header_out
        .iter()
        .zip(idents)
        .map(|(name, ident)| {
            let render = encode_expr(&quote! { #ident });
            quote! { (#name.to_owned(), #render), }
        })
        .collect()
}

/// A `body = "bytes"` operation's own arm: with no declared `header_out`, `success` is
/// `(Vec<u8>, String)`; with one declared, `parse.rs`'s own `is_bytes_success_shape` has already
/// required `success` to carry one more element per entry after the content type, and each is
/// written out as a response header exactly as the JSON path's own `header_out` composition does.
fn bytes_answer_block(
    shape: &HttpShape,
    called: &TokenStream,
    status_expr: &TokenStream,
    panic_fault: &TokenStream,
    has_stream: bool,
) -> TokenStream {
    let ok_status = shape.ok_status;
    let raw_body = body_field(has_stream, &quote! { body });
    let error_arm = quote! {
        Ok(Err(declared_error)) => {
            let status = #status_expr;
            return json_response(status, ::std::vec::Vec::new(), &declared_error);
        }
        Err(panicked) => { #panic_fault }
    };
    if shape.header_out.is_empty() {
        quote! {
            match #called {
                Ok(Ok((body, content_type))) => {
                    return OutgoingResponse {
                        status: #ok_status,
                        headers: ::std::vec![("content-type".to_owned(), content_type)],
                        #raw_body,
                    };
                }
                #error_arm
            }
        }
    } else {
        let header_idents = header_out_idents(shape.header_out.len());
        let header_entries = header_out_entries(shape, &header_idents);
        quote! {
            match #called {
                Ok(Ok((body, content_type, #(#header_idents),*))) => {
                    let mut headers: Vec<(String, String)> =
                        ::std::vec![("content-type".to_owned(), content_type)];
                    headers.extend(::std::vec![#header_entries]);
                    return OutgoingResponse {
                        status: #ok_status,
                        headers,
                        #raw_body,
                    };
                }
                #error_arm
            }
        }
    }
}

/// A `body = "stream"` operation's own arm: with no declared `header_out`, the status and the one
/// header a streamed answer carries come from the variant itself - `Full` answers the declared
/// `ok_status` with no extra header, `Partial` answers `206` with `content-range` set to what the
/// handler built. With one declared, `parse.rs`'s own `is_stream_success_shape` has already
/// required `success` to wrap the answer in a tuple carrying one more element per entry, composed
/// onto both arms exactly as the JSON path's own `header_out` composition does - `content-range`
/// stands beside the declared headers on `Partial` rather than being replaced by them. Either way
/// the body is handed on undrained, in `OutgoingBody::Stream`, for an adapter to pull onto the wire.
fn stream_answer_block(
    module: &Ident,
    shape: &HttpShape,
    called: &TokenStream,
    status_expr: &TokenStream,
    panic_fault: &TokenStream,
) -> TokenStream {
    let ok_status = shape.ok_status;
    let error_arm = quote! {
        Ok(Err(declared_error)) => {
            let status = #status_expr;
            return json_response(status, ::std::vec::Vec::new(), &declared_error);
        }
        Err(panicked) => { #panic_fault }
    };
    if shape.header_out.is_empty() {
        quote! {
            match #called {
                Ok(Ok($crate::#module::StreamedAnswer::Full(source))) => {
                    return OutgoingResponse {
                        status: #ok_status,
                        headers: ::std::vec::Vec::new(),
                        body: OutgoingBody::Stream(source),
                    };
                }
                Ok(Ok($crate::#module::StreamedAnswer::Partial { source, content_range })) => {
                    return OutgoingResponse {
                        status: 206,
                        headers: ::std::vec![("content-range".to_owned(), content_range)],
                        body: OutgoingBody::Stream(source),
                    };
                }
                #error_arm
            }
        }
    } else {
        let header_idents = header_out_idents(shape.header_out.len());
        let header_entries = header_out_entries(shape, &header_idents);
        quote! {
            match #called {
                Ok(Ok(($crate::#module::StreamedAnswer::Full(source), #(#header_idents),*))) => {
                    let headers: Vec<(String, String)> = ::std::vec![#header_entries];
                    return OutgoingResponse {
                        status: #ok_status,
                        headers,
                        body: OutgoingBody::Stream(source),
                    };
                }
                Ok(Ok((
                    $crate::#module::StreamedAnswer::Partial { source, content_range },
                    #(#header_idents),*
                ))) => {
                    let mut headers: Vec<(String, String)> =
                        ::std::vec![("content-range".to_owned(), content_range)];
                    headers.extend(::std::vec![#header_entries]);
                    return OutgoingResponse {
                        status: 206,
                        headers,
                        body: OutgoingBody::Stream(source),
                    };
                }
                #error_arm
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The client
// ---------------------------------------------------------------------------------------------

fn client_macro(service: &ServiceDef, transport: Transport) -> TokenStream {
    let contract = &service.ident;
    let client = format_ident!("{contract}Client", span = contract.span());
    let published = super::client_macro_ident(service, transport);
    let generated = Generated::of(module_ident(service));
    let placement = placement_doc(&published, "http_client", "the_contract_crate::");
    let macro_doc = format!(
        "The `{contract}` client for the `{}` transport, held as tokens rather than compiled \
         here.\n\n\
         It takes no arguments and emits bare items - the transport seam, the client type and one \
         method per operation - so the module they land in is the invoking crate's to name. Every \
         method builds and validates the operation's own message first, then fills the declared \
         path template, query string and `header_in` headers from it, sends it over `Transport`, \
         and decodes the answer by status: the declared `ok_status` into the success type (or the \
         success tuple, `header_out` elements read back from response headers; or, for an \
         operation declaring `body = \"bytes\"`, the response bytes and their `content-type`; or, \
         for `body = \"stream\"`, a `StreamedAnswer` pulling from the response body - `Full` at \
         `200`, `Partial` with `content-range` at `206`), a \
         mapped status into the declared error, and anything else - including a fixed fault \
         status this crate did not expect at that code - into a fault.\n\n\
         The invoking crate names `serde` and `serde_json` in its own manifest. It names no \
         `tracing`: nothing here catches a panic, so nothing here has anything to write down.\n\n\
         {placement}",
        transport.name()
    );
    // Whether `IncomingResponse` carries a plain `Vec<u8>` body or the `IncomingBody` enum a
    // streamed operation's undrained body needs - the same whole-service choice
    // `dispatcher_items` makes for `OutgoingResponse`, so a mixed service's dispatcher and client
    // agree about the shape a streamed answer travels in.
    let has_stream = service_declares_a_stream(service);
    let has_multipart = service_declares_multipart(service);
    let seam = transport_trait(has_stream, has_multipart, &generated.module);
    let declares_a_reply = !service.operations.is_empty();
    let fault_mirror_types = if declares_a_reply {
        client_fault_mirror_types(&generated)
    } else {
        TokenStream::new()
    };
    let fault_mirror_fn = if declares_a_reply {
        client_fault_mirror_fn(&generated)
    } else {
        TokenStream::new()
    };
    // `percent_encoded` renders a path segment or a query value; a service with neither never
    // calls it.
    let needs_percent_encode = service.operations.iter().any(|operation| {
        let shape = HttpShape::of(operation);
        !shape.placeholder_names().is_empty() || has_query_fields(operation, &shape)
    });
    let percent_encode_fn = if needs_percent_encode {
        percent_encode_helper()
    } else {
        TokenStream::new()
    };
    let methods = service
        .operations
        .iter()
        .map(|operation| client_method(operation, &generated, has_multipart));
    let client_doc = format!(
        "A `{contract}` caller over `http_rest`.\n\n\
         Every operation on the trait has a method here, taking that operation's arguments and \
         nothing else."
    );
    quote! {
        #[doc = #macro_doc]
        #[macro_export]
        macro_rules! #published {
            () => {
                #seam
                #fault_mirror_types

                #[doc = #client_doc]
                pub struct #client<T: Transport> {
                    transport: T,
                }

                impl<T: Transport> #client<T> {
                    /// Binds a client to a transport.
                    pub const fn new(transport: T) -> Self {
                        Self { transport }
                    }

                    /// The transport this client was bound to.
                    pub const fn transport(&self) -> &T {
                        &self.transport
                    }
                }

                impl<T: Transport + Sync> #client<T> {
                    #(#methods)*
                }

                #fault_mirror_fn
                #percent_encode_fn
            };
        }
    }
}

/// `OutgoingRequest`, `IncomingResponse` and `Transport`. `IncomingResponse` carries a plain
/// `Vec<u8>` body (the exact tokens every service answered before the streamed kind existed) unless
/// the service declares a streamed operation, in which case its body is the `IncomingBody` enum a
/// streamed answer's undrained source needs. `OutgoingRequest` carries `parts` only where the
/// service declares a multipart operation - a `body = "bytes"`/`"json"`/`"stream"` method still
/// carries its content as `body`, unaffected either way.
fn transport_trait(has_stream: bool, has_multipart: bool, module: &Ident) -> TokenStream {
    let outgoing_request = outgoing_request_items(has_multipart, module);
    if has_stream {
        streamed_transport_items(&outgoing_request, module)
    } else {
        plain_transport_items(&outgoing_request)
    }
}

fn outgoing_request_items(has_multipart: bool, module: &Ident) -> TokenStream {
    let outgoing_part = if has_multipart {
        quote! {
            /// One outgoing multipart part: text content, or a file part's own undrained handle.
            pub enum OutgoingPart {
                File(::std::boxed::Box<dyn $crate::#module::BodySource + Send>),
                Text(::std::string::String),
            }
        }
    } else {
        TokenStream::new()
    };
    let parts_field = if has_multipart {
        quote! { parts: Vec<(String, OutgoingPart)>, }
    } else {
        TokenStream::new()
    };
    let parts_accessor = if has_multipart {
        quote! {
            /// Takes every part a `body = "multipart"` method built - by value, since a file
            /// part's own handle has to be owned by whichever adapter finally sends it. Empty
            /// for any other body kind.
            pub fn into_parts(self) -> Vec<(String, OutgoingPart)> {
                self.parts
            }
        }
    } else {
        TokenStream::new()
    };
    quote! {
        #outgoing_part

        /// One outgoing HTTP request, in plain terms: nothing here names a framework.
        pub struct OutgoingRequest {
            body: Vec<u8>,
            headers: Vec<(String, String)>,
            method: String,
            #parts_field
            path: String,
            query: String,
        }

        impl OutgoingRequest {
            /// The request body; empty for a method that carries none, and for a `body =
            /// "multipart"` method, whose content is [`into_parts`](Self::into_parts) instead.
            pub fn body(&self) -> &[u8] {
                &self.body
            }

            /// Every header this request carries.
            pub fn headers(&self) -> &[(String, String)] {
                &self.headers
            }

            #parts_accessor

            /// The method this request answers to.
            pub fn method(&self) -> &str {
                &self.method
            }

            /// The path, placeholders already filled in.
            pub fn path(&self) -> &str {
                &self.path
            }

            /// The query string, with no leading `?`; empty where the operation has none.
            pub fn query(&self) -> &str {
                &self.query
            }
        }
    }
}

/// `IncomingResponse` and `Transport` for a service with no streamed operation - the exact tokens
/// every service answered before the streamed kind existed.
fn plain_transport_items(outgoing_request: &TokenStream) -> TokenStream {
    quote! {
        #outgoing_request

        /// One HTTP response as the seam read it back.
        pub struct IncomingResponse {
            body: Vec<u8>,
            headers: Vec<(String, String)>,
            status: u16,
        }

        impl IncomingResponse {
            /// The response body.
            pub fn body(&self) -> &[u8] {
                &self.body
            }

            /// One response header, read case-insensitively as HTTP headers are.
            pub fn header(&self, name: &str) -> Option<&str> {
                self.headers
                    .iter()
                    .find(|(carried, _)| carried.eq_ignore_ascii_case(name))
                    .map(|(_, value)| value.as_str())
            }

            /// Binds one response as the seam implementation read it off the wire.
            pub const fn new(status: u16, headers: Vec<(String, String)>, body: Vec<u8>) -> Self {
                Self {
                    status,
                    headers,
                    body,
                }
            }

            /// The status this response answered with.
            pub const fn status(&self) -> u16 {
                self.status
            }
        }

        /// What binds a client to a real HTTP stack: one method, sending a whole request and
        /// answering with a whole response or with what stopped it in words.
        pub trait Transport {
            /// Sends `request` and answers with the response, or `Err` with what stopped it in
            /// words if the call never landed.
            fn send(
                &self,
                request: OutgoingRequest,
            ) -> impl ::core::future::Future<Output = Result<IncomingResponse, String>> + Send;
        }
    }
}

/// `IncomingResponse` and `Transport` for a service carrying a streamed operation: the body is
/// `IncomingBody`, bytes already in hand or a source pulled from the wire a chunk at a time.
fn streamed_transport_items(outgoing_request: &TokenStream, module: &Ident) -> TokenStream {
    quote! {
        #outgoing_request

        /// One HTTP response body, as the seam read it back: bytes already in hand, or a source a
        /// client pulls from a chunk at a time rather than buffering the whole answer first.
        pub enum IncomingBody {
            Bytes(Vec<u8>),
            Stream(::std::boxed::Box<dyn $crate::#module::BodySource + Send>),
        }

        /// One HTTP response as the seam read it back.
        pub struct IncomingResponse {
            body: IncomingBody,
            headers: Vec<(String, String)>,
            status: u16,
        }

        impl IncomingResponse {
            /// The response body's bytes - empty where the seam answered a `Stream` instead, which
            /// only a streamed operation's own decode ever reaches for, through
            /// [`into_body`](Self::into_body).
            pub fn body(&self) -> &[u8] {
                match &self.body {
                    IncomingBody::Bytes(bytes) => bytes,
                    IncomingBody::Stream(_source) => &[],
                }
            }

            /// One response header, read case-insensitively as HTTP headers are.
            pub fn header(&self, name: &str) -> Option<&str> {
                self.headers
                    .iter()
                    .find(|(carried, _)| carried.eq_ignore_ascii_case(name))
                    .map(|(_, value)| value.as_str())
            }

            /// Takes the body, for a streamed operation's own decode to read incrementally -
            /// by value, since pulling from a boxed body source needs to own it.
            pub fn into_body(self) -> IncomingBody {
                self.body
            }

            /// Binds one response as the seam implementation read it off the wire.
            pub const fn new(status: u16, headers: Vec<(String, String)>, body: IncomingBody) -> Self {
                Self {
                    status,
                    headers,
                    body,
                }
            }

            /// The status this response answered with.
            pub const fn status(&self) -> u16 {
                self.status
            }
        }

        /// What binds a client to a real HTTP stack: one method, sending a whole request and
        /// answering with a whole response or with what stopped it in words.
        pub trait Transport {
            /// Sends `request` and answers with the response, or `Err` with what stopped it in
            /// words if the call never landed.
            fn send(
                &self,
                request: OutgoingRequest,
            ) -> impl ::core::future::Future<Output = Result<IncomingResponse, String>> + Send;
        }
    }
}

/// The private mirror a fixed fault is read back through: a `ServiceFault` never derives
/// `Deserialize` of its own, so this is what stands in for one on the way back into a fault minted
/// through the service's own constructors.
fn client_fault_mirror_types(generated: &Generated) -> TokenStream {
    let Generated {
        fault,
        call_error: _call_error,
        module: _module,
    } = generated;
    quote! {
        #[derive(::serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct FaultOnTheWire {
            detail: String,
            field: Option<String>,
            kind: FaultKindOnTheWire,
            operation: String,
        }

        #[derive(::serde::Deserialize)]
        #[serde(rename_all = "kebab-case")]
        enum FaultKindOnTheWire {
            FailedValidation,
            HandlerPanic,
            TransportFailure,
            UndeserializablePayload,
            UnknownOperation,
        }

        impl FaultOnTheWire {
            fn into_fault(self) -> #fault {
                match self.kind {
                    FaultKindOnTheWire::FailedValidation => #fault::failed_validation(
                        &self.operation,
                        self.field.as_deref(),
                        &self.detail,
                    ),
                    FaultKindOnTheWire::HandlerPanic => {
                        #fault::handler_panic(&self.operation, &self.detail)
                    }
                    FaultKindOnTheWire::TransportFailure => {
                        #fault::transport_failure(&self.operation, &self.detail)
                    }
                    FaultKindOnTheWire::UndeserializablePayload => {
                        #fault::undeserializable_payload(&self.operation, &self.detail)
                    }
                    FaultKindOnTheWire::UnknownOperation => {
                        #fault::unknown_operation(&self.operation)
                    }
                }
            }
        }
    }
}

/// Reads a fixed-fault status back into a fault, from whatever the body holds. A body that will
/// not read as the fixed fault shape is itself a defect answered as one. A function rather than
/// part of [`client_fault_mirror_types`]'s own `impl`, so every function the macro emits still
/// groups after every type and every impl.
fn client_fault_mirror_fn(generated: &Generated) -> TokenStream {
    let Generated {
        fault,
        call_error: _call_error,
        module: _module,
    } = generated;
    quote! {
        fn fault_from_body(operation: &str, body: &[u8]) -> #fault {
            match ::serde_json::from_slice::<FaultOnTheWire>(body) {
                Ok(mirrored) => mirrored.into_fault(),
                Err(rejected) => {
                    #fault::undeserializable_payload(operation, &rejected.to_string())
                }
            }
        }
    }
}

/// Percent-encodes a path segment or a query value's reserved bytes, so a value carrying `/`, `?`,
/// `&`, `=`, `%` or a space travels as one segment or one value rather than reopening the template.
fn percent_encode_helper() -> TokenStream {
    quote! {
        fn percent_encoded(raw: &str) -> String {
            let mut encoded = String::with_capacity(raw.len());
            for byte in raw.bytes() {
                match byte {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                        encoded.push(byte as char);
                    }
                    other => encoded.push_str(&format!("%{other:02X}")),
                }
            }
            encoded
        }
    }
}

fn client_method(
    operation: &OperationDef,
    generated: &Generated,
    has_multipart: bool,
) -> TokenStream {
    let Generated {
        call_error,
        fault,
        module,
    } = generated;
    let shape = HttpShape::of(operation);
    let wire = &operation.wire_name;
    let named = &operation.ident;
    let validator = message_validator_ident(operation);
    let (mut taken, packed) = call_message(operation, module);
    // `header_in` and `part` each claim their own argument out of the message `call_message`
    // builds, so the signature it returns carries neither; the caller still has to supply them,
    // one ordinary parameter per binding, in the same order the dispatcher reads them back out
    // of - `header_in` first, then `part`.
    taken.extend(shape.header_in.iter().map(|header| {
        let parameter = &header.parameter;
        let ty = &header.ty;
        quote! { #parameter: #ty }
    }));
    taken.extend(shape.multipart_parts.iter().map(|part| {
        let parameter = &part.parameter;
        let ty = &part.ty;
        quote! { #parameter: #ty }
    }));
    let refusal = outbound_refusal(operation, generated);

    let path_build = path_build_stmts(operation, &shape);
    let query_build = query_build_stmts(operation, &shape);
    let headers_build = header_in_build_stmts(&shape);
    let is_multipart = matches!(shape.body_kind, BodyKind::Multipart);
    let body_build = if is_multipart || !shape.method.carries_a_body() {
        quote! { let body = ::std::vec::Vec::new(); }
    } else {
        quote! { let body = ::serde_json::to_vec(&sending).unwrap_or_default(); }
    };
    let parts_build = if has_multipart {
        multipart_parts_build_stmt(operation, &shape)
    } else {
        TokenStream::new()
    };
    let parts_field = if has_multipart {
        quote! { parts, }
    } else {
        TokenStream::new()
    };
    let method_str = shape.method.name();

    let (transport_failure, decode) = match &operation.outcome {
        OperationOutcome::OneWay => (
            quote! { Err(#fault::transport_failure(#wire, &uncarried)) },
            one_way_decode(wire, &shape, fault),
        ),
        OperationOutcome::Reply { error, success } => (
            quote! { Err(#call_error::Fault(#fault::transport_failure(#wire, &uncarried))) },
            reply_decode(wire, &shape, call_error, fault, error, success, module),
        ),
    };

    let answers = answers(operation, generated);
    let doc = format!(
        " Calls `{wire}` over `{method_str} {}`.\n\n\
         # Errors\n\n\
         See [`CallError`]({module}::CallError) - the error `{named}` declared, or a fault from \
         the remote, from this client's own outbound validation, or from the transport.",
        shape.path_template()
    );
    quote! {
        #[doc = #doc]
        pub fn #named(
            &self #(, #taken)*
        ) -> impl ::core::future::Future<Output = #answers> + Send {
            async move {
                #packed
                if let Err(violations) = $crate::#module::#validator(&sending) {
                    #refusal
                }
                #path_build
                #query_build
                #headers_build
                #body_build
                #parts_build
                let request = OutgoingRequest {
                    method: #method_str.to_owned(),
                    path,
                    query,
                    headers,
                    body,
                    #parts_field
                };
                let response = match self.transport.send(request).await {
                    Ok(response) => response,
                    Err(uncarried) => return #transport_failure,
                };
                #decode
            }
        }
    }
}

/// The client's own placeholder value: a Generated field is read off `sending` by name; a Named
/// message with several placeholders is read the same way, under the same documented requirement
/// that its fields are visible under those names; a Named message answering to exactly one
/// placeholder *is* the value.
fn client_placeholder_value(operation: &OperationDef, placeholder: &str) -> TokenStream {
    match &operation.inputs {
        // A path placeholder on an `Empty` input is refused at parse time - there is no field for
        // one to bind - so this arm is never reached by a program that compiles.
        OperationInputs::Empty => quote! { () },
        OperationInputs::Generated(_) => {
            let ident = format_ident!("{placeholder}");
            quote! { sending.#ident }
        }
        OperationInputs::Named(_) => {
            let shape = HttpShape::of(operation);
            if shape.placeholder_names().len() == 1 {
                quote! { sending }
            } else {
                let ident = format_ident!("{placeholder}");
                quote! { sending.#ident }
            }
        }
    }
}

fn path_build_stmts(operation: &OperationDef, shape: &HttpShape) -> TokenStream {
    let pushes: TokenStream = shape
        .path
        .iter()
        .map(|segment| match segment {
            PathSegment::Literal(text) => quote! { path.push_str(#text); },
            PathSegment::Placeholder(name) => {
                let value = client_placeholder_value(operation, name);
                let rendered = encode_expr(&quote! { #value });
                quote! { path.push_str(&percent_encoded(&(#rendered))); }
            }
        })
        .collect();
    quote! {
        let mut path = String::new();
        #pushes
    }
}

fn query_build_stmts(operation: &OperationDef, shape: &HttpShape) -> TokenStream {
    let OperationInputs::Generated(fields) = &operation.inputs else {
        return quote! { let query = String::new(); };
    };
    if shape.method.carries_a_body() {
        return quote! { let query = String::new(); };
    }
    let placeholders = shape.placeholder_names();
    let field_pushes: Vec<TokenStream> = fields
        .iter()
        .filter_map(|(field, ty)| {
            let field_name = field.to_string();
            if placeholders.contains(&field_name) {
                return None;
            }
            let key = wire_key(field);
            // A bodyless method's own placeholder_refusals already guarantees every field here is
            // `Option<...>` — a required field with nowhere else to go must be path-bound — so
            // there is no third, unconditional-push case to write.
            Some(vec_inner(option_inner(ty).unwrap_or(ty)).map_or_else(
                || {
                    let rendered = encode_expr(&quote! { value });
                    quote! {
                        if let Some(value) = &sending.#field {
                            query_parts.push(format!("{}={}", #key, percent_encoded(&#rendered)));
                        }
                    }
                },
                |_inner| {
                    let rendered = encode_expr(&quote! { element });
                    quote! {
                        if let Some(values) = &sending.#field {
                            let joined = values
                                .iter()
                                .map(|element| #rendered)
                                .collect::<Vec<String>>()
                                .join(",");
                            query_parts.push(format!("{}={}", #key, percent_encoded(&joined)));
                        }
                    }
                },
            ))
        })
        .collect();
    if field_pushes.is_empty() {
        return quote! { let query = String::new(); };
    }
    let pushes: TokenStream = field_pushes.into_iter().collect();
    quote! {
        let query = {
            let mut query_parts: Vec<String> = ::std::vec::Vec::new();
            #pushes
            query_parts.join("&")
        };
    }
}

/// Builds the outgoing header list, one entry per `header_in` binding — except an `Option<T>`
/// binding holding `None`, which pushes nothing at all rather than the text `encode_expr` would
/// otherwise render for a JSON `null` (`"null"`, indistinguishable on the wire from a header a
/// caller actually meant to send). Mirrors [`query_build_stmts`]'s own `Option` handling: an absent
/// value is a header the request never carries, not one carrying a placeholder word.
fn header_in_build_stmts(shape: &HttpShape) -> TokenStream {
    if shape.header_in.is_empty() {
        return quote! { let headers: Vec<(String, String)> = ::std::vec::Vec::new(); };
    }
    let pushes: TokenStream = shape
        .header_in
        .iter()
        .map(|header| {
            let name = &header.name;
            let parameter = &header.parameter;
            if option_inner(&header.ty).is_some() {
                let rendered = encode_expr(&quote! { value });
                quote! {
                    if let Some(value) = &#parameter {
                        headers.push((#name.to_owned(), #rendered));
                    }
                }
            } else {
                let rendered = encode_expr(&quote! { #parameter });
                quote! { headers.push((#name.to_owned(), #rendered)); }
            }
        })
        .collect();
    quote! {
        let mut headers: Vec<(String, String)> = ::std::vec::Vec::new();
        #pushes
    }
}

/// One `body = "multipart"` field's own text-part push: an `Option<T>` is omitted entirely when
/// absent (the same convention [`header_in_build_stmts`] uses), a `Vec<T>` joins each element's
/// own rendered text with a comma (mirroring `decode_expr`'s reverse reading on the dispatcher
/// side), and anything else renders through [`encode_expr`] directly.
fn multipart_field_push(field: &Ident, ty: &Type) -> TokenStream {
    let key = wire_key(field);
    let inner = option_inner(ty).unwrap_or(ty);
    let push = vec_inner(inner).map_or_else(
        || {
            let rendered = encode_expr(&quote! { value });
            quote! { parts.push((#key.to_owned(), OutgoingPart::Text(#rendered))); }
        },
        |_element| {
            let rendered = encode_expr(&quote! { element });
            quote! {
                let joined = value
                    .iter()
                    .map(|element| #rendered)
                    .collect::<::std::vec::Vec<String>>()
                    .join(",");
                parts.push((#key.to_owned(), OutgoingPart::Text(joined)));
            }
        },
    );
    if option_inner(ty).is_some() {
        quote! {
            if let Some(value) = &sending.#field {
                #push
            }
        }
    } else {
        quote! {
            let value = &sending.#field;
            #push
        }
    }
}

/// The `parts` a `body = "multipart"` method sends: one text entry per carried `Generated` field
/// (under its own wire key), then one file entry per declared `part` binding (under its own
/// declared name, its handle taken from the client's own extra argument) - the body pair the
/// dispatcher's own decode reads back the same way. Every other body kind sends an empty `parts`
/// list, `body` carrying its content instead.
fn multipart_parts_build_stmt(operation: &OperationDef, shape: &HttpShape) -> TokenStream {
    if !matches!(shape.body_kind, BodyKind::Multipart) {
        return quote! {
            let parts: ::std::vec::Vec<(::std::string::String, OutgoingPart)> = ::std::vec::Vec::new();
        };
    }
    let placeholder_names = shape.placeholder_names();
    let field_pushes: TokenStream = if let OperationInputs::Generated(fields) = &operation.inputs {
        fields
            .iter()
            .filter(|(field, _)| !placeholder_names.contains(&field.to_string()))
            .map(|(field, ty)| multipart_field_push(field, ty))
            .collect()
    } else {
        TokenStream::new()
    };
    let part_pushes: TokenStream = shape
        .multipart_parts
        .iter()
        .map(|part| {
            let name = &part.name;
            let parameter = &part.parameter;
            quote! { parts.push((#name.to_owned(), OutgoingPart::File(#parameter))); }
        })
        .collect();
    quote! {
        let mut parts: ::std::vec::Vec<(::std::string::String, OutgoingPart)> = ::std::vec::Vec::new();
        #field_pushes
        #part_pushes
    }
}

fn client_error_condition(shape: &HttpShape) -> TokenStream {
    if shape.error_status.is_empty() {
        let fallback = DEFAULT_BINDING_ERROR_STATUS;
        return quote! { status == #fallback };
    }
    let mut condition = TokenStream::new();
    for (index, (_, code)) in shape.error_status.iter().enumerate() {
        if index > 0 {
            condition.extend(quote! { || });
        }
        condition.extend(quote! { status == #code });
    }
    condition
}

/// The client's own `header_out` read-back: one `let` per declared entry, decoding the response
/// header named for it into the type `elements` carries at the same position - `elements` being
/// the full success tuple, `body_elements` the count that ride the body itself (the JSON path's
/// bare value, or the bytes pair) rather than a header. The one seam every reply-decoding arm
/// builds its header reads through, `idents` already named by [`header_out_idents`].
fn header_out_read_lets(
    wire: &str,
    shape: &HttpShape,
    call_error: &TokenStream,
    fault: &TokenStream,
    idents: &[Ident],
    elements: &[&Type],
    body_elements: usize,
) -> TokenStream {
    shape
        .header_out
        .iter()
        .zip(idents)
        .zip(elements.iter().skip(body_elements))
        .map(|((name, ident), element_ty)| {
            let decode = decode_expr(element_ty, &quote! { text });
            quote! {
                let #ident: #element_ty = match response.header(#name) {
                    Some(text) => match ::serde_json::from_value(#decode) {
                        Ok(value) => value,
                        Err(_rejected) => return Err(#call_error::Fault(
                            #fault::undeserializable_payload(
                                #wire,
                                "a response header did not match its declared type",
                            ),
                        )),
                    },
                    None => return Err(#call_error::Fault(#fault::undeserializable_payload(
                        #wire,
                        "a declared response header was missing",
                    ))),
                };
            }
        })
        .collect()
}

fn one_way_decode(wire: &str, shape: &HttpShape, fault: &TokenStream) -> TokenStream {
    let ok_status = shape.ok_status;
    quote! {
        let status = response.status();
        if status == #ok_status {
            return Ok(());
        }
        if matches!(status, 400 | 404 | 500) {
            return Err(fault_from_body(#wire, response.body()));
        }
        Err(#fault::undeserializable_payload(
            #wire,
            &format!("an unexpected status ({status}) answered"),
        ))
    }
}

/// What one operation's client method answers once the response has come back: the declared status
/// into the success type (or the success tuple, `header_out` elements read back from response
/// headers), a mapped status into the declared error, a fixed-fault status into a fault, and
/// anything else into a fault naming the status this crate did not expect.
fn reply_decode(
    wire: &str,
    shape: &HttpShape,
    call_error: &TokenStream,
    fault: &TokenStream,
    error: &Type,
    success: &Type,
    module: &Ident,
) -> TokenStream {
    if matches!(shape.body_kind, BodyKind::Stream) {
        return stream_reply_decode(wire, shape, call_error, fault, error, success, module);
    }
    let ok_status = shape.ok_status;
    let error_condition = client_error_condition(shape);
    let success_return = if matches!(shape.body_kind, BodyKind::Bytes) {
        // The dispatcher writes the bytes bare and the content type as a response header; the
        // client reads both straight back, no `serde_json` involved on the success path.
        if shape.header_out.is_empty() {
            quote! {
                return Ok((
                    response.body().to_vec(),
                    response.header("content-type").unwrap_or_default().to_owned(),
                ));
            }
        } else {
            // `is_bytes_success_shape` (parse.rs) already requires `success` to carry one more
            // element per declared `header_out` entry after the bytes and their content type, so
            // the lookups below never miss.
            let elements: Vec<&Type> = tuple_elements(success).into_iter().flatten().collect();
            let header_idents = header_out_idents(shape.header_out.len());
            let header_lets =
                header_out_read_lets(wire, shape, call_error, fault, &header_idents, &elements, 2);
            quote! {
                #header_lets
                return Ok((
                    response.body().to_vec(),
                    response.header("content-type").unwrap_or_default().to_owned(),
                    #(#header_idents),*
                ));
            }
        }
    } else if shape.header_out.is_empty() {
        if is_unit_type(success) {
            quote! { return Ok(()); }
        } else {
            quote! {
                return match ::serde_json::from_slice::<#success>(response.body()) {
                    Ok(value) => Ok(value),
                    Err(rejected) => Err(#call_error::Fault(
                        #fault::undeserializable_payload(#wire, &rejected.to_string()),
                    )),
                };
            }
        }
    } else {
        // `header_out`'s own arity check guarantees `success` is a tuple of exactly this many
        // elements, so the lookups below never miss.
        let elements: Vec<&Type> = tuple_elements(success).into_iter().flatten().collect();
        let first = elements.first().copied().unwrap_or(success);
        let header_idents = header_out_idents(shape.header_out.len());
        let header_lets =
            header_out_read_lets(wire, shape, call_error, fault, &header_idents, &elements, 1);
        quote! {
            return match ::serde_json::from_slice::<#first>(response.body()) {
                Ok(value) => {
                    #header_lets
                    Ok((value, #(#header_idents),*))
                }
                Err(rejected) => Err(#call_error::Fault(
                    #fault::undeserializable_payload(#wire, &rejected.to_string()),
                )),
            };
        }
    };
    quote! {
        let status = response.status();
        if status == #ok_status {
            #success_return
        }
        if #error_condition {
            return match ::serde_json::from_slice::<#error>(response.body()) {
                Ok(declared) => Err(#call_error::Operation(declared)),
                Err(rejected) => Err(#call_error::Fault(
                    #fault::undeserializable_payload(#wire, &rejected.to_string()),
                )),
            };
        }
        if matches!(status, 400 | 404 | 500) {
            return Err(#call_error::Fault(fault_from_body(#wire, response.body())));
        }
        Err(#call_error::Fault(#fault::undeserializable_payload(
            #wire,
            &format!("an unexpected status ({status}) answered"),
        )))
    }
}

/// A `body = "stream"` operation's own decode: `206` answers `StreamedAnswer::Partial` with
/// `content-range` read back before the body is taken; the declared `ok_status` (`200` by default)
/// answers `StreamedAnswer::Full`; everything else falls through the same declared-error,
/// fixed-fault and unexpected-status ladder every other kind answers through. A response the seam
/// already buffered still satisfies `BodySource` once wrapped in a `Cursor` - the same blanket
/// `Read` impl a real chunked reader relies on, so the client answers a body source either way.
///
/// With a declared `header_out`, `is_stream_success_shape` (parse.rs) has already required
/// `success` to wrap the answer in a tuple carrying one more element per entry; each is read back
/// off its own response header before the body is taken - `into_body` consumes `response`, so
/// nothing can be read off it afterward - and composed onto both `Full` and `Partial` exactly as
/// the JSON and bytes kinds compose theirs.
fn stream_reply_decode(
    wire: &str,
    shape: &HttpShape,
    call_error: &TokenStream,
    fault: &TokenStream,
    error: &Type,
    success: &Type,
    module: &Ident,
) -> TokenStream {
    let ok_status = shape.ok_status;
    let error_condition = client_error_condition(shape);
    let tail = quote! {
        if #error_condition {
            return match ::serde_json::from_slice::<#error>(response.body()) {
                Ok(declared) => Err(#call_error::Operation(declared)),
                Err(rejected) => Err(#call_error::Fault(
                    #fault::undeserializable_payload(#wire, &rejected.to_string()),
                )),
            };
        }
        if matches!(status, 400 | 404 | 500) {
            return Err(#call_error::Fault(fault_from_body(#wire, response.body())));
        }
        Err(#call_error::Fault(#fault::undeserializable_payload(
            #wire,
            &format!("an unexpected status ({status}) answered"),
        )))
    };
    if shape.header_out.is_empty() {
        quote! {
            let status = response.status();
            if status == 206 {
                let content_range = response.header("content-range").unwrap_or_default().to_owned();
                let source: ::std::boxed::Box<dyn $crate::#module::BodySource + Send> =
                    match response.into_body() {
                        IncomingBody::Bytes(bytes) => ::std::boxed::Box::new(::std::io::Cursor::new(bytes)),
                        IncomingBody::Stream(source) => source,
                    };
                return Ok($crate::#module::StreamedAnswer::Partial { source, content_range });
            }
            if status == #ok_status {
                let source: ::std::boxed::Box<dyn $crate::#module::BodySource + Send> =
                    match response.into_body() {
                        IncomingBody::Bytes(bytes) => ::std::boxed::Box::new(::std::io::Cursor::new(bytes)),
                        IncomingBody::Stream(source) => source,
                    };
                return Ok($crate::#module::StreamedAnswer::Full(source));
            }
            #tail
        }
    } else {
        let elements: Vec<&Type> = tuple_elements(success).into_iter().flatten().collect();
        let header_idents = header_out_idents(shape.header_out.len());
        let header_lets =
            header_out_read_lets(wire, shape, call_error, fault, &header_idents, &elements, 1);
        quote! {
            let status = response.status();
            if status == 206 {
                let content_range = response.header("content-range").unwrap_or_default().to_owned();
                #header_lets
                let source: ::std::boxed::Box<dyn $crate::#module::BodySource + Send> =
                    match response.into_body() {
                        IncomingBody::Bytes(bytes) => ::std::boxed::Box::new(::std::io::Cursor::new(bytes)),
                        IncomingBody::Stream(source) => source,
                    };
                return Ok((
                    $crate::#module::StreamedAnswer::Partial { source, content_range },
                    #(#header_idents),*
                ));
            }
            if status == #ok_status {
                #header_lets
                let source: ::std::boxed::Box<dyn $crate::#module::BodySource + Send> =
                    match response.into_body() {
                        IncomingBody::Bytes(bytes) => ::std::boxed::Box::new(::std::io::Cursor::new(bytes)),
                        IncomingBody::Stream(source) => source,
                    };
                return Ok(($crate::#module::StreamedAnswer::Full(source), #(#header_idents),*));
            }
            #tail
        }
    }
}
