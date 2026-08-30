//! The messages an operation did not name: `<Operation>Request` for the argument-list and
//! zero-argument shapes, emitted like any other type with its TypeScript, Zod and JSON Schema.
//!
//! Not written yet. It lands in svcschema-03, which reads
//! [`OperationInputs`](super::parse::OperationInputs) and never re-reads the trait.

use super::parse::ServiceDef;
use proc_macro2::TokenStream;

pub fn emit(_service: &ServiceDef) -> TokenStream {
    TokenStream::new()
}
