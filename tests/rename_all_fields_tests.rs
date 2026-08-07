//! Tests that an enum's `#[serde(rename_all_fields = "...")]` cases the members of every struct
//! variant on all three surfaces, exactly as serde writes them.
//!
//! serde keeps this rule apart from `rename_all`: the latter renames variant names only. Both are
//! pinned here, under each of the four taggings, against what serde itself puts on the wire.

#[cfg(test)]
#[cfg(feature = "serde")]
#[path = "rename_all_fields_tests/tests.rs"]
mod tests;
