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
//! # Why `Reply` has three methods
//!
//! Acknowledging and replying are one act on the transport this has to live on. Every
//! acknowledgement in the message bus this was measured against sits inside a send, there is no
//! `nack`, and the consumer that one-way traffic reaches asks for manual acknowledgement with no
//! dead-letter exchange, no message TTL and no timeout behind it. A one-way operation that
//! returned without touching the handle would leave its delivery unacknowledged forever and stall
//! the consumer against its prefetch, so `done` settles the message and publishes nothing. `send`
//! takes a serializable value rather than bytes because the transport mutates what it is handed
//! before serializing — an error flag and the correlation id go in — and neither is reachable
//! behind an encoded buffer.

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
    // The trait ident snake-cased under the same `_schema` suffix a `#[model_schema]` type's
    // generated module carries. Derived through `RenameRule`, as the other two spellings of an
    // operation's name are, rather than through the casing helper in `utils` — that one is gated
    // on the surface features, and this module carries nothing a feature writes.
    let module = format_ident!(
        "{}_schema",
        RenameRule::SnakeCase.apply_to_variant(&declared.to_string())
    );
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

            /// The field that failed validation, and `None` for every other kind.
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

/// The four ways a fault comes into being, one per kind, private to the generated module so that
/// only the dispatcher and the client — the two emitters written inside it — can reach them.
///
/// A service implementation cannot construct one. This is the run above with a single line added,
/// so the refusal can only be the constructor's privacy:
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

            fn undeserializable_payload(operation: &str, detail: &str) -> Self {
                Self {
                    detail: detail.to_owned(),
                    field: None,
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
fn fault_declaration(declared: &Ident) -> TokenStream {
    let fault_doc = format!(
        "A failure `{declared}` never declared: a payload that would not deserialize, a message \
         that failed validation, an operation name nothing recognises, a handler that \
         panicked.\n\n\
         It is a defect rather than a condition, so it is logged at error level and meant to page \
         a human. No implementation of [`{declared}`] can produce one — an operation's signature \
         admits only its own error type, and the constructors are private to this module, which \
         only the generated dispatcher and the generated client are written inside."
    );
    quote! {
        #[doc = #fault_doc]
        #[derive(Clone, Debug, Eq, PartialEq, ::serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        pub struct ServiceFault {
            detail: String,
            field: Option<String>,
            kind: ServiceFaultKind,
            operation: String,
        }

        /// Which kind of defect a [`ServiceFault`] reports.
        #[derive(Clone, Copy, Debug, Eq, PartialEq, ::serde::Serialize)]
        #[serde(rename_all = "kebab-case")]
        pub enum ServiceFaultKind {
            /// A message reached its operation and did not satisfy the operation's schema. The
            /// fault names the field that failed.
            FailedValidation,
            /// The operation's handler panicked.
            HandlerPanic,
            /// The payload would not deserialize into the operation's message at all.
            UndeserializablePayload,
            /// Nothing on this service answers to the operation name that arrived.
            UnknownOperation,
        }
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
///     async fn done(&self) {
///         self.settled.lock().unwrap().push("settled, nothing published".to_owned());
///     }
///
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
         Exactly one of the three is called for every message: a request-and-reply operation \
         answers with [`send`](Reply::send) or [`fault`](Reply::fault), a one-way operation \
         settles with [`done`](Reply::done). Encoding sits behind the trait, which is what keeps \
         the generator out of the wire format."
    );
    quote! {
        #[doc = #reply_doc]
        pub trait Reply {
            /// Settle this message and publish nothing. What a one-way operation calls, and the
            /// only way a delivery carrying no reply destination gets acknowledged.
            fn done(&self) -> impl ::core::future::Future<Output = ()> + Send;

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
