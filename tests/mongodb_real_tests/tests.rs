// Real MongoDB ObjectId compatibility tests
// These tests use the actual mongodb library to ensure our macro works
// correctly with real MongoDB ObjectIds

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tixschema::model_schema;

// Import the real MongoDB ObjectId - only available in tests
use mongodb::bson::oid::ObjectId;

// Complex struct with various ObjectId usages
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct RealDocument {
    author_id: ObjectId,
    id: ObjectId,
    metadata: HashMap<String, ObjectId>,
    nested_refs: HashMap<String, Vec<ObjectId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<ObjectId>,
    references: Vec<ObjectId>,
    title: String,
}

// An enum-keyed map enumerates its keys, so every member carries the value schema outright
// instead of the open `additionalProperties` the String-keyed maps above use.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RefSlot {
    Author,
    Reviewer,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct SlottedRefs {
    by_slot: HashMap<RefSlot, ObjectId>,
}

// A map value that is itself a map carries the same closed `$oid` object its members would carry
// one level up.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NestedRefMaps {
    ids_by_group: HashMap<String, HashMap<String, ObjectId>>,
    run_batches: HashMap<String, Vec<HashMap<String, ObjectId>>>,
}

// Every position an `ObjectId` can be written in, gathered into one item: a field, an array item, a
// tuple element, and a member of a map on each key path.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct EveryOidPosition {
    by_name: HashMap<String, ObjectId>,
    by_slot: HashMap<RefSlot, ObjectId>,
    many: Vec<ObjectId>,
    nested: HashMap<String, HashMap<String, ObjectId>>,
    one: ObjectId,
    pair: (ObjectId, String),
}

// A tuple struct's lone element is a slot, not a field.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct OidSlot(ObjectId);

// Basic struct with real ObjectId
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct RealUser {
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    id: ObjectId,
    name: String,
}

#[test]
#[cfg(all(feature = "object_id", feature = "typescript", feature = "zod"))]
fn test_real_objectid_basic_types() {
    let ts_definition = RealUser::ts_definition();

    // TypeScript should use ObjectId type
    assert!(ts_definition.contains("id: ObjectId;"));
    assert!(ts_definition.contains("name: string;"));
    assert!(ts_definition.contains("email: string | undefined;"));

    // Zod schema should use the MongoDB ObjectId structure with regex validation - now in separate method
    let zod_schema = RealUser::zod_schema();
    assert!(zod_schema.contains("id: z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: \"Invalid ObjectId\" }) }),"));
    assert!(zod_schema.contains("name: z.string(),"));
    assert!(
        zod_schema.contains("email: z.union([z.string(), z.undefined()]).prefault(undefined),")
    );
}

#[test]
#[cfg(all(feature = "object_id", feature = "jsonschema", feature = "typescript"))]
fn test_real_objectid_compilation_smoke_test() {
    // This test ensures all ObjectId types compile without panics with real MongoDB ObjectIds
    let user_schema = RealUser::json_schema();
    let _document_schema = RealDocument::json_schema();

    let _user_ts = RealUser::ts_definition();
    let _document_ts = RealDocument::ts_definition();

    // If we get here without panics, real MongoDB ObjectId support is working
    assert!(!user_schema.is_null());
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

    let document = RealDocument {
        id: doc_id,
        title: "Test Document".to_owned(),
        author_id,
        references: vec![ref1, ref2],
        metadata: {
            let mut map = HashMap::new();
            map.insert("template".to_owned(), meta_oid);
            map
        },
        parent_id: Some(parent_id),
        nested_refs: {
            let mut map = HashMap::new();
            map.insert("related".to_owned(), vec![nested_oid1, nested_oid2]);
            map
        },
    };

    // Test serialization
    let serialized = serde_json::to_string_pretty(&document).unwrap();

    // println!("=== REAL MONGODB OBJECTID SERIALIZATION ===");
    // println!("{serialized}");

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
    let deserialized: RealDocument = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.id, doc_id);
    assert_eq!(deserialized.author_id, author_id);
    assert_eq!(deserialized.references, vec![ref1, ref2]);
    assert_eq!(deserialized.parent_id, Some(parent_id));
}

#[test]
#[cfg(all(feature = "object_id", feature = "typescript", feature = "zod"))]
fn test_real_objectid_complex_structures() {
    let ts_definition = RealDocument::ts_definition();

    // TypeScript should handle all ObjectId variations
    assert!(ts_definition.contains("id: ObjectId;"));
    assert!(ts_definition.contains("author_id: ObjectId;"));
    assert!(ts_definition.contains("references: Array<ObjectId>;"));
    assert!(ts_definition.contains("metadata: Partial<Record<string, ObjectId>>;"));
    assert!(ts_definition.contains("parent_id: ObjectId | undefined;"));
    assert!(ts_definition.contains("nested_refs: Partial<Record<string, Array<ObjectId>>>;"));

    // Zod schema should handle all ObjectId variations with regex validation - now in separate method
    let zod_schema = RealDocument::zod_schema();
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
fn test_real_objectid_json_schema() {
    let schema = RealUser::json_schema();
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
#[cfg(all(feature = "object_id", feature = "jsonschema"))]
fn test_real_objectid_json_schema_structure() {
    let schema = RealDocument::json_schema();
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
    assert_eq!(
        meta_additional_props["properties"]["$oid"]["type"],
        "string"
    );
    assert_eq!(meta_additional_props["required"][0], "$oid");
    assert_eq!(meta_additional_props["additionalProperties"], false);
}

#[test]
fn test_nested_ref_maps_constructible() {
    let oid = ObjectId::new();
    let nested = NestedRefMaps {
        ids_by_group: HashMap::from([("first".to_owned(), HashMap::from([("a".to_owned(), oid)]))]),
        run_batches: HashMap::from([(
            "first".to_owned(),
            vec![HashMap::from([("a".to_owned(), oid)])],
        )]),
    };
    assert_eq!(nested.ids_by_group["first"]["a"], oid);
    assert_eq!(nested.run_batches["first"][0]["a"], oid);
}

#[test]
fn test_slotted_refs_constructible() {
    let slotted = SlottedRefs {
        by_slot: HashMap::from([
            (RefSlot::Author, ObjectId::new()),
            (RefSlot::Reviewer, ObjectId::new()),
        ]),
    };
    assert_eq!(slotted.by_slot.len(), 2);
}

#[test]
#[cfg(all(feature = "object_id", feature = "jsonschema"))]
fn test_enum_keyed_objectid_map_json_schema() {
    let schema = SlottedRefs::json_schema();
    let by_slot = &schema["properties"]["by_slot"];

    assert_eq!(by_slot["type"], "object");
    assert_eq!(by_slot["additionalProperties"], false);
    let members = RefSlot::enum_members();
    assert_eq!(
        by_slot["properties"].as_object().unwrap().len(),
        members.len(),
        "in: {by_slot}"
    );
    for member in members {
        assert_eq!(
            by_slot["properties"][&member],
            unified_oid_object(),
            "member {member} in: {by_slot}"
        );
    }
}

/// The member schema a nested map carries is the value type's own — for an `ObjectId` the one
/// `$oid` object every position spells, at whatever depth the map nests.
#[test]
#[cfg(all(feature = "object_id", feature = "jsonschema"))]
fn test_nested_objectid_map_json_schema() {
    let schema = NestedRefMaps::json_schema();
    let properties = schema["properties"].as_object().unwrap();
    let oid_member = unified_oid_object();

    for (field_name, expected_value_schema) in [
        (
            "ids_by_group",
            serde_json::json!({
                "type": "object",
                "additionalProperties": oid_member
            }),
        ),
        (
            "run_batches",
            serde_json::json!({
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": oid_member
                }
            }),
        ),
    ] {
        let field = &properties[field_name];
        assert_eq!(field["type"], "object", "in: {field}");
        assert_eq!(
            field["additionalProperties"], expected_value_schema,
            "in: {field}"
        );
    }
}

/// The one `$oid` object every position spells: closed, because that is the object serde writes and
/// every other object this crate emits is closed, and carrying the hex pattern, because that is what
/// the string inside it always holds.
#[cfg(all(feature = "object_id", feature = "jsonschema"))]
fn unified_oid_object() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": { "$oid": { "type": "string", "pattern": "^[a-f0-9]{24}$" } },
        "required": ["$oid"],
        "additionalProperties": false
    })
}

/// An `ObjectId` describes the same wherever it is written — a field, an array item, a tuple
/// element, a tuple struct's own slot, and a map member on either key path all read one builder, so
/// no position can spell the `$oid` object its own way.
#[test]
#[cfg(all(feature = "object_id", feature = "jsonschema"))]
fn every_position_spells_the_same_oid_object() {
    let unified = unified_oid_object();
    let schema = EveryOidPosition::json_schema();
    let properties = &schema["properties"];

    for (position, spelled) in [
        ("field", &properties["one"]),
        ("array item", &properties["many"]["items"]),
        ("tuple element", &properties["pair"]["prefixItems"][0]),
        (
            "String-keyed map member",
            &properties["by_name"]["additionalProperties"],
        ),
        (
            "enum-keyed map member",
            &properties["by_slot"]["properties"]["Author"],
        ),
        (
            "nested map member",
            &properties["nested"]["additionalProperties"]["additionalProperties"],
        ),
    ] {
        assert_eq!(*spelled, unified, "{position} in: {schema}");
    }

    assert_eq!(OidSlot::json_schema(), unified);
}

/// Every object carrying a `$oid` member anywhere in `value`, at any depth.
#[cfg(all(feature = "object_id", feature = "jsonschema"))]
fn collect_oid_objects(value: &serde_json::Value, found: &mut Vec<serde_json::Value>) {
    match value {
        serde_json::Value::Object(members) => {
            if members.contains_key("$oid") {
                found.push(value.clone());
            }
            for member in members.values() {
                collect_oid_objects(member, found);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_oid_objects(item, found);
            }
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {}
    }
}

/// The closure and the pattern are only true information if serde never writes anything else: every
/// `$oid` object it writes, in every position, holds that one member and a hex the pattern matches.
#[test]
#[cfg(all(feature = "object_id", feature = "jsonschema"))]
fn every_serde_written_oid_payload_satisfies_the_unified_spelling() {
    let unified = unified_oid_object();
    let hex_pattern =
        regex::Regex::new(unified["properties"]["$oid"]["pattern"].as_str().unwrap()).unwrap();

    let mut written = Vec::new();
    for _ in 0_u32..64_u32 {
        let payload = EveryOidPosition {
            by_name: HashMap::from([("a".to_owned(), ObjectId::new())]),
            by_slot: HashMap::from([
                (RefSlot::Author, ObjectId::new()),
                (RefSlot::Reviewer, ObjectId::new()),
            ]),
            many: vec![ObjectId::new(), ObjectId::new()],
            nested: HashMap::from([(
                "group".to_owned(),
                HashMap::from([("a".to_owned(), ObjectId::new())]),
            )]),
            one: ObjectId::new(),
            pair: (ObjectId::new(), "x".to_owned()),
        };
        collect_oid_objects(&serde_json::to_value(&payload).unwrap(), &mut written);
        collect_oid_objects(
            &serde_json::to_value(OidSlot(ObjectId::new())).unwrap(),
            &mut written,
        );
    }
    assert!(!written.is_empty());

    for oid_object in written {
        let members = oid_object.as_object().unwrap();
        assert_eq!(
            members.keys().collect::<Vec<_>>(),
            vec!["$oid"],
            "a closed schema admits no other member: {oid_object}"
        );
        let hex = members["$oid"].as_str().unwrap();
        assert!(
            hex_pattern.is_match(hex),
            "the pattern rejects a hex serde wrote: {hex}"
        );
    }
}

#[test]
fn test_real_objectid_serialization() {
    // Create a real ObjectId
    let real_oid = ObjectId::new();

    let user = RealUser {
        id: real_oid,
        name: "Test User".to_owned(),
        email: Some("test@example.com".to_owned()),
    };

    // Test serialization
    let serialized = serde_json::to_string(&user).unwrap();

    // Should contain the MongoDB $oid structure
    assert!(serialized.contains("\"$oid\""));
    assert!(serialized.contains(&real_oid.to_hex()));

    // Test deserialization
    let deserialized: RealUser = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.id, real_oid);
    assert_eq!(deserialized.name, "Test User");
    assert_eq!(deserialized.email, Some("test@example.com".to_owned()));
}

/// The hex a real `ObjectId` round-trips through serde, held to both generated surfaces at once.
///
/// The `pattern` keyword is a flagless ECMA-262 regex, so the Zod literal's flags are read here
/// rather than dropped: a flag on one surface and not on the other is two contracts for one member,
/// whatever the source spelling says. Lower-case is the only case there is to pin, because
/// `ObjectId::to_hex()` is the only thing that writes this member.
#[test]
#[cfg(all(feature = "object_id", feature = "jsonschema", feature = "zod"))]
fn a_real_object_id_hex_satisfies_the_zod_literal_and_the_json_schema_pattern_alike() {
    let real_oid = ObjectId::new();
    let user = RealUser {
        id: real_oid,
        name: "Test User".to_owned(),
        email: Some("test@example.com".to_owned()),
    };

    let wire = serde_json::to_value(&user).unwrap();
    let hex = wire["id"]["$oid"].as_str().unwrap().to_owned();
    assert_eq!(hex, real_oid.to_hex());
    assert_eq!(
        serde_json::from_value::<RealUser>(wire).unwrap(),
        user,
        "the `$oid` object serde wrote does not read back"
    );

    let pattern = RealUser::json_schema()["properties"]["id"]["properties"]["$oid"]["pattern"]
        .as_str()
        .unwrap()
        .to_owned();

    let zod_schema = RealUser::zod_schema();
    let literal = zod_schema.split_once(".regex(/").unwrap().1;
    let (source, after) = literal.split_once('/').unwrap();
    let (flags, _) = after.split_once(',').unwrap();
    assert_eq!(
        source, pattern,
        "the two surfaces spell the `$oid` hex differently"
    );
    assert_eq!(
        flags, "",
        "the Zod literal carries flags the `pattern` keyword has nowhere to hold"
    );

    let hex_pattern = regex::Regex::new(&pattern).unwrap();
    assert!(
        hex_pattern.is_match(&hex),
        "the one hex both surfaces read rejects what serde wrote: {hex}"
    );
}

#[test]
fn test_real_objectid_validation_compatibility() {
    // Test that real ObjectIds produce valid hex strings that match our regex
    let real_oid = ObjectId::new();
    let hex_string = real_oid.to_hex();

    // Should be exactly 24 characters
    assert_eq!(hex_string.len(), 24);

    // Should match our regex pattern: /^[a-f0-9]{24}$/
    let regex = regex::Regex::new("^[a-f0-9]{24}$").unwrap();
    assert!(
        regex.is_match(&hex_string),
        "Real ObjectId hex '{hex_string}' should match our validation regex"
    );

    // Test with multiple ObjectIds to ensure consistency
    for _ in 0_u32..10_u32 {
        let oid = ObjectId::new();
        let hex = oid.to_hex();
        assert_eq!(hex.len(), 24);
        assert!(
            regex.is_match(&hex),
            "ObjectId hex '{hex}' should match regex"
        );
    }
}
