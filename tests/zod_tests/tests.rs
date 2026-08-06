use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tixschema::model_schema;

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Address {
    city: String,
    street: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct AllNumericTypes {
    f32: f32,
    f64: f64,
    i16: i16,
    i32: i32,
    i64: i64,
    i8: i8,
    isize: isize,
    u16: u16,
    u32: u32,
    u64: u64,
    u8: u8,
    usize: usize,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ArrayFields {
    numbers: Vec<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_array: Option<Vec<String>>,
    tags: Vec<String>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct BasicStruct {
    active: bool,
    age: u32,
    name: String,
    score: f64,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ComplexCollections {
    array_of_maps: Vec<HashMap<String, String>>,
    map_of_arrays: HashMap<String, Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_map: Option<HashMap<String, i32>>,
}

#[cfg(feature = "serde")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct CustomFieldRename {
    #[serde(rename = "customName")]
    field_name: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct EmptyStruct;

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct MapFields {
    counts: HashMap<String, i32>,
    metadata: HashMap<String, String>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct OptionalFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_bool: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_number: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_string: Option<String>,
    required: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
enum Payment {
    BankTransfer { account: String },
    Cash { amount: f64 },
    CreditCard { amount: f64, card_number: String },
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Person {
    address: Address,
    name: String,
    previous_addresses: Vec<Address>,
}

#[cfg(feature = "serde")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct RenamedFields {
    user_email: String,
    user_name: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
enum Status {
    Active,
    Inactive,
    Pending,
}

#[test]
fn test_basic_struct_generates_zod_schema() {
    let zod = BasicStruct::zod_schema();

    assert!(
        zod.contains("z.strictObject({"),
        "Should use z.strictObject for structs. Got: {zod}"
    );
    assert!(
        zod.contains("name: z.string()"),
        "String fields should use z.string(). Got: {zod}"
    );
    assert!(
        zod.contains("age: z.number().int()"),
        "Integer fields should use z.number().int(). Got: {zod}"
    );
    assert!(
        zod.contains("score: z.number()"),
        "Float fields should use z.number() without .int(). Got: {zod}"
    );
    assert!(
        zod.contains("active: z.boolean()"),
        "Boolean fields should use z.boolean(). Got: {zod}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_zod_with_typescript_feature_includes_type_annotations() {
    let zod = BasicStruct::zod_schema();

    assert!(
        zod.contains("const BasicStruct$RawSchema = z.strictObject({"),
        "Should declare RawSchema const when typescript feature enabled. Got: {zod}"
    );
    assert!(
        zod.contains(
            "export const BasicStruct$Schema: ZodType<BasicStruct> = BasicStruct$RawSchema;"
        ),
        "Should export typed schema when typescript feature enabled. Got: {zod}"
    );
}

#[test]
fn test_optional_fields_use_union_with_undefined() {
    let zod = OptionalFields::zod_schema();

    assert!(
        zod.contains("required: z.string()"),
        "Required fields should not be wrapped. Got: {zod}"
    );
    assert!(
        zod.contains(
            "optional_string: z.union([z.null().transform(() => undefined), z.string(), z.undefined()])"
        ),
        "Optional strings should use z.union with undefined. Got: {zod}"
    );
    assert!(
        zod.contains(
            "optional_number: z.union([z.null().transform(() => undefined), z.number().int(), z.undefined()])"
        ),
        "Optional numbers should use z.union with undefined. Got: {zod}"
    );
    assert!(
        zod.contains(
            "optional_bool: z.union([z.null().transform(() => undefined), z.boolean(), z.undefined()])"
        ),
        "Optional booleans should use z.union with undefined. Got: {zod}"
    );
}

#[test]
fn test_vec_fields_use_z_array() {
    let zod = ArrayFields::zod_schema();

    assert!(
        zod.contains("tags: z.array(z.string())"),
        "Vec<String> should use z.array(z.string()). Got: {zod}"
    );
    assert!(
        zod.contains("numbers: z.array(z.number().int())"),
        "Vec<i32> should use z.array(z.number().int()). Got: {zod}"
    );
    assert!(
        zod.contains(
            "optional_array: z.union([z.null().transform(() => undefined), z.array(z.string()), z.undefined()])"
        ),
        "Optional arrays should wrap array in union. Got: {zod}"
    );
}

#[test]
fn test_hashmap_uses_z_record() {
    let zod = MapFields::zod_schema();

    assert!(
        zod.contains("metadata: z.record(z.string(), z.string())"),
        "HashMap<String, String> should use z.record. Got: {zod}"
    );
    assert!(
        zod.contains("counts: z.record(z.string(), z.number().int())"),
        "HashMap<String, i32> should use z.record with int values. Got: {zod}"
    );
}

#[test]
fn test_nested_structs_reference_schema() {
    let zod = Person::zod_schema();

    assert!(
        zod.contains("address: Address$Schema"),
        "Nested struct should reference its schema. Got: {zod}"
    );
    assert!(
        zod.contains("previous_addresses: z.array(Address$Schema)"),
        "Array of nested structs should use z.array with schema ref. Got: {zod}"
    );
}

#[test]
fn test_plain_enum_uses_z_enum() {
    let zod = Status::zod_schema();

    assert!(
        zod.contains("z.enum(["),
        "Plain enums should use z.enum. Got: {zod}"
    );
    assert!(
        zod.contains("\"Active\""),
        "Should contain Active variant. Got: {zod}"
    );
    assert!(
        zod.contains("\"Inactive\""),
        "Should contain Inactive variant. Got: {zod}"
    );
    assert!(
        zod.contains("\"Pending\""),
        "Should contain Pending variant. Got: {zod}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_plain_enum_with_typescript_has_type_annotation() {
    let zod = Status::zod_schema();

    assert!(
        zod.contains("const Status$RawSchema = z.enum(["),
        "Plain enum with typescript should have $RawSchema. Got: {zod}"
    );
    assert!(
        zod.contains("export const Status$Schema: ZodType<Status> = Status$RawSchema;"),
        "Plain enum with typescript should have typed $Schema. Got: {zod}"
    );
}

#[test]
fn test_discriminated_union_uses_z_discriminated_union() {
    let zod = Payment::zod_schema();

    assert!(
        zod.contains("z.discriminatedUnion"),
        "Discriminated unions should use z.discriminatedUnion. Got: {zod}"
    );
    assert!(
        zod.contains("\"type\""),
        "Should reference the discriminator field. Got: {zod}"
    );
}

#[test]
fn test_integer_types_use_int_modifier() {
    let zod = AllNumericTypes::zod_schema();

    assert!(
        zod.contains("u8: z.number().int()"),
        "u8 should use .int(). Got: {zod}"
    );
    assert!(
        zod.contains("i32: z.number().int()"),
        "i32 should use .int(). Got: {zod}"
    );
    assert!(
        zod.contains("usize: z.number().int()"),
        "usize should use .int(). Got: {zod}"
    );
    assert!(
        zod.contains("isize: z.number().int()"),
        "isize should use .int(). Got: {zod}"
    );
}

#[test]
fn test_float_types_do_not_use_int_modifier() {
    let zod = AllNumericTypes::zod_schema();

    assert!(
        zod.contains("f32: z.number()") && !zod.contains("f32: z.number().int()"),
        "f32 should not use .int(). Got: {zod}"
    );
    assert!(
        zod.contains("f64: z.number()") && !zod.contains("f64: z.number().int()"),
        "f64 should not use .int(). Got: {zod}"
    );
}

#[test]
#[cfg(feature = "serde")]
fn test_serde_rename_all_affects_field_names() {
    let zod = RenamedFields::zod_schema();

    assert!(
        zod.contains("userName: z.string()"),
        "rename_all should convert to camelCase. Got: {zod}"
    );
    assert!(
        zod.contains("userEmail: z.string()"),
        "rename_all should convert to camelCase. Got: {zod}"
    );
    assert!(
        !zod.contains("user_name") && !zod.contains("user_email"),
        "Should not contain original snake_case names. Got: {zod}"
    );
}

#[test]
#[cfg(feature = "serde")]
fn test_serde_field_rename() {
    let zod = CustomFieldRename::zod_schema();

    assert!(
        zod.contains("customName: z.string()"),
        "Field rename should use custom name. Got: {zod}"
    );
    assert!(
        !zod.contains("field_name"),
        "Should not contain original field name. Got: {zod}"
    );
}

#[test]
fn test_complex_nested_collections() {
    let zod = ComplexCollections::zod_schema();

    assert!(
        zod.contains("map_of_arrays: z.record(z.string(), z.array(z.string()))"),
        "HashMap<String, Vec<String>> should nest correctly. Got: {zod}"
    );
    assert!(
        zod.contains(
            "optional_map: z.union([z.null().transform(() => undefined), z.record(z.string(), z.number().int()), z.undefined()])"
        ),
        "Optional HashMap should wrap in union. Got: {zod}"
    );
    assert!(
        zod.contains("array_of_maps: z.array(z.record(z.string(), z.string()))"),
        "Vec<HashMap> should nest correctly. Got: {zod}"
    );
}

#[test]
#[cfg(feature = "object_id")]
fn test_objectid_generates_proper_validation() {
    use mongodb::bson::oid::ObjectId;

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct Document {
        #[serde(skip_serializing_if = "Option::is_none")]
        author_id: Option<ObjectId>,
        id: ObjectId,
        tag_ids: Vec<ObjectId>,
    }

    let zod = Document::zod_schema();

    assert!(
        zod.contains("z.object({ $oid: z.string().regex("),
        "ObjectId should have regex validation. Got: {zod}"
    );
    assert!(
        zod.contains("Invalid ObjectId"),
        "Should include error message. Got: {zod}"
    );
    assert!(
        zod.contains("tag_ids: z.array(z.object({ $oid: z.string().regex("),
        "Vec<ObjectId> should work. Got: {zod}"
    );
    assert!(
        zod.contains(
            "author_id: z.union([z.null().transform(() => undefined), z.object({ $oid: z.string().regex("
        ),
        "Option<ObjectId> should work. Got: {zod}"
    );
}

#[test]
fn test_empty_struct_generates_valid_schema() {
    let zod = EmptyStruct::zod_schema();

    assert!(
        zod.contains("z.strictObject({"),
        "Empty struct should still generate strictObject. Got: {zod}"
    );
    assert!(
        zod.contains("})"),
        "Should properly close the object. Got: {zod}"
    );
}

#[test]
#[cfg(not(feature = "typescript"))]
fn test_zod_without_typescript_uses_simple_export() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct Simple {
        field: String,
    }

    let zod = Simple::zod_schema();

    assert!(
        zod.contains("export const Simple$Schema = z.strictObject({"),
        "Without typescript, should use simple export. Got: {zod}"
    );
    assert!(
        !zod.contains("ZodType"),
        "Without typescript, should not have type annotations. Got: {zod}"
    );
    assert!(
        !zod.contains("$RawSchema"),
        "Without typescript, should not have RawSchema. Got: {zod}"
    );
}
