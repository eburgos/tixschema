//! Tests for `#[model_schema()]` on items that declare type parameters.
//!
//! Deliberately **not** feature-gated at the harness level: the fixtures below must expand in
//! every feature combination of the powerset, because the emitted impl block is what carries the
//! item's generics and nothing about it is feature-dependent.

extern crate alloc;

#[cfg(test)]
#[path = "generic_types_tests/tests.rs"]
mod tests;
