//! A field serde neither writes nor reads is not part of the wire, and no surface describes it.
//!
//! There is nothing to describe without a generation feature, so the module is gated on those. The
//! attributes deciding it sit on the item whether or not the `serde` feature is on, so nothing here
//! is gated on that one: one declaration describes one wire under every toggle.

#[cfg(test)]
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[path = "skipped_field_tests/tests.rs"]
mod tests;
