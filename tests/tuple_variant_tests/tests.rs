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

/// The content key of a single-element tuple variant is a slot: serde always writes the key, so a
/// `None` at the outermost level reaches the wire as a `null` under it rather than dropping the
/// key. The three variants below are the three answers the surfaces owe — the `Option` around the
/// array, the `Option` among its items, and no `Option` at all.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "value")]
pub enum SlotContent {
    Covered(Option<Vec<u32>>),
    Element(Vec<Option<u32>>),
    Plain(String),
}

/// Every position of a multi-element tuple variant is a slot for the same reason: serde writes each
/// one, so a `None` among them reaches the wire as a `null` in place rather than shortening the
/// tuple. The non-`Option` element beside it carries no null.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "value")]
pub enum SlotElements {
    Pair(String, Option<u32>),
}

/// An enum carrying no `#[serde(tag = ..., content = ...)]` is externally tagged: serde writes the
/// variant name as the sole key of an object holding the content, and a unit variant as the bare
/// name. The four variants below are the four contents that key can hold, plus the one that has no
/// key at all.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum External {
    Bare,
    Fields { a: String, b: bool },
    Pair(u32, u32),
    Single(String),
}

/// The same variants written with the tagging attributes, which keeps the adjacent form beside the
/// external one so the two can be read against each other.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type", content = "value")]
pub enum Adjacent {
    Bare,
    Fields { a: String, b: bool },
    Pair(u32, u32),
    Single(String),
}

/// A variant's key is its wire name, and a rename can spell that as something no JavaScript
/// identifier can hold.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RenamedExternal {
    BigThing(u32),
    #[serde(rename = "application/pdf")]
    Mime(String),
    UnitThing,
}

/// The type an internally tagged newtype variant wraps. Its fields are what serde writes beside
/// the tag, so they are the ones the surfaces owe.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TagPayload {
    pub a: String,
    pub b: bool,
}

/// An enum carrying `#[serde(tag = ...)]` and no `content` is internally tagged: there is no key
/// for a variant's data, so what it writes are members of the object the tag is written in. The
/// three variants are the three things that can be written there — nothing at all, a struct
/// variant's own fields, and the members of a newtype variant's inner type.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum Internal {
    Bare,
    Fields { a: String, b: bool },
    Wrapped(TagPayload),
}

/// The same form with no newtype variant, which is every member a `z.discriminatedUnion` can hold.
/// Only the Zod surface names that union, so the fixture is declared where it is read.
#[cfg(feature = "zod")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum InternalNamedOnly {
    Fields { a: String },
}

/// The shape the crate refuses to describe, declared without `#[model_schema()]` so serde's own
/// answer for it can be read off the wire.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum InternalScalar {
    Single(String),
}

/// A plain enum, and a bare tag over it. Also declared without `#[model_schema()]`: a name says
/// nothing about what serde writes for it, and this one writes no object, so the crate refuses the
/// declaration — leaving the wire form readable only from a plain serde type.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum InternalHue {
    Red,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum InternalOverEnum {
    EnumInner(InternalHue),
}

/// A newtype over a `String`: serde writes it as the string it wraps, and the registry cannot tell
/// it apart from a struct — both register as having no enum members. So the declaration compiles
/// and the divergence is left for the merge to catch.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct InternalSlug(pub String);

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum InternalOverBrand {
    Branded(InternalSlug),
}

/// An untagged enum every member of which serde writes as an object, wrapped by a bare tag. The
/// content joins the tag the same way a `#[serde(flatten)]` base does, so what lands beside the tag
/// is the members of whichever union member matched.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct InternalFirst {
    pub a: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct InternalSecond {
    pub b: bool,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum InternalEither {
    First(InternalFirst),
    Second(InternalSecond),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum InternalOverUntagged {
    Wrapped(InternalEither),
}

/// The same wrapping over a union with one member serde writes as a string.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum InternalScalarEither {
    Obj(InternalFirst),
    Text(String),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum InternalOverScalarUntagged {
    Wrapped(InternalScalarEither),
}

/// A recursive enum with no tagging attributes: what sits under a key is the enum itself. Only the
/// Zod surface spells the deferral, so the fixture is declared where it is read.
#[cfg(feature = "zod")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RecursiveExternal {
    Arr(Vec<Self>),
    Txt(String),
}

/// The `oneOf` member an externally tagged variant renders as: the one whose sole required key is
/// the variant's name.
#[cfg(feature = "jsonschema")]
fn external_member(schema: &serde_json::Value, variant: &str) -> serde_json::Value {
    let member = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|member| member["required"] == serde_json::json!([variant]))
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    assert!(
        !member.is_null(),
        "No member keyed `{variant}`. Got: {schema}"
    );
    member
}

/// The JSON type name a value carries, as a schema spells it.
#[cfg(feature = "jsonschema")]
const fn json_type_name(value: &serde_json::Value) -> &'static str {
    match *value {
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Null => "null",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::Object(_) => "object",
        serde_json::Value::String(_) => "string",
    }
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

    // The variant name is the key and its content sits under it, unwrapped.
    assert!(ts.contains("\"Text\": string"), "Missing Text. Got: {ts}");
    assert!(
        ts.contains("\"Number\": number"),
        "Missing Number. Got: {ts}"
    );
    assert!(ts.contains("\"Flag\": boolean"), "Missing Flag. Got: {ts}");
    assert!(
        ts.contains("\"Decimal\": number"),
        "Missing Decimal. Got: {ts}"
    );
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

    // The key carries the discriminator, so there is no one field every member shares for
    // `z.discriminatedUnion` to switch on.
    assert!(zod.contains("z.union(["), "Missing union. Got: {zod}");
    assert!(
        !zod.contains("z.discriminatedUnion"),
        "No shared field to discriminate on. Got: {zod}"
    );

    assert!(
        zod.contains("\"Text\": z.string()"),
        "Missing Text. Got: {zod}"
    );
    assert!(
        zod.contains("\"Number\": z.number().int()"),
        "Missing Number. Got: {zod}"
    );
    assert!(
        zod.contains("\"Flag\": z.boolean()"),
        "Missing Flag. Got: {zod}"
    );
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

    // The elements are the array under the variant's key.
    assert!(
        ts.contains("\"Pair\": [string, number]"),
        "Missing Pair tuple type. Got: {ts}"
    );
    assert!(
        ts.contains("\"Triple\": [string, number, boolean]"),
        "Missing Triple tuple type. Got: {ts}"
    );
    assert!(
        ts.contains("\"Quad\": [string, number, boolean, number]"),
        "Missing Quad tuple type. Got: {ts}"
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

    // Unit variant is the bare name: serde writes no key for it.
    assert!(ts.contains("\"Empty\""), "Missing Empty. Got: {ts}");
    assert!(
        !ts.contains("\"Empty\":"),
        "A unit variant carries no key. Got: {ts}"
    );

    assert!(ts.contains("\"Text\": string"), "Missing Text. Got: {ts}");

    assert!(
        ts.contains("\"Pair\": [string, number]"),
        "Missing Pair tuple. Got: {ts}"
    );

    // A struct variant's fields sit in an object under its key.
    assert!(ts.contains("\"Named\": {"), "Missing Named. Got: {ts}");
    assert!(ts.contains("field_a: string"), "Missing field_a. Got: {ts}");
    assert!(
        ts.contains("field_b: boolean"),
        "Missing field_b. Got: {ts}"
    );
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

    assert!(zod.contains("z.union(["), "Missing union. Got: {zod}");

    // A unit variant is the bare name, so it is a literal rather than an object.
    assert!(
        zod.contains("z.literal(\"Empty\")"),
        "Missing Empty literal. Got: {zod}"
    );

    assert!(
        zod.contains("\"Text\": z.string()"),
        "Missing Text. Got: {zod}"
    );

    assert!(
        zod.contains("\"Pair\": z.tuple("),
        "Missing Pair tuple. Got: {zod}"
    );

    assert!(
        zod.contains("\"Named\": z.strictObject("),
        "Missing Named object. Got: {zod}"
    );
    assert!(
        zod.contains("field_a: z.string()"),
        "Missing field_a. Got: {zod}"
    );
    assert!(
        zod.contains("field_b: z.boolean()"),
        "Missing field_b. Got: {zod}"
    );
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

    assert!(schema_str.contains("\"oneOf\""), "Missing oneOf");

    // The variant name is the property; there is no content key beside a tag.
    assert!(
        !schema_str.contains("\"value\""),
        "Externally tagged members carry no content key. Got: {schema_str}"
    );
    assert_eq!(
        external_member(&schema, "Double")["properties"]["Double"]["prefixItems"]
            .as_array()
            .unwrap()
            .len(),
        2,
        "Multi-tuple should use prefixItems. Got: {schema_str}"
    );
    assert_eq!(
        external_member(&schema, "Single")["properties"]["Single"],
        serde_json::json!({ "type": "string" })
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

    assert!(
        ts.contains("\"Image\": [string, Array<number>]"),
        "Image should have [string, Array<number>]. Got: {ts}"
    );

    assert!(
        ts.contains("\"Data\": Array<string>"),
        "Data should have Array<string>. Got: {ts}"
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

    assert!(
        zod.contains("\"Image\": z.tuple([z.string(), z.array(z.number().int())])"),
        "Image should have z.tuple. Got: {zod}"
    );

    assert!(
        zod.contains("\"Data\": z.array(z.string())"),
        "Data should have z.array. Got: {zod}"
    );
}

/// Test 8: a struct variant's fields sit in an object under the variant's key.
#[test]
fn test_named_struct_variant_content_is_an_object_under_the_key() {
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

    assert!(
        ts.contains("\"CreditCard\": {"),
        "Missing CreditCard. Got: {ts}"
    );
    assert!(
        ts.contains("card_number: string"),
        "Missing card_number. Got: {ts}"
    );
    assert!(ts.contains("expiry: string"), "Missing expiry. Got: {ts}");

    assert!(
        ts.contains("\"BankTransfer\": {"),
        "Missing BankTransfer. Got: {ts}"
    );
    assert!(
        ts.contains("account_number: string"),
        "Missing account_number. Got: {ts}"
    );
    assert!(
        ts.contains("routing_number: string"),
        "Missing routing_number. Got: {ts}"
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

    // The variant's key is a slot, so the `None` is a `null` under it.
    assert!(
        ts.contains("\"Maybe\": string | null"),
        "Maybe should have nullable string. Got: {ts}"
    );

    // Multi tuple with optional element: each position is a slot too, so the same null flavor.
    assert!(
        ts.contains("\"MaybePair\": [string, number | null]"),
        "MaybePair should have tuple with optional element. Got: {ts}"
    );
}

/// Test 10: Tuple variant with nested custom type.
#[test]
fn test_tuple_with_custom_type() {
    let ts = Outer::ts_definition();

    // Should reference the inner type
    assert!(
        ts.contains("\"Wrapped\": Inner"),
        "Wrapped should reference Inner type. Got: {ts}"
    );
    assert!(
        ts.contains("\"Paired\": [Inner, number]"),
        "Paired should have tuple with Inner. Got: {ts}"
    );
}

/// And the JSON schema of that variant's tuple slot carries the same reference: the sibling's own
/// schema, in the position the TypeScript above names it.
#[cfg(feature = "jsonschema")]
#[test]
fn test_tuple_variant_sibling_element_carries_the_sibling_schema() {
    let variant = external_member(&Outer::json_schema(), "Paired");

    assert_eq!(
        variant["properties"]["Paired"]["prefixItems"][0],
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
    assert!(
        !ts.contains("\n  : "),
        "Should not have empty field name (found line starting with colon)"
    );
    assert!(
        !ts.contains("\"\":"),
        "JSON Schema should not have empty property names"
    );

    assert!(
        ts.contains("\"Alphanumeric\": string"),
        "Missing Alphanumeric. Got: {ts}"
    );

    assert!(
        ts.contains("\"Image\": [string, Array<number>]"),
        "Image should have tuple. Got: {ts}"
    );

    assert!(
        ts.contains("\"Decimal\": number"),
        "Missing Decimal. Got: {ts}"
    );
    assert!(
        ts.contains("\"Integer\": number"),
        "Missing Integer. Got: {ts}"
    );
    assert!(
        ts.contains("\"Boolean\": boolean"),
        "Missing Boolean. Got: {ts}"
    );
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
        ts.contains("\"Alphanumeric\": string"),
        "Missing Alphanumeric. Got: {ts}"
    );

    // Multi-element tuple
    assert!(
        ts.contains("\"Image\": [string, Array<number>]"),
        "Missing Image. Got: {ts}"
    );
    assert!(
        ts.contains("\"Tuple\": [number, boolean]"),
        "Missing Tuple. Got: {ts}"
    );

    // Unit variant carries no key at all.
    assert!(
        ts.contains("\"SingleValue\""),
        "Missing SingleValue. Got: {ts}"
    );
    assert!(
        !ts.contains("\"SingleValue\":"),
        "A unit variant carries no key. Got: {ts}"
    );

    // Named struct variant
    assert!(ts.contains("\"Complex\": {"), "Missing Complex. Got: {ts}");
    assert!(ts.contains("a: string"), "Missing field a. Got: {ts}");
    assert!(ts.contains("b: boolean"), "Missing field b. Got: {ts}");
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

    assert!(
        ts.contains("\"Normal\": string"),
        "Missing Normal. Got: {ts}"
    );
    assert!(ts.contains("\"Unit\""), "Missing Unit. Got: {ts}");
    assert!(
        !ts.contains("\"Unit\":"),
        "A unit variant carries no key. Got: {ts}"
    );
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

    let variant = external_member(&Row::json_schema(), "Link");

    let value = &variant["properties"]["Link"];
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

    // Each member carries its variant's own docs above the key it is named by.
    assert!(
        ts.contains("* Multi\n"),
        "Multi should be documented. Got: {ts}"
    );
    assert!(
        ts.contains("\"Multi\": [string, number]"),
        "Missing Multi. Got: {ts}"
    );
    assert!(
        ts.contains("* Single\n"),
        "Single should be documented. Got: {ts}"
    );
    assert!(
        ts.contains("\"Single\": string"),
        "Missing Single. Got: {ts}"
    );
}

/// Test 17: what serde writes for a single-element tuple variant whose content is an `Option` —
/// the capture the three surfaces below are read against.
#[test]
fn test_single_tuple_variant_option_content_writes_null_under_the_key() {
    assert_eq!(
        serde_json::to_value(SlotContent::Covered(None)).unwrap(),
        serde_json::json!({ "type": "Covered", "value": null }),
        "A `None` content keeps the key and writes `null` under it"
    );
    assert_eq!(
        serde_json::to_value(SlotContent::Element(vec![None])).unwrap(),
        serde_json::json!({ "type": "Element", "value": [null] }),
        "A `None` among the items is a `null` among them"
    );
}

/// Test 17a: TypeScript describes that content key as the slot it is.
#[test]
fn test_single_tuple_variant_option_content_typescript_null_flavor() {
    let ts = SlotContent::ts_definition();

    assert!(
        ts.contains("value: Array<number> | null"),
        "Covered's content is the slot the `None` fills with `null`. Got: {ts}"
    );
    assert!(
        ts.contains("value: Array<number | null>"),
        "Element's `None` stays among the items. Got: {ts}"
    );
    assert!(
        ts.contains("value: string;"),
        "Plain's content carries no null. Got: {ts}"
    );
}

/// Test 17b: the Zod schema of that content key admits the `null` serde writes.
#[cfg(feature = "zod")]
#[test]
fn test_single_tuple_variant_option_content_zod_null_flavor() {
    let zod = SlotContent::zod_schema();

    assert!(
        zod.contains("value: z.nullable(z.array(z.number().int()))"),
        "Covered's content admits the `null`. Got: {zod}"
    );
    assert!(
        zod.contains("value: z.array(z.nullable(z.number().int()))"),
        "Element's `null` stays among the items. Got: {zod}"
    );
    assert!(
        zod.contains("value: z.string()"),
        "Plain's content carries no null. Got: {zod}"
    );
}

/// Test 17c: the JSON schema already said so, and keeps saying it unchanged.
#[cfg(feature = "jsonschema")]
#[test]
fn test_single_tuple_variant_option_content_json_schema_null_flavor() {
    let schema = SlotContent::json_schema();
    let variant_of = |discriminator: &str| {
        schema["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .find(|member| member["properties"]["type"]["const"] == discriminator)
            .unwrap()
            .clone()
    };

    let covered = variant_of("Covered");
    assert_eq!(
        covered["properties"]["value"],
        serde_json::json!({
            "anyOf": [
                { "type": "array", "items": { "type": "integer" } },
                { "type": "null" }
            ]
        })
    );
    assert!(
        covered["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::Value::String("value".to_owned())),
        "The key a `None` cannot drop stays required. Got: {}",
        covered["required"]
    );

    assert_eq!(
        variant_of("Element")["properties"]["value"],
        serde_json::json!({
            "type": "array",
            "items": { "anyOf": [{ "type": "integer" }, { "type": "null" }] }
        })
    );
    assert_eq!(
        variant_of("Plain")["properties"]["value"],
        serde_json::json!({ "type": "string" })
    );
}

/// Test 18: what serde writes for a multi-element tuple variant whose second element is an
/// `Option` — the capture the three surfaces below are read against.
#[test]
fn test_multi_tuple_variant_option_element_writes_null_in_place() {
    assert_eq!(
        serde_json::to_value(SlotElements::Pair("a".to_owned(), None)).unwrap(),
        serde_json::json!({ "type": "Pair", "value": ["a", null] }),
        "A `None` element keeps its position and writes `null` there"
    );
}

/// Test 18a: TypeScript describes those elements as the slots they are.
#[test]
fn test_multi_tuple_variant_option_element_typescript_null_flavor() {
    let ts = SlotElements::ts_definition();

    assert!(
        ts.contains("value: [string, number | null]"),
        "Pair's second element is the slot the `None` fills with `null`. Got: {ts}"
    );
}

/// Test 18b: the Zod schema of that tuple admits the `null` serde writes. A `z.tuple` element
/// cannot be omitted, so an undefined-flavored union there would leave `["a", null]` unmatched.
#[cfg(feature = "zod")]
#[test]
fn test_multi_tuple_variant_option_element_zod_null_flavor() {
    let zod = SlotElements::zod_schema();

    assert!(
        zod.contains("value: z.tuple([z.string(), z.nullable(z.number().int())])"),
        "Pair's tuple admits the `null` in place. Got: {zod}"
    );
}

/// Test 18c: the JSON schema already said so, and keeps saying it — now over the fixed-arity array
/// every other tuple position writes, bounds included.
#[cfg(feature = "jsonschema")]
#[test]
fn test_multi_tuple_variant_option_element_json_schema_null_flavor() {
    let schema = SlotElements::json_schema();
    let variant = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|member| member["properties"]["type"]["const"] == "Pair")
        .unwrap();

    assert_eq!(
        variant["properties"]["value"],
        serde_json::json!({
            "type": "array",
            "prefixItems": [
                { "type": "string" },
                { "anyOf": [{ "type": "integer" }, { "type": "null" }] }
            ],
            "items": false,
            "minItems": 2_u64,
            "maxItems": 2_u64
        })
    );
}

/// The two-element tuple a variant carries and the two-element tuple a field carries are the same
/// JSON array, so they describe as the same schema — bounds included.
#[cfg(feature = "jsonschema")]
#[test]
fn test_multi_tuple_variant_json_schema_matches_tuple_field() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ArityField {
        pub pair: (u32, u32),
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum ArityVariant {
        Pair(u32, u32),
    }

    let member = external_member(&ArityVariant::json_schema(), "Pair");
    let variant_value = &member["properties"]["Pair"];

    assert_eq!(
        *variant_value,
        ArityField::json_schema()["properties"]["pair"],
        "A variant's tuple array must describe as the tuple field does. Got: {variant_value}"
    );
    assert_eq!(
        *variant_value,
        serde_json::json!({
            "type": "array",
            "prefixItems": [{ "type": "integer" }, { "type": "integer" }],
            "items": false,
            "minItems": 2_u64,
            "maxItems": 2_u64
        })
    );
}

/// The bounds are read against real payloads: the array serde writes for the variant sits inside
/// them, and the short and long arrays serde can neither write nor read back sit outside.
#[cfg(feature = "jsonschema")]
#[test]
fn test_multi_tuple_variant_arity_bounds_reject_wrong_length_arrays() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(tag = "type", content = "value")]
    pub enum ArityProbe {
        Pair(u32, u32),
    }

    let schema = ArityProbe::json_schema();
    let value = &schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|member| member["properties"]["type"]["const"] == "Pair")
        .unwrap()["properties"]["value"];
    let admitted = value["minItems"].as_u64().unwrap()..=value["maxItems"].as_u64().unwrap();

    let written = serde_json::to_value(ArityProbe::Pair(1, 2)).unwrap();
    let written_len = u64::try_from(written["value"].as_array().unwrap().len()).unwrap();
    assert!(
        admitted.contains(&written_len),
        "What serde writes must validate. Got length {written_len} against {admitted:?}"
    );

    for rejected in [
        serde_json::json!([]),
        serde_json::json!([1_u32]),
        serde_json::json!([1_u32, 2_u32, 3_u32]),
    ] {
        let len = u64::try_from(rejected.as_array().unwrap().len()).unwrap();
        assert!(
            !admitted.contains(&len),
            "An array of {len} elements is not the tuple. Got {admitted:?}"
        );
    }
}

/// Test 19: what serde writes for an enum carrying no tagging attributes — the capture every
/// surface assertion below is read against.
#[test]
fn test_attribute_less_enum_writes_the_externally_tagged_form() {
    assert_eq!(
        serde_json::to_value(External::Pair(1, 2)).unwrap(),
        serde_json::json!({ "Pair": [1_u32, 2_u32] }),
        "A tuple variant's name is the key and its elements are the array under it"
    );
    assert_eq!(
        serde_json::to_value(External::Single("a".to_owned())).unwrap(),
        serde_json::json!({ "Single": "a" }),
        "A newtype variant's content sits under the key, unwrapped"
    );
    assert_eq!(
        serde_json::to_value(External::Fields {
            a: "a".to_owned(),
            b: true
        })
        .unwrap(),
        serde_json::json!({ "Fields": { "a": "a", "b": true } }),
        "A struct variant's fields sit in an object under the key"
    );
    assert_eq!(
        serde_json::to_value(External::Bare).unwrap(),
        serde_json::json!("Bare"),
        "A unit variant carries no key at all: it is the bare name"
    );
}

/// Test 19a: TypeScript describes that form: a union of single-key objects, plus the bare name for
/// the variant that writes no key.
#[test]
fn test_attribute_less_enum_typescript_is_the_externally_tagged_union() {
    let ts = External::ts_definition();

    assert!(
        ts.contains("\"Pair\": [number, number]"),
        "The tuple sits under the variant's key. Got: {ts}"
    );
    assert!(
        ts.contains("\"Single\": string"),
        "The newtype content sits under the key, unwrapped. Got: {ts}"
    );
    assert!(
        ts.contains("\"Fields\": {"),
        "The struct fields sit in an object under the key. Got: {ts}"
    );
    assert!(ts.contains("\"Bare\""), "Missing Bare. Got: {ts}");
    assert!(
        !ts.contains("\"Bare\":"),
        "A unit variant carries no key. Got: {ts}"
    );
    assert!(
        !ts.contains("type: \"Pair\""),
        "Nothing writes a tag beside the content. Got: {ts}"
    );
}

/// Test 19b: the Zod schema admits the same union. The discriminator is the key itself, so the
/// members are plain `z.strictObject`s in a `z.union` rather than a `z.discriminatedUnion`.
#[cfg(feature = "zod")]
#[test]
fn test_attribute_less_enum_zod_is_the_externally_tagged_union() {
    let zod = External::zod_schema();

    assert!(zod.contains("z.union(["), "Missing union. Got: {zod}");
    assert!(
        !zod.contains("z.discriminatedUnion"),
        "No shared field to discriminate on. Got: {zod}"
    );
    assert!(
        zod.contains(
            "z.strictObject({\n  \"Pair\": z.tuple([z.number().int(), z.number().int()]),\n})"
        ),
        "Pair's member is the closed object holding its tuple. Got: {zod}"
    );
    assert!(
        zod.contains("z.strictObject({\n  \"Single\": z.string(),\n})"),
        "Single's member holds its content unwrapped. Got: {zod}"
    );
    assert!(
        zod.contains("\"Fields\": z.strictObject("),
        "Fields' content is the object its fields sit in. Got: {zod}"
    );
    assert!(
        zod.contains("z.literal(\"Bare\")"),
        "Bare is the name alone. Got: {zod}"
    );
}

/// Test 19c: the JSON schema is a `oneOf` over closed single-key objects, plus the string constant
/// the unit variant writes. Each key holds the content the wire capture writes under it.
#[cfg(feature = "jsonschema")]
#[test]
fn test_attribute_less_enum_json_schema_is_the_externally_tagged_union() {
    let schema = External::json_schema();

    assert_eq!(
        external_member(&schema, "Pair")["properties"]["Pair"],
        serde_json::json!({
            "type": "array",
            "prefixItems": [{ "type": "integer" }, { "type": "integer" }],
            "items": false,
            "minItems": 2_u64,
            "maxItems": 2_u64
        })
    );
    assert_eq!(
        external_member(&schema, "Single")["properties"]["Single"],
        serde_json::json!({ "type": "string" })
    );
    assert_eq!(
        external_member(&schema, "Fields")["properties"]["Fields"],
        serde_json::json!({
            "type": "object",
            "properties": { "a": { "type": "string" }, "b": { "type": "boolean" } },
            "required": ["a", "b"],
            "additionalProperties": false
        })
    );

    // Every keyed member is closed around its one key, which is what serde's single-key map
    // deserializer accepts.
    for variant in ["Pair", "Single", "Fields"] {
        let member = external_member(&schema, variant);
        assert_eq!(member["type"], "object", "Got: {member}");
        assert_eq!(member["additionalProperties"], false, "Got: {member}");
        assert_eq!(
            member["properties"].as_object().unwrap().len(),
            1,
            "A member carries the one key serde writes. Got: {member}"
        );
    }

    assert!(
        schema["oneOf"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!({ "type": "string", "const": "Bare" })),
        "The unit variant is the bare name. Got: {schema}"
    );
}

/// Test 19d: the round trip both schemas owe. Every payload serde writes lands in the member the
/// JSON schema names for it, with the content that member declares; and a payload built from what
/// a member admits reads back into the value it describes.
#[cfg(feature = "jsonschema")]
#[test]
fn test_attribute_less_enum_round_trips_against_its_schema() {
    let schema = External::json_schema();

    for (variant, value) in [
        ("Pair", External::Pair(1, 2)),
        ("Single", External::Single("a".to_owned())),
        (
            "Fields",
            External::Fields {
                a: "a".to_owned(),
                b: true,
            },
        ),
    ] {
        let written = serde_json::to_value(&value).unwrap();
        let member = external_member(&schema, variant);
        let content = &written[variant];

        assert_eq!(
            written.as_object().unwrap().len(),
            1,
            "A closed member admits exactly the one key. Got: {written}"
        );
        let declared = &member["properties"][variant];
        assert_eq!(
            declared["type"],
            json_type_name(content),
            "The key holds what serde writes under it. Got: {declared}"
        );

        assert_eq!(
            serde_json::from_value::<External>(written.clone()).unwrap(),
            value,
            "What serde writes must read back. Got: {written}"
        );
    }

    // The bare name is the whole value, and reads back as the variant it names.
    let bare = serde_json::to_value(External::Bare).unwrap();
    assert_eq!(
        serde_json::from_value::<External>(bare).unwrap(),
        External::Bare
    );

    // A payload the schema admits deserializes: the key the member requires, holding the content
    // it declares.
    assert_eq!(
        serde_json::from_value::<External>(serde_json::json!({ "Pair": [7_u32, 8_u32] })).unwrap(),
        External::Pair(7, 8)
    );

    // And the adjacent form the schema no longer describes is one the type cannot read either.
    assert!(
        serde_json::from_value::<External>(
            serde_json::json!({ "type": "Pair", "value": [1_u32, 2_u32] })
        )
        .is_err(),
        "The adjacent form is not what this enum reads"
    );
}

/// Test 19e: naming the tagging attributes keeps the adjacent form untouched. The variants are the
/// same ones the external form renders above, so what differs is placement and nothing else.
#[test]
fn test_explicitly_tagged_twin_keeps_the_adjacent_form() {
    assert_eq!(
        serde_json::to_value(Adjacent::Pair(1, 2)).unwrap(),
        serde_json::json!({ "type": "Pair", "value": [1_u32, 2_u32] })
    );

    let ts = Adjacent::ts_definition();
    assert!(ts.contains("type: \"Pair\""), "Got: {ts}");
    assert!(ts.contains("value: [number, number]"), "Got: {ts}");
    assert!(ts.contains("type: \"Bare\""), "Got: {ts}");
    assert!(ts.contains("a: string"), "Got: {ts}");
}

/// Test 20: what serde writes for a renamed variant, and the keys the surfaces carry for it. The
/// key is quoted because a rename can spell it as something no bare identifier can hold.
#[test]
fn test_renamed_variant_key_is_the_wire_name() {
    assert_eq!(
        serde_json::to_value(RenamedExternal::Mime("x".to_owned())).unwrap(),
        serde_json::json!({ "application/pdf": "x" })
    );
    assert_eq!(
        serde_json::to_value(RenamedExternal::BigThing(1)).unwrap(),
        serde_json::json!({ "bigThing": 1_u32 })
    );
    assert_eq!(
        serde_json::to_value(RenamedExternal::UnitThing).unwrap(),
        serde_json::json!("unitThing")
    );

    let ts = RenamedExternal::ts_definition();
    assert!(
        ts.contains("\"application/pdf\": string"),
        "The renamed key is held as written. Got: {ts}"
    );
    assert!(
        ts.contains("\"bigThing\": number"),
        "`rename_all` reaches the key. Got: {ts}"
    );
    assert!(
        ts.contains("\"unitThing\""),
        "The unit variant is its renamed name. Got: {ts}"
    );
}

/// Test 20a: the Zod schema holds the same keys.
#[cfg(feature = "zod")]
#[test]
fn test_renamed_variant_key_is_the_wire_name_in_zod() {
    let zod = RenamedExternal::zod_schema();

    assert!(
        zod.contains("\"application/pdf\": z.string()"),
        "Got: {zod}"
    );
    assert!(zod.contains("\"bigThing\": z.number().int()"), "Got: {zod}");
    assert!(zod.contains("z.literal(\"unitThing\")"), "Got: {zod}");
}

/// Test 20b: and the JSON schema names the same keys, required and closed.
#[cfg(feature = "jsonschema")]
#[test]
fn test_renamed_variant_key_is_the_wire_name_in_json_schema() {
    let schema = RenamedExternal::json_schema();

    assert_eq!(
        external_member(&schema, "application/pdf")["properties"]["application/pdf"],
        serde_json::json!({ "type": "string" })
    );
    assert_eq!(
        external_member(&schema, "bigThing")["properties"]["bigThing"],
        serde_json::json!({ "type": "integer" })
    );
    assert!(
        schema["oneOf"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!({ "type": "string", "const": "unitThing" })),
        "Got: {schema}"
    );
}

/// Test 21: a member whose content references the enum defers it through a getter, as the adjacent
/// form's content key does. The member beside it holds no reference and needs none.
#[cfg(feature = "zod")]
#[test]
fn test_recursive_external_variant_defers_its_reference() {
    let zod = RecursiveExternal::zod_schema();

    assert!(
        zod.contains("get \"Arr\"() { return z.array(RecursiveExternal$Schema); },"),
        "Got: {zod}"
    );
    assert!(zod.contains("\"Txt\": z.string()"), "Got: {zod}");
}

/// Test 19f: and the tagged twin's Zod schema still switches on the tag it names.
#[cfg(feature = "zod")]
#[test]
fn test_explicitly_tagged_twin_keeps_the_adjacent_zod_form() {
    let zod = Adjacent::zod_schema();

    assert!(zod.contains("z.discriminatedUnion(\"type\""), "Got: {zod}");
    assert!(
        zod.contains("value: z.tuple([z.number().int(), z.number().int()])"),
        "Got: {zod}"
    );
}

/// Test 19g: and its JSON schema still holds the tuple under the content key.
#[cfg(feature = "jsonschema")]
#[test]
fn test_explicitly_tagged_twin_keeps_the_adjacent_json_schema_form() {
    let schema = Adjacent::json_schema();

    assert_eq!(schema["type"], "object");
    let variant = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|member| member["properties"]["type"]["const"] == "Pair")
        .unwrap();
    assert_eq!(
        variant["properties"]["value"],
        serde_json::json!({
            "type": "array",
            "prefixItems": [{ "type": "integer" }, { "type": "integer" }],
            "items": false,
            "minItems": 2_u64,
            "maxItems": 2_u64
        })
    );
}

/// Test 22: what serde writes for a bare tag. There is no content key: a struct variant's fields
/// and a newtype variant's inner members are both written beside the tag, in the same object.
#[test]
fn test_bare_tag_writes_the_variant_data_beside_the_tag() {
    assert_eq!(
        serde_json::to_value(Internal::Fields {
            a: "a".to_owned(),
            b: true
        })
        .unwrap(),
        serde_json::json!({ "type": "Fields", "a": "a", "b": true }),
        "A struct variant's fields sit beside the tag"
    );
    assert_eq!(
        serde_json::to_value(Internal::Wrapped(TagPayload {
            a: "a".to_owned(),
            b: true
        }))
        .unwrap(),
        serde_json::json!({ "type": "Wrapped", "a": "a", "b": true }),
        "A newtype variant's inner members sit beside the tag too: no key holds them"
    );
    assert_eq!(
        serde_json::to_value(Internal::Bare).unwrap(),
        serde_json::json!({ "type": "Bare" }),
        "A unit variant is the tag alone"
    );
}

/// Test 22a: and the shape that has no members to write there. serde refuses it at run time, which
/// is why the crate refuses the declaration: there is no schema to write for a value that cannot
/// exist on the wire.
#[test]
fn test_bare_tag_newtype_over_a_scalar_is_unserializable() {
    let refusal = serde_json::to_value(InternalScalar::Single("a".to_owned()))
        .unwrap_err()
        .to_string();
    assert!(
        refusal.contains("cannot serialize tagged newtype variant"),
        "Got: {refusal}"
    );
    assert!(refusal.contains("containing a string"), "Got: {refusal}");
}

/// Test 22b: TypeScript spreads the inner type beside the tag rather than putting it under a key.
/// `&` binds tighter than `|`, so the intersection is one member of the union.
#[test]
fn test_bare_tag_typescript_spreads_the_inner_type_beside_the_tag() {
    let ts = Internal::ts_definition();

    assert!(
        ts.contains("type: \"Wrapped\";\n} & TagPayload"),
        "The inner type joins the tag's object. Got: {ts}"
    );
    assert!(
        !ts.contains("value:"),
        "Nothing writes a content key under a bare tag. Got: {ts}"
    );
    assert!(ts.contains("type: \"Fields\";"), "Got: {ts}");
    assert!(ts.contains("  a: string;"), "Got: {ts}");
    assert!(ts.contains("type: \"Bare\";"), "Got: {ts}");
}

/// Test 22c: the Zod member for a newtype variant is the tag's object intersected with the inner
/// schema. An intersection has no shape of its own to read a discriminator out of, so the union
/// that holds it is a plain `z.union`.
#[cfg(feature = "zod")]
#[test]
fn test_bare_tag_zod_intersects_the_inner_schema() {
    let zod = Internal::zod_schema();

    assert!(
        zod.contains(
            "z.strictObject({\n  type: z.literal(\"Wrapped\"),\n}).and(TagPayload$Schema)"
        ),
        "Got: {zod}"
    );
    assert!(zod.contains("z.union(["), "Got: {zod}");
    assert!(
        !zod.contains("z.discriminatedUnion"),
        "An intersection member cannot be discriminated on. Got: {zod}"
    );
    assert!(!zod.contains("value:"), "Got: {zod}");
}

/// Test 22d: with no newtype variant every member is an object carrying the tag, so the union still
/// switches on it.
#[cfg(feature = "zod")]
#[test]
fn test_bare_tag_without_a_newtype_variant_still_discriminates() {
    let zod = InternalNamedOnly::zod_schema();

    assert!(
        zod.contains("z.discriminatedUnion(\"type\", [z.strictObject({\n  type: z.literal(\"Fields\"),\n  a: z.string(),\n})])"),
        "Got: {zod}"
    );
}

/// Test 22e: the JSON schema holds the inner type's fields beside the tag, required where the inner
/// requires them, and closed around exactly the members serde writes.
#[cfg(feature = "jsonschema")]
#[test]
fn test_bare_tag_json_schema_holds_the_inner_fields_beside_the_tag() {
    let schema = Internal::json_schema();
    let wrapped = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|member| member["properties"]["type"]["const"] == "Wrapped")
        .unwrap();

    assert_eq!(
        wrapped["properties"],
        serde_json::json!({
            "type": { "type": "string", "const": "Wrapped" },
            "a": { "type": "string" },
            "b": { "type": "boolean" }
        })
    );
    assert_eq!(
        wrapped["required"],
        serde_json::json!(["type", "a", "b"]),
        "The tag and everything the inner type requires"
    );
    assert_eq!(wrapped["additionalProperties"], false);
    assert!(
        wrapped["properties"]["value"].is_null(),
        "There is no content key. Got: {wrapped}"
    );
}

/// Test 22f: the round trip the schema owes. Every payload serde writes is admitted by the member
/// the schema names for it, and reads back into the value it describes.
#[cfg(feature = "jsonschema")]
#[test]
fn test_bare_tag_round_trips_against_its_schema() {
    let schema = Internal::json_schema();

    for value in [
        Internal::Bare,
        Internal::Fields {
            a: "a".to_owned(),
            b: true,
        },
        Internal::Wrapped(TagPayload {
            a: "a".to_owned(),
            b: true,
        }),
    ] {
        let written = serde_json::to_value(&value).unwrap();
        let member = schema["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .find(|member| member["properties"]["type"]["const"] == written["type"])
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        assert!(!member.is_null(), "No member for {written}");

        let declared = member["properties"].as_object().unwrap();
        for key in written.as_object().unwrap().keys() {
            assert!(
                declared.contains_key(key),
                "The member admits every key serde writes. Missing `{key}` in {member}"
            );
        }
        for required in member["required"].as_array().unwrap() {
            assert!(
                written[required.as_str().unwrap()] != serde_json::Value::Null,
                "The member requires only keys serde writes. Got: {written}"
            );
        }

        assert_eq!(
            serde_json::from_value::<Internal>(written.clone()).unwrap(),
            value,
            "What serde writes must read back. Got: {written}"
        );
    }

    // And the adjacent form the schema no longer describes is one the type cannot read either.
    assert!(
        serde_json::from_value::<Internal>(
            serde_json::json!({ "type": "Wrapped", "value": { "a": "a", "b": true } })
        )
        .is_err(),
        "A content key is not what this enum reads"
    );
}

/// Test 22g: naming a content key beside the tag keeps the adjacent form, content key and all.
#[test]
fn test_adjacent_twin_keeps_its_content_key() {
    assert_eq!(
        serde_json::to_value(Adjacent::Single("a".to_owned())).unwrap(),
        serde_json::json!({ "type": "Single", "value": "a" })
    );

    let ts = Adjacent::ts_definition();
    assert!(ts.contains("value: string"), "Got: {ts}");
}

/// Test 22h: what serde writes for a bare tag over a plain enum. The enum writes its own variant
/// name, so what lands beside the tag is a key holding null — a key no member closed around the tag
/// names, which is why the declaration is refused rather than described.
#[test]
fn test_bare_tag_over_a_plain_enum_writes_a_key_the_tag_does_not_name() {
    assert_eq!(
        serde_json::to_value(InternalOverEnum::EnumInner(InternalHue::Red)).unwrap(),
        serde_json::json!({ "type": "EnumInner", "Red": null })
    );
}

/// Test 22i: and what serde writes for a bare tag over a newtype that reaches the wire as a string
/// — nothing. The run-time refusal is the same one every scalar content gets; the name in front of
/// it is all that let this one past the declaration guard.
#[test]
fn test_bare_tag_over_a_string_newtype_is_unserializable() {
    let refusal = serde_json::to_value(InternalOverBrand::Branded(InternalSlug("s".to_owned())))
        .unwrap_err()
        .to_string();
    assert!(
        refusal.contains("cannot serialize tagged newtype variant"),
        "Got: {refusal}"
    );
    assert!(refusal.contains("containing a string"), "Got: {refusal}");
}

/// Test 22j: so the merge refuses it too, rather than closing the object around the tag alone. The
/// schema the content resolves to is what says it, and it is read at the moment the wrong document
/// would otherwise be written.
#[cfg(feature = "jsonschema")]
#[test]
#[should_panic(
    expected = "`InternalOverBrand`: the content of variant `Branded`, `InternalSlug` is not written as an object"
)]
fn test_bare_tag_over_a_string_newtype_is_refused_by_the_merge() {
    assert!(InternalOverBrand::json_schema().is_object());
}

/// Test 22k: the remedy the refusal names is one the author can act on.
#[cfg(feature = "jsonschema")]
#[test]
#[should_panic(expected = "name a `content` key so the content gets an object of its own")]
fn test_the_merge_refusal_names_the_remedy() {
    assert!(InternalOverBrand::json_schema().is_object());
}

/// Test 22l: a struct inner is untouched by either refusal — the document a bare tag wrote before
/// them, byte for byte. Only the flattened member goes through the merge, which is why only it
/// carries the merge's own `"type": "object"`.
#[cfg(feature = "jsonschema")]
#[test]
fn test_bare_tag_over_a_struct_documents_byte_identically() {
    assert_eq!(
        serde_json::to_string(&Internal::json_schema()).unwrap(),
        r#"{"type":"object","oneOf":[{"additionalProperties":false,"properties":{"type":{"type":"string","const":"Bare"}},"required":["type"]},{"additionalProperties":false,"properties":{"type":{"type":"string","const":"Fields"},"a":{"type":"string"},"b":{"type":"boolean"}},"required":["type","a","b"]},{"type":"object","properties":{"type":{"type":"string","const":"Wrapped"},"a":{"type":"string"},"b":{"type":"boolean"}},"required":["type","a","b"],"additionalProperties":false}]}"#
    );
}

/// Test 22m: what serde writes for a bare tag over an untagged enum. The union names no type of its
/// own, but every member of this one writes an object, so what lands beside the tag is that member's
/// members — one key set per member.
#[test]
fn test_bare_tag_over_an_untagged_enum_writes_the_matched_members_keys() {
    assert_eq!(
        serde_json::to_value(InternalOverUntagged::Wrapped(InternalEither::First(
            InternalFirst { a: "x".to_owned() }
        )))
        .unwrap(),
        serde_json::json!({ "type": "Wrapped", "a": "x" })
    );
    assert_eq!(
        serde_json::to_value(InternalOverUntagged::Wrapped(InternalEither::Second(
            InternalSecond { b: true }
        )))
        .unwrap(),
        serde_json::json!({ "type": "Wrapped", "b": true })
    );
}

/// Test 22n: so the tag multiplies over the union the way it already multiplies over a discriminated
/// one — a branch per member, each closed around the tag plus that member's members.
#[cfg(feature = "jsonschema")]
#[test]
fn test_bare_tag_over_an_untagged_enum_multiplies_over_its_members() {
    assert_eq!(
        serde_json::to_string(&InternalOverUntagged::json_schema()).unwrap(),
        r#"{"type":"object","oneOf":[{"type":"object","oneOf":[{"type":"object","properties":{"type":{"type":"string","const":"Wrapped"},"a":{"type":"string"}},"required":["type","a"],"additionalProperties":false},{"type":"object","properties":{"type":{"type":"string","const":"Wrapped"},"b":{"type":"boolean"}},"required":["type","b"],"additionalProperties":false}]}]}"#
    );
}

/// Test 22o: and the keys those branches require are exactly the keys the captures carry. Before the
/// tag multiplied out, one branch required the tag alone and named nothing else, so neither capture
/// was accepted.
#[cfg(feature = "jsonschema")]
#[test]
fn test_bare_tag_over_an_untagged_enum_requires_every_key_serde_writes() {
    let schema = InternalOverUntagged::json_schema();
    let required: Vec<&serde_json::Value> = schema["oneOf"][0]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .map(|branch| &branch["required"])
        .collect();
    assert_eq!(
        required,
        vec![
            &serde_json::json!(["type", "a"]),
            &serde_json::json!(["type", "b"])
        ],
        "Got: {schema}"
    );
}

/// Test 22p: a union member serde writes as a string is a member serde cannot put beside the tag at
/// all, so the merge refuses the whole union and names the branch that cannot join it.
#[cfg(feature = "jsonschema")]
#[test]
#[should_panic(
    expected = "`InternalOverScalarUntagged`: the content of variant `Wrapped`, `InternalScalarEither` writes a union member that is not an object — its branch 2 describes a `string`"
)]
fn test_a_string_member_of_a_tagged_untagged_enum_is_refused_by_the_merge() {
    assert!(InternalOverScalarUntagged::json_schema().is_object());
}
