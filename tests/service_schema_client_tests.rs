//! The client `#[service_schema]` generates, driven over transports written by hand: one that
//! hands out prepared answers and records what it was asked to send, and one that loops straight
//! back into the generated dispatcher so both halves of the seam are read against each other.
//!
//! Deliberately **not** feature-gated at the harness level. The client carries no surface a
//! feature writes, so it has to compile and run in every feature combination.

#[cfg(test)]
#[path = "service_schema_client_tests/tests.rs"]
mod tests;
