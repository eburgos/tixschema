use super::*;

#[test]
fn test_object_id_detection() {
    assert!(is_object_id_type("ObjectId"));
    assert!(!is_object_id_type("String"));
    assert!(!is_object_id_type("UserId"));
}

#[test]
fn test_object_id_typescript_type() {
    assert_eq!(get_object_id_typescript_type(), "ObjectId");
}

#[cfg(feature = "object_id")]
#[test]
fn test_object_id_zod_schema() {
    let schema = get_object_id_zod_schema();
    assert!(schema.contains("$oid"));
    assert!(schema.contains("regex"));
    assert!(schema.contains("24"));
}
