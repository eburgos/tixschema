use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tixschema::model_schema;

// ========== Type definitions at module level ==========

/// Address struct for sibling reference test.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Address {
    pub city: String,
    pub street: String,
}

/// Complex `DynamicValue`-like enum with multiple recursive variants.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "value")]
pub enum DynamicValueTest {
    #[serde(rename = "array")]
    Array(Vec<Self>),
    #[serde(rename = "boolean")]
    Bool(bool),
    #[serde(rename = "decimal")]
    Decimal(f64),
    #[serde(rename = "integer")]
    Integer(i64),
    #[serde(rename = "object")]
    Object(HashMap<String, Self>),
    #[serde(rename = "string")]
    String(String),
}

/// Person struct referencing `Address` (not recursive).
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Person {
    pub address: Address,
    pub name: String,
}

/// Recursive enum with `HashMap` of self.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "value")]
pub enum RecursiveMapEnum {
    Number(i64),
    Object(HashMap<String, Self>),
}

/// Recursive enum with `Vec` of self.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "value")]
pub enum RecursiveVecEnum {
    Array(Vec<Self>),
    Text(String),
}

/// Non-recursive enum for comparison.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "value")]
pub enum SimpleEnum {
    Flag(bool),
    Number(i64),
    Text(String),
}

/// Non-recursive struct for comparison.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SimpleStruct {
    pub active: bool,
    pub age: u32,
    pub name: String,
}

/// Recursive enum with named struct variant.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum TreeEnum {
    Branch { nodes: Vec<Self> },
    Leaf { text: String },
}

/// Recursive struct with `Vec` of self.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TreeNode {
    pub children: Vec<Self>,
    pub val: String,
}

// ========== Tests ==========

/// Test 1: Recursive enum with `Vec` of self.
#[test]
fn test_recursive_enum_with_vec() {
    let zod = RecursiveVecEnum::zod_schema();

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

/// Test 2: Recursive enum with `HashMap` of self.
#[test]
fn test_recursive_enum_with_hashmap() {
    let zod = RecursiveMapEnum::zod_schema();

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

/// Test 3: Recursive struct with `Vec` of self.
#[test]
fn test_recursive_struct() {
    let zod = TreeNode::zod_schema();

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

/// Test 4: Complex `DynamicValue`-like enum with multiple recursive variants.
#[test]
fn test_complex_dynamic_value() {
    let zod = DynamicValueTest::zod_schema();

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

/// Test 5: Non-recursive types should not use getter syntax.
#[test]
fn test_non_recursive_enum_no_getter() {
    let zod = SimpleEnum::zod_schema();

    // Should NOT contain getter syntax
    assert!(
        !zod.contains("get "),
        "Non-recursive enum should not use getter syntax. Got: {zod}"
    );
}

/// Test 6: Non-recursive struct should not use getter syntax.
#[test]
fn test_non_recursive_struct_no_getter() {
    let zod = SimpleStruct::zod_schema();

    // Should NOT contain getter syntax
    assert!(
        !zod.contains("get "),
        "Non-recursive struct should not use getter syntax. Got: {zod}"
    );
}

/// Test 7: Struct referencing other types (not self) should not use getter.
#[test]
fn test_struct_with_sibling_type_no_getter() {
    let zod = Person::zod_schema();

    // Should NOT contain getter syntax (Address is a different type, not self)
    assert!(
        !zod.contains("get "),
        "Struct with sibling type reference should not use getter syntax. Got: {zod}"
    );
}

/// Test 8: Named struct variant with recursive field.
#[test]
fn test_recursive_named_struct_variant() {
    let zod = TreeEnum::zod_schema();

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
