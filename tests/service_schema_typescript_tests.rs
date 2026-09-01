//! The TypeScript a `#[service_schema]` service publishes, and the bundle one registration line
//! per artifact produces.
//!
//! Gated on the `serde` feature, which `#[service_schema]` requires: a build without it is
//! refused at the declaration, so a harness declaring a service would not compile at all. The
//! refusal itself is read in the crate's own unit tests, which run in every combination. What the
//! published TypeScript then says is read inside a gated module, since only a build that writes
//! TypeScript writes any.

#[cfg(test)]
#[path = "service_schema_typescript_tests/tests.rs"]
mod tests;

// What `$crate` reaches. A client's macro body names everything the declaration generated from the
// declaring crate's *root*, and the declaration sits in the module above; importing it here puts
// those names where an expansion looks for them. Private: nothing outside this binary reads them.
#[cfg(test)]
#[cfg(all(feature = "serde", feature = "typescript", feature = "zod"))]
use tests::*;
