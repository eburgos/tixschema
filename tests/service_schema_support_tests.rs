//! The supporting types `#[service_schema]` generates, driven at runtime: a transport implementing
//! two services' reply handles, and a call error carrying the operation's own error.
//!
//! Deliberately **not** feature-gated at the harness level. Nothing here is written by a feature,
//! so it has to compile and run in every feature combination.

#[cfg(test)]
#[path = "service_schema_support_tests/tests.rs"]
mod tests;
