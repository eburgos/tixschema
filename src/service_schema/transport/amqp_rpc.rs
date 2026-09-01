//! The AMQP request-and-reply transport: two inert macros, `{service}_amqp_rpc_dispatcher!` and
//! `{service}_amqp_rpc_client!`, and nothing compiled where the service is declared.
//!
//! Two macros rather than one, because the two halves of a service usually live in different
//! crates: a crate that calls the service can see the contract but has no business seeing the
//! server's backend, and a server crate has no use for a client. Each is invoked and placed by the
//! half that wants it, neither drags in the other, and a crate that wants both places both.
//!
//! # The dispatcher: what one arm does, and in what order
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
//! # The client
//!
//! One method per operation, returning the operation's success type or a call error that is either
//! the operation's own error or a fault — one the remote produced, one this client raised against
//! its own outgoing message, or one the transport reported about a call that never landed.
//!
//! ## Three outcomes, two arms
//!
//! A call succeeds, returns the error the operation declared, or produces a fault. `Result` has
//! two arms, so the failure arm carries `CallError<E>`. A Rust service cannot *produce* a fault —
//! its signature admits only its own error type — but a Rust client can *receive* one, because the
//! remote it called produced it.
//!
//! ## Outbound validation comes before the transport, not after it
//!
//! The client runs the outgoing message's own validator first. A failure returns
//! `Err(CallError::Fault(…))` naming the field **without touching the transport**: the operation
//! never ran, so it is not a declared error, and a caller's code is identical whether the fault
//! came from its own validator or from the far end.
//!
//! ## A transport that could not carry the call has somewhere to say so
//!
//! Both transport methods answer a `Result`, the failure arm carrying whatever the transport wants
//! to say in words. Without one, a transport that knows its reply is never coming — a deadline it
//! imposed, a connection that went away — can only panic or hang, and the caller is left holding a
//! call that never completes and no fault to report. The client turns that arm into
//! `ServiceFaultKind::TransportFailure`, so a caller reads a call that did not travel the same way
//! it reads every other defect. Whether a deadline exists at all, and how long it is, stays the
//! transport's: this is where the answer is reported, not where it is decided.
//!
//! ## Reading a fault back
//!
//! `ServiceFault` derives `Serialize` and deliberately not `Deserialize`, so a fault never arrives
//! simply by having been written on the wire. The client deserializes into a private mirror that
//! does derive it and mints the fault through the constructors the service's module publishes. The
//! mirror is the seam; the seal on the fault survives it.
//!
//! ## A client takes no context
//!
//! The trait's context exists for the thing that answers: a logger, and later whatever else an
//! implementation reaches for that has no business being in a message. A caller has no
//! implementation to hand one to, so a client method takes the operation's arguments and nothing
//! else — which is what the generated TypeScript client has always taken.
//!
//! # Why the tokens sit inside a macro
//!
//! Everything below is a stored token sequence in the crate that declares the service, and is
//! compiled only in the crate that invokes the macro. That is what keeps `serde_json` and
//! `tracing` out of the declaring crate's manifest, and what lets a service decline this shape
//! entirely: `IncomingMessage`'s operation-name-over-opaque-bytes routing is one transport model,
//! and a service that asks for no transport is emitted none of it.
//!
//! Each macro emits bare items and opens no module of its own, so the caller names the module and
//! two transports in one crate cannot collide:
//!
//! ```text
//! mod amqp_transport {
//!     declaring_crate::usage_service_amqp_rpc_dispatcher!();
//! }
//!
//! mod amqp_client {
//!     use declaring_crate::*;
//!     declaring_crate::usage_service_amqp_rpc_client!();
//! }
//! ```
//!
//! # Where a path inside the macro resolves
//!
//! Paths in a `macro_rules!` body resolve at the *invocation* site, so the two kinds are spelled
//! apart. Everything tixschema generated is reached through `$crate::` — the trait and the message
//! types at the scope the author declared them in, and the fault, the envelope, the message aliases
//! and the per-operation validators inside `$crate::{service}_schema` — which resolves in the crate
//! that *defined* the macro. Every runtime crate is reached through a leading `::` and resolves in
//! the invoking crate, which is therefore the one that names it: `::serde`, `::serde_json`,
//! `::tracing`, `::core` and `::std` for the dispatcher, `::serde`, `::serde_json` and `::core` for
//! the client. The client reaches no `::tracing` — nothing there catches a panic, so nothing there
//! has anything to write down, and a caller that only wants to make calls names one crate fewer
//! than a crate that answers them.
//!
//! What each macro writes itself is named bare: those items land in the module the caller supplied
//! and exist nowhere else. The dispatcher's are `IncomingMessage`, the `Reply` handle, the panic
//! guard and the reader that classifies a serde refusal; the client's are the `Transport` trait,
//! the fault mirror, the client type and the reader that turns one envelope into three outcomes.
//! The two sets do not overlap, so a crate that wants both can place them in one module.
//!
//! A type the *author* wrote is the one thing spelled as they wrote it: `Vec<Slug>` and `String`
//! share no crate prefix that would be true of both. The module the client macro is invoked in
//! supplies them, exactly as the service's own module supplies them through its `use super::*` —
//! and a caller has them in scope regardless, having to build the messages it sends.
//!
//! `#[macro_export]` puts each macro at the declaring crate's root whatever module it was written
//! in, and `$crate` reads from that same root — so a service declared in a submodule is reached by
//! the names it hoists there. Two of them are the declaration's own and are checked where it is
//! written: the service's generated module, which everything below the dispatcher is reached
//! through, and the trait, which the dispatcher's `where` clause binds. `support::root_anchors`
//! resolves both at the declaration, so a crate that leaves either unreachable stops compiling
//! itself rather than breaking every crate that goes on to invoke a macro. The client adds one
//! more class, unchecked: it builds each message the macro declared as `$crate::{Operation}Request`
//! at the root, so a service in a submodule re-exports those too. A service declared at the crate
//! root hoists nothing.

use super::Transport;
use crate::service_schema::parse::{OperationDef, OperationInputs, OperationOutcome, ServiceDef};
use crate::service_schema::support::{message_alias_ident, message_validator_ident, module_ident};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

/// The names the service's own module publishes that the client writes, each spelled so it resolves
/// from the crate the macro is invoked in.
struct Generated {
    call_error: TokenStream,
    fault: TokenStream,
    module: Ident,
}

impl Generated {
    fn of(module: Ident) -> Self {
        Self {
            call_error: quote! { $crate::#module::CallError },
            fault: quote! { $crate::#module::ServiceFault },
            module,
        }
    }
}

pub fn emit(service: &ServiceDef, transport: Transport) -> TokenStream {
    let dispatcher = dispatcher_macro(service, transport);
    let client = client_macro(service, transport);
    quote! {
        #dispatcher
        #client
    }
}

/// The reader that turns one answer off the wire into the three outcomes a call has.
fn answer_reader(generated: &Generated) -> TokenStream {
    let Generated {
        call_error,
        fault,
        module,
    } = generated;
    quote! {
        /// The three outcomes, read out of one envelope: the value, the error the operation
        /// declared, or a fault. An envelope that contradicts itself — `ok` with no value, or a
        /// failure with no error — is itself a defect and becomes a fault.
        fn read_answer<S, E>(operation: &str, encoded: &[u8]) -> Result<S, #call_error<E>>
        where
            S: ::serde::de::DeserializeOwned,
            E: ::serde::de::DeserializeOwned,
        {
            let answered = match ::serde_json::from_slice::<
                $crate::#module::Answered<S, ReportedError<E>>,
            >(encoded) {
                Ok(answered) => answered,
                Err(rejected) => {
                    return Err(#call_error::Fault(#fault::undeserializable_payload(
                        operation,
                        &rejected.to_string(),
                    )));
                }
            };
            match answered.carried() {
                Ok(Some(value)) => Ok(value),
                Ok(None) => Err(#call_error::Fault(#fault::undeserializable_payload(
                    operation,
                    "the answer said `ok` and carried no value",
                ))),
                Err(None) => Err(#call_error::Fault(#fault::undeserializable_payload(
                    operation,
                    "the answer said it had failed and carried no error",
                ))),
                Err(Some(ReportedError::Fault(tagged))) => {
                    Err(#call_error::Fault(tagged.reported(operation)))
                }
                Err(Some(ReportedError::Operation(declared))) => {
                    Err(#call_error::Operation(declared))
                }
            }
        }
    }
}

/// What one operation's client method answers.
fn answers(operation: &OperationDef, generated: &Generated) -> TokenStream {
    let Generated {
        call_error, fault, ..
    } = generated;
    match &operation.outcome {
        OperationOutcome::OneWay => quote! { Result<(), #fault> },
        OperationOutcome::Reply { error, success } => {
            quote! { Result<#success, #call_error<#error>> }
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

/// The arguments an operation's client method takes, and the message they are packed into before it
/// is sent.
fn call_message(operation: &OperationDef) -> (Vec<TokenStream>, TokenStream) {
    match &operation.inputs {
        OperationInputs::Empty => {
            let declared = operation.generated_message_ident();
            (Vec::new(), quote! { let sending = $crate::#declared {}; })
        }
        OperationInputs::Generated(arguments) => {
            let declared = operation.generated_message_ident();
            let taken = arguments
                .iter()
                .map(|(field, carried)| quote! { #field: #carried })
                .collect();
            let fields = arguments.iter().map(|(field, _)| field);
            (
                taken,
                quote! { let sending = $crate::#declared { #(#fields,)* }; },
            )
        }
        OperationInputs::Named(declared) => (
            vec![quote! { req: #declared }],
            quote! { let sending = req; },
        ),
    }
}

/// The client half: the transport seam, the fault mirror, the client type and one method per
/// operation, held as tokens for whoever wants to make calls.
fn client_macro(service: &ServiceDef, transport: Transport) -> TokenStream {
    let contract = &service.ident;
    let client = format_ident!("{contract}Client", span = contract.span());
    let published = super::client_macro_ident(service, transport);
    let generated = Generated::of(module_ident(service));
    let methods = service
        .operations
        .iter()
        .map(|operation| method(operation, &generated));
    let macro_doc = format!(
        "The `{contract}` client for the `{}` transport, held as tokens rather than compiled \
         here.\n\n\
         It takes no arguments and emits bare items - the transport seam, the client type and one \
         method per operation - so the module they land in is the invoking crate's to name:\n\n\
         ```text\n\
         mod amqp_client {{\n    \
         use the_contract_crate::*;\n    \
         the_contract_crate::{published}!();\n\
         }}\n\
         ```\n\n\
         The invoking crate names `serde` and `serde_json` in its own manifest, the expansion \
         calling both. It names no `tracing`: nothing here catches a panic, so nothing here has \
         anything to write down. The `use` above is what resolves the types the author declared, \
         which are spelled here as they were written there.",
        transport.name()
    );
    let client_doc = format!(
        "A `{contract}` caller, over any transport that can send an operation name beside a \
         payload.\n\n\
         Every operation on the trait has a method here, taking that operation's arguments and \
         nothing else: the context is the implementation's and never reaches a caller. A \
         request-and-reply operation answers `Result<Success, CallError<Error>>`; a one-way \
         operation answers nothing beyond the send, save for the fault it owes when the message it \
         was handed fails its own validation or the transport could not put it out."
    );
    let seam = transport_trait(contract);
    let mirror = fault_mirror(&generated);
    let reader = answer_reader(&generated);
    quote! {
        #[doc = #macro_doc]
        #[macro_export]
        macro_rules! #published {
            () => {
                #seam
                #mirror

                #[doc = #client_doc]
                pub struct #client<T: Transport> {
                    transport: T,
                }

                impl<T: Transport> #client<T> {
                    /// Binds a client to a transport.
                    pub const fn new(transport: T) -> Self {
                        Self { transport }
                    }

                    /// The transport this client was bound to.
                    pub const fn transport(&self) -> &T {
                        &self.transport
                    }
                }

                // The operations sit apart, under the `Sync` a call's future needs: it borrows the
                // client across an await, and a borrow is only `Send` where what it borrows is
                // `Sync`. Binding a client asks for no such thing.
                impl<T: Transport + Sync> #client<T> {
                    #(#methods)*
                }

                #reader
            };
        }
    }
}

/// The dispatcher half: everything that turns one delivery into a call on an implementation, held
/// as tokens for whoever answers the service.
fn dispatcher_macro(service: &ServiceDef, transport: Transport) -> TokenStream {
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
         It takes no arguments and emits bare items — `IncomingMessage`, `Reply`, the panic guard \
         and `dispatch` — so the caller supplies the module they land in and two transports in one \
         crate cannot collide. The invoking crate names `serde`, `serde_json` and `tracing` in its \
         own manifest, because the items below call them.",
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
    let reply = reply_trait(contract, &module);
    let reader = refusal_reader(&module);
    let guard = panic_guard();
    quote! {
        #[doc = #macro_doc]
        #[macro_export]
        macro_rules! #macro_name {
            () => {
                #incoming
                #reply
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
                    R: Reply + Sync,
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

/// The private mirror of `ServiceFault`, the only thing here that deserializes one, and the tagged
/// member the failure arm carries it in.
///
/// A fault is minted through the constructors the service's module publishes rather than written as
/// a literal: the fields are private and this expands outside the module they are private to. Each
/// kind therefore carries exactly what its own constructor carries.
fn fault_mirror(generated: &Generated) -> TokenStream {
    let Generated { fault, .. } = generated;
    quote! {
        /// A fault as it arrives, which is the one shape that reads one back. `ServiceFault`
        /// derives no `Deserialize` of its own, that being a public constructor by another name.
        #[derive(::serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct FaultOnTheWire {
            detail: String,
            field: Option<String>,
            kind: FaultKindOnTheWire,
            operation: String,
        }

        /// The kinds, spelled as the wire spells them.
        #[derive(::serde::Deserialize)]
        #[serde(rename_all = "kebab-case")]
        enum FaultKindOnTheWire {
            FailedValidation,
            HandlerPanic,
            TransportFailure,
            UndeserializablePayload,
            UnknownOperation,
        }

        /// What the failure arm carries when the failure was never declared, tagged so a caller in
        /// either language can tell it from the operation's own error.
        #[derive(::serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct TaggedFault {
            fault: FaultOnTheWire,
            is_service_fault: bool,
        }

        /// What a failure arm holds: a fault, or the error the operation declared. The tagged
        /// member is tried first, which is what the tag is for.
        #[derive(::serde::Deserialize)]
        #[serde(untagged)]
        enum ReportedError<E> {
            Fault(TaggedFault),
            Operation(E),
        }

        impl FaultOnTheWire {
            /// The fault itself, minted through the constructors the service's own module
            /// publishes: its fields are private, and this reads a fault back from wherever the
            /// caller placed the client.
            fn into_fault(self) -> #fault {
                match self.kind {
                    FaultKindOnTheWire::FailedValidation => #fault::failed_validation(
                        &self.operation,
                        self.field.as_deref(),
                        &self.detail,
                    ),
                    FaultKindOnTheWire::HandlerPanic => {
                        #fault::handler_panic(&self.operation, &self.detail)
                    }
                    FaultKindOnTheWire::TransportFailure => {
                        #fault::transport_failure(&self.operation, &self.detail)
                    }
                    FaultKindOnTheWire::UndeserializablePayload => {
                        #fault::undeserializable_payload(&self.operation, &self.detail)
                    }
                    FaultKindOnTheWire::UnknownOperation => {
                        #fault::unknown_operation(&self.operation)
                    }
                }
            }
        }

        impl TaggedFault {
            /// The fault the remote reported. A failure arm that named the tag and then denied it
            /// is a contradiction, and a contradiction on the wire is itself a defect.
            fn reported(self, operation: &str) -> #fault {
                if self.is_service_fault {
                    self.fault.into_fault()
                } else {
                    #fault::undeserializable_payload(
                        operation,
                        "the failure arm tagged itself `isServiceFault: false`",
                    )
                }
            }
        }
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

/// One operation's client method: validate, then send. The transport is reached only once the
/// message has passed its own validator, which is what makes the never-called-transport case
/// observable.
fn method(operation: &OperationDef, generated: &Generated) -> TokenStream {
    let Generated {
        call_error,
        fault,
        module,
    } = generated;
    let wire = &operation.wire_name;
    let named = &operation.ident;
    let check = message_validator_ident(operation);
    let (taken, packed) = call_message(operation);
    let refusal = outbound_refusal(operation, generated);
    let answered = match &operation.outcome {
        OperationOutcome::OneWay => quote! {
            self.transport
                .notify(#wire, sending)
                .await
                .map_err(|uncarried| #fault::transport_failure(#wire, &uncarried))
        },
        OperationOutcome::Reply { .. } => quote! {
            match self.transport.request(#wire, sending).await {
                Ok(encoded) => read_answer(#wire, &encoded),
                Err(uncarried) => Err(#call_error::Fault(#fault::transport_failure(
                    #wire,
                    &uncarried,
                ))),
            }
        },
    };
    let answers = answers(operation, generated);
    let doc = method_doc(operation);
    quote! {
        #[doc = #doc]
        pub fn #named(
            &self #(, #taken)*
        ) -> impl ::core::future::Future<Output = #answers> + Send {
            async move {
                #packed
                if let Err(violations) = $crate::#module::#check(&sending) {
                    #refusal
                }
                #answered
            }
        }
    }
}

fn method_doc(operation: &OperationDef) -> String {
    let named = &operation.ident;
    let carried = &operation.wire_name;
    let context = " No context is taken. A context is what an implementation needs and a caller \
                    has nothing to hand one to, so a call carries the message and nothing else.";
    match operation.outcome {
        OperationOutcome::OneWay => format!(
            " Sends `{carried}`, which expects no reply.\n\n\
             Nothing is awaited beyond the send, there being no reply to carry an error. The two \
             failures it can report are both about the send itself: a message that fails its own \
             validator never reaches the transport, and the fault names the field; a message the \
             transport could not put out comes back as a `transport-failure` fault carrying what \
             the transport said.\n\n\
            {context}"
        ),
        OperationOutcome::Reply { .. } => format!(
            " Calls `{carried}` and waits for the answer.\n\n\
             `Err(CallError::Operation(…))` is the error `{named}` declared. \
             `Err(CallError::Fault(…))` is a defect: the remote produced one, this client refused \
             the message it was about to send, in which case the transport was never reached, or \
             the transport reported that the call never landed.\n\n\
            {context}"
        ),
    }
}

/// What a message failing its own validation answers, which is a fault either way and never a
/// transport call.
fn outbound_refusal(operation: &OperationDef, generated: &Generated) -> TokenStream {
    let Generated {
        call_error,
        fault,
        module,
    } = generated;
    let wire = &operation.wire_name;
    let built = quote! {
        #fault::failed_validation(
            #wire,
            $crate::#module::violated_field(&violations),
            &$crate::#module::violation_detail(&violations),
        )
    };
    match operation.outcome {
        OperationOutcome::OneWay => quote! { return Err(#built); },
        OperationOutcome::Reply { .. } => quote! { return Err(#call_error::Fault(#built)); },
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
/// every caught panic is recorded through `tracing::error!`. `tracing` is one of the runtime crates
/// the *invoking* crate names in its own manifest, beside `serde` and `serde_json`, and it is named
/// for the same reason: the tokens below call it.
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

/// The `Reply` trait, which a transport implements once per dispatcher it places.
///
/// It travels with the dispatcher because its shape is the dispatcher's: one reply per message,
/// answered with a value or with a defect.
fn reply_trait(contract: &Ident, module: &Ident) -> TokenStream {
    let reply_doc = format!(
        "The handle a transport gives the `{contract}` dispatcher so it can settle *this* \
         message.\n\n\
         A request-and-reply operation answers with [`send`](Reply::send) or \
         [`fault`](Reply::fault). A one-way operation calls neither, and the transport \
         acknowledges the delivery after dispatch returns. Encoding sits behind the trait, which \
         is what keeps the generator out of the wire format."
    );
    quote! {
        #[doc = #reply_doc]
        pub trait Reply {
            /// Answer with a defect the operation never declared.
            fn fault(
                &self,
                fault: $crate::#module::ServiceFault,
            ) -> impl ::core::future::Future<Output = ()> + Send;

            /// Answer with a value. The transport serializes it, which is why it is handed the
            /// value rather than an encoded buffer.
            fn send<T>(&self, value: T) -> impl ::core::future::Future<Output = ()> + Send
            where
                T: ::serde::Serialize + Send;
        }
    }
}

/// The `Transport` trait, which is what a caller supplies to bind a client to a bus.
fn transport_trait(contract: &Ident) -> TokenStream {
    let transport_doc = format!(
        "What binds a `{contract}` client to a bus.\n\n\
         The operation name travels beside the payload rather than inside it, so no message type \
         has to reserve a key for routing. The payload is handed over as a value rather than as \
         bytes, for the same reason `Reply::send` is: a transport merges its own fields — a \
         correlation id, an error flag — into the object before serializing it, and neither is \
         reachable behind an encoded buffer.\n\n\
         Both methods answer a `Result`. `Err` is for a call that did not travel — a deadline the \
         transport imposed, a connection that went away — and carries whatever the transport wants \
         to say about it in words; the client turns it into a fault of kind `transport-failure`. \
         Deadlines, retries and backpressure are the transport's own: this arm is where the answer \
         is reported, not where it is decided."
    );
    quote! {
        #[doc = #transport_doc]
        pub trait Transport {
            /// Sends a message no reply is expected for, answering `Err` with what stopped it in
            /// words if the message never went out.
            fn notify<T>(
                &self,
                operation: &str,
                payload: T,
            ) -> impl ::core::future::Future<Output = Result<(), String>> + Send
            where
                T: ::serde::Serialize + Send;

            /// Sends a message and answers with the encoded reply, or `Err` with what stopped it
            /// in words if the call never landed and no reply is coming.
            fn request<T>(
                &self,
                operation: &str,
                payload: T,
            ) -> impl ::core::future::Future<Output = Result<Vec<u8>, String>> + Send
            where
                T: ::serde::Serialize + Send;
        }
    }
}
