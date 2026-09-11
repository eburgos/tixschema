//! The `ws_rpc` transport: the `amqp_rpc` request, notify and reply terms over one WebSocket as
//! JSON text frames. The dispatcher and client macros land here in the tasks that follow.

use super::Transport;
use crate::service_schema::parse::ServiceDef;
use proc_macro2::TokenStream;

pub fn emit(_service: &ServiceDef, _transport: Transport) -> TokenStream {
    TokenStream::new()
}
