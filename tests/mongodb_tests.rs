use core_model_macros::model_schema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::{self, Visitor, MapAccess};
use std::collections::HashMap;

#[cfg(all(test, feature = "object_id"))]
mod tests {
    use super::*;

    // Mock ObjectId type for testing - compatible with mongodb::bson::oid::ObjectId
    // The real MongoDB ObjectId serializes to { "$oid": "hex_string" } in JSON
    // and to a plain string in other contexts
    #[derive(Debug, Clone, PartialEq)]
    pub struct ObjectId(String);

    impl Serialize for ObjectId {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            // MongoDB ObjectId serializes as { "$oid": "hex_string" } in JSON
            use serde::ser::SerializeStruct;
            let mut state = serializer.serialize_struct("ObjectId", 1)?;
            state.serialize_field("$oid", &self.0)?;
            state.end()
        }
    }

    impl<'de> Deserialize<'de> for ObjectId {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct ObjectIdVisitor;

            impl<'de> Visitor<'de> for ObjectIdVisitor {
                type Value = ObjectId;

                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("an ObjectId with $oid field")
                }

                fn visit_map<V>(self, mut map: V) -> Result<ObjectId, V::Error>
                where
                    V: MapAccess<'de>,
                {
                    let mut oid = None;
                    while let Some(key) = map.next_key()? {
                        match key {
                            "$oid" => {
                                if oid.is_some() {
                                    return Err(de::Error::duplicate_field("$oid"));
                                }
                                oid = Some(map.next_value()?);
                            }
                            _ => {
                                let _: de::IgnoredAny = map.next_value()?;
                            }
                        }
                    }
                    let oid = oid.ok_or_else(|| de::Error::missing_field("$oid"))?;
                    Ok(ObjectId(oid))
                }
            }

            deserializer.deserialize_struct("ObjectId", &["$oid"], ObjectIdVisitor)
        }
    }

    // Test struct with ObjectId field
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct UserJson {
        id: ObjectId,
        name: String,
        email: Option<String>,
    }

    // Test struct with optional ObjectId field  
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct UserWithOptionalIdJson {
        id: Option<ObjectId>,
        name: String,
        email: String,
    }

    // Test struct with ObjectId array
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct UserWithObjectIdArrayJson {
        id: ObjectId,
        name: String,
        friend_ids: Vec<ObjectId>,
    }

    // Test struct with ObjectId in HashMap
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct UserWithObjectIdMapJson {
        id: ObjectId,
        name: String,
        relationships: HashMap<String, ObjectId>,
    }

    // Test struct with complex ObjectId usage
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct DocumentJson {
        id: ObjectId,
        title: String,
        author_id: ObjectId,
        references: Vec<ObjectId>,
        metadata: HashMap<String, ObjectId>,
        parent_id: Option<ObjectId>,
        nested_refs: HashMap<String, Vec<ObjectId>>,
    }

    // Test struct with HashMap<String, ObjectId>
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct UserWithHashMapObjectIdJson {
        id: ObjectId,
        name: String,
        relationships: HashMap<String, ObjectId>,
    }

    // Test struct with more complex ObjectId nesting
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct ComplexDocumentJson {
        id: ObjectId,
        title: String,
        author_id: ObjectId,
        references: Vec<ObjectId>,
        metadata: HashMap<String, ObjectId>,
        parent_id: Option<ObjectId>,
        nested_refs: HashMap<String, Vec<ObjectId>>,
    }

    // Test struct with optional nested ObjectId
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct PostJson {
        id: ObjectId,
        title: String,
        author_id: ObjectId,
        parent_id: Option<ObjectId>,
    }

    // Test struct with HashMap<String, ObjectId>
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct UserWithOtherHashMapObjectIdJson {
        id: ObjectId,
        name: String,
        metadata: HashMap<String, ObjectId>,
    }

    impl ObjectId {
        fn new() -> Self {
            ObjectId("507f1f77bcf86cd799439011".to_string())
        }

        fn to_hex(&self) -> String {
            self.0.clone()
        }
    }

    #[test]
    #[cfg(all(feature = "object_id", feature = "typescript", feature = "zod"))]
    fn test_basic_object_id_types() {
        let ts_definition = UserJson::ts_definition();
        
        // TypeScript should use ObjectId type
        assert!(ts_definition.contains("id: ObjectId;"));
        assert!(ts_definition.contains("name: string;"));
        assert!(ts_definition.contains("email: string | undefined;"));
        
        // Zod schema should use the MongoDB ObjectId structure with regex validation - now in separate method
        let zod_schema = UserJson::zod_schema();
        assert!(zod_schema.contains("id: z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) }),"));
        assert!(zod_schema.contains("name: z.string(),"));
        assert!(zod_schema.contains("email: z.string().or(z.undefined()),"));
    }

    #[test]
    #[cfg(all(feature = "object_id", feature = "jsonschema"))]
    fn test_object_id_json_schema() {
        let schema = UserJson::json_schema();
        let properties = schema["properties"].as_object().unwrap();
        
        // Check basic ObjectId field
        let id_prop = &properties["id"];
        assert_eq!(id_prop["type"], "object");
        assert_eq!(id_prop["properties"]["$oid"]["type"], "string");
        assert_eq!(id_prop["required"][0], "$oid");
        assert_eq!(id_prop["additionalProperties"], false);
        
        // Check other fields are unaffected
        assert_eq!(properties["name"]["type"], "string");
    }

    #[test]
    #[cfg(all(feature = "object_id", feature = "typescript", feature = "zod"))]
    fn test_optional_object_id() {
        let ts_definition = UserWithOptionalIdJson::ts_definition();
        
        // TypeScript should use ObjectId type
        assert!(ts_definition.contains("id: ObjectId | undefined;"));
        assert!(ts_definition.contains("name: string;"));
        assert!(ts_definition.contains("email: string;"));
        
        // Zod schema should use the MongoDB ObjectId structure with regex validation - now in separate method
        let zod_schema = UserWithOptionalIdJson::zod_schema();
        assert!(zod_schema.contains("id: z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) }).or(z.undefined()),"));
        assert!(zod_schema.contains("name: z.string(),"));
        assert!(zod_schema.contains("email: z.string(),"));
    }

    #[test]
    #[cfg(all(feature = "object_id", feature = "zod"))]
    fn test_optional_object_id_zod_schema() {
        let zod_schema = UserWithOptionalIdJson::zod_schema();
        
        // Should handle optional ObjectId correctly
        assert!(zod_schema.contains("id: z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) }).or(z.undefined()),"));
        assert!(zod_schema.contains("name: z.string(),"));
        assert!(zod_schema.contains("email: z.string(),"));
    }

    #[test]
    #[cfg(all(feature = "object_id", feature = "typescript", feature = "zod"))]
    fn test_object_id_arrays() {
        let ts_definition = UserWithObjectIdArrayJson::ts_definition();
        
        // TypeScript should use ObjectId type
        assert!(ts_definition.contains("id: ObjectId;"));
        assert!(ts_definition.contains("name: string;"));
        assert!(ts_definition.contains("friend_ids: Array<ObjectId>;"));
        
        // Zod schema should use the MongoDB ObjectId structure with regex validation - now in separate method
        let zod_schema = UserWithObjectIdArrayJson::zod_schema();
        assert!(zod_schema.contains("id: z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) }),"));
        assert!(zod_schema.contains("name: z.string(),"));
        assert!(zod_schema.contains("friend_ids: z.array(z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) })),"));
    }

    #[test]
    #[cfg(all(feature = "object_id", feature = "jsonschema"))]
    fn test_object_id_arrays_json_schema() {
        let schema = UserWithObjectIdArrayJson::json_schema();
        let properties = schema["properties"].as_object().unwrap();
        
        // Check array of ObjectId
        let friend_ids_prop = &properties["friend_ids"];
        assert_eq!(friend_ids_prop["type"], "array");
        
        let items = &friend_ids_prop["items"];
        assert_eq!(items["type"], "object");
        assert_eq!(items["properties"]["$oid"]["type"], "string");
        assert_eq!(items["required"][0], "$oid");
        assert_eq!(items["additionalProperties"], false);
    }

    #[test]
    #[cfg(all(feature = "object_id", feature = "zod"))]
    fn test_object_id_arrays_zod_schema() {
        let zod_schema = UserWithObjectIdArrayJson::zod_schema();
        
        // Should handle array of ObjectId correctly
        assert!(zod_schema.contains("friend_ids: z.array(z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) })),"));
    }

    #[test]
    #[cfg(all(feature = "object_id", feature = "typescript", feature = "zod"))]
    fn test_hashmap_with_object_id_values() {
        let ts_definition = UserWithObjectIdMapJson::ts_definition();
        
        // TypeScript should use ObjectId type
        assert!(ts_definition.contains("id: ObjectId;"));
        assert!(ts_definition.contains("name: string;"));
        assert!(ts_definition.contains("relationships: Partial<Record<string, ObjectId>>;"));
        
        // Zod schema should use the MongoDB ObjectId structure with regex validation - now in separate method
        let zod_schema = UserWithObjectIdMapJson::zod_schema();
        assert!(zod_schema.contains("id: z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) }),"));
        assert!(zod_schema.contains("name: z.string(),"));
        assert!(zod_schema.contains("relationships: z.record(z.string(), z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) })),"));
    }

    #[test]
    #[cfg(all(feature = "object_id", feature = "jsonschema"))]
    fn test_hashmap_object_id_json_schema() {
        let schema = UserWithObjectIdMapJson::json_schema();
        let properties = schema["properties"].as_object().unwrap();
        
        // Check HashMap<String, ObjectId>
        let relationships_prop = &properties["relationships"];
        assert_eq!(relationships_prop["type"], "object");
        
        let additional_props = &relationships_prop["additionalProperties"];
        assert_eq!(additional_props["type"], "object");
        assert_eq!(additional_props["properties"]["$oid"]["type"], "string");
        assert_eq!(additional_props["required"][0], "$oid");
        assert_eq!(additional_props["additionalProperties"], false);
    }

    #[test]
    #[cfg(all(feature = "object_id", feature = "zod"))]
    fn test_hashmap_object_id_zod_schema() {
        let zod_schema = UserWithObjectIdMapJson::zod_schema();
        
        // Should handle HashMap<String, ObjectId> correctly
        assert!(zod_schema.contains("relationships: z.record(z.string(), z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) })),"));
    }

    #[test]
    #[cfg(all(feature = "object_id", feature = "typescript", feature = "zod"))]
    fn test_complex_nested_object_id_structures() {
        let ts_definition = ComplexDocumentJson::ts_definition();
        
        // TypeScript should handle all ObjectId variations
        assert!(ts_definition.contains("id: ObjectId;"));
        assert!(ts_definition.contains("author_id: ObjectId;"));
        assert!(ts_definition.contains("references: Array<ObjectId>;"));
        assert!(ts_definition.contains("metadata: Partial<Record<string, ObjectId>>;"));
        assert!(ts_definition.contains("parent_id: ObjectId | undefined;"));
        assert!(ts_definition.contains("nested_refs: Partial<Record<string, Array<ObjectId>>>;"));
        
        // Zod schema should handle all ObjectId variations with regex validation - now in separate method
        let zod_schema = ComplexDocumentJson::zod_schema();
        let regex_pattern = "z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" })";
        assert!(zod_schema.contains(&format!("id: z.object({{ $oid: {} }}),", regex_pattern)));
        assert!(zod_schema.contains(&format!("author_id: z.object({{ $oid: {} }}),", regex_pattern)));
        assert!(zod_schema.contains(&format!("references: z.array(z.object({{ $oid: {} }})),", regex_pattern)));
        assert!(zod_schema.contains(&format!("metadata: z.record(z.string(), z.object({{ $oid: {} }})),", regex_pattern)));
        assert!(zod_schema.contains(&format!("parent_id: z.object({{ $oid: {} }}).or(z.undefined()),", regex_pattern)));
        assert!(zod_schema.contains(&format!("nested_refs: z.record(z.string(), z.array(z.object({{ $oid: {} }}))),", regex_pattern)));
    }

    #[test]
    #[cfg(all(feature = "object_id", feature = "jsonschema"))]
    fn test_complex_object_id_json_schema() {
        let schema = ComplexDocumentJson::json_schema();
        let properties = schema["properties"].as_object().unwrap();
        
        // Test nested_refs: HashMap<String, Vec<ObjectId>>
        let nested_refs_prop = &properties["nested_refs"];
        assert_eq!(nested_refs_prop["type"], "object");
        let additional_props = &nested_refs_prop["additionalProperties"];
        assert_eq!(additional_props["type"], "array");
        let items = &additional_props["items"];
        assert_eq!(items["type"], "object");
        assert_eq!(items["properties"]["$oid"]["type"], "string");
        assert_eq!(items["required"][0], "$oid");
        assert_eq!(items["additionalProperties"], false);
    }

    #[test]
    #[cfg(all(feature = "object_id", feature = "zod"))]
    fn test_complex_object_id_zod_schema() {
        let zod_schema = ComplexDocumentJson::zod_schema();
        
        // Test that complex nested ObjectId structures work
        let regex_pattern = "z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" })";
        assert!(zod_schema.contains(&format!("nested_refs: z.record(z.string(), z.array(z.object({{ $oid: {} }}))),", regex_pattern)));
    }

    #[test]
    #[cfg(all(feature = "object_id", feature = "jsonschema"))]
    fn test_json_schema_optional_parent() {
        let schema = PostJson::json_schema();
        let properties = schema["properties"].as_object().unwrap();
        
        // parent_id should be optional
        let required = schema["required"].as_array().unwrap();
        assert!(!required.contains(&serde_json::Value::String("parent_id".to_string())));
        
        // But still should have the ObjectId structure
        let parent_id_prop = &properties["parent_id"];
        assert_eq!(parent_id_prop["type"], "object");
        assert_eq!(parent_id_prop["properties"]["$oid"]["type"], "string");
        assert_eq!(parent_id_prop["required"][0], "$oid");
        assert_eq!(parent_id_prop["additionalProperties"], false);
    }

    #[test]
    #[cfg(all(feature = "object_id", feature = "zod"))]
    fn test_object_id_zod_schema() {
        let zod_schema = PostJson::zod_schema();
        
        // Should handle optional ObjectId correctly
        assert!(zod_schema.contains("parent_id: z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) }).or(z.undefined()),"));
    }

    #[test]
    fn test_object_id_compilation_smoke_test() {
        // This test ensures all ObjectId types compile without panics
        let _user = UserJson {
            id: ObjectId::new(),
            name: "Test User".to_string(),
            email: Some("test@example.com".to_string()),
        };
        
        // If we get here without panics, ObjectId support is working at compile time
        assert!(true);
    }
} 