//! The messages `#[service_schema]` declares for the operations that named none, read off the
//! three surfaces a client on the far side needs and off the wire itself.
//!
//! Gated on the `serde` feature, which `#[service_schema]` requires: a build without it is
//! refused at the declaration, so a harness declaring a service would not compile at all. The
//! refusal itself is read in the crate's own unit tests, which run in every combination. What the
//! describing surfaces then say is read inside further gated modules, since only those are
//! compiled when the feature that writes them is on.

#[cfg(test)]
#[path = "service_schema_message_tests/tests.rs"]
mod tests;
