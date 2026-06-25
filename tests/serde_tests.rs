//! Comprehensive tests for Serde attribute handling.
//!
//! This module tests how tixschema handles Serde attributes when the `serde` feature is enabled.
//!
//! ## What is tested:
//! - `rename_all` with different case conventions (camelCase, `snake_case`, `PascalCase`, etc.).
//! - Field-level rename attribute.
//! - Combination of `rename_all` and field rename.
//! - Serde attributes with optional fields.
//! - Serde attributes in TypeScript, Zod, and JSON schema generation.
//! - Enum `rename_all`.

#[cfg(test)]
#[cfg(feature = "serde")]
#[path = "serde_tests/tests.rs"]
mod tests;
