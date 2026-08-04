use serde::{Deserialize, Serialize};
use tixschema::model_schema;

#[cfg(feature = "typescript")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub struct PlainStruct {
    pub label: String,
}

#[cfg(feature = "typescript")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub struct SuffixedStructJson {
    pub label: String,
}

#[cfg(feature = "typescript")]
#[model_schema(name = "RenamedStruct")]
#[derive(Serialize, Deserialize, Debug)]
pub struct StructUnderRustName {
    pub label: String,
}

/// A documented struct.
#[cfg(feature = "typescript")]
#[model_schema(name = "RenamedDocStruct")]
#[derive(Serialize, Deserialize, Debug)]
pub struct DocStructUnderRustName {
    pub label: String,
}

#[cfg(feature = "typescript")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub struct PlainTuple(pub String, pub u32);

#[cfg(feature = "typescript")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub struct SuffixedTupleJson(pub String, pub u32);

#[cfg(feature = "typescript")]
#[model_schema(name = "RenamedTuple")]
#[derive(Serialize, Deserialize, Debug)]
pub struct TupleUnderRustName(pub String, pub u32);

#[cfg(any(feature = "typescript", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum PlainSlot {
    Primary,
    Secondary,
}

#[cfg(any(feature = "typescript", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum SuffixedSlotJson {
    Primary,
    Secondary,
}

#[cfg(any(feature = "typescript", feature = "zod"))]
#[model_schema(name = "RenamedSlot")]
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum SlotUnderRustName {
    Primary,
    Secondary,
}

#[cfg(feature = "typescript")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub enum PlainShape {
    Named { side: u8 },
    Unit,
}

#[cfg(feature = "typescript")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub enum SuffixedShapeJson {
    Named { side: u8 },
    Unit,
}

#[cfg(feature = "typescript")]
#[model_schema(name = "RenamedShape")]
#[derive(Serialize, Deserialize, Debug)]
pub enum ShapeUnderRustName {
    Named { side: u8 },
    Unit,
}

#[cfg(feature = "typescript")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "kind", content = "payload")]
pub enum PlainAdjacent {
    Named { side: u8 },
    Unit,
}

#[cfg(feature = "typescript")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "kind", content = "payload")]
pub enum SuffixedAdjacentJson {
    Named { side: u8 },
    Unit,
}

#[cfg(feature = "typescript")]
#[model_schema(name = "RenamedAdjacent")]
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "kind", content = "payload")]
pub enum AdjacentUnderRustName {
    Named { side: u8 },
    Unit,
}

#[cfg(feature = "typescript")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum PlainEither {
    Count(u32),
    Label(String),
}

#[cfg(feature = "typescript")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum SuffixedEitherJson {
    Count(u32),
    Label(String),
}

#[cfg(feature = "typescript")]
#[model_schema(name = "RenamedEither")]
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum EitherUnderRustName {
    Count(u32),
    Label(String),
}

#[cfg(feature = "zod")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct PlainBrand(pub String);

#[cfg(feature = "zod")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct SuffixedBrandJson(pub String);

#[cfg(feature = "zod")]
#[model_schema(name = "RenamedBrand")]
#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct BrandUnderRustName(pub String);

#[cfg(feature = "typescript")]
fn assert_jsdoc_opens_with(ts: &str, expected: &str) {
    let header = format!("/**\n * {expected}\n");
    assert!(ts.starts_with(&header), "expected {expected} to open: {ts}");
}

#[cfg(feature = "typescript")]
#[test]
fn an_undocumented_item_names_itself_in_jsdoc_as_it_is_exported() {
    for (ts, exported) in [
        (StructUnderRustName::ts_definition(), "RenamedStruct"),
        (TupleUnderRustName::ts_definition(), "RenamedTuple"),
        (SlotUnderRustName::ts_definition(), "RenamedSlot"),
        (ShapeUnderRustName::ts_definition(), "RenamedShape"),
        (AdjacentUnderRustName::ts_definition(), "RenamedAdjacent"),
        (EitherUnderRustName::ts_definition(), "RenamedEither"),
        (SuffixedStructJson::ts_definition(), "SuffixedStruct"),
        (SuffixedTupleJson::ts_definition(), "SuffixedTuple"),
        (SuffixedSlotJson::ts_definition(), "SuffixedSlot"),
        (SuffixedShapeJson::ts_definition(), "SuffixedShape"),
        (SuffixedAdjacentJson::ts_definition(), "SuffixedAdjacent"),
        (SuffixedEitherJson::ts_definition(), "SuffixedEither"),
    ] {
        assert_jsdoc_opens_with(&ts, exported);
        assert!(
            ts.contains(&format!("export type {exported} ")),
            "{exported} not exported by: {ts}"
        );
    }
}

/// The reported failure was a `JSDoc` header contradicting the `export type` one line under it, so
/// the name the item is declared under must reach neither.
#[cfg(feature = "typescript")]
#[test]
fn an_undocumented_item_never_writes_its_rust_ident() {
    for ts in [
        StructUnderRustName::ts_definition(),
        TupleUnderRustName::ts_definition(),
        SlotUnderRustName::ts_definition(),
        ShapeUnderRustName::ts_definition(),
        AdjacentUnderRustName::ts_definition(),
        EitherUnderRustName::ts_definition(),
    ] {
        assert!(!ts.contains("UnderRustName"), "rust ident reached: {ts}");
    }
    for (ts, ident) in [
        (SuffixedStructJson::ts_definition(), "SuffixedStructJson"),
        (SuffixedTupleJson::ts_definition(), "SuffixedTupleJson"),
        (SuffixedSlotJson::ts_definition(), "SuffixedSlotJson"),
        (SuffixedShapeJson::ts_definition(), "SuffixedShapeJson"),
        (
            SuffixedAdjacentJson::ts_definition(),
            "SuffixedAdjacentJson",
        ),
        (SuffixedEitherJson::ts_definition(), "SuffixedEitherJson"),
    ] {
        assert!(!ts.contains(ident), "rust ident reached: {ts}");
    }
}

/// The overwhelming case: an item declared under the name it exports, whose header this must leave
/// exactly where it was.
#[cfg(feature = "typescript")]
#[test]
fn an_item_exported_under_its_rust_ident_keeps_the_header_it_had() {
    for (ts, declared) in [
        (PlainStruct::ts_definition(), "PlainStruct"),
        (PlainTuple::ts_definition(), "PlainTuple"),
        (PlainSlot::ts_definition(), "PlainSlot"),
        (PlainShape::ts_definition(), "PlainShape"),
        (PlainAdjacent::ts_definition(), "PlainAdjacent"),
        (PlainEither::ts_definition(), "PlainEither"),
    ] {
        assert_jsdoc_opens_with(&ts, declared);
    }
}

/// The fallback fires only for an item with nothing to say, so a documented one keeps its docs
/// whatever it is exported as.
#[cfg(feature = "typescript")]
#[test]
fn a_documented_item_keeps_its_docs_over_either_name() {
    let ts = DocStructUnderRustName::ts_definition();
    assert_jsdoc_opens_with(&ts, "A documented struct.");
    assert!(
        ts.contains("export type RenamedDocStruct "),
        "override missing from: {ts}"
    );
}

/// A plain enum is the one shape that writes the fallback twice — once as the `JSDoc` header and
/// once as the Zod `description` — and the two are the same string.
#[cfg(feature = "zod")]
#[test]
fn a_plain_enum_describes_itself_as_it_is_exported() {
    for (zod, exported) in [
        (SlotUnderRustName::zod_schema(), "RenamedSlot"),
        (SuffixedSlotJson::zod_schema(), "SuffixedSlot"),
        (PlainSlot::zod_schema(), "PlainSlot"),
    ] {
        assert!(
            zod.contains(&format!("description: \"{exported}\"")),
            "{exported} not described by: {zod}"
        );
    }
    assert!(
        !SlotUnderRustName::zod_schema().contains("UnderRustName"),
        "rust ident reached the description"
    );
    assert!(
        !SuffixedSlotJson::zod_schema().contains("SuffixedSlotJson"),
        "rust ident reached the description"
    );
}

/// The brand path already spelled its description from the export name; it is the spelling the
/// other shapes were brought onto, so it must not move.
#[cfg(feature = "zod")]
#[test]
fn a_brand_describes_itself_as_it_is_exported() {
    for (zod, exported) in [
        (BrandUnderRustName::zod_schema(), "RenamedBrand"),
        (SuffixedBrandJson::zod_schema(), "SuffixedBrand"),
        (PlainBrand::zod_schema(), "PlainBrand"),
    ] {
        assert!(
            zod.contains(&format!("description: \"{exported}\"")),
            "{exported} not described by: {zod}"
        );
    }
}
