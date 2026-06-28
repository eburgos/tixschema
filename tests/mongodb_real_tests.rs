//! Real `MongoDB` `ObjectId` compatibility tests.
//!
//! These tests use the actual mongodb library to ensure our macro works
//! correctly with real `MongoDB` `ObjectId`s.

#[cfg(test)]
#[cfg(feature = "object_id")]
#[path = "mongodb_real_tests/tests.rs"]
mod tests;
