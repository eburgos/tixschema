//! Tests for fixed-size arrays.
//!
//! serde writes a `[T; N]` as a JSON array of exactly `N` items and reads one back only at that
//! length, so the count is part of what the field describes and not a detail of the Rust spelling.
//! These tests read every surface against that wire: the two validating surfaces carry the bound,
//! and TypeScript keeps `Array<T>`.

#[cfg(test)]
#[path = "fixed_array_tests/tests.rs"]
mod tests;
