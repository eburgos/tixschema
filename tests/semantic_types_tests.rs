//! Comprehensive tests for type alias support.
//!
//! Type aliases allow developers to create semantic types (e.g., `DocumentId`, `UserId`)
//! that are distinct in TypeScript but use primitive types in Rust.
//!
//! This module tests:
//! - Basic type aliases (String, numeric types).
//! - Optional type aliases (Option<T>).
//! - Generic type aliases (Pair<T, U>).
//! - Nested type aliases (alias referencing another alias).
//! - Type aliases used as struct fields.
//! - Type aliases in collections (Vec, `HashMap`).
//! - TypeScript, Zod, and JSON schema generation.
//! - Module structure and naming.

#[cfg(test)]
#[path = "semantic_types_tests/tests.rs"]
mod tests;
