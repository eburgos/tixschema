//! Tests for chrono date/time type support.
//!
//! These tests verify that `NaiveDate`, `NaiveTime`, `NaiveDateTime`, and `DateTime`<Tz>
//! types are properly converted to TypeScript types and Zod schemas.

#[cfg(all(test, feature = "chrono"))]
#[path = "chrono_tests/tests.rs"]
mod tests;
