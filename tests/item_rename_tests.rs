//! Tests for `#[model_schema(name = "…")]` written on a declared item — a struct, a tuple struct,
//! an enum, a branded newtype — and on the types that reference one.
//!
//! Deliberately **not** feature-gated at the fixtures: the override renames the Rust module the
//! item's schema is published from, so a build that emits a reference to that module without
//! emitting the module under the same name is exactly what these must catch.

#[cfg(test)]
#[path = "item_rename_tests/tests.rs"]
mod tests;
