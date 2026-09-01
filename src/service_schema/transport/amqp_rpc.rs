//! The AMQP request-and-reply transport: an inert `{service}_amqp_rpc_dispatcher!` macro carrying
//! the dispatcher, and nothing compiled where the service is declared.
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
//! # Why the tokens sit inside a macro
//!
//! Everything below is a stored token sequence in the crate that declares the service, and is
//! compiled only in the crate that invokes the macro. That is what keeps `serde_json` and
//! `tracing` out of the declaring crate's manifest, and what lets a service decline this shape
//! entirely: `IncomingMessage`'s operation-name-over-opaque-bytes routing is one transport model,
//! and a service that asks for no transport is emitted none of it.
//!
//! The macro emits bare items and opens no module of its own, so the caller names the module and
//! two transports in one crate cannot collide:
//!
//! ```text
//! mod amqp_transport {
//!     declaring_crate::usage_service_amqp_rpc_dispatcher!();
//! }
//! ```
//!
//! # Where a path inside the macro resolves
//!
//! Paths in a `macro_rules!` body resolve at the *invocation* site, so the two kinds are spelled
//! apart. Everything tixschema generated is reached through `$crate::` — the trait and the message
//! types at the scope the author declared them in, the fault and the rest inside
//! `$crate::{service}_schema` — which resolves in the crate that *defined* the macro. Every
//! runtime crate is reached through a leading `::` — `::serde_json`, `::tracing`, `::core`,
//! `::std` — and resolves in the invoking crate, which is therefore the one that names them. The
//! items the macro writes itself (`IncomingMessage`, the panic guard, the reader that classifies a
//! serde refusal) are named bare: they land beside `dispatch` in the module the caller supplied
//! and exist nowhere else.
//!
//! `#[macro_export]` puts the macro at the declaring crate's root whatever module it was written
//! in, and `$crate` reads from that same root — so a service declared in a submodule is reached by
//! the names it hoists there, one `pub use` covering the module and one covering each type the
//! macro names. A service declared at the crate root hoists nothing.

use super::Transport;
use crate::service_schema::parse::{OperationDef, OperationInputs, OperationOutcome, ServiceDef};
use crate::service_schema::support::{message_alias_ident, message_validator_ident, module_ident};
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

pub fn emit(service: &ServiceDef, transport: Transport) -> TokenStream {
    let contract = &service.ident;
    let module = module_ident(service);
    let macro_name = super::dispatcher_macro_ident(service, transport);
    let arms = service
        .operations
        .iter()
        .map(|operation| arm(&module, operation));
    let macro_doc = format!(
        "The `{contract}` dispatcher for the `{}` transport, held as tokens rather than compiled \
         here.\n\n\
         It takes no arguments and emits bare items — `IncomingMessage`, the panic guard and \
         `dispatch` — so the caller supplies the module they land in and two transports in one \
         crate cannot collide. The invoking crate names `serde_json` and `tracing` in its own \
         manifest, because the items below call them.",
        transport.name()
    );
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
    let reader = refusal_reader(&module);
    let guard = panic_guard();
    quote! {
        #[doc = #macro_doc]
        #[macro_export]
        macro_rules! #macro_name {
            () => {
                #incoming
                #reader
                #guard

                #[doc = #dispatch_doc]
                pub fn dispatch<S, Ctx, R>(
                    svc: &S,
                    ctx: &Ctx,
                    message: &IncomingMessage,
                    reply: &R,
                ) -> impl ::core::future::Future<Output = ()> + Send
                where
                    S: $crate::#contract<Ctx> + Sync,
                    Ctx: Sync,
                    R: $crate::#module::Reply + Sync,
                {
                    async move {
                        match message.operation.as_str() {
                            #(#arms)*
                            unrecognised => {
                                reply
                                    .fault($crate::#module::ServiceFault::unknown_operation(
                                        unrecognised,
                                    ))
                                    .await
                            }
                        }
                    }
                }
            };
        }
    }
}

/// One arm: deserialize, validate, call behind the panic guard, record and answer. Every fault path
/// names the wire name rather than what arrived, this arm being the one that answered to it.
///
/// A one-way arm answers nothing once the implementation has been entered, a panic included: the
/// operation declared no reply and the delivery carries no queue for one to go to. What the guard
/// buys there is the return itself — the transport acknowledges after `dispatch` returns, and a
/// panic that unwound past it would leave the delivery outstanding. The record is what keeps that
/// return from being silent, so a panic is written down on both outcomes.
fn arm(module: &Ident, operation: &OperationDef) -> TokenStream {
    let wire = &operation.wire_name;
    let message = message_alias_ident(operation);
    let validator = message_validator_ident(operation);
    let call = call_arguments(operation);
    let method = &operation.ident;
    let called = quote! { caught(move || svc.#method(ctx #(, #call)*)).await };
    let settled = match operation.outcome {
        OperationOutcome::OneWay => quote! {
            if let Err(panicked) = #called {
                record_panic(#wire, &panicked);
            }
        },
        OperationOutcome::Reply { .. } => quote! {
            match #called {
                Ok(answered) => {
                    reply.send($crate::#module::Answered::answering(answered)).await
                }
                Err(panicked) => {
                    record_panic(#wire, &panicked);
                    reply
                        .fault($crate::#module::ServiceFault::handler_panic(#wire, &panicked))
                        .await
                }
            }
        },
    };
    quote! {
        #wire => {
            let received = match ::serde_json::from_slice::<$crate::#module::#message>(
                &message.payload,
            ) {
                Ok(received) => received,
                Err(rejected) => {
                    return reply.fault(refused_payload(#wire, &rejected)).await;
                }
            };
            if let Err(violations) = $crate::#module::#validator(&received) {
                return reply
                    .fault($crate::#module::ServiceFault::failed_validation(
                        #wire,
                        $crate::#module::violated_field(&violations),
                        &$crate::#module::violation_detail(&violations),
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

/// The guard a handler is called behind, the record a caught panic leaves, and the reader that
/// turns one into a detail.
///
/// Two things make this the arm's business rather than the transport adapter's. The delivery is
/// acknowledged after `dispatch` returns, so a panic that unwound past it is never acknowledged at
/// all — and the consumer this was measured against asks for manual acknowledgement with no
/// `nack`, no dead-letter exchange, no message TTL and no timeout, so that delivery stays
/// outstanding against the prefetch until the channel closes. And a handler that panicked failed at
/// something its operation never declared, which is exactly what a fault reports.
///
/// Catching a panic without writing it down would trade a stalled consumer for a silent one, so
/// every caught panic is recorded through `tracing::error!`. `tracing` is one of the two runtime
/// crates the *invoking* crate names in its own manifest, beside `serde_json`, and it is named for
/// the same reason: the tokens below call it.
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

        /// Writes down that a handler came apart, so that catching a panic is not the same as
        /// losing it.
        ///
        /// It runs on both outcomes. A one-way operation declared no reply and its delivery
        /// carries no queue for one to go to, so without this the panic is visible to nobody at
        /// all. A request-and-reply operation answers its caller a fault, and that is the
        /// *caller's* record rather than the operator's — the two are frequently not the same
        /// party, and a panic is a defect in this service whichever way its operation was
        /// declared. So both write the same event, and a service reads its handlers' failures off
        /// one place.
        ///
        /// `tracing` is named because the operator's subscriber is where a service's records
        /// already go. The default panic hook has printed the panic to stderr by the time this
        /// runs, but that line carries no operation name, is not structured, and is gone entirely
        /// under a hook the service replaced.
        fn record_panic(operation: &str, detail: &str) {
            ::tracing::error!(
                operation = operation,
                detail = detail,
                "the handler for this operation panicked"
            );
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

/// Which fault a `serde_json` refusal is, and the reader of the field serde names in its own words.
///
/// Both travel with the dispatcher rather than sitting in the generated module, because the
/// refusal they read is a `serde_json::Error` belonging to the crate that read the payload.
fn refusal_reader(module: &Ident) -> TokenStream {
    quote! {
        /// The field serde names in its own words. It writes one into exactly two sentences —
        /// `missing field \u{60}creditCount\u{60}` and
        /// `unknown field \u{60}extra\u{60}, expected …` — and into both between backticks. A
        /// refusal that names none, a type mismatch saying what it expected and not where, leaves
        /// the fault's field empty.
        ///
        /// The name it carries is the key as the wire spells it, since that is the name serde was
        /// reading for; a validator's report names the Rust field, that being what it holds.
        fn serde_named_field(reported: &str) -> Option<&str> {
            let named = reported
                .strip_prefix("missing field ")
                .or_else(|| reported.strip_prefix("unknown field "))?;
            let (field, _rest) = named.strip_prefix('`')?.split_once('`')?;
            Some(field)
        }

        /// Which fault a serde refusal is, and what it says.
        ///
        /// serde_json classifies its own refusals, and that classification is the line between the
        /// two kinds. `Syntax` and `Eof` say the bytes are not a document at all, which is a
        /// sender whose serialization is broken. `Data` says the bytes read as a document and did
        /// not match the message — a value someone supplied that the message does not admit, which
        /// is the same failure the validator answers for and is answered under the same kind. That
        /// is where the TypeScript service serving the same operation draws it too: its reader
        /// parses the payload, and its schema then judges what was read, so a type mismatch and a
        /// broken bound are one kind there and are one kind here.
        ///
        /// The byte offset serde appends is dropped. It locates the failure inside an encoding the
        /// caller never saw, and it is removed by rebuilding it from the refusal's own line and
        /// column rather than by matching the sentence for it.
        fn refused_payload(
            operation: &str,
            refusal: &::serde_json::Error,
        ) -> $crate::#module::ServiceFault {
            let reported = refusal.to_string();
            let offset = format!(
                " at line {} column {}",
                refusal.line(),
                refusal.column()
            );
            let said = reported.strip_suffix(&offset).unwrap_or(reported.as_str());
            if matches!(refusal.classify(), ::serde_json::error::Category::Data) {
                let named = $crate::#module::named_field(said)
                    .or_else(|| serde_named_field(said));
                return $crate::#module::ServiceFault::failed_validation(operation, named, said);
            }
            $crate::#module::ServiceFault::undeserializable_payload(operation, said)
        }
    }
}
