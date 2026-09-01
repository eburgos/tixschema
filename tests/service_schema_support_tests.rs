//! The supporting types `#[service_schema]` generates, driven at runtime: a transport implementing
//! two services' reply handles, and a call error carrying the operation's own error.
//!
//! Gated on the `serde` feature, which `#[service_schema]` requires: a build without it is
//! refused at the declaration, so a harness declaring a service would not compile at all. The
//! refusal itself is read in the crate's own unit tests, which run in every combination. Nothing
//! here is gated any further — what the construct emits carries no other surface a feature writes.

#[cfg(test)]
#[macro_use]
#[path = "service_schema_support_tests/tests.rs"]
mod tests;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_support_tests/amqp_transport.rs"]
mod amqp_transport;

// A transport's dispatcher reaches what the service declared through `$crate`, which is this
// binary's root: a service written in a submodule is named here for the expansion to resolve.
#[cfg(all(test, feature = "serde"))]
use tests::{SweepService, sweep_service_schema};
