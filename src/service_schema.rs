//! Spike scaffolding for `#[service_schema]`. Nothing here is the shipping macro.
//!
//! It exists to answer two questions with running code. Whether a struct this expansion emits,
//! carrying `#[model_schema()]`, re-expands into its TypeScript, Zod and JSON Schema surfaces —
//! which is what makes an operation taking several arguments get a usable message. And whether a
//! trait's context type parameter can be read here and threaded through every operation while
//! staying out of every generated message.

use crate::rename_rule::RenameRule;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::spanned::Spanned as _;
use syn::{
    FnArg, GenericParam, Ident, ItemTrait, Pat, PatType, ReturnType, TraitItem, TraitItemFn, Type,
};

pub fn exec_service_schema(_args: TokenStream, input: TokenStream) -> TokenStream {
    let declared = match syn::parse2::<ItemTrait>(input) {
        Ok(parsed) => parsed,
        Err(rejection) => return rejection.to_compile_error(),
    };
    let Some(context) = context_parameter(&declared) else {
        return syn::Error::new(
            declared.ident.span(),
            "service_schema: the trait must declare a context type parameter",
        )
        .to_compile_error();
    };

    let mut messages = Vec::new();
    let mut rejections = Vec::new();
    for member in &declared.items {
        if let TraitItem::Fn(operation) = member {
            match generated_message(operation, &context) {
                Ok(Some(message)) => messages.push(message),
                Ok(None) => (),
                Err(rejection) => rejections.push(rejection.to_compile_error()),
            }
        }
    }

    let operations = desugared_trait(&declared);
    quote! {
        #(#rejections)*
        #(#messages)*
        #operations
    }
}

/// The context is the trait's first type parameter, and it is the only one this spike reads.
fn context_parameter(declared: &ItemTrait) -> Option<Ident> {
    declared
        .generics
        .params
        .iter()
        .find_map(|parameter| match parameter {
            GenericParam::Type(named) => Some(named.ident.clone()),
            GenericParam::Const(_) | GenericParam::Lifetime(_) => None,
        })
}

/// Everything an operation takes that is neither the receiver nor the context — the argument list
/// a message is declared from.
fn operation_arguments<'operation>(
    operation: &'operation TraitItemFn,
    context: &Ident,
) -> Vec<&'operation PatType> {
    operation
        .sig
        .inputs
        .iter()
        .filter_map(|input| match input {
            FnArg::Typed(typed) => Some(typed),
            FnArg::Receiver(_) => None,
        })
        .filter(|typed| !is_context_argument(typed.ty.as_ref(), context))
        .collect()
}

fn is_context_argument(declared: &Type, context: &Ident) -> bool {
    let Type::Reference(borrowed) = declared else {
        return false;
    };
    let Type::Path(named) = borrowed.elem.as_ref() else {
        return false;
    };
    named.qself.is_none() && named.path.is_ident(context)
}

/// One argument is already the message and nothing is declared. Any other count gets a message
/// declared from the argument list, annotated exactly as a hand-written one is.
fn generated_message(
    operation: &TraitItemFn,
    context: &Ident,
) -> Result<Option<TokenStream>, syn::Error> {
    let arguments = operation_arguments(operation, context);
    if arguments.len() == 1 {
        return Ok(None);
    }

    let mut members = Vec::new();
    for argument in arguments {
        let Pat::Ident(named) = argument.pat.as_ref() else {
            return Err(syn::Error::new(
                argument.pat.span(),
                "service_schema: an operation argument must be a plain name",
            ));
        };
        let member = &named.ident;
        let carried = argument.ty.as_ref();
        members.push(quote! { pub #member: #carried });
    }

    let message = format_ident!(
        "{}Request",
        RenameRule::PascalCase.apply_to_field(&operation.sig.ident.to_string())
    );
    Ok(Some(quote! {
        #[::tixschema::model_schema()]
        #[derive(::serde::Serialize, ::serde::Deserialize)]
        pub struct #message {
            #(#members,)*
        }
    }))
}

/// `async fn` in a public trait is a warning the compiler asks you to desugar yourself, and the
/// desugaring below is the one it recommends.
fn desugared_trait(declared: &ItemTrait) -> ItemTrait {
    let mut emitted = declared.clone();
    for member in &mut emitted.items {
        if let TraitItem::Fn(operation) = member
            && operation.sig.asyncness.take().is_some()
        {
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
