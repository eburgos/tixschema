//! Tests for a field whose type arrives from a `macro_rules!` `$t:ty` metavariable.
//!
//! A declaration written once and expanded through a metavariable says exactly what the same
//! declaration written out by hand says. rustc hands the substitution to the attribute wrapped in
//! an invisible grouping — its way of keeping the substituted type one unit whatever surrounds it
//! — so the two spellings arrive as different `syn::Type` shapes carrying the same type. Every
//! surface here is asked to describe them identically, because there is nothing about the field
//! that differs.
//!
//! The suite compiles wherever a surface does: the fixtures exist to be described, and where
//! nothing describes them there is nothing to ask.

#[cfg(test)]
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[path = "substituted_field_type_tests/tests.rs"]
mod tests;
