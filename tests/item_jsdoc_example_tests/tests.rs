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

/// A documented alias.
/// ```rust example
/// let item: DocumentedAlias = 3;
/// println!("Item: {:?}", item);
/// ```
#[model_schema()]
pub type DocumentedAlias = u32;

/// A struct whose documented member is a field.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub struct DocumentedFieldHolder {
    /// A documented field.
    /// ```rust example
    /// let item = DocumentedFieldHolder {
    ///     label: "alpha".to_string(),
    /// };
    /// println!("Item: {:?}", item);
    /// ```
    pub label: String,
}

/// An internally tagged enum whose documented member is a variant.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum DocumentedInternalVariant {
    /// A documented variant.
    /// ```rust example
    /// let item = DocumentedInternalVariant::Unit;
    /// println!("Item: {:?}", item);
    /// ```
    Named {
        side: u8,
    },
    Unit,
}

/// An externally tagged enum whose documented member is a variant.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub enum DocumentedExternalVariant {
    /// A documented variant.
    /// ```rust example
    /// let item = DocumentedExternalVariant::Unit;
    /// println!("Item: {:?}", item);
    /// ```
    Named {
        side: u8,
    },
    Unit,
}

/// A plain enum whose documented member is a variant, commented inside the union rather than over
/// a property — the one member body not written as ` * ` lines.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum DocumentedSlotVariant {
    /// A documented slot.
    /// ```rust example
    /// let item = DocumentedSlotVariant::Primary;
    /// println!("Item: {:?}", item);
    /// ```
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
        (
            documented_alias_schema::Schema::ts_definition(),
            "A documented alias.",
        ),
    ]
}

/// Every shape whose example is written on a member rather than on the type, paired with the
/// description that member's `JSDoc` is left with once the example is dropped.
fn documented_members() -> Vec<(String, &'static str)> {
    vec![
        (
            DocumentedFieldHolder::ts_definition(),
            "A documented field.",
        ),
        (
            DocumentedInternalVariant::ts_definition(),
            "A documented variant.",
        ),
        (
            DocumentedExternalVariant::ts_definition(),
            "A documented variant.",
        ),
        (DocumentedSlotVariant::ts_definition(), "A documented slot."),
    ]
}

/// The reported failure: a struct's `JSDoc` opened with the fence and the Rust body under it, so
/// neither the fence nor anything written inside it may reach the emitted `TypeScript`.
#[test]
fn a_documented_item_never_carries_its_rust_example_into_the_emitted_typescript() {
    // The alias publishes `ts_definition` from its own module rather than from the alias, so naming
    // the type here is what keeps the fixture from being pruned as unused.
    let aliased: DocumentedAlias = 3;
    assert_eq!(aliased, 3);

    for (ts, description) in documented_shapes() {
        for leaked in ["```", "let item", "println!"] {
            assert!(!ts.contains(leaked), "{leaked} reached: {ts}");
        }
        assert!(ts.contains(description), "docs dropped from: {ts}");
    }
}

/// The rule is the doc body's, not the type's: a field's and a variant's `JSDoc` are written from
/// the same lines an item's is, so an example written on a member is dropped where an example
/// written on the type is, and the description under it is what survives.
#[test]
fn a_documented_member_never_carries_its_rust_example_into_the_emitted_typescript() {
    for (ts, description) in documented_members() {
        for leaked in ["```", "let item", "println!"] {
            assert!(!ts.contains(leaked), "{leaked} reached: {ts}");
        }
        assert!(ts.contains(description), "docs dropped from: {ts}");
    }
}

/// Only the `JSDoc` side of an alias changed. What its example reaches on the Zod side is what it
/// always reached: nothing — an alias publishes no `example` field to drop it into.
#[cfg(feature = "zod")]
#[test]
fn a_documented_alias_publishes_the_same_zod_schema_it_always_did() {
    let zod = documented_alias_schema::Schema::zod_schema();
    for leaked in ["```", "let item", "println!", "example"] {
        assert!(!zod.contains(leaked), "{leaked} reached: {zod}");
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
