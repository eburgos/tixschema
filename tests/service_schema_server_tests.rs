//! The server macro `#[service_schema]` generates for the `amqp_rpc` transport, compiled for real:
//! a service, its own `serve_until` named at a concrete type, and the wire framing a reply is built
//! through.
//!
//! Gated on the `serde` feature, which `#[service_schema]` requires: a build without it is refused
//! at the declaration, so a harness declaring a service would not compile at all.
//!
//! # Why the placements below are shaped the way they are
//!
//! A `macro_rules!` body is linted under the levels of the crate that *invokes* it, so this file is
//! a consumer of the server macro as much as a test of it. The same four properties that keep the
//! dispatcher harness clean apply here:
//!
//! 1. every module is `#[path]`-attributed and lives in a file of its own, never `mod x { … }`,
//!    which is what `clippy::inline_modules` refuses;
//! 2. the `mod` declarations sit above the `use` items, which is the grouping
//!    `clippy::arbitrary_source_item_ordering` asks for;
//! 3. everything is private, so no generated item is *exported* — a `pub` module here would
//!    publish the proc macro's own message and support types along with it;
//! 4. the tests dispatch a message through the emitted `dispatch` and name `serve_until` at a
//!    concrete type, so nothing the macro emits is dead. A real `lapin::Channel` cannot be built
//!    without a connection, so `serve_until` itself is named rather than run.

#[cfg(test)]
#[macro_use]
#[path = "service_schema_server_tests/tests.rs"]
mod tests;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_server_tests/amqp_server.rs"]
mod amqp_server;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_server_tests/wire.rs"]
mod wire;

// The server macro reaches what the service declared through `$crate`, which is this binary's
// root: the service's own module, which every message it answers is built through.
#[cfg(all(test, feature = "serde"))]
use tests::{PingService, ping_service_schema};
