//! The AMQP request-and-reply transport: what a service asking for `amqp_rpc` gets.
//!
//! One artifact so far, the Rust client, and it is emitted inert. A crate that calls the service
//! and a crate that answers it are usually not the same crate — a caller can see the contract but
//! has no business seeing the backend — so the client is published as a `macro_rules!` at the
//! declaring crate's root and expanded by whoever wants one, into a module of their own naming.
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
//! The client runs the outgoing message's own `validate()` first. A failure returns
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
//! # What a name inside the macro body resolves against
//!
//! Two rules, because the body is written here and expanded somewhere else entirely.
//!
//! Everything the declaring crate generates is reached through `$crate`: the message types beside
//! the trait, and `CallError`, `ServiceFault` and each operation's own check inside the service's
//! own module. [`Generated`] holds the first two spellings so no emitter below writes one by hand.
//!
//! The check is a function *there* rather than a `validate()` written here because which answer it
//! gives depends on the message's concrete type — an inherent method beats the fallback trait's —
//! and only the module that declared the message names it unambiguously.
//!
//! Every runtime crate carries a leading `::`, because it resolves in the **invoking** crate and
//! must be named in that crate's manifest. The client reaches two: `serde`, for the payload it
//! serializes and the answer it reads, and `serde_json`, for the envelope. It reaches no `tracing`
//! — nothing here catches a panic, so nothing here has anything to write down, and a caller that
//! only wants to make calls names one crate fewer than a crate that answers them.
//!
//! A type the *author* wrote is the one thing spelled as they wrote it: `Vec<Slug>` and `String`
//! share no crate prefix that would be true of both. So the module the macro is invoked in supplies
//! them, exactly as the generated module used to supply them through its own `use super::*` — and
//! a caller has them in scope regardless, having to build the messages it sends.

use super::super::dispatch::{validator_ident, violation_readers};
use super::super::parse::{OperationDef, OperationInputs, OperationOutcome, ServiceDef};
use super::super::support::module_ident;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

/// The two names the service's own module publishes that the client writes, each spelled so it
/// resolves from the crate the macro is invoked in.
struct Generated {
    call_error: TokenStream,
    fault: TokenStream,
}

impl Generated {
    fn of(module: &Ident) -> Self {
        Self {
            call_error: quote! { $crate::#module::CallError },
            fault: quote! { $crate::#module::ServiceFault },
        }
    }
}

pub fn emit(service: &ServiceDef) -> TokenStream {
    let contract = &service.ident;
    let client = format_ident!("{contract}Client", span = contract.span());
    let published = super::Transport::AmqpRpc.client_macro_ident(service);
    let module = module_ident(service);
    let generated = Generated::of(&module);
    let methods = service
        .operations
        .iter()
        .map(|operation| method(operation, &generated, &module));
    let macro_doc = format!(
        "The `{contract}` client over AMQP request-and-reply, ready to be placed.\n\n\
         It takes no arguments and expands to bare items - the transport seam, the client type and \
         one method per operation - so the module they land in is the invoking crate's to \
         name:\n\n\
         ```text\n\
         mod amqp_client {{\n    \
         use the_contract_crate::*;\n    \
         the_contract_crate::{published}!();\n\
         }}\n\
         ```\n\n\
         The invoking crate names `serde` and `serde_json` in its own manifest, the expansion \
         calling both. It names no `tracing`: nothing here catches a panic, so nothing here has \
         anything to write down. The `use` above is what resolves the types the author declared, \
         which are spelled here as they were written there."
    );
    let client_doc = format!(
        "A `{contract}` caller, over any transport that can send an operation name beside a \
         payload.\n\n\
         Every operation on the trait has a method here, taking that operation's arguments and \
         nothing else: the context is the implementation's and never reaches a caller. A \
         request-and-reply operation answers `Result<Success, CallError<Error>>`; a \
         one-way operation answers nothing beyond the send, save for the fault it owes when the \
         message it was handed fails its own validation or the transport could not put it out."
    );
    let transport = transport_trait(contract);
    let envelope = answered_envelope();
    let mirror = fault_mirror(&generated);
    let reader = answer_reader(&generated);
    let readers = violation_readers();
    quote! {
        #[doc = #macro_doc]
        #[macro_export]
        macro_rules! #published {
            () => {
                #transport
                #envelope
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
                #readers
            };
        }
    }
}

/// The reader that turns one answer off the wire into the three outcomes a call has.
fn answer_reader(generated: &Generated) -> TokenStream {
    let Generated { call_error, fault } = generated;
    quote! {
        /// The three outcomes, read out of one envelope: the value, the error the operation
        /// declared, or a fault. An envelope that contradicts itself — `ok` with no value, or a
        /// failure with no error — is itself a defect and becomes a fault.
        fn read_answer<S, E>(operation: &str, encoded: &[u8]) -> Result<S, #call_error<E>>
        where
            S: ::serde::de::DeserializeOwned,
            E: ::serde::de::DeserializeOwned,
        {
            let answered = match ::serde_json::from_slice::<Answered<S, ReportedError<E>>>(encoded)
            {
                Ok(answered) => answered,
                Err(rejected) => {
                    return Err(#call_error::Fault(#fault::undeserializable_payload(
                        operation,
                        &rejected.to_string(),
                    )));
                }
            };
            if answered.ok {
                return answered.value.ok_or_else(|| {
                    #call_error::Fault(#fault::undeserializable_payload(
                        operation,
                        "the answer said `ok` and carried no value",
                    ))
                });
            }
            Err(match answered.error {
                None => #call_error::Fault(#fault::undeserializable_payload(
                    operation,
                    "the answer said it had failed and carried no error",
                )),
                Some(ReportedError::Fault(tagged)) => #call_error::Fault(tagged.reported(operation)),
                Some(ReportedError::Operation(declared)) => #call_error::Operation(declared),
            })
        }
    }
}

/// The reading half of the `{ ok, value }` / `{ ok, error }` envelope the dispatcher writes.
///
/// The writing half stays with the dispatcher, which is the only side that ever builds one: a
/// constructor emitted here would be reached by nothing, and an unreached item in a caller's crate
/// is a warning that caller did not ask for.
fn answered_envelope() -> TokenStream {
    quote! {
        /// What a request-and-reply operation answered in: the envelope, with the message the
        /// operation declared left exactly as it is inside it.
        #[derive(::serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Answered<T, E> {
            error: Option<E>,
            ok: bool,
            value: Option<T>,
        }
    }
}

/// What one operation's method answers.
fn answers(operation: &OperationDef, generated: &Generated) -> TokenStream {
    let Generated { call_error, fault } = generated;
    match &operation.outcome {
        OperationOutcome::OneWay => quote! { Result<(), #fault> },
        OperationOutcome::Reply { error, success } => {
            quote! { Result<#success, #call_error<#error>> }
        }
    }
}

/// The arguments an operation's client method takes, and the message they are packed into before
/// it is sent.
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

/// The private mirror of `ServiceFault`, the only thing here that deserializes one, and the tagged
/// member the failure arm carries it in.
///
/// A fault is minted through the constructors the service's module publishes rather than written
/// as a literal: the fields are private and this expands outside the module they are private to.
/// Each kind therefore carries exactly what its own constructor carries.
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

/// One operation's method: validate, then send. The transport is reached only once the message has
/// passed its own validator, which is what makes the never-called-transport case observable.
fn method(operation: &OperationDef, generated: &Generated, module: &Ident) -> TokenStream {
    let Generated { call_error, fault } = generated;
    let wire = &operation.wire_name;
    let named = &operation.ident;
    let check = validator_ident(operation);
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
             `validate()` never reaches the transport, and the fault names the field; a message \
             the transport could not put out comes back as a `transport-failure` fault carrying \
             what the transport said.\n\n\
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
    let Generated { call_error, fault } = generated;
    let wire = &operation.wire_name;
    let built = quote! {
        #fault::failed_validation(
            #wire,
            violated_field(&violations),
            &violation_detail(&violations),
        )
    };
    match operation.outcome {
        OperationOutcome::OneWay => quote! { return Err(#built); },
        OperationOutcome::Reply { .. } => quote! { return Err(#call_error::Fault(#built)); },
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
