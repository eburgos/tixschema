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

// The remargin compact-row shape is `Vec<(Option<String>, Vec<usize>, String,
// Option<String>)>`. That exact struct field trips `clippy::type_complexity`
// (the outer `Vec` + nested `Vec<usize>` + 4 elements together clear the
// threshold), and factoring it into a `type` alias would make the macro treat it
// as a sibling reference rather than a tuple — so the happy path is proven in two
// composable halves: the exact inner tuple below (all four slots, including
// `Vec<usize>` → `Array<number>` and both `Option` → null), and the outer
// `Vec` → `Array<[...]>` wrap in `test_tuple_element_option_array_wrap`. The wire
// round-trip exercises the full shape through a `type` alias, which serde reads
// natively.

/// Null-flavor happy path (TS): each `Option` element inside a tuple renders as
/// `T | null` — a positional slot serializes `None` as JSON `null`, unlike an
/// omittable object key. Also the scenario-5 negative guard: a required tuple
/// field emits no `undefined` at all.
#[test]
fn test_tuple_element_option_null_flavor_ts() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Row {
        pub row: (Option<String>, Vec<usize>, String, Option<String>),
    }

    let ts = Row::ts_definition();
    assert!(
        ts.contains("row: [string | null, Array<number>, string, string | null]"),
        "Expected null-flavored tuple elements in TS. Got: {ts}"
    );
    assert!(
        !ts.contains("undefined"),
        "A required tuple field must not emit `undefined`. Got: {ts}"
    );
}

/// Null-flavor happy path (Zod): each tuple-element `Option<T>` renders as
/// `z.nullable(T)`, and the required field emits no `z.undefined()`.
#[cfg(feature = "zod")]
#[test]
fn test_tuple_element_option_null_flavor_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Row {
        pub row: (Option<String>, Vec<usize>, String, Option<String>),
    }

    let zod = Row::zod_schema();
    assert!(
        zod.contains(
            "row: z.tuple([z.nullable(z.string()), z.array(z.number().int()), z.string(), z.nullable(z.string())])"
        ),
        "Expected z.nullable tuple elements in Zod. Got: {zod}"
    );
    assert!(
        !zod.contains("undefined"),
        "A required tuple field must not emit `undefined`. Got: {zod}"
    );
}

/// Null-flavor happy path (JSON Schema): each optional slot becomes
/// `anyOf [<base>, null]`; arity (`minItems`/`maxItems`) stays 4 — nullability
/// never changes item count.
#[cfg(feature = "jsonschema")]
#[test]
fn test_tuple_element_option_null_flavor_json_schema() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Row {
        pub row: (Option<String>, Vec<usize>, String, Option<String>),
    }

    let schema = Row::json_schema();
    let tuple = &schema["properties"]["row"];

    assert_eq!(tuple["type"].as_str(), Some("array"));
    assert_eq!(tuple["items"].as_bool(), Some(false));
    assert_eq!(tuple["minItems"].as_u64(), Some(4));
    assert_eq!(tuple["maxItems"].as_u64(), Some(4));

    let prefix = tuple["prefixItems"].as_array().unwrap();
    assert_eq!(prefix.len(), 4, "Arity stays 4. Got: {prefix:?}");

    let nullable_string =
        serde_json::json!({ "anyOf": [{ "type": "string" }, { "type": "null" }] });
    assert_eq!(
        prefix[0], nullable_string,
        "Slot 0 (Option<String>) should be anyOf null. Got: {}",
        prefix[0]
    );
    assert_eq!(
        prefix[3], nullable_string,
        "Slot 3 (Option<String>) should be anyOf null. Got: {}",
        prefix[3]
    );
    // Non-optional slots stay plain — no null wrapping.
    assert_eq!(
        prefix[1],
        serde_json::json!({ "type": "array", "items": { "type": "integer" } })
    );
    assert_eq!(prefix[2], serde_json::json!({ "type": "string" }));
}

/// Array-wrap composition (TS): a `Vec<(Option<Vec<usize>>, String)>` field wraps
/// the tuple in `Array<[...]>`, and the null flavor survives the wrap. The
/// `Option<Vec<usize>>` slot proves the array wrap happens inside the base with
/// `null` on top: `Array<number> | null`.
#[test]
fn test_tuple_element_option_array_wrap_ts() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Rows {
        pub pair: Vec<(Option<Vec<usize>>, String)>,
    }

    let ts = Rows::ts_definition();
    assert!(
        ts.contains("pair: Array<[Array<number> | null, string]>"),
        "Expected outer Array wrap over null-flavored tuple in TS. Got: {ts}"
    );
    assert!(
        !ts.contains("undefined"),
        "A required tuple field must not emit `undefined`. Got: {ts}"
    );
}

/// Array-wrap composition (Zod): `z.array(z.tuple([...]))` with the null flavor
/// preserved under the array wrap.
#[cfg(feature = "zod")]
#[test]
fn test_tuple_element_option_array_wrap_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Rows {
        pub pair: Vec<(Option<Vec<usize>>, String)>,
    }

    let zod = Rows::zod_schema();
    assert!(
        zod.contains("pair: z.array(z.tuple([z.nullable(z.array(z.number().int())), z.string()]))"),
        "Expected z.array wrap over null-flavored tuple in Zod. Got: {zod}"
    );
    assert!(
        !zod.contains("undefined"),
        "A required tuple field must not emit `undefined`. Got: {zod}"
    );
}

/// Array-wrap composition (JSON Schema): the outer `Vec` becomes
/// `{ type: array, items: <tuple schema> }`, and the optional slot keeps its
/// `anyOf [<array>, null]` flavor.
#[cfg(feature = "jsonschema")]
#[test]
fn test_tuple_element_option_array_wrap_json_schema() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Rows {
        pub pair: Vec<(Option<Vec<usize>>, String)>,
    }

    let schema = Rows::json_schema();
    let outer = &schema["properties"]["pair"];
    assert_eq!(outer["type"].as_str(), Some("array"));

    let tuple = &outer["items"];
    assert_eq!(tuple["minItems"].as_u64(), Some(2));
    assert_eq!(tuple["maxItems"].as_u64(), Some(2));

    let prefix = tuple["prefixItems"].as_array().unwrap();
    assert_eq!(
        prefix[0],
        serde_json::json!({
            "anyOf": [{ "type": "array", "items": { "type": "integer" } }, { "type": "null" }]
        }),
        "Slot 0 (Option<Vec<usize>>) should be anyOf [array, null]. Got: {}",
        prefix[0]
    );
}

/// Wire round-trip: serde emits `null` for a `None` tuple slot and reads it back
/// as `None`. This is the behavior the null-flavored schemas describe, exercised
/// on the exact remargin row shape (a `type` alias, so serde handles it natively).
#[test]
fn test_tuple_element_option_serde_roundtrip() {
    type Row = (Option<String>, Vec<usize>, String, Option<String>);

    let row: Row = (None, vec![26_usize], "internal_report.md".to_owned(), None);
    let json = serde_json::to_value(&row).unwrap();
    assert_eq!(
        json,
        serde_json::json!([null, [26_i64], "internal_report.md", null]),
        "None tuple slots must serialize as null. Got: {json}"
    );

    let back: Row = serde_json::from_value(json).unwrap();
    assert_eq!(back, row, "null must deserialize back to None");
}
