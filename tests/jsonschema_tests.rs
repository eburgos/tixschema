//! Comprehensive tests for JSON Schema generation.
//!
//! This module tests the JSON schema generation feature of tixschema.
//! The `jsonschema` feature generates JSON schema objects from Rust types.
//!
//! ## What is tested:
//! - Basic primitive types (string, number, integer, boolean).
//! - Optional fields (not in required array).
//! - Arrays.
//! - `HashMaps` (as objects with additionalProperties).
//! - Nested structs.
//! - Plain enums (as string enums).
//! - All numeric types (integer vs number).
//! - Serde rename attributes.
//! - `ObjectId` support (when `object_id` feature enabled).

#[cfg(test)]
#[cfg(feature = "jsonschema")]
#[path = "jsonschema_tests/tests.rs"]
mod tests;
