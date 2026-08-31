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
#[path = "service_schema_tests/tests.rs"]
mod tests;
