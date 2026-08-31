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
//! - `ts_definition()`: every generated message's type and schema, the fault type and the kind it
//!   reports, and one [`result`] type per operation that answers.
//! - [`ts_client()`](client): the transport seam, the client type and the factory that binds one.
//! - [`ts_service()`](service): the interface an implementation satisfies in full, the outcome
//!   types it answers with, and the dispatcher factory.
//!
//! # Every emitted name carries the service
//!
//! TypeScript has no per-service scope. Rust puts each service's supporting types in a module of
//! its own, and a bundle is one flat file — so a consuming codebase with ten services in one
//! bundle would declare `ServiceFault` ten times and would not compile, and two services sharing
//! an operation name would collide on the result type the same way. Every name emitted here is
//! therefore prefixed with the service: `UsageServiceFault`, `UsageServiceGetBalanceResult`,
//! `UsageServiceClient`. The prefix makes TypeScript say what Rust already means.
//!
//! # The fault's TypeScript is generated, not written
//!
//! The Rust `ServiceFault` carries `#[model_schema()]`, so its TypeScript comes from the same
//! declaration as the Rust type and the two cannot drift. Nothing here writes a fault type; the
//! registration below asks the Rust type for its own, exactly as it does for every message.

mod client;
mod message;
mod result;
mod service;

use crate::service_schema::parse::ServiceDef;
use crate::service_schema::support::module_ident;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub fn emit(service: &ServiceDef) -> TokenStream {
    let named = service.ident.to_string();
    let registry = format_ident!("{named}Schema", span = service.ident.span());
    let rustdoc = registry_rustdoc(&named);
    let published = published(service);
    let client = client::emit(service).join("\n\n");
    let service_side = service::emit(service).join("\n\n");
    quote! {
        #(#[doc = #rustdoc])*
        pub struct #registry;

        impl #registry {
            #[doc = " The service's generated TypeScript client: the transport seam it is bound"]
            #[doc = " to, the type its methods are declared on, and the factory that binds one."]
            pub fn ts_client() -> String {
                #client.to_owned()
            }

            #[doc = " Every TypeScript type this service publishes: the messages the macro declared"]
            #[doc = " for it, the fault a caller can receive, and one result type per operation that"]
            #[doc = " answers."]
            pub fn ts_definition() -> String {
                [#(#published),*].join("\n\n")
            }

            #[doc = " The service's implementable TypeScript interface, the outcome types an"]
            #[doc = " implementation answers with, and the dispatcher factory that drives one."]
            pub fn ts_service() -> String {
                #service_side.to_owned()
            }
        }
    }
}

/// One expression per published artifact, each answering with a `String`, in the order they are
/// written into the bundle: every declared message first, so the types the result envelopes name
/// are read before the envelopes themselves, then the fault, then the results.
///
/// The fault and the kind it reports are asked for by name rather than written here. Both are
/// ordinary `#[model_schema()]` types inside the service's own module, so their TypeScript comes
/// from the declarations the Rust dispatcher and the Rust client build faults from — the one thing
/// that keeps the type a caller narrows on and the value the wire carries from drifting apart.
///
/// A message's Zod schema is one of those artifacts and is registered here for the same reason its
/// type is — nobody else has a line to write it on. It is asked for only in a build that writes
/// Zod at all.
fn published(service: &ServiceDef) -> Vec<TokenStream> {
    let module = module_ident(service);
    let mut collected = Vec::new();
    for declared in &service.generated_messages {
        let message = &declared.ident;
        collected.push(quote! { #message::ts_definition() });
        #[cfg(feature = "zod")]
        collected.push(quote! { #message::zod_schema() });
    }
    let fault = format_ident!("{}Fault", service.ident);
    let kind = format_ident!("{}FaultKind", service.ident);
    collected.push(quote! { #module::#kind::ts_definition() });
    collected.push(quote! { #module::#fault::ts_definition() });
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
