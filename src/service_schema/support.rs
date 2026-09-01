//! The two types an operation's outcome is carried in, emitted per service into the service's own
//! module: the fault a caller can receive but no implementation can answer with, and the client's
//! call-error enum.
//!
//! # Why they are generated rather than imported
//!
//! tixschema is a build-time macro crate and stays one. A service that had to depend on it at
//! runtime to name `ServiceFault` is exactly what was rejected when a marker type was proposed for
//! one-way operations, so each service gets its own copies in its own module. Two services in one
//! crate therefore carry two unrelated `ServiceFault` types, and a transport serving both answers
//! with both — that is the cost of the crate staying build-time only, and it is the cost the
//! design accepted.
//!
//! # The seal on `ServiceFault`
//!
//! The fault reports a failure the operation never declared, so no implementation answers with one:
//! an operation's signature admits only its own error type. The fields stay private, so a literal
//! written by hand is refused with `E0451` and the constructors are the only way one comes into
//! being.
//!
//! Those constructors are public. A fault is what a *dispatcher* answers a defect with, and a
//! dispatcher is not always the one emitted here — a hand-written one works against the contract
//! surface alone, and it can do nothing with a defect it has no way to report.
//!
//! `Deserialize` is deliberately not derived on the fault, so a fault never arrives simply by
//! having been written on the wire. `Serialize` is derived, because the transport has to put one
//! there. Reading one back off it is the generated client's business, and it happens through a
//! mirror that derives what the fault does not and then mints the fault through the constructors
//! published here.
//!
//! # What a dispatcher or a client expanded elsewhere reads from here
//!
//! The `Answered` envelope one writes and the other reads, the readers that turn a violation report
//! into the field and the detail a fault carries, and — one of each per operation — the name its
//! message is reached under and the validator that message runs. Both halves of a service are held
//! as macro tokens and expanded in crates that are usually not this one, so everything either of
//! them reaches is published for the same reason the fault's constructors are.
//!
//! An operation's message is republished here under `{Operation}Message` so that either half names
//! one path for every spelling: an author's own type, a type the macro declared, a type written
//! under a module of its own. It also gives two services in one crate room to receive same-named
//! messages, each module being its own namespace where the crate root is not.
//!
//! The validators are here because the fallback they run behind is: `MessageValidation` is shut in
//! a module of its own so that two blanket `validate()` methods are never in scope at once, and an
//! import written inside a transport's `macro_rules!` body is reported unused wherever the message
//! published an inherent `validate()` of its own. One per operation rather than one generic
//! function, because an inherent method only wins at a concrete type.
//!
//! # What is not here
//!
//! The `Reply` handle. Its shape is one transport model's — one reply per message, answered with a
//! value or a defect — so it travels inside a transport's own macro, and a service that asks for no
//! transport is emitted none of it.

use super::parse::{OperationDef, OperationInputs, ServiceDef};
use super::transport::Transport;
use crate::rename_rule::RenameRule;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::Ident;

/// The service's own module, holding everything either half of a service reads and nothing that
/// belongs to one of them.
///
/// # A service is declared at module scope, never inside a function body
///
/// The module opens with `use super::*;`, which is how the message aliases and the validators
/// behind them reach the trait and the message types the author declared beside it. A module
/// written inside a function body has the enclosing *module* as its parent rather than the
/// function, so `super` from there reaches past every name that function declared and finds none
/// of them. `#[model_schema]` carries the same requirement, for the same reason and through the
/// same import.
///
/// The macro cannot refuse the placement. An attribute macro is handed the annotated item's own
/// tokens and nothing about the scope it was written in, and a trait written inside a function is
/// the same tokens as one written beside it — so there is no signal to read and nothing to refuse
/// on. What a function-scoped declaration earns instead is the compiler's own resolution errors:
/// one for the trait, and one for each type an operation named.
///
/// The consequence worth knowing is that rustdoc wraps a doctest with no explicit `fn main` in one,
/// so a doctest declaring a service writes `fn main() {}` of its own. The pair below is exactly
/// that difference and nothing else. At module scope it compiles:
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
///     async fn get_balance(
///         &self,
///         ctx: &Ctx,
///         req: BalanceRequest,
///     ) -> Result<BalanceResponse, BalanceError>;
/// }
///
/// fn main() {}
/// ```
///
/// The run below is that one with `fn main() {}` taken off the end, which puts the whole
/// declaration inside the `fn main` rustdoc writes around it. Nothing else differs:
///
/// ```rust,compile_fail
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
///     async fn get_balance(
///         &self,
///         ctx: &Ctx,
///         req: BalanceRequest,
///     ) -> Result<BalanceResponse, BalanceError>;
/// }
/// ```
///
/// A `compile_fail` doctest asserts only that *some* error was raised, and an error code named in
/// the annotation is not checked on this toolchain — a deliberately wrong one still passes. So the
/// snippet above was compiled standalone as an ordinary test file, wrapped in the `fn main` rustdoc
/// would have written, and the diagnostics read off that run, verbatim. These four were all of
/// them:
///
/// ```text
/// error[E0405]: cannot find trait `UsageService` in module `super`
///   --> tests/zz_probe.rs:16:15
///    |
/// 16 |     pub trait UsageService<Ctx> {
///    |               ^^^^^^^^^^^^ not found in `super`
///
/// error[E0425]: cannot find type `BalanceRequest` in this scope
///   --> tests/zz_probe.rs:20:18
///    |
/// 20 |             req: BalanceRequest,
///    |                  ^^^^^^^^^^^^^^ not found in this scope
///
/// error[E0425]: cannot find type `BalanceResponse` in this scope
///   --> tests/zz_probe.rs:21:21
///    |
/// 21 |         ) -> Result<BalanceResponse, BalanceError>;
///    |                     ^^^^^^^^^^^^^^^ not found in this scope
///
/// error[E0425]: cannot find type `BalanceError` in this scope
///   --> tests/zz_probe.rs:21:38
///    |
/// 21 |         ) -> Result<BalanceResponse, BalanceError>;
///    |                                      ^^^^^^^^^^^^ not found in this scope
///
/// error: could not compile `tixschema` (test "zz_probe") due to 4 previous errors
/// ```
///
/// The same file with `fn main() {}` added and nothing else changed compiles, so the refusal is the
/// placement and nothing else about the program. The import those four errors resolve against is
/// what the unit test `the_generated_module_reaches_the_author_s_declarations_through_super` reads
/// back off the expansion.
pub fn emit(service: &ServiceDef, asked: &[Transport]) -> TokenStream {
    let declared = &service.ident;
    let module = module_ident(service);
    let module_doc = format!(
        "What `#[service_schema]` generates for [`{declared}`] beside the trait itself.\n\n\
         Every type here belongs to this service alone: the crate that declares `{declared}` \
         owns them, and nothing is imported from tixschema at runtime."
    );
    let fault = fault_declaration(declared);
    let call_error = call_error_declaration(declared);
    let accessors = fault_accessors();
    let constructors = fault_constructors();
    let renderings = renderings();
    let envelope = answered_envelope();
    let validation = message_validation();
    let messages = message_type_aliases(service);
    let validators = message_validators(service);
    let readers = violation_readers();
    let anchors = root_anchors(service, asked);
    quote! {
        #[doc = #module_doc]
        pub mod #module {
            use super::*;

            #fault
            #call_error
            #accessors
            #constructors
            #renderings
            #envelope
            #validation
            #messages
            #validators
            #readers
            #anchors
        }
    }
}

/// The two root names a transport's macro reaches through `$crate`, resolved here so a declaration
/// that leaves them unreachable fails in the crate that can fix it.
///
/// `#[macro_export]` hoists a transport's macro to the declaring crate's root and `$crate` reads
/// from that same root, so a service written below the root owes the root a `pub use` of its
/// generated module and one of its trait. Nothing checked that before: the declaring crate compiled
/// clean and every crate invoking the macro failed inside an expansion, in errors naming `$crate`
/// rather than the crate, pointing at the invocation rather than at the declaration, and saying
/// nothing about a re-export.
///
/// The two unnamed consts below resolve one root name each, where the service is declared. One
/// carries the module in its own type; the other carries the trait in the bound of a `PhantomData`
/// anchor no caller can reach, since the whole item lives inside the const. Nothing is instantiated
/// beyond the zero-sized value the anchor's own const holds, and neither anchor publishes a name.
///
/// # Why two unnamed consts and not an import or a published type
///
/// Every simpler spelling either resolves nothing or is reported unused, and the emitted code has
/// to be lint-clean in a crate whose lint levels are not ours to set — no `allow` and no `expect`
/// is written anywhere in what this crate emits. Measured: `use crate::{Trait as _, module as _};`
/// resolves both roots and is warned about as an unused import; a `pub use` of a module imported
/// anonymously is unused always; a function or type alias naming the roots is dead code; and a
/// **named** struct carrying the bound — public, private, or wrapped in a trait impl that builds
/// one — is `struct RootAnchor is never constructed` wherever the generated module is not itself
/// publicly reachable, which is every test binary and every service declared under a private
/// module. An unnamed const is the one item kind dead-code analysis has no name to report, so each
/// anchor is one, and the anchor type sits inside the second, where a const of its own builds it.
///
/// The pair is clean in a library, in a binary and in an integration test, under
/// `clippy::pedantic` and `clippy::nursery` alike.
///
/// # One type argument, the dispatcher's own
///
/// The trait anchor writes `crate::{Trait}<Ctx>`, the shape the dispatcher's own `where` clause
/// writes, so a trait declaring more than one type parameter earns `E0107` at the declaration
/// rather than at every consumer of a dispatcher that could never have compiled. Mirroring the
/// trait's full generics instead would let the anchor pass while the macro it stands for stayed
/// broken. The unit tests read both out of one expansion, so the two cannot drift.
///
/// # A service that asked for no transport is emitted neither
///
/// It publishes no macro, so it reaches no root and owes none, and it keeps compiling below the
/// crate root with nothing re-exported.
///
/// # The pair, both directions
///
/// The service below is declared two modules down and its crate root names neither of the two, so
/// the crate that declares it no longer builds:
///
/// ```rust,compile_fail
/// mod services {
///     pub mod usage {
///         use tixschema::service_schema;
///
///         #[derive(serde::Deserialize, serde::Serialize)]
///         pub struct BalanceRequest;
///
///         #[derive(serde::Deserialize, serde::Serialize)]
///         pub struct BalanceResponse;
///
///         #[derive(serde::Deserialize, serde::Serialize)]
///         pub enum BalanceError {
///             DbError,
///         }
///
///         #[service_schema(transports = ["amqp_rpc"])]
///         pub trait UsageService<Ctx> {
///             async fn get_balance(
///                 &self,
///                 ctx: &Ctx,
///                 req: BalanceRequest,
///             ) -> Result<BalanceResponse, BalanceError>;
///         }
///     }
/// }
///
/// fn main() {}
/// ```
///
/// The same source with one line added and nothing else changed compiles:
///
/// ```rust
/// pub use services::usage::{UsageService, usage_service_schema};
///
/// mod services {
///     pub mod usage {
///         use tixschema::service_schema;
///
///         #[derive(serde::Deserialize, serde::Serialize)]
///         pub struct BalanceRequest;
///
///         #[derive(serde::Deserialize, serde::Serialize)]
///         pub struct BalanceResponse;
///
///         #[derive(serde::Deserialize, serde::Serialize)]
///         pub enum BalanceError {
///             DbError,
///         }
///
///         #[service_schema(transports = ["amqp_rpc"])]
///         pub trait UsageService<Ctx> {
///             async fn get_balance(
///                 &self,
///                 ctx: &Ctx,
///                 req: BalanceRequest,
///             ) -> Result<BalanceResponse, BalanceError>;
///         }
///     }
/// }
///
/// fn main() {}
/// ```
///
/// # What the declaring crate is told
///
/// A `compile_fail` doctest asserts only that *some* error was raised, so the same arrangement was
/// compiled standalone as a library crate — the service in `src/services/usage.rs`, the crate root
/// naming neither — and the diagnostics read off that run, verbatim. These two were all of them,
/// one per anchor, so a crate that re-exports one of the pair is told about the other rather than
/// about both again (both measured on their own):
///
/// ```text
/// error[E0433]: cannot find `usage_service_schema` in `crate`
///   --> src/services/usage.rs:15:11
///    |
/// 14 | #[service_schema(transports = ["amqp_rpc"])]
///    | -------------------------------------------- in this attribute macro expansion
/// 15 | pub trait UsageService<Ctx> {
///    |           ^^^^^^^^^^^^ unresolved import
///    |
///    = note: this error originates in the attribute macro `service_schema` (in Nightly builds, run with -Z macro-backtrace for more info)
/// help: a similar path exists
///    |
/// 15 - pub trait UsageService<Ctx> {
/// 15 + pub trait services::usage::usage_service_schema<Ctx> {
///    |
///
/// error[E0405]: cannot find trait `UsageService` in the crate root
///   --> src/services/usage.rs:15:11
///    |
/// 14 | #[service_schema(transports = ["amqp_rpc"])]
///    | -------------------------------------------- in this attribute macro expansion
/// 15 | pub trait UsageService<Ctx> {
///    |           ^^^^^^^^^^^^ not found in the crate root
///    |
///    = help: consider importing this trait:
///            crate::services::usage::UsageService
///    = note: this error originates in the attribute macro `service_schema` (in Nightly builds, run with -Z macro-backtrace for more info)
///
/// error: could not compile `declaring` (lib) due to 2 previous errors
/// ```
///
/// The wording is rustc's, and that is a measured limit rather than a choice: nothing can attach a
/// sentence of ours to a path that does not resolve. An aliased import never prints its alias, and
/// a const-eval `panic!` that would print one has nothing it can read to tell whether the re-export
/// is missing. What the change buys is where the error lands, how early, and against which crate —
/// and the second of the two does name the `pub use` to write.
fn root_anchors(service: &ServiceDef, asked: &[Transport]) -> TokenStream {
    if asked.is_empty() {
        return TokenStream::new();
    }
    let contract = &service.ident;
    // Located at the trait's ident so the caret sits on the declaration, resolved at the call site
    // so rustc names the attribute macro that asked for a name the author never wrote.
    let anchored = contract.span().resolved_at(Span::call_site());
    let module = format_ident!("{}", module_ident(service), span = anchored);
    let bound = format_ident!("{}", contract, span = anchored);
    quote_spanned! { anchored=>
        const _: ::core::marker::PhantomData<crate::#module::ServiceFault> =
            ::core::marker::PhantomData;

        const _: () = {
            struct RootAnchor<S, Ctx>(::core::marker::PhantomData<(S, Ctx)>);

            impl<S, Ctx> RootAnchor<S, Ctx>
            where
                S: crate::#bound<Ctx>,
            {
            }

            const _: RootAnchor<(), ()> = RootAnchor(::core::marker::PhantomData);
        };
    }
}

/// The `{ ok, value }` / `{ ok, error }` envelope a request-and-reply operation answers in, which
/// the client reads back and a TypeScript caller of the same operation narrows on.
///
/// Published, with a constructor and a reader, because neither half sits in this module any more:
/// a dispatcher answers in it and a client reads one back, both from wherever they were expanded.
fn answered_envelope() -> TokenStream {
    quote! {
        /// What a request-and-reply operation puts on the wire: the envelope, with the message the
        /// operation declared left exactly as it is inside it.
        #[derive(::serde::Deserialize, ::serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        pub struct Answered<T, E> {
            #[serde(skip_serializing_if = "Option::is_none")]
            error: Option<E>,
            ok: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            value: Option<T>,
        }

        impl<T, E> Answered<T, E> {
            /// The envelope around the outcome an implementation produced.
            pub fn answering(outcome: Result<T, E>) -> Self {
                match outcome {
                    Ok(value) => Self {
                        error: None,
                        ok: true,
                        value: Some(value),
                    },
                    Err(declared) => Self {
                        error: Some(declared),
                        ok: false,
                        value: None,
                    },
                }
            }

            /// What the envelope said, for a client reading one back from outside this module.
            ///
            /// The arm is the `ok` flag's, and what it carries is `None` where the envelope
            /// contradicted itself — `ok` with no value, a failure with no error. That is a defect
            /// on the wire, which the reader answers for rather than this.
            pub fn carried(self) -> Result<Option<T>, Option<E>> {
                if self.ok {
                    Ok(self.value)
                } else {
                    Err(self.error)
                }
            }
        }
    }
}

/// What a message answers when it publishes no `validate()` of its own.
///
/// It is shut inside a module of its own and brought into scope by
/// [`message_validation_in_scope`] only in the function bodies that ask a message to validate
/// itself. A blanket `validate()` visible across the whole module would be a second candidate for
/// every `validate()` call written there — an operation's generated message type is in scope here
/// through `use super::*`, and each walks its own nested fields through a fallback of exactly this
/// shape, so two in scope at once is `E0034` on a declaration that named neither.
fn message_validation() -> TokenStream {
    quote! {
        pub mod message_validation {
            /// The answer a message with no declared constraints gives when asked to validate
            /// itself.
            ///
            /// `#[model_schema()]` writes an inherent `validate()` onto a type with constrained
            /// fields and none onto a type without one, and an inherent method takes precedence
            /// over a trait's — so a message that declared constraints runs them, and one that
            /// declared none passes here.
            pub trait MessageValidation {
                /// `Ok(())`, there being nothing declared to check.
                fn validate(&self) -> Result<(), Vec<String>> {
                    Ok(())
                }
            }

            impl<T> MessageValidation for T {}
        }
    }
}

/// Brings the fallback into scope for one function body written *inside* the module — see
/// [`message_validation`] for why it is not in scope for the module that body is written in.
pub fn message_validation_in_scope() -> TokenStream {
    quote! {
        use message_validation::MessageValidation;
    }
}

/// One name per operation for the message it receives, so a dispatcher outside this module reaches
/// every spelling of one the same way.
fn message_type_aliases(service: &ServiceDef) -> TokenStream {
    let declared = service.operations.iter().map(|operation| {
        let named = message_alias_ident(operation);
        let message = message_type(operation);
        let doc = format!("The message operation `{}` receives.", operation.wire_name);
        quote! {
            #[doc = #doc]
            pub type #named = #message;
        }
    });
    quote! {
        #(#declared)*
    }
}

/// The name an operation's validator is published under: `read_balance` becomes
/// `validated_read_balance`.
pub fn message_validator_ident(operation: &OperationDef) -> Ident {
    format_ident!(
        "validated_{}",
        operation.ident,
        span = operation.ident.span()
    )
}

/// The name an operation's message is republished under: `read_balance` becomes
/// `ReadBalanceMessage`.
pub fn message_alias_ident(operation: &OperationDef) -> Ident {
    format_ident!(
        "{}Message",
        RenameRule::PascalCase.apply_to_field(&operation.ident.to_string()),
        span = operation.ident.span()
    )
}

/// One validator per operation: the operation's message asked to validate itself, at the concrete
/// type an inherent `validate()` needs to win at.
fn message_validators(service: &ServiceDef) -> TokenStream {
    let declared = service.operations.iter().map(|operation| {
        let named = message_validator_ident(operation);
        let message = message_alias_ident(operation);
        let doc = format!(
            "What operation `{}` answers when its message is asked to validate itself.",
            operation.wire_name
        );
        let in_scope = message_validation_in_scope();
        quote! {
            #[doc = #doc]
            pub fn #named(received: &#message) -> Result<(), Vec<String>> {
                #in_scope
                received.validate()
            }
        }
    });
    quote! {
        #(#declared)*
    }
}

/// The type an operation's payload deserializes into, named as it resolves inside the module.
fn message_type(operation: &OperationDef) -> TokenStream {
    match &operation.inputs {
        OperationInputs::Named(declared) => quote! { #declared },
        OperationInputs::Empty | OperationInputs::Generated(_) => {
            let declared: Option<Ident> = operation.generated_message_ident();
            quote! { #declared }
        }
    }
}

/// The readers that turn a violation report into what a fault carries.
///
/// `named_field` reads a deserializer's refusal as well as a validator's report, so a dispatcher
/// reaches it from wherever it was expanded; the reader that classifies a refusal travels with the
/// dispatcher instead, since it is the only caller and it is the side holding the `serde_json`
/// error.
fn violation_readers() -> TokenStream {
    quote! {
        /// The field one line names, where it is written in the shape every validator
        /// `#[model_schema()]` generates: the field first and in single quotes —
        /// `'organization_id': too short: …`. A line written any other way names none.
        ///
        /// It is read off a deserializer's refusal as well as off a validator's report, because
        /// those are the same message. A field carrying a constraint gets a serde
        /// `deserialize_with` hook running the very check `validate()` runs, and the hook hands
        /// serde that check's message verbatim — so a payload refused before it ever became a
        /// message still names the field it got wrong.
        pub fn named_field(reported: &str) -> Option<&str> {
            let (field, _rest) = reported.strip_prefix('\'')?.split_once('\'')?;
            Some(field)
        }

        /// Everything that failed, in one line, for the fault's detail.
        pub fn violation_detail(reported: &[String]) -> String {
            reported.join("; ")
        }

        /// The field a violation report names, which is its first line's. A violation naming no
        /// field, as a constrained newtype's does, leaves the fault's field empty.
        pub fn violated_field(reported: &[String]) -> Option<&str> {
            named_field(reported.first()?)
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

/// What the fault's *fields* publish as in TypeScript: `UsageService` becomes
/// `UsageServiceFaultFields`.
///
/// It is also the ident the struct is declared under, a type publishing under the ident it was
/// declared with. `UsageServiceFault` belongs to the sealed type the TypeScript emitter writes over
/// these fields, so the two halves — this declaration and that alias — read one spelling and cannot
/// name different types.
pub fn fault_fields_typescript_name(declared: &str) -> String {
    format!("{declared}FaultFields")
}

/// The Rust ident the fault is declared under, which is also the name it publishes to TypeScript.
/// Read through [`fault_fields_typescript_name`] rather than spelled again, so the declaration and
/// the sealed alias the TypeScript emitter writes over it cannot name different types.
fn fault_fields_ident(declared: &Ident) -> Ident {
    format_ident!(
        "{}",
        fault_fields_typescript_name(&declared.to_string()),
        span = declared.span()
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
/// The read surface resolves from an implementation's own scope — the companion to the run on
/// [`fault_constructors`], which is this one with the read swapped for a build:
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

/// The five ways a fault comes into being, one per kind, public so that a dispatcher written
/// outside this module can build one.
///
/// A hand-written dispatcher has the contract surface and nothing else, and a fault is what it
/// answers a defect with, so the constructors have to resolve from anywhere the module is nameable.
/// This is the run beside the accessors with the read swapped for a build:
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
/// fn refuse(named: &str) -> sweep_service_schema::ServiceFault {
///     sweep_service_schema::ServiceFault::unknown_operation(named)
/// }
///
/// fn main() {}
/// ```
fn fault_constructors() -> TokenStream {
    quote! {
        impl ServiceFault {
            /// A message that failed its own schema, naming the field when the violation named one.
            #[must_use]
            pub fn failed_validation(operation: &str, field: Option<&str>, detail: &str) -> Self {
                Self {
                    detail: detail.to_owned(),
                    field: field.map(str::to_owned),
                    kind: ServiceFaultKind::FailedValidation,
                    operation: operation.to_owned(),
                }
            }

            /// A handler that came apart. The transport still settles the delivery.
            #[must_use]
            pub fn handler_panic(operation: &str, detail: &str) -> Self {
                Self {
                    detail: detail.to_owned(),
                    field: None,
                    kind: ServiceFaultKind::HandlerPanic,
                    operation: operation.to_owned(),
                }
            }

            /// A call that never travelled, in the words the transport reported it in.
            #[must_use]
            pub fn transport_failure(operation: &str, detail: &str) -> Self {
                Self {
                    detail: detail.to_owned(),
                    // The transport reports that the call did not travel, not that a value inside
                    // it was wrong, so there is no field to name.
                    field: None,
                    kind: ServiceFaultKind::TransportFailure,
                    operation: operation.to_owned(),
                }
            }

            /// Bytes that were not the document the operation reads its message out of.
            #[must_use]
            pub fn undeserializable_payload(operation: &str, detail: &str) -> Self {
                Self {
                    detail: detail.to_owned(),
                    // `refused_payload` is the only caller, and it sends here only the refusals
                    // serde_json classified as the bytes not being a document: bytes that are not
                    // JSON at all, a document that ends early. Nothing was read far enough for a
                    // key to be what went wrong, so there is no field to name.
                    field: None,
                    kind: ServiceFaultKind::UndeserializablePayload,
                    operation: operation.to_owned(),
                }
            }

            /// An operation this service answers to under no name.
            #[must_use]
            pub fn unknown_operation(operation: &str) -> Self {
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
/// needs to put a fault on the wire, and neither derives `Deserialize`, so a fault never arrives
/// simply by having been written there.
///
/// Both also carry `#[model_schema()]`, so the TypeScript a caller narrows on comes from this
/// declaration rather than from a literal written beside it: one wire, one source. `model_schema`
/// writes reading surfaces only — a TypeScript string, a schema — so it widens nothing: the fields
/// stay private, no `Deserialize` appears, and the constructors below remain the only way a fault
/// comes into being.
///
/// # Two names for one type, and why
///
/// The declaration carries `UsageServiceFaultFields`, and a type publishes under the ident it was
/// declared with, so that is what its TypeScript is called. The prefix is there because TypeScript
/// has no per-service scope — a bundle is one flat file, and a consuming codebase with ten services
/// would otherwise declare one fault type ten times over and not compile. The `Fields` is there
/// because the name a TypeScript *caller* reads, `UsageServiceFault`, belongs to the sealed type
/// the TypeScript emitter writes over these fields: the same members plus a brand keyed on a symbol
/// the bundle exports nowhere. That brand is what stops a TypeScript implementation writing a fault
/// as an object literal, the way private fields stop a Rust one with `E0451`. In Rust there is
/// nothing to draw that distinction against — the fields below are private whatever the
/// constructors publish — so Rust has the one type and TypeScript has the two names.
///
/// The fields themselves come from this declaration and from nowhere else, in both languages, which
/// is what keeps the type a caller narrows on and the value the wire carries from drifting apart.
///
/// Rust needs no prefix at all, this module being the scope TypeScript lacks, so `ServiceFault` is
/// bound beside it as an alias — the unstuttering spelling everything generated here writes, and
/// the one a transport implementing a reply handle names. An alias reaches Rust alone and publishes
/// nothing, so the flat name stays claimed exactly once per service.
///
/// The kind is declared before the fault that carries it, so the field walk resolves its name off
/// the registry rather than falling back to a spelling written before the type expanded.
fn fault_declaration(declared: &Ident) -> TokenStream {
    let fields = fault_fields_ident(declared);
    let kind = format_ident!("{declared}FaultKind", span = declared.span());
    let fault_doc = format!(
        "A failure `{declared}` never declared: a payload that would not deserialize, a message \
         that failed validation, an operation name nothing recognises, a handler that panicked, a \
         call the transport could not carry.\n\n\
         It is a defect rather than a condition, so it is logged at error level and meant to page \
         a human. No implementation of [`{declared}`] answers with one — an operation's signature \
         admits only its own error type — and its fields are private, so the constructors below \
         are the only way one comes into being.\n\n\
         In TypeScript this publishes as `{fields}`, which is the fault's fields and nothing else. \
         What a caller reads there is `{declared}Fault`: those same fields under a brand the bundle \
         declares and exports nowhere, so an object written by hand is not one. That brand is the \
         TypeScript answer to the private fields above."
    );
    let kind_doc = format!("Which kind of defect a `{declared}` fault reports.");
    let alias_doc = format!(
        "The fault under the name everything generated inside this module writes. `{fields}` is \
         the same type, declared under the name its *fields* are published as in TypeScript — this \
         module is the scope TypeScript has no equivalent of."
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
        pub struct #fields {
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
        pub type ServiceFault = #fields;

        #[doc = #kind_alias_doc]
        pub type ServiceFaultKind = #kind;
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
