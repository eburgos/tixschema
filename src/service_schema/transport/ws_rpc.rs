//! The `ws_rpc` transport: the `amqp_rpc` request, notify and reply terms over one WebSocket as
//! JSON text frames.
//!
//! `{service}_ws_rpc_dispatcher!()` emits every item every transport built on the shared
//! dispatcher seam publishes — `IncomingMessage`, `Reply`, `dispatch` and the arm readers — plus
//! this transport's own frame codec (`Frame`, its `decode`, `ping_frame`/`pong_frame`),
//! `FrameReply` and `answer`: the one call an adapter makes per inbound text frame, decoding it,
//! dispatching a request or a notify, answering a ping with a pong, and returning the reply frame
//! to send, or nothing.
//!
//! `{service}_ws_rpc_client!()` emits everything the shared client seam publishes for every
//! transport — `Transport`, `{Service}Client`, the fault mirror and the envelope readers — plus
//! its own copy of the frame codec, `request_frame`, `notify_frame`, `FrameWriter` and
//! `FrameSession`. Both transports are generated in `core` and `std` alone: `FrameWriter` wraps a
//! boxed send function into a one-way `Transport`, and `FrameSession` adds the request-and-reply
//! half — a correlation map keyed on an id from an atomic counter, settled by `deliver` — so a
//! caller's own adapter shrinks to that one send function plus one call to `deliver` per inbound
//! frame.

use super::Transport;
use super::amqp_rpc::{
    Generated, answer_reader, declares_a_reply, declares_header_in, declares_header_out,
    dispatcher_fns, fault_mirror, fault_mirror_readers, header_decoder, incoming_message,
    incoming_message_accessors, method, placement_doc, reply_trait, transport_trait,
};
use crate::service_schema::parse::ServiceDef;
use crate::service_schema::support::module_ident;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

pub fn emit(service: &ServiceDef, transport: Transport) -> TokenStream {
    let dispatcher = dispatcher_macro(service, transport);
    let client = client_macro(service, transport);
    quote! {
        #dispatcher
        #client
    }
}

/// The dispatcher half: everything that turns one inbound text frame into a call on an
/// implementation and a reply frame to send, held as tokens for whoever answers the service.
///
/// Grouped types, then impls, then functions, whole macro through, the way
/// [`super::amqp_rpc::server_macro`] interleaves its own pieces with the shared dispatcher
/// items rather than running them back to back — `incoming_message`, `reply_trait` and
/// `incoming_message_accessors` are what `dispatcher_items` calls too, so the one `dispatch`
/// this macro and the `amqp_rpc` dispatcher answer through is still the one emitter. Within each
/// impl or trait, members fall in the alphabetical order
/// `clippy::arbitrary_source_item_ordering` asks for — `FrameReply`'s own inherent impl is
/// `into_text`, `new`, `write`; its `Reply` impl is `fault`, `send`.
fn dispatcher_macro(service: &ServiceDef, transport: Transport) -> TokenStream {
    let contract = &service.ident;
    let module = module_ident(service);
    let macro_name = super::dispatcher_macro_ident(service, transport);
    let placement = placement_doc(&macro_name, "ws_transport", "the_contract_crate::");
    let macro_doc = format!(
        "The `{contract}` dispatcher for the `{}` transport, held as tokens rather than compiled \
         here.\n\n\
         It takes no arguments and emits bare items - `IncomingMessage`, `Reply` and `dispatch`, \
         beside the readers an arm needs, plus this transport's own frame codec, `FrameReply` and \
         `answer` - so the caller supplies the module they land in and two transports in one \
         crate cannot collide. The invoking crate names `serde`, `serde_json` and `tracing` in \
         its own manifest, because the items below call them.\n\n\
         {placement}",
        transport.name()
    );
    let claims_headers = declares_header_in(service);
    let service_const = frame_codec_const(&contract.to_string());
    let frame_type = frame_codec_type();
    let reply_struct_type = frame_reply_type();
    let incoming_type = incoming_message(claims_headers);
    let reply_trait_type = reply_trait(contract, &module);
    let frame_impl = frame_codec_impl();
    let reply_struct_impl = frame_reply_impl();
    let reply_trait_impl = frame_reply_trait_impl(&module);
    let incoming_impl = incoming_message_accessors(claims_headers);
    let dispatch_fns = dispatcher_fns(service);
    let codec_fns = frame_codec_fns();
    let answer_doc = format!(
        "Answers one text frame on this `{contract}`'s behalf: a request naming this service is \
         dispatched and its reply frame returned; a notify naming this service is dispatched and \
         nothing returned; a ping is answered with a pong. Anything else - another service's \
         frame, a reply, text this transport does not recognise - answers `None`."
    );
    quote! {
        #[doc = #macro_doc]
        #[macro_export]
        macro_rules! #macro_name {
            () => {
                #service_const

                #frame_type
                #reply_struct_type
                #incoming_type
                #reply_trait_type

                #frame_impl
                #reply_struct_impl
                #reply_trait_impl
                #incoming_impl

                #dispatch_fns
                #codec_fns

                #[doc = #answer_doc]
                pub async fn answer<S, Ctx>(text: &str, svc: &S, ctx: &Ctx) -> Option<String>
                where
                    S: $crate::#contract<Ctx> + Sync,
                    Ctx: Sync,
                {
                    match Frame::decode(text) {
                        Ok(Frame::Request { id, service, message }) if service == SERVICE => {
                            let reply = FrameReply::new(&id);
                            dispatch(svc, ctx, &message, &reply).await;
                            Some(reply.into_text())
                        }
                        Ok(Frame::Notify { service, message }) if service == SERVICE => {
                            dispatch(svc, ctx, &message, &FrameReply::new("")).await;
                            None
                        }
                        Ok(Frame::Ping) => Some(pong_frame()),
                        _ => None,
                    }
                }
            };
        }
    }
}

/// `SERVICE`, the name `Frame::decode`, `FrameReply` and `answer` all match a frame's own
/// `service` field against.
fn frame_codec_const(contract_name: &str) -> TokenStream {
    quote! {
        /// The name every frame for this service carries in its own `service` field.
        pub const SERVICE: &str = #contract_name;
    }
}

/// `Frame`, the one type a frame off the socket reads into.
fn frame_codec_type() -> TokenStream {
    quote! {
        /// One frame as read off the socket: a call this service is asked to make and must
        /// answer, a call it is told about and owes no reply, an answer to a call this side made
        /// over the client macro, or a liveness probe.
        pub enum Frame {
            /// A call this service is asked to make and must answer.
            Request {
                id: String,
                service: String,
                message: IncomingMessage,
            },
            /// A call this service is told about and owes no reply.
            Notify { service: String, message: IncomingMessage },
            /// An answer to a call this side made, read by the client macro.
            Reply {
                id: String,
                service: String,
                envelope: ::serde_json::Map<String, ::serde_json::Value>,
            },
            /// A liveness probe this side answers with a pong.
            Ping,
            /// A liveness answer to a ping this side sent.
            Pong,
        }
    }
}

/// `Frame::decode`, the one reader every frame kind above is read through.
fn frame_codec_impl() -> TokenStream {
    quote! {
        impl Frame {
            /// Reads one frame off the wire. Text that is not JSON, is not a JSON object, or
            /// names no `kind` this transport recognises, is refused, naming why.
            pub fn decode(text: &str) -> Result<Frame, String> {
                let parsed: ::serde_json::Value = ::serde_json::from_str(text)
                    .map_err(|refused| format!("not JSON: {refused}"))?;
                let ::serde_json::Value::Object(frame) = parsed else {
                    return Err("frame is not an object".to_owned());
                };
                let read_message = |on: &::serde_json::Map<String, ::serde_json::Value>| -> Result<IncomingMessage, String> {
                    let payload = on.get("payload").cloned().unwrap_or(::serde_json::Value::Null);
                    Ok(IncomingMessage::new(
                        string_of(on, "operation")?,
                        ::serde_json::to_vec(&payload)
                            .map_err(|unrepresentable| unrepresentable.to_string())?,
                        headers_of(on),
                    ))
                };
                match string_of(&frame, "kind")?.as_str() {
                    "request" => Ok(Frame::Request {
                        id: string_of(&frame, "id")?,
                        service: string_of(&frame, "service")?,
                        message: read_message(&frame)?,
                    }),
                    "notify" => Ok(Frame::Notify {
                        service: string_of(&frame, "service")?,
                        message: read_message(&frame)?,
                    }),
                    "reply" => {
                        let mut envelope = frame.clone();
                        for key in ["kind", "id", "service"] {
                            envelope.remove(key);
                        }
                        Ok(Frame::Reply {
                            id: string_of(&frame, "id")?,
                            service: string_of(&frame, "service")?,
                            envelope,
                        })
                    }
                    "ping" => Ok(Frame::Ping),
                    "pong" => Ok(Frame::Pong),
                    other => Err(format!("unknown frame kind `{other}`")),
                }
            }
        }
    }
}

/// `ping_frame`/`pong_frame`, and the private helpers `Frame::decode` and `FrameReply::write`
/// read a frame's fields and its headers through.
fn frame_codec_fns() -> TokenStream {
    quote! {
        /// The text frame a liveness probe is sent as.
        pub fn ping_frame() -> String {
            ::serde_json::json!({ "kind": "ping" }).to_string()
        }

        /// The text frame a liveness probe is answered with.
        pub fn pong_frame() -> String {
            ::serde_json::json!({ "kind": "pong" }).to_string()
        }

        /// One required string key off a frame, or `Err` naming it.
        fn string_of(
            frame: &::serde_json::Map<String, ::serde_json::Value>,
            key: &str,
        ) -> Result<String, String> {
            match frame.get(key) {
                Some(::serde_json::Value::String(read)) => Ok(read.clone()),
                _ => Err(format!("frame carries no string `{key}`")),
            }
        }

        /// The headers a frame carried beside its payload, read into the `(name, text)` pairs
        /// `IncomingMessage` stores and a `header_in` binding decodes — each value JSON-encoded,
        /// which is what lets it decode into whatever type a binding declared. `headers_table` is
        /// the inverse.
        fn headers_of(
            frame: &::serde_json::Map<String, ::serde_json::Value>,
        ) -> Vec<(String, String)> {
            let Some(::serde_json::Value::Object(headers)) = frame.get("headers") else {
                return Vec::new();
            };
            headers
                .iter()
                .map(|(name, value)| (name.clone(), value.to_string()))
                .collect()
        }

        /// The inverse of `headers_of`: a header list, decoded back off its JSON encoding into one
        /// object a frame carries under `headers` — a reply's own `header_out` values, or a
        /// request's or notify's own `header_in` values. `None` for an empty list, so the key is
        /// left off entirely rather than sent empty.
        fn headers_table(headers: Vec<(String, String)>) -> Option<::serde_json::Value> {
            if headers.is_empty() {
                return None;
            }
            let mut table = ::serde_json::Map::new();
            for (name, encoded) in headers {
                let value = ::serde_json::from_str(&encoded)
                    .unwrap_or_else(|_| ::serde_json::Value::String(encoded));
                table.insert(name, value);
            }
            Some(::serde_json::Value::Object(table))
        }
    }
}

/// `FrameReply`, the one type a request frame is answered through.
fn frame_reply_type() -> TokenStream {
    quote! {
        /// The `Reply` a request frame is answered through: renders the reply frame the adapter
        /// sends, and the empty success a `request` naming a one-way operation is left unwritten.
        pub struct FrameReply {
            id: String,
            written: ::std::sync::Mutex<Option<String>>,
        }
    }
}

/// `FrameReply`'s own inherent methods: `into_text`, `new` and the private `write` both other
/// methods settle through, in that alphabetical order.
fn frame_reply_impl() -> TokenStream {
    quote! {
        impl FrameReply {
            /// The reply frame, or the empty success a `request` naming a one-way operation is
            /// answered with — a requester is never left waiting on a reply that never comes.
            pub fn into_text(self) -> String {
                self.written.into_inner().unwrap().unwrap_or_else(|| {
                    ::serde_json::json!({
                        "kind": "reply",
                        "id": self.id,
                        "service": SERVICE,
                        "ok": true,
                        "value": null,
                    })
                    .to_string()
                })
            }

            pub fn new(id: &str) -> Self {
                Self {
                    id: id.to_owned(),
                    written: ::std::sync::Mutex::new(None),
                }
            }

            /// Merges `kind`, `id`, `service` and, where the arm wrote any, `headers`, into the
            /// answer's own top-level keys, and records the result for `into_text` to return.
            ///
            /// An arm answers only with `Answered` or the fault literal, both objects, so
            /// `answered` is always one; a value that somehow was not still becomes a reply,
            /// carried under `value` rather than merged.
            fn write(&self, answered: ::serde_json::Value, headers: Vec<(String, String)>) {
                let mut envelope = match answered {
                    ::serde_json::Value::Object(carried) => carried,
                    other => {
                        let mut wrapped = ::serde_json::Map::new();
                        wrapped.insert("value".to_owned(), other);
                        wrapped
                    }
                };
                envelope.insert("kind".to_owned(), ::serde_json::Value::String("reply".to_owned()));
                envelope.insert("id".to_owned(), ::serde_json::Value::String(self.id.clone()));
                envelope.insert(
                    "service".to_owned(),
                    ::serde_json::Value::String(SERVICE.to_owned()),
                );
                if let Some(table) = headers_table(headers) {
                    envelope.insert("headers".to_owned(), table);
                }
                *self.written.lock().unwrap() = Some(::serde_json::Value::Object(envelope).to_string());
            }
        }
    }
}

/// `impl Reply for FrameReply`: `fault` and `send`, in that alphabetical order. Both write to
/// `self.written` synchronously and awake nothing — a `Future` this trait needs only for the seam
/// every transport's `Reply` shares, not for any wait `FrameReply` itself has to do.
fn frame_reply_trait_impl(module: &Ident) -> TokenStream {
    quote! {
        impl Reply for FrameReply {
            fn fault(
                &self,
                fault: $crate::#module::ServiceFault,
            ) -> impl ::core::future::Future<Output = ()> + Send {
                self.write(
                    ::serde_json::json!({
                        "ok": false,
                        "error": { "isServiceFault": true, "fault": fault },
                    }),
                    Vec::new(),
                );
                ::core::future::ready(())
            }

            fn send<T>(
                &self,
                value: T,
                headers: Vec<(String, String)>,
            ) -> impl ::core::future::Future<Output = ()> + Send
            where
                T: ::serde::Serialize + Send,
            {
                match ::serde_json::to_value(&value) {
                    Ok(answered) => self.write(answered, headers),
                    Err(unserializable) => ::tracing::error!(
                        error = %unserializable,
                        "an answer would not serialize; the caller is left without a reply",
                    ),
                }
                ::core::future::ready(())
            }
        }
    }
}

/// The macro-level doc and the client type's own doc, split out of [`client_macro`] to keep that
/// function under clippy's line count.
fn client_macro_docs(
    contract: &Ident,
    transport: Transport,
    published: &Ident,
) -> (String, String) {
    let placement = placement_doc(
        published,
        "ws_client",
        "use the_contract_crate::{AvailableBalanceRequest, AvailableBalanceResponse};\n\n\
         the_contract_crate::",
    );
    let macro_doc = format!(
        "The `{contract}` client for the `{}` transport, held as tokens rather than compiled \
         here.\n\n\
         It takes no arguments and emits bare items - the seam, the client type and one method \
         per operation, plus this transport's own `SERVICE`, `Frame`, `request_frame`, \
         `notify_frame`, `FrameWriter` and `FrameSession` - so the caller supplies the module \
         they land in and two transports in one crate cannot collide.\n\n\
         The invoking crate names `serde` and `serde_json` in its own manifest, the expansion \
         calling both. It names no `tracing`: nothing here catches a panic, so nothing here has \
         anything to write down. `FrameSession`'s correlation map names no runtime crate beyond \
         those two either - `core` and `std` are enough for a mutex and a waker.\n\n\
         {placement}\n\n\
         The `use` names the types the author declared one by one - the messages, the successes \
         and the errors a method signature spells, which this expansion writes exactly as they \
         were written there. `use the_contract_crate::*;` would resolve the same names and earn \
         the consumer a `clippy::wildcard_imports` they cannot fix from where they stand.",
        transport.name()
    );
    let client_doc = format!(
        "A `{contract}` caller, over any transport that can send an operation name beside a \
         payload - `FrameWriter` for a one-way push, `FrameSession` for a call that waits on its \
         own reply.\n\n\
         Every operation on the trait has a method here, taking that operation's arguments and \
         nothing else: the context is the implementation's and never reaches a caller. A \
         request-and-reply operation answers `Result<Success, CallError<Error>>`; a one-way \
         operation answers nothing beyond the send, save for the fault it owes when the message it \
         was handed fails its own validation or the transport could not put it out."
    );
    (macro_doc, client_doc)
}

/// The client half: the shared client items every transport composes (the `Transport` seam, the
/// fault mirror, the client type and one `method` per operation) plus this transport's own copy of
/// the frame codec, `request_frame`, `notify_frame`, `FrameWriter` and `FrameSession`, held as
/// tokens for whoever wants to make calls or push events.
///
/// Grouped types, then impls, then functions, whole macro through, the same way
/// [`dispatcher_macro`] interleaves its own pieces with the shared client items rather than
/// running them back to back.
fn client_macro(service: &ServiceDef, transport: Transport) -> TokenStream {
    let contract = &service.ident;
    let client = format_ident!("{contract}Client", span = contract.span());
    let published = super::client_macro_ident(service, transport);
    let generated = Generated::of(module_ident(service));
    let methods = service
        .operations
        .iter()
        .map(|operation| method(operation, &generated));
    let (macro_doc, client_doc) = client_macro_docs(contract, transport, &published);
    let claims_headers = declares_header_in(service);
    let service_const = frame_codec_const(&contract.to_string());
    let seam = transport_trait(contract);
    let incoming_type = incoming_message(claims_headers);
    let incoming_impl = incoming_message_accessors(claims_headers);
    let frame_type = frame_codec_type();
    let frame_impl = frame_codec_impl();
    let send_type = send_frame_type();
    let writer_type = frame_writer_type();
    let writer_impl = frame_writer_impl();
    let writer_transport_impl = frame_writer_transport_impl();
    let session_types = frame_session_types();
    let awaiting_impl = awaiting_future_impl();
    let session_impl = frame_session_impl();
    let session_transport_impl = frame_session_transport_impl();
    // A fault and an answer both arrive in a reply, so a service that declares none has nothing
    // for either to read — mirrors `client_macro` in `amqp_rpc`.
    let (mirror, minting, reader) = if declares_a_reply(service) {
        (
            fault_mirror(),
            fault_mirror_readers(&generated),
            answer_reader(service, &generated),
        )
    } else {
        (TokenStream::new(), TokenStream::new(), TokenStream::new())
    };
    let header_decode = declares_header_out(service)
        .then(header_decoder)
        .unwrap_or_default();
    let codec_fns = frame_codec_fns();
    let encoders = frame_encoders();
    let boxed_send = boxed_send_fn();
    quote! {
        #[doc = #macro_doc]
        #[macro_export]
        macro_rules! #published {
            () => {
                #service_const
                #seam
                #mirror
                #incoming_type
                #frame_type
                #send_type
                #writer_type
                #session_types

                #[doc = #client_doc]
                pub struct #client<T: Transport> {
                    transport: T,
                }

                #minting
                #incoming_impl
                #frame_impl
                #writer_impl
                #writer_transport_impl
                #awaiting_impl
                #session_impl
                #session_transport_impl

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

                // The operations sit apart, under the `Sync` a call's future needs: it borrows
                // the client across an await, and a borrow is only `Send` where what it borrows
                // is `Sync`. Binding a client asks for no such thing.
                impl<T: Transport + Sync> #client<T> {
                    #(#methods)*
                }

                #reader
                #header_decode
                #codec_fns
                #encoders
                #boxed_send
            };
        }
    }
}

/// The boxed send function both `FrameWriter` and `FrameSession` carry: a text frame in, `Ok(())`
/// once it is on the wire or `Err` in words if it never went out. Boxed rather than a type
/// parameter, so `FrameWriter` and `FrameSession` are nameable non-generic types and a struct field
/// can hold one - `LedgerClient<FrameSession>` needs a concrete second argument to name.
fn send_frame_type() -> TokenStream {
    quote! {
        type SendFrame = Box<
            dyn Fn(String) -> ::core::pin::Pin<
                    Box<dyn ::core::future::Future<Output = Result<(), String>> + Send>,
                > + Send
                + Sync,
        >;
    }
}

/// Boxes a send closure into a [`SendFrame`](send_frame_type), pinning the future it returns.
fn boxed_send_fn() -> TokenStream {
    quote! {
        fn boxed_send<F, Fut>(send: F) -> SendFrame
        where
            F: Fn(String) -> Fut + Send + Sync + 'static,
            Fut: ::core::future::Future<Output = Result<(), String>> + Send + 'static,
        {
            Box::new(move |text| {
                let sending: ::core::pin::Pin<Box<dyn ::core::future::Future<Output = _> + Send>> =
                    Box::pin(send(text));
                sending
            })
        }
    }
}

/// `FrameWriter`, the one-way `Transport` a server holds to push to a browser.
fn frame_writer_type() -> TokenStream {
    quote! {
        /// A one-way `Transport` over any function that puts a text frame on the wire. `request`
        /// answers `Err`: a writer keeps no correlation map to resolve one against, so a call that
        /// wants an answer needs a `FrameSession` instead.
        pub struct FrameWriter(SendFrame);
    }
}

/// `FrameWriter::new`, the one constructor a `FrameWriter` is built through.
fn frame_writer_impl() -> TokenStream {
    quote! {
        impl FrameWriter {
            /// Binds a writer to the function that puts a text frame on the wire.
            pub fn new<F, Fut>(send: F) -> Self
            where
                F: Fn(String) -> Fut + Send + Sync + 'static,
                Fut: ::core::future::Future<Output = Result<(), String>> + Send + 'static,
            {
                Self(boxed_send(send))
            }
        }
    }
}

/// `impl Transport for FrameWriter`: `notify` writes a notify frame through the send function;
/// `request` answers `Err` naming the operation, in that alphabetical order - the same order the
/// trait declares them in.
fn frame_writer_transport_impl() -> TokenStream {
    quote! {
        impl Transport for FrameWriter {
            async fn notify<T>(
                &self,
                operation: &str,
                payload: T,
                headers: Vec<(String, String)>,
            ) -> Result<(), String>
            where
                T: ::serde::Serialize + Send,
            {
                (self.0)(notify_frame(operation, &payload, headers)?).await
            }

            fn request<T>(
                &self,
                operation: &str,
                _: T,
                _: Vec<(String, String)>,
            ) -> impl ::core::future::Future<Output = Result<(Vec<u8>, Vec<(String, String)>), String>> + Send
            where
                T: ::serde::Serialize + Send,
            {
                // No `.await` in this body: a writer answers a call for an answer it cannot wait
                // on immediately, rather than as `async fn` sugar over nothing that ever yields.
                ::core::future::ready(Err(format!(
                    "`{operation}` expects an answer; a one-way writer carries no correlation \
                     map, use a session"
                )))
            }
        }
    }
}

/// `Slot`, `Awaiting`, `SessionInner` and `FrameSession` itself: the correlation map a
/// request-and-reply call over one socket is settled through, in `core` and `std` alone.
fn frame_session_types() -> TokenStream {
    quote! {
        /// One outstanding request's own answer: empty until `deliver` or `close` fills it, and
        /// the waker whoever is polling [`Awaiting`] for it parked there meanwhile.
        struct Slot {
            answer: Option<Result<(Vec<u8>, Vec<(String, String)>), String>>,
            waker: Option<::core::task::Waker>,
        }

        /// The `Future` one `request` call awaits: ready once `deliver` or `close` filled its
        /// slot, parking its waker there otherwise.
        struct Awaiting(::std::sync::Arc<::std::sync::Mutex<Slot>>);

        /// What every clone of a [`FrameSession`] shares: the send function, the id counter every
        /// request draws its own id from, and the slot each outstanding request is waiting on,
        /// keyed by the id it was sent under.
        struct SessionInner {
            next: ::std::sync::atomic::AtomicU64,
            pending: ::std::sync::Mutex<::std::collections::HashMap<String, ::std::sync::Arc<::std::sync::Mutex<Slot>>>>,
            send: SendFrame,
        }

        /// A request-and-reply `Transport` over one socket, in `core` and `std` alone. Cloning
        /// shares the session, so one clone drives the client while another feeds `deliver` every
        /// frame the socket reads.
        #[derive(Clone)]
        pub struct FrameSession {
            inner: ::std::sync::Arc<SessionInner>,
        }
    }
}

/// `impl Future for Awaiting`: ready with whatever `deliver` or `close` left in the slot, or
/// pending with this poll's own waker parked there to be woken by whichever reaches it first.
fn awaiting_future_impl() -> TokenStream {
    quote! {
        impl ::core::future::Future for Awaiting {
            type Output = Result<(Vec<u8>, Vec<(String, String)>), String>;

            fn poll(
                self: ::core::pin::Pin<&mut Self>,
                context: &mut ::core::task::Context<'_>,
            ) -> ::core::task::Poll<Self::Output> {
                let mut slot = self.0.lock().unwrap();
                match slot.answer.take() {
                    Some(answer) => ::core::task::Poll::Ready(answer),
                    None => {
                        slot.waker = Some(context.waker().clone());
                        ::core::task::Poll::Pending
                    }
                }
            }
        }
    }
}

/// `FrameSession`'s own inherent methods: `close`, `deliver` and `new`, in that alphabetical
/// order.
fn frame_session_impl() -> TokenStream {
    quote! {
        impl FrameSession {
            /// Fails every request still waiting with `detail`, in words, and wakes each one -
            /// what a closed socket owes every call it will now never answer.
            pub fn close(&self, detail: &str) {
                let waiting = ::core::mem::take(&mut *self.inner.pending.lock().unwrap());
                for slot in waiting.into_values() {
                    let mut slot = slot.lock().unwrap();
                    slot.answer = Some(Err(detail.to_owned()));
                    if let Some(waker) = slot.waker.take() {
                        waker.wake();
                    }
                }
            }

            /// Reads one frame off the socket: a reply naming this service fills the slot its id
            /// is waiting on and wakes it; a ping is answered with a pong; a request, a notify or
            /// a reply for another service is ignored - this side never receives the first two,
            /// and has nothing to do with the third.
            pub async fn deliver(&self, text: &str) {
                match Frame::decode(text) {
                    Ok(Frame::Reply {
                        id,
                        service,
                        envelope,
                    }) if service == SERVICE => {
                        let Some(slot) = self.inner.pending.lock().unwrap().remove(&id) else {
                            return;
                        };
                        let headers = headers_of(&envelope);
                        let encoded = ::serde_json::to_vec(&envelope).unwrap_or_default();
                        let mut slot = slot.lock().unwrap();
                        slot.answer = Some(Ok((encoded, headers)));
                        if let Some(waker) = slot.waker.take() {
                            waker.wake();
                        }
                    }
                    Ok(Frame::Ping) => {
                        let _ = (self.inner.send)(pong_frame()).await;
                    }
                    _ => {}
                }
            }

            /// Binds a session to the function that puts a text frame on the wire.
            pub fn new<F, Fut>(send: F) -> Self
            where
                F: Fn(String) -> Fut + Send + Sync + 'static,
                Fut: ::core::future::Future<Output = Result<(), String>> + Send + 'static,
            {
                Self {
                    inner: ::std::sync::Arc::new(SessionInner {
                        next: ::std::sync::atomic::AtomicU64::new(0),
                        pending: ::std::sync::Mutex::new(::std::collections::HashMap::new()),
                        send: boxed_send(send),
                    }),
                }
            }
        }
    }
}

/// `impl Transport for FrameSession`: `notify` writes a notify frame through the send function;
/// `request` allocates the next id, registers a slot, sends the request frame and awaits it - in
/// that alphabetical order, the same order the trait declares them in.
fn frame_session_transport_impl() -> TokenStream {
    quote! {
        impl Transport for FrameSession {
            async fn notify<T>(
                &self,
                operation: &str,
                payload: T,
                headers: Vec<(String, String)>,
            ) -> Result<(), String>
            where
                T: ::serde::Serialize + Send,
            {
                (self.inner.send)(notify_frame(operation, &payload, headers)?).await
            }

            async fn request<T>(
                &self,
                operation: &str,
                payload: T,
                headers: Vec<(String, String)>,
            ) -> Result<(Vec<u8>, Vec<(String, String)>), String>
            where
                T: ::serde::Serialize + Send,
            {
                let id =
                    (self.inner.next.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed) + 1)
                        .to_string();
                let slot = ::std::sync::Arc::new(::std::sync::Mutex::new(Slot {
                    answer: None,
                    waker: None,
                }));
                self.inner
                    .pending
                    .lock()
                    .unwrap()
                    .insert(id.clone(), ::std::sync::Arc::clone(&slot));
                (self.inner.send)(request_frame(&id, operation, &payload, headers)?).await?;
                Awaiting(slot).await
            }
        }
    }
}

/// `request_frame`, `notify_frame` and the private helper both build a frame through.
fn frame_encoders() -> TokenStream {
    quote! {
        /// The text frame one request-and-reply call sends: this call's own id, the operation and
        /// payload, and, where the operation claimed any, the outgoing `header_in` values.
        pub fn request_frame<T>(
            id: &str,
            operation: &str,
            payload: &T,
            headers: Vec<(String, String)>,
        ) -> Result<String, String>
        where
            T: ::serde::Serialize,
        {
            encoded_frame("request", Some(id), operation, payload, headers)
        }

        /// The text frame one one-way call sends: the operation and payload, and, where the
        /// operation claimed any, the outgoing `header_in` values.
        pub fn notify_frame<T>(
            operation: &str,
            payload: &T,
            headers: Vec<(String, String)>,
        ) -> Result<String, String>
        where
            T: ::serde::Serialize,
        {
            encoded_frame("notify", None, operation, payload, headers)
        }

        /// What `request_frame` and `notify_frame` both build: `kind`, `id` where the call
        /// expects an answer, `service`, `operation`, `payload` and, where the caller wrote any,
        /// `headers`.
        fn encoded_frame<T>(
            kind: &str,
            id: Option<&str>,
            operation: &str,
            payload: &T,
            headers: Vec<(String, String)>,
        ) -> Result<String, String>
        where
            T: ::serde::Serialize,
        {
            let payload = ::serde_json::to_value(payload)
                .map_err(|unrepresentable| unrepresentable.to_string())?;
            let mut frame = ::serde_json::Map::new();
            frame.insert("kind".to_owned(), ::serde_json::Value::String(kind.to_owned()));
            if let Some(id) = id {
                frame.insert("id".to_owned(), ::serde_json::Value::String(id.to_owned()));
            }
            frame.insert(
                "service".to_owned(),
                ::serde_json::Value::String(SERVICE.to_owned()),
            );
            frame.insert(
                "operation".to_owned(),
                ::serde_json::Value::String(operation.to_owned()),
            );
            frame.insert("payload".to_owned(), payload);
            if let Some(table) = headers_table(headers) {
                frame.insert("headers".to_owned(), table);
            }
            Ok(::serde_json::Value::Object(frame).to_string())
        }
    }
}
