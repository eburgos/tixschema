use core_model_macros::model_schema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(test)]
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
            S: serde::Serializer,
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
            D: serde::Deserializer<'de>,
        {
            use serde::de::{self, MapAccess, Visitor};
            use std::fmt;

            struct ObjectIdVisitor;

            impl<'de> Visitor<'de> for ObjectIdVisitor {
                type Value = ObjectId;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
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
                                let _: serde_json::Value = map.next_value()?;
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

    // Basic ObjectId usage
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct UserJson {
        id: ObjectId,
        name: String,
        email: Option<String>,
    }

    // ObjectId arrays
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct PostJson {
        id: ObjectId,
        title: String,
        tags: Vec<ObjectId>,
        author_id: ObjectId,
    }

    // Optional ObjectId fields
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct CommentJson {
        id: ObjectId,
        content: String,
        parent_id: Option<ObjectId>,
        author_id: ObjectId,
    }

    // HashMap with ObjectId values
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct ProjectJson {
        id: ObjectId,
        name: String,
        member_roles: HashMap<String, ObjectId>,
        tags: Vec<ObjectId>,
    }

    // Complex nested structures with ObjectId
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct ComplexDocumentJson {
        id: ObjectId,
        title: String,
        author_id: ObjectId,
        references: Vec<ObjectId>,
        metadata: HashMap<String, ObjectId>,
        optional_parent: Option<ObjectId>,
        nested_refs: HashMap<String, Vec<ObjectId>>,
    }

    #[test]
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
    fn test_object_id_arrays() {
        let ts_definition = PostJson::ts_definition();
        
        // TypeScript should use Array<ObjectId>
        assert!(ts_definition.contains("id: ObjectId;"));
        assert!(ts_definition.contains("tags: Array<ObjectId>;"));
        assert!(ts_definition.contains("author_id: ObjectId;"));
        
        // Zod schema should use array of ObjectId objects with regex validation - now in separate method
        let zod_schema = PostJson::zod_schema();
        assert!(zod_schema.contains("id: z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) }),"));
        assert!(zod_schema.contains("tags: z.array(z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) })),"));
        assert!(zod_schema.contains("author_id: z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) }),"));
    }

    #[test]
    fn test_object_id_arrays_json_schema() {
        let schema = PostJson::json_schema();
        let properties = schema["properties"].as_object().unwrap();
        
        // Check array of ObjectId
        let tags_prop = &properties["tags"];
        assert_eq!(tags_prop["type"], "array");
        assert_eq!(tags_prop["items"]["type"], "object");
        assert_eq!(tags_prop["items"]["properties"]["$oid"]["type"], "string");
        assert_eq!(tags_prop["items"]["required"][0], "$oid");
        assert_eq!(tags_prop["items"]["additionalProperties"], false);
    }

    #[test]
    fn test_optional_object_id() {
        let ts_definition = CommentJson::ts_definition();
        
        // TypeScript should use ObjectId | undefined
        assert!(ts_definition.contains("id: ObjectId;"));
        assert!(ts_definition.contains("parent_id: ObjectId | undefined;"));
        assert!(ts_definition.contains("author_id: ObjectId;"));
        
        // Zod schema should use .or(z.undefined()) for optional with regex validation - now in separate method
        let zod_schema = CommentJson::zod_schema();
        assert!(zod_schema.contains("id: z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) }),"));
        assert!(zod_schema.contains("parent_id: z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) }).or(z.undefined()),"));
        assert!(zod_schema.contains("author_id: z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) }),"));
    }

    #[test]
    fn test_hashmap_with_object_id_values() {
        let ts_definition = ProjectJson::ts_definition();
        
        // TypeScript should use Partial<Record<string, ObjectId>>
        assert!(ts_definition.contains("member_roles: Partial<Record<string, ObjectId>>;"));
        assert!(ts_definition.contains("tags: Array<ObjectId>;"));
        
        // Zod schema should use record with ObjectId values with regex validation - now in separate method
        let zod_schema = ProjectJson::zod_schema();
        assert!(zod_schema.contains("member_roles: z.record(z.string(), z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) })),"));
        assert!(zod_schema.contains("tags: z.array(z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) })),"));
    }

    #[test]
    fn test_hashmap_object_id_json_schema() {
        let schema = ProjectJson::json_schema();
        let properties = schema["properties"].as_object().unwrap();
        
        // Check HashMap<String, ObjectId>
        let member_roles_prop = &properties["member_roles"];
        assert_eq!(member_roles_prop["type"], "object");
        let additional_props = &member_roles_prop["additionalProperties"];
        assert_eq!(additional_props["type"], "object");
        assert_eq!(additional_props["properties"]["$oid"]["type"], "string");
        assert_eq!(additional_props["required"][0], "$oid");
        assert_eq!(additional_props["additionalProperties"], false);
    }

    #[test]
    fn test_complex_nested_object_id_structures() {
        let ts_definition = ComplexDocumentJson::ts_definition();
        
        // TypeScript should handle all ObjectId variations
        assert!(ts_definition.contains("id: ObjectId;"));
        assert!(ts_definition.contains("author_id: ObjectId;"));
        assert!(ts_definition.contains("references: Array<ObjectId>;"));
        assert!(ts_definition.contains("metadata: Partial<Record<string, ObjectId>>;"));
        assert!(ts_definition.contains("optional_parent: ObjectId | undefined;"));
        assert!(ts_definition.contains("nested_refs: Partial<Record<string, Array<ObjectId>>>;"));
        
        // Zod schema should handle all ObjectId variations with regex validation - now in separate method
        let zod_schema = ComplexDocumentJson::zod_schema();
        assert!(zod_schema.contains("id: z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) }),"));
        assert!(zod_schema.contains("author_id: z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) }),"));
        assert!(zod_schema.contains("references: z.array(z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) })),"));
        assert!(zod_schema.contains("metadata: z.record(z.string(), z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) })),"));
        assert!(zod_schema.contains("optional_parent: z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) }).or(z.undefined()),"));
        assert!(zod_schema.contains("nested_refs: z.record(z.string(), z.array(z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) }))),"));
    }

    #[test]
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
        
        // Test optional_parent: Option<ObjectId>
        let optional_parent_prop = &properties["optional_parent"];
        assert_eq!(optional_parent_prop["type"], "object");
        assert_eq!(optional_parent_prop["properties"]["$oid"]["type"], "string");
        assert_eq!(optional_parent_prop["required"][0], "$oid");
        assert_eq!(optional_parent_prop["additionalProperties"], false);
    }

    #[test]
    fn test_object_id_compilation_smoke_test() {
        // This test ensures all ObjectId types compile without panics
        let _user_schema = UserJson::json_schema();
        let _post_schema = PostJson::json_schema();
        let _comment_schema = CommentJson::json_schema();
        let _project_schema = ProjectJson::json_schema();
        let _complex_schema = ComplexDocumentJson::json_schema();
        
        let _user_ts = UserJson::ts_definition();
        let _post_ts = PostJson::ts_definition();
        let _comment_ts = CommentJson::ts_definition();
        let _project_ts = ProjectJson::ts_definition();
        let _complex_ts = ComplexDocumentJson::ts_definition();
        
        // If we get here without panics, ObjectId support is working
        assert!(true);
    }
} 