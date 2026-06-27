use serde::{Deserialize, Serialize};
use tixschema::model_schema;

/// Test 1 + 2 + 3: A `(String, String)` struct field renders as a tuple in
/// TypeScript, Zod, and JSON Schema.
#[test]
fn test_string_pair_tuple_field() {
    /// One row of an alphanumeric "map-value" lookup table.
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    #[serde(rename_all = "camelCase")]
    pub struct AlphanumericMapInput {
        pub output: String,
        /// A (matchValue) pair, serialized as a 2-element array.
        pub values: (String, String),
    }

    let ts = AlphanumericMapInput::ts_definition();

    // TS: tuple, not an object literal.
    assert!(
        ts.contains("values: [string, string]"),
        "Expected a tuple field. Got: {ts}"
    );
    assert!(
        !ts.contains("element_0"),
        "Should not emit element_N object keys. Got: {ts}"
    );
}

/// Test 2: Zod renders `z.tuple([...])`.
#[cfg(feature = "zod")]
#[test]
fn test_string_pair_tuple_field_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Pair {
        pub values: (String, String),
    }

    let zod = Pair::zod_schema();

    assert!(
        zod.contains("values: z.tuple([z.string(), z.string()])"),
        "Expected z.tuple in Zod schema. Got: {zod}"
    );
    // Must not emit the malformed bare-object-literal form.
    assert!(
        !zod.contains("element_0"),
        "Should not emit element_N keys in Zod. Got: {zod}"
    );
}

/// Test 3: JSON Schema renders a fixed-arity array with prefixItems.
#[cfg(feature = "jsonschema")]
#[test]
fn test_string_pair_tuple_field_json_schema() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Pair {
        pub values: (String, String),
    }

    let schema = Pair::json_schema();
    let values = &schema["properties"]["values"];

    assert_eq!(values["type"].as_str(), Some("array"));
    assert_eq!(values["items"].as_bool(), Some(false));
    assert_eq!(values["minItems"].as_u64(), Some(2));
    assert_eq!(values["maxItems"].as_u64(), Some(2));

    let prefix = values["prefixItems"].as_array().unwrap();
    assert_eq!(prefix.len(), 2, "Two prefix items. Got: {prefix:?}");
    for item in prefix {
        assert_eq!(item["type"].as_str(), Some("string"));
    }
}

/// Test 4: Mixed element types `(String, i64, bool)`.
#[test]
fn test_mixed_element_tuple_field() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Mixed {
        pub triple: (String, i64, bool),
    }

    let ts = Mixed::ts_definition();
    assert!(
        ts.contains("triple: [string, number, boolean]"),
        "Expected mixed tuple in TS. Got: {ts}"
    );
}

/// Test 4b: Mixed element types in Zod.
#[cfg(feature = "zod")]
#[test]
fn test_mixed_element_tuple_field_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Mixed {
        pub triple: (String, i64, bool),
    }

    let zod = Mixed::zod_schema();
    assert!(
        zod.contains("triple: z.tuple([z.string(), z.number().int(), z.boolean()])"),
        "Expected mixed tuple in Zod. Got: {zod}"
    );
}

/// Test 5: A sibling/custom element type renders by reference.
#[test]
fn test_sibling_element_tuple_field() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct DocumentId {
        pub id: String,
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct WithSibling {
        pub pair: (DocumentId, String),
    }

    let ts = WithSibling::ts_definition();
    assert!(
        ts.contains("pair: [DocumentId, string]"),
        "Expected sibling element in TS tuple. Got: {ts}"
    );

    #[cfg(feature = "zod")]
    {
        let zod = WithSibling::zod_schema();
        assert!(
            zod.contains("pair: z.tuple([DocumentId$Schema, z.string()])"),
            "Expected sibling $Schema in Zod tuple. Got: {zod}"
        );
    }
}

/// Test 6: serde round-trip — a tuple field serializes as a JSON array.
#[test]
fn test_tuple_field_serde_roundtrip() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct S {
        pub output: String,
        pub values: (String, String),
    }

    let original = S {
        output: "out".to_owned(),
        values: ("a".to_owned(), "b".to_owned()),
    };

    let json = serde_json::to_value(&original).unwrap();
    assert_eq!(
        json.get("values"),
        Some(&serde_json::json!(["a", "b"])),
        "Tuple should serialize as a JSON array. Got: {json}"
    );

    let back: S = serde_json::from_value(json).unwrap();
    assert_eq!(back, original);
}

/// Test: an optional tuple field gets the optional wrapping.
#[cfg(feature = "zod")]
#[test]
fn test_optional_tuple_field_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct OptionalPair {
        pub values: Option<(String, String)>,
    }

    let zod = OptionalPair::zod_schema();
    assert!(
        zod.contains(
            "values: z.union([z.tuple([z.string(), z.string()]), z.undefined()]).prefault(undefined)"
        ),
        "Expected optional tuple wrapping. Got: {zod}"
    );
}
