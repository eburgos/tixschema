//! Tests for recursive type support in Zod schema generation
//!
//! These tests verify that recursive types (types that reference themselves)
//! generate correct Zod schemas using JavaScript getter syntax to defer
//! the reference and avoid "use before declaration" errors.

#[cfg(all(test, feature = "typescript", feature = "zod", feature = "serde"))]
mod tests {
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use tixschema::model_schema;

    // ========== Type definitions at module level ==========

    /// Recursive enum with Vec of self
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(tag = "type", content = "value")]
    pub enum RecursiveVecEnumJson {
        Text(String),
        Array(Vec<RecursiveVecEnumJson>),
    }

    /// Recursive enum with HashMap of self
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(tag = "type", content = "value")]
    pub enum RecursiveMapEnumJson {
        Number(i64),
        Object(HashMap<String, RecursiveMapEnumJson>),
    }

    /// Recursive struct with Vec of self
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct TreeNodeJson {
        pub val: String,
        pub children: Vec<TreeNodeJson>,
    }

    /// Complex DynamicValue-like enum with multiple recursive variants
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(tag = "type", content = "value")]
    pub enum DynamicValueTestJson {
        #[serde(rename = "string")]
        String(String),
        #[serde(rename = "integer")]
        Integer(i64),
        #[serde(rename = "boolean")]
        Bool(bool),
        #[serde(rename = "decimal")]
        Decimal(f64),
        #[serde(rename = "array")]
        Array(Vec<DynamicValueTestJson>),
        #[serde(rename = "object")]
        Object(HashMap<String, DynamicValueTestJson>),
    }

    /// Non-recursive enum for comparison
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(tag = "type", content = "value")]
    pub enum SimpleEnumJson {
        Text(String),
        Number(i64),
        Flag(bool),
    }

    /// Non-recursive struct for comparison
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct SimpleStructJson {
        pub name: String,
        pub age: u32,
        pub active: bool,
    }

    /// Address struct for sibling reference test
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct AddressJson {
        pub street: String,
        pub city: String,
    }

    /// Person struct referencing Address (not recursive)
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct PersonJson {
        pub name: String,
        pub address: AddressJson,
    }

    /// Recursive enum with named struct variant
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(tag = "type")]
    pub enum TreeEnumJson {
        Leaf { text: String },
        Branch { nodes: Vec<TreeEnumJson> },
    }

    // ========== Tests ==========

    /// Test 1: Recursive enum with Vec of self
    #[test]
    fn test_recursive_enum_with_vec() {
        let zod = RecursiveVecEnumJson::zod_schema();
        println!("Generated Zod:\n{}", zod);

        // Should contain getter syntax for Array variant (recursive)
        assert!(
            zod.contains("get value()"),
            "Recursive Array variant should use getter syntax. Got: {zod}"
        );

        // Text variant should NOT use getter (not recursive)
        assert!(
            zod.contains("value: z.string()"),
            "Non-recursive Text variant should use normal property syntax. Got: {zod}"
        );
    }

    /// Test 2: Recursive enum with HashMap of self
    #[test]
    fn test_recursive_enum_with_hashmap() {
        let zod = RecursiveMapEnumJson::zod_schema();
        println!("Generated Zod:\n{}", zod);

        // Should contain getter syntax for Object variant (recursive)
        assert!(
            zod.contains("get value()"),
            "Recursive Object variant should use getter syntax. Got: {zod}"
        );

        // Number variant should NOT use getter (not recursive)
        assert!(
            zod.contains("value: z.number()"),
            "Non-recursive Number variant should use normal property syntax. Got: {zod}"
        );
    }

    /// Test 3: Recursive struct with Vec of self
    #[test]
    fn test_recursive_struct() {
        let zod = TreeNodeJson::zod_schema();
        println!("Generated Zod:\n{}", zod);

        // Should contain getter syntax for children (recursive)
        assert!(
            zod.contains("get children()"),
            "Recursive children field should use getter syntax. Got: {zod}"
        );

        // val field should NOT use getter (not recursive)
        assert!(
            zod.contains("val: z.string()"),
            "Non-recursive val field should use normal property syntax. Got: {zod}"
        );
    }

    /// Test 4: Complex DynamicValue-like enum with multiple recursive variants
    #[test]
    fn test_complex_dynamic_value() {
        let zod = DynamicValueTestJson::zod_schema();
        println!("Generated Zod:\n{}", zod);

        // Count getter occurrences - should have exactly 2 (array and object)
        let getter_count = zod.matches("get value()").count();
        assert_eq!(
            getter_count, 2,
            "Should have exactly 2 getter properties (array and object). Got: {getter_count}. Schema: {zod}"
        );

        // Verify non-recursive variants use normal syntax
        assert!(
            zod.contains("value: z.string()"),
            "String variant should use normal syntax"
        );
        assert!(
            zod.contains("value: z.number().int()"),
            "Integer variant should use normal syntax"
        );
        assert!(
            zod.contains("value: z.boolean()"),
            "Boolean variant should use normal syntax"
        );
    }

    /// Test 5: Non-recursive types should not use getter syntax
    #[test]
    fn test_non_recursive_enum_no_getter() {
        let zod = SimpleEnumJson::zod_schema();
        println!("Generated Zod:\n{}", zod);

        // Should NOT contain getter syntax
        assert!(
            !zod.contains("get "),
            "Non-recursive enum should not use getter syntax. Got: {zod}"
        );
    }

    /// Test 6: Non-recursive struct should not use getter syntax
    #[test]
    fn test_non_recursive_struct_no_getter() {
        let zod = SimpleStructJson::zod_schema();
        println!("Generated Zod:\n{}", zod);

        // Should NOT contain getter syntax
        assert!(
            !zod.contains("get "),
            "Non-recursive struct should not use getter syntax. Got: {zod}"
        );
    }

    /// Test 7: Struct referencing other types (not self) should not use getter
    #[test]
    fn test_struct_with_sibling_type_no_getter() {
        let zod = PersonJson::zod_schema();
        println!("Generated Zod:\n{}", zod);

        // Should NOT contain getter syntax (Address is a different type, not self)
        assert!(
            !zod.contains("get "),
            "Struct with sibling type reference should not use getter syntax. Got: {zod}"
        );
    }

    /// Test 8: Named struct variant with recursive field
    #[test]
    fn test_recursive_named_struct_variant() {
        let zod = TreeEnumJson::zod_schema();
        println!("Generated Zod:\n{}", zod);

        // Should contain getter syntax for nodes field in Branch variant
        assert!(
            zod.contains("get nodes()"),
            "Recursive nodes field in named variant should use getter syntax. Got: {zod}"
        );

        // Leaf variant should NOT use getter
        assert!(
            zod.contains("text: z.string()"),
            "Non-recursive text field should use normal syntax. Got: {zod}"
        );
    }
}
