//! Tests for tuple variant enum support.
//!
//! This module tests the handling of enum tuple variants in TypeScript/Zod/JSON Schema generation:
//! - Single-element tuples: `Variant(T)` -> `{ type: "Variant", value: T }`
//! - Multi-element tuples: `Variant(T1, T2)` -> `{ type: "Variant", value: [T1, T2] }`
//! - Unit variants in mixed enums: `Variant` -> `{ type: "Variant" }`
//! - Plain enums (all unit variants, no tag and no `untagged`): string union `"V1" | "V2" | "V3"`

#[cfg(test)]
#[cfg(feature = "typescript")]
#[cfg(feature = "serde")]
#[path = "tuple_variant_tests/tests.rs"]
mod tests;
