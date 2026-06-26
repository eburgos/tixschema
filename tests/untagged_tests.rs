//! Tests for `#[serde(untagged)]` enum support (TypeScript union / Zod `z.union` / JSON `anyOf`).

// `#[serde(untagged)]` parsing requires the `serde` feature, and the branded `DateString`
// newtype is only emitted when a generation feature (typescript/zod/jsonschema) is enabled.
#[cfg(test)]
#[cfg(feature = "serde")]
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[path = "untagged_tests/tests.rs"]
mod tests;
