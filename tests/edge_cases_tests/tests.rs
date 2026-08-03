#[cfg(all(test, any(feature = "typescript", feature = "zod", feature = "serde")))]
use serde::{Deserialize, Serialize};

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
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct Address {
    city: String,
    street: String,
    zip_code: String,
}

#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
// Test the specific edge case that was originally failing
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct OriginalBugReproduction {
    // This was the original failing case
    problematic_map: HashMap<String, Vec<u64>>,

    // Nested cases
    string_to_optional_vec_u64: HashMap<String, Option<Vec<u64>>>,

    // Additional cases that might have similar issues
    string_to_vec_bool: HashMap<String, Vec<bool>>,
    string_to_vec_f64: HashMap<String, Vec<f64>>,
    string_to_vec_i64: HashMap<String, Vec<i64>>,
    string_to_vec_string: HashMap<String, Vec<String>>,
}

// A named map value is rendered as the schema module its own `#[model_schema()]` expansion emits,
// so an alias standing in a map value has to carry the attribute like any other referenced type.
#[cfg(all(test, any(feature = "typescript", feature = "zod", feature = "serde")))]
// Inner value type for the optional deeply nested map.
#[model_schema()]
type OptionalNestedValue = Vec<Option<HashMap<String, Option<Vec<i64>>>>>;

#[cfg(all(test, any(feature = "typescript", feature = "zod", feature = "serde")))]
// Inner value type for the quadruple nested map.
#[model_schema()]
type QuadrupleNestedValue = Vec<HashMap<String, Vec<HashMap<String, u64>>>>;

#[cfg(all(test, any(feature = "typescript", feature = "zod", feature = "serde")))]
// Now let's try the really complex case
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct ReallyComplexTest {
    // Another challenging case with optional nested structures
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_nested: Option<HashMap<String, OptionalNestedValue>>,

    // The quadruple nested case that was causing issues
    quadruple_nested: HashMap<String, QuadrupleNestedValue>,
}

#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
// Let's start with just one complex case to debug
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct SimpleComplexTest {
    // Start with just triple-nested to see what fails
    nested_map_of_arrays: HashMap<String, Vec<HashMap<String, u64>>>,
}

#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
// A map value that is itself a map is described at every level: the outer members are inner maps,
// and those maps' own members carry the value type's schema.
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct NestedStringKeyedMaps {
    counts_by_group: HashMap<String, HashMap<String, u64>>,
    labels_by_group: HashMap<String, HashMap<String, String>>,
    rows_by_group: HashMap<String, Vec<HashMap<String, String>>>,
    scores_by_group: HashMap<String, HashMap<String, Option<f64>>>,
    tallies_by_region: HashMap<String, HashMap<String, HashMap<String, u64>>>,
}

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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct UserWithAddress {
    address: Address,
    backup_addresses: Vec<Address>,
    id: String,
    name: String,
}

#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
#[test]
fn test_edge_case_structs_constructible() {
    let address = Address {
        city: String::new(),
        street: String::new(),
        zip_code: String::new(),
    };
    assert!(address.city.is_empty());
    let original = OriginalBugReproduction {
        problematic_map: HashMap::new(),
        string_to_optional_vec_u64: HashMap::new(),
        string_to_vec_bool: HashMap::new(),
        string_to_vec_f64: HashMap::new(),
        string_to_vec_i64: HashMap::new(),
        string_to_vec_string: HashMap::new(),
    };
    assert!(original.problematic_map.is_empty());
    let nested = NestedStringKeyedMaps {
        counts_by_group: HashMap::new(),
        labels_by_group: HashMap::from([(
            "first".to_owned(),
            HashMap::from([("a".to_owned(), "one".to_owned())]),
        )]),
        rows_by_group: HashMap::new(),
        scores_by_group: HashMap::from([(
            "first".to_owned(),
            HashMap::from([("a".to_owned(), None)]),
        )]),
        tallies_by_region: HashMap::new(),
    };
    assert_eq!(nested.labels_by_group["first"]["a"], "one");
    assert_eq!(nested.scores_by_group["first"]["a"], None);
    assert!(nested.counts_by_group.is_empty());
    assert!(nested.rows_by_group.is_empty());
    assert!(nested.tallies_by_region.is_empty());
    let simple = SimpleComplexTest {
        nested_map_of_arrays: HashMap::new(),
    };
    assert!(simple.nested_map_of_arrays.is_empty());
    let user = UserWithAddress {
        address,
        backup_addresses: Vec::new(),
        id: String::new(),
        name: String::new(),
    };
    assert!(user.id.is_empty());
}

#[cfg(all(test, any(feature = "typescript", feature = "zod", feature = "serde")))]
#[test]
fn test_edge_case_aliases_and_complex_constructible() {
    let optional_nested: OptionalNestedValue = Vec::new();
    assert!(optional_nested.is_empty());
    let quadruple: QuadrupleNestedValue = Vec::new();
    assert!(quadruple.is_empty());
    let complex = ReallyComplexTest {
        optional_nested: None,
        quadruple_nested: HashMap::new(),
    };
    assert!(complex.quadruple_nested.is_empty());
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_nested_struct_json_schema() {
    let user_schema = UserWithAddress::json_schema();
    let address_schema = Address::json_schema();

    let properties = user_schema["properties"].as_object().unwrap();

    // Single nested object should reference the nested type
    assert!(properties.contains_key("address"));

    // Array of nested objects should be an array with items referencing the nested type
    assert!(properties.contains_key("backup_addresses"));
    assert_eq!(properties["backup_addresses"]["type"], "array");

    // Verify Address schema exists and is correct
    assert_eq!(address_schema["type"], "object");
    let address_properties = address_schema["properties"].as_object().unwrap();
    assert!(address_properties.contains_key("street"));
    assert!(address_properties.contains_key("city"));
    assert!(address_properties.contains_key("zip_code"));
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_nested_struct_ts_definition() {
    let user_definition = UserWithAddress::ts_definition();
    let address_definition = Address::ts_definition();

    // Check that nested types are referenced properly (without Json suffix)
    assert!(user_definition.contains("address: Address;"));
    assert!(user_definition.contains("backup_addresses: Array<Address>;"));

    // Verify Address definition exists (without Json suffix in export type)
    assert!(address_definition.contains("export type Address = {"));
    assert!(address_definition.contains("street: string;"));
    assert!(address_definition.contains("city: string;"));
    assert!(address_definition.contains("zip_code: string;"));

    // Check Zod schema references (without Json suffix) - now in separate method
    let user_zod_schema = UserWithAddress::zod_schema();
    assert!(user_zod_schema.contains("address: Address$Schema"));
    assert!(user_zod_schema.contains("backup_addresses: z.array(Address$Schema)"));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_original_bug_reproduction_json_schema() {
    let schema = OriginalBugReproduction::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    // The original problematic case
    assert_eq!(properties["problematic_map"]["type"], "object");
    assert_eq!(
        properties["problematic_map"]["additionalProperties"]["type"],
        "array"
    );
    assert_eq!(
        properties["problematic_map"]["additionalProperties"]["items"]["type"],
        "integer"
    );

    // Similar cases
    assert_eq!(
        properties["string_to_vec_i64"]["additionalProperties"]["type"],
        "array"
    );
    assert_eq!(
        properties["string_to_vec_i64"]["additionalProperties"]["items"]["type"],
        "integer"
    );

    assert_eq!(
        properties["string_to_vec_f64"]["additionalProperties"]["type"],
        "array"
    );
    assert_eq!(
        properties["string_to_vec_f64"]["additionalProperties"]["items"]["type"],
        "number"
    );

    assert_eq!(
        properties["string_to_vec_bool"]["additionalProperties"]["type"],
        "array"
    );
    assert_eq!(
        properties["string_to_vec_bool"]["additionalProperties"]["items"]["type"],
        "boolean"
    );

    assert_eq!(
        properties["string_to_vec_string"]["additionalProperties"]["type"],
        "array"
    );
    assert_eq!(
        properties["string_to_vec_string"]["additionalProperties"]["items"]["type"],
        "string"
    );
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_original_bug_reproduction_typescript() {
    let ts_definition = OriginalBugReproduction::ts_definition();

    // TypeScript should use Array<T> syntax for the HashMap values
    assert!(ts_definition.contains("problematic_map: Partial<Record<string, Array<number>>>;"));
    assert!(ts_definition.contains("string_to_vec_i64: Partial<Record<string, Array<number>>>;"));
    assert!(ts_definition.contains("string_to_vec_f64: Partial<Record<string, Array<number>>>;"));
    assert!(ts_definition.contains("string_to_vec_bool: Partial<Record<string, Array<boolean>>>;"));
    assert!(
        ts_definition.contains("string_to_vec_string: Partial<Record<string, Array<string>>>;")
    );

    // Zod schemas should use z.array(...) for the HashMap values - now in separate method
    let zod_schema = OriginalBugReproduction::zod_schema();
    assert!(
        zod_schema.contains("problematic_map: z.record(z.string(), z.array(z.number().int()))")
    );
    assert!(
        zod_schema.contains("string_to_vec_i64: z.record(z.string(), z.array(z.number().int()))")
    );
    assert!(zod_schema.contains("string_to_vec_f64: z.record(z.string(), z.array(z.number()))"));
    assert!(zod_schema.contains("string_to_vec_bool: z.record(z.string(), z.array(z.boolean()))"));
    assert!(zod_schema.contains("string_to_vec_string: z.record(z.string(), z.array(z.string()))"));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_complex_nested_maps_json_schema() {
    let schema = SimpleComplexTest::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    // Test the triple-nested structure: HashMap<String, Vec<HashMap<String, u64>>>
    // Expected: object -> array -> object -> integer
    assert_eq!(properties["nested_map_of_arrays"]["type"], "object");

    let additional_props = properties["nested_map_of_arrays"]["additionalProperties"]
        .as_object()
        .unwrap();
    assert_eq!(additional_props["type"], "array");

    let items = additional_props["items"].as_object().unwrap();
    assert_eq!(items["type"], "object");

    let inner_additional_props = items["additionalProperties"].as_object().unwrap();
    assert_eq!(inner_additional_props["type"], "integer");
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_complex_nested_maps_typescript() {
    let ts_definition = SimpleComplexTest::ts_definition();

    // TypeScript should use the correct nested structure
    assert!(ts_definition.contains(
        "nested_map_of_arrays: Partial<Record<string, Array<Partial<Record<string, number>>>>>;"
    ));

    // Zod schema should use the correct nested structure - now in separate method
    let zod_schema = SimpleComplexTest::zod_schema();
    assert!(zod_schema.contains("nested_map_of_arrays: z.record(z.string(), z.array(z.record(z.string(), z.number().int()))),"));
}

#[test]
#[cfg(all(feature = "jsonschema", feature = "typescript"))]
fn test_quadruple_nested_maps_compilation() {
    // If this compiles without panic, it's a huge success!
    let schema = ReallyComplexTest::json_schema();
    let ts_definition = ReallyComplexTest::ts_definition();

    // Check that the schema contains our fields
    let properties = schema["properties"].as_object().unwrap();
    assert!(properties.contains_key("quadruple_nested"));
    assert!(properties.contains_key("optional_nested"));

    // Basic structure checks
    assert_eq!(properties["quadruple_nested"]["type"], "object");

    // Check TypeScript contains our fields (exact types may be complex)
    assert!(ts_definition.contains("quadruple_nested"));
    assert!(ts_definition.contains("optional_nested"));
}

#[test]
#[cfg(all(feature = "jsonschema", feature = "typescript", feature = "serde"))]
fn test_quadruple_nested_maps_compilation_serde() {
    // If this compiles without panic, it's a huge success!
    let schema = ReallyComplexTest::json_schema();
    let ts_definition = ReallyComplexTest::ts_definition();

    // Check that the schema contains our fields
    let properties = schema["properties"].as_object().unwrap();
    assert!(properties.contains_key("quadruple_nested"));
    assert!(properties.contains_key("optional_nested"));

    // Basic structure checks
    assert_eq!(properties["quadruple_nested"]["type"], "object");

    // Check TypeScript contains our fields (exact types may be complex)
    assert!(ts_definition.contains("quadruple_nested"));
    assert!(ts_definition.contains("optional_nested"));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_nested_string_keyed_maps_json_schema() {
    let schema = NestedStringKeyedMaps::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    for (field_name, expected_value_schema) in [
        (
            "counts_by_group",
            serde_json::json!({
                "type": "object",
                "additionalProperties": { "type": "integer" }
            }),
        ),
        (
            "labels_by_group",
            serde_json::json!({
                "type": "object",
                "additionalProperties": { "type": "string" }
            }),
        ),
        (
            "rows_by_group",
            serde_json::json!({
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                }
            }),
        ),
        (
            "scores_by_group",
            serde_json::json!({
                "type": "object",
                "additionalProperties": {
                    "anyOf": [{ "type": "number" }, { "type": "null" }]
                }
            }),
        ),
        (
            "tallies_by_region",
            serde_json::json!({
                "type": "object",
                "additionalProperties": {
                    "type": "object",
                    "additionalProperties": { "type": "integer" }
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

/// TypeScript and Zod recurse through a map value on their own, so the nesting they render is the
/// one the JSON schema now describes — pinned here so the three surfaces stay in step.
#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_nested_string_keyed_maps_typescript_and_zod() {
    let ts_definition = NestedStringKeyedMaps::ts_definition();
    for expected in [
        "counts_by_group: Partial<Record<string, Partial<Record<string, number>>>>;",
        "labels_by_group: Partial<Record<string, Partial<Record<string, string>>>>;",
        "rows_by_group: Partial<Record<string, Array<Partial<Record<string, string>>>>>;",
        "scores_by_group: Partial<Record<string, Partial<Record<string, number | null>>>>;",
        "tallies_by_region: Partial<Record<string, Partial<Record<string, Partial<Record<string, number>>>>>>;",
    ] {
        assert!(
            ts_definition.contains(expected),
            "missing {expected}, got: {ts_definition}"
        );
    }

    let zod_schema = NestedStringKeyedMaps::zod_schema();
    for expected in [
        "counts_by_group: z.record(z.string(), z.record(z.string(), z.number().int())),",
        "labels_by_group: z.record(z.string(), z.record(z.string(), z.string())),",
        "rows_by_group: z.record(z.string(), z.array(z.record(z.string(), z.string()))),",
        "scores_by_group: z.record(z.string(), z.record(z.string(), z.nullable(z.number()))),",
        "tallies_by_region: z.record(z.string(), z.record(z.string(), z.record(z.string(), z.number().int()))),",
    ] {
        assert!(
            zod_schema.contains(expected),
            "missing {expected}, got: {zod_schema}"
        );
    }
}
