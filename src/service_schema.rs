//! `#[service_schema]`: a service declared once as a trait, read once, and handed to the emitters.
//!
//! [`parse`] reads and validates the declared trait into a
//! [`ServiceDef`](parse::ServiceDef) — the representation everything below consumes and nothing
//! below re-derives. The trait itself is emitted here, as declared save for the `async fn`
//! desugaring the compiler asks for; every other artifact belongs to one of the emitter modules,
//! each of which is landed by its own task.

mod client;
mod dispatch;
mod messages;
pub mod parse;
pub mod support;

#[cfg(feature = "typescript")]
use crate::features::service_schema::emit as emit_typescript;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{ItemTrait, ReturnType, TraitItem};

pub fn exec_service_schema(_args: TokenStream, input: TokenStream) -> TokenStream {
    let declared = match syn::parse2::<ItemTrait>(input) {
        Ok(parsed) => parsed,
        Err(rejection) => return rejection.to_compile_error(),
    };
    // The trait is emitted whether or not it validates, so a service with one bad operation
    // reports that operation rather than burying it under an unresolved trait name at every
    // implementation and every call site.
    let contract = emitted_trait(&declared);
    match parse::parse_service(&declared) {
        Ok(service) => {
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
        Err(refusal) => {
            let refusals = refusal.to_compile_error();
            quote! {
                #refusals
                #contract
            }
        }
    }
}

/// The trait as the author declared it, less the per-operation directives, with every `async fn`
/// desugared to the `-> impl Future + Send` the `async_fn_in_trait` warning recommends writing.
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
#[cfg(feature = "typescript")]
fn typescript(service: &parse::ServiceDef) -> TokenStream {
    emit_typescript(service)
}

#[cfg(not(feature = "typescript"))]
fn typescript(_service: &parse::ServiceDef) -> TokenStream {
    TokenStream::new()
}

#[cfg(test)]
mod tests;
