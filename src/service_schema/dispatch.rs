//! The Rust dispatcher: given the incoming message and a way to answer, it settles the message
//! itself. Generic over the implementing type, because a trait with `async fn` is not dyn
//! compatible.
//!
//! # What one arm does, and in what order
//!
//! Deserialize the payload into the operation's message, run *that message's own* `validate()`,
//! call the implementation, answer. The order is the point: **an implementation may assume its
//! incoming message is valid, because an invalid one never reaches it.** A payload that will not
//! deserialize, a message that fails validation and an operation name nothing recognises are all
//! faults, and a fault goes through the reply handle like any other answer rather than becoming a
//! return value the transport has to interpret.
//!
//! Every arm calls exactly one of `send`, `fault` and `done`, and a one-way arm calls `done`. On
//! the bus this was measured against, acknowledging and replying are one act: an arm that returned
//! without touching the handle would leave its delivery unacknowledged forever and stall the
//! consumer against its prefetch.
//!
//! # What this file emits beside `dispatch`, and who else reads it
//!
//! Three things land here that the [`client`](super::client) also reads, both being spliced into
//! the one module [`support`](super::support) opens: the `Answered` envelope, the
//! `MessageValidation` fallback, and the two readers that turn a violation report into the field
//! and the detail a fault carries.
//!
//! `Answered` is the `{ ok, value }` / `{ ok, error }` envelope the design ratified, and the
//! dispatcher hands it to `send` rather than handing over a bare `Result`. Serde writes a `Result`
//! as `{"Ok": …}`, which is not what a TypeScript caller of the same operation reads, and the two
//! languages describing one call the same way is what the envelope keeps.
//!
//! `MessageValidation` exists because `validate()` is not universal. `#[model_schema()]` writes an
//! inherent `validate()` onto a type with constrained fields and none onto a type without one, and
//! a message the author declared may carry no annotation at all. An inherent method takes
//! precedence over a trait's, so a message that declared constraints runs them and one that
//! declared none passes.

use super::parse::{OperationDef, OperationInputs, OperationOutcome, ServiceDef};
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

pub fn emit(service: &ServiceDef) -> TokenStream {
    let contract = &service.ident;
    let arms = service.operations.iter().map(arm);
    let dispatch_doc = format!(
        "Turns one incoming message into a call on a `{contract}` implementation, and settles \
         it.\n\n\
         It answers through `reply` rather than returning anything, so nothing has to represent \
         \"no reply\" and an answer cannot reach the wrong message. The operation is matched from \
         [`IncomingMessage::operation`], which the transport read off the wire beside the payload; \
         it is never read out of the payload itself.\n\n\
         Generic over the implementing type rather than taking `&dyn {contract}`: a trait whose \
         methods are `async` is not dyn compatible, so there is no such form to offer. The future \
         it hands back carries nothing but `()`, written in the same `-> impl Future + Send` \
         desugaring the trait itself is emitted in, so a consumer loop can spawn it."
    );
    let incoming = incoming_message();
    let envelope = answered_envelope();
    let validation = message_validation();
    let readers = violation_readers();
    quote! {
        #incoming
        #envelope
        #validation
        #readers

        #[doc = #dispatch_doc]
        pub fn dispatch<S, Ctx, R>(
            svc: &S,
            ctx: &Ctx,
            message: &IncomingMessage,
            reply: &R,
        ) -> impl ::core::future::Future<Output = ()> + Send
        where
            S: super::#contract<Ctx> + Sync,
            Ctx: Sync,
            R: Reply + Sync,
        {
            async move {
                match message.operation.as_str() {
                    #(#arms)*
                    unrecognised => {
                        reply.fault(ServiceFault::unknown_operation(unrecognised)).await
                    }
                }
            }
        }
    }
}

/// The `{ ok, value }` / `{ ok, error }` envelope a request-and-reply operation answers in, which
/// the client reads back and a TypeScript caller of the same operation narrows on.
fn answered_envelope() -> TokenStream {
    quote! {
        /// What a request-and-reply operation puts on the wire: the envelope, with the message the
        /// operation declared left exactly as it is inside it.
        #[derive(::serde::Deserialize, ::serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Answered<T, E> {
            #[serde(skip_serializing_if = "Option::is_none")]
            error: Option<E>,
            ok: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            value: Option<T>,
        }

        impl<T, E> Answered<T, E> {
            /// The envelope around the outcome an implementation produced.
            fn answering(outcome: Result<T, E>) -> Self {
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
        }
    }
}

/// One arm: deserialize, validate, call, answer. Both fault paths name the wire name rather than
/// what arrived, this arm being the one that answered to it.
fn arm(operation: &OperationDef) -> TokenStream {
    let wire = &operation.wire_name;
    let message = message_type(operation);
    let call = call_arguments(operation);
    let method = &operation.ident;
    let settled = match operation.outcome {
        OperationOutcome::OneWay => quote! {
            svc.#method(ctx #(, #call)*).await;
            reply.done().await
        },
        OperationOutcome::Reply { .. } => quote! {
            reply
                .send(Answered::answering(svc.#method(ctx #(, #call)*).await))
                .await
        },
    };
    quote! {
        #wire => {
            let received = match ::serde_json::from_slice::<#message>(&message.payload) {
                Ok(received) => received,
                Err(rejected) => {
                    return reply
                        .fault(ServiceFault::undeserializable_payload(
                            #wire,
                            &rejected.to_string(),
                        ))
                        .await;
                }
            };
            if let Err(violations) = received.validate() {
                return reply
                    .fault(ServiceFault::failed_validation(
                        #wire,
                        violated_field(&violations),
                        &violation_detail(&violations),
                    ))
                    .await;
            }
            #settled
        }
    }
}

/// What the implementation is handed after the context: the message itself where the operation
/// named one, and otherwise the fields of the message declared for it, unpacked back into the
/// arguments the operation was written with.
fn call_arguments(operation: &OperationDef) -> Vec<TokenStream> {
    match &operation.inputs {
        OperationInputs::Empty => Vec::new(),
        OperationInputs::Generated(arguments) => arguments
            .iter()
            .map(|(field, _)| quote! { received.#field })
            .collect(),
        OperationInputs::Named(_) => vec![quote! { received }],
    }
}

/// `IncomingMessage`: everything the dispatcher reads off the wire, which is the operation and the
/// bytes.
fn incoming_message() -> TokenStream {
    quote! {
        /// One message as the transport read it: the operation it names, and the payload it
        /// carries.
        ///
        /// The operation travels beside the payload rather than inside it — the `operation-name`
        /// header on AMQP, the method name on gRPC, the path on HTTP — so no message type has to
        /// reserve a key for routing.
        pub struct IncomingMessage {
            /// The wire name of the operation this message is for.
            pub operation: String,
            /// The encoded message, which the dispatcher deserializes into the operation's own
            /// message type.
            pub payload: Vec<u8>,
        }
    }
}

/// What a message answers when it publishes no `validate()` of its own.
fn message_validation() -> TokenStream {
    quote! {
        /// The answer a message with no declared constraints gives when asked to validate itself.
        ///
        /// `#[model_schema()]` writes an inherent `validate()` onto a type with constrained fields
        /// and none onto a type without one, and an inherent method takes precedence over a
        /// trait's — so a message that declared constraints runs them, and one that declared none
        /// passes here.
        pub trait MessageValidation {
            /// `Ok(())`, there being nothing declared to check.
            fn validate(&self) -> Result<(), Vec<String>> {
                Ok(())
            }
        }

        impl<T> MessageValidation for T {}
    }
}

/// The type the payload deserializes into: the argument that already is the message, or the
/// message the macro declared for the operation.
fn message_type(operation: &OperationDef) -> TokenStream {
    match &operation.inputs {
        OperationInputs::Named(declared) => quote! { #declared },
        OperationInputs::Empty | OperationInputs::Generated(_) => {
            let declared: Option<Ident> = operation.generated_message_ident();
            quote! { #declared }
        }
    }
}

/// The two readers that turn a violation report into what a fault carries.
fn violation_readers() -> TokenStream {
    quote! {
        /// Everything that failed, in one line, for the fault's detail.
        fn violation_detail(reported: &[String]) -> String {
            reported.join("; ")
        }

        /// The field a violation names. Every validator `#[model_schema()]` generates writes the
        /// field first and in single quotes — `'organization_id' is too short: …` — so the name is
        /// read back off the report rather than tracked beside it. A violation naming no field, as
        /// a constrained newtype's does, leaves the fault's field empty.
        fn violated_field(reported: &[String]) -> Option<&str> {
            let (field, _rest) = reported.first()?.strip_prefix('\'')?.split_once('\'')?;
            Some(field)
        }
    }
}
