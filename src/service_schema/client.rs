//! The Rust client: one method per operation, returning the operation's success type or a call
//! error that is either the operation's own error or a fault — one the remote produced, one this
//! client raised against its own outgoing message, or one the transport reported about a call that
//! never landed.
//!
//! # Three outcomes, two arms
//!
//! A call succeeds, returns the error the operation declared, or produces a fault. `Result` has
//! two arms, so the failure arm carries `CallError<E>`. A Rust service cannot *produce* a fault —
//! its signature admits only its own error type — but a Rust client can *receive* one, because the
//! remote it called produced it.
//!
//! # Outbound validation comes before the transport, not after it
//!
//! The client runs the outgoing message's own `validate()` first. A failure returns
//! `Err(CallError::Fault(…))` naming the field **without touching the transport**: the operation
//! never ran, so it is not a declared error, and a caller's code is identical whether the fault
//! came from its own validator or from the far end. This is the second of the two generated places
//! allowed to build a fault, which is why it is emitted inside the module the constructors are
//! private to.
//!
//! # A transport that could not carry the call has somewhere to say so
//!
//! Both transport methods answer a `Result`, the failure arm carrying whatever the transport wants
//! to say in words. Without one, a transport that knows its reply is never coming — a deadline it
//! imposed, a connection that went away — can only panic or hang, and the caller is left holding a
//! call that never completes and no fault to report. The client turns that arm into
//! `ServiceFaultKind::TransportFailure`, so a caller reads a call that did not travel the same way
//! it reads every other defect. Whether a deadline exists at all, and how long it is, stays the
//! transport's: this is where the answer is reported, not where it is decided.
//!
//! # Reading a fault back
//!
//! `ServiceFault` derives `Serialize` and deliberately not `Deserialize`, a public `Deserialize`
//! being a public constructor by another name. So the client deserializes into a private mirror
//! that does derive it and converts, inside the module, where the fields are reachable. The mirror
//! is the seam; the seal on the fault survives it.
//!
//! # A client takes no context
//!
//! The trait's context exists for the thing that answers: a logger, and later whatever else an
//! implementation reaches for that has no business being in a message. A caller has no
//! implementation to hand one to, so a client method takes the operation's arguments and nothing
//! else — which is what the generated TypeScript client has always taken.
//!
//! # What this file reads from [`dispatch`](super::dispatch)
//!
//! `Answered`, `MessageValidation`, `violated_field` and `violation_detail`, all of which land in
//! the same module. The dispatcher writes the envelope this reads, which is the point of their
//! sharing one. `MessageValidation` is the one of the four that is not in scope where it lands —
//! it is shut in a private module, so each client method opens its body by `use`ing it.

use super::parse::{OperationDef, OperationInputs, OperationOutcome, ServiceDef};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

pub fn emit(service: &ServiceDef) -> TokenStream {
    let contract = &service.ident;
    let client = format_ident!("{contract}Client", span = contract.span());
    let methods = service.operations.iter().map(method);
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
    let mirror = fault_mirror();
    let reader = answer_reader();
    quote! {
        #transport
        #mirror
        #reader

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

        // The operations sit apart, under the `Sync` a call's future needs: it borrows the client
        // across an await, and a borrow is only `Send` where what it borrows is `Sync`. Binding a
        // client asks for no such thing.
        impl<T: Transport + Sync> #client<T> {
            #(#methods)*
        }
    }
}

/// The reader that turns one answer off the wire into the three outcomes a call has.
fn answer_reader() -> TokenStream {
    quote! {
        /// The three outcomes, read out of one envelope: the value, the error the operation
        /// declared, or a fault. An envelope that contradicts itself — `ok` with no value, or a
        /// failure with no error — is itself a defect and becomes a fault.
        fn read_answer<S, E>(operation: &str, encoded: &[u8]) -> Result<S, CallError<E>>
        where
            S: ::serde::de::DeserializeOwned,
            E: ::serde::de::DeserializeOwned,
        {
            let answered = match ::serde_json::from_slice::<Answered<S, ReportedError<E>>>(encoded)
            {
                Ok(answered) => answered,
                Err(rejected) => {
                    return Err(CallError::Fault(ServiceFault::undeserializable_payload(
                        operation,
                        &rejected.to_string(),
                    )));
                }
            };
            if answered.ok {
                return answered.value.ok_or_else(|| {
                    CallError::Fault(ServiceFault::undeserializable_payload(
                        operation,
                        "the answer said `ok` and carried no value",
                    ))
                });
            }
            Err(match answered.error {
                None => CallError::Fault(ServiceFault::undeserializable_payload(
                    operation,
                    "the answer said it had failed and carried no error",
                )),
                Some(ReportedError::Fault(tagged)) => CallError::Fault(tagged.reported(operation)),
                Some(ReportedError::Operation(declared)) => CallError::Operation(declared),
            })
        }
    }
}

/// The arguments an operation's client method takes, and the message they are packed into before
/// it is sent.
fn call_message(operation: &OperationDef) -> (Vec<TokenStream>, TokenStream) {
    match &operation.inputs {
        OperationInputs::Empty => {
            let declared = operation.generated_message_ident();
            (Vec::new(), quote! { let sending = #declared {}; })
        }
        OperationInputs::Generated(arguments) => {
            let declared = operation.generated_message_ident();
            let taken = arguments
                .iter()
                .map(|(field, carried)| quote! { #field: #carried })
                .collect();
            let fields = arguments.iter().map(|(field, _)| field);
            (taken, quote! { let sending = #declared { #(#fields,)* }; })
        }
        OperationInputs::Named(declared) => (
            vec![quote! { req: #declared }],
            quote! { let sending = req; },
        ),
    }
}

/// The private mirror of `ServiceFault`, the only thing here that deserializes one, and the tagged
/// member the failure arm carries it in.
fn fault_mirror() -> TokenStream {
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
            /// The fault itself. The fields are private and this is inside the module they are
            /// private to, so nothing is widened to read one back.
            fn into_fault(self) -> ServiceFault {
                ServiceFault {
                    detail: self.detail,
                    field: self.field,
                    kind: match self.kind {
                        FaultKindOnTheWire::FailedValidation => ServiceFaultKind::FailedValidation,
                        FaultKindOnTheWire::HandlerPanic => ServiceFaultKind::HandlerPanic,
                        FaultKindOnTheWire::TransportFailure => ServiceFaultKind::TransportFailure,
                        FaultKindOnTheWire::UndeserializablePayload => {
                            ServiceFaultKind::UndeserializablePayload
                        }
                        FaultKindOnTheWire::UnknownOperation => ServiceFaultKind::UnknownOperation,
                    },
                    operation: self.operation,
                }
            }
        }

        impl TaggedFault {
            /// The fault the remote reported. A failure arm that named the tag and then denied it
            /// is a contradiction, and a contradiction on the wire is itself a defect.
            fn reported(self, operation: &str) -> ServiceFault {
                if self.is_service_fault {
                    self.fault.into_fault()
                } else {
                    ServiceFault::undeserializable_payload(
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
fn method(operation: &OperationDef) -> TokenStream {
    let wire = &operation.wire_name;
    let named = &operation.ident;
    let (taken, packed) = call_message(operation);
    let refusal = outbound_refusal(operation);
    let answered = match &operation.outcome {
        OperationOutcome::OneWay => quote! {
            self.transport
                .notify(#wire, sending)
                .await
                .map_err(|uncarried| ServiceFault::transport_failure(#wire, &uncarried))
        },
        OperationOutcome::Reply { .. } => quote! {
            match self.transport.request(#wire, sending).await {
                Ok(encoded) => read_answer(#wire, &encoded),
                Err(uncarried) => Err(CallError::Fault(ServiceFault::transport_failure(
                    #wire,
                    &uncarried,
                ))),
            }
        },
    };
    let answers = answers(operation);
    let doc = method_doc(operation);
    let in_scope = super::dispatch::message_validation_in_scope();
    quote! {
        #[doc = #doc]
        pub fn #named(
            &self #(, #taken)*
        ) -> impl ::core::future::Future<Output = #answers> + Send {
            async move {
                #in_scope
                #packed
                if let Err(violations) = sending.validate() {
                    #refusal
                }
                #answered
            }
        }
    }
}

/// What one operation's method answers.
fn answers(operation: &OperationDef) -> TokenStream {
    match &operation.outcome {
        OperationOutcome::OneWay => quote! { Result<(), ServiceFault> },
        OperationOutcome::Reply { error, success } => {
            quote! { Result<#success, CallError<#error>> }
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
fn outbound_refusal(operation: &OperationDef) -> TokenStream {
    let wire = &operation.wire_name;
    let built = quote! {
        ServiceFault::failed_validation(
            #wire,
            violated_field(&violations),
            &violation_detail(&violations),
        )
    };
    match operation.outcome {
        OperationOutcome::OneWay => quote! { return Err(#built); },
        OperationOutcome::Reply { .. } => quote! { return Err(CallError::Fault(#built)); },
    }
}

/// The `Transport` trait, which is what a caller supplies to bind a client to a bus.
fn transport_trait(contract: &Ident) -> TokenStream {
    let transport_doc = format!(
        "What binds a `{contract}` client to a bus.\n\n\
         The operation name travels beside the payload rather than inside it, so no message type \
         has to reserve a key for routing. The payload is handed over as a value rather than as \
         bytes, for the same reason [`Reply::send`] is: a transport merges its own fields — a \
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
