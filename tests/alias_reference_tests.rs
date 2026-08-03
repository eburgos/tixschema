//! Tests for structs whose fields reference a `#[model_schema()]` type alias.
//!
//! Deliberately **not** feature-gated: the alias fixtures below must be expanded in every
//! feature combination of the powerset. Every other alias fixture in this suite sits behind
//! `feature = "typescript"`, which is why a jsonschema-without-typescript build referencing a
//! missing alias schema module went undetected.

#[cfg(test)]
#[path = "alias_reference_tests/tests.rs"]
mod tests;
