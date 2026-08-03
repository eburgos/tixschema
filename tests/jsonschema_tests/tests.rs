use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tixschema::model_schema;

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Address {
    city: String,
    street: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct AllNumericTypes {
    f32: f32,
    f64: f64,
    i16: i16,
    i32: i32,
    i64: i64,
    i8: i8,
    isize: isize,
    u16: u16,
    u32: u32,
    u64: u64,
    u8: u8,
    usize: usize,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ArrayFields {
    numbers: Vec<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_array: Option<Vec<String>>,
    tags: Vec<String>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct BasicStruct {
    active: bool,
    age: u32,
    name: String,
    score: f64,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ComplexCollections {
    map_of_arrays: HashMap<String, Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_map: Option<HashMap<String, i32>>,
}

#[cfg(feature = "serde")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct CustomFieldRename {
    #[serde(rename = "customName")]
    field_name: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct EmptyStruct;

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct MapFields {
    counts: HashMap<String, i32>,
    metadata: HashMap<String, String>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct OpaqueFields {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_payload: Option<serde_json::Value>,
    payload: serde_json::Value,
    payloads: Vec<serde_json::Value>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct OpaqueValueMaps {
    payload_lists: HashMap<String, Vec<serde_json::Value>>,
    payloads: HashMap<String, serde_json::Value>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct OptionalFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_bool: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_number: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_string: Option<String>,
    required: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Person {
    address: Address,
    name: String,
    previous_addresses: Vec<Address>,
}

#[cfg(feature = "serde")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct RenamedFields {
    user_email: String,
    user_name: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
enum Status {
    Active,
    Inactive,
    Pending,
}

#[test]
fn test_basic_struct_json_schema() {
    let schema = BasicStruct::json_schema();

    // Check top-level schema properties
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["additionalProperties"], false);

    // Check properties exist
    let properties = schema["properties"].as_object().unwrap();
    assert!(properties.contains_key("name"));
    assert!(properties.contains_key("age"));
    assert!(properties.contains_key("score"));
    assert!(properties.contains_key("active"));

    // Check property types
    assert_eq!(properties["name"]["type"], "string");
    assert_eq!(properties["age"]["type"], "integer");
    assert_eq!(properties["score"]["type"], "number");
    assert_eq!(properties["active"]["type"], "boolean");

    // Check required fields
    let required = schema["required"].as_array().unwrap();
    assert_eq!(required.len(), 4);
    assert!(required.contains(&Value::String("name".to_owned())));
    assert!(required.contains(&Value::String("age".to_owned())));
    assert!(required.contains(&Value::String("score".to_owned())));
    assert!(required.contains(&Value::String("active".to_owned())));
}

#[test]
fn test_optional_fields_not_in_required() {
    let schema = OptionalFields::json_schema();

    let properties = schema["properties"].as_object().unwrap();
    assert!(properties.contains_key("required"));
    assert!(properties.contains_key("optional_string"));
    assert!(properties.contains_key("optional_number"));
    assert!(properties.contains_key("optional_bool"));

    // Check that optional fields have correct types
    assert_eq!(properties["optional_string"]["type"], "string");
    assert_eq!(properties["optional_number"]["type"], "integer");
    assert_eq!(properties["optional_bool"]["type"], "boolean");

    // Check required array only contains required field
    let required = schema["required"].as_array().unwrap();
    assert_eq!(required.len(), 1);
    assert!(required.contains(&Value::String("required".to_owned())));
    assert!(!required.contains(&Value::String("optional_string".to_owned())));
    assert!(!required.contains(&Value::String("optional_number".to_owned())));
    assert!(!required.contains(&Value::String("optional_bool".to_owned())));
}

#[test]
fn test_vec_fields_generate_array_schemas() {
    let schema = ArrayFields::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    // Check tags array
    assert_eq!(properties["tags"]["type"], "array");
    assert_eq!(properties["tags"]["items"]["type"], "string");

    // Check numbers array
    assert_eq!(properties["numbers"]["type"], "array");
    assert_eq!(properties["numbers"]["items"]["type"], "integer");

    // Check optional array
    assert_eq!(properties["optional_array"]["type"], "array");
    assert_eq!(properties["optional_array"]["items"]["type"], "string");

    // Check required fields
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&Value::String("tags".to_owned())));
    assert!(required.contains(&Value::String("numbers".to_owned())));
    assert!(!required.contains(&Value::String("optional_array".to_owned())));
}

#[test]
fn test_hashmap_generates_object_with_additional_properties() {
    let schema = MapFields::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    // Check metadata map
    assert_eq!(properties["metadata"]["type"], "object");
    assert_eq!(
        properties["metadata"]["additionalProperties"]["type"],
        "string"
    );

    // Check counts map
    assert_eq!(properties["counts"]["type"], "object");
    assert_eq!(
        properties["counts"]["additionalProperties"]["type"],
        "integer"
    );
}

#[test]
fn test_opaque_value_fields_generate_permissive_schemas() {
    let schema = OpaqueFields::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    // An opaque field carries no type name, so it admits any value.
    assert_eq!(properties["payload"], serde_json::json!({}));
    assert_eq!(properties["optional_payload"], serde_json::json!({}));

    assert_eq!(properties["payloads"]["type"], "array");
    assert_eq!(properties["payloads"]["items"], serde_json::json!({}));

    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&Value::String("payload".to_owned())));
    assert!(required.contains(&Value::String("payloads".to_owned())));
    assert!(!required.contains(&Value::String("optional_payload".to_owned())));
}

#[test]
fn test_opaque_map_values_generate_permissive_schemas() {
    let schema = OpaqueValueMaps::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    // An opaque value carries no type name here either, so every member admits any value.
    assert_eq!(properties["payloads"]["type"], "object");
    assert_eq!(
        properties["payloads"]["additionalProperties"],
        serde_json::json!({})
    );

    assert_eq!(properties["payload_lists"]["type"], "object");
    assert_eq!(
        properties["payload_lists"]["additionalProperties"],
        serde_json::json!({ "type": "array", "items": {} })
    );
}

#[test]
fn test_nested_structs_present_in_schema() {
    let schema = Person::json_schema();
    let address_schema = Address::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    // Single nested object should have the address property
    assert!(properties.contains_key("address"));

    // Array of nested objects should be an array
    assert!(properties.contains_key("previous_addresses"));
    assert_eq!(properties["previous_addresses"]["type"], "array");

    // Verify Address schema exists and is correct
    assert_eq!(address_schema["type"], "object");
    let address_properties = address_schema["properties"].as_object().unwrap();
    assert!(address_properties.contains_key("street"));
    assert!(address_properties.contains_key("city"));
}

#[test]
fn test_plain_enum_generates_string_enum() {
    let schema = Status::json_schema();

    assert_eq!(schema["type"], "string");

    let enum_values = schema["enum"].as_array().unwrap();
    assert_eq!(enum_values.len(), 3);
    assert!(enum_values.contains(&Value::String("Active".to_owned())));
    assert!(enum_values.contains(&Value::String("Inactive".to_owned())));
    assert!(enum_values.contains(&Value::String("Pending".to_owned())));
}

#[test]
fn test_integer_types_use_integer_schema() {
    let schema = AllNumericTypes::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    // All integer types should have type "integer"
    assert_eq!(properties["u8"]["type"], "integer");
    assert_eq!(properties["u16"]["type"], "integer");
    assert_eq!(properties["u32"]["type"], "integer");
    assert_eq!(properties["u64"]["type"], "integer");
    assert_eq!(properties["i8"]["type"], "integer");
    assert_eq!(properties["i16"]["type"], "integer");
    assert_eq!(properties["i32"]["type"], "integer");
    assert_eq!(properties["i64"]["type"], "integer");
    assert_eq!(properties["usize"]["type"], "integer");
    assert_eq!(properties["isize"]["type"], "integer");
}

#[test]
fn test_float_types_use_number_schema() {
    let schema = AllNumericTypes::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    // Float types should have type "number"
    assert_eq!(properties["f32"]["type"], "number");
    assert_eq!(properties["f64"]["type"], "number");
}

#[test]
#[cfg(feature = "serde")]
fn test_serde_rename_all_affects_property_names() {
    let schema = RenamedFields::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    // Should use camelCase names
    assert!(properties.contains_key("userName"));
    assert!(properties.contains_key("userEmail"));

    // Should not contain original snake_case names
    assert!(!properties.contains_key("user_name"));
    assert!(!properties.contains_key("user_email"));

    // Check required array uses renamed fields
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&Value::String("userName".to_owned())));
    assert!(required.contains(&Value::String("userEmail".to_owned())));
}

#[test]
#[cfg(feature = "serde")]
fn test_serde_field_rename() {
    let schema = CustomFieldRename::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    assert!(properties.contains_key("customName"));
    assert!(!properties.contains_key("field_name"));

    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&Value::String("customName".to_owned())));
}

#[test]
fn test_complex_nested_collections() {
    let schema = ComplexCollections::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    // HashMap<String, Vec<String>>
    assert_eq!(properties["map_of_arrays"]["type"], "object");
    let map_of_arrays_additional = properties["map_of_arrays"]["additionalProperties"]
        .as_object()
        .unwrap();
    assert_eq!(map_of_arrays_additional["type"], "array");
    assert_eq!(map_of_arrays_additional["items"]["type"], "string");

    // Option<HashMap<String, i32>>
    assert_eq!(properties["optional_map"]["type"], "object");
    assert_eq!(
        properties["optional_map"]["additionalProperties"]["type"],
        "integer"
    );

    // Check required fields
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&Value::String("map_of_arrays".to_owned())));
    assert!(!required.contains(&Value::String("optional_map".to_owned())));
}

#[test]
#[cfg(feature = "object_id")]
fn test_objectid_generates_proper_schema() {
    use mongodb::bson::oid::ObjectId;

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct Document {
        #[serde(skip_serializing_if = "Option::is_none")]
        author_id: Option<ObjectId>,
        id: ObjectId,
        tag_ids: Vec<ObjectId>,
    }

    let schema = Document::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    // ObjectId should be object with $oid field
    let id_prop = &properties["id"];
    assert_eq!(id_prop["type"], "object");
    assert_eq!(id_prop["properties"]["$oid"]["type"], "string");
    assert_eq!(id_prop["required"][0], "$oid");
    assert_eq!(id_prop["additionalProperties"], false);

    // Optional ObjectId
    let author_prop = &properties["author_id"];
    assert_eq!(author_prop["type"], "object");
    assert_eq!(author_prop["properties"]["$oid"]["type"], "string");

    // Vec<ObjectId>
    let tags_prop = &properties["tag_ids"];
    assert_eq!(tags_prop["type"], "array");
    let items = &tags_prop["items"];
    assert_eq!(items["type"], "object");
    assert_eq!(items["properties"]["$oid"]["type"], "string");
    assert_eq!(items["required"][0], "$oid");
    assert_eq!(items["additionalProperties"], false);
}

#[test]
fn test_empty_struct_generates_valid_schema() {
    let schema = EmptyStruct::json_schema();

    assert_eq!(schema["type"], "object");
    assert_eq!(schema["additionalProperties"], false);

    let properties = schema["properties"].as_object().unwrap();
    assert_eq!(properties.len(), 0);

    let required = schema["required"].as_array().unwrap();
    assert_eq!(required.len(), 0);
}

#[test]
fn test_schema_has_required_structure() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct Test {
        field: String,
    }

    let schema = Test::json_schema();

    // Should be a JSON object
    assert!(schema.is_object());

    // Should have required keys
    assert!(schema.get("type").is_some());
    assert!(schema.get("properties").is_some());
    assert!(schema.get("required").is_some());
    assert!(schema.get("additionalProperties").is_some());
}
