//! Tests that generated enum strings match serde's `rename_all` output exactly.
//!
//! Covers the rename modes the codebase relies on — `lowercase`, `snake_case`, per-variant
//! `rename`, and no rule at all — and pins every generated string against what serde itself
//! writes on the wire, so the two can never drift apart silently.

#[cfg(test)]
#[cfg(feature = "serde")]
#[path = "rename_all_tests/tests.rs"]
mod tests;
