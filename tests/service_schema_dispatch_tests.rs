//! The dispatcher `#[service_schema]` generates, driven end to end: a probe service, a probe reply
//! handle that writes down how each message was settled, and a payload for every path through an
//! arm.
//!
//! Deliberately **not** feature-gated at the harness level. The dispatcher carries no surface a
//! feature writes, so it has to compile and run in every feature combination. The one thing that
//! does depend on a feature is whether a message publishes a `validate()` at all, and the module
//! that reads validation says so itself.

#[cfg(test)]
#[path = "service_schema_dispatch_tests/tests.rs"]
mod tests;
