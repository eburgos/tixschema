//! The three types an operation's outcome is carried in, emitted per service into the service's
//! own module: the fault a caller can receive but no implementation can construct, the reply
//! handle a transport implements, and the client's call-error enum.
//!
//! # Why they are generated rather than imported
//!
//! tixschema is a build-time macro crate and stays one. A service that had to depend on it at
//! runtime to name `ServiceFault` is exactly what was rejected when a marker type was proposed for
//! one-way operations, so each service gets its own copies in its own module. Two services in one
//! crate therefore carry two unrelated `ServiceFault` types and two unrelated `Reply` traits, and
//! a transport serving both implements both — that is the cost of the crate staying build-time
//! only, and it is the cost the design accepted.
//!
//! # The seal on `ServiceFault`
//!
//! The fault reports a failure the operation never declared, so no implementation may produce one:
//! an operation's signature admits only its own error type, and the constructors are private to
//! the generated module. `pub(crate)` would not do it — the macro expands inside the consuming
//! crate, so a service written beside its own trait would reach a `pub(crate)` constructor, and so
//! would the compile-fail test that has to prove it cannot.
//!
//! `Deserialize` is deliberately not derived on the fault, for the same reason: a public
//! `Deserialize` is a public constructor, and an author holding one could report a defect the
//! operation never declared. `Serialize` is derived, because the transport has to put a fault on
//! the wire. Reading one back off the wire is the generated client's business and therefore
//! happens inside the module, where the constructors are.
//!
//! **The dispatcher and the Rust client are emitted inside this module**, which is why [`emit`]
//! takes their tokens rather than being spliced beside them. They are the only two places a fault
//! is built, so the constructors below stay private and reachable from nowhere else. The module
//! also opens with `use super::*;`, since both of them name the trait's own message types, which
//! the author declared beside the trait.
//!
//! # Why `Reply` has exactly two methods
//!
//! Replying is all the handle does. A request-and-reply operation answers with `send` or `fault`;
//! a one-way operation never touches it at all, so nothing about replying appears on a path
//! that never replies. Acknowledgement is the transport's: `dispatch` returns nothing, so the
//! adapter that called it still holds the delivery when the arm finishes and acknowledges there,
//! for every message including a one-way one. That placement matters on the bus this was measured
//! against, where every acknowledgement sits inside a send, there is no `nack`, and the consumer
//! one-way traffic reaches asks for manual acknowledgement with no dead-letter exchange, no
//! message TTL and no timeout behind it. `send` takes a serializable value rather than bytes
//! because the transport mutates what it is handed before serializing — an error flag and the
//! correlation id go in — and neither is reachable behind an encoded buffer.

use super::parse::ServiceDef;
use crate::rename_rule::RenameRule;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

/// `generated` is what [`dispatch`](super::dispatch) and [`client`](super::client) wrote: it
/// lands inside the module rather than beside it, because both reach constructors that are private
/// to it.
pub fn emit(service: &ServiceDef, generated: &TokenStream) -> TokenStream {
    let declared = &service.ident;
    let module = module_ident(service);
    let module_doc = format!(
        "What `#[service_schema]` generates for [`{declared}`] beside the trait itself.\n\n\
         Every type here belongs to this service alone: the crate that declares `{declared}` \
         owns them, and nothing is imported from tixschema at runtime."
    );
    let fault = fault_declaration(declared);
    let call_error = call_error_declaration(declared);
    let reply = reply_declaration(declared);
    let accessors = fault_accessors();
    let constructors = fault_constructors();
    let renderings = renderings();
    quote! {
        #[doc = #module_doc]
        pub mod #module {
            use super::*;

            #fault
            #call_error
            #reply
            #accessors
            #constructors
            #renderings
            #generated
        }
    }
}

/// The module a service's generated types land in: `UsageService` becomes `usage_service_schema`.
///
/// The trait ident snake-cased under the same `_schema` suffix a `#[model_schema]` type's
/// generated module carries. Derived through `RenameRule`, as the other two spellings of an
/// operation's name are, rather than through the casing helper in `utils` — that one is gated on
/// the surface features, and this module carries nothing a feature writes. Public because the
/// TypeScript emitters name types inside it and must spell it the same way.
pub fn module_ident(service: &ServiceDef) -> Ident {
    format_ident!(
        "{}_schema",
        RenameRule::SnakeCase.apply_to_variant(&service.ident.to_string())
    )
}

/// `CallError<E>`, the failure arm of every generated client call.
///
/// A call site matches at both levels, which is the price of not pretending a fault is an ordinary
/// error:
///
/// ```rust
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct BalanceRequest;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct BalanceResponse;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum BalanceError {
///     DbError,
/// }
///
/// #[service_schema()]
/// pub trait UsageService<Ctx> {
///     async fn get_available_balance(
///         &self,
///         ctx: &Ctx,
///         req: BalanceRequest,
///     ) -> Result<BalanceResponse, BalanceError>;
/// }
///
/// use usage_service_schema::CallError;
///
/// fn acted_on(answered: Result<BalanceResponse, CallError<BalanceError>>) -> &'static str {
///     match answered {
///         Ok(_balance) => "rendered",
///         Err(CallError::Operation(BalanceError::DbError)) => "retried later",
///         Err(CallError::Fault(_defect)) => "reported, and a human paged",
///     }
/// }
///
/// // Declared at module scope, which is where the generated module reaches for them.
/// fn main() {}
/// ```
fn call_error_declaration(declared: &Ident) -> TokenStream {
    let call_error_doc = format!(
        "What a `{declared}` client returns in the failure position, a call having three \
         outcomes where `Result` has two arms."
    );
    quote! {
        #[doc = #call_error_doc]
        ///
        /// [`Operation`](CallError::Operation) is the error the operation declared — the thing it
        /// said it could fail at. [`Fault`](CallError::Fault) means a defect reached the caller.
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum CallError<E> {
            /// A defect reached the caller: the remote produced a fault, or the client refused the
            /// message it was about to send.
            Fault(ServiceFault),
            /// The error the operation declared.
            Operation(E),
        }
    }
}

/// What a receiver reads off a fault. Everything a fault carries is readable and nothing about it
/// is writable, which is the whole point of the type.
///
/// The read surface resolves from an implementation's own scope — the companion to the
/// compile-fail run on [`fault_constructors`], which differs from this only by reaching for a
/// constructor:
///
/// ```rust
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct PurgeRequest;
///
/// #[service_schema()]
/// pub trait SweepService<Ctx> {
///     #[service_schema_op(one_way)]
///     async fn purge(&self, ctx: &Ctx, req: PurgeRequest);
/// }
///
/// pub struct SweepBackEnd;
///
/// impl SweepService<()> for SweepBackEnd {
///     async fn purge(&self, _ctx: &(), _req: PurgeRequest) {}
/// }
///
/// fn log_line(fault: &sweep_service_schema::ServiceFault) -> String {
///     format!(
///         "{} in `{}` at {:?}: {}",
///         fault.kind(),
///         fault.operation(),
///         fault.field(),
///         fault.detail(),
///     )
/// }
///
/// fn main() {}
/// ```
fn fault_accessors() -> TokenStream {
    quote! {
        impl ServiceFault {
            /// What went wrong, in words, for the log line a receiver writes.
            #[must_use]
            pub fn detail(&self) -> &str {
                &self.detail
            }

            /// The field the failure named, where it named one.
            ///
            /// A message that failed validation names the field that failed, and so does a
            /// payload refused by the serde hook a constrained field carries — that hook runs
            /// the same check and reports it in the same words. Everything else leaves it empty:
            /// a payload refused for its shape rather than its values, an operation name nothing
            /// answers to, a handler that panicked.
            #[must_use]
            pub fn field(&self) -> Option<&str> {
                self.field.as_deref()
            }

            /// Which kind of defect this is.
            #[must_use]
            pub const fn kind(&self) -> ServiceFaultKind {
                self.kind
            }

            /// The operation involved, on the wire. For
            /// [`UnknownOperation`](ServiceFaultKind::UnknownOperation) it is the name that
            /// arrived and nothing answered to.
            #[must_use]
            pub fn operation(&self) -> &str {
                &self.operation
            }
        }
    }
}

/// The five ways a fault comes into being, one per kind, private to the generated module so that
/// only the dispatcher and the client — the two emitters written inside it — can reach them.
///
/// A service implementation cannot construct one. This is the run above with a single line added,
/// so the refusal can only be the constructor's privacy. Which refusal it is was read by compiling
/// the snippet on its own rather than assumed: `E0624`, the associated function being private.
/// `compile_fail` asserts only that *some* error is raised, and this toolchain does not check the
/// error code a doctest names.
///
/// ```rust,compile_fail
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct PurgeRequest;
///
/// #[service_schema()]
/// pub trait SweepService<Ctx> {
///     #[service_schema_op(one_way)]
///     async fn purge(&self, ctx: &Ctx, req: PurgeRequest);
/// }
///
/// pub struct SweepBackEnd;
///
/// impl SweepService<()> for SweepBackEnd {
///     async fn purge(&self, _ctx: &(), _req: PurgeRequest) {
///         let _refused = sweep_service_schema::ServiceFault::unknown_operation("purge");
///     }
/// }
///
/// fn main() {}
/// ```
fn fault_constructors() -> TokenStream {
    quote! {
        impl ServiceFault {
            fn failed_validation(operation: &str, field: Option<&str>, detail: &str) -> Self {
                Self {
                    detail: detail.to_owned(),
                    field: field.map(str::to_owned),
                    kind: ServiceFaultKind::FailedValidation,
                    operation: operation.to_owned(),
                }
            }

            fn handler_panic(operation: &str, detail: &str) -> Self {
                Self {
                    detail: detail.to_owned(),
                    field: None,
                    kind: ServiceFaultKind::HandlerPanic,
                    operation: operation.to_owned(),
                }
            }

            fn transport_failure(operation: &str, detail: &str) -> Self {
                Self {
                    detail: detail.to_owned(),
                    // The transport reports that the call did not travel, not that a value inside
                    // it was wrong, so there is no field to name.
                    field: None,
                    kind: ServiceFaultKind::TransportFailure,
                    operation: operation.to_owned(),
                }
            }

            fn undeserializable_payload(operation: &str, detail: &str) -> Self {
                Self {
                    detail: detail.to_owned(),
                    // A field carrying a constraint is refused by a serde hook running the very
                    // check `validate()` runs, and the hook hands serde that check's message
                    // verbatim. So the refusal that stopped the payload from ever becoming a
                    // message still names the field it got wrong, and reading it back is reading
                    // the same report a violation is read from. A refusal written any other way —
                    // a type mismatch, a missing key — names none.
                    field: named_field(detail).map(str::to_owned),
                    kind: ServiceFaultKind::UndeserializablePayload,
                    operation: operation.to_owned(),
                }
            }

            fn unknown_operation(operation: &str) -> Self {
                Self {
                    detail: "the service answers to no operation by that name".to_owned(),
                    field: None,
                    kind: ServiceFaultKind::UnknownOperation,
                    operation: operation.to_owned(),
                }
            }
        }
    }
}

/// `ServiceFault` and the kind it reports. Both derive `Serialize`, which is what the transport
/// needs to put a fault on the wire, and neither derives `Deserialize`, which would be a public
/// constructor by another name.
///
/// Both also carry `#[model_schema()]`, so the TypeScript a caller narrows on comes from this
/// declaration rather than from a literal written beside it: one wire, one source. `model_schema`
/// writes reading surfaces only — a TypeScript string, a schema — so it widens nothing: the fields
/// stay private, no `Deserialize` appears, and the constructors below remain the only way a fault
/// comes into being.
///
/// # Two names for one type, and why
///
/// The declaration carries the service's own name, `UsageServiceFault`, because that is the name
/// its TypeScript is published under and a type publishes under the ident it was declared with.
/// TypeScript has no per-service scope: a bundle is one flat file, and a consuming codebase with
/// ten services would otherwise declare `ServiceFault` ten times over and not compile.
///
/// Rust needs no such prefix, this module being the scope TypeScript lacks, so `ServiceFault` is
/// bound beside it as an alias — the unstuttering spelling everything generated here writes, and
/// the one a transport implementing [`Reply`](reply_declaration) names. An alias reaches Rust
/// alone and publishes nothing, so the flat name stays claimed exactly once per service.
///
/// The kind is declared before the fault that carries it, so the field walk resolves its name off
/// the registry rather than falling back to a spelling written before the type expanded.
fn fault_declaration(declared: &Ident) -> TokenStream {
    let fault = format_ident!("{declared}Fault", span = declared.span());
    let kind = format_ident!("{declared}FaultKind", span = declared.span());
    let fault_doc = format!(
        "A failure `{declared}` never declared: a payload that would not deserialize, a message \
         that failed validation, an operation name nothing recognises, a handler that panicked, a \
         call the transport could not carry.\n\n\
         It is a defect rather than a condition, so it is logged at error level and meant to page \
         a human. No implementation of [`{declared}`] can produce one — an operation's signature \
         admits only its own error type, and the constructors are private to this module, which \
         only the generated dispatcher and the generated client are written inside."
    );
    let kind_doc = format!("Which kind of defect a `{declared}` fault reports.");
    let alias_doc = format!(
        "The fault under the name everything generated inside this module writes. `{fault}` is \
         the same type, declared under the name its TypeScript is published as — this module is \
         the scope TypeScript has no equivalent of."
    );
    let kind_alias_doc =
        format!("Which kind of defect a [`ServiceFault`] reports. The same type as [`{kind}`].");
    quote! {
        #[doc = #kind_doc]
        #[::tixschema::model_schema()]
        #[derive(Clone, Copy, Debug, Eq, PartialEq, ::serde::Serialize)]
        #[serde(rename_all = "kebab-case")]
        pub enum #kind {
            /// A message reached its operation and did not satisfy the operation's schema. The
            /// fault names the field that failed.
            FailedValidation,
            /// The operation's handler panicked.
            HandlerPanic,
            /// The transport could not carry the call: the message did not go out, or the reply
            /// never came back. Only a client reports one — the far side, by definition, was
            /// never reached.
            TransportFailure,
            /// The payload would not deserialize into the operation's message at all.
            UndeserializablePayload,
            /// Nothing on this service answers to the operation name that arrived.
            UnknownOperation,
        }

        #[doc = #fault_doc]
        #[::tixschema::model_schema()]
        #[derive(Clone, Debug, Eq, PartialEq, ::serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        pub struct #fault {
            detail: String,
            // Omitted rather than written as `null` when there is no field to name, which is the
            // same convention the reply envelope follows and what lets the generated TypeScript
            // spell it `string | undefined` and be right about the wire.
            #[serde(skip_serializing_if = "Option::is_none")]
            field: Option<String>,
            kind: #kind,
            operation: String,
        }

        #[doc = #alias_doc]
        pub type ServiceFault = #fault;

        #[doc = #kind_alias_doc]
        pub type ServiceFaultKind = #kind;
    }
}

/// The `Reply` trait, which a transport implements once per service it serves.
///
/// It is implementable by hand — an `async fn` satisfies each returned future, and the value
/// `send` is handed is serialized by the transport rather than by the generator:
///
/// ```rust
/// use std::sync::Mutex;
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct PurgeRequest;
///
/// #[service_schema()]
/// pub trait SweepService<Ctx> {
///     #[service_schema_op(one_way)]
///     async fn purge(&self, ctx: &Ctx, req: PurgeRequest);
/// }
///
/// /// A transport that writes down what it was asked to do instead of publishing it.
/// pub struct ProbeTransport {
///     settled: Mutex<Vec<String>>,
/// }
///
/// impl sweep_service_schema::Reply for ProbeTransport {
///     async fn fault(&self, fault: sweep_service_schema::ServiceFault) {
///         self.settled.lock().unwrap().push(fault.to_string());
///     }
///
///     async fn send<T>(&self, value: T)
///     where
///         T: serde::Serialize + Send,
///     {
///         self.settled.lock().unwrap().push(serde_json::to_string(&value).unwrap());
///     }
/// }
///
/// fn main() {}
/// ```
fn reply_declaration(declared: &Ident) -> TokenStream {
    let reply_doc = format!(
        "The handle a transport gives the `{declared}` dispatcher so it can settle *this* \
         message.\n\n\
         A request-and-reply operation answers with [`send`](Reply::send) or \
         [`fault`](Reply::fault). A one-way operation calls neither, and the transport \
         acknowledges the delivery after dispatch returns. Encoding sits behind the trait, which \
         is what keeps the generator out of the wire format."
    );
    quote! {
        #[doc = #reply_doc]
        pub trait Reply {
            /// Answer with a defect the operation never declared.
            fn fault(
                &self,
                fault: ServiceFault,
            ) -> impl ::core::future::Future<Output = ()> + Send;

            /// Answer with a value. The transport serializes it, which is why it is handed the
            /// value rather than an encoded buffer.
            fn send<T>(&self, value: T) -> impl ::core::future::Future<Output = ()> + Send
            where
                T: ::serde::Serialize + Send;
        }
    }
}

/// How a fault and a call error read in a log line. A fault is meant to page a human, so the one
/// line it renders to names the kind, the operation and the field before the detail.
fn renderings() -> TokenStream {
    quote! {
        impl ::core::fmt::Display for ServiceFault {
            fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                match self.field.as_deref() {
                    Some(named) => ::core::write!(
                        formatter,
                        "{} in operation `{}`, field `{}`: {}",
                        self.kind,
                        self.operation,
                        named,
                        self.detail
                    ),
                    None => ::core::write!(
                        formatter,
                        "{} in operation `{}`: {}",
                        self.kind,
                        self.operation,
                        self.detail
                    ),
                }
            }
        }

        impl ::core::error::Error for ServiceFault {}

        impl ::core::fmt::Display for ServiceFaultKind {
            fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                formatter.write_str(match *self {
                    Self::FailedValidation => "failed validation",
                    Self::HandlerPanic => "handler panic",
                    Self::TransportFailure => "transport failure",
                    Self::UndeserializablePayload => "undeserializable payload",
                    Self::UnknownOperation => "unknown operation",
                })
            }
        }

        impl<E: ::core::fmt::Display> ::core::fmt::Display for CallError<E> {
            fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                match *self {
                    Self::Fault(ref fault) => ::core::fmt::Display::fmt(fault, formatter),
                    Self::Operation(ref declared) => ::core::fmt::Display::fmt(declared, formatter),
                }
            }
        }
    }
}
