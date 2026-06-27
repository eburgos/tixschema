//! Tests for tuple struct-field support.
//!
//! A Rust tuple struct field `(A, B, ...)` serializes (via serde) as a
//! fixed-length JSON array, so it is rendered as:
//! - TypeScript: `[A, B, ...]`
//! - Zod: `z.tuple([A, B, ...])`
//! - JSON Schema: `{ type: "array", prefixItems: [...], items: false,
//!   minItems: N, maxItems: N }`
//!
//! This mirrors the already-correct tuple **variant** path
//! (see `tests/tuple_variant_tests`).

#[cfg(test)]
#[cfg(feature = "typescript")]
#[cfg(feature = "serde")]
#[path = "tuple_field_tests/tests.rs"]
mod tests;
