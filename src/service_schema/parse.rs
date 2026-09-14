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
//!   is declared from, or nothing, which gets an empty message. Where the macro declares one,
//!   [`OperationDef::generated_message_ident`] is what it is called, derived here rather than at
//!   each emitter so the messages, the dispatcher and the client cannot disagree about the name.
//! - **What it answers with.** [`OperationOutcome::Reply`] carries the two declared arms, or
//!   [`OperationOutcome::OneWay`] says there is no reply to carry.
//! - **What it answers as HTTP.** [`OperationDef::http`] carries what
//!   `#[service_schema_op(http(...))]` declared — method, path, the status table, and the header
//!   bindings — or `None` for an operation that named no group, which defaults to `POST
//!   /{wire-name}`, status 200, the whole message as the body. A transport reads this one parsed
//!   shape and never re-parses the attribute.
//!
//! The context is on [`ServiceDef`], not on any operation: every operation takes it, it is the
//! same type for all of them, and it reaches no message and no schema.
//!
//! # What is deliberately not here
//!
//! **Nothing about how a message is annotated** is here. Every ident and every type below is the
//! author's own, carried verbatim, so an emitter is free to write whatever derives and serde
//! attributes a generated message needs onto them.

use crate::rename_rule::RenameRule;
use proc_macro2::TokenTree;
use quote::{ToTokens as _, format_ident};
use std::collections::{HashMap, HashSet};
use syn::meta::ParseNestedMeta;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned as _;
use syn::{
    Attribute, FnArg, GenericArgument, GenericParam, Ident, ItemTrait, LitInt, LitStr, Pat,
    PathArguments, ReturnType, Token, TraitItem, TraitItemFn, Type,
};

/// The per-operation directive, read and then stripped before the trait is emitted.
pub const OPERATION_DIRECTIVE: &str = "service_schema_op";

const UNKNOWN_DIRECTIVE_MESSAGE: &str = concat!(
    "service_schema: unknown `service_schema_op` directive\n",
    "       the directives are `message = \"<wire name>\"`, `one_way` and `http(...)`"
);

const UNKNOWN_HTTP_ARGUMENT_MESSAGE: &str = concat!(
    "service_schema: unknown `http` argument\n",
    "       the arguments are `method`, `path`, `ok_status`, `error_status`, `header_in`, \
     `header_out`, `part` and `body`"
);

const BYTES_BODY_SUCCESS_SHAPE_MESSAGE: &str = concat!(
    "service_schema: `body = \"bytes\"` requires a success type of `(Vec<u8>, String)`, plus one \
     more element per declared `header_out` entry\n",
    "       answer `Result<(Vec<u8>, String, ...), Error>` - the bytes, their content type, then \
     each declared header's own value, in declaration order"
);

const STREAM_BODY_SUCCESS_SHAPE_MESSAGE: &str = concat!(
    "service_schema: `body = \"stream\"` requires a success type naming `StreamedAnswer`, plus \
     one more element per declared `header_out` entry\n",
    "       answer `Result<StreamedAnswer, Error>` - the module `#[service_schema]` generates \
     beside this trait publishes that type; reach it with a `use`, or write the module-qualified \
     path directly; declaring `header_out` entries wraps it in a tuple with one more element per \
     entry, in declaration order"
);

const MISSING_HTTP_METHOD_MESSAGE: &str = concat!(
    "service_schema: `http(...)` declares no `method`\n",
    "       write `method = \"GET\"` (or `\"POST\"`, `\"PUT\"`, `\"DELETE\"`, `\"PATCH\"`)"
);

const MISSING_HTTP_PATH_MESSAGE: &str = concat!(
    "service_schema: `http(...)` declares no `path`\n",
    "       write `path = \"/resource/{field}\"`"
);

const HTTP_ERROR_STATUS_SHAPE_MESSAGE: &str = concat!(
    "service_schema: `error_status` entries are `Variant = code`\n",
    "       write `error_status(NotFound = 404, ...)`"
);

const UNTERMINATED_PLACEHOLDER_MESSAGE: &str = concat!(
    "service_schema: `http(...)`'s path opens `{` with no matching `}`\n",
    "       write `{field}`, closed before the path ends"
);

const EMPTY_PLACEHOLDER_MESSAGE: &str = concat!(
    "service_schema: `http(...)`'s path has an empty `{}`\n",
    "       name the field it binds: `{field}`"
);

const UNMATCHED_CLOSING_BRACE_MESSAGE: &str = concat!(
    "service_schema: `http(...)`'s path has a `}` with no matching `{`\n",
    "       write `{field}`, or escape a literal brace some other way"
);

/// The status a declared error answers at when its operation named no `http(...)` group at all,
/// and so declared no `error_status` table to read a code from. Distinct from every fixed fault
/// status (400, 404, 500) so a caller can tell "the operation declared this failure" from "a
/// defect answered instead" even on an operation that paid no annotation cost.
///
/// `pub`: read identically by the Rust `http_rest` transport and its TypeScript client, so a
/// second copy of the number could only drift from this one.
pub const DEFAULT_BINDING_ERROR_STATUS: u16 = 422;

/// One service, read once.
pub struct ServiceDef {
    /// The trait's type parameter, which every operation takes and no message carries.
    pub context_param: Ident,
    /// Every message the macro declares for this service, in declaration order: one per operation
    /// that named none. Recorded so the emitter that writes them and the emitter that registers
    /// the service's published artifacts read one list rather than each deciding again what the
    /// macro declared, and so nothing the macro wrote can be left out of that registration.
    pub generated_messages: Vec<GeneratedMessage>,
    /// The trait as declared: `UsageService`.
    pub ident: Ident,
    pub operations: Vec<OperationDef>,
}

/// One message the macro declares, for an operation that named none. Everything the type needs to
/// be written and to be registered, so neither reader re-reads the operation it came from.
pub struct GeneratedMessage {
    /// The operation it was declared for: `expire_credit`. Its rustdoc names it.
    pub declared_for: Ident,
    /// One field per argument, in declaration order, or none at all where the operation takes
    /// nothing after the context.
    pub fields: Vec<(Ident, Type)>,
    /// The type declared: `ExpireCreditRequest`.
    pub ident: Ident,
}

/// One operation: a name in three spellings, a message in, and either a reply or nothing.
pub struct OperationDef {
    /// What `http(...)` declared, or `None` for an operation that named no group.
    pub http: Option<HttpBinding>,
    /// The trait method as declared: `get_available_balance`.
    pub ident: Ident,
    pub inputs: OperationInputs,
    pub outcome: OperationOutcome,
    /// How a TypeScript caller spells it: `getAvailableBalance`.
    pub ts_name: String,
    /// What the wire carries: `get-available-balance`, or the `message = "..."` override.
    pub wire_name: String,
}

impl OperationDef {
    /// What the message declared for this operation is called: `expire_credit` becomes
    /// `ExpireCreditRequest`. Nothing for the operation whose one argument already is the
    /// message, since none is declared for it.
    ///
    /// Spanned on the method name, so every error about the declared type — this crate's own
    /// refusals and the compiler's duplicate-definition report alike — points at the operation
    /// that declared it rather than at a call site of the macro.
    pub fn generated_message_ident(&self) -> Option<Ident> {
        match self.inputs {
            OperationInputs::Named(_) => None,
            OperationInputs::Empty | OperationInputs::Generated(_) => Some(format_ident!(
                "{}Request",
                RenameRule::PascalCase.apply_to_field(&self.ident.to_string()),
                span = self.ident.span()
            )),
        }
    }
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

/// What `http(...)` declared on one operation, checked against its signature and its outcome.
///
/// An operation that named no group carries `None` on [`OperationDef::http`] instead of one of
/// these — the default (`POST /{wire-name}`, status 200, the whole message as the body) needs
/// nothing checked against the signature, so a transport computes it on its own rather than
/// reading it off a materialized value here.
#[derive(Debug)]
pub struct HttpBinding {
    /// How the body is carried: `Json` (the default), `Bytes` (`body = "bytes"`) or `Stream`
    /// (`body = "stream"`), each checked against the signature by [`build_http_binding`].
    pub body_kind: BodyKind,
    /// One entry per declared `error_status(Variant = code)`, in declaration order. Each variant
    /// keeps its own span from the attribute, so a misspelling is rustc's own "no variant" error
    /// rather than one this crate wrote, and a variant the mapping left out is rustc's own
    /// `E0004` naming it — see [`crate::service_schema::support`]'s completeness check.
    pub error_status: Vec<(Ident, u16)>,
    /// One entry per `header_in("name" = parameter)`, naming the request header and the
    /// operation's own argument it fills.
    pub header_in: Vec<HeaderIn>,
    /// One entry per bare `header_out("name")`, in declaration order. The success type is a
    /// tuple of exactly this many elements plus the response, and this is the response header
    /// each element after the first is written out as.
    pub header_out: Vec<String>,
    /// The method the operation answers to.
    pub method: HttpMethod,
    /// One entry per `part("name" = parameter)`, naming a `body = "multipart"` operation's own
    /// file part and the operation's own argument it fills. Empty for every body kind but
    /// `Multipart`.
    pub multipart_parts: Vec<MultipartPart>,
    /// The status a success answers with: the declared `ok_status`, or 204 for a no-payload
    /// operation and 200 for every other one where the author wrote neither.
    pub ok_status: u16,
    /// The path template, walked left to right: a literal segment as written, or a `{field}`
    /// placeholder naming a field the path binds.
    pub path: Vec<PathSegment>,
}

/// The five methods `http(...)` answers to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpMethod {
    Delete,
    Get,
    Patch,
    Post,
    Put,
}

impl HttpMethod {
    /// Whether this method carries a JSON body. `GET` and `DELETE` do not, so a field the path
    /// leaves unbound has nowhere left to go — [`build_http_binding`] refuses it rather than
    /// silently dropping it.
    ///
    /// `pub(crate)`: the `http_rest` transport reads the same fact to decide whether a field
    /// belongs in the body or the query string, on the wire rather than in this parser.
    pub(crate) const fn carries_a_body(self) -> bool {
        matches!(self, Self::Patch | Self::Post | Self::Put)
    }

    fn from_name(written: &str) -> Option<Self> {
        match written {
            "DELETE" => Some(Self::Delete),
            "GET" => Some(Self::Get),
            "PATCH" => Some(Self::Patch),
            "POST" => Some(Self::Post),
            "PUT" => Some(Self::Put),
            _ => None,
        }
    }

    /// The name an operation writes for it, for a refusal to quote back.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Delete => "DELETE",
            Self::Get => "GET",
            Self::Patch => "PATCH",
            Self::Post => "POST",
            Self::Put => "PUT",
        }
    }
}

/// One segment of an `http(...)` path template.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PathSegment {
    /// Written exactly as it appears in the template, slashes included.
    Literal(String),
    /// A `{field}` placeholder, holding the name between the braces.
    Placeholder(String),
}

/// One `header_in("name" = parameter)` binding.
#[derive(Clone, Debug)]
pub struct HeaderIn {
    /// The header name, as written.
    pub name: String,
    /// The operation's own argument it fills, keeping the argument's real span.
    pub parameter: Ident,
    /// The argument's declared type, read off the same signature `parameter` names — a transport
    /// carrying the header over its own channel types its extra parameter with this rather than
    /// reading the signature a second time.
    pub ty: Type,
}

/// One `part("name" = parameter)` binding — a `body = "multipart"` operation's own file part,
/// claimed out of the message's own fields exactly as [`HeaderIn`] claims a header-bound one. A
/// scalar field carries no binding of its own: it is read straight off the same-named part, the
/// same way a bodyless method's field is read off the query string.
#[derive(Clone, Debug)]
pub struct MultipartPart {
    /// The part name, as written.
    pub name: String,
    /// The operation's own argument it fills, keeping the argument's real span.
    pub parameter: Ident,
    /// The argument's declared type, read off the same signature `parameter` names.
    pub ty: Type,
}

/// How `http(...)` carries the body. `Json` is the default a group that writes no `body` gets.
/// `Bytes` answers raw bytes under a declared content type. `Stream` answers a pulled body source,
/// full or (through `StreamedAnswer::Partial`) a `206` range slice with `content-range`, through
/// the seam `#[service_schema]` publishes beside the trait. `Multipart` reads the *request* as
/// named parts instead of one JSON object: a scalar field is read off the same-named part exactly
/// as a bodyless method's field is read off the query string, and a `part("name" = parameter)`
/// binding hands a file part through as a [`crate::service_schema::support`] `BodySource` handle,
/// undecoded. `Multipart` says nothing about the *response* — its success and error types are
/// ordinary JSON, `header_out` included, exactly like `Json`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyKind {
    Bytes,
    Json,
    Multipart,
    Stream,
}

impl BodyKind {
    fn from_name(written: &str) -> Option<Self> {
        match written {
            "bytes" => Some(Self::Bytes),
            "json" => Some(Self::Json),
            "multipart" => Some(Self::Multipart),
            "stream" => Some(Self::Stream),
            _ => None,
        }
    }
}

/// The three shapes a query, path or header value coerces to. Anything this crate does not
/// recognise coerces as [`Text`](ScalarKind::Text) — a custom type, a `chrono` type, a plain
/// `String` — since a JSON string is what every one of those already reads from serde.
///
/// `pub`: the Rust `http_rest` transport and its TypeScript client both coerce a raw wire
/// string the same way, from the same declared type, so both read this judgement rather than a
/// second copy of it.
#[derive(Clone, Copy)]
pub enum ScalarKind {
    Bool,
    Number,
    Text,
}

/// What `http(...)` resolves to for one operation, whether it was written or defaulted. Every
/// renderer over HTTP — the Rust dispatcher, the Rust client, the TypeScript client — reads this
/// rather than [`OperationDef::http`] itself, so the default case is computed in exactly one
/// place.
pub struct HttpShape {
    pub body_kind: BodyKind,
    pub error_status: Vec<(Ident, u16)>,
    pub header_in: Vec<HeaderIn>,
    pub header_out: Vec<String>,
    pub method: HttpMethod,
    pub multipart_parts: Vec<MultipartPart>,
    pub ok_status: u16,
    pub path: Vec<PathSegment>,
}

impl HttpShape {
    pub fn of(operation: &OperationDef) -> Self {
        operation.http.as_ref().map_or_else(
            || Self {
                body_kind: BodyKind::Json,
                error_status: Vec::new(),
                header_in: Vec::new(),
                header_out: Vec::new(),
                method: HttpMethod::Post,
                multipart_parts: Vec::new(),
                ok_status: default_ok_status(&operation.outcome),
                path: vec![PathSegment::Literal(format!("/{}", operation.wire_name))],
            },
            |binding| Self {
                body_kind: binding.body_kind,
                error_status: binding.error_status.clone(),
                header_in: binding.header_in.clone(),
                header_out: binding.header_out.clone(),
                method: binding.method,
                multipart_parts: binding.multipart_parts.clone(),
                ok_status: binding.ok_status,
                path: binding.path.clone(),
            },
        )
    }

    /// The template written back out as `http(...)` would have declared it: `{field}` for a
    /// placeholder, the literal text otherwise.
    pub fn path_template(&self) -> String {
        self.path
            .iter()
            .map(|segment| match segment {
                PathSegment::Literal(text) => text.clone(),
                PathSegment::Placeholder(name) => format!("{{{name}}}"),
            })
            .collect()
    }

    /// Every placeholder's name, in the template's own order.
    pub fn placeholder_names(&self) -> Vec<String> {
        self.path
            .iter()
            .filter_map(|segment| match segment {
                PathSegment::Placeholder(name) => Some(name.clone()),
                PathSegment::Literal(_) => None,
            })
            .collect()
    }
}

/// What `http(...)` said, before it is checked against the operation's signature and outcome.
struct RawHttp {
    body: Option<(BodyKind, LitStr)>,
    error_status: Vec<(Ident, u16)>,
    header_in: Vec<(LitStr, Ident)>,
    header_out: Vec<LitStr>,
    method: (HttpMethod, LitStr),
    multipart_parts: Vec<(LitStr, Ident)>,
    ok_status: Option<u16>,
    path: LitStr,
}

/// What one `#[service_schema_op(...)]` said, before anything is derived from it.
struct OperationDirective {
    http: Option<RawHttp>,
    message: Option<String>,
    one_way: bool,
}

impl OperationDirective {
    fn read(attrs: &[Attribute]) -> Result<Self, syn::Error> {
        let mut directive = Self {
            http: None,
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
                if meta.path.is_ident("http") {
                    directive.http = Some(read_http_directive(&meta)?);
                    return Ok(());
                }
                Err(meta.error(UNKNOWN_DIRECTIVE_MESSAGE))
            })?;
        }
        Ok(directive)
    }
}

/// Whether any operation in `service` declares `body = "stream"` — the body-source seam
/// (`BodySource`, `StreamedAnswer`) is published beside the trait only where at least one
/// operation needs it.
///
/// `pub`: read identically by `support` (which publishes the seam) and by the `http_rest`
/// transport (which reaches for it through `$crate`), so the two cannot disagree about whether a
/// service carries it.
pub fn service_declares_a_stream(service: &ServiceDef) -> bool {
    service.operations.iter().any(|operation| {
        operation
            .http
            .as_ref()
            .is_some_and(|binding| matches!(binding.body_kind, BodyKind::Stream))
    })
}

/// Whether any operation in `service` declares `body = "multipart"` — the request is read as
/// named parts rather than one JSON object, and a file part's own `BodySource` handle is the same
/// seam type a streamed *response* uses.
///
/// `pub`: read identically by `support` (which publishes the seam `BodySource` belongs to) and by
/// the `http_rest` transport, so the two cannot disagree about whether a service carries it.
pub fn service_declares_multipart(service: &ServiceDef) -> bool {
    service.operations.iter().any(|operation| {
        operation
            .http
            .as_ref()
            .is_some_and(|binding| matches!(binding.body_kind, BodyKind::Multipart))
    })
}

/// Whether `service` needs the `BodySource` seam published at all — either body direction reaches
/// for it: a streamed *response* pulls from one, and a multipart *request*'s own file part hands
/// one through. The one gate [`crate::service_schema::support`]'s own `stream_seam` reads, so
/// declaring either kind alone is enough to publish it.
pub fn service_needs_body_source_seam(service: &ServiceDef) -> bool {
    service_declares_a_stream(service) || service_declares_multipart(service)
}

pub fn scalar_kind(ty: &Type) -> ScalarKind {
    let Type::Path(named) = ty else {
        return ScalarKind::Text;
    };
    let Some(leaf) = named.path.segments.last() else {
        return ScalarKind::Text;
    };
    match leaf.ident.to_string().as_str() {
        "bool" => ScalarKind::Bool,
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64" | "i128"
        | "isize" | "f32" | "f64" => ScalarKind::Number,
        _ => ScalarKind::Text,
    }
}

/// Whether a Named type's own declared type is one of the wire-scalar shapes this crate
/// recognises — the only case a whole message may be read back from a single path placeholder
/// rather than an object.
pub fn is_scalar_named_type(ty: &Type) -> bool {
    let Type::Path(named) = ty else {
        return false;
    };
    let Some(leaf) = named.path.segments.last() else {
        return false;
    };
    matches!(
        leaf.ident.to_string().as_str(),
        "String"
            | "str"
            | "bool"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "f32"
            | "f64"
            | "NaiveDate"
            | "NaiveDateTime"
            | "NaiveTime"
    )
}

/// The one generic argument inside `Option<...>` or `Vec<...>`, if `ty` is written as that generic.
pub fn generic_inner<'ty>(ty: &'ty Type, wanted: &str) -> Option<&'ty Type> {
    let Type::Path(named) = ty else {
        return None;
    };
    let last = named.path.segments.last()?;
    if last.ident != wanted {
        return None;
    }
    let PathArguments::AngleBracketed(generic) = &last.arguments else {
        return None;
    };
    generic.args.iter().find_map(|argument| {
        if let GenericArgument::Type(inner) = argument {
            Some(inner)
        } else {
            None
        }
    })
}

pub fn option_inner(ty: &Type) -> Option<&Type> {
    generic_inner(ty, "Option")
}

pub fn vec_inner(ty: &Type) -> Option<&Type> {
    generic_inner(ty, "Vec")
}

/// The elements of a tuple type with at least one element, or `None` for anything else (unit
/// included). A [`HttpBinding::header_out`] binding's own arity check guarantees a success type
/// naming one is exactly this shape, so a caller holding that guarantee reads the first element as
/// the body and the rest as the declared headers, in order.
pub fn tuple_elements(ty: &Type) -> Option<&Punctuated<Type, Token![,]>> {
    let Type::Tuple(tuple) = ty else {
        return None;
    };
    (!tuple.elems.is_empty()).then_some(&tuple.elems)
}

/// The camelCase wire key one of an operation's own generated fields is read or written under:
/// a path segment, a query parameter and a header value all coerce against the same key a JSON
/// body would carry the field under.
pub fn wire_key(field: &Ident) -> String {
    RenameRule::CamelCase.apply_to_field(&field.to_string())
}

fn unknown_http_method_message(written: &str) -> String {
    format!(
        "service_schema: `{written}` is not an HTTP method this version knows\n       \
         write one of `GET`, `POST`, `PUT`, `DELETE`, `PATCH`"
    )
}

fn unknown_body_kind_message(written: &str) -> String {
    format!(
        "service_schema: `{written}` is not a body kind this version knows\n       \
         write `\"json\"` (the default), `\"bytes\"`, `\"multipart\"` or `\"stream\"`"
    )
}

fn parse_method_arg(inner: &ParseNestedMeta<'_>) -> Result<(HttpMethod, LitStr), syn::Error> {
    let written: LitStr = inner.value()?.parse()?;
    let Some(parsed) = HttpMethod::from_name(&written.value()) else {
        return Err(syn::Error::new(
            written.span(),
            unknown_http_method_message(&written.value()),
        ));
    };
    Ok((parsed, written))
}

fn parse_body_arg(inner: &ParseNestedMeta<'_>) -> Result<(BodyKind, LitStr), syn::Error> {
    let written: LitStr = inner.value()?.parse()?;
    let Some(parsed) = BodyKind::from_name(&written.value()) else {
        return Err(syn::Error::new(
            written.span(),
            unknown_body_kind_message(&written.value()),
        ));
    };
    Ok((parsed, written))
}

fn parse_error_status_arg(inner: &ParseNestedMeta<'_>) -> Result<Vec<(Ident, u16)>, syn::Error> {
    let mut error_status = Vec::new();
    inner.parse_nested_meta(|deepest| {
        let Some(variant) = deepest.path.get_ident().cloned() else {
            return Err(deepest.error(HTTP_ERROR_STATUS_SHAPE_MESSAGE));
        };
        let code: LitInt = deepest.value()?.parse()?;
        error_status.push((variant, code.base10_parse()?));
        Ok(())
    })?;
    Ok(error_status)
}

/// Reads a `("name" = parameter)` group out of `inner` - the shape `header_in` and `part` share.
fn parse_named_parameter_arg(inner: &ParseNestedMeta<'_>) -> Result<(LitStr, Ident), syn::Error> {
    let content;
    syn::parenthesized!(content in inner.input);
    let name: LitStr = content.parse()?;
    content.parse::<Token![=]>()?;
    let parameter: Ident = content.parse()?;
    Ok((name, parameter))
}

fn parse_header_out_arg(inner: &ParseNestedMeta<'_>) -> Result<LitStr, syn::Error> {
    let content;
    syn::parenthesized!(content in inner.input);
    content.parse()
}

/// Reads the content of one `http(...)` group: `method`, `path` and `ok_status` parse by plain
/// recursion, every key there opening with a bare ident — `Meta`-shaped syntax `parse_nested_meta`
/// already handles. `header_in`, `header_out` and `part` cannot: their first token is a string
/// literal, which `parse_nested_meta` rejects before it ever reaches the `=`, so each is instead
/// read by hand out of the parenthesized group its key opens, one string literal and (for
/// `header_in` and `part`) one `= parameter` after it.
fn read_http_directive(meta: &ParseNestedMeta<'_>) -> Result<RawHttp, syn::Error> {
    let mut method_written: Option<(HttpMethod, LitStr)> = None;
    let mut path_written: Option<LitStr> = None;
    let mut ok_status: Option<u16> = None;
    let mut body_written: Option<(BodyKind, LitStr)> = None;
    let mut error_status: Vec<(Ident, u16)> = Vec::new();
    let mut header_in: Vec<(LitStr, Ident)> = Vec::new();
    let mut header_out: Vec<LitStr> = Vec::new();
    let mut multipart_parts: Vec<(LitStr, Ident)> = Vec::new();

    meta.parse_nested_meta(|inner| {
        if inner.path.is_ident("method") {
            method_written = Some(parse_method_arg(&inner)?);
            return Ok(());
        }
        if inner.path.is_ident("path") {
            path_written = Some(inner.value()?.parse()?);
            return Ok(());
        }
        if inner.path.is_ident("ok_status") {
            let written: LitInt = inner.value()?.parse()?;
            ok_status = Some(written.base10_parse()?);
            return Ok(());
        }
        if inner.path.is_ident("body") {
            body_written = Some(parse_body_arg(&inner)?);
            return Ok(());
        }
        if inner.path.is_ident("error_status") {
            error_status = parse_error_status_arg(&inner)?;
            return Ok(());
        }
        if inner.path.is_ident("part") {
            multipart_parts.push(parse_named_parameter_arg(&inner)?);
            return Ok(());
        }
        if inner.path.is_ident("header_in") {
            header_in.push(parse_named_parameter_arg(&inner)?);
            return Ok(());
        }
        if inner.path.is_ident("header_out") {
            header_out.push(parse_header_out_arg(&inner)?);
            return Ok(());
        }
        Err(inner.error(UNKNOWN_HTTP_ARGUMENT_MESSAGE))
    })?;

    let resolved_method = method_written
        .ok_or_else(|| syn::Error::new(meta.path.span(), MISSING_HTTP_METHOD_MESSAGE))?;
    let resolved_path =
        path_written.ok_or_else(|| syn::Error::new(meta.path.span(), MISSING_HTTP_PATH_MESSAGE))?;

    Ok(RawHttp {
        body: body_written,
        error_status,
        header_in,
        header_out,
        method: resolved_method,
        multipart_parts,
        ok_status,
        path: resolved_path,
    })
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
        generated_messages: operations.iter().filter_map(generated_message).collect(),
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

/// Everything the operation takes after `&self` and the context, in declaration order, less
/// whatever `header_in` or `part` claimed. An operation with no `http(...)` group claims nothing,
/// so this is exactly the pre-`http` behavior for it: every argument still becomes a field on the
/// message.
fn operation_inputs(
    operation: &TraitItemFn,
    context: &Ident,
    extra_claims: &HashSet<String>,
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
        if extra_claims.contains(&argument.ident.to_string()) {
            continue;
        }
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
    // `header_in` and `part` each claim one ordinary argument beside the message, out of the same
    // pool: neither becomes a field on the message [`operation_inputs`] declares one from.
    let extra_claims: HashSet<String> = directive
        .http
        .as_ref()
        .map(|raw| {
            raw.header_in
                .iter()
                .map(|(_, parameter)| parameter.to_string())
                .chain(
                    raw.multipart_parts
                        .iter()
                        .map(|(_, parameter)| parameter.to_string()),
                )
                .collect()
        })
        .unwrap_or_default();
    let inputs = operation_inputs(operation, context, &extra_claims)?;
    let outcome = operation_outcome(operation, directive.one_way)?;
    let ident = operation.sig.ident.clone();
    let http = directive
        .http
        .map(|raw| build_http_binding(&ident, operation, raw, &inputs, &outcome))
        .transpose()?;
    let declared = ident.to_string();
    Ok(OperationDef {
        http,
        ident,
        inputs,
        outcome,
        ts_name: RenameRule::CamelCase.apply_to_field(&declared),
        wire_name: directive
            .message
            .unwrap_or_else(|| RenameRule::KebabCase.apply_to_field(&declared)),
    })
}

/// Every argument's name and declared type beyond `&self` and the context — checked against a
/// `header_in` claim, since [`operation_inputs`] has already removed its own claims from what it
/// returns and cannot itself say whether one named nothing real, and read again here for the type
/// a `header_in` binding carries forward so a transport need not read the signature a second time.
fn extra_arguments(operation: &TraitItemFn) -> HashMap<String, Type> {
    operation
        .sig
        .inputs
        .iter()
        .skip(2)
        .filter_map(|input| {
            let FnArg::Typed(typed) = input else {
                return None;
            };
            let Pat::Ident(named) = typed.pat.as_ref() else {
                return None;
            };
            Some((named.ident.to_string(), typed.ty.as_ref().clone()))
        })
        .collect()
}

/// Checks `http(...)` against the operation it was written on: every `header_in` claims a real
/// argument, every path placeholder matches a field the message actually has, a required field a
/// bodyless method cannot carry any other way is bound in the path, and a tuple success type is
/// exactly explained by `header_out`.
///
/// The error-variant mapping is deliberately not checked here — this function cannot see the
/// error type's own declaration, that type being an ordinary sibling item rather than one this
/// macro reads, the same problem `error_status` completeness always has. `support::emit` answers
/// it instead, with a match carrying exactly the declared arms and no wildcard, so rustc's own
/// exhaustiveness check is what refuses a variant the mapping left out — see
/// `http_error_status_completeness` in [`crate::service_schema::support`] for that one.
///
/// # A full declaration, compiling
///
/// Every grammar arm at once: a path placeholder bound to a message field, one header claimed
/// beside the message, one header written out beside the response, and a complete status table.
///
/// ```rust
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct GetVersionRequest {
///     pub document_id: String,
///     pub version_id: String,
/// }
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct VersionResponse {
///     pub content: String,
/// }
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum GetVersionError {
///     NotFound,
///     VersionGone,
/// }
///
/// #[service_schema(transports = ["http_rest"])]
/// pub trait DocumentService<Ctx> {
///     #[service_schema_op(http(
///         method = "GET",
///         path = "/documents/{document_id}/versions/{version_id}",
///         ok_status = 200,
///         header_in("range" = byte_range),
///         header_out("etag"),
///         error_status(NotFound = 404, VersionGone = 410),
///     ))]
///     async fn get_version(
///         &self,
///         ctx: &Ctx,
///         req: GetVersionRequest,
///         byte_range: Option<String>,
///     ) -> Result<(VersionResponse, String), GetVersionError>;
/// }
///
/// fn main() {}
/// ```
///
/// # A path placeholder naming no field is refused, naming the placeholder
///
/// The message below has fields `widget_id` and `label`; the path names neither:
///
/// ```rust,compile_fail
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct WidgetResponse;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum WidgetError {
///     NotFound,
/// }
///
/// #[service_schema()]
/// pub trait WidgetService<Ctx> {
///     #[service_schema_op(http(
///         method = "POST",
///         path = "/widgets/{item_id}",
///         ok_status = 200,
///         error_status(NotFound = 404),
///     ))]
///     async fn get_widget(
///         &self,
///         ctx: &Ctx,
///         widget_id: String,
///         label: String,
///     ) -> Result<WidgetResponse, WidgetError>;
/// }
///
/// fn main() {}
/// ```
///
/// A `compile_fail` doctest asserts only that *something* was refused, so the file above was
/// compiled standalone and the diagnostic read off that run, verbatim, and it was the only error
/// the file earned:
///
/// ```text
/// error: service_schema: operation `get_widget`'s path names `{item_id}`, and its message has no field named `item_id`
///               a path placeholder binds a same-named field on the message
///   --> tests/zz_probe.rs:15:16
///    |
/// 15 |         path = "/widgets/{item_id}",
///    |                ^^^^^^^^^^^^^^^^^^^^
///
/// error: could not compile `tixschema` (test "zz_probe") due to 1 previous error
/// ```
///
/// # A required field a bodyless method cannot carry is refused, naming the field
///
/// The same shape with the path corrected to name `widget_id`, `method` changed to `GET`, and
/// `label` left unbound — `GET` carries no body, so `label` has nowhere left to go:
///
/// ```rust,compile_fail
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct WidgetResponse;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum WidgetError {
///     NotFound,
/// }
///
/// #[service_schema()]
/// pub trait WidgetService<Ctx> {
///     #[service_schema_op(http(
///         method = "GET",
///         path = "/widgets/{widget_id}",
///         ok_status = 200,
///         error_status(NotFound = 404),
///     ))]
///     async fn get_widget(
///         &self,
///         ctx: &Ctx,
///         widget_id: String,
///         filter: String,
///     ) -> Result<WidgetResponse, WidgetError>;
/// }
///
/// fn main() {}
/// ```
///
/// ```text
/// error: service_schema: operation `get_widget`'s field `filter` is required and is bound by no path placeholder
///               `GET` carries no body, so a required field must appear in the path
///   --> tests/zz_probe.rs:15:16
///    |
/// 15 |         path = "/widgets/{widget_id}",
///    |                ^^^^^^^^^^^^^^^^^^^^^^
///
/// error: could not compile `tixschema` (test "zz_probe") due to 1 previous error
/// ```
///
/// # `header_in` naming an argument that does not exist is refused, naming the parameter
///
/// `byte_range` is written nowhere in the signature:
///
/// ```rust,compile_fail
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct WidgetResponse;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum WidgetError {
///     NotFound,
/// }
///
/// #[service_schema()]
/// pub trait WidgetService<Ctx> {
///     #[service_schema_op(http(
///         method = "GET",
///         path = "/widgets/{widget_id}",
///         ok_status = 200,
///         header_in("range" = byte_range),
///         error_status(NotFound = 404),
///     ))]
///     async fn get_widget(
///         &self,
///         ctx: &Ctx,
///         widget_id: String,
///     ) -> Result<WidgetResponse, WidgetError>;
/// }
///
/// fn main() {}
/// ```
///
/// ```text
/// error: service_schema: operation `get_widget` binds header "range" to a parameter named `byte_range`, and `get_widget` takes no argument by that name
///               `header_in` binds one ordinary argument beside the message; name it in the signature, or remove the binding
///   --> tests/zz_probe.rs:17:29
///    |
/// 17 |         header_in("range" = byte_range),
///    |                             ^^^^^^^^^^
///
/// error: could not compile `tixschema` (test "zz_probe") due to 1 previous error
/// ```
///
/// # A tuple success type with no `header_out` is refused, naming the requirement
///
/// The same shape returning a two-element tuple and declaring no `header_out`:
///
/// ```rust,compile_fail
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct WidgetResponse;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum WidgetError {
///     NotFound,
/// }
///
/// #[service_schema()]
/// pub trait WidgetService<Ctx> {
///     #[service_schema_op(http(
///         method = "GET",
///         path = "/widgets/{widget_id}",
///         ok_status = 200,
///         error_status(NotFound = 404),
///     ))]
///     async fn get_widget(
///         &self,
///         ctx: &Ctx,
///         widget_id: String,
///     ) -> Result<(WidgetResponse, String), WidgetError>;
/// }
///
/// fn main() {}
/// ```
///
/// ```text
/// error: service_schema: operation `get_widget` returns a tuple success type and declares no `header_out`
///               name what each element after the first is with `header_out("name")`, or return the type directly
///   --> tests/zz_probe.rs:23:17
///    |
/// 23 |     ) -> Result<(WidgetResponse, String), WidgetError>;
///    |                 ^^^^^^^^^^^^^^^^^^^^^^^^
///
/// error: could not compile `tixschema` (test "zz_probe") due to 1 previous error
/// ```
///
/// # A `body = "bytes"` declaration whose signature still claims a JSON success type is refused
///
/// `body = "bytes"` requires a reply's success type to be exactly `(Vec<u8>, String)` — the bytes,
/// then their content type. The operation below still answers its own JSON type:
///
/// ```rust,compile_fail
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct ThumbnailResponse {
///     pub url: String,
/// }
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum ThumbnailError {
///     NotFound,
/// }
///
/// #[service_schema()]
/// pub trait ThumbnailService<Ctx> {
///     #[service_schema_op(http(
///         method = "GET",
///         path = "/thumbnails/{document_id}",
///         body = "bytes",
///         error_status(NotFound = 404),
///     ))]
///     async fn get_thumbnail(
///         &self,
///         ctx: &Ctx,
///         document_id: String,
///     ) -> Result<ThumbnailResponse, ThumbnailError>;
/// }
///
/// fn main() {}
/// ```
///
/// ```text
/// error: service_schema: `body = "bytes"` requires a success type of `(Vec<u8>, String)`
///               the operation's signature still claims a JSON success type - answer `Result<(Vec<u8>, String), Error>`, the bytes and their content type
///   --> tests/zz_probe.rs:25:17
///    |
/// 25 |     ) -> Result<ThumbnailResponse, ThumbnailError>;
///    |                 ^^^^^^^^^^^^^^^^^
///
/// error: could not compile `tixschema` (test "zz_probe") due to 1 previous error
/// ```
///
/// # A `body = "bytes"` declaration composes `header_out` onto its own tuple, body pair first
///
/// `header_out` appends one more element per entry after the bytes and their content type, in
/// declaration order — the dispatcher writes each as a response header exactly as the JSON path
/// does, and the client reads them back the same way:
///
/// ```rust
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum ThumbnailError {
///     NotFound,
/// }
///
/// #[service_schema()]
/// pub trait ThumbnailService<Ctx> {
///     #[service_schema_op(http(
///         method = "GET",
///         path = "/thumbnails/{document_id}",
///         body = "bytes",
///         header_out("etag"),
///         error_status(NotFound = 404),
///     ))]
///     async fn get_thumbnail(
///         &self,
///         ctx: &Ctx,
///         document_id: String,
///     ) -> Result<(Vec<u8>, String, String), ThumbnailError>;
/// }
///
/// fn main() {}
/// ```
///
/// # A `body = "bytes"` declaration with `header_out` but the wrong tuple arity is refused
///
/// One `header_out` entry requires a three-element tuple; the signature below still answers only
/// the bytes and their content type:
///
/// ```rust,compile_fail
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum ThumbnailError {
///     NotFound,
/// }
///
/// #[service_schema()]
/// pub trait ThumbnailService<Ctx> {
///     #[service_schema_op(http(
///         method = "GET",
///         path = "/thumbnails/{document_id}",
///         body = "bytes",
///         header_out("etag"),
///         error_status(NotFound = 404),
///     ))]
///     async fn get_thumbnail(
///         &self,
///         ctx: &Ctx,
///         document_id: String,
///     ) -> Result<(Vec<u8>, String), ThumbnailError>;
/// }
///
/// fn main() {}
/// ```
///
/// ```text
/// error: service_schema: `body = "bytes"` requires a success type of `(Vec<u8>, String)`, plus one more element per declared `header_out` entry
///               answer `Result<(Vec<u8>, String, ...), Error>` - the bytes, their content type, then each declared header's own value, in declaration order
///   --> tests/zz_probe.rs:25:17
///    |
/// 25 |     ) -> Result<(Vec<u8>, String), ThumbnailError>;
///    |                 ^^^^^^^^^^^^^^^^^
///
/// error: could not compile `tixschema` (test "zz_probe") due to 1 previous error
/// ```
///
/// # A `body = "stream"` declaration whose signature still claims a JSON success type is refused
///
/// `body = "stream"` requires a reply's success type to name `StreamedAnswer`, the seam type
/// `#[service_schema]` publishes beside the trait. The operation below still answers its own JSON
/// type:
///
/// ```rust,compile_fail
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct ThumbnailResponse {
///     pub url: String,
/// }
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum ContentError {
///     NotFound,
/// }
///
/// #[service_schema()]
/// pub trait ContentService<Ctx> {
///     #[service_schema_op(http(
///         method = "GET",
///         path = "/documents/{document_id}/content",
///         body = "stream",
///         error_status(NotFound = 404),
///     ))]
///     async fn get_content(
///         &self,
///         ctx: &Ctx,
///         document_id: String,
///     ) -> Result<ThumbnailResponse, ContentError>;
/// }
///
/// fn main() {}
/// ```
///
/// ```text
/// error: service_schema: `body = "stream"` requires a success type naming `StreamedAnswer`, plus one more element per declared `header_out` entry
///               answer `Result<StreamedAnswer, Error>` - the module `#[service_schema]` generates beside this trait publishes that type; reach it with a `use`, or write the module-qualified path directly; declaring `header_out` entries wraps it in a tuple with one more element per entry, in declaration order
///   --> tests/zz_probe.rs:25:17
///    |
/// 25 |     ) -> Result<ThumbnailResponse, ContentError>;
///    |                 ^^^^^^^^^^^^^^^^^
///
/// error: could not compile `tixschema` (test "zz_probe") due to 1 previous error
/// ```
///
/// # A `body = "stream"` declaration composes `header_out` onto its own answer, wrapped in a tuple
///
/// `header_out` wraps `StreamedAnswer` in a tuple with one more element per entry — the header
/// travels beside whichever arm (`Full` or `Partial`) the handler answers with, exactly as the
/// JSON path's own `header_out` composition works:
///
/// ```rust
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum ContentError {
///     NotFound,
/// }
///
/// #[service_schema()]
/// pub trait ContentService<Ctx> {
///     #[service_schema_op(http(
///         method = "GET",
///         path = "/documents/{document_id}/content",
///         body = "stream",
///         header_out("etag"),
///         error_status(NotFound = 404),
///     ))]
///     async fn get_content(
///         &self,
///         ctx: &Ctx,
///         document_id: String,
///     ) -> Result<(content_service_schema::StreamedAnswer, String), ContentError>;
/// }
///
/// fn main() {}
/// ```
///
/// # A `body = "stream"` declaration with `header_out` but the wrong tuple arity is refused
///
/// One `header_out` entry requires a two-element tuple; the signature below still answers the
/// bare `StreamedAnswer`:
///
/// ```rust,compile_fail
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum ContentError {
///     NotFound,
/// }
///
/// #[service_schema()]
/// pub trait ContentService<Ctx> {
///     #[service_schema_op(http(
///         method = "GET",
///         path = "/documents/{document_id}/content",
///         body = "stream",
///         header_out("etag"),
///         error_status(NotFound = 404),
///     ))]
///     async fn get_content(
///         &self,
///         ctx: &Ctx,
///         document_id: String,
///     ) -> Result<content_service_schema::StreamedAnswer, ContentError>;
/// }
///
/// fn main() {}
/// ```
///
/// ```text
/// error: service_schema: `body = "stream"` requires a success type naming `StreamedAnswer`, plus one more element per declared `header_out` entry
///               answer `Result<StreamedAnswer, Error>` - the module `#[service_schema]` generates beside this trait publishes that type; reach it with a `use`, or write the module-qualified path directly; declaring `header_out` entries wraps it in a tuple with one more element per entry, in declaration order
///   --> tests/zz_probe.rs:14:20
///    |
/// 14 |         header_out("etag"),
///    |                    ^^^^^^
///
/// error: could not compile `tixschema` (test "zz_probe") due to 1 previous error
/// ```
///
/// # A `body = "multipart"` declaration on a bodyless method is refused
///
/// `GET` carries no request body for parts to travel in:
///
/// ```rust,compile_fail
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct UploadResponse {
///     pub document_id: String,
/// }
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum UploadError {
///     TooLarge,
/// }
///
/// #[service_schema()]
/// pub trait UploadService<Ctx> {
///     #[service_schema_op(http(
///         method = "GET",
///         path = "/documents",
///         body = "multipart",
///         error_status(TooLarge = 413),
///     ))]
///     async fn upload_document(
///         &self,
///         ctx: &Ctx,
///         title: String,
///     ) -> Result<UploadResponse, UploadError>;
/// }
///
/// fn main() {}
/// ```
///
/// ```text
/// error: service_schema: operation `upload_document` declares `body = "multipart"` on a `GET`
///               `GET` carries no request body for parts to travel in; write `POST`, `PUT` or `PATCH`
///   --> tests/zz_probe.rs:16:16
///    |
/// 16 |         body = "multipart",
///    |                ^^^^^^^^^^^
///
/// error: could not compile `tixschema` (test "zz_probe") due to 1 previous error
/// ```
///
/// # A `part` naming an argument the operation does not have is refused
///
/// `attachment` is written nowhere in the signature:
///
/// ```rust,compile_fail
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct UploadResponse {
///     pub document_id: String,
/// }
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum UploadError {
///     TooLarge,
/// }
///
/// #[service_schema()]
/// pub trait UploadService<Ctx> {
///     #[service_schema_op(http(
///         method = "POST",
///         path = "/documents",
///         body = "multipart",
///         part("file" = attachment),
///         error_status(TooLarge = 413),
///     ))]
///     async fn upload_document(
///         &self,
///         ctx: &Ctx,
///         title: String,
///     ) -> Result<UploadResponse, UploadError>;
/// }
///
/// fn main() {}
/// ```
///
/// ```text
/// error: service_schema: operation `upload_document` binds part "file" to a parameter named `attachment`, and `upload_document` takes no argument by that name
///               `part` binds one ordinary argument beside the message; name it in the signature, or remove the binding
///   --> tests/zz_probe.rs:18:24
///    |
/// 18 |         part("file" = attachment),
///    |                       ^^^^^^^^^^
///
/// error: could not compile `tixschema` (test "zz_probe") due to 1 previous error
/// ```
///
/// # A body-source-shaped argument with no `part` binding is refused
///
/// `attachment` looks like a file part - its type names `BodySource` - but nothing declares which
/// multipart part fills it, and a generated message has no `Deserialize` for it to fall back to:
///
/// ```rust,compile_fail
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct UploadResponse {
///     pub document_id: String,
/// }
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum UploadError {
///     TooLarge,
/// }
///
/// #[service_schema()]
/// pub trait UploadService<Ctx> {
///     #[service_schema_op(http(
///         method = "POST",
///         path = "/documents",
///         body = "multipart",
///         error_status(TooLarge = 413),
///     ))]
///     async fn upload_document(
///         &self,
///         ctx: &Ctx,
///         title: String,
///         attachment: Box<dyn upload_service_schema::BodySource + Send>,
///     ) -> Result<UploadResponse, UploadError>;
/// }
///
/// fn main() {}
/// ```
///
/// ```text
/// error: service_schema: operation `upload_document`'s argument `attachment` is a body-source handle with no `part(...)` binding
///               name it with `part("..." = attachment)`, or it has nowhere to read its bytes from - a generated message has no `Deserialize` for it to fall back to
///   --> tests/zz_probe.rs:20:21
///    |
/// 20 |         attachment: Box<dyn upload_service_schema::BodySource + Send>,
///    |                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
///
/// error: could not compile `tixschema` (test "zz_probe") due to 1 previous error
/// ```
fn build_http_binding(
    operation_ident: &Ident,
    operation: &TraitItemFn,
    raw: RawHttp,
    inputs: &OperationInputs,
    outcome: &OperationOutcome,
) -> Result<HttpBinding, syn::Error> {
    let mut refusals: Option<syn::Error> = None;

    let existing = extra_arguments(operation);
    for (name, parameter) in &raw.header_in {
        if !existing.contains_key(&parameter.to_string()) {
            refusals = Some(combined(
                refusals.take(),
                syn::Error::new(
                    parameter.span(),
                    unclaimed_extra_parameter_message(operation_ident, &name.value(), parameter),
                ),
            ));
        }
    }

    if let Some(refusal) = multipart_refusals(operation_ident, &raw, &existing) {
        refusals = Some(combined(refusals.take(), refusal));
    }

    let path = parse_path_template(&raw.path)?;
    if let Some(refusal) =
        placeholder_refusals(operation_ident, &raw.path, raw.method.0, &path, inputs)
    {
        refusals = Some(combined(refusals.take(), refusal));
    }

    if let Some(refusal) = header_out_refusals(operation_ident, &raw, outcome) {
        refusals = Some(combined(refusals.take(), refusal));
    }

    if let Some(refusal) = body_kind_refusals(&raw, outcome) {
        refusals = Some(combined(refusals.take(), refusal));
    }

    if let Some(built) = refusals {
        return Err(built);
    }

    Ok(HttpBinding {
        body_kind: raw.body.as_ref().map_or(BodyKind::Json, |(kind, _)| *kind),
        error_status: raw.error_status,
        // A parameter absent from `existing` was already refused above, and `refusals` returned
        // `Err` before this point ran — `filter_map` drops it here rather than asserting an
        // invariant this function has already checked once.
        header_in: raw
            .header_in
            .into_iter()
            .filter_map(|(name, parameter)| {
                let ty = existing.get(&parameter.to_string())?.clone();
                Some(HeaderIn {
                    name: name.value(),
                    parameter,
                    ty,
                })
            })
            .collect(),
        header_out: raw
            .header_out
            .into_iter()
            .map(|name| name.value())
            .collect(),
        method: raw.method.0,
        multipart_parts: raw
            .multipart_parts
            .into_iter()
            .filter_map(|(name, parameter)| {
                let ty = existing.get(&parameter.to_string())?.clone();
                Some(MultipartPart {
                    name: name.value(),
                    parameter,
                    ty,
                })
            })
            .collect(),
        ok_status: raw.ok_status.unwrap_or_else(|| default_ok_status(outcome)),
        path,
    })
}

/// Splits a path template into its literal runs and `{field}` placeholders, left to right.
fn parse_path_template(path: &LitStr) -> Result<Vec<PathSegment>, syn::Error> {
    let written = path.value();
    let mut segments = Vec::new();
    let mut literal = String::new();
    let mut chars = written.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                if !literal.is_empty() {
                    segments.push(PathSegment::Literal(literal));
                    literal = String::new();
                }
                let mut name = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(inner) => name.push(inner),
                        None => {
                            return Err(syn::Error::new(
                                path.span(),
                                UNTERMINATED_PLACEHOLDER_MESSAGE,
                            ));
                        }
                    }
                }
                if name.is_empty() {
                    return Err(syn::Error::new(path.span(), EMPTY_PLACEHOLDER_MESSAGE));
                }
                segments.push(PathSegment::Placeholder(name));
            }
            '}' => {
                return Err(syn::Error::new(
                    path.span(),
                    UNMATCHED_CLOSING_BRACE_MESSAGE,
                ));
            }
            other => literal.push(other),
        }
    }
    if !literal.is_empty() {
        segments.push(PathSegment::Literal(literal));
    }
    Ok(segments)
}

/// Whether a field's declared type is `Option<...>`, read syntactically off the written type —
/// there being no type resolution available to a proc macro, this is the same shallow check every
/// other optional-field convention in this crate makes.
fn is_option_type(ty: &Type) -> bool {
    let Type::Path(named) = ty else {
        return false;
    };
    named
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Option")
}

/// Whether a type is the unit type `()`, which is what "an empty declared reply" writes.
///
/// `pub`: the `http_rest` transport and its TypeScript client both answer a unit success with no
/// payload the same way, so both read this judgement rather than a second copy of it. `pub` rather
/// than `pub(crate)` for the same reason [`default_ok_status`] is: this module is private, so
/// nothing wider than the crate can reach it regardless.
pub fn is_unit_type(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty())
}

/// Whether `ty` is written as the bare path `name`, read the same shallow, syntactic way every
/// other check here reads a type.
fn is_named_type(ty: &Type, name: &str) -> bool {
    let Type::Path(named) = ty else {
        return false;
    };
    named.path.is_ident(name)
}

/// Whether `ty` is written exactly as `Vec<u8>`.
fn is_vec_u8_type(ty: &Type) -> bool {
    let Type::Path(named) = ty else {
        return false;
    };
    let Some(last) = named.path.segments.last() else {
        return false;
    };
    if last.ident != "Vec" {
        return false;
    }
    let PathArguments::AngleBracketed(generic) = &last.arguments else {
        return false;
    };
    let mut arguments = generic.args.iter();
    match (arguments.next(), arguments.next()) {
        (Some(GenericArgument::Type(inner)), None) => is_named_type(inner, "u8"),
        _ => false,
    }
}

/// Whether `ty` is `(Vec<u8>, String)` — the shape `body = "bytes"` requires of a reply's success
/// type with no declared `header_out` — followed by exactly `header_out_len` further elements
/// where one was declared: the bytes, their content type, then each declared header's own value,
/// in declaration order.
fn is_bytes_success_shape(ty: &Type, header_out_len: usize) -> bool {
    let Type::Tuple(tuple) = ty else {
        return false;
    };
    let mut elements = tuple.elems.iter();
    match (elements.next(), elements.next()) {
        (Some(first), Some(second)) => {
            is_vec_u8_type(first)
                && is_named_type(second, "String")
                && elements.len() == header_out_len
        }
        _ => false,
    }
}

/// Whether `ty`'s last path segment is `StreamedAnswer` — the seam type `support` publishes
/// beside the trait, named either bare (`StreamedAnswer`, in scope through `use super::*` inside
/// that module) or module-qualified (`{module}::StreamedAnswer`) from the trait's own signature.
/// Read by its last segment rather than [`is_named_type`]'s single-segment match for exactly that
/// reason: `body = "bytes"`'s own tuple elements are always bare stdlib names, this one is not.
fn is_streamed_answer_type(ty: &Type) -> bool {
    let Type::Path(named) = ty else {
        return false;
    };
    named
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "StreamedAnswer")
}

/// Whether `ty` is `StreamedAnswer` (bare or module-qualified) — the shape `body = "stream"`
/// requires with no declared `header_out` — or, where one was declared, a tuple whose first
/// element is that same shape, followed by exactly `header_out_len` further elements: the answer,
/// then each declared header's own value, in declaration order.
fn is_stream_success_shape(ty: &Type, header_out_len: usize) -> bool {
    if header_out_len == 0 {
        return is_streamed_answer_type(ty);
    }
    let Type::Tuple(tuple) = ty else {
        return false;
    };
    let mut elements = tuple.elems.iter();
    elements
        .next()
        .is_some_and(|first| is_streamed_answer_type(first) && elements.len() == header_out_len)
}

/// Refuses a `body = "bytes"` or `body = "stream"` declaration whose success type does not match
/// what that body kind requires: `(Vec<u8>, String)` for bytes, a bare or module-qualified
/// `StreamedAnswer` for stream — either one followed by exactly one further tuple element per
/// declared `header_out` entry, in declaration order, where one was declared. A one-way operation,
/// having no reply to shape at all, is refused under the same message either way.
fn body_kind_refusals(raw: &RawHttp, outcome: &OperationOutcome) -> Option<syn::Error> {
    let (kind, declared) = raw.body.as_ref()?;
    let header_out_len = raw.header_out.len();
    let (shape_message, shaped_correctly): (&str, bool) = match kind {
        // Neither says anything about the *response* shape: `Json` never did, and `Multipart`
        // only changes how the *request* is read - its own refusals are `multipart_refusals`'s.
        BodyKind::Json | BodyKind::Multipart => return None,
        BodyKind::Bytes => (
            BYTES_BODY_SUCCESS_SHAPE_MESSAGE,
            match outcome {
                OperationOutcome::OneWay => false,
                OperationOutcome::Reply {
                    success,
                    error: _error,
                } => is_bytes_success_shape(success, header_out_len),
            },
        ),
        BodyKind::Stream => (
            STREAM_BODY_SUCCESS_SHAPE_MESSAGE,
            match outcome {
                OperationOutcome::OneWay => false,
                OperationOutcome::Reply {
                    success,
                    error: _error,
                } => is_stream_success_shape(success, header_out_len),
            },
        ),
    };
    if shaped_correctly {
        return None;
    }
    let spanned = match outcome {
        OperationOutcome::OneWay => declared.span(),
        OperationOutcome::Reply {
            success,
            error: _error,
        } => success.span(),
    };
    Some(syn::Error::new(spanned, shape_message))
}

/// The status a success answers with where the author wrote no `ok_status`: 204 for an operation
/// with nothing to serialize, 200 for every other one.
///
/// `pub` rather than private: an operation naming no `http(...)` group carries no [`HttpBinding`]
/// at all — "a transport defaults it on its own, and nothing here manufactures one to default" —
/// and the `http_rest` transport reaches for the very same rule this file already applies to a
/// group that named no `ok_status`, so the two cases cannot silently drift apart into two
/// different defaults. `pub` rather than `pub(crate)`: this module is private, so nothing wider
/// than the crate can reach it regardless, and `clippy::redundant_pub_crate` asks for the plainer
/// spelling wherever that is already true.
pub fn default_ok_status(outcome: &OperationOutcome) -> u16 {
    match outcome {
        OperationOutcome::OneWay => 204,
        OperationOutcome::Reply {
            success,
            error: _error,
        } if is_unit_type(success) => 204,
        OperationOutcome::Reply {
            error: _error,
            success: _success,
        } => 200,
    }
}

fn unclaimed_extra_parameter_message(operation: &Ident, name: &str, parameter: &Ident) -> String {
    format!(
        "service_schema: operation `{operation}` binds header \"{name}\" to a parameter named \
         `{parameter}`, and `{operation}` takes no argument by that name\n       \
         `header_in` binds one ordinary argument beside the message; name it in the signature, \
         or remove the binding"
    )
}

fn unclaimed_part_parameter_message(operation: &Ident, name: &str, parameter: &Ident) -> String {
    format!(
        "service_schema: operation `{operation}` binds part \"{name}\" to a parameter named \
         `{parameter}`, and `{operation}` takes no argument by that name\n       \
         `part` binds one ordinary argument beside the message; name it in the signature, or \
         remove the binding"
    )
}

fn multipart_on_bodyless_method_message(operation: &Ident, method: HttpMethod) -> String {
    format!(
        "service_schema: operation `{operation}` declares `body = \"multipart\"` on a `{}`\n       \
         `{}` carries no request body for parts to travel in; write `POST`, `PUT` or `PATCH`",
        method.name(),
        method.name(),
    )
}

fn unbound_body_source_message(operation: &Ident, parameter: &str) -> String {
    format!(
        "service_schema: operation `{operation}`'s argument `{parameter}` is a body-source \
         handle with no `part(...)` binding\n       \
         name it with `part(\"...\" = {parameter})`, or it has nowhere to read its bytes from - \
         a generated message has no `Deserialize` for it to fall back to"
    )
}

/// Whether `ty` mentions `BodySource` anywhere in its written tokens — shallow and syntactic, the
/// same way [`names_the_context`] reads for the trait's own type parameter. What this catches: an
/// argument typed `Box<dyn BodySource + Send>` (or any wrapper naming it) that no `part(...)`
/// binding claims, which would otherwise fall through to becoming an ordinary field on the
/// generated message — and `BodySource` publishes no `Deserialize` for that field's own derive to
/// find.
fn mentions_body_source(ty: &Type) -> bool {
    fn mentions(tree: &TokenTree) -> bool {
        match tree {
            TokenTree::Group(group) => group.stream().into_iter().any(|inner| mentions(&inner)),
            TokenTree::Ident(named) => named == "BodySource",
            TokenTree::Literal(_) | TokenTree::Punct(_) => false,
        }
    }
    ty.to_token_stream().into_iter().any(|tree| mentions(&tree))
}

/// The three ways a multipart declaration can be wrong at compile time, checked together since
/// none needs the others' guarantee first: a `part(...)` naming an argument the operation does not
/// have, `body = "multipart"` on a method with no request body for parts to travel in, and a
/// body-source-shaped argument no `part(...)` binding claims.
fn multipart_refusals(
    operation_ident: &Ident,
    raw: &RawHttp,
    existing: &HashMap<String, Type>,
) -> Option<syn::Error> {
    let mut refusals: Option<syn::Error> = None;
    for (name, parameter) in &raw.multipart_parts {
        if !existing.contains_key(&parameter.to_string()) {
            refusals = Some(combined(
                refusals.take(),
                syn::Error::new(
                    parameter.span(),
                    unclaimed_part_parameter_message(operation_ident, &name.value(), parameter),
                ),
            ));
        }
    }
    if let Some((BodyKind::Multipart, declared)) = &raw.body
        && !raw.method.0.carries_a_body()
    {
        refusals = Some(combined(
            refusals.take(),
            syn::Error::new(
                declared.span(),
                multipart_on_bodyless_method_message(operation_ident, raw.method.0),
            ),
        ));
    }
    let claimed: HashSet<String> = raw
        .header_in
        .iter()
        .map(|(_, parameter)| parameter.to_string())
        .chain(
            raw.multipart_parts
                .iter()
                .map(|(_, parameter)| parameter.to_string()),
        )
        .collect();
    // A sorted, owned snapshot rather than iterating `existing` (a `HashMap`) directly: the
    // arguments this walks are the operation's own signature, few enough that cloning them is
    // free, and a refusal's own ordering must not depend on the hasher's arbitrary bucket order.
    let mut candidates: Vec<(&String, &Type)> = existing.iter().collect();
    candidates.sort_by_key(|(name, _)| (*name).clone());
    for (name, ty) in candidates {
        if claimed.contains(name) || !mentions_body_source(ty) {
            continue;
        }
        refusals = Some(combined(
            refusals.take(),
            syn::Error::new(
                ty.span(),
                unbound_body_source_message(operation_ident, name),
            ),
        ));
    }
    refusals
}

fn unmatched_placeholder_message(operation: &Ident, placeholder: &str) -> String {
    format!(
        "service_schema: operation `{operation}`'s path names `{{{placeholder}}}`, and its \
         message has no field named `{placeholder}`\n       \
         a path placeholder binds a same-named field on the message"
    )
}

fn unbound_required_field_message(operation: &Ident, field: &str, method: HttpMethod) -> String {
    format!(
        "service_schema: operation `{operation}`'s field `{field}` is required and is bound by \
         no path placeholder\n       \
         `{}` carries no body, so a required field must appear in the path",
        method.name(),
    )
}

/// Every placeholder the path names must match a field the message actually has, and — on a
/// method with no body to fall back to — every required field must be named by one.
///
/// Only checked where the message's fields are visible to this macro: [`OperationInputs::Empty`]
/// (there are none) and [`OperationInputs::Generated`] (the operation's own argument list, read
/// directly). [`OperationInputs::Named`] is an author's own type declared elsewhere, and this
/// macro cannot see its fields any more than [`build_http_binding`] can see an error enum's
/// variants — checking it is future work.
fn placeholder_refusals(
    operation_ident: &Ident,
    path_literal: &LitStr,
    method: HttpMethod,
    path: &[PathSegment],
    inputs: &OperationInputs,
) -> Option<syn::Error> {
    let fields: &[(Ident, Type)] = match inputs {
        OperationInputs::Generated(fields) => fields,
        OperationInputs::Empty => &[],
        OperationInputs::Named(_) => return None,
    };
    let placeholders: Vec<&str> = path
        .iter()
        .filter_map(|segment| match segment {
            PathSegment::Placeholder(name) => Some(name.as_str()),
            PathSegment::Literal(_) => None,
        })
        .collect();

    let mut refusals: Option<syn::Error> = None;
    for &placeholder in &placeholders {
        if !fields.iter().any(|(field, _)| field == placeholder) {
            refusals = Some(combined(
                refusals.take(),
                syn::Error::new(
                    path_literal.span(),
                    unmatched_placeholder_message(operation_ident, placeholder),
                ),
            ));
        }
    }

    if !method.carries_a_body() {
        for (field, ty) in fields {
            let named = field.to_string();
            if placeholders.contains(&named.as_str()) || is_option_type(ty) {
                continue;
            }
            refusals = Some(combined(
                refusals.take(),
                syn::Error::new(
                    path_literal.span(),
                    unbound_required_field_message(operation_ident, &named, method),
                ),
            ));
        }
    }
    refusals
}

fn tuple_without_header_out_message(operation: &Ident) -> String {
    format!(
        "service_schema: operation `{operation}` returns a tuple success type and declares no \
         `header_out`\n       \
         name what each element after the first is with `header_out(\"name\")`, or return the \
         type directly"
    )
}

fn header_out_arity_message(operation: &Ident, declared: usize) -> String {
    let expected = declared + 1;
    let plural = if declared == 1 { "entry" } else { "entries" };
    format!(
        "service_schema: operation `{operation}` declares {declared} `header_out` {plural}, and \
         its success type is not a tuple of {expected} elements\n       \
         the tuple carries the response first, then one element per `header_out`, in declaration \
         order"
    )
}

fn header_out_on_one_way_message(operation: &Ident) -> String {
    format!(
        "service_schema: operation `{operation}` is marked `one_way` and declares `header_out`\n       \
         a one-way operation produces no reply to carry a header in"
    )
}

/// A tuple success type is explained only by `header_out`: as many entries as elements after the
/// first, in declaration order. A one-way operation has no reply to carry one in at all.
fn header_out_refusals(
    operation_ident: &Ident,
    raw: &RawHttp,
    outcome: &OperationOutcome,
) -> Option<syn::Error> {
    match outcome {
        OperationOutcome::OneWay => raw.header_out.first().map(|first| {
            syn::Error::new(first.span(), header_out_on_one_way_message(operation_ident))
        }),
        OperationOutcome::Reply {
            success,
            error: _error,
        } => {
            // `body = "bytes"` and `body = "stream"` each compose `header_out` onto their own
            // fixed shape (the bytes pair, or the streamed answer) rather than an ordinary tuple
            // with no further meaning of its own — `body_kind_refusals` checks the resulting arity
            // itself, with a message naming that kind's own requirement, so this check stands down
            // rather than reading the same success type as an unexplained `header_out` arity.
            if matches!(raw.body, Some((BodyKind::Bytes | BodyKind::Stream, _))) {
                return None;
            }
            let tuple_arity = if let Type::Tuple(tuple) = success.as_ref() {
                (!tuple.elems.is_empty()).then(|| tuple.elems.len())
            } else {
                None
            };
            let declared = raw.header_out.len();
            let explained =
                (tuple_arity.is_none() && declared == 0) || tuple_arity == Some(declared + 1);
            if explained {
                None
            } else if tuple_arity.is_some() && declared == 0 {
                Some(syn::Error::new(
                    success.span(),
                    tuple_without_header_out_message(operation_ident),
                ))
            } else {
                Some(syn::Error::new(
                    success.span(),
                    header_out_arity_message(operation_ident, declared),
                ))
            }
        }
    }
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

/// The message declared for one operation, or nothing where its one argument already is the
/// message.
fn generated_message(operation: &OperationDef) -> Option<GeneratedMessage> {
    let fields = match &operation.inputs {
        OperationInputs::Named(_) => return None,
        OperationInputs::Empty => Vec::new(),
        OperationInputs::Generated(arguments) => arguments.clone(),
    };
    Some(GeneratedMessage {
        declared_for: operation.ident.clone(),
        fields,
        ident: operation.generated_message_ident()?,
    })
}

fn generated_message_collision_message(
    operation: &Ident,
    declared: &Ident,
    taken: &Ident,
) -> String {
    format!(
        "service_schema: operation `{operation}` names no message, so `{declared}` is declared \
         for it, and operation `{taken}` already names a type spelled `{declared}`\n       \
         one name cannot carry two declarations; rename the operation, or have it take the \
         existing `{declared}` as its one argument"
    )
}

/// A message the macro declares lands beside the trait, so a type of that name written there
/// already would be declared twice, and the compiler would report a duplicate definition against
/// a declaration the author never wrote. What is visible from here is the service itself: a name
/// another operation writes as its message or as a result arm is a type the author declared, and
/// colliding with one is refused by name.
///
/// Two operations declaring the same message need no rule of their own — the `<Operation>Request`
/// name and the TypeScript spelling are the same derivation but for the leading letter's case, so
/// a pair that collides in one collides in the other, and the TypeScript rule above refuses it.
///
/// A type declared in the module but named nowhere in the service is out of reach of any rule
/// written here; what covers that case is the span
/// [`OperationDef::generated_message_ident`] writes, which puts the compiler's own
/// duplicate-definition report on the operation the second declaration came from.
fn generated_message_collisions(service: &ServiceDef) -> Option<syn::Error> {
    let mut refusals: Option<syn::Error> = None;
    for declared in &service.generated_messages {
        let Some(taken) = service
            .operations
            .iter()
            .filter(|other| other.ident != declared.declared_for)
            .find(|other| {
                wire_types(other)
                    .into_iter()
                    .any(|named| unqualified_name(named) == Some(&declared.ident))
            })
        else {
            continue;
        };
        refusals = Some(combined(
            refusals.take(),
            syn::Error::new(
                declared.ident.span(),
                generated_message_collision_message(
                    &declared.declared_for,
                    &declared.ident,
                    &taken.ident,
                ),
            ),
        ));
    }
    refusals
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
    if let Some(collisions) = generated_message_collisions(service) {
        refusals = Some(combined(refusals.take(), collisions));
    }
    refusals
}

/// The name a type is written with, where that name is the one that resolves in the scope a
/// generated message lands in: a path of one segment, unqualified and carrying no arguments.
/// Anything else — `crate::messages::SweepRequest`, `Vec<SweepRequest>` — names something a
/// declaration beside the trait does not collide with.
fn unqualified_name(declared: &Type) -> Option<&Ident> {
    let Type::Path(named) = declared else {
        return None;
    };
    if named.qself.is_some() || named.path.segments.len() != 1 {
        return None;
    }
    let only = named.path.segments.first()?;
    only.arguments.is_none().then_some(&only.ident)
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
