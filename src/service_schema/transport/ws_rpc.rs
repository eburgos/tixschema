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
//! The client macro lands beside it later, the way `amqp_rpc`'s does.

use super::Transport;
use super::amqp_rpc::{
    declares_header_in, dispatcher_fns, incoming_message, incoming_message_accessors,
    placement_doc, reply_trait,
};
use crate::service_schema::parse::ServiceDef;
use crate::service_schema::support::module_ident;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

pub fn emit(service: &ServiceDef, transport: Transport) -> TokenStream {
    dispatcher_macro(service, transport)
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

        /// The inverse of `headers_of`: every `header_out` value an arm wrote, decoded back off
        /// its JSON encoding into one object a reply frame carries under `headers` — `None` for a
        /// reply that wrote none, so the key is left off entirely rather than sent empty.
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
