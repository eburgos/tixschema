//! Comprehensive tests for JSON Schema generation
//!
//! This module tests the JSON schema generation feature of tixschema.
//! The `jsonschema` feature generates JSON schema objects from Rust types.
//!
//! ## What is tested:
//! - Basic primitive types (string, number, integer, boolean)
//! - Optional fields (not in required array)
//! - Arrays
//! - `HashMaps` (as objects with additionalProperties)
//! - Nested structs
//! - Plain enums (as string enums)
//! - All numeric types (integer vs number)
//! - Serde rename attributes
//! - `ObjectId` support (when `object_id` feature enabled)

#[cfg(feature = "jsonschema")]
#[expect(clippy::unwrap_used, reason = "This is a test file")]
#[expect(clippy::struct_field_names, reason = "This is a test file")]
mod tests {
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use tixschema::model_schema;

    // ========================================================================
    // Basic Struct Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct BasicStruct {
        name: String,
        age: u32,
        score: f64,
        active: bool,
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
        assert!(required.contains(&Value::String("name".to_string())));
        assert!(required.contains(&Value::String("age".to_string())));
        assert!(required.contains(&Value::String("score".to_string())));
        assert!(required.contains(&Value::String("active".to_string())));
    }

    // ========================================================================
    // Optional Fields Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct OptionalFields {
        required: String,
        optional_string: Option<String>,
        optional_number: Option<i32>,
        optional_bool: Option<bool>,
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
        assert!(required.contains(&Value::String("required".to_string())));
        assert!(!required.contains(&Value::String("optional_string".to_string())));
        assert!(!required.contains(&Value::String("optional_number".to_string())));
        assert!(!required.contains(&Value::String("optional_bool".to_string())));
    }

    // ========================================================================
    // Array/Vec Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct ArrayFields {
        tags: Vec<String>,
        numbers: Vec<i32>,
        optional_array: Option<Vec<String>>,
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
        assert!(required.contains(&Value::String("tags".to_string())));
        assert!(required.contains(&Value::String("numbers".to_string())));
        assert!(!required.contains(&Value::String("optional_array".to_string())));
    }

    // ========================================================================
    // HashMap/Map Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct MapFields {
        metadata: std::collections::HashMap<String, String>,
        counts: std::collections::HashMap<String, i32>,
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

    // ========================================================================
    // Nested Structs Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct Address {
        street: String,
        city: String,
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct Person {
        name: String,
        address: Address,
        previous_addresses: Vec<Address>,
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

    // ========================================================================
    // Plain Enum Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    enum Status {
        Active,
        Inactive,
        Pending,
    }

    #[test]
    fn test_plain_enum_generates_string_enum() {
        let schema = Status::json_schema();

        assert_eq!(schema["type"], "string");

        let enum_values = schema["enum"].as_array().unwrap();
        assert_eq!(enum_values.len(), 3);
        assert!(enum_values.contains(&Value::String("Active".to_string())));
        assert!(enum_values.contains(&Value::String("Inactive".to_string())));
        assert!(enum_values.contains(&Value::String("Pending".to_string())));
    }

    // ========================================================================
    // All Numeric Types Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct AllNumericTypes {
        u8_val: u8,
        u16_val: u16,
        u32_val: u32,
        u64_val: u64,
        i8_val: i8,
        i16_val: i16,
        i32_val: i32,
        i64_val: i64,
        f32_val: f32,
        f64_val: f64,
        usize_val: usize,
        isize_val: isize,
    }

    #[test]
    fn test_integer_types_use_integer_schema() {
        let schema = AllNumericTypes::json_schema();

        let properties = schema["properties"].as_object().unwrap();

        // All integer types should have type "integer"
        assert_eq!(properties["u8_val"]["type"], "integer");
        assert_eq!(properties["u16_val"]["type"], "integer");
        assert_eq!(properties["u32_val"]["type"], "integer");
        assert_eq!(properties["u64_val"]["type"], "integer");
        assert_eq!(properties["i8_val"]["type"], "integer");
        assert_eq!(properties["i16_val"]["type"], "integer");
        assert_eq!(properties["i32_val"]["type"], "integer");
        assert_eq!(properties["i64_val"]["type"], "integer");
        assert_eq!(properties["usize_val"]["type"], "integer");
        assert_eq!(properties["isize_val"]["type"], "integer");
    }

    #[test]
    fn test_float_types_use_number_schema() {
        let schema = AllNumericTypes::json_schema();

        let properties = schema["properties"].as_object().unwrap();

        // Float types should have type "number"
        assert_eq!(properties["f32_val"]["type"], "number");
        assert_eq!(properties["f64_val"]["type"], "number");
    }

    // ========================================================================
    // Serde Rename Tests
    // ========================================================================

    #[cfg(feature = "serde")]
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(rename_all = "camelCase")]
    struct RenamedFields {
        user_name: String,
        user_email: String,
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
        assert!(required.contains(&Value::String("userName".to_string())));
        assert!(required.contains(&Value::String("userEmail".to_string())));
    }

    #[cfg(feature = "serde")]
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct CustomFieldRename {
        #[serde(rename = "customName")]
        field_name: String,
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde_field_rename() {
        let schema = CustomFieldRename::json_schema();

        let properties = schema["properties"].as_object().unwrap();

        assert!(properties.contains_key("customName"));
        assert!(!properties.contains_key("field_name"));

        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&Value::String("customName".to_string())));
    }

    // ========================================================================
    // Complex Collections Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct ComplexCollections {
        map_of_arrays: std::collections::HashMap<String, Vec<String>>,
        optional_map: Option<std::collections::HashMap<String, i32>>,
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
        assert!(required.contains(&Value::String("map_of_arrays".to_string())));
        assert!(!required.contains(&Value::String("optional_map".to_string())));
    }

    // ========================================================================
    // ObjectId Tests (when feature enabled)
    // ========================================================================

    #[test]
    #[cfg(feature = "object_id")]
    fn test_objectid_generates_proper_schema() {
        use mongodb::bson::oid::ObjectId;

        #[model_schema()]
        #[derive(Serialize, Deserialize, Debug, Clone)]
        struct Document {
            id: ObjectId,
            author_id: Option<ObjectId>,
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

    // ========================================================================
    // Empty Struct Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct EmptyStruct {}

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

    // ========================================================================
    // Schema Structure Tests
    // ========================================================================

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
}
