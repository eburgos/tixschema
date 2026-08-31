//! The messages an operation did not name: `<Operation>Request` for the argument-list and
//! zero-argument shapes, emitted like any other type with its TypeScript, Zod and JSON Schema.
//!
//! Reads the [`GeneratedMessage`] list [`parse`](super::parse) recorded off
//! [`OperationInputs`](super::parse::OperationInputs), and never re-reads the trait — so what gets
//! written here and what gets registered downstream are one list, and neither can name a type the
//! other does not.
//!
//! Each message is annotated exactly as a hand-written one is, and for the same reasons. It
//! carries `#[model_schema()]`, so a client on the far side gets its TypeScript type, its Zod
//! schema and its JSON Schema rather than a Rust-only type it cannot construct. It carries the
//! serde derives and `rename_all = "camelCase"` itself, because the author never wrote the type
//! and has nowhere to put either: an argument is `snake_case` in Rust and camelCase on the wire,
//! exactly as a hand-written field is.

use super::parse::{GeneratedMessage, ServiceDef};
use proc_macro2::TokenStream;
use quote::quote;

/// A generated message publishes under the operation's own name, with no service prefix.
///
/// That is deliberate — it is the name a caller in either language types — and it means two
/// services in one module cannot both leave the same operation name's message to the macro. The
/// second declaration is a duplicate definition in Rust, so a bundle carrying two same-named
/// generated messages cannot be built at all. The result types either service publishes *are*
/// prefixed, because those the macro names itself and TypeScript has no per-service scope.
///
/// Two services whose generated messages differ compile:
///
/// ```rust
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct Report;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum Failed {
///     DbError,
/// }
///
/// #[service_schema()]
/// pub trait AlphaService<Ctx> {
///     async fn sweep(&self, ctx: &Ctx) -> Result<Report, Failed>;
/// }
///
/// #[service_schema()]
/// pub trait BetaService<Ctx> {
///     async fn reconcile(&self, ctx: &Ctx) -> Result<Report, Failed>;
/// }
///
/// fn main() {}
/// ```
///
/// The run below is that one with `reconcile` spelled `sweep`, and nothing else changed, so the
/// refusal can only be the message name the two now share:
///
/// ```rust,compile_fail
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct Report;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum Failed {
///     DbError,
/// }
///
/// #[service_schema()]
/// pub trait AlphaService<Ctx> {
///     async fn sweep(&self, ctx: &Ctx) -> Result<Report, Failed>;
/// }
///
/// #[service_schema()]
/// pub trait BetaService<Ctx> {
///     async fn sweep(&self, ctx: &Ctx) -> Result<Report, Failed>;
/// }
///
/// fn main() {}
/// ```
///
/// `compile_fail` says only that *something* was refused and this toolchain checks no error code a
/// doctest names, so the pair above was compiled standalone and read. The refusal is a pile rather
/// than one sentence — eleven errors, the first two naming the message and the module beside it,
/// the rest following from them — and none of them names the operation or the two services:
///
/// ```text
/// error[E0428]: the name `sweep_request_schema` is defined multiple times
/// error[E0428]: the name `SweepRequest` is defined multiple times
/// error[E0119]: conflicting implementations of trait `Deserialize<'_>` for type `SweepRequest`
/// error[E0119]: conflicting implementations of trait `Serialize` for type `SweepRequest`
/// error[E0592]: duplicate definitions with name `json_schema`
/// error[E0592]: duplicate definitions with name `ts_definition`
/// error[E0592]: duplicate definitions with name `zod_schema`
/// error[E0034]: multiple applicable items in scope        (×4)
/// error: could not compile `tixschema` (test "zz_probe") due to 11 previous errors
/// ```
///
/// It is a build failure either way, which is what a bundle needs: nothing that reaches TypeScript
/// can carry the same generated message twice. Whether the macro should say so in one sentence
/// instead is a separate question and not one this emitter answers today.
pub fn emit(service: &ServiceDef) -> TokenStream {
    let declared = service.generated_messages.iter().map(message);
    quote! {
        #(#declared)*
    }
}

/// The rustdoc a generated message carries, written where an author will read it before reaching
/// for the multi-argument form: its field names are parameter names, so renaming a parameter moves
/// a key on the wire and nothing in Rust says so.
fn cost_note(operation: &str, empty: bool) -> Vec<String> {
    if empty {
        vec![
            format!(
                " The message operation `{operation}` receives, declared empty because the \
                 operation takes nothing after the context."
            ),
            String::new(),
            " An operation that later needs a field gains one here, rather than changing from \
             carrying no payload to carrying one and breaking every caller."
                .to_owned(),
            String::new(),
            " A field added here takes its name from the parameter it stands for, so renaming that \
             parameter moves a key on the wire and no compiler will flag it."
                .to_owned(),
        ]
    } else {
        vec![
            format!(
                " The message operation `{operation}` receives, declared from its argument list \
                 because the operation names no message of its own."
            ),
            String::new(),
            " Its field names are the operation's parameter names, so renaming a parameter, an \
             invisible refactor in Rust, moves a key on the wire and no compiler will flag it. An \
             operation that takes one already-declared message instead pays nothing of the sort."
                .to_owned(),
        ]
    }
}

fn message(declared: &GeneratedMessage) -> TokenStream {
    let named = &declared.ident;
    let members = declared
        .fields
        .iter()
        .map(|(field, carried)| quote! { pub #field: #carried });
    let rustdoc = cost_note(
        &declared.declared_for.to_string(),
        declared.fields.is_empty(),
    );
    quote! {
        #(#[doc = #rustdoc])*
        #[::tixschema::model_schema()]
        #[derive(::serde::Serialize, ::serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct #named {
            #(#members,)*
        }
    }
}
