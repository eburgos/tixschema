//! An `Option` field whose `None` serde leaves out of the output has an optional *key*, and every
//! surface has to say so.
//!
//! Reading the omission takes the `serde` feature, and there is nothing to describe without a
//! generation feature, so the module is gated on both.

#[cfg(test)]
#[cfg(feature = "serde")]
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[path = "omitted_key_tests/tests.rs"]
mod tests;
