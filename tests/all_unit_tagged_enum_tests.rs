//! Tests for an enum whose variants are *all* unit variants under a serde tagging attribute — the
//! canonical shape of a service's error code. Serde keeps the tag key for such an enum exactly as
//! it does when a variant carries a field, so every describing surface has to keep it too rather
//! than falling back to the bare string union an untagged all-unit enum publishes.
//!
//! Every expectation here is held against bytes serde actually writes, read in the same test, so a
//! published type and the wire it describes cannot drift apart.

#[cfg(test)]
#[cfg(feature = "serde")]
#[path = "all_unit_tagged_enum_tests/tests.rs"]
mod tests;
