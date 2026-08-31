//! The dispatcher `#[service_schema]` generates, driven end to end: a probe service, a probe reply
//! handle that writes down how each message was settled, and a payload for every path through an
//! arm.
//!
//! Gated on the `serde` feature, which `#[service_schema]` requires: a build without it is
//! refused at the declaration, so a harness declaring a service would not compile at all. The
//! refusal itself is read in the crate's own unit tests, which run in every combination. Nothing
//! here is gated any further — what the construct emits carries no other surface a feature writes.
//!
//! The one thing that does depend on a further feature is whether a message publishes a
//! `validate()` at all, and the module that reads validation says so itself.

#[cfg(test)]
#[path = "service_schema_dispatch_tests/tests.rs"]
mod tests;
