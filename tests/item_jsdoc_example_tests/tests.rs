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

/// ```rust example
/// let item = ExampleOnlyStruct {
///     label: "alpha".to_string(),
/// };
/// println!("Item: {:?}", item);
/// ```
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub struct ExampleOnlyStruct {
    /// ```rust example
    /// let label = "alpha".to_string();
    /// println!("Label: {}", label);
    /// ```
    pub label: String,
}

// An internally tagged enum whose one documented member says nothing but an example. Commented with
// `//`, so the item itself is held against its twin as the undocumented one it is.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum ExampleOnlyVariantHolder {
    /// ```rust example
    /// let item = ExampleOnlyVariantHolder::Unit;
    /// println!("Item: {:?}", item);
    /// ```
    Named {
        side: u8,
    },
    Unit,
}

/// ```rust example
/// let item: ExampleOnlyAlias = 3;
/// println!("Item: {:?}", item);
/// ```
#[model_schema()]
pub type ExampleOnlyAlias = u32;

/// ```rust example
/// let item = ExampleOnlySlot::Primary;
/// println!("Item: {:?}", item);
/// ```
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum ExampleOnlySlot {
    Primary,
    Secondary,
}

/// ```rust example
/// let item = ExampleOnlyBrand("alpha".to_string());
/// println!("Item: {:?}", item);
/// ```
// A brand publishes a description and no `JSDoc` header of its own, so it is read on the Zod
// surface alone and is gated on the feature that surface is.
#[cfg(feature = "zod")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct ExampleOnlyBrand(pub String);

// The undocumented twins of the shapes above. Dropping an example is the whole of what the strip
// does, so a shape documented with nothing else is left exactly where a shape documented with
// nothing at all already stands — which is what the twins below are held against.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub struct UndocumentedStruct {
    pub label: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum UndocumentedVariantHolder {
    Named { side: u8 },
    Unit,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum UndocumentedSlot {
    Primary,
    Secondary,
}

#[cfg(feature = "zod")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct UndocumentedBrand(pub String);

#[model_schema()]
pub type UndocumentedAlias = u32;

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

/// Every shape documented with nothing but an example, paired with the undocumented twin it is
/// held against: the `TypeScript` one writes is the other's with the fixture's own name in it.
fn example_only_twins() -> Vec<(String, String, &'static str, &'static str)> {
    vec![
        (
            ExampleOnlyStruct::ts_definition(),
            UndocumentedStruct::ts_definition(),
            "ExampleOnlyStruct",
            "UndocumentedStruct",
        ),
        (
            ExampleOnlyVariantHolder::ts_definition(),
            UndocumentedVariantHolder::ts_definition(),
            "ExampleOnlyVariantHolder",
            "UndocumentedVariantHolder",
        ),
        (
            ExampleOnlySlot::ts_definition(),
            UndocumentedSlot::ts_definition(),
            "ExampleOnlySlot",
            "UndocumentedSlot",
        ),
        (
            example_only_alias_schema::Schema::ts_definition(),
            undocumented_alias_schema::Schema::ts_definition(),
            "ExampleOnlyAlias",
            "UndocumentedAlias",
        ),
    ]
}

/// The reported failure: an item or member documented with nothing but an example emitted an
/// empty `JSDoc` body, where an undocumented one names what it's documenting. What the strip
/// leaves empty falls back to the name, so the shape becomes the undocumented one, byte for byte.
#[test]
fn an_example_only_shape_writes_what_its_undocumented_twin_writes() {
    // The aliases publish `ts_definition` from their own modules rather than from the alias, so
    // naming the types here is what keeps the fixtures from being pruned as unused.
    let aliased: ExampleOnlyAlias = 3;
    let aliased_twin: UndocumentedAlias = 3;
    assert_eq!(aliased, aliased_twin);

    for (example_only, twin, named, twin_named) in example_only_twins() {
        assert_eq!(example_only.replace(named, twin_named), twin, "for {named}");
    }
}

/// What the fallback writes, read off the emitted `TypeScript` rather than off the twin: the item
/// names itself as it is exported, and each member names itself as it is serialized.
#[test]
fn an_example_only_item_and_member_name_themselves() {
    let ts = ExampleOnlyStruct::ts_definition();
    assert!(
        ts.starts_with("/**\n * ExampleOnlyStruct\n * \n"),
        "item did not name itself: {ts}"
    );
    assert!(
        ts.contains("  /**\n   * label\n   * \n"),
        "field unnamed: {ts}"
    );

    let variants = ExampleOnlyVariantHolder::ts_definition();
    assert!(
        variants.contains("  /**\n   * Named\n   * \n"),
        "variant unnamed: {variants}"
    );

    let alias = example_only_alias_schema::Schema::ts_definition();
    assert!(
        alias.starts_with("/**\n * ExampleOnlyAliasType\n * \n"),
        "alias did not name itself: {alias}"
    );
}

/// The `description` a shape publishes is spelled from the lines its `JSDoc` body is spelled from,
/// so the fallback fires on the same reading at both. The two shapes that publish one are covered:
/// a plain enum writes it beside a header of its own, and a brand writes it instead of one.
#[cfg(feature = "zod")]
#[test]
fn an_example_only_shape_describes_itself_as_its_undocumented_twin_does() {
    for (zod, exported) in [
        (ExampleOnlySlot::zod_schema(), "ExampleOnlySlot"),
        (UndocumentedSlot::zod_schema(), "UndocumentedSlot"),
        (ExampleOnlyBrand::zod_schema(), "ExampleOnlyBrand"),
        (UndocumentedBrand::zod_schema(), "UndocumentedBrand"),
    ] {
        assert!(
            zod.contains(&format!("description: \"{exported}\"")),
            "{exported} did not describe itself: {zod}"
        );
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
