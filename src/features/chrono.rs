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

/// Generates TypeScript type name for `DateTime<Tz>` (native `Date` default).
pub fn get_datetime_typescript_type() -> String {
    "Date".to_owned()
}

/// Generates TypeScript type name for a `DateTime<Tz>` rendered with the
/// `#[model_schema_prop(as_number)]` opt-out (epoch milliseconds).
pub fn get_datetime_number_typescript_type() -> String {
    "number".to_owned()
}

/// Generates Zod schema for `NaiveDate` (date validation).
/// Uses Zod v4's `z.iso.date()` syntax (`z.string().date()` is deprecated).
#[cfg(any(test, feature = "zod"))]
pub fn get_naive_date_zod_schema() -> String {
    "z.iso.date()".to_owned()
}

/// Generates Zod schema for `NaiveTime` (time validation).
/// Stays a TS `string` (`z.iso.time()`) but the inline preprocessor also accepts
/// millis-since-start-of-day, converting it to an `HH:MM:SS` string before validation.
/// `z.string().time()` is deprecated.
#[cfg(any(test, feature = "zod"))]
pub fn get_naive_time_zod_schema() -> String {
    "z.preprocess((arg) => { if (typeof arg === \"number\") { const s = Math.floor(arg / 1000); \
const hh = String(Math.floor(s / 3600)).padStart(2, \"0\"); \
const mm = String(Math.floor((s % 3600) / 60)).padStart(2, \"0\"); \
const ss = String(s % 60).padStart(2, \"0\"); return `${hh}:${mm}:${ss}`; } return arg; }, z.iso.time())"
        .to_owned()
}

/// Generates Zod schema for `NaiveDateTime` (local datetime validation).
/// Uses Zod v4's `z.iso.datetime()` syntax (`z.string().datetime()` is deprecated).
#[cfg(any(test, feature = "zod"))]
pub fn get_naive_datetime_zod_schema() -> String {
    "z.iso.datetime({ local: true })".to_owned()
}

/// Generates Zod schema for `DateTime<Tz>` rendered as a native TS `Date` (default).
/// `z.coerce.date()` accepts `Date` instances, ISO strings, and epoch numbers,
/// matching Rust's serialized forms while producing a BSON `Date` (`MongoDB` TTL).
#[cfg(any(test, feature = "zod"))]
pub fn get_datetime_native_zod_schema() -> String {
    "z.coerce.date()".to_owned()
}

/// Generates Zod schema for a `DateTime<Tz>` rendered with the
/// `#[model_schema_prop(as_number)]` opt-out (epoch milliseconds).
///
/// A self-contained inline coercer: `Date` → `getTime()`, ISO string → `Date.parse`,
/// otherwise passed through to `z.number()`. No imported helper, so generated
/// modules remain self-contained.
#[cfg(any(test, feature = "zod"))]
pub fn get_datetime_number_zod_schema() -> String {
    "z.preprocess((arg) => { if (arg instanceof Date) return arg.getTime(); \
if (typeof arg === \"string\") return Date.parse(arg); return arg; }, z.number())"
        .to_owned()
}

#[cfg(test)]
mod tests;
