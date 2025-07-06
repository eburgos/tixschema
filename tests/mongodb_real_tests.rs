// Real MongoDB ObjectId compatibility tests
// These tests use the actual mongodb library to ensure our macro works
// correctly with real MongoDB ObjectIds

#[cfg(test)]
mod tests {
    use core_model_macros::model_schema;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    
    // Import the real MongoDB ObjectId - only available in tests
    use mongodb::bson::oid::ObjectId;

    // Basic struct with real ObjectId
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct RealUserJson {
        id: ObjectId,
        name: String,
        email: Option<String>,
    }

    // Complex struct with various ObjectId usages
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct RealDocumentJson {
        id: ObjectId,
        title: String,
        author_id: ObjectId,
        references: Vec<ObjectId>,
        metadata: HashMap<String, ObjectId>,
        parent_id: Option<ObjectId>,
        nested_refs: HashMap<String, Vec<ObjectId>>,
    }

    #[test]
    fn test_real_objectid_basic_types() {
        let ts_definition = RealUserJson::ts_definition();
        
        // TypeScript should use ObjectId type
        assert!(ts_definition.contains("id: ObjectId;"));
        assert!(ts_definition.contains("name: string;"));
        assert!(ts_definition.contains("email: string | undefined;"));
        
        // Zod schema should use the MongoDB ObjectId structure with regex validation
        assert!(ts_definition.contains("id: z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) }),"));
        assert!(ts_definition.contains("name: z.string(),"));
        assert!(ts_definition.contains("email: z.string().or(z.undefined()),"));
    }

    #[test]
    fn test_real_objectid_json_schema() {
        let schema = RealUserJson::json_schema();
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
    fn test_real_objectid_serialization() {
        // Create a real ObjectId
        let real_oid = ObjectId::new();
        
        let user = RealUserJson {
            id: real_oid,
            name: "Test User".to_string(),
            email: Some("test@example.com".to_string()),
        };
        
        // Test serialization
        let serialized = serde_json::to_string(&user).unwrap();
        
        // Should contain the MongoDB $oid structure
        assert!(serialized.contains("\"$oid\""));
        assert!(serialized.contains(&real_oid.to_hex()));
        
        // Test deserialization
        let deserialized: RealUserJson = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.id, real_oid);
        assert_eq!(deserialized.name, "Test User");
        assert_eq!(deserialized.email, Some("test@example.com".to_string()));
    }

    #[test]
    fn test_real_objectid_complex_structures() {
        let ts_definition = RealDocumentJson::ts_definition();
        
        // TypeScript should handle all ObjectId variations
        assert!(ts_definition.contains("id: ObjectId;"));
        assert!(ts_definition.contains("author_id: ObjectId;"));
        assert!(ts_definition.contains("references: Array<ObjectId>;"));
        assert!(ts_definition.contains("metadata: Partial<Record<string, ObjectId>>;"));
        assert!(ts_definition.contains("parent_id: ObjectId | undefined;"));
        assert!(ts_definition.contains("nested_refs: Partial<Record<string, Array<ObjectId>>>;"));
        
        // Zod schema should handle all ObjectId variations with regex validation
        let regex_pattern = "z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" })";
        assert!(ts_definition.contains(&format!("id: z.object({{ $oid: {} }}),", regex_pattern)));
        assert!(ts_definition.contains(&format!("author_id: z.object({{ $oid: {} }}),", regex_pattern)));
        assert!(ts_definition.contains(&format!("references: z.array(z.object({{ $oid: {} }})),", regex_pattern)));
        assert!(ts_definition.contains(&format!("metadata: z.record(z.string(), z.object({{ $oid: {} }})),", regex_pattern)));
        assert!(ts_definition.contains(&format!("parent_id: z.object({{ $oid: {} }}).or(z.undefined()),", regex_pattern)));
        assert!(ts_definition.contains(&format!("nested_refs: z.record(z.string(), z.array(z.object({{ $oid: {} }}))),", regex_pattern)));
    }

    #[test]
    fn test_real_objectid_complex_serialization() {
        // Create real ObjectIds
        let doc_id = ObjectId::new();
        let author_id = ObjectId::new();
        let ref1 = ObjectId::new();
        let ref2 = ObjectId::new();
        let meta_oid = ObjectId::new();
        let parent_id = ObjectId::new();
        let nested_oid1 = ObjectId::new();
        let nested_oid2 = ObjectId::new();
        
        let document = RealDocumentJson {
            id: doc_id,
            title: "Test Document".to_string(),
            author_id,
            references: vec![ref1, ref2],
            metadata: {
                let mut map = HashMap::new();
                map.insert("template".to_string(), meta_oid);
                map
            },
            parent_id: Some(parent_id),
            nested_refs: {
                let mut map = HashMap::new();
                map.insert("related".to_string(), vec![nested_oid1, nested_oid2]);
                map
            },
        };
        
        // Test serialization
        let serialized = serde_json::to_string_pretty(&document).unwrap();
        
        println!("=== REAL MONGODB OBJECTID SERIALIZATION ===");
        println!("{}", serialized);
        
        // Should contain all the ObjectId hex values in $oid format
        assert!(serialized.contains(&doc_id.to_hex()));
        assert!(serialized.contains(&author_id.to_hex()));
        assert!(serialized.contains(&ref1.to_hex()));
        assert!(serialized.contains(&ref2.to_hex()));
        assert!(serialized.contains(&meta_oid.to_hex()));
        assert!(serialized.contains(&parent_id.to_hex()));
        assert!(serialized.contains(&nested_oid1.to_hex()));
        assert!(serialized.contains(&nested_oid2.to_hex()));
        
        // Should use proper MongoDB structure
        assert!(serialized.contains("\"$oid\""));
        
        // Test round-trip deserialization
        let deserialized: RealDocumentJson = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.id, doc_id);
        assert_eq!(deserialized.author_id, author_id);
        assert_eq!(deserialized.references, vec![ref1, ref2]);
        assert_eq!(deserialized.parent_id, Some(parent_id));
    }

    #[test]
    fn test_real_objectid_json_schema_structure() {
        let schema = RealDocumentJson::json_schema();
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
        
        // Test metadata: HashMap<String, ObjectId>
        let metadata_prop = &properties["metadata"];
        assert_eq!(metadata_prop["type"], "object");
        let meta_additional_props = &metadata_prop["additionalProperties"];
        assert_eq!(meta_additional_props["type"], "object");
        assert_eq!(meta_additional_props["properties"]["$oid"]["type"], "string");
        assert_eq!(meta_additional_props["required"][0], "$oid");
        assert_eq!(meta_additional_props["additionalProperties"], false);
    }

    #[test]
    fn test_real_objectid_validation_compatibility() {
        // Test that real ObjectIds produce valid hex strings that match our regex
        let real_oid = ObjectId::new();
        let hex_string = real_oid.to_hex();
        
        // Should be exactly 24 characters
        assert_eq!(hex_string.len(), 24);
        
        // Should match our regex pattern: /^[a-f\d]{24}$/i
        let regex = regex::Regex::new(r"^[a-f\d]{24}$").unwrap();
        assert!(regex.is_match(&hex_string), "Real ObjectId hex '{}' should match our validation regex", hex_string);
        
        // Test with multiple ObjectIds to ensure consistency
        for _ in 0..10 {
            let oid = ObjectId::new();
            let hex = oid.to_hex();
            assert_eq!(hex.len(), 24);
            assert!(regex.is_match(&hex), "ObjectId hex '{}' should match regex", hex);
        }
    }

    #[test]
    fn test_real_objectid_compilation_smoke_test() {
        // This test ensures all ObjectId types compile without panics with real MongoDB ObjectIds
        let _user_schema = RealUserJson::json_schema();
        let _document_schema = RealDocumentJson::json_schema();
        
        let _user_ts = RealUserJson::ts_definition();
        let _document_ts = RealDocumentJson::ts_definition();
        
        // If we get here without panics, real MongoDB ObjectId support is working
        assert!(true);
    }
} 