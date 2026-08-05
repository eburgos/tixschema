#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
use serde::{Deserialize, Serialize};
#[cfg(all(test, feature = "jsonschema"))]
use serde_json::Value;
#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
use std::collections::HashMap;
#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
use tixschema::model_schema;

#[cfg(all(
    test,
    any(feature = "typescript", feature = "jsonschema", feature = "zod")
))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct LargeNumbers {
    array_of_i64: Vec<i64>,
    array_of_u64: Vec<u64>,
    id: String,
    large_signed: i64,
    large_unsigned: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_large_signed: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_large_unsigned: Option<u64>,
}

#[cfg(all(
    test,
    any(feature = "typescript", feature = "jsonschema", feature = "zod")
))]
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct MixedIntegers {
    isize_type: isize,
    large_i64: i64,
    large_u64: u64,
    medium_i16: i16,
    medium_u16: u16,
    normal_i32: i32,
    normal_u32: u32,
    size_type: usize,
    small_i8: i8,
    small_u8: u8,
}

#[cfg(all(
    test,
    any(feature = "typescript", feature = "jsonschema", feature = "zod")
))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct PrimitiveTypesShowcase {
    arch_signed: isize,
    arch_unsigned: usize,
    array_f64: Vec<f64>,
    array_i8: Vec<i8>,
    array_u64: Vec<u64>,
    float_double: f64,
    float_single: f32,
    large_signed: i64,
    large_unsigned: u64,
    map_to_f64: HashMap<String, f64>,
    map_to_f64_array: HashMap<String, Vec<f64>>,
    map_to_i8: HashMap<String, i8>,
    map_to_i8_array: HashMap<String, Vec<i8>>,
    map_to_u64: HashMap<String, u64>,
    map_to_u64_array: HashMap<String, Vec<u64>>,
    medium_signed: i32,
    medium_unsigned: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    opt_array_f64: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    opt_array_i8: Option<Vec<i8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    opt_array_u64: Option<Vec<u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    opt_f64: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    opt_i8: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    opt_u64: Option<u64>,
    small_signed: i16,
    small_unsigned: u16,
    tiny_signed: i8,
    tiny_unsigned: u8,
}

/// `char` in every position it can be written: a field, an array element, an optional field, a map
/// value, and a tuple slot. Gated on the union of every consuming test's own gate, including the
/// serde-only round-trip, so every feature combination sees the fixture either used or absent.
#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct CharTypesShowcase {
    array_of_char: Vec<char>,
    initial: char,
    map_to_char: HashMap<String, char>,
    #[serde(skip_serializing_if = "Option::is_none")]
    opt_char: Option<char>,
    pair: (char, char),
}

#[cfg(any(feature = "typescript", feature = "jsonschema", feature = "zod"))]
#[test]
fn test_primitive_structs_constructible() {
    let large = LargeNumbers {
        array_of_i64: Vec::new(),
        array_of_u64: Vec::new(),
        id: String::new(),
        large_signed: 0,
        large_unsigned: 0,
        optional_large_signed: None,
        optional_large_unsigned: None,
    };
    assert!(large.id.is_empty());
    let mixed = MixedIntegers {
        isize_type: 0,
        large_i64: 0,
        large_u64: 0,
        medium_i16: 0,
        medium_u16: 0,
        normal_i32: 0,
        normal_u32: 0,
        size_type: 0,
        small_i8: 0,
        small_u8: 0,
    };
    assert_eq!(mixed.normal_i32, 0_i32);
    let showcase = PrimitiveTypesShowcase {
        arch_signed: 0,
        arch_unsigned: 0,
        array_f64: Vec::new(),
        array_i8: Vec::new(),
        array_u64: Vec::new(),
        float_double: 0.0,
        float_single: 0.0,
        large_signed: 0,
        large_unsigned: 0,
        map_to_f64: HashMap::new(),
        map_to_f64_array: HashMap::new(),
        map_to_i8: HashMap::new(),
        map_to_i8_array: HashMap::new(),
        map_to_u64: HashMap::new(),
        map_to_u64_array: HashMap::new(),
        medium_signed: 0,
        medium_unsigned: 0,
        opt_array_f64: None,
        opt_array_i8: None,
        opt_array_u64: None,
        opt_f64: None,
        opt_i8: None,
        opt_u64: None,
        small_signed: 0,
        small_unsigned: 0,
        tiny_signed: 0,
        tiny_unsigned: 0,
    };
    assert!(showcase.array_f64.is_empty());
}

#[cfg(any(feature = "typescript", feature = "jsonschema", feature = "zod"))]
#[test]
fn test_char_struct_constructible() {
    let showcase = CharTypesShowcase {
        array_of_char: vec!['a', 'b'],
        initial: 'x',
        map_to_char: HashMap::from([("k".to_owned(), 'v')]),
        opt_char: Some('o'),
        pair: ('p', 'q'),
    };
    assert_eq!(showcase.initial, 'x');
}

/// serde writes a `char` as the one-character string it renders through `Display`, and reads only
/// that back — the wire every surface's rendering is fixed from.
#[cfg(feature = "serde")]
#[test]
fn test_char_serde_roundtrip() {
    let showcase = CharTypesShowcase {
        array_of_char: vec!['a', 'b'],
        initial: 'x',
        map_to_char: HashMap::from([("k".to_owned(), 'v')]),
        opt_char: Some('o'),
        pair: ('p', 'q'),
    };
    let json = serde_json::to_value(&showcase).unwrap();
    assert_eq!(json["initial"], serde_json::json!("x"));
    assert_eq!(json["array_of_char"], serde_json::json!(["a", "b"]));
    assert_eq!(json["map_to_char"]["k"], serde_json::json!("v"));
    assert_eq!(json["opt_char"], serde_json::json!("o"));
    assert_eq!(json["pair"], serde_json::json!(["p", "q"]));

    let round_tripped: CharTypesShowcase = serde_json::from_value(json).unwrap();
    assert_eq!(round_tripped, showcase);
}

#[cfg(all(feature = "typescript", feature = "zod"))]
#[test]
fn test_char_types_typescript_and_zod() {
    let ts_definition = CharTypesShowcase::ts_definition();
    assert!(
        ts_definition.contains("initial: string;"),
        "Got: {ts_definition}"
    );
    assert!(
        ts_definition.contains("array_of_char: Array<string>;"),
        "Got: {ts_definition}"
    );
    assert!(
        ts_definition.contains("map_to_char: Partial<Record<string, string>>;"),
        "Got: {ts_definition}"
    );
    assert_ts_omitted_fields_contain(&ts_definition, &["opt_char"], "string");
    assert!(
        ts_definition.contains("pair: [string, string];"),
        "Got: {ts_definition}"
    );

    let zod_schema = CharTypesShowcase::zod_schema();
    assert!(
        zod_schema.contains("initial: z.string().length(1)"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema.contains("array_of_char: z.array(z.string().length(1))"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema.contains("map_to_char: z.record(z.string(), z.string().length(1))"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema.contains("opt_char: z.union([z.string().length(1), z.undefined()])"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema.contains("pair: z.tuple([z.string().length(1), z.string().length(1)])"),
        "Got: {zod_schema}"
    );
}

#[cfg(feature = "jsonschema")]
#[test]
fn test_char_types_json_schema() {
    let schema = CharTypesShowcase::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    let one_character_string =
        serde_json::json!({ "type": "string", "minLength": 1_i32, "maxLength": 1_i32 });
    assert_eq!(properties["initial"], one_character_string);
    assert_eq!(
        properties["array_of_char"],
        serde_json::json!({ "type": "array", "items": one_character_string })
    );
    assert_eq!(
        properties["map_to_char"],
        serde_json::json!({ "type": "object", "additionalProperties": one_character_string })
    );
    assert_eq!(properties["opt_char"], one_character_string);
    assert_eq!(
        properties["pair"],
        serde_json::json!({
            "type": "array",
            "prefixItems": [one_character_string, one_character_string],
            "items": false,
            "minItems": 2_u64,
            "maxItems": 2_u64
        })
    );

    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&Value::String("initial".to_owned())));
    assert!(!required.contains(&Value::String("opt_char".to_owned())));
}

#[cfg(feature = "jsonschema")]
fn assert_array_property(
    properties: &serde_json::Map<String, serde_json::Value>,
    field_name: &str,
    item_type: &str,
) {
    assert_eq!(properties[field_name]["type"], "array");
    assert_eq!(properties[field_name]["items"]["type"], item_type);
}

#[cfg(all(feature = "typescript", feature = "zod"))]
fn assert_zod_fields_contain(zod_schema: &str, fields: &[&str], expected_pattern: &str) {
    for field in fields {
        let expected = format!("{field}: {expected_pattern}");
        assert!(zod_schema.contains(&expected));
    }
}

#[cfg(all(feature = "typescript", feature = "zod"))]
fn assert_ts_fields_contain(ts_definition: &str, fields: &[&str], expected_suffix: &str) {
    for field in fields {
        let expected = format!("{field}: {expected_suffix};");
        assert!(ts_definition.contains(&expected));
    }
}

/// The member an `Option` field written with a `skip_serializing_if` renders as. The attribute
/// decides the wire, not the spelling, and none of these fields carries `ts_optional`.
#[cfg(all(feature = "typescript", feature = "zod"))]
fn omitted_member(name: &str, ts_type: &str) -> String {
    format!("{name}: {ts_type} | undefined;")
}

#[cfg(all(feature = "typescript", feature = "zod"))]
fn assert_ts_omitted_fields_contain(ts_definition: &str, fields: &[&str], ts_type: &str) {
    for field in fields {
        let expected = omitted_member(field, ts_type);
        assert!(
            ts_definition.contains(&expected),
            "missing {expected}, got: {ts_definition}"
        );
    }
}

#[cfg(feature = "jsonschema")]
fn assert_hashmap_array_property(
    properties: &serde_json::Map<String, serde_json::Value>,
    field_name: &str,
    item_type: &str,
) {
    assert_eq!(properties[field_name]["type"], "object");
    assert_eq!(
        properties[field_name]["additionalProperties"]["type"],
        "array"
    );
    assert_eq!(
        properties[field_name]["additionalProperties"]["items"]["type"],
        item_type
    );
}

#[cfg(feature = "jsonschema")]
fn assert_hashmap_property(
    properties: &serde_json::Map<String, serde_json::Value>,
    field_name: &str,
    value_type: &str,
) {
    assert_eq!(properties[field_name]["type"], "object");
    assert_eq!(
        properties[field_name]["additionalProperties"]["type"],
        value_type
    );
}

#[cfg(feature = "jsonschema")]
fn assert_property_type(
    properties: &serde_json::Map<String, serde_json::Value>,
    field_name: &str,
    expected_type: &str,
) {
    assert_eq!(properties[field_name]["type"], expected_type);
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_64bit_integers_json_schema() {
    let schema = LargeNumbers::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    assert!(properties.contains_key("large_unsigned"));
    assert!(properties.contains_key("large_signed"));

    assert_eq!(properties["large_unsigned"]["type"], "integer");
    assert_eq!(properties["large_signed"]["type"], "integer");

    assert!(properties.contains_key("optional_large_unsigned"));
    assert!(properties.contains_key("optional_large_signed"));

    assert_eq!(properties["array_of_u64"]["type"], "array");
    assert_eq!(properties["array_of_u64"]["items"]["type"], "integer");
    assert_eq!(properties["array_of_i64"]["type"], "array");
    assert_eq!(properties["array_of_i64"]["items"]["type"], "integer");

    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&Value::String("large_unsigned".to_owned())));
    assert!(required.contains(&Value::String("large_signed".to_owned())));
    assert!(required.contains(&Value::String("array_of_u64".to_owned())));
    assert!(required.contains(&Value::String("array_of_i64".to_owned())));

    assert!(!required.contains(&Value::String("optional_large_unsigned".to_owned())));
    assert!(!required.contains(&Value::String("optional_large_signed".to_owned())));
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_64bit_integers_ts_definition() {
    let ts_definition = LargeNumbers::ts_definition();

    assert!(ts_definition.contains("large_unsigned: number;"));
    assert!(ts_definition.contains("large_signed: number;"));
    assert_ts_omitted_fields_contain(
        &ts_definition,
        &["optional_large_unsigned", "optional_large_signed"],
        "number",
    );
    assert!(ts_definition.contains("array_of_u64: Array<number>;"));
    assert!(ts_definition.contains("array_of_i64: Array<number>;"));

    let zod_schema = LargeNumbers::zod_schema();
    assert!(zod_schema.contains("large_unsigned: z.number().int()"));
    assert!(zod_schema.contains("large_signed: z.number().int()"));
    assert!(
        zod_schema.contains("optional_large_unsigned: z.union([z.number().int(), z.undefined()])")
    );
    assert!(
        zod_schema.contains("optional_large_signed: z.union([z.number().int(), z.undefined()])")
    );
    assert!(zod_schema.contains("array_of_u64: z.array(z.number().int())"));
    assert!(zod_schema.contains("array_of_i64: z.array(z.number().int())"));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_mixed_integers_json_schema() {
    let schema = MixedIntegers::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    assert_eq!(properties["small_u8"]["type"], "integer");
    assert_eq!(properties["small_i8"]["type"], "integer");
    assert_eq!(properties["medium_u16"]["type"], "integer");
    assert_eq!(properties["medium_i16"]["type"], "integer");
    assert_eq!(properties["normal_u32"]["type"], "integer");
    assert_eq!(properties["normal_i32"]["type"], "integer");
    assert_eq!(properties["large_u64"]["type"], "integer");
    assert_eq!(properties["large_i64"]["type"], "integer");
    assert_eq!(properties["size_type"]["type"], "integer");
    assert_eq!(properties["isize_type"]["type"], "integer");
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_mixed_integers_ts_definition() {
    let ts_definition = MixedIntegers::ts_definition();

    assert!(ts_definition.contains("small_u8: number;"));
    assert!(ts_definition.contains("small_i8: number;"));
    assert!(ts_definition.contains("medium_u16: number;"));
    assert!(ts_definition.contains("medium_i16: number;"));
    assert!(ts_definition.contains("normal_u32: number;"));
    assert!(ts_definition.contains("normal_i32: number;"));
    assert!(ts_definition.contains("large_u64: number;"));
    assert!(ts_definition.contains("large_i64: number;"));
    assert!(ts_definition.contains("size_type: number;"));
    assert!(ts_definition.contains("isize_type: number;"));

    let zod_schema = MixedIntegers::zod_schema();
    assert!(zod_schema.contains("small_u8: z.number().int()"));
    assert!(zod_schema.contains("small_i8: z.number().int()"));
    assert!(zod_schema.contains("medium_u16: z.number().int()"));
    assert!(zod_schema.contains("medium_i16: z.number().int()"));
    assert!(zod_schema.contains("normal_u32: z.number().int()"));
    assert!(zod_schema.contains("normal_i32: z.number().int()"));
    assert!(zod_schema.contains("large_u64: z.number().int()"));
    assert!(zod_schema.contains("large_i64: z.number().int()"));
    assert!(zod_schema.contains("size_type: z.number().int()"));
    assert!(zod_schema.contains("isize_type: z.number().int()"));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_primitive_types_json_schema_details() {
    let schema = PrimitiveTypesShowcase::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    let integer_fields = [
        "tiny_signed",
        "tiny_unsigned",
        "small_signed",
        "small_unsigned",
        "medium_signed",
        "medium_unsigned",
        "large_signed",
        "large_unsigned",
        "arch_signed",
        "arch_unsigned",
    ];
    for field in &integer_fields {
        assert_property_type(properties, field, "integer");
    }

    assert_property_type(properties, "float_single", "number");
    assert_property_type(properties, "float_double", "number");

    let required = schema["required"].as_array().unwrap();
    for opt_field in &["opt_i8", "opt_u64", "opt_f64"] {
        assert!(!required.contains(&serde_json::Value::String((*opt_field).to_owned())));
    }

    assert_array_property(properties, "array_i8", "integer");
    assert_array_property(properties, "array_u64", "integer");
    assert_array_property(properties, "array_f64", "number");

    assert_hashmap_property(properties, "map_to_i8", "integer");
    assert_hashmap_property(properties, "map_to_u64", "integer");
    assert_hashmap_property(properties, "map_to_f64", "number");

    assert_hashmap_array_property(properties, "map_to_i8_array", "integer");
    assert_hashmap_array_property(properties, "map_to_u64_array", "integer");
    assert_hashmap_array_property(properties, "map_to_f64_array", "number");
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_primitive_types_typescript_generation_details() {
    let ts_definition = PrimitiveTypesShowcase::ts_definition();

    let numeric_fields = [
        "tiny_signed",
        "tiny_unsigned",
        "small_signed",
        "small_unsigned",
        "medium_signed",
        "medium_unsigned",
        "large_signed",
        "large_unsigned",
        "arch_signed",
        "arch_unsigned",
        "float_single",
        "float_double",
    ];
    assert_ts_fields_contain(&ts_definition, &numeric_fields, "number");

    assert_ts_omitted_fields_contain(&ts_definition, &["opt_i8", "opt_u64", "opt_f64"], "number");

    assert_ts_fields_contain(
        &ts_definition,
        &["array_i8", "array_u64", "array_f64"],
        "Array<number>",
    );

    assert_ts_omitted_fields_contain(
        &ts_definition,
        &["opt_array_i8", "opt_array_u64", "opt_array_f64"],
        "Array<number>",
    );

    assert_ts_fields_contain(
        &ts_definition,
        &["map_to_i8", "map_to_u64", "map_to_f64"],
        "Partial<Record<string, number>>",
    );

    assert_ts_fields_contain(
        &ts_definition,
        &["map_to_i8_array", "map_to_u64_array", "map_to_f64_array"],
        "Partial<Record<string, Array<number>>>",
    );

    let zod_schema = PrimitiveTypesShowcase::zod_schema();

    assert_zod_fields_contain(
        &zod_schema,
        &[
            "tiny_signed",
            "tiny_unsigned",
            "large_signed",
            "large_unsigned",
        ],
        "z.number().int()",
    );

    assert_zod_fields_contain(&zod_schema, &["float_single", "float_double"], "z.number()");

    assert_zod_fields_contain(
        &zod_schema,
        &["opt_i8", "opt_u64"],
        "z.union([z.number().int(), z.undefined()])",
    );
    assert!(zod_schema.contains("opt_f64: z.union([z.number(), z.undefined()])"));

    assert_zod_fields_contain(
        &zod_schema,
        &["array_i8", "array_u64"],
        "z.array(z.number().int())",
    );
    assert!(zod_schema.contains("array_f64: z.array(z.number())"));

    assert_zod_fields_contain(
        &zod_schema,
        &["map_to_i8", "map_to_u64"],
        "z.record(z.string(), z.number().int())",
    );
    assert!(zod_schema.contains("map_to_f64: z.record(z.string(), z.number())"));

    assert_zod_fields_contain(
        &zod_schema,
        &["map_to_i8_array", "map_to_u64_array"],
        "z.record(z.string(), z.array(z.number().int()))",
    );
    assert!(zod_schema.contains("map_to_f64_array: z.record(z.string(), z.array(z.number()))"));
}
