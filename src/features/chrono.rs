//! Chrono date/time type support.
//!
//! This module handles chrono type detection and generates appropriate
//! TypeScript and Zod schema code when the `chrono` feature is enabled.
//!
//! ## Supported Types
//! - `NaiveDate` - Date without timezone (ISO 8601 format: "2025-11-29")
//! - `NaiveTime` - Time without timezone (format: "14:30:00")
//! - `NaiveDateTime` - `DateTime` without timezone (format: "2025-11-29T14:30:00")
//! - `DateTime<Tz>` - `DateTime` with timezone (format: "2025-11-29T14:30:00Z")

/// Generates TypeScript type name for `NaiveDate`.
pub fn get_naive_date_typescript_type() -> String {
    "string".to_owned()
}

/// Generates TypeScript type name for `NaiveTime`.
pub fn get_naive_time_typescript_type() -> String {
    "string".to_owned()
}

/// Generates TypeScript type name for `NaiveDateTime`.
pub fn get_naive_datetime_typescript_type() -> String {
    "string".to_owned()
}

/// Generates TypeScript type name for `DateTime<Tz>`.
pub fn get_datetime_typescript_type() -> String {
    "string".to_owned()
}

/// Generates Zod schema for `NaiveDate` (date validation).
/// Uses Zod v4's `z.iso.date()` syntax (`z.string().date()` is deprecated).
#[cfg(any(test, feature = "zod"))]
pub fn get_naive_date_zod_schema() -> String {
    "z.iso.date()".to_owned()
}

/// Generates Zod schema for `NaiveTime` (time validation).
/// Uses Zod v4's `z.iso.time()` syntax (`z.string().time()` is deprecated).
#[cfg(any(test, feature = "zod"))]
pub fn get_naive_time_zod_schema() -> String {
    "z.iso.time()".to_owned()
}

/// Generates Zod schema for `NaiveDateTime` (local datetime validation).
/// Uses Zod v4's `z.iso.datetime()` syntax (`z.string().datetime()` is deprecated).
#[cfg(any(test, feature = "zod"))]
pub fn get_naive_datetime_zod_schema() -> String {
    "z.iso.datetime({ local: true })".to_owned()
}

/// Generates Zod schema for `DateTime<Tz>` (datetime with timezone validation).
/// Uses Zod v4's `z.iso.datetime({ offset: true })` so both `Z` and numeric
/// offsets (e.g. `-04:00`) are accepted, matching Rust's default serialized
/// form for `DateTime<FixedOffset>` (and still accepting `DateTime<Utc>` /
/// `DateTime<Local>`). `z.string().datetime()` is deprecated.
#[cfg(any(test, feature = "zod"))]
pub fn get_datetime_zod_schema() -> String {
    "z.iso.datetime({ offset: true })".to_owned()
}

#[cfg(test)]
mod tests;
