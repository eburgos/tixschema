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
//! The other half of that boundary is the bundle writer's. A type the *author* named is not the
//! service's to publish — the service does not own it and has no schema of its own to write for
//! it — so it takes two lines in the bundle rather than one: its type and its schema. A bundle
//! naming only the type still parses through `<Type>$Schema` in the client and the dispatcher, and
//! nothing here can refuse that: the bundle is assembled at run time by the consuming codebase out
//! of strings, and expansion sees a type path on a trait and no bundle at all. The consuming
//! codebase's own compiler refuses it, naming the value at every parse site, which is why the
//! requirement is written into the registry's rustdoc rather than checked for.
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
//! - `ts_client()`: the transport seam, the client type and the factory that binds one.
//! - `ts_service()`: the interface an implementation satisfies in full, the outcome types it
//!   answers with, and the dispatcher factory.
//!
//! # The client and the dispatcher exist only where the Zod surface does
//!
//! A message validates when it is constructed, in both directions: the client parses what it is
//! about to send before a transport is reached, and the dispatcher parses what arrived before an
//! implementation is entered, so an implementation may assume its message is valid. On the
//! TypeScript side both checks are the same parse, against the `<Message>$Schema` const
//! `#[model_schema()]` publishes — and only a build with the `zod` feature publishes one.
//!
//! So a build with `typescript` on and `zod` off emits neither. The two artifacts it can still
//! write truthfully — the message types and the result envelopes — are published exactly as they
//! always are, because they describe what a Rust service puts on the wire and that half validates
//! either way. The two it cannot are absent rather than emitted without their check: a client that
//! forwards whatever it is handed and a dispatcher that narrows an unread payload with `as` would
//! both compile, both look like the checked ones, and neither would hold the guarantee every
//! caller of them is written against.
//!
//! A bundle naming `<Service>Schema::ts_client()` in such a build is refused where it names it,
//! which is the one place the choice of features can still be acted on.
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
//! declaration as the Rust type and the two cannot drift. Nothing here writes a fault's fields; the
//! registration below asks the Rust type for them, exactly as it does for every message.
//!
//! What the registration adds is the seal. The fields publish under a name of their own,
//! `<Service>FaultFields`, and [`fault`] declares `<Service>Fault` over them as those fields plus a
//! brand keyed on a symbol the bundle exports nowhere. Rust refuses a fabricated fault twice over —
//! `E0451` on the fields, `E0624` on the constructors — and this is what TypeScript can be given in
//! their place: a type a caller reads exactly as before and an implementation cannot write.

#[cfg(feature = "zod")]
mod client;
mod fault;
#[cfg(feature = "zod")]
mod message;
mod result;
#[cfg(feature = "zod")]
mod service;

use crate::service_schema::parse::ServiceDef;
use crate::service_schema::support::{fault_fields_typescript_name, module_ident};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub fn emit(service: &ServiceDef) -> TokenStream {
    let named = service.ident.to_string();
    let registry = format_ident!("{named}Schema", span = service.ident.span());
    let rustdoc = registry_rustdoc(&named);
    let published = published(service);
    let seam = seam(service);
    quote! {
        #(#[doc = #rustdoc])*
        pub struct #registry;

        impl #registry {
            #[doc = " Every TypeScript type this service publishes: the messages the macro declared"]
            #[doc = " for it, the fault a caller can receive, and one result type per operation that"]
            #[doc = " answers."]
            pub fn ts_definition() -> String {
                [#(#published),*].join("\n\n")
            }

            #seam
        }
    }
}

/// The two artifacts that carry the validation decision D11 binds: the client that checks a
/// message before it reaches a transport, and the dispatcher that checks one before it reaches an
/// implementation. Both parse against the Zod schema `#[model_schema()]` publishes for the message,
/// so a build without the Zod surface has nothing for either of them to check against and publishes
/// neither.
#[cfg(feature = "zod")]
fn seam(service: &ServiceDef) -> TokenStream {
    let client = client::emit(service).join("\n\n");
    let service_side = service::emit(service).join("\n\n");
    quote! {
        #[doc = " The service's generated TypeScript client: the transport seam it is bound"]
        #[doc = " to, the type its methods are declared on, and the factory that binds one."]
        pub fn ts_client() -> String {
            #client.to_owned()
        }

        #[doc = " The service's implementable TypeScript interface, the outcome types an"]
        #[doc = " implementation answers with, and the dispatcher factory that drives one."]
        pub fn ts_service() -> String {
            #service_side.to_owned()
        }
    }
}

#[cfg(not(feature = "zod"))]
fn seam(_service: &ServiceDef) -> TokenStream {
    TokenStream::new()
}

/// One expression per published artifact, each answering with a `String`, in the order they are
/// written into the bundle: every declared message first, so the types the result envelopes name
/// are read before the envelopes themselves, then the fault, then the results.
///
/// The fault's fields and the kind it reports are asked for by name rather than written here. Both
/// are ordinary `#[model_schema()]` types inside the service's own module, so their TypeScript
/// comes from the declarations the Rust dispatcher and the Rust client build faults from — the one
/// thing that keeps the type a caller narrows on and the value the wire carries from drifting
/// apart. What [`fault`] adds beside them is the seal and nothing else: no field, no kind, no
/// spelling of either.
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
    let fields = format_ident!(
        "{}",
        fault_fields_typescript_name(&service.ident.to_string())
    );
    let kind = format_ident!("{}FaultKind", service.ident);
    collected.push(quote! { #module::#kind::ts_definition() });
    collected.push(quote! { #module::#fields::ts_definition() });
    collected.extend(
        fault::emit(service)
            .iter()
            .map(|rendered| quote! { #rendered.to_owned() }),
    );
    collected.extend(
        result::emit(service)
            .iter()
            .map(|rendered| quote! { #rendered.to_owned() }),
    );
    collected
}

fn registry_rustdoc(service: &str) -> Vec<String> {
    let mut written = vec![
        format!(" What `{service}` publishes to TypeScript, in one place per artifact."),
        String::new(),
        format!(
            " A bundle names `{service}Schema::ts_definition()` once and receives the service's own \
             types together with every message the macro declared for it, so no generated message \
             needs a registration line of its own."
        ),
    ];
    written.extend(seam_rustdoc(service));
    written
}

/// What the registry's own rustdoc says about the client and the dispatcher. In a build that
/// publishes them, what a bundle carrying them has to declare beside them — the two methods carry
/// the rest themselves. In a build that does not, the reason they are missing, written where a
/// reader looking for them arrives.
#[cfg(feature = "zod")]
fn seam_rustdoc(service: &str) -> Vec<String> {
    vec![
        String::new(),
        format!(
            " The line above carries a schema for every message the macro declared and for no type \
             the author named: `{service}` does not own those and cannot publish their schemas. So \
             a type named on an operation takes two lines in a bundle, `ts_definition()` and \
             `zod_schema()` — the client and the dispatcher parse a message through \
             `<Type>$Schema`, and a bundle carrying only the type declares no such value. Nothing \
             here refuses that bundle. The consuming codebase's compiler does, with `error TS2304: \
             Cannot find name '<Type>$Schema'` at every site the message is parsed."
        ),
    ]
}

#[cfg(not(feature = "zod"))]
fn seam_rustdoc(service: &str) -> Vec<String> {
    vec![
        String::new(),
        format!(
            " This build publishes no `{service}Schema::ts_client()` and no \
             `{service}Schema::ts_service()`. Both parse a message against the schema \
             `#[model_schema()]` writes for it, and only a build with tixschema's `zod` feature \
             writes one — so rather than a client and a dispatcher that check nothing, this build \
             publishes the service's types and leaves the two seam artifacts out. Add `features = \
             [\"zod\"]` to the tixschema dependency to get them."
        ),
    ]
}

#[cfg(test)]
mod tests;
