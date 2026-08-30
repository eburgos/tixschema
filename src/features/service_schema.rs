//! The TypeScript a `#[service_schema]` service publishes, behind one registration line.
//!
//! # Registration rides with the service
//!
//! In the consuming codebase a type reaches the emitted TypeScript only by being named by hand in
//! a bundle's entity list. A message the macro declared has nobody to write that line: the author
//! never wrote the type and has no reason to know its name, so a forgotten line would leave a
//! Rust-only message and a client unable to call the operation at all.
//!
//! So `ts_definition()` answers for the service *and* for every message
//! [`parse`](crate::service_schema::parse) recorded on
//! [`ServiceDef::generated_messages`](crate::service_schema::parse::ServiceDef::generated_messages)
//! — the same list the message emitter writes the types from, read rather
//! than re-derived, so what is written and what is registered cannot disagree. A service is added
//! to a bundle once and nothing it declared can be left behind.
//!
//! # Why the registration hangs off `<Service>Schema`
//!
//! The design spells the bundle line `UsageService::ts_definition()`, and Rust does not allow it:
//! `UsageService` is a trait, an inherent `impl` on a trait is not a thing, and calling a trait's
//! associated function without naming an implementing type is `error[E0790]`. A struct of the same
//! name would collide with the trait in the type namespace. So the artifacts hang off a unit struct
//! named for the service, `UsageServiceSchema`, and the bundle line reads
//! `UsageServiceSchema::ts_definition()` — still one line per artifact, still nothing to remember
//! per message.
//!
//! # What each artifact is
//!
//! - `ts_definition()`: every generated message's type and schema, the [`fault`] type, and one
//!   [`result`] type per operation that answers.
//! - `ts_client()` and `ts_service()`: empty until the tasks that write the client and the
//!   implementable service fill them. They exist now so those tasks change a body rather than
//!   invent a registration surface, and so a bundle written against this one keeps compiling.

mod fault;
mod result;

use crate::service_schema::parse::ServiceDef;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub fn emit(service: &ServiceDef) -> TokenStream {
    let named = service.ident.to_string();
    let registry = format_ident!("{named}Schema", span = service.ident.span());
    let rustdoc = registry_rustdoc(&named);
    let published = published(service);
    quote! {
        #(#[doc = #rustdoc])*
        pub struct #registry;

        impl #registry {
            #[doc = " The service's generated TypeScript client. Empty until that emitter lands."]
            pub fn ts_client() -> String {
                String::new()
            }

            #[doc = " Every TypeScript type this service publishes: the messages the macro declared"]
            #[doc = " for it, the fault a caller can receive, and one result type per operation that"]
            #[doc = " answers."]
            pub fn ts_definition() -> String {
                [#(#published),*].join("\n\n")
            }

            #[doc = " The service's implementable TypeScript interface and its dispatcher factory."]
            #[doc = " Empty until that emitter lands."]
            pub fn ts_service() -> String {
                String::new()
            }
        }
    }
}

/// One expression per published artifact, each answering with a `String`, in the order they are
/// written into the bundle: every declared message first, so the types the result envelopes name
/// are read before the envelopes themselves, then the fault, then the results.
///
/// A message's Zod schema is one of those artifacts and is registered here for the same reason its
/// type is — nobody else has a line to write it on. It is asked for only in a build that writes
/// Zod at all.
fn published(service: &ServiceDef) -> Vec<TokenStream> {
    let mut collected = Vec::new();
    for declared in &service.generated_messages {
        let message = &declared.ident;
        collected.push(quote! { #message::ts_definition() });
        #[cfg(feature = "zod")]
        collected.push(quote! { #message::zod_schema() });
    }
    let fault = fault::TYPESCRIPT;
    collected.push(quote! { #fault.to_owned() });
    collected.extend(
        result::emit(service)
            .iter()
            .map(|rendered| quote! { #rendered.to_owned() }),
    );
    collected
}

fn registry_rustdoc(service: &str) -> Vec<String> {
    vec![
        format!(" What `{service}` publishes to TypeScript, in one place per artifact."),
        String::new(),
        format!(
            " A bundle names `{service}Schema::ts_definition()` once and receives the service's own \
             types together with every message the macro declared for it, so no generated message \
             needs a registration line of its own."
        ),
    ]
}

#[cfg(test)]
mod tests;
