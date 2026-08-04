use serde::{Deserialize, Serialize};
use tixschema::model_schema;

/// A documented struct.
/// ```rust example
/// let item = DocumentedStruct {
///     label: "alpha".to_string(),
/// };
/// println!("Item: {:?}", item);
/// ```
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub struct DocumentedStruct {
    pub label: String,
}

/// A documented tuple struct.
/// ```rust example
/// let item = DocumentedTuple("alpha".to_string(), 7);
/// println!("Item: {:?}", item);
/// ```
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub struct DocumentedTuple(pub String, pub u32);

/// A documented adjacently tagged enum.
/// ```rust example
/// let item = DocumentedAdjacent::Unit;
/// println!("Item: {:?}", item);
/// ```
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "kind", content = "payload")]
pub enum DocumentedAdjacent {
    Named { side: u8 },
    Unit,
}

/// A documented externally tagged enum.
/// ```rust example
/// let item = DocumentedExternal::Unit;
/// println!("Item: {:?}", item);
/// ```
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub enum DocumentedExternal {
    Named { side: u8 },
    Unit,
}

/// A documented internally tagged enum.
/// ```rust example
/// let item = DocumentedInternal::Unit;
/// println!("Item: {:?}", item);
/// ```
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum DocumentedInternal {
    Named { side: u8 },
    Unit,
}

/// A documented untagged enum.
/// ```rust example
/// let item = DocumentedUntagged::Count(3);
/// println!("Item: {:?}", item);
/// ```
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum DocumentedUntagged {
    Count(u32),
    Label(String),
}

/// A documented plain enum.
/// ```rust example
/// let item = DocumentedSlot::Primary;
/// println!("Item: {:?}", item);
/// ```
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum DocumentedSlot {
    Primary,
    Secondary,
}

/// Every documented shape above, paired with the description its `JSDoc` is left with once the
/// example is dropped. The plain enum rides along as the shape that already dropped it.
fn documented_shapes() -> Vec<(String, &'static str)> {
    vec![
        (DocumentedStruct::ts_definition(), "A documented struct."),
        (
            DocumentedTuple::ts_definition(),
            "A documented tuple struct.",
        ),
        (
            DocumentedAdjacent::ts_definition(),
            "A documented adjacently tagged enum.",
        ),
        (
            DocumentedExternal::ts_definition(),
            "A documented externally tagged enum.",
        ),
        (
            DocumentedInternal::ts_definition(),
            "A documented internally tagged enum.",
        ),
        (
            DocumentedUntagged::ts_definition(),
            "A documented untagged enum.",
        ),
        (DocumentedSlot::ts_definition(), "A documented plain enum."),
    ]
}

/// The reported failure: a struct's `JSDoc` opened with the fence and the Rust body under it, so
/// neither the fence nor anything written inside it may reach the emitted `TypeScript`.
#[test]
fn a_documented_item_never_carries_its_rust_example_into_the_emitted_typescript() {
    for (ts, description) in documented_shapes() {
        for leaked in ["```", "let item =", "println!"] {
            assert!(!ts.contains(leaked), "{leaked} reached: {ts}");
        }
        assert!(ts.contains(description), "docs dropped from: {ts}");
    }
}

/// The example was the only thing separating the shapes, so with it gone every one of them opens
/// its `JSDoc` the same way: the description, then the blank continuation line.
#[test]
fn every_documented_shape_opens_its_jsdoc_identically() {
    for (ts, description) in documented_shapes() {
        let header = format!("/**\n * {description}\n * \n");
        assert!(
            ts.starts_with(&header),
            "expected {description} to open: {ts}"
        );
    }
}
