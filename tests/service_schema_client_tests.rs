//! The client `#[service_schema]` generates, driven over transports written by hand: one that
//! hands out prepared answers and records what it was asked to send, and one that loops straight
//! back into the generated dispatcher so both halves of the seam are read against each other.
//!
//! Gated on the `serde` feature, which `#[service_schema]` requires: a build without it is
//! refused at the declaration, so a harness declaring a service would not compile at all. The
//! refusal itself is read in the crate's own unit tests, which run in every combination. Nothing
//! here is gated any further — what the construct emits carries no other surface a feature writes.
//!
//! The `use` below is what `$crate` reaches. A client's macro body names everything the
//! declaration generated from the declaring crate's *root* — the messages beside the trait, and
//! the fault and the call error inside the service's own module — and the declarations sit in the
//! module beside this file. Importing them here puts those names where an expansion looks for
//! them. Private, because nothing outside this test binary reads them.

#![cfg(feature = "serde")]

#[cfg(test)]
#[path = "service_schema_client_tests/tests.rs"]
mod tests;

#[cfg(test)]
use tests::*;
