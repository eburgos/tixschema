//! `answer`, the `ws_rpc` dispatcher's one per-frame call, driven against a real expansion: every
//! frame kind the wire carries, each answered as the reply table specifies.
//!
//! Gated on the `serde` feature, which `#[service_schema]` requires: a build without it is
//! refused at the declaration, so a harness declaring a service would not compile at all.

#![cfg(feature = "serde")]

#[cfg(test)]
#[macro_use]
#[path = "service_schema_ws_rpc_tests/tests.rs"]
mod tests;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_ws_rpc_tests/ws_transport.rs"]
mod ws_transport;

#[cfg(all(test, feature = "serde"))]
use tests::{DocumentSession, document_session_schema};
