//! `#[service_schema]` end to end: one declaration, implemented and then reached three ways — by
//! its Rust name through the emitted trait, by its wire name through the dispatcher, and by its
//! TypeScript name in what the service publishes.
//!
//! Gated on the `serde` feature, which `#[service_schema]` requires: a build without it is
//! refused at the declaration, so a harness declaring a service would not compile at all. The
//! refusal itself is read in the crate's own unit tests, which run in every combination. What the
//! published TypeScript then says is read inside a gated module, since only a build that writes
//! TypeScript writes any.

#[cfg(test)]
#[macro_use]
#[path = "service_schema_tests/tests.rs"]
mod tests;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_tests/amqp_transport.rs"]
mod amqp_transport;

// A transport's dispatcher reaches what the service declared through `$crate`, which is this
// binary's root: a service written in a submodule is named here for the expansion to resolve.
#[cfg(all(test, feature = "serde"))]
use tests::{ProbeService, probe_service_schema};
