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
