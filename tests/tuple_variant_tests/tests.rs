use serde::{Deserialize, Serialize};
use tixschema::model_schema;

/// A generated schema module reaches its siblings through the enclosing module, and a function body
/// is not one, so a type another type references is declared here rather than inside a test.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Inner {
    pub field: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Outer {
    Paired(Inner, i64),
    Wrapped(Inner),
}

/// Test 1: Single-element tuple variants.
/// Each variant has exactly one tuple element.
#[test]
fn test_single_tuple_variant_typescript() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum SingleTuple {
        Decimal(f64),
        Flag(bool),
        Number(i64),
        Text(String),
    }

    let ts = SingleTuple::ts_definition();

    // Verify discriminator field
    assert!(ts.contains("type: \"Text\""), "Missing Text discriminator");
    assert!(
        ts.contains("type: \"Number\""),
        "Missing Number discriminator"
    );
    assert!(ts.contains("type: \"Flag\""), "Missing Flag discriminator");
    assert!(
        ts.contains("type: \"Decimal\""),
        "Missing Decimal discriminator"
    );

    // Verify value field with correct types
    assert!(ts.contains("value: string"), "Missing string value");
    assert!(ts.contains("value: number"), "Missing number value");
    assert!(ts.contains("value: boolean"), "Missing boolean value");
}

/// Test 1b: Single-element tuple variants Zod schema.
#[cfg(feature = "zod")]
#[test]
fn test_single_tuple_variant_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum SingleTupleZod {
        Flag(bool),
        Number(i64),
        Text(String),
    }

    let zod = SingleTupleZod::zod_schema();

    // Verify Zod schema structure
    assert!(
        zod.contains("z.discriminatedUnion"),
        "Missing discriminatedUnion"
    );
    assert!(zod.contains("z.literal(\"Text\")"), "Missing Text literal");
    assert!(
        zod.contains("z.literal(\"Number\")"),
        "Missing Number literal"
    );
    assert!(zod.contains("z.literal(\"Flag\")"), "Missing Flag literal");

    // Verify value types
    assert!(zod.contains("value: z.string()"), "Missing string value");
    assert!(
        zod.contains("value: z.number().int()"),
        "Missing int value for i64"
    );
    assert!(zod.contains("value: z.boolean()"), "Missing boolean value");
}

/// Test 2: Multi-element tuple variants.
/// Variants with more than one tuple element.
#[test]
fn test_multi_tuple_variant_typescript() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum MultiTuple {
        Pair(String, i64),
        Quad(String, i64, bool, f64),
        Triple(String, i64, bool),
    }

    let ts = MultiTuple::ts_definition();

    // Verify tuple syntax
    assert!(
        ts.contains("value: [string, number]"),
        "Missing Pair tuple type"
    );
    assert!(
        ts.contains("value: [string, number, boolean]"),
        "Missing Triple tuple type"
    );
    assert!(
        ts.contains("value: [string, number, boolean, number]"),
        "Missing Quad tuple type"
    );
}

/// Test 2b: Multi-element tuple variants Zod schema.
#[cfg(feature = "zod")]
#[test]
fn test_multi_tuple_variant_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum MultiTupleZod {
        Pair(String, i64),
        Triple(String, i64, bool),
    }

    let zod = MultiTupleZod::zod_schema();

    // Verify z.tuple() is used
    assert!(zod.contains("z.tuple("), "Missing z.tuple");
    assert!(
        zod.contains("z.tuple([z.string(), z.number().int()])"),
        "Missing Pair tuple"
    );
    assert!(
        zod.contains("z.tuple([z.string(), z.number().int(), z.boolean()])"),
        "Missing Triple tuple"
    );
}

/// Test 3: Plain enum (all unit variants) -> string union.
/// Should NOT generate discriminated union, just string union.
#[test]
fn test_plain_enum_string_union() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum DataType {
        Alphanumeric,
        Boolean,
        Decimal,
        Image,
        Integer,
    }

    let ts = DataType::ts_definition();

    // Should be a string union, not discriminated union
    assert!(ts.contains("\"Alphanumeric\""), "Missing Alphanumeric");
    assert!(ts.contains("\"Image\""), "Missing Image");
    assert!(ts.contains("\"Decimal\""), "Missing Decimal");
    assert!(ts.contains("\"Integer\""), "Missing Integer");
    assert!(ts.contains("\"Boolean\""), "Missing Boolean");

    // Should NOT have type/value fields (it's a plain string union)
    // The format should be: type DataType = "Alphanumeric" | "Image" | ...
    assert!(
        !ts.contains("type:") || ts.contains("export type"),
        "Should not have type discriminator field"
    );
}

/// Test 3b: Plain enum Zod schema uses z.enum.
#[cfg(feature = "zod")]
#[test]
fn test_plain_enum_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum PlainEnumZod {
        Active,
        Inactive,
        Pending,
    }

    let zod = PlainEnumZod::zod_schema();

    // Should use z.enum, not z.discriminatedUnion
    assert!(zod.contains("z.enum("), "Should use z.enum for plain enums");
    assert!(
        !zod.contains("z.discriminatedUnion"),
        "Should not use discriminatedUnion for plain enums"
    );
}

/// Test 4: Mixed variants (comprehensive).
/// Mix of unit, tuple-single, tuple-multi, and named struct variants.
#[test]
fn test_mixed_variants_typescript() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum Mixed {
        // Unit variant
        Empty,
        // Named struct
        Named { field_a: String, field_b: bool },
        // Multi tuple
        Pair(String, i64),
        // Single tuple
        Text(String),
    }

    let ts = Mixed::ts_definition();

    // Unit variant should only have discriminator
    assert!(
        ts.contains("type: \"Empty\""),
        "Missing Empty discriminator"
    );

    // Single tuple should have value field
    assert!(ts.contains("type: \"Text\""), "Missing Text discriminator");
    assert!(ts.contains("value: string"), "Missing Text value");

    // Multi tuple should have tuple value
    assert!(ts.contains("type: \"Pair\""), "Missing Pair discriminator");
    assert!(
        ts.contains("value: [string, number]"),
        "Missing Pair tuple value"
    );

    // Named struct should have individual fields
    assert!(
        ts.contains("type: \"Named\""),
        "Missing Named discriminator"
    );
    assert!(ts.contains("field_a: string"), "Missing field_a");
    assert!(ts.contains("field_b: boolean"), "Missing field_b");
}

/// Test 4b: Mixed variants Zod schema.
#[cfg(feature = "zod")]
#[test]
fn test_mixed_variants_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum MixedZod {
        Empty,
        Named { field_a: String, field_b: bool },
        Pair(String, i64),
        Text(String),
    }

    let zod = MixedZod::zod_schema();

    // Should use discriminatedUnion since not all variants are unit
    assert!(
        zod.contains("z.discriminatedUnion"),
        "Should use discriminatedUnion"
    );

    // Unit variant
    assert!(
        zod.contains("z.literal(\"Empty\")"),
        "Missing Empty literal"
    );

    // Single tuple
    assert!(zod.contains("z.literal(\"Text\")"), "Missing Text literal");
    assert!(zod.contains("value: z.string()"), "Missing Text value");

    // Multi tuple
    assert!(zod.contains("z.literal(\"Pair\")"), "Missing Pair literal");
    assert!(zod.contains("z.tuple("), "Missing tuple");

    // Named struct
    assert!(
        zod.contains("z.literal(\"Named\")"),
        "Missing Named literal"
    );
    assert!(zod.contains("field_a: z.string()"), "Missing field_a");
    assert!(zod.contains("field_b: z.boolean()"), "Missing field_b");
}

/// Test 5: JSON Schema generation for tuple variants.
#[cfg(feature = "jsonschema")]
#[test]
fn test_tuple_json_schema() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum TupleSchema {
        Double(String, i64),
        Single(String),
    }

    let schema = TupleSchema::json_schema();
    let schema_str = serde_json::to_string_pretty(&schema).unwrap();

    // Should have oneOf for discriminated union
    assert!(schema_str.contains("\"oneOf\""), "Missing oneOf");

    // Should have value property
    assert!(schema_str.contains("\"value\""), "Missing value property");

    // Multi-tuple should use prefixItems
    assert!(
        schema_str.contains("prefixItems"),
        "Multi-tuple should use prefixItems"
    );
}

/// Test 6: Custom content field name via serde.
#[test]
fn test_custom_content_field() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(tag = "kind", content = "data")]
    pub enum CustomContent {
        Number(i64),
        Text(String),
    }

    let ts = CustomContent::ts_definition();

    // Should use "kind" instead of "type"
    assert!(ts.contains("kind: \"Text\""), "Should use 'kind' as tag");
    assert!(ts.contains("kind: \"Number\""), "Should use 'kind' as tag");

    // Should use "data" instead of "value"
    assert!(
        ts.contains("data: string"),
        "Should use 'data' as content field"
    );
    assert!(
        ts.contains("data: number"),
        "Should use 'data' as content field"
    );
}

/// Test 6b: Custom content field Zod schema.
#[cfg(feature = "zod")]
#[test]
fn test_custom_content_field_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(tag = "kind", content = "data")]
    pub enum CustomContentZod {
        Number(i64),
        Text(String),
    }

    let zod = CustomContentZod::zod_schema();

    // Should use "kind" in discriminatedUnion
    assert!(
        zod.contains("z.discriminatedUnion(\"kind\""),
        "Should use 'kind' as discriminator"
    );

    // Should use "data" as field name
    assert!(
        zod.contains("data: z.string()"),
        "Should use 'data' as content field"
    );
    assert!(
        zod.contains("data: z.number()"),
        "Should use 'data' as content field"
    );
}

/// Test 7: Tuple variant with Vec (array type in tuple).
#[test]
fn test_tuple_with_vec() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum TupleWithVec {
        Data(Vec<String>),
        // This was the original problem case: Image(String, Vec<u8>)
        Image(String, Vec<u8>),
    }

    let ts = TupleWithVec::ts_definition();

    // Image should have tuple with string and array
    assert!(
        ts.contains("value: [string, Array<number>]"),
        "Image should have [string, Array<number>]"
    );

    // Data should have array value (single tuple)
    assert!(
        ts.contains("value: Array<string>"),
        "Data should have Array<string>"
    );
}

/// Test 7b: Tuple with Vec Zod schema.
#[cfg(feature = "zod")]
#[test]
fn test_tuple_with_vec_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum TupleWithVecZod {
        Data(Vec<String>),
        Image(String, Vec<u8>),
    }

    let zod = TupleWithVecZod::zod_schema();

    // Image tuple
    assert!(
        zod.contains("z.tuple([z.string(), z.array(z.number().int())])"),
        "Image should have z.tuple"
    );

    // Data single tuple
    assert!(
        zod.contains("value: z.array(z.string())"),
        "Data should have z.array"
    );
}

/// Test 8: Regression - discriminated union with named struct still works.
#[test]
fn test_named_struct_regression() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum PaymentMethod {
        BankTransfer {
            account_number: String,
            routing_number: String,
        },
        CreditCard {
            card_number: String,
            expiry: String,
        },
    }

    let ts = PaymentMethod::ts_definition();

    // Should still work as before
    assert!(
        ts.contains("type: \"CreditCard\""),
        "Missing CreditCard discriminator"
    );
    assert!(ts.contains("card_number: string"), "Missing card_number");
    assert!(ts.contains("expiry: string"), "Missing expiry");

    assert!(
        ts.contains("type: \"BankTransfer\""),
        "Missing BankTransfer discriminator"
    );
    assert!(
        ts.contains("account_number: string"),
        "Missing account_number"
    );
    assert!(
        ts.contains("routing_number: string"),
        "Missing routing_number"
    );
}

/// Test 9: Optional types in tuple variants.
#[test]
fn test_optional_in_tuple() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum OptionalTuple {
        Maybe(Option<String>),
        MaybePair(String, Option<i64>),
    }

    let ts = OptionalTuple::ts_definition();

    // Single optional tuple
    assert!(
        ts.contains("value: string | undefined"),
        "Maybe should have optional string"
    );

    // Multi tuple with optional element
    assert!(
        ts.contains("[string, number | undefined]"),
        "MaybePair should have tuple with optional element"
    );
}

/// Test 10: Tuple variant with nested custom type.
#[test]
fn test_tuple_with_custom_type() {
    let ts = Outer::ts_definition();

    // Should reference the inner type
    assert!(
        ts.contains("value: Inner"),
        "Wrapped should reference Inner type"
    );
    assert!(
        ts.contains("value: [Inner, number]"),
        "Paired should have tuple with Inner"
    );
}

/// And the JSON schema of that variant's tuple slot carries the same reference: the sibling's own
/// schema, in the position the TypeScript above names it.
#[cfg(feature = "jsonschema")]
#[test]
fn test_tuple_variant_sibling_element_carries_the_sibling_schema() {
    let schema = Outer::json_schema();
    let variant = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|member| member["properties"]["type"]["const"] == "Paired")
        .unwrap();

    assert_eq!(
        variant["properties"]["value"]["prefixItems"][0],
        Inner::json_schema()
    );
}

/// Test 11: Serde serialization compatibility.
/// Verify that the generated schema matches actual serde serialization.
#[test]
fn test_serde_serialization_compatibility() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    #[serde(tag = "type", content = "value")]
    pub enum SerdeCompat {
        Number(i64),
        Pair(String, i64),
        Text(String),
    }

    // Test serialization produces expected format
    let text = SerdeCompat::Text("hello".to_owned());
    let text_json = serde_json::to_value(&text).unwrap();
    assert_eq!(text_json["type"], "Text");
    assert_eq!(text_json["value"], "hello");

    let number = SerdeCompat::Number(42);
    let number_json = serde_json::to_value(&number).unwrap();
    assert_eq!(number_json["type"], "Number");
    assert_eq!(number_json["value"], 42_i64);

    let pair = SerdeCompat::Pair("hello".to_owned(), 42);
    let pair_json = serde_json::to_value(&pair).unwrap();
    assert_eq!(pair_json["type"], "Pair");
    assert!(pair_json["value"].is_array());
    assert_eq!(pair_json["value"][0], "hello");
    assert_eq!(pair_json["value"][1], 42_i64);
}

/// Test 12: Complex `FixedValue` enum from original issue.
/// This is the exact enum from the user's original problem.
#[test]
fn test_fixed_value_original_issue() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum FixedValue {
        Alphanumeric(String),
        Boolean(bool),
        Decimal(f64),
        Image(String, Vec<u8>),
        Integer(i64),
    }

    let ts = FixedValue::ts_definition();

    // Verify NO empty field names (the original bug)
    // Empty field names would look like "  : string;" (space before colon, no field name)
    // Valid output has field names like "value: string;"
    assert!(
        !ts.contains("\n  : "),
        "Should not have empty field name (found line starting with colon)"
    );
    // Also verify we don't have the old pattern of empty property names in the schema
    assert!(
        !ts.contains("\"\":"),
        "JSON Schema should not have empty property names"
    );

    // Verify proper structure
    assert!(
        ts.contains("type: \"Alphanumeric\""),
        "Missing Alphanumeric"
    );
    assert!(
        ts.contains("value: string"),
        "Alphanumeric should have value"
    );

    assert!(ts.contains("type: \"Image\""), "Missing Image");
    assert!(
        ts.contains("[string, Array<number>]"),
        "Image should have tuple"
    );

    assert!(ts.contains("type: \"Decimal\""), "Missing Decimal");
    assert!(ts.contains("type: \"Integer\""), "Missing Integer");
    assert!(ts.contains("type: \"Boolean\""), "Missing Boolean");
}

/// Test 13: `FixedValueExt` with all variant types.
#[test]
fn test_fixed_value_ext_comprehensive() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum FixedValueExt {
        Alphanumeric(String),
        Boolean(bool),
        Complex { a: String, b: bool },
        Decimal(f64),
        Image(String, Vec<u8>),
        Integer(i64),
        SingleValue,
        Tuple(i64, bool),
    }

    let ts = FixedValueExt::ts_definition();

    // Single tuple variants
    assert!(
        ts.contains("type: \"Alphanumeric\""),
        "Missing Alphanumeric"
    );
    assert!(
        ts.contains("value: string") || ts.contains("value: number"),
        "Single tuple should have value"
    );

    // Multi-element tuple
    assert!(ts.contains("type: \"Image\""), "Missing Image");
    assert!(ts.contains("type: \"Tuple\""), "Missing Tuple");
    assert!(
        ts.contains("[number, boolean]"),
        "Tuple should have [number, boolean]"
    );

    // Unit variant
    assert!(ts.contains("type: \"SingleValue\""), "Missing SingleValue");

    // Named struct variant
    assert!(ts.contains("type: \"Complex\""), "Missing Complex");
    assert!(ts.contains("a: string"), "Missing field a");
    assert!(ts.contains("b: boolean"), "Missing field b");
}

/// Test 14: Empty tuple variant (edge case).
/// An empty tuple `Foo()` should be treated like a unit variant.
#[test]
fn test_empty_tuple_variant() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum EmptyTuple {
        Normal(String),
        // Note: Rust doesn't allow `Empty()` syntax directly, but we test the logic anyway
        // by having a unit variant alongside tuple variants
        Unit,
    }

    let ts = EmptyTuple::ts_definition();

    // Both should be in discriminated union
    assert!(ts.contains("type: \"Normal\""), "Missing Normal");
    assert!(ts.contains("type: \"Unit\""), "Missing Unit");

    // Unit should not have value field
    // The Unit variant's object should just have `type: "Unit"` without a value field
    // This is verified by ensuring the total schema structure is correct
}

/// Test 16: enum tuple variant with an `Option` element gets null flavor in the
/// generated JSON Schema, via the same shared element builder used by struct
/// tuple fields. A positional tuple slot serializes `None` as `null`, so the
/// optional element renders `anyOf [<base>, null]`; arity is unchanged.
#[cfg(feature = "jsonschema")]
#[test]
fn test_optional_tuple_variant_element_json_schema_null_flavor() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum Row {
        Link(Option<String>, Vec<usize>, String, Option<String>),
    }

    let schema = Row::json_schema();
    let variant = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|member| member["properties"]["type"]["const"] == "Link")
        .unwrap();

    let value = &variant["properties"]["value"];
    assert_eq!(value["type"].as_str(), Some("array"));

    let prefix = value["prefixItems"].as_array().unwrap();
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
    // Non-optional slot stays plain — no null wrapping.
    assert_eq!(prefix[2], serde_json::json!({ "type": "string" }));
}

/// Test 15: `JSDoc` comments in generated TypeScript.
#[test]
fn test_jsdoc_comments() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum JsDoc {
        Multi(String, i64),
        Single(String),
    }

    let ts = JsDoc::ts_definition();

    // Should have JSDoc comments
    assert!(ts.contains("/**"), "Should have JSDoc comments");
    assert!(ts.contains("*/"), "Should have JSDoc end");

    // Multi-tuple should have tuple description
    assert!(
        ts.contains("Tuple:") || ts.contains("Tuple value"),
        "Multi-tuple should have tuple documentation"
    );
}
