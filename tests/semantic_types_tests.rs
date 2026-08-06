//! Tests for type alias support — semantic types (e.g. `DocumentId`, `UserId`) distinct in
//! TypeScript but backed by primitives in Rust, covering basic, optional, generic and nested
//! aliases, alias fields and collections, and module structure across all three surfaces.

#[cfg(test)]
#[path = "semantic_types_tests/tests.rs"]
mod tests;
