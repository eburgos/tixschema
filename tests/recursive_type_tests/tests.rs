/// What a self-referential type describes as on the JSON-schema and TypeScript surfaces.
///
/// Each description is produced in a child copy of this test binary. A description that does not
/// terminate overflows the stack, and a stack overflow aborts the process rather than unwinding —
/// there is nothing to catch where it happens. Running it in a child turns that abort into an exit
/// status the parent asserts on, so a regression fails one test instead of killing the suite.
#[cfg(feature = "jsonschema")]
mod describes {
    use super::{ChainNode, DynamicValueTest, RecursiveMapEnum, TreeNode};
    use serde_json::{Value, json};
    use std::env;
    use std::process::Command;
    use std::thread;

    /// Selects the description a child run produces.
    const DESCRIPTION_VAR: &str = "TIXSCHEMA_RECURSIVE_DESCRIPTION";

    /// Opens the child's stderr report; everything after it is the description, newlines and all.
    const PRODUCED_MARKER: &str = "--- produced ---\n";

    /// The stack a description is produced on. Small enough that one that does not terminate dies
    /// at once instead of working its way through the default stack.
    const PRODUCTION_STACK_BYTES: usize = 256 * 1024;

    /// The libtest path of [`production`], which the parent names to run only it in the child.
    const CHILD_TEST: &str = "tests::describes::production";

    /// The deepest object or array nesting anywhere in a description.
    ///
    /// A type that names itself has a description whose depth is set by its own fields. One that
    /// terminated by unrolling itself a fixed number of times instead would be far deeper, which
    /// is the difference this measures.
    fn depth(value: &Value) -> usize {
        match value {
            Value::Object(members) => 1 + members.values().map(depth).max().unwrap_or_default(),
            Value::Array(items) => 1 + items.iter().map(depth).max().unwrap_or_default(),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => 0,
        }
    }

    fn produce(description: &str) -> String {
        match description {
            "vec_self_json" => serde_json::to_string(&TreeNode::json_schema()).unwrap(),
            "vec_self_ts" => TreeNode::ts_definition(),
            "boxed_self_json" => serde_json::to_string(&ChainNode::json_schema()).unwrap(),
            "boxed_self_ts" => ChainNode::ts_definition(),
            "map_self_json" => serde_json::to_string(&RecursiveMapEnum::json_schema()).unwrap(),
            "enum_self_json" => serde_json::to_string(&DynamicValueTest::json_schema()).unwrap(),
            "enum_self_ts" => DynamicValueTest::ts_definition(),
            other => format!("no description named `{other}`"),
        }
    }

    /// Produces the one description a parent run asked for. Without [`DESCRIPTION_VAR`] this is
    /// not a child run and there is nothing to produce.
    #[test]
    fn production() {
        let Ok(description) = env::var(DESCRIPTION_VAR) else {
            return;
        };
        let produced = thread::Builder::new()
            .stack_size(PRODUCTION_STACK_BYTES)
            .spawn(move || produce(&description))
            .unwrap()
            .join()
            .unwrap();
        eprint!("{PRODUCED_MARKER}{produced}");
    }

    fn produced(description: &str) -> String {
        let child = Command::new(env::current_exe().unwrap())
            .args([CHILD_TEST, "--exact", "--nocapture", "--test-threads=1"])
            .env(DESCRIPTION_VAR, description)
            .output()
            .unwrap();
        let reported = String::from_utf8_lossy(&child.stderr).into_owned();
        assert!(
            child.status.success(),
            "describing `{description}` did not terminate ({status}):\n{reported}",
            status = child.status
        );
        assert!(
            reported.contains(PRODUCED_MARKER),
            "describing `{description}` produced nothing:\n{reported}"
        );
        reported.split_once(PRODUCED_MARKER).unwrap().1.to_owned()
    }

    #[test]
    fn vec_of_self_is_a_reference_into_the_documents_own_defs() {
        let document: Value = serde_json::from_str(&produced("vec_self_json")).unwrap();

        assert_eq!(document["$ref"], json!("#/$defs/TreeNode"));
        let body = &document["$defs"]["TreeNode"];
        assert_eq!(
            body["properties"]["children"],
            json!({ "type": "array", "items": { "$ref": "#/$defs/TreeNode" } })
        );
        assert_eq!(body["properties"]["val"], json!({ "type": "string" }));
        assert_eq!(body["required"], json!(["children", "val"]));
        assert!(
            depth(&document) <= 8,
            "unrolled rather than named: {document}"
        );
    }

    #[test]
    fn boxed_option_of_self_is_a_reference_into_the_documents_own_defs() {
        let document: Value = serde_json::from_str(&produced("boxed_self_json")).unwrap();

        assert_eq!(document["$ref"], json!("#/$defs/ChainNode"));
        let body = &document["$defs"]["ChainNode"];
        assert_eq!(
            body["properties"]["next"],
            json!({ "$ref": "#/$defs/ChainNode" })
        );
        assert_eq!(body["properties"]["label"], json!({ "type": "string" }));
        assert_eq!(body["required"], json!(["label"]));
        assert!(
            depth(&document) <= 8,
            "unrolled rather than named: {document}"
        );
    }

    #[test]
    fn a_self_holding_map_value_is_a_reference() {
        let document: Value = serde_json::from_str(&produced("map_self_json")).unwrap();

        assert_eq!(document["$ref"], json!("#/$defs/RecursiveMapEnum"));
        let variants = document["$defs"]["RecursiveMapEnum"]["oneOf"]
            .as_array()
            .unwrap();
        let described = serde_json::to_string(variants).unwrap();
        assert!(
            described.contains(r##""additionalProperties":{"$ref":"#/$defs/RecursiveMapEnum"}"##),
            "the map's values should be described by reference: {described}"
        );
    }

    #[test]
    fn an_enum_naming_itself_from_several_variants_is_described_once() {
        let document: Value = serde_json::from_str(&produced("enum_self_json")).unwrap();

        assert_eq!(document["$ref"], json!("#/$defs/DynamicValueTest"));
        let described = serde_json::to_string(&document).unwrap();
        assert_eq!(
            described
                .matches(r##"{"$ref":"#/$defs/DynamicValueTest"}"##)
                .count(),
            2,
            "the array element and the map value each name the enum: {described}"
        );
        assert!(
            depth(&document) <= 10,
            "unrolled rather than named: {document}"
        );
    }

    #[test]
    fn typescript_names_the_type_it_is_defining() {
        let vec_self = produced("vec_self_ts");
        assert!(
            vec_self.contains("children: Array<TreeNode>;"),
            "a Vec of self is an array of the type's own name: {vec_self}"
        );

        let boxed_self = produced("boxed_self_ts");
        assert!(
            boxed_self.contains("next: ChainNode | undefined;"),
            "an optional box of self is the type's own name: {boxed_self}"
        );

        let enum_self = produced("enum_self_ts");
        assert!(
            enum_self.contains("Array<DynamicValueTest>"),
            "a recursive variant names the enum: {enum_self}"
        );
    }
}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tixschema::model_schema;

// ========== Type definitions at module level ==========

/// Recursive struct holding at most one of itself.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChainNode {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<Box<Self>>,
}

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

/// Test 9: Struct holding at most one of itself.
#[test]
fn test_recursive_boxed_option_struct() {
    let zod = ChainNode::zod_schema();

    assert!(
        zod.contains("get next()"),
        "Recursive next field should use getter syntax. Got: {zod}"
    );
    assert!(
        zod.contains("label: z.string()"),
        "Non-recursive label field should use normal property syntax. Got: {zod}"
    );
}
