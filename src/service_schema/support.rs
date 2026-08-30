//! The types an operation's result is carried in: the service fault a caller can receive but no
//! implementation can construct, the reply handle a transport implements, and the client's
//! call-error enum.
//!
//! Not written yet. It lands in svcschema-04.

use super::parse::ServiceDef;
use proc_macro2::TokenStream;

pub fn emit(_service: &ServiceDef) -> TokenStream {
    TokenStream::new()
}
