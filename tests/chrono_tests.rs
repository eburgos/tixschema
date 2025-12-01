//! Tests for chrono date/time type support
//!
//! These tests verify that NaiveDate, NaiveTime, NaiveDateTime, and DateTime<Tz>
//! types are properly converted to TypeScript types and Zod schemas.

#[cfg(all(test, feature = "chrono"))]
#[expect(clippy::unwrap_used, reason = "This is a test file")]
mod tests {
    use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, Utc};
    use std::collections::HashMap;

    #[cfg(feature = "serde")]
    use serde::{Deserialize, Serialize};

    use tixschema::model_schema;

    // Test struct with NaiveDate field
    #[model_schema()]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Debug, Clone, PartialEq)]
    struct EventWithDateJson {
        name: String,
        date: NaiveDate,
    }

    // Test struct with NaiveTime field
    #[model_schema()]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Debug, Clone, PartialEq)]
    struct ScheduleJson {
        task: String,
        start_time: NaiveTime,
    }

    // Test struct with NaiveDateTime field
    #[model_schema()]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Debug, Clone, PartialEq)]
    struct LocalEventJson {
        title: String,
        local_datetime: NaiveDateTime,
    }

    // Test struct with DateTime<Utc> field
    #[model_schema()]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Debug, Clone, PartialEq)]
    struct TimestampedRecordJson {
        id: String,
        created_at: DateTime<Utc>,
    }

    // Test struct with DateTime<Local> field
    #[model_schema()]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Debug, Clone, PartialEq)]
    struct LocalTimestampJson {
        id: String,
        local_time: DateTime<Local>,
    }

    // Test struct with optional DateTime
    #[model_schema()]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Debug, Clone, PartialEq)]
    struct OptionalTimestampJson {
        id: String,
        updated_at: Option<DateTime<Utc>>,
    }

    // Test struct with Vec of dates
    #[model_schema()]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Debug, Clone, PartialEq)]
    struct DateListJson {
        name: String,
        dates: Vec<NaiveDate>,
    }

    // Test struct with HashMap of DateTime
    #[model_schema()]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Debug, Clone, PartialEq)]
    struct DateMapJson {
        name: String,
        events: HashMap<String, DateTime<Utc>>,
    }

    // Test struct with all chrono types
    #[model_schema()]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Debug, Clone, PartialEq)]
    struct AllChronoTypesJson {
        date: NaiveDate,
        time: NaiveTime,
        local_datetime: NaiveDateTime,
        utc_datetime: DateTime<Utc>,
        local_timestamp: DateTime<Local>,
    }

    // Test enum with DateTime variant (the original use case!)
    #[model_schema()]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Debug, Clone, PartialEq)]
    #[cfg_attr(feature = "serde", serde(tag = "type"))]
    pub enum FixedValueJson {
        Alphanumeric(String),
        Decimal(f64),
        Integer(i64),
        Boolean(bool),
        Time(NaiveTime),
        Date(NaiveDate),
        DateTime(DateTime<Local>),
    }

    // ========== TypeScript Type Tests ==========

    #[test]
    #[cfg(feature = "typescript")]
    fn test_naive_date_typescript() {
        let ts = EventWithDateJson::ts_definition();
        assert!(
            ts.contains("date: string;"),
            "NaiveDate should map to string. Got: {ts}"
        );
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_naive_time_typescript() {
        let ts = ScheduleJson::ts_definition();
        assert!(
            ts.contains("start_time: string;"),
            "NaiveTime should map to string. Got: {ts}"
        );
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_naive_datetime_typescript() {
        let ts = LocalEventJson::ts_definition();
        assert!(
            ts.contains("local_datetime: string;"),
            "NaiveDateTime should map to string. Got: {ts}"
        );
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_datetime_utc_typescript() {
        let ts = TimestampedRecordJson::ts_definition();
        assert!(
            ts.contains("created_at: string;"),
            "DateTime<Utc> should map to string. Got: {ts}"
        );
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_datetime_local_typescript() {
        let ts = LocalTimestampJson::ts_definition();
        assert!(
            ts.contains("local_time: string;"),
            "DateTime<Local> should map to string. Got: {ts}"
        );
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_optional_datetime_typescript() {
        let ts = OptionalTimestampJson::ts_definition();
        assert!(
            ts.contains("updated_at: string | undefined;"),
            "Option<DateTime<Utc>> should map to string | undefined. Got: {ts}"
        );
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_vec_date_typescript() {
        let ts = DateListJson::ts_definition();
        assert!(
            ts.contains("dates: Array<string>;"),
            "Vec<NaiveDate> should map to Array<string>. Got: {ts}"
        );
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_hashmap_datetime_typescript() {
        let ts = DateMapJson::ts_definition();
        assert!(
            ts.contains("events: Partial<Record<string, string>>;"),
            "HashMap<String, DateTime<Utc>> should map to Partial<Record<string, string>>. Got: {ts}"
        );
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_all_chrono_types_typescript() {
        let ts = AllChronoTypesJson::ts_definition();
        assert!(
            ts.contains("date: string;"),
            "NaiveDate should be string. Got: {ts}"
        );
        assert!(
            ts.contains("time: string;"),
            "NaiveTime should be string. Got: {ts}"
        );
        assert!(
            ts.contains("local_datetime: string;"),
            "NaiveDateTime should be string. Got: {ts}"
        );
        assert!(
            ts.contains("utc_datetime: string;"),
            "DateTime<Utc> should be string. Got: {ts}"
        );
        assert!(
            ts.contains("local_timestamp: string;"),
            "DateTime<Local> should be string. Got: {ts}"
        );
    }

    // ========== Zod Schema Tests ==========

    #[test]
    #[cfg(feature = "zod")]
    fn test_naive_date_zod() {
        let zod = EventWithDateJson::zod_schema();
        assert!(
            zod.contains("date: z.iso.date(),"),
            "NaiveDate should use z.iso.date(). Got: {zod}"
        );
    }

    #[test]
    #[cfg(feature = "zod")]
    fn test_naive_time_zod() {
        let zod = ScheduleJson::zod_schema();
        assert!(
            zod.contains("start_time: z.iso.time(),"),
            "NaiveTime should use z.iso.time(). Got: {zod}"
        );
    }

    #[test]
    #[cfg(feature = "zod")]
    fn test_naive_datetime_zod() {
        let zod = LocalEventJson::zod_schema();
        assert!(
            zod.contains("local_datetime: z.iso.datetime({ local: true }),"),
            "NaiveDateTime should use z.iso.datetime({{ local: true }}). Got: {zod}"
        );
    }

    #[test]
    #[cfg(feature = "zod")]
    fn test_datetime_utc_zod() {
        let zod = TimestampedRecordJson::zod_schema();
        assert!(
            zod.contains("created_at: z.iso.datetime(),"),
            "DateTime<Utc> should use z.iso.datetime(). Got: {zod}"
        );
    }

    #[test]
    #[cfg(feature = "zod")]
    fn test_datetime_local_zod() {
        let zod = LocalTimestampJson::zod_schema();
        assert!(
            zod.contains("local_time: z.iso.datetime(),"),
            "DateTime<Local> should use z.iso.datetime(). Got: {zod}"
        );
    }

    #[test]
    #[cfg(feature = "zod")]
    fn test_optional_datetime_zod() {
        let zod = OptionalTimestampJson::zod_schema();
        assert!(
            zod.contains("updated_at: z.union([z.iso.datetime(), z.undefined()]),"),
            "Option<DateTime<Utc>> should use z.union([...datetime(), z.undefined()]). Got: {zod}"
        );
    }

    #[test]
    #[cfg(feature = "zod")]
    fn test_vec_date_zod() {
        let zod = DateListJson::zod_schema();
        assert!(
            zod.contains("dates: z.array(z.iso.date()),"),
            "Vec<NaiveDate> should use z.array(z.iso.date()). Got: {zod}"
        );
    }

    #[test]
    #[cfg(feature = "zod")]
    fn test_hashmap_datetime_zod() {
        let zod = DateMapJson::zod_schema();
        assert!(
            zod.contains("events: z.record(z.string(), z.iso.datetime()),"),
            "HashMap<String, DateTime<Utc>> should use z.record(z.string(), z.iso.datetime()). Got: {zod}"
        );
    }

    // ========== JSON Schema Tests ==========

    #[test]
    #[cfg(feature = "jsonschema")]
    fn test_naive_date_json_schema() {
        let schema = EventWithDateJson::json_schema();
        let properties = schema["properties"].as_object().unwrap();
        let date_prop = &properties["date"];
        assert_eq!(date_prop["type"], "string");
        assert_eq!(date_prop["format"], "date");
    }

    #[test]
    #[cfg(feature = "jsonschema")]
    fn test_naive_time_json_schema() {
        let schema = ScheduleJson::json_schema();
        let properties = schema["properties"].as_object().unwrap();
        let time_prop = &properties["start_time"];
        assert_eq!(time_prop["type"], "string");
        assert_eq!(time_prop["format"], "time");
    }

    #[test]
    #[cfg(feature = "jsonschema")]
    fn test_naive_datetime_json_schema() {
        let schema = LocalEventJson::json_schema();
        let properties = schema["properties"].as_object().unwrap();
        let datetime_prop = &properties["local_datetime"];
        assert_eq!(datetime_prop["type"], "string");
        assert_eq!(datetime_prop["format"], "date-time");
    }

    #[test]
    #[cfg(feature = "jsonschema")]
    fn test_datetime_utc_json_schema() {
        let schema = TimestampedRecordJson::json_schema();
        let properties = schema["properties"].as_object().unwrap();
        let datetime_prop = &properties["created_at"];
        assert_eq!(datetime_prop["type"], "string");
        assert_eq!(datetime_prop["format"], "date-time");
    }

    #[test]
    #[cfg(feature = "jsonschema")]
    fn test_vec_date_json_schema() {
        let schema = DateListJson::json_schema();
        let properties = schema["properties"].as_object().unwrap();
        let dates_prop = &properties["dates"];
        assert_eq!(dates_prop["type"], "array");
        assert_eq!(dates_prop["items"]["type"], "string");
        assert_eq!(dates_prop["items"]["format"], "date");
    }

    // ========== Enum Tests (Original Use Case) ==========

    #[test]
    #[cfg(feature = "typescript")]
    fn test_enum_with_datetime_typescript() {
        let ts = FixedValueJson::ts_definition();
        // The enum should compile and generate TypeScript
        assert!(
            ts.contains("FixedValue"),
            "Should generate FixedValue type. Got: {ts}"
        );
        // DateTime variant should be present
        assert!(
            ts.contains("DateTime"),
            "Should contain DateTime variant. Got: {ts}"
        );
    }

    #[test]
    #[cfg(feature = "zod")]
    fn test_enum_with_datetime_zod() {
        let zod = FixedValueJson::zod_schema();
        // The enum should compile and generate Zod schema
        assert!(
            zod.contains("FixedValue$Schema"),
            "Should generate FixedValue$Schema. Got: {zod}"
        );
    }

    // ========== Compilation Smoke Test ==========

    #[test]
    fn test_chrono_compilation_smoke_test() {
        // This test ensures all chrono types compile without panics
        let event = EventWithDateJson {
            name: "Test Event".to_string(),
            date: NaiveDate::from_ymd_opt(2025, 11, 29).unwrap(),
        };

        let schedule = ScheduleJson {
            task: "Meeting".to_string(),
            start_time: NaiveTime::from_hms_opt(14, 30, 0).unwrap(),
        };

        let timestamp = TimestampedRecordJson {
            id: "123".to_string(),
            created_at: Utc::now(),
        };

        // If we get here without panics, chrono support is working at compile time
        assert_eq!(event.name, "Test Event");
        assert_eq!(schedule.task, "Meeting");
        assert!(!timestamp.id.is_empty());
    }
}
