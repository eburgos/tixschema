//! `#[service_schema]`: a service declared once as a trait, read once, and handed to the emitters.
//!
//! [`parse`] reads and validates the declared trait into a
//! [`ServiceDef`](parse::ServiceDef) — the representation everything below consumes and nothing
//! below re-derives. The trait itself is emitted here, as declared save for the `async fn`
//! desugaring the compiler asks for; every other artifact belongs to one of the emitter modules,
//! each of which is landed by its own task.
//!
//! # The construct exists only where the `serde` feature does
//!
//! Everything a service emits leans on serde the crate unconditionally — every declared message
//! and both fault types derive `Serialize` and `Deserialize`, and the dispatcher reads its payload
//! through `serde_json`. What tixschema's own `serde` feature decides is whether the *describing*
//! surfaces read serde attributes, and the macro writes those attributes itself: `camelCase` onto
//! every message it declares, `kebab-case` onto the fault kind. Without the feature a service puts
//! those keys on the wire and publishes TypeScript naming the Rust idents instead — two halves of
//! one service disagreeing about the wire between them, and nothing failing to compile.
//!
//! So the feature's absence is a build failure rather than a silent lie, and the emitters below
//! are gated on it: in a build without it there is no dispatcher, no client and no supporting type
//! to emit, and [`serde_feature_refusal`] `parse`
//! is the whole of what a declaration produces. `parse` is gated with them: how a service is
//! *written* only matters where one can exist, and nothing in that reading is feature-dependent
//! anyway — the refusals it produces are read by its own unit tests, in every build that has the
//! feature.
//!
//! # What a crate declaring a service names in its own manifest
//!
//! Three runtime crates, because the generated code calls them: `serde`, `serde_json` and
//! `tracing`. tixschema itself is build-time only and none of the three is reached through it —
//! a service names them the way it names any dependency of code it compiles.
//!
//! `tracing` is the newest of the three and is there so that a caught panic is written down. A
//! dispatcher catches a panicking handler in order to return, so the transport can settle the
//! delivery; catching without recording would trade a stalled consumer for a silent one, so the
//! arm emits `tracing::error!` naming the operation and what the panic said.
//!
//! It costs a consumer that wants no logging very little. With no subscriber installed the
//! callsite registers against `NoSubscriber`, whose `register_callsite` answers `Interest::never()`
//! and whose `enabled` answers `false`, so the event is never built. The crate itself pulls in
//! `tracing-core` (and its `once_cell`), `pin-project-lite`, and the `tracing-attributes` proc
//! macro, which brings the same `proc-macro2`/`quote`/`syn` tixschema already builds against —
//! nothing more. The machinery that formats and writes records lives in `tracing-subscriber`,
//! which is not named here and which only a service that wants output adds.
//!
//! A crate that forgets it earns one error, spanned on the declaration, naming the crate:
//! `error[E0433]: cannot find `tracing` in the crate root`. Forgetting `serde_json` earns seven.
//!
//! **`#[model_schema]` requires none of this.** Only a declared service emits a dispatcher, so a
//! crate that describes types and declares no service names no `tracing` and reaches no logger.

#[cfg(feature = "serde")]
mod client;
#[cfg(feature = "serde")]
mod dispatch;
#[cfg(feature = "serde")]
mod messages;
#[cfg(feature = "serde")]
pub mod parse;
#[cfg(feature = "serde")]
pub mod support;
#[cfg(feature = "serde")]
pub mod transport;

#[cfg(all(feature = "serde", feature = "typescript"))]
use crate::features::service_schema::emit as emit_typescript;
use proc_macro2::TokenStream;
#[cfg(feature = "serde")]
use quote::quote;
use syn::ItemTrait;
#[cfg(feature = "serde")]
use syn::{ReturnType, TraitItem};

/// A build without the `serde` feature answers a declaration with refusals and nothing else.
///
/// Nothing further would be truthful. The emitters are gated out of this build, so there is no
/// dispatcher, no client and no supporting type to write, and the trait on its own would resolve an
/// `impl` block while every call site still failed on a module that was never emitted.
///
/// One sentence naming the feature is the whole of what this configuration has to say, and saying
/// more would mean reading a declaration for a construct this build cannot have.
#[cfg(not(feature = "serde"))]
pub fn exec_service_schema(_args: TokenStream, input: TokenStream) -> TokenStream {
    match syn::parse2::<ItemTrait>(input) {
        Ok(declared) => serde_feature_refusal(&declared),
        Err(rejection) => rejection.to_compile_error(),
    }
}

#[cfg(feature = "serde")]
pub fn exec_service_schema(args: TokenStream, input: TokenStream) -> TokenStream {
    let asked = transport::parse_transports(args);
    let declared = match syn::parse2::<ItemTrait>(input) {
        Ok(parsed) => parsed,
        Err(rejection) => return rejection.to_compile_error(),
    };
    // The trait is emitted whether or not it validates, so a service with one bad operation
    // reports that operation rather than burying it under an unresolved trait name at every
    // implementation and every call site.
    let contract = emitted_trait(&declared);
    // Both readings are answered together, so a service that asks for a transport nobody has and
    // declares a bad operation is told about each rather than about whichever was read first.
    match (asked, parse::parse_service(&declared)) {
        // The transports the service asked for reach no emitter yet; the reading is what refuses a
        // list nobody could honour.
        (Ok(_), Ok(service)) => {
            let messages = messages::emit(&service);
            // The dispatcher and the client land inside the module `support` opens: they are the
            // only two builders of a fault, and the constructors are private to it.
            let inside = [dispatch::emit(&service), client::emit(&service)];
            let support = support::emit(&service, &quote! { #(#inside)* });
            // The TypeScript artifacts are strings rather than callers of anything private, so they
            // stay at the trait's scope where a bundle can name them.
            let typescript = typescript(&service);
            quote! {
                #messages
                #support
                #contract
                #typescript
            }
        }
        (transports, read) => {
            let refusals: TokenStream = [transports.err(), read.err()]
                .into_iter()
                .flatten()
                .map(|refusal| refusal.to_compile_error())
                .collect();
            quote! {
                #refusals
                #contract
            }
        }
    }
}

/// What a build without the `serde` feature is told, and why it is told rather than worked around.
///
/// A service already cannot function without serde the crate: every message the macro declares
/// derives `Serialize` and `Deserialize`, both fault types do, and the dispatcher reads its payload
/// through `serde_json`. None of that is behind a `cfg`. What tixschema's own `serde` feature
/// controls is whether the *describing* surfaces read serde attributes — and the macro writes those
/// attributes itself, `rename_all = "camelCase"` onto every declared message and
/// `rename_all = "kebab-case"` onto the fault kind. So a build without the feature puts camelCase
/// and kebab-case on the wire and publishes TypeScript naming the Rust idents, and the two halves
/// of one service disagree about the wire between them.
///
/// The refusal rides *beside* the whole expansion rather than replacing it. Everything the macro
/// writes still compiles in this configuration — that is exactly why the disagreement went
/// unnoticed — so emitting it keeps every name a caller wrote resolvable and leaves this the only
/// error the build reports. Replacing the expansion would bury the sentence that explains the
/// problem under an unresolved name at every implementation and every call site.
#[cfg(not(feature = "serde"))]
fn serde_feature_refusal(declared: &ItemTrait) -> TokenStream {
    syn::Error::new(
        declared.ident.span(),
        "service_schema: a service needs tixschema's `serde` feature, and this build does not \
         have it\n       without it the TypeScript a service publishes names Rust fields rather \
         than the camelCase\n       and kebab-case keys its own dispatcher writes, so the two \
         halves disagree about the wire\n       add `features = [\"serde\"]` to the tixschema \
         dependency in Cargo.toml",
    )
    .to_compile_error()
}

/// The trait as the author declared it, less the per-operation directives, with every `async fn`
/// desugared to the `-> impl Future + Send` the `async_fn_in_trait` warning recommends writing.
///
/// Gated with the rest of the construct: a build without the `serde` feature answers a declaration
/// with the refusal alone, so there is no trait to emit and no example here that could compile.
///
/// # Adding an operation breaks every implementation, which is the whole reason for a trait
///
/// The defect the construct exists to prevent is an operation that is declared, typed into a
/// response union, and implemented by nobody. Only a compiler refusing the incomplete
/// implementation prevents it, so there are no default bodies and no way to mark an operation
/// provisional. An implementation covering every operation compiles:
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
///
///     async fn sweep(&self, ctx: &Ctx) -> Result<BalanceResponse, BalanceError>;
/// }
///
/// pub struct UsageBackEnd;
///
/// impl UsageService<()> for UsageBackEnd {
///     async fn get_balance(
///         &self,
///         _ctx: &(),
///         _req: BalanceRequest,
///     ) -> Result<BalanceResponse, BalanceError> {
///         Ok(BalanceResponse)
///     }
///
///     async fn sweep(&self, _ctx: &()) -> Result<BalanceResponse, BalanceError> {
///         Ok(BalanceResponse)
///     }
/// }
///
/// fn main() {}
/// ```
///
/// The run below is that one with `sweep` taken out of the implementation and left on the trait,
/// and nothing else changed, so the refusal can only be that operation's absence:
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
///
///     async fn sweep(&self, ctx: &Ctx) -> Result<BalanceResponse, BalanceError>;
/// }
///
/// pub struct UsageBackEnd;
///
/// impl UsageService<()> for UsageBackEnd {
///     async fn get_balance(
///         &self,
///         _ctx: &(),
///         _req: BalanceRequest,
///     ) -> Result<BalanceResponse, BalanceError> {
///         Ok(BalanceResponse)
///     }
/// }
///
/// fn main() {}
/// ```
///
/// A `compile_fail` doctest asserts only that *some* error was raised, and an error code named in
/// the annotation is not checked on this toolchain — a deliberately wrong one still passes. So the
/// snippet above was compiled standalone as an ordinary test file and the diagnostic read off that
/// run, verbatim, and it was the **only** error the file earned:
///
/// ```text
/// error[E0046]: not all trait items implemented, missing: `sweep`
///    |
///    |     async fn sweep(&self, ctx: &Ctx) -> Result<BalanceResponse, BalanceError>;
///    |           -------------------------------------------------------------------- `sweep` from trait
/// ...
///    | impl UsageService<()> for UsageBackEnd {
///    | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ missing `sweep` in implementation
///
/// error: could not compile `tixschema` (test "zz_probe") due to 1 previous error
/// ```
///
/// The operation is named because the emitted trait declares it under the ident the author wrote,
/// which is what the unit test
/// `the_emitted_trait_names_the_operation_a_missing_implementation_is_refused_for` reads back off
/// the expansion.
#[cfg(feature = "serde")]
fn emitted_trait(declared: &ItemTrait) -> ItemTrait {
    let mut emitted = declared.clone();
    for member in &mut emitted.items {
        let TraitItem::Fn(operation) = member else {
            continue;
        };
        operation
            .attrs
            .retain(|attribute| !attribute.path().is_ident(parse::OPERATION_DIRECTIVE));
        if operation.sig.asyncness.take().is_some() {
            let answered = match &operation.sig.output {
                ReturnType::Default => quote! { () },
                ReturnType::Type(_, carried) => quote! { #carried },
            };
            operation.sig.output = syn::parse_quote! {
                -> impl ::core::future::Future<Output = #answered> + Send
            };
        }
    }
    emitted
}

/// The service's TypeScript, which only a build that writes TypeScript at all has anything to say
/// for.
#[cfg(all(feature = "serde", feature = "typescript"))]
fn typescript(service: &parse::ServiceDef) -> TokenStream {
    emit_typescript(service)
}

#[cfg(all(feature = "serde", not(feature = "typescript")))]
fn typescript(_service: &parse::ServiceDef) -> TokenStream {
    TokenStream::new()
}

#[cfg(test)]
mod feature_tests;

#[cfg(test)]
mod tests;
