use serde::{Deserialize, Serialize};
use tixschema::model_schema;

#[model_schema(name = "RenamedItem")]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ItemUnderRustName {
    pub count: u32,
    pub label: String,
}

#[model_schema(name = "RenamedPair")]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PairUnderRustName(pub String, pub u32);

#[model_schema(name = "RenamedBrand")]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(transparent)]
pub struct BrandUnderRustName(pub String);

#[model_schema(name = "RenamedSlot")]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotUnderRustName {
    Primary,
    Secondary,
}

#[model_schema(name = "RenamedShape")]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ShapeUnderRustName {
    Named { side: u8 },
    Unit,
}

/// A renamed type that names itself: the recursion is what forces the JSON surface to publish a
/// definition and point at it, which is the only place the definition's name is written out.
#[model_schema(name = "RenamedTree")]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TreeUnderRustName {
    pub children: Vec<Self>,
    pub label: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReferencesRenamedItems {
    pub brand: BrandUnderRustName,
    pub item: ItemUnderRustName,
    pub pair: PairUnderRustName,
    pub shape: ShapeUnderRustName,
    pub slot: SlotUnderRustName,
    pub tree: TreeUnderRustName,
}

#[test]
fn every_renamed_shape_expands_in_this_feature_combination() {
    let value = ReferencesRenamedItems {
        brand: BrandUnderRustName("three".to_owned()),
        item: ItemUnderRustName {
            count: 1,
            label: "one".to_owned(),
        },
        pair: PairUnderRustName("two".to_owned(), 2),
        shape: ShapeUnderRustName::Named { side: 4 },
        slot: SlotUnderRustName::Primary,
        tree: TreeUnderRustName {
            children: vec![],
            label: "five".to_owned(),
        },
    };
    assert_eq!(value.item.count, 1);
    assert_eq!(value.pair.1, 2);
    assert_eq!(value.brand.0, "three");
    assert_eq!(value.slot, SlotUnderRustName::Primary);
    assert!(value.tree.children.is_empty());
}

#[cfg(feature = "typescript")]
#[test]
fn every_renamed_shape_is_written_under_the_override() {
    for (ts, declared) in [
        (
            ItemUnderRustName::ts_definition(),
            "export type RenamedItem",
        ),
        (
            PairUnderRustName::ts_definition(),
            "export type RenamedPair",
        ),
        (
            BrandUnderRustName::ts_definition(),
            "export type RenamedBrand",
        ),
        (
            SlotUnderRustName::ts_definition(),
            "export type RenamedSlot",
        ),
        (
            ShapeUnderRustName::ts_definition(),
            "export type RenamedShape",
        ),
    ] {
        assert!(ts.contains(declared), "{declared} missing from: {ts}");
    }
}

/// The brand is the one shape whose name reaches the surface twice: once as the exported type and
/// once as the brand tag the values carry, which is what makes two brands distinct in TypeScript.
#[cfg(all(feature = "typescript", feature = "zod"))]
#[test]
fn a_renamed_brand_is_tagged_by_the_override() {
    let ts = BrandUnderRustName::ts_definition();
    assert!(ts.contains("$brand<\"RenamedBrand\">"), "got: {ts}");
    let zod = BrandUnderRustName::zod_schema();
    assert!(zod.contains(".brand<\"RenamedBrand\">()"), "got: {zod}");
}

#[cfg(feature = "zod")]
#[test]
fn every_renamed_shape_publishes_its_zod_schema_under_the_override() {
    for (zod, declared) in [
        (
            ItemUnderRustName::zod_schema(),
            "export const RenamedItem$Schema",
        ),
        (
            PairUnderRustName::zod_schema(),
            "export const RenamedPair$Schema",
        ),
        (
            BrandUnderRustName::zod_schema(),
            "export const RenamedBrand$Schema",
        ),
        (
            SlotUnderRustName::zod_schema(),
            "export const RenamedSlot$Schema",
        ),
        (
            ShapeUnderRustName::zod_schema(),
            "export const RenamedShape$Schema",
        ),
    ] {
        assert!(zod.contains(declared), "{declared} missing from: {zod}");
    }
}

#[cfg(feature = "typescript")]
#[test]
fn a_reference_to_a_renamed_item_resolves_the_override() {
    let ts = ReferencesRenamedItems::ts_definition();
    for referenced in [
        "item: RenamedItem;",
        "pair: RenamedPair;",
        "brand: RenamedBrand;",
        "slot: RenamedSlot;",
        "shape: RenamedShape;",
        "tree: RenamedTree;",
    ] {
        assert!(ts.contains(referenced), "{referenced} missing from: {ts}");
    }
}

#[cfg(feature = "zod")]
#[test]
fn a_zod_reference_to_a_renamed_item_resolves_the_override() {
    let zod = ReferencesRenamedItems::zod_schema();
    for referenced in [
        "item: RenamedItem$Schema",
        "pair: RenamedPair$Schema",
        "brand: RenamedBrand$Schema",
        "slot: RenamedSlot$Schema",
        "shape: RenamedShape$Schema",
        "tree: RenamedTree$Schema",
    ] {
        assert!(zod.contains(referenced), "{referenced} missing from: {zod}");
    }
}

/// The reported failure was a rename that reached no surface, leaving every reference spelled from
/// the Rust ident. A referencing type carries nothing but references, so the Rust names of the
/// types it names must not appear in what it publishes.
#[cfg(any(feature = "typescript", feature = "zod"))]
#[test]
fn a_reference_to_a_renamed_item_never_carries_its_rust_name() {
    #[cfg(feature = "typescript")]
    let ts = ReferencesRenamedItems::ts_definition();
    #[cfg(not(feature = "typescript"))]
    let ts = String::new();
    #[cfg(feature = "zod")]
    let zod = ReferencesRenamedItems::zod_schema();
    #[cfg(not(feature = "zod"))]
    let zod = String::new();
    for surface in [&ts, &zod] {
        assert!(
            !surface.contains("UnderRustName"),
            "rust name reached: {surface}"
        );
    }
}

#[cfg(feature = "jsonschema")]
#[test]
fn a_renamed_item_names_its_json_definition_by_the_override() {
    let schema = TreeUnderRustName::json_schema();
    let pointer = serde_json::json!({ "$ref": "#/$defs/RenamedTree" });
    assert_eq!(
        schema.get("$ref").unwrap(),
        &pointer["$ref"],
        "got: {schema}"
    );
    let definition = schema.get("$defs").unwrap().get("RenamedTree").unwrap();
    assert_eq!(
        definition
            .get("properties")
            .unwrap()
            .get("children")
            .unwrap()
            .get("items")
            .unwrap(),
        &pointer,
        "got: {schema}"
    );
}

/// The JSON reference is a Rust path to the renamed item's schema module, so this is the one
/// surface where the override has to reach the module name as well as the exported name.
#[cfg(feature = "jsonschema")]
#[test]
fn the_module_a_json_reference_resolves_to_is_the_one_the_override_named() {
    let referencing = ReferencesRenamedItems::json_schema();
    let properties = referencing.get("properties").unwrap();
    assert_eq!(
        properties.get("item").unwrap(),
        &renamed_item_schema::Schema::json_schema()
    );
    assert_eq!(
        properties.get("slot").unwrap(),
        &renamed_slot_schema::Schema::json_schema()
    );
    assert_eq!(
        properties.get("brand").unwrap(),
        &renamed_brand_schema::Schema::json_schema()
    );
}
