//! Comprehensive tests for Zod v4 schema generation
//!
//! This module tests the Zod schema generation feature of tixschema.
//! The `zod` feature generates Zod v4 validation schemas from Rust types.
//!
//! ## What is tested:
//! - Basic primitive types (string, number, boolean)
//! - Optional fields using z.union([type, `z.undefined()`])
//! - Arrays using `z.array()`
//! - `HashMaps` using `z.record()`
//! - Nested structs
//! - Plain enums using `z.enum()`
//! - Discriminated union enums using `z.discriminatedUnion()`
//! - Integration with TypeScript feature (type annotations)
//! - Serde rename attributes
//! - `ObjectId` support (when `object_id` feature enabled)
//! - All numeric types (integers vs floats)

#[cfg(feature = "zod")]
#[expect(clippy::struct_field_names, reason = "This is a test file")]
mod tests {
    use serde::{Deserialize, Serialize};
    use tixschema::model_schema;

    // ========================================================================
    // Basic Struct Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct BasicStructJson {
        name: String,
        age: u32,
        score: f64,
        active: bool,
    }

    #[test]
    fn test_basic_struct_generates_zod_schema() {
        let zod = BasicStructJson::zod_schema();

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
        let zod = BasicStructJson::zod_schema();

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

    // ========================================================================
    // Optional Fields Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct OptionalFieldsJson {
        required: String,
        optional_string: Option<String>,
        optional_number: Option<i32>,
        optional_bool: Option<bool>,
    }

    #[test]
    fn test_optional_fields_use_union_with_undefined() {
        let zod = OptionalFieldsJson::zod_schema();

        assert!(
            zod.contains("required: z.string()"),
            "Required fields should not be wrapped. Got: {zod}"
        );
        assert!(
            zod.contains("optional_string: z.union([z.string(), z.undefined()])"),
            "Optional strings should use z.union with undefined. Got: {zod}"
        );
        assert!(
            zod.contains("optional_number: z.union([z.number().int(), z.undefined()])"),
            "Optional numbers should use z.union with undefined. Got: {zod}"
        );
        assert!(
            zod.contains("optional_bool: z.union([z.boolean(), z.undefined()])"),
            "Optional booleans should use z.union with undefined. Got: {zod}"
        );
    }

    // ========================================================================
    // Array/Vec Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct ArrayFieldsJson {
        tags: Vec<String>,
        numbers: Vec<i32>,
        optional_array: Option<Vec<String>>,
    }

    #[test]
    fn test_vec_fields_use_z_array() {
        let zod = ArrayFieldsJson::zod_schema();

        assert!(
            zod.contains("tags: z.array(z.string())"),
            "Vec<String> should use z.array(z.string()). Got: {zod}"
        );
        assert!(
            zod.contains("numbers: z.array(z.number().int())"),
            "Vec<i32> should use z.array(z.number().int()). Got: {zod}"
        );
        assert!(
            zod.contains("optional_array: z.union([z.array(z.string()), z.undefined()])"),
            "Optional arrays should wrap array in union. Got: {zod}"
        );
    }

    // ========================================================================
    // HashMap/Map Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct MapFieldsJson {
        metadata: std::collections::HashMap<String, String>,
        counts: std::collections::HashMap<String, i32>,
    }

    #[test]
    fn test_hashmap_uses_z_record() {
        let zod = MapFieldsJson::zod_schema();

        assert!(
            zod.contains("metadata: z.record(z.string(), z.string())"),
            "HashMap<String, String> should use z.record. Got: {zod}"
        );
        assert!(
            zod.contains("counts: z.record(z.string(), z.number().int())"),
            "HashMap<String, i32> should use z.record with int values. Got: {zod}"
        );
    }

    // ========================================================================
    // Nested Structs Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct AddressJson {
        street: String,
        city: String,
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct PersonJson {
        name: String,
        address: AddressJson,
        previous_addresses: Vec<AddressJson>,
    }

    #[test]
    fn test_nested_structs_reference_schema() {
        let zod = PersonJson::zod_schema();

        assert!(
            zod.contains("address: Address$Schema"),
            "Nested struct should reference its schema. Got: {zod}"
        );
        assert!(
            zod.contains("previous_addresses: z.array(Address$Schema)"),
            "Array of nested structs should use z.array with schema ref. Got: {zod}"
        );
    }

    // ========================================================================
    // Plain Enum Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    enum StatusJson {
        Active,
        Inactive,
        Pending,
    }

    #[test]
    fn test_plain_enum_uses_z_enum() {
        let zod = StatusJson::zod_schema();

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
        let zod = StatusJson::zod_schema();

        assert!(
            zod.contains("export const Status$Schema: ZodType<Status> = z.enum(["),
            "Plain enum with typescript should have type annotation. Got: {zod}"
        );
    }

    // ========================================================================
    // Discriminated Union Enum Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(tag = "type")]
    enum PaymentJson {
        Cash { amount: f64 },
        CreditCard { card_number: String, amount: f64 },
        BankTransfer { account: String },
    }

    #[test]
    fn test_discriminated_union_uses_z_discriminated_union() {
        let zod = PaymentJson::zod_schema();

        assert!(
            zod.contains("z.discriminatedUnion"),
            "Discriminated unions should use z.discriminatedUnion. Got: {zod}"
        );
        assert!(
            zod.contains("\"type\""),
            "Should reference the discriminator field. Got: {zod}"
        );
    }

    // ========================================================================
    // All Numeric Types Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct AllNumericTypesJson {
        u8_val: u8,
        u16_val: u16,
        u32_val: u32,
        u64_val: u64,
        i8_val: i8,
        i16_val: i16,
        i32_val: i32,
        i64_val: i64,
        f32_val: f32,
        f64_val: f64,
        usize_val: usize,
        isize_val: isize,
    }

    #[test]
    fn test_integer_types_use_int_modifier() {
        let zod = AllNumericTypesJson::zod_schema();

        // All integer types should use .int()
        assert!(
            zod.contains("u8_val: z.number().int()"),
            "u8 should use .int(). Got: {zod}"
        );
        assert!(
            zod.contains("i32_val: z.number().int()"),
            "i32 should use .int(). Got: {zod}"
        );
        assert!(
            zod.contains("usize_val: z.number().int()"),
            "usize should use .int(). Got: {zod}"
        );
        assert!(
            zod.contains("isize_val: z.number().int()"),
            "isize should use .int(). Got: {zod}"
        );
    }

    #[test]
    fn test_float_types_do_not_use_int_modifier() {
        let zod = AllNumericTypesJson::zod_schema();

        // Float types should not use .int()
        assert!(
            zod.contains("f32_val: z.number()") && !zod.contains("f32_val: z.number().int()"),
            "f32 should not use .int(). Got: {zod}"
        );
        assert!(
            zod.contains("f64_val: z.number()") && !zod.contains("f64_val: z.number().int()"),
            "f64 should not use .int(). Got: {zod}"
        );
    }

    // ========================================================================
    // Serde Rename Tests
    // ========================================================================

    #[cfg(feature = "serde")]
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(rename_all = "camelCase")]
    struct RenamedFieldsJson {
        user_name: String,
        user_email: String,
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde_rename_all_affects_field_names() {
        let zod = RenamedFieldsJson::zod_schema();

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

    #[cfg(feature = "serde")]
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct CustomFieldRenameJson {
        #[serde(rename = "customName")]
        field_name: String,
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde_field_rename() {
        let zod = CustomFieldRenameJson::zod_schema();

        assert!(
            zod.contains("customName: z.string()"),
            "Field rename should use custom name. Got: {zod}"
        );
        assert!(
            !zod.contains("field_name"),
            "Should not contain original field name. Got: {zod}"
        );
    }

    // ========================================================================
    // Complex Collections Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct ComplexCollectionsJson {
        map_of_arrays: std::collections::HashMap<String, Vec<String>>,
        optional_map: Option<std::collections::HashMap<String, i32>>,
        array_of_maps: Vec<std::collections::HashMap<String, String>>,
    }

    #[test]
    fn test_complex_nested_collections() {
        let zod = ComplexCollectionsJson::zod_schema();

        assert!(
            zod.contains("map_of_arrays: z.record(z.string(), z.array(z.string()))"),
            "HashMap<String, Vec<String>> should nest correctly. Got: {zod}"
        );
        assert!(
            zod.contains(
                "optional_map: z.union([z.record(z.string(), z.number().int()), z.undefined()])"
            ),
            "Optional HashMap should wrap in union. Got: {zod}"
        );
        assert!(
            zod.contains("array_of_maps: z.array(z.record(z.string(), z.string()))"),
            "Vec<HashMap> should nest correctly. Got: {zod}"
        );
    }

    // ========================================================================
    // ObjectId Tests (when feature enabled)
    // ========================================================================

    #[test]
    #[cfg(feature = "object_id")]
    fn test_objectid_generates_proper_validation() {
        use mongodb::bson::oid::ObjectId;

        #[model_schema()]
        #[derive(Serialize, Deserialize, Debug, Clone)]
        struct DocumentJson {
            id: ObjectId,
            author_id: Option<ObjectId>,
            tag_ids: Vec<ObjectId>,
        }

        let zod = DocumentJson::zod_schema();

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
            zod.contains("author_id: z.union([z.object({ $oid: z.string().regex("),
            "Option<ObjectId> should work. Got: {zod}"
        );
    }

    // ========================================================================
    // Empty Struct Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct EmptyStructJson {}

    #[test]
    fn test_empty_struct_generates_valid_schema() {
        let zod = EmptyStructJson::zod_schema();

        assert!(
            zod.contains("z.strictObject({"),
            "Empty struct should still generate strictObject. Got: {zod}"
        );
        assert!(
            zod.contains("})"),
            "Should properly close the object. Got: {zod}"
        );
    }

    // ========================================================================
    // Schema Export Format Tests
    // ========================================================================

    #[test]
    #[cfg(not(feature = "typescript"))]
    fn test_zod_without_typescript_uses_simple_export() {
        #[model_schema()]
        #[derive(Serialize, Deserialize, Debug, Clone)]
        struct SimpleJson {
            field: String,
        }

        let zod = SimpleJson::zod_schema();

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
}
