//! Tests for how tixschema handles Serde attributes under the `serde` feature — `rename_all`
//! case conventions, field-level rename, combinations of the two, optional fields, and enum
//! `rename_all`, across TypeScript, Zod, and JSON schema generation.

#[cfg(test)]
#[cfg(feature = "serde")]
#[path = "serde_tests/tests.rs"]
mod tests;
