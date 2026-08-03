//! Tests for tuple structs that are not branded newtypes.
//!
//! serde writes a one-slot tuple struct as its inner value alone and a wider one as a fixed-arity
//! array, so neither is the object the named-field emitters describe. These tests read every
//! surface against the wire value serde actually writes.

#[cfg(test)]
#[path = "tuple_struct_tests/tests.rs"]
mod tests;
