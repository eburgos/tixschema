//! Tests for recursive type support in Zod schema generation.
//!
//! These tests verify that recursive types (types that reference themselves)
//! generate correct Zod schemas using JavaScript getter syntax to defer
//! the reference and avoid "use before declaration" errors.

#[cfg(test)]
#[cfg(all(feature = "typescript", feature = "zod", feature = "serde"))]
#[path = "recursive_type_tests/tests.rs"]
mod tests;
