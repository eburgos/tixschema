/// What a self-referential type describes as on the JSON-schema and TypeScript surfaces.
///
/// Each description runs in a child copy of this test binary: a description that doesn't
/// terminate overflows the stack, and a stack overflow aborts the process with nothing to catch.
/// Running it in a child turns that abort into an exit status the parent asserts on, so a
/// regression fails one test instead of killing the suite.
#[cfg(feature = "jsonschema")]
mod describes {
    use super::{
        ChainNode, CycleA, DynamicValueTest, Holder, Nest, Ping, Pong, RecursiveMapEnum, Registry,
        TreeNode,
    };
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

    /// The deepest object or array nesting anywhere in a description. A type that names itself has
    /// a description whose depth is set by its own fields — one that terminated by unrolling
    /// itself a fixed number of times instead would be far deeper, which is the difference this
    /// measures.
    fn depth(value: &Value) -> usize {
        match value {
            Value::Object(members) => 1 + members.values().map(depth).max().unwrap_or_default(),
            Value::Array(items) => 1 + items.iter().map(depth).max().unwrap_or_default(),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => 0,
        }
    }

    /// The pointer every `$ref` anywhere in a description carries, in document order.
    fn references(value: &Value) -> Vec<String> {
        match value {
            Value::Object(members) => members
                .iter()
                .flat_map(|(key, member)| {
                    if key == "$ref" {
                        member.as_str().map(ToOwned::to_owned).into_iter().collect()
                    } else {
                        references(member)
                    }
                })
                .collect(),
            Value::Array(items) => items.iter().flat_map(references).collect(),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => Vec::new(),
        }
    }

    /// Walks every reference in a description against the description itself, which is what a
    /// validator reading it does. A pointer landing on nothing — or on the null a definition
    /// reserved but never filled would leave — is the dangling reference.
    fn every_reference_resolves(document: &Value) -> usize {
        let found = references(document);
        for reference in &found {
            let resolved = reference
                .strip_prefix('#')
                .and_then(|path| document.pointer(path))
                .filter(|target| !target.is_null());
            assert!(
                resolved.is_some(),
                "`{reference}` resolves to nothing in {document}"
            );
        }
        found.len()
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
            "mutual_ping_json" => serde_json::to_string(&Ping::json_schema()).unwrap(),
            "mutual_pong_json" => serde_json::to_string(&Pong::json_schema()).unwrap(),
            "mutual_ping_ts" => Ping::ts_definition(),
            "cycle_json" => serde_json::to_string(&CycleA::json_schema()).unwrap(),
            "held_json" => serde_json::to_string(&Holder::json_schema()).unwrap(),
            "registry_json" => serde_json::to_string(&Registry::json_schema()).unwrap(),
            "nested_json" => serde_json::to_string(&Nest::json_schema()).unwrap(),
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
            json!({ "anyOf": [{ "$ref": "#/$defs/ChainNode" }, { "type": "null" }] })
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

    /// Neither expansion can see the cycle: each is written before the other exists, and a type
    /// names the other by inlining it. Only the run knows it has come back around.
    #[test]
    fn two_types_naming_each_other_terminate_and_resolve() {
        for (description, def_name) in [("mutual_ping_json", "Ping"), ("mutual_pong_json", "Pong")]
        {
            let document: Value = serde_json::from_str(&produced(description)).unwrap();

            assert_eq!(document["$ref"], json!(format!("#/$defs/{def_name}")));
            assert!(
                every_reference_resolves(&document) >= 2,
                "the pair should name each other by reference: {document}"
            );
            assert!(
                depth(&document) <= 12,
                "unrolled rather than named: {document}"
            );
        }
    }

    /// A cycle longer than a pair closes just the same, and closes on the one name the run
    /// re-entered rather than on every name it passed through.
    #[test]
    fn a_three_type_cycle_terminates_and_resolves() {
        let document: Value = serde_json::from_str(&produced("cycle_json")).unwrap();

        assert_eq!(document["$ref"], json!("#/$defs/CycleA"));
        assert_eq!(
            document["$defs"].as_object().map(serde_json::Map::len),
            Some(1),
            "only the re-entered name needs a definition: {document}"
        );
        assert!(every_reference_resolves(&document) >= 1);
        assert!(
            depth(&document) <= 16,
            "unrolled rather than named: {document}"
        );
    }

    /// A recursive type carries references that are pointers from a document root. Held by another
    /// type, the root is the holder's, so that is where its definition has to be.
    #[test]
    fn a_recursive_type_held_by_another_resolves_in_the_holders_document() {
        let document: Value = serde_json::from_str(&produced("held_json")).unwrap();

        assert_eq!(document["type"], json!("object"));
        assert_eq!(
            document["properties"]["root"],
            json!({ "$ref": "#/$defs/Node" })
        );
        assert_eq!(document["required"], json!(["root"]));
        assert_eq!(
            document["$defs"]["Node"]["properties"]["children"],
            json!({ "type": "array", "items": { "$ref": "#/$defs/Node" } })
        );
        assert!(every_reference_resolves(&document) >= 2);
    }

    /// The definition is hoisted once however many positions name the type, and every position
    /// points at that one entry — a field, an array element, a map value.
    #[test]
    fn a_recursive_type_named_from_several_positions_is_hoisted_once() {
        let document: Value = serde_json::from_str(&produced("registry_json")).unwrap();

        let reference = json!({ "$ref": "#/$defs/Node" });
        assert_eq!(document["properties"]["primary"], reference);
        assert_eq!(document["properties"]["spares"]["items"], reference);
        assert_eq!(
            document["properties"]["by_name"]["additionalProperties"],
            reference
        );
        assert_eq!(
            document["$defs"].as_object().map(serde_json::Map::len),
            Some(1),
            "one name, one definition: {document}"
        );
        assert_eq!(
            document["$defs"]["Node"]["properties"]["val"],
            json!({ "type": "string" }),
            "the hoisted entry is the body, not the place held for it: {document}"
        );
        assert!(every_reference_resolves(&document) >= 4);
    }

    /// A recursive type holding another one puts two definitions at the same root, and the
    /// document is the reference into its own.
    #[test]
    fn a_recursive_type_holding_another_hoists_both_definitions() {
        let document: Value = serde_json::from_str(&produced("nested_json")).unwrap();

        assert_eq!(document["$ref"], json!("#/$defs/Nest"));
        assert_eq!(
            document["$defs"]["Nest"]["properties"]["inner"],
            json!({ "$ref": "#/$defs/Node" })
        );
        assert_eq!(
            document["$defs"]["Nest"]["properties"]["kids"],
            json!({ "type": "array", "items": { "$ref": "#/$defs/Nest" } })
        );
        assert!(every_reference_resolves(&document) >= 3);
    }

    #[test]
    fn typescript_names_the_type_it_is_defining() {
        let vec_self = produced("vec_self_ts");
        assert!(
            vec_self.contains("children: Array<TreeNode>;"),
            "a Vec of self is an array of the type's own name: {vec_self}"
        );

        let boxed_self = produced("boxed_self_ts");
        let next = "next: ChainNode | undefined;";
        assert!(
            boxed_self.contains(next),
            "an optional box of self is the type's own name: {boxed_self}"
        );

        let enum_self = produced("enum_self_ts");
        assert!(
            enum_self.contains("Array<DynamicValueTest>"),
            "a recursive variant names the enum: {enum_self}"
        );

        // The JSDoc embeds the JSON schema, so a pair that cannot be described takes the
        // TypeScript surface down with it.
        let mutual = produced("mutual_ping_ts");
        assert!(
            mutual.contains("pong: Array<Pong>;"),
            "a mutually recursive field names the other type: {mutual}"
        );
    }
}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tixschema::model_schema;

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

// The types below exist to be described as JSON schema and nothing else — the Zod and TypeScript
// surfaces they also carry are read off the types above. So they are written only where something
// asks them what they describe as.

/// One half of a pair that names the other half, written before that half exists.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Ping {
    pub pong: Vec<Pong>,
}

/// The other half, which names the first back.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Pong {
    pub ping: Vec<Ping>,
}

/// The head of a cycle three types long.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CycleA {
    pub b: CycleB,
}

#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CycleB {
    pub c: CycleC,
}

#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CycleC {
    pub a: Vec<CycleA>,
}

/// A type that names itself, held by types that are not recursive themselves.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Node {
    pub children: Vec<Self>,
    pub val: String,
}

/// Holds one recursive type in one field.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Holder {
    pub root: Node,
}

/// Names the same recursive type from a field, an array element, and a map value.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Registry {
    pub by_name: HashMap<String, Node>,
    pub primary: Node,
    pub spares: Vec<Node>,
}

/// Names itself and another recursive type, so one document holds both definitions.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Nest {
    pub inner: Node,
    pub kids: Vec<Self>,
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

/// Two non-generic structs cycling through a `Vec` — `Chapter` names `Section` before `Section`
/// is declared, and `Section` names `Chapter` back afterward. Legal Rust, no `Box` needed, since
/// the cycle runs through a collection on both sides; the zero-argument counterpart of
/// `CycleLeader`/`CycleFollower` in `generic_types_tests`.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Chapter {
    pub sections: Vec<Section>,
}

/// Declared BELOW `Chapter`, which names it first — the forward half of the cycle.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Section {
    pub back_refs: Vec<Chapter>,
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

/// `TreeEnum`'s adjacent twin: the same recursive named variant, but nested under a content key
/// rather than sitting beside the tag. The field-level getter has to keep deferring once nested, or
/// the content object reads a binding at its own initializer.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "value")]
pub enum TreeEnumAdjacent {
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

/// Test 1: Recursive enum with `Vec` of self.
#[test]
fn test_recursive_enum_with_vec() {
    let zod = RecursiveVecEnum::zod_schema();

    assert!(
        zod.contains("get value()"),
        "Recursive Array variant should use getter syntax. Got: {zod}"
    );

    assert!(
        zod.contains("value: z.string()"),
        "Non-recursive Text variant should use normal property syntax. Got: {zod}"
    );
}

/// Test 2: Recursive enum with `HashMap` of self.
#[test]
fn test_recursive_enum_with_hashmap() {
    let zod = RecursiveMapEnum::zod_schema();

    assert!(
        zod.contains("get value()"),
        "Recursive Object variant should use getter syntax. Got: {zod}"
    );

    assert!(
        zod.contains("value: z.number()"),
        "Non-recursive Number variant should use normal property syntax. Got: {zod}"
    );
}

/// Test 3: Recursive struct with `Vec` of self.
#[test]
fn test_recursive_struct() {
    let zod = TreeNode::zod_schema();

    assert!(
        zod.contains("get children()"),
        "Recursive children field should use getter syntax. Got: {zod}"
    );

    assert!(
        zod.contains("val: z.string()"),
        "Non-recursive val field should use normal property syntax. Got: {zod}"
    );
}

/// Test 4: Complex `DynamicValue`-like enum with multiple recursive variants.
#[test]
fn test_complex_dynamic_value() {
    let zod = DynamicValueTest::zod_schema();

    let getter_count = zod.matches("get value()").count();
    assert_eq!(
        getter_count, 2,
        "Should have exactly 2 getter properties (array and object). Got: {getter_count}. Schema: {zod}"
    );

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

    assert!(
        !zod.contains("get "),
        "Non-recursive enum should not use getter syntax. Got: {zod}"
    );
}

/// Test 6: Non-recursive struct should not use getter syntax.
#[test]
fn test_non_recursive_struct_no_getter() {
    let zod = SimpleStruct::zod_schema();

    assert!(
        !zod.contains("get "),
        "Non-recursive struct should not use getter syntax. Got: {zod}"
    );
}

/// Test 7: Struct referencing other types (not self) should not use getter.
#[test]
fn test_struct_with_sibling_type_no_getter() {
    let zod = Person::zod_schema();

    assert!(
        !zod.contains("get "),
        "Struct with sibling type reference should not use getter syntax. Got: {zod}"
    );
}

/// Test 8: Named struct variant with recursive field.
#[test]
fn test_recursive_named_struct_variant() {
    let zod = TreeEnum::zod_schema();

    assert!(
        zod.contains("get nodes()"),
        "Recursive nodes field in named variant should use getter syntax. Got: {zod}"
    );

    assert!(
        zod.contains("text: z.string()"),
        "Non-recursive text field should use normal syntax. Got: {zod}"
    );
}

/// Test 8b: the same recursive field, now nested under an adjacent form's content key. The field's
/// own getter still defers the reference; the content key needs none — a second getter would be
/// wrong precedent (see `render_external_variant`'s `defer_key`, never set for a `Named` variant).
#[test]
fn test_recursive_named_struct_variant_adjacent() {
    let zod = TreeEnumAdjacent::zod_schema();

    assert!(
        zod.contains("get nodes() { return z.array(TreeEnumAdjacent$Schema); },"),
        "Got: {zod}"
    );
    assert!(zod.contains("text: z.string()"), "Got: {zod}");
    assert!(
        zod.contains("value: z.strictObject({"),
        "The content key holds the field-level getter directly; it needs none of its own. Got: {zod}"
    );
    assert!(!zod.contains("get value()"), "Got: {zod}");
    assert!(!zod.contains("get \"value\""), "Got: {zod}");
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

/// Test 10: a non-generic forward reference — a bare, zero-argument sibling declared BELOW the
/// type naming it — has to defer exactly as a generic forward reference does, or the module
/// throws at import in every concatenation order: whichever half lands first names a `const` the
/// other has not published yet.
#[test]
fn test_non_generic_forward_reference_defers_through_a_getter() {
    let chapter_zod = Chapter::zod_schema();
    assert!(
        chapter_zod.contains("get sections() { return z.array(Section$Schema); },"),
        "The forward reference to Section should defer through a getter. Got: {chapter_zod}"
    );

    // The backward half — Section naming Chapter, already registered by the time Section is
    // expanded — stays eager exactly as every non-cyclic sibling reference already does.
    let section_zod = Section::zod_schema();
    assert!(
        section_zod.contains("back_refs: z.array(Chapter$Schema),"),
        "The backward reference to Chapter should stay eager. Got: {section_zod}"
    );
    assert!(!section_zod.contains("get "), "Got: {section_zod}");
}
