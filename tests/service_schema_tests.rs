//! `#[service_schema]` end to end: the trait the macro emits, implemented and called.
//!
//! Deliberately **not** feature-gated at the harness level. The emitted trait carries no surface
//! a feature writes, so it has to compile and run in every feature combination.

#[cfg(test)]
#[path = "service_schema_tests/tests.rs"]
mod tests;
