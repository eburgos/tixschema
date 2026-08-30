//! The Rust client: one method per operation, returning the operation's success type or a call
//! error that is either the operation's own error or a fault the remote produced.
//!
//! Not written yet. It lands in svcschema-06.

use super::parse::ServiceDef;
use proc_macro2::TokenStream;

pub fn emit(_service: &ServiceDef) -> TokenStream {
    TokenStream::new()
}
