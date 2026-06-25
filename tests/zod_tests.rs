//! Comprehensive tests for Zod v4 schema generation.
//!
//! This module tests the Zod schema generation feature of tixschema.
//! The `zod` feature generates Zod v4 validation schemas from Rust types.
//!
//! ## What is tested:
//! - Basic primitive types (string, number, boolean).
//! - Optional fields using z.union([type, `z.undefined()`]).
//! - Arrays using `z.array()`.
//! - `HashMaps` using `z.record()`.
//! - Nested structs.
//! - Plain enums using `z.enum()`.
//! - Discriminated union enums using `z.discriminatedUnion()`.
//! - Integration with TypeScript feature (type annotations).
//! - Serde rename attributes.
//! - `ObjectId` support (when `object_id` feature enabled).
//! - All numeric types (integers vs floats).

#[cfg(test)]
#[cfg(feature = "zod")]
#[path = "zod_tests/tests.rs"]
mod tests;
