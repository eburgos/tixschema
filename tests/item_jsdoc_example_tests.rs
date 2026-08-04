//! Tests for what an item's ` ```rust example ` block reaches in the emitted `TypeScript`.
//!
//! The example is Rust source. It does not compile, render, or mean anything inside a `TypeScript`
//! `JSDoc` comment, so no item shape carries it there — the fence and its body are dropped from
//! every `JSDoc` body, whatever the item is declared as.

#[cfg(feature = "typescript")]
#[cfg(test)]
#[path = "item_jsdoc_example_tests/tests.rs"]
mod tests;
