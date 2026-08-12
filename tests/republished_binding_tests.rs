//! Tests for the annotation an item writes over a binding it republishes rather than builds.
//!
//! Deliberately **not** feature-gated: the brand, alias and one-slot tuple-struct fixtures below
//! must expand in every feature combination of the powerset, since the wire form each one round-
//! trips through serde is the same in all of them. Only the emitted-annotation assertions carry a
//! gate, and each names exactly the surface it reads.

#[cfg(test)]
#[path = "republished_binding_tests/tests.rs"]
mod tests;
