//! Joins the generated `ws_rpc` client and dispatcher for `DocumentSession`, and the generated
//! `ws_rpc` client and dispatcher for `SessionEvents`, through one in-memory `Wire` — no socket —
//! proving the two halves answer each other in both directions: a client's call answered by a
//! dispatcher, a header round trip, and a server's own client pushing an event a dispatcher on the
//! other side delivers.
//!
//! Gated on the `serde` feature, which `#[service_schema]` requires: a build without it is
//! refused at the declaration, so a harness declaring a service would not compile at all.
//!
//! The `use` at the foot is what `$crate` reaches inside every macro this crate expands: each
//! service's own module, and the trait its dispatcher and client bind.

#![cfg(feature = "serde")]

extern crate alloc;

#[cfg(test)]
#[macro_use]
#[path = "service_schema_ws_rpc_proof_tests/tests.rs"]
mod tests;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_ws_rpc_proof_tests/ws_transport.rs"]
mod ws_transport;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_ws_rpc_proof_tests/ws_client.rs"]
mod ws_client;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_ws_rpc_proof_tests/events_client.rs"]
mod events_client;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_ws_rpc_proof_tests/events_transport.rs"]
mod events_transport;

#[cfg(all(test, feature = "serde"))]
use tests::{DocumentSession, SessionEvents, document_session_schema, session_events_schema};
