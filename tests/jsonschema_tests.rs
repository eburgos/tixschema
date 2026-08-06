//! Tests for JSON Schema generation — the `jsonschema` feature's JSON schema objects from Rust
//! types, covering primitives, optional fields, arrays, maps, nested structs, enums, numeric
//! types, serde rename attributes, and `ObjectId` support.

#[cfg(test)]
#[cfg(feature = "jsonschema")]
#[path = "jsonschema_tests/tests.rs"]
mod tests;
