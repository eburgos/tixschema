//! How the client and the service name the one message an operation receives.
//!
//! Read by the client and the dispatcher and by nothing else, so it is gated with them: only a
//! build with the Zod surface publishes either, and a name and a schema nobody asks for is dead
//! code.
//!
//! Both sides take the message as a single object — `getBalance(req)` — where Rust unpacks an
//! argument list, because that is what a TypeScript caller of the hand-written client types today.
//! Which type that is, and which schema validates it, is read off the parsed operation rather than
//! decided again on each side, so the two cannot disagree about what crosses.

use crate::field_type::get_field_def;
use crate::service_schema::parse::{OperationDef, OperationInputs};

/// The schema the message validates against: the one `#[model_schema()]` published for it, read
/// through the same field walk every other reference to the type goes through rather than by
/// pasting a suffix onto a name.
pub fn schema(operation: &OperationDef) -> String {
    match &operation.inputs {
        OperationInputs::Named(declared) => get_field_def("req", declared, "").zod_type(),
        OperationInputs::Empty | OperationInputs::Generated(_) => operation
            .generated_message_ident()
            .map_or_else(String::new, |declared| {
                let named: syn::Type = syn::parse_quote! { #declared };
                get_field_def("req", &named, "").zod_type()
            }),
    }
}

/// The message's TypeScript name: the type the operation named, or the one the macro declared for
/// an operation that named none.
pub fn typename(operation: &OperationDef) -> String {
    match &operation.inputs {
        OperationInputs::Named(declared) => {
            get_field_def("req", declared, "").typescript_typename()
        }
        OperationInputs::Empty | OperationInputs::Generated(_) => operation
            .generated_message_ident()
            .map_or_else(|| "unknown".to_owned(), |declared| declared.to_string()),
    }
}
