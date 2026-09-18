//! The emitted clients put through a real runtime, with a real message object.
//!
//! Every other assertion about an emitted client here reads strings: which expression was written,
//! never what it computes. `String(sending)` is well-formed TypeScript that renders every object
//! as the constant `[object Object]`, so the only witness is a runtime.
//!
//! **No toolchain is required.** A runtime is looked up on `PATH`, or at whatever the group's own
//! variable names, and a build that finds none stands down rather than failing. `just
//! test-emitted` is the entry point that refuses to stand down.
//!
//! Gated on `serde` (which `#[service_schema]` requires), and on `zod` and `typescript`, since
//! `ts_http_client` is published only where the Zod surface a client validates against is.

#![cfg(all(feature = "serde", feature = "zod", feature = "typescript"))]

#[cfg(test)]
#[macro_use]
#[path = "service_schema_emitted_client_tests/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "service_schema_emitted_client_tests/run_node.rs"]
mod run_node;

#[cfg(test)]
#[path = "service_schema_emitted_client_tests/run_dart.rs"]
mod run_dart;

#[cfg(test)]
#[path = "service_schema_emitted_client_tests/runtime.rs"]
mod runtime;

// The transport's expansion reaches the service through `$crate`, which is this binary's root:
// the trait and its schema module are named here for it to resolve.
#[cfg(test)]
use tests::{ConversationClientService, conversation_client_service_schema};
