use core::fmt;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use tixschema::model_schema;

// Test struct with more complex ObjectId nesting
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct ComplexDocument {
    author_id: ObjectId,
    id: ObjectId,
    metadata: HashMap<String, ObjectId>,
    nested_refs: HashMap<String, Vec<ObjectId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<ObjectId>,
    references: Vec<ObjectId>,
    title: String,
}

// Test struct with complex ObjectId usage
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Document {
    author_id: ObjectId,
    id: ObjectId,
    metadata: HashMap<String, ObjectId>,
    nested_refs: HashMap<String, Vec<ObjectId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<ObjectId>,
    references: Vec<ObjectId>,
    title: String,
}

// Mock ObjectId type for testing - compatible with mongodb::bson::oid::ObjectId
// The real MongoDB ObjectId serializes to { "$oid": "hex_string" } in JSON
// and to a plain string in other contexts
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectId(String);

// Test struct with optional nested ObjectId
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Post {
    author_id: ObjectId,
    id: ObjectId,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<ObjectId>,
    title: String,
}

// Test struct with ObjectId field
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct User {
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    id: ObjectId,
    name: String,
}

// Test struct with HashMap<String, ObjectId>
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct UserWithHashMapObjectId {
    id: ObjectId,
    name: String,
    relationships: HashMap<String, ObjectId>,
}

// Test struct with ObjectId array
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct UserWithObjectIdArray {
    friend_ids: Vec<ObjectId>,
    id: ObjectId,
    name: String,
}

// Test struct with ObjectId in HashMap
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct UserWithObjectIdMap {
    id: ObjectId,
    name: String,
    relationships: HashMap<String, ObjectId>,
}

// Test struct with optional ObjectId field
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct UserWithOptionalId {
    email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<ObjectId>,
    name: String,
}

// Test struct with HashMap<String, ObjectId>
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct UserWithOtherHashMapObjectId {
    id: ObjectId,
    metadata: HashMap<String, ObjectId>,
    name: String,
}

impl<'de> Deserialize<'de> for ObjectId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
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
                            let _: de::IgnoredAny = map.next_value()?;
                        }
                    }
                }
                let resolved_oid = oid.ok_or_else(|| de::Error::missing_field("$oid"))?;
                Ok(ObjectId(resolved_oid))
            }
        }

        deserializer.deserialize_struct("ObjectId", &["$oid"], ObjectIdVisitor)
    }
}

impl ObjectId {
    fn new() -> Self {
        Self("507f1f77bcf86cd799439011".to_owned())
    }
}

impl Serialize for ObjectId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // MongoDB ObjectId serializes as { "$oid": "hex_string" } in JSON
        use serde::ser::SerializeStruct as _;
        let mut state = serializer.serialize_struct("ObjectId", 1)?;
        state.serialize_field("$oid", &self.0)?;
        state.end()
    }
}

#[test]
fn test_mongodb_structs_constructible() {
    let complex = ComplexDocument {
        author_id: ObjectId::new(),
        id: ObjectId::new(),
        metadata: HashMap::new(),
        nested_refs: HashMap::new(),
        parent_id: None,
        references: Vec::new(),
        title: String::new(),
    };
    assert!(complex.title.is_empty());
    let document = Document {
        author_id: ObjectId::new(),
        id: ObjectId::new(),
        metadata: HashMap::new(),
        nested_refs: HashMap::new(),
        parent_id: None,
        references: Vec::new(),
        title: String::new(),
    };
    assert!(document.title.is_empty());
    let post = Post {
        author_id: ObjectId::new(),
        id: ObjectId::new(),
        parent_id: None,
        title: String::new(),
    };
    assert!(post.title.is_empty());
    let hashmap_obj = UserWithHashMapObjectId {
        id: ObjectId::new(),
        name: String::new(),
        relationships: HashMap::new(),
    };
    assert!(hashmap_obj.name.is_empty());
    let array_obj = UserWithObjectIdArray {
        friend_ids: Vec::new(),
        id: ObjectId::new(),
        name: String::new(),
    };
    assert!(array_obj.name.is_empty());
    let map_obj = UserWithObjectIdMap {
        id: ObjectId::new(),
        name: String::new(),
        relationships: HashMap::new(),
    };
    assert!(map_obj.name.is_empty());
    let optional_id = UserWithOptionalId {
        email: String::new(),
        id: None,
        name: String::new(),
    };
    assert!(optional_id.name.is_empty());
    let other_map = UserWithOtherHashMapObjectId {
        id: ObjectId::new(),
        metadata: HashMap::new(),
        name: String::new(),
    };
    assert!(other_map.name.is_empty());
}

#[test]
#[cfg(all(feature = "object_id", feature = "typescript", feature = "zod"))]
fn test_basic_object_id_types() {
    let ts_definition = User::ts_definition();

    // TypeScript should use ObjectId type
    assert!(ts_definition.contains("id: ObjectId;"));
    assert!(ts_definition.contains("name: string;"));
    assert!(ts_definition.contains("email: string | undefined;"));

    // Zod schema should use the MongoDB ObjectId structure with regex validation - now in separate method
    let zod_schema = User::zod_schema();
    assert!(zod_schema.contains("id: z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: \"Invalid ObjectId\" }) }),"));
    assert!(zod_schema.contains("name: z.string(),"));
    assert!(
        zod_schema.contains("email: z.union([z.string(), z.undefined()]).prefault(undefined),")
    );
}

#[test]
#[cfg(all(feature = "object_id", feature = "typescript", feature = "zod"))]
fn test_complex_nested_object_id_structures() {
    let ts_definition = ComplexDocument::ts_definition();

    // TypeScript should handle all ObjectId variations
    assert!(ts_definition.contains("id: ObjectId;"));
    assert!(ts_definition.contains("author_id: ObjectId;"));
    assert!(ts_definition.contains("references: Array<ObjectId>;"));
    assert!(ts_definition.contains("metadata: Partial<Record<string, ObjectId>>;"));
    assert!(ts_definition.contains("parent_id: ObjectId | undefined;"));
    assert!(ts_definition.contains("nested_refs: Partial<Record<string, Array<ObjectId>>>;"));

    // Zod schema should handle all ObjectId variations with regex validation - now in separate method
    let zod_schema = ComplexDocument::zod_schema();
    let regex_pattern = "z.string().regex(/^[a-f0-9]{24}$/, { message: \"Invalid ObjectId\" })";
    assert!(zod_schema.contains(&format!("id: z.object({{ $oid: {regex_pattern} }}),")));
    assert!(zod_schema.contains(&format!(
        "author_id: z.object({{ $oid: {regex_pattern} }}),"
    )));
    assert!(zod_schema.contains(&format!(
        "references: z.array(z.object({{ $oid: {regex_pattern} }})),"
    )));
    assert!(zod_schema.contains(&format!(
        "metadata: z.record(z.string(), z.object({{ $oid: {regex_pattern} }})),"
    )));
    assert!(zod_schema.contains(&format!(
        "parent_id: z.union([z.object({{ $oid: {regex_pattern} }}), z.undefined()]).prefault(undefined),"
    )));
    assert!(zod_schema.contains(&format!(
        "nested_refs: z.record(z.string(), z.array(z.object({{ $oid: {regex_pattern} }}))),"
    )));
}

#[test]
#[cfg(all(feature = "object_id", feature = "jsonschema"))]
fn test_complex_object_id_json_schema() {
    let schema = ComplexDocument::json_schema();
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
    let zod_schema = ComplexDocument::zod_schema();

    // Test that complex nested ObjectId structures work
    let regex_pattern = "z.string().regex(/^[a-f0-9]{24}$/, { message: \"Invalid ObjectId\" })";
    assert!(zod_schema.contains(&format!(
        "nested_refs: z.record(z.string(), z.array(z.object({{ $oid: {regex_pattern} }}))),"
    )));
}

// Test that includes Document
#[test]
#[cfg(feature = "jsonschema")]
fn test_document_json() {
    let schema = Document::json_schema();
    let properties = schema["properties"].as_object().unwrap();
    let metadata_prop = &properties["metadata"];
    assert_eq!(metadata_prop["type"], "object");
    assert_eq!(metadata_prop["additionalProperties"]["type"], "object");
    assert_eq!(
        metadata_prop["additionalProperties"]["properties"]["$oid"]["type"],
        "string"
    );
}

#[test]
#[cfg(all(feature = "object_id", feature = "jsonschema"))]
fn test_hashmap_object_id_json_schema() {
    let schema = UserWithObjectIdMap::json_schema();
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
    let zod_schema = UserWithObjectIdMap::zod_schema();

    // Should handle HashMap<String, ObjectId> correctly
    assert!(zod_schema.contains("relationships: z.record(z.string(), z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: \"Invalid ObjectId\" }) })),"));
}

#[test]
#[cfg(all(feature = "object_id", feature = "typescript", feature = "zod"))]
fn test_hashmap_with_object_id_values() {
    let ts_definition = UserWithObjectIdMap::ts_definition();

    // TypeScript should use ObjectId type
    assert!(ts_definition.contains("id: ObjectId;"));
    assert!(ts_definition.contains("name: string;"));
    assert!(ts_definition.contains("relationships: Partial<Record<string, ObjectId>>;"));

    // Zod schema should use the MongoDB ObjectId structure with regex validation - now in separate method
    let zod_schema = UserWithObjectIdMap::zod_schema();
    assert!(zod_schema.contains("id: z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: \"Invalid ObjectId\" }) }),"));
    assert!(zod_schema.contains("name: z.string(),"));
    assert!(zod_schema.contains("relationships: z.record(z.string(), z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: \"Invalid ObjectId\" }) })),"));
}

#[test]
#[cfg(all(feature = "object_id", feature = "jsonschema"))]
fn test_json_schema_optional_parent() {
    let schema = Post::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    // parent_id should be optional
    let required = schema["required"].as_array().unwrap();
    assert!(!required.contains(&serde_json::Value::String("parent_id".to_owned())));

    // But still should have the ObjectId structure
    let parent_id_prop = &properties["parent_id"];
    assert_eq!(parent_id_prop["type"], "object");
    assert_eq!(parent_id_prop["properties"]["$oid"]["type"], "string");
    assert_eq!(parent_id_prop["required"][0], "$oid");
    assert_eq!(parent_id_prop["additionalProperties"], false);
}

#[test]
#[cfg(all(feature = "object_id", feature = "typescript", feature = "zod"))]
fn test_object_id_arrays() {
    let ts_definition = UserWithObjectIdArray::ts_definition();

    // TypeScript should use ObjectId type
    assert!(ts_definition.contains("id: ObjectId;"));
    assert!(ts_definition.contains("name: string;"));
    assert!(ts_definition.contains("friend_ids: Array<ObjectId>;"));

    // Zod schema should use the MongoDB ObjectId structure with regex validation - now in separate method
    let zod_schema = UserWithObjectIdArray::zod_schema();
    assert!(zod_schema.contains("id: z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: \"Invalid ObjectId\" }) }),"));
    assert!(zod_schema.contains("name: z.string(),"));
    assert!(zod_schema.contains("friend_ids: z.array(z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: \"Invalid ObjectId\" }) })),"));
}

#[test]
#[cfg(all(feature = "object_id", feature = "jsonschema"))]
fn test_object_id_arrays_json_schema() {
    let schema = UserWithObjectIdArray::json_schema();
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
    let zod_schema = UserWithObjectIdArray::zod_schema();

    // Should handle array of ObjectId correctly
    assert!(zod_schema.contains("friend_ids: z.array(z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: \"Invalid ObjectId\" }) })),"));
}

#[test]
fn test_object_id_compilation_smoke_test() {
    // This test ensures all ObjectId types compile without panics
    let user = User {
        id: ObjectId::new(),
        name: "Test User".to_owned(),
        email: Some("test@example.com".to_owned()),
    };

    // If we get here without panics, ObjectId support is working at compile time
    assert_eq!(user.name, "Test User");
}

#[test]
#[cfg(all(feature = "object_id", feature = "jsonschema"))]
fn test_object_id_json_schema() {
    let schema = User::json_schema();
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
#[cfg(all(feature = "object_id", feature = "zod"))]
fn test_object_id_zod_schema() {
    let zod_schema = Post::zod_schema();

    // Should handle optional ObjectId correctly
    assert!(zod_schema.contains("parent_id: z.union([z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: \"Invalid ObjectId\" }) }), z.undefined()]).prefault(undefined),"));
}

#[test]
#[cfg(all(feature = "object_id", feature = "typescript", feature = "zod"))]
fn test_optional_object_id() {
    let ts_definition = UserWithOptionalId::ts_definition();

    // TypeScript should use ObjectId type
    assert!(ts_definition.contains("id: ObjectId | undefined;"));
    assert!(ts_definition.contains("name: string;"));
    assert!(ts_definition.contains("email: string;"));

    // Zod schema should use the MongoDB ObjectId structure with regex validation - now in separate method
    let zod_schema = UserWithOptionalId::zod_schema();
    assert!(zod_schema.contains("id: z.union([z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: \"Invalid ObjectId\" }) }), z.undefined()]).prefault(undefined),"));
    assert!(zod_schema.contains("name: z.string(),"));
    assert!(zod_schema.contains("email: z.string(),"));
}

#[test]
#[cfg(all(feature = "object_id", feature = "zod"))]
fn test_optional_object_id_zod_schema() {
    let zod_schema = UserWithOptionalId::zod_schema();

    // Should handle optional ObjectId correctly
    assert!(zod_schema.contains("id: z.union([z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: \"Invalid ObjectId\" }) }), z.undefined()]).prefault(undefined),"));
    assert!(zod_schema.contains("name: z.string(),"));
    assert!(zod_schema.contains("email: z.string(),"));
}

// Test that includes UserWithHashMapObjectId
#[test]
#[cfg(feature = "jsonschema")]
fn test_user_with_hashmap_object_id_json() {
    let schema = UserWithHashMapObjectId::json_schema();
    let properties = schema["properties"].as_object().unwrap();
    let relationships_prop = &properties["relationships"];
    assert_eq!(relationships_prop["type"], "object");
    assert_eq!(relationships_prop["additionalProperties"]["type"], "object");
    assert_eq!(
        relationships_prop["additionalProperties"]["properties"]["$oid"]["type"],
        "string"
    );
}

// Test that includes UserWithOtherHashMapObjectId
#[test]
#[cfg(feature = "jsonschema")]
fn test_user_with_other_hashmap_object_id_json() {
    let schema = UserWithOtherHashMapObjectId::json_schema();
    let properties = schema["properties"].as_object().unwrap();
    let metadata_prop = &properties["metadata"];
    assert_eq!(metadata_prop["type"], "object");
    assert_eq!(metadata_prop["additionalProperties"]["type"], "object");
    assert_eq!(
        metadata_prop["additionalProperties"]["properties"]["$oid"]["type"],
        "string"
    );
}
