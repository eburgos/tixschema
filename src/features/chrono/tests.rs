use super::*;

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
    assert_eq!(
        get_datetime_zod_schema(),
        "z.iso.datetime({ offset: true })"
    );
}
