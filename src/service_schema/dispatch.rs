//! The Rust dispatcher: given the incoming message and a way to answer, it settles the message
//! itself. Generic over the implementing type, because a trait with `async fn` is not dyn
//! compatible.
//!
//! Not written yet. It lands in svcschema-05.

use super::parse::ServiceDef;
use proc_macro2::TokenStream;

pub fn emit(_service: &ServiceDef) -> TokenStream {
    TokenStream::new()
}
