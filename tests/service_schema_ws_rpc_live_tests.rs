//! The axum server loop and the tokio-tungstenite client loop a developer copies around the
//! generated `ws_rpc` pieces, run for real: a real socket bound to an ephemeral port, one process
//! playing both ends. Every other `ws_rpc` proof drives `answer`, `FrameWriter` and `FrameSession`
//! by hand, with no socket in the middle; this is the one that puts both adapter loops through a
//! real accept, a real upgrade and real frames on the wire.
//!
//! Gated on the `serde` feature, which `#[service_schema]` requires: a build without it is
//! refused at the declaration, so a harness declaring a service would not compile at all.

#![cfg(feature = "serde")]

extern crate alloc;

#[cfg(test)]
#[macro_use]
#[path = "service_schema_ws_rpc_live_tests/tests.rs"]
mod tests;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_ws_rpc_live_tests/ledger_ws.rs"]
mod ledger_ws;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_ws_rpc_live_tests/ledger_events_ws.rs"]
mod ledger_events_ws;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_ws_rpc_live_tests/ledger_client_ws.rs"]
mod ledger_client_ws;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_ws_rpc_live_tests/screen_ws.rs"]
mod screen_ws;

// The four macros above reach the trait and the schema module through `$crate`, which is this
// binary's root: both are declared inside `tests`, so a private `use` here is what makes
// `crate::Ledger`, `crate::LedgerEvents`, `crate::ledger_schema` and `crate::ledger_events_schema`
// resolve from every sibling module — privacy in Rust reaches a defining module's descendants, and
// every module above is one, this file being the crate root of this test binary.
#[cfg(all(test, feature = "serde"))]
use tests::{Ledger, LedgerEvents, ledger_events_schema, ledger_schema};
