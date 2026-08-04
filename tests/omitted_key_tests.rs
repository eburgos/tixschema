//! A field whose key serde leaves out of the output has an optional *key*, and every surface has
//! to say so.
//!
//! There is nothing to describe without a generation feature, so the module is gated on those. The
//! omission is read off attribute text that sits on the item whether or not the `serde` feature is
//! on, so nothing here is gated on that one: one declaration describes one wire under every toggle,
//! and these tests are where that is held.

#[cfg(test)]
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[path = "omitted_key_tests/tests.rs"]
mod tests;
