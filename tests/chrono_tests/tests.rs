use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use tixschema::model_schema;

// Test struct with all chrono types.
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct AllChronoTypes {
    date: NaiveDate,
    local_datetime: NaiveDateTime,
    local_timestamp: DateTime<Local>,
    time: NaiveTime,
    utc_datetime: DateTime<Utc>,
}

// Test struct with Vec of dates.
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct DateList {
    dates: Vec<NaiveDate>,
    name: String,
}

// Test struct with HashMap of DateTime.
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct DateMap {
    events: HashMap<String, DateTime<Utc>>,
    name: String,
}

// Test struct with NaiveDate field.
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct EventWithDate {
    date: NaiveDate,
    name: String,
}

// Test enum with DateTime variant (the original use case!).
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
pub enum FixedValue {
    Alphanumeric(String),
    Boolean(bool),
    Date(NaiveDate),
    DateTime(DateTime<Local>),
    Decimal(f64),
    Integer(i64),
    Time(NaiveTime),
}

// Test struct with NaiveDateTime field.
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct LocalEvent {
    local_datetime: NaiveDateTime,
    title: String,
}

// Test struct with DateTime<Local> field.
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct LocalTimestamp {
    id: String,
    local_time: DateTime<Local>,
}

// Test struct with optional DateTime.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct OptionalTimestamp {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<DateTime<Utc>>,
}

// Test struct with NaiveTime field.
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct Schedule {
    start_time: NaiveTime,
    task: String,
}

// Test struct with DateTime<Utc> field.
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct TimestampedRecord {
    created_at: DateTime<Utc>,
    id: String,
}

// Test struct mixing the default Date rendering with the `as_number` opt-out.
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct Sample {
    #[model_schema_prop(as_number)]
    created_at: DateTime<Utc>,
    due_at: DateTime<Utc>,
    start_time: NaiveTime,
}

// Test enum with a TupleSingle DateTime variant honoring the as_number opt-out.
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
pub enum DynamicValue {
    Native(DateTime<Utc>),
    Number(#[model_schema_prop(as_number)] DateTime<Utc>),
}

#[test]
fn test_chrono_types_constructible() {
    let date = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
    let time = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
    let naive_dt = date.and_time(time);
    let utc_dt = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
    let local_dt = utc_dt.with_timezone(&Local);

    let all = AllChronoTypes {
        date,
        local_datetime: naive_dt,
        local_timestamp: local_dt,
        time,
        utc_datetime: utc_dt,
    };
    assert_eq!(all.date, date);
    let date_list = DateList {
        dates: Vec::new(),
        name: String::new(),
    };
    assert!(date_list.dates.is_empty());
    let date_map = DateMap {
        events: HashMap::new(),
        name: String::new(),
    };
    assert!(date_map.events.is_empty());
    let values = [
        FixedValue::Alphanumeric(String::new()),
        FixedValue::Boolean(false),
        FixedValue::Date(date),
        FixedValue::DateTime(local_dt),
        FixedValue::Decimal(0.0),
        FixedValue::Integer(0),
        FixedValue::Time(time),
    ];
    assert_eq!(values.len(), 7);
    let local_event = LocalEvent {
        local_datetime: naive_dt,
        title: String::new(),
    };
    assert!(local_event.title.is_empty());
    let local_timestamp = LocalTimestamp {
        id: String::new(),
        local_time: local_dt,
    };
    assert!(local_timestamp.id.is_empty());
    let optional_timestamp = OptionalTimestamp {
        id: String::new(),
        updated_at: None,
    };
    assert!(optional_timestamp.updated_at.is_none());
}

// ========== TypeScript Type Tests ==========

#[test]
#[cfg(feature = "typescript")]
fn test_naive_date_typescript() {
    let ts = EventWithDate::ts_definition();
    assert!(
        ts.contains("date: string;"),
        "NaiveDate should map to string. Got: {ts}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_naive_time_typescript() {
    let ts = Schedule::ts_definition();
    assert!(
        ts.contains("start_time: string;"),
        "NaiveTime should map to string. Got: {ts}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_naive_datetime_typescript() {
    let ts = LocalEvent::ts_definition();
    assert!(
        ts.contains("local_datetime: string;"),
        "NaiveDateTime should map to string. Got: {ts}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_datetime_utc_typescript() {
    let ts = TimestampedRecord::ts_definition();
    assert!(
        ts.contains("created_at: Date;"),
        "DateTime<Utc> should map to Date. Got: {ts}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_datetime_local_typescript() {
    let ts = LocalTimestamp::ts_definition();
    assert!(
        ts.contains("local_time: Date;"),
        "DateTime<Local> should map to Date. Got: {ts}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_optional_datetime_typescript() {
    let ts = OptionalTimestamp::ts_definition();
    assert!(
        ts.contains("updated_at: Date | undefined;"),
        "Option<DateTime<Utc>> should map to Date | undefined. Got: {ts}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_vec_date_typescript() {
    let ts = DateList::ts_definition();
    assert!(
        ts.contains("dates: Array<string>;"),
        "Vec<NaiveDate> should map to Array<string>. Got: {ts}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_hashmap_datetime_typescript() {
    let ts = DateMap::ts_definition();
    assert!(
        ts.contains("events: Partial<Record<string, Date>>;"),
        "HashMap<String, DateTime<Utc>> should map to Partial<Record<string, Date>>. Got: {ts}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_all_chrono_types_typescript() {
    let ts = AllChronoTypes::ts_definition();
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
        ts.contains("utc_datetime: Date;"),
        "DateTime<Utc> should be Date. Got: {ts}"
    );
    assert!(
        ts.contains("local_timestamp: Date;"),
        "DateTime<Local> should be Date. Got: {ts}"
    );
}

// ========== Zod Schema Tests ==========

#[test]
#[cfg(feature = "zod")]
fn test_naive_date_zod() {
    let zod = EventWithDate::zod_schema();
    assert!(
        zod.contains("date: z.iso.date(),"),
        "NaiveDate should use z.iso.date(). Got: {zod}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_naive_time_zod() {
    let zod = Schedule::zod_schema();
    assert!(
        zod.contains("start_time: z.preprocess((arg) =>"),
        "NaiveTime should be wrapped in a millis-accepting preprocessor. Got: {zod}"
    );
    assert!(
        zod.contains("}, z.iso.time()),"),
        "NaiveTime preprocessor should validate with z.iso.time(). Got: {zod}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_naive_datetime_zod() {
    let zod = LocalEvent::zod_schema();
    assert!(
        zod.contains("local_datetime: z.iso.datetime({ local: true }),"),
        "NaiveDateTime should use z.iso.datetime({{ local: true }}). Got: {zod}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_datetime_utc_zod() {
    let zod = TimestampedRecord::zod_schema();
    assert!(
        zod.contains("created_at: z.coerce.date(),"),
        "DateTime<Utc> should use z.coerce.date(). Got: {zod}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_datetime_local_zod() {
    let zod = LocalTimestamp::zod_schema();
    assert!(
        zod.contains("local_time: z.coerce.date(),"),
        "DateTime<Local> should use z.coerce.date(). Got: {zod}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_optional_datetime_zod() {
    let zod = OptionalTimestamp::zod_schema();
    assert!(
        zod.contains("updated_at: z.union([z.coerce.date(), z.undefined()]).prefault(undefined),"),
        "Option<DateTime<Utc>> should use z.union([z.coerce.date(), z.undefined()]). Got: {zod}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_vec_date_zod() {
    let zod = DateList::zod_schema();
    assert!(
        zod.contains("dates: z.array(z.iso.date()),"),
        "Vec<NaiveDate> should use z.array(z.iso.date()). Got: {zod}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_hashmap_datetime_zod() {
    let zod = DateMap::zod_schema();
    assert!(
        zod.contains("events: z.record(z.string(), z.coerce.date()),"),
        "HashMap<String, DateTime<Utc>> should use z.record(z.string(), z.coerce.date()). Got: {zod}"
    );
}

// ========== JSON Schema Tests ==========

#[test]
#[cfg(feature = "jsonschema")]
fn test_naive_date_json_schema() {
    let schema = EventWithDate::json_schema();
    let properties = schema["properties"].as_object().unwrap();
    let date_prop = &properties["date"];
    assert_eq!(date_prop["type"], "string");
    assert_eq!(date_prop["format"], "date");
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_naive_time_json_schema() {
    let schema = Schedule::json_schema();
    let properties = schema["properties"].as_object().unwrap();
    let time_prop = &properties["start_time"];
    assert_eq!(time_prop["type"], "string");
    assert_eq!(time_prop["format"], "time");
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_naive_datetime_json_schema() {
    let schema = LocalEvent::json_schema();
    let properties = schema["properties"].as_object().unwrap();
    let datetime_prop = &properties["local_datetime"];
    assert_eq!(datetime_prop["type"], "string");
    assert_eq!(datetime_prop["format"], "date-time");
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_datetime_utc_json_schema() {
    let schema = TimestampedRecord::json_schema();
    let properties = schema["properties"].as_object().unwrap();
    let datetime_prop = &properties["created_at"];
    assert_eq!(datetime_prop["type"], "string");
    assert_eq!(datetime_prop["format"], "date-time");
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_vec_date_json_schema() {
    let schema = DateList::json_schema();
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
    let ts = FixedValue::ts_definition();
    // The enum should compile and generate TypeScript.
    assert!(
        ts.contains("FixedValue"),
        "Should generate FixedValue type. Got: {ts}"
    );
    // DateTime variant should be present.
    assert!(
        ts.contains("DateTime"),
        "Should contain DateTime variant. Got: {ts}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_enum_with_datetime_zod() {
    let zod = FixedValue::zod_schema();
    // The enum should compile and generate Zod schema.
    assert!(
        zod.contains("FixedValue$Schema"),
        "Should generate FixedValue$Schema. Got: {zod}"
    );
}

// ========== Compilation Smoke Test ==========

#[test]
fn test_chrono_compilation_smoke_test() {
    // This test ensures all chrono types compile without panics.
    let event = EventWithDate {
        name: "Test Event".to_owned(),
        date: NaiveDate::from_ymd_opt(2025, 11, 29).unwrap(),
    };

    let schedule = Schedule {
        task: "Meeting".to_owned(),
        start_time: NaiveTime::from_hms_opt(14, 30, 0).unwrap(),
    };

    let timestamp = TimestampedRecord {
        id: "123".to_owned(),
        created_at: Utc::now(),
    };

    // If we get here without panics, chrono support is working at compile time.
    assert_eq!(event.name, "Test Event");
    assert_eq!(schedule.task, "Meeting");
    assert!(!timestamp.id.is_empty());
}

// ========== as_number opt-out + native Date default ==========

#[test]
#[cfg(feature = "typescript")]
fn test_sample_typescript_mixes_date_and_number() {
    let ts = Sample::ts_definition();
    assert!(
        ts.contains("due_at: Date;"),
        "default DateTime<Utc> should be Date. Got: {ts}"
    );
    assert!(
        ts.contains("created_at: number;"),
        "as_number DateTime<Utc> should be number. Got: {ts}"
    );
    assert!(
        ts.contains("start_time: string;"),
        "NaiveTime should stay string. Got: {ts}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_sample_zod_mixes_coerce_date_and_inline_number() {
    let zod = Sample::zod_schema();
    assert!(
        zod.contains("due_at: z.coerce.date(),"),
        "default DateTime<Utc> should use z.coerce.date(). Got: {zod}"
    );
    assert!(
        zod.contains("created_at: z.preprocess((arg) =>"),
        "as_number DateTime<Utc> should use an inline preprocess arrow. Got: {zod}"
    );
    assert!(
        zod.contains("arg instanceof Date) return arg.getTime();"),
        "as_number coercer should map Date -> getTime(). Got: {zod}"
    );
    assert!(
        zod.contains("}, z.number()),"),
        "as_number coercer should validate with z.number(). Got: {zod}"
    );
    // It must not fall back to the ISO datetime renderer or a named fn.
    assert!(
        !zod.contains("created_at: z.iso.datetime"),
        "as_number field must not use z.iso.datetime. Got: {zod}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_tuple_variant_datetime_honors_as_number() {
    let ts = DynamicValue::ts_definition();
    assert!(
        ts.contains("Date"),
        "Native variant payload should render Date. Got: {ts}"
    );
    assert!(
        ts.contains("number"),
        "Number variant payload should render number via as_number. Got: {ts}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_tuple_variant_datetime_zod_honors_as_number() {
    let zod = DynamicValue::zod_schema();
    assert!(
        zod.contains("z.coerce.date()"),
        "Native variant payload should use z.coerce.date(). Got: {zod}"
    );
    assert!(
        zod.contains("}, z.number())"),
        "Number variant payload should use the inline as_number coercer. Got: {zod}"
    );
}

#[test]
fn test_as_number_types_constructible() {
    let dt = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
    let sample = Sample {
        created_at: dt,
        due_at: dt,
        start_time: NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    };
    assert_eq!(sample.created_at, dt);
    let values = [DynamicValue::Native(dt), DynamicValue::Number(dt)];
    assert_eq!(values.len(), 2);
}
