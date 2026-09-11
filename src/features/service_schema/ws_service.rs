//! The TypeScript `ws_rpc` dispatcher attachment: reads every `notify` and `request` frame naming
//! a service off a socket, drives the generated dispatcher, and answers a request with a reply
//! frame.
//!
//! # A required `onFault`
//!
//! A `notify` frame that fails inside the dispatcher — an unknown operation, a payload that will
//! not become the operation's message — answers a framed fault with nobody waiting on a reply to
//! carry it. `onFault` is where that fault goes: passed once, at the attachment, rather than
//! reached for at every place a frame could be pushed.
//!
//! # A one-way operation reached as a request is still answered
//!
//! A caller that sent a `request` is left waiting until something replies, so a request naming a
//! one-way operation still gets one: `{ ok: true, value: null }` once the dispatcher's own promise
//! settles with `undefined`.
//!
//! # Gated with the dispatcher it drives
//!
//! `create{Service}Dispatcher` — and the `{Service}Impl` and `{Service}Fault` types this reaches
//! for — exist only where [`super::service`] and [`super::fault`] are, so this module is emitted
//! only there too. The socket type it attaches to is [`super::ws_client`]'s own, so a bundle names
//! `ts_ws_client()` before `ts_ws_service()`.

use crate::service_schema::parse::ServiceDef;

pub fn emit(service: &ServiceDef) -> Vec<String> {
    vec![attach_dispatcher_fn(service)]
}

/// Reads every `notify` and `request` frame naming this service off `socket`, drives `impl`
/// through the generated dispatcher, and answers a `request` with a `reply` frame — `{ ok: true,
/// value: null }` where the dispatcher answered nothing. A refused `notify` has nobody to reply to
/// and reaches `onFault` instead. A frame naming another service, and text that is not JSON, are
/// both dropped before either is asked. Returns the function that detaches the listener.
fn attach_dispatcher_fn(service: &ServiceDef) -> String {
    let named = service.ident.to_string();
    format!(
        "/**\n \
         * Reads every `notify` and `request` frame naming `{named}` off `socket`, drives `impl` \
         through\n \
         * the generated dispatcher, and answers a `request` with a `reply` frame — `{{ ok: true, \
         value:\n \
         * null }}` where the dispatcher answered nothing. A refused `notify` has nobody to reply \
         to and\n \
         * reaches `onFault` instead. Returns the function that detaches the listener.\n \
         */\n\
         export function attach{named}WsDispatcher<Ctx>(\n  \
         socket: {named}WsSocket,\n  \
         ctx: Ctx,\n  \
         impl: {named}Impl<Ctx>,\n  \
         onFault: (fault: {named}Fault) => void,\n\
         ): () => void {{\n  \
         const service = \"{named}\";\n  \
         const dispatch = create{named}Dispatcher(impl);\n  \
         const onMessage = (event: {{ data: unknown }}) => {{\n    \
         let frame: unknown;\n    \
         try {{ frame = JSON.parse(String(event.data)); }} catch {{ return; }}\n    \
         if (typeof frame !== \"object\" || frame === null) return;\n    \
         const {{ kind, id, service: named, operation, payload }} = frame as Record<string, \
         unknown>;\n    \
         if (named !== service || typeof operation !== \"string\") return;\n    \
         if (kind === \"notify\") {{\n      \
         void dispatch(ctx, operation, payload).then((answered) => {{\n        \
         if (answered === undefined) return;\n        \
         const {{ error }} = answered as {{ error: {{ isServiceFault: true; fault: {named}Fault \
         }} }};\n        \
         onFault(error.fault);\n      \
         }});\n      \
         return;\n    \
         }}\n    \
         if (kind !== \"request\" || typeof id !== \"string\") return;\n    \
         void dispatch(ctx, operation, payload).then((answered) => {{\n      \
         const envelope = answered === undefined ? {{ ok: true, value: null }} : (answered as \
         object);\n      \
         socket.send(JSON.stringify({{ kind: \"reply\", id, service, ...envelope }}));\n    \
         }});\n  \
         }};\n  \
         socket.addEventListener(\"message\", onMessage);\n  \
         return () => socket.removeEventListener(\"message\", onMessage);\n\
         }}"
    )
}
