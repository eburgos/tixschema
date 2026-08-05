use super::*;

#[test]
fn test_typescript_types() {
    assert_eq!(get_naive_date_typescript_type(), "string");
    assert_eq!(get_naive_time_typescript_type(), "string");
    assert_eq!(get_naive_datetime_typescript_type(), "string");
    assert_eq!(get_datetime_typescript_type(), "Date");
    assert_eq!(get_datetime_number_typescript_type(), "number");
}

#[test]
fn test_zod_schemas() {
    assert_eq!(get_naive_date_zod_schema(), "z.iso.date()");
    assert_eq!(
        get_naive_datetime_zod_schema(),
        "z.iso.datetime({ local: true })"
    );
    assert_eq!(get_datetime_native_zod_schema(), "z.coerce.date()");
}

#[test]
fn test_datetime_number_zod_schema_is_self_contained_arrow() {
    let schema = get_datetime_number_zod_schema();
    assert!(schema.starts_with("z.preprocess((arg) =>"));
    assert!(schema.contains("arg instanceof Date) return arg.getTime();"));
    assert!(schema.contains("typeof arg === \"string\") return Date.parse(arg);"));
    assert!(schema.ends_with("z.number())"));
    assert!(!schema.contains("z.iso.datetime"));
}

#[test]
fn test_naive_time_zod_schema_accepts_millis() {
    let schema = get_naive_time_zod_schema();
    assert!(schema.starts_with("z.preprocess((arg) =>"));
    assert!(schema.contains("typeof arg === \"number\""));
    assert!(schema.contains("Math.floor(arg / 1000)"));
    assert!(schema.contains("padStart(2, \"0\")"));
    assert!(schema.ends_with("z.iso.time())"));
}
