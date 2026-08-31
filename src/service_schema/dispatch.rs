//! The Rust dispatcher: given the incoming message and a way to answer, it settles the message
//! itself. Generic over the implementing type, because a trait with `async fn` is not dyn
//! compatible.
//!
//! # What one arm does, and in what order
//!
//! Deserialize the payload into the operation's message, run *that message's own* `validate()`,
//! call the implementation behind a panic guard, answer. The order is the point: **an
//! implementation may assume its incoming message is valid, because an invalid one never reaches
//! it.** A payload that will not deserialize, a message that fails validation, an operation name
//! nothing recognises and a handler that panicked are all faults, and a fault goes through the
//! reply handle like any other answer rather than becoming a return value the transport has to
//! interpret.
//!
//! A request-and-reply arm calls exactly one of `send` and `fault`. A one-way arm calls neither
//! once its implementation has been entered, so nothing about replying appears on a path that
//! never replies. Acknowledgement is the transport's, not the handle's — `dispatch` returns
//! nothing, so the adapter that called it still holds the delivery and acknowledges once dispatch
//! is done. That placement is why the panic guard exists rather than being an extra: a panic
//! unwinding out of `dispatch` never reaches the acknowledgement, and the bus this was measured
//! against has no `nack`, no dead-letter exchange, no message TTL and no timeout to settle the
//! delivery in its place.
//!
//! # What this file emits beside `dispatch`, and who else reads it
//!
//! Four things land here that the [`client`](super::client) also reads, both being spliced into
//! the one module [`support`](super::support) opens: the `Answered` envelope, the
//! `MessageValidation` fallback, the readers that turn a violation report into the field and the
//! detail a fault carries, and the reader for the field a single line names.
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
    let guard = panic_guard();
    quote! {
        #incoming
        #envelope
        #validation
        #readers
        #guard

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

/// One arm: deserialize, validate, call behind the panic guard, answer. Every fault path names the
/// wire name rather than what arrived, this arm being the one that answered to it.
///
/// A one-way arm answers nothing once the implementation has been entered, a panic included: the
/// operation declared no reply and the delivery carries no queue for one to go to. What the guard
/// buys there is the return itself — the transport acknowledges after `dispatch` returns, and a
/// panic that unwound past it would leave the delivery outstanding.
fn arm(operation: &OperationDef) -> TokenStream {
    let wire = &operation.wire_name;
    let message = message_type(operation);
    let call = call_arguments(operation);
    let method = &operation.ident;
    let called = quote! { caught(move || svc.#method(ctx #(, #call)*)).await };
    let settled = match operation.outcome {
        OperationOutcome::OneWay => quote! {
            let _unreported = #called;
        },
        OperationOutcome::Reply { .. } => quote! {
            match #called {
                Ok(answered) => reply.send(Answered::answering(answered)).await,
                Err(panicked) => {
                    reply.fault(ServiceFault::handler_panic(#wire, &panicked)).await
                }
            }
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

/// The readers that turn a violation report — or a deserializer's own refusal — into what a fault
/// carries.
fn violation_readers() -> TokenStream {
    quote! {
        /// The field one line names, where it is written in the shape every validator
        /// `#[model_schema()]` generates: the field first and in single quotes —
        /// `'organization_id' is too short: …`. A line written any other way names none.
        ///
        /// It is read off a deserializer's refusal as well as off a validator's report, because
        /// those are the same message. A field carrying a constraint gets a serde
        /// `deserialize_with` hook running the very check `validate()` runs, and the hook hands
        /// serde that check's message verbatim — so a payload refused before it ever became a
        /// message still names the field it got wrong.
        fn named_field(reported: &str) -> Option<&str> {
            let (field, _rest) = reported.strip_prefix('\'')?.split_once('\'')?;
            Some(field)
        }

        /// Everything that failed, in one line, for the fault's detail.
        fn violation_detail(reported: &[String]) -> String {
            reported.join("; ")
        }

        /// The field a violation report names, which is its first line's. A violation naming no
        /// field, as a constrained newtype's does, leaves the fault's field empty.
        fn violated_field(reported: &[String]) -> Option<&str> {
            named_field(reported.first()?)
        }
    }
}

/// The guard a handler is called behind, and the reader that turns a caught panic into a detail.
///
/// Two things make this the arm's business rather than the transport's. The delivery is
/// acknowledged after `dispatch` returns, so a panic that unwound past it is never acknowledged at
/// all — and the consumer this was measured against asks for manual acknowledgement with no
/// `nack`, no dead-letter exchange, no message TTL and no timeout, so that delivery stays
/// outstanding against the prefetch until the channel closes. And a handler that panicked failed at
/// something its operation never declared, which is exactly what a fault reports.
fn panic_guard() -> TokenStream {
    quote! {
        /// Runs a handler, answering `Err` with what it said where it panicked rather than letting
        /// the panic unwind out through `dispatch`.
        ///
        /// The handler is taken as the closure that makes its future rather than as the future, so
        /// that a panic raised while the call is set up is caught beside one raised while it runs:
        /// the trait's `async fn` is emitted desugared, and an implementation may answer it with an
        /// ordinary `fn` that does work before handing back a future.
        ///
        /// Unwind safety is asserted rather than proved. What a caught panic leaves behind is the
        /// implementation's own state, which nothing here can examine; the alternative is a
        /// delivery that is never settled, and a caller owed an answer either way. Under
        /// `panic = "abort"` nothing is caught and the process ends, that being the profile's
        /// decision rather than this one's.
        async fn caught<Making, Running>(making: Making) -> Result<Running::Output, String>
        where
            Making: FnOnce() -> Running,
            Running: ::core::future::Future,
        {
            let running =
                match ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(making)) {
                    Ok(running) => running,
                    Err(panicked) => return Err(panic_detail(&*panicked)),
                };
            let mut running = ::core::pin::pin!(running);
            ::core::future::poll_fn(move |polling| {
                match ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                    ::core::future::Future::poll(running.as_mut(), polling)
                })) {
                    Ok(::core::task::Poll::Pending) => ::core::task::Poll::Pending,
                    Ok(::core::task::Poll::Ready(answered)) => {
                        ::core::task::Poll::Ready(Ok(answered))
                    }
                    Err(panicked) => ::core::task::Poll::Ready(Err(panic_detail(&*panicked))),
                }
            })
            .await
        }

        /// What a caught panic said, for the fault's detail. A panic payload is whatever reached
        /// `panic!` — a `&str` for a literal message and a `String` for a formatted one — and
        /// anything else carries nothing a reader could be shown.
        fn panic_detail(panicked: &(dyn ::core::any::Any + Send)) -> String {
            if let Some(said) = panicked.downcast_ref::<&str>() {
                return (*said).to_owned();
            }
            if let Some(said) = panicked.downcast_ref::<String>() {
                return said.clone();
            }
            "the handler panicked, and said nothing that reads back".to_owned()
        }
    }
}
