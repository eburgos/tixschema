//! Chrono date/time type support
//!
//! This module handles chrono type detection and generates appropriate
//! TypeScript and Zod schema code when the `chrono` feature is enabled.
//!
//! ## Supported Types
//! - `NaiveDate` - Date without timezone (ISO 8601 format: "2025-11-29")
//! - `NaiveTime` - Time without timezone (format: "14:30:00")
//! - `NaiveDateTime` - DateTime without timezone (format: "2025-11-29T14:30:00")
//! - `DateTime<Tz>` - DateTime with timezone (format: "2025-11-29T14:30:00Z")

// Allow unused functions - they are used in tests and may be useful for future extensions
#![allow(dead_code)]

/// Detects if a type name represents a chrono `NaiveDate`
pub fn is_naive_date_type(type_name: &str) -> bool {
    type_name == "NaiveDate"
}

/// Detects if a type name represents a chrono `NaiveTime`
pub fn is_naive_time_type(type_name: &str) -> bool {
    type_name == "NaiveTime"
}

/// Detects if a type name represents a chrono `NaiveDateTime`
pub fn is_naive_datetime_type(type_name: &str) -> bool {
    type_name == "NaiveDateTime"
}

/// Detects if a type name represents a chrono `DateTime<Tz>`
pub fn is_datetime_type(type_name: &str) -> bool {
    type_name == "DateTime"
}

/// Generates TypeScript type name for `NaiveDate`
pub fn get_naive_date_typescript_type() -> String {
    "string".to_string()
}

/// Generates TypeScript type name for `NaiveTime`
pub fn get_naive_time_typescript_type() -> String {
    "string".to_string()
}

/// Generates TypeScript type name for `NaiveDateTime`
pub fn get_naive_datetime_typescript_type() -> String {
    "string".to_string()
}

/// Generates TypeScript type name for `DateTime<Tz>`
pub fn get_datetime_typescript_type() -> String {
    "string".to_string()
}

/// Generates Zod schema for `NaiveDate` (date validation)
/// Uses Zod v4's `z.iso.date()` syntax (z.string().date() is deprecated)
#[cfg(any(test, feature = "zod"))]
pub fn get_naive_date_zod_schema() -> String {
    "z.iso.date()".to_string()
}

/// Generates Zod schema for `NaiveTime` (time validation)
/// Uses Zod v4's `z.iso.time()` syntax (z.string().time() is deprecated)
#[cfg(any(test, feature = "zod"))]
pub fn get_naive_time_zod_schema() -> String {
    "z.iso.time()".to_string()
}

/// Generates Zod schema for `NaiveDateTime` (local datetime validation)
/// Uses Zod v4's `z.iso.datetime()` syntax (z.string().datetime() is deprecated)
#[cfg(any(test, feature = "zod"))]
pub fn get_naive_datetime_zod_schema() -> String {
    "z.iso.datetime({ local: true })".to_string()
}

/// Generates Zod schema for `DateTime<Tz>` (datetime with timezone validation)
/// Uses Zod v4's `z.iso.datetime()` syntax (z.string().datetime() is deprecated)
#[cfg(any(test, feature = "zod"))]
pub fn get_datetime_zod_schema() -> String {
    "z.iso.datetime()".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_naive_date_detection() {
        assert!(is_naive_date_type("NaiveDate"));
        assert!(!is_naive_date_type("String"));
        assert!(!is_naive_date_type("DateTime"));
    }

    #[test]
    fn test_naive_time_detection() {
        assert!(is_naive_time_type("NaiveTime"));
        assert!(!is_naive_time_type("String"));
        assert!(!is_naive_time_type("NaiveDate"));
    }

    #[test]
    fn test_naive_datetime_detection() {
        assert!(is_naive_datetime_type("NaiveDateTime"));
        assert!(!is_naive_datetime_type("DateTime"));
        assert!(!is_naive_datetime_type("NaiveDate"));
    }

    #[test]
    fn test_datetime_detection() {
        assert!(is_datetime_type("DateTime"));
        assert!(!is_datetime_type("NaiveDateTime"));
        assert!(!is_datetime_type("String"));
    }

    #[test]
    fn test_typescript_types() {
        assert_eq!(get_naive_date_typescript_type(), "string");
        assert_eq!(get_naive_time_typescript_type(), "string");
        assert_eq!(get_naive_datetime_typescript_type(), "string");
        assert_eq!(get_datetime_typescript_type(), "string");
    }

    #[test]
    fn test_zod_schemas() {
        assert_eq!(get_naive_date_zod_schema(), "z.iso.date()");
        assert_eq!(get_naive_time_zod_schema(), "z.iso.time()");
        assert_eq!(
            get_naive_datetime_zod_schema(),
            "z.iso.datetime({ local: true })"
        );
        assert_eq!(get_datetime_zod_schema(), "z.iso.datetime()");
    }
}
