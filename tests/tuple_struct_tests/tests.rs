use serde::{Deserialize, Serialize};
use tixschema::model_schema;

/// A newtype struct: serde writes the slot's value alone, with nothing around it.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Plain(pub String);

/// A wider tuple struct: serde writes a fixed-arity array.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Pair(pub String, pub u32);

/// A slot cannot be omitted the way an object key can, so a `None` here reaches the wire as
/// `null`.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MaybeName(pub Option<String>);

/// A slot serde carries in neither direction: it is absent from the array serde writes and from
/// every array serde reads, and the slot behind it moves up into its place.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SkippedLeadSlot(#[serde(skip)] pub Option<String>, pub String);

/// The same wire, written as the two halves rather than the word abbreviating them.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct BothHalvesDroppedSlot(
    #[serde(skip_serializing, skip_deserializing)] pub Option<String>,
    pub String,
);

/// A dropped slot at the end, where nothing moves up behind it and only the arity changes.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SkippedTrailingSlot(pub String, #[serde(skip)] pub u32);

/// A dropped slot between two carried ones, which is where renumbering is visible.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SkippedMiddleSlot(pub String, #[serde(skip)] pub Option<String>, pub u32);

/// Every slot dropped, which serde writes as the empty array rather than as nothing at all.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EverySlotSkipped(#[serde(skip)] pub String, #[serde(skip)] pub u32);

/// The lone slot of a newtype struct, which serde writes and reads whatever the attribute says.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SkippedLoneSlot(#[serde(skip)] pub String);

/// The wire criterion the newtype surfaces are read against.
#[test]
fn test_serde_writes_a_newtype_struct_as_the_inner_value_alone() {
    assert_eq!(
        serde_json::to_value(Plain("hello".to_owned())).unwrap(),
        serde_json::json!("hello")
    );
}

/// The wire criterion the wider surfaces are read against.
#[test]
fn test_serde_writes_a_wider_tuple_struct_as_a_fixed_array() {
    assert_eq!(
        serde_json::to_value(Pair("hello".to_owned(), 1_u32)).unwrap(),
        serde_json::json!(["hello", 1_u32])
    );
}

/// The wire criterion the nullable surfaces are read against: the slot is always written, so a
/// `None` arrives as `null` rather than as an absent key.
#[test]
fn test_serde_writes_a_none_slot_as_null() {
    assert_eq!(
        serde_json::to_value(MaybeName(None)).unwrap(),
        serde_json::json!(null)
    );
    assert_eq!(
        serde_json::to_value(MaybeName(Some("ada".to_owned()))).unwrap(),
        serde_json::json!("ada")
    );
}

#[cfg(feature = "typescript")]
#[test]
fn test_a_newtype_struct_describes_the_bare_inner_type_in_typescript() {
    let wire = serde_json::to_value(Plain("hello".to_owned())).unwrap();
    assert!(wire.is_string(), "wire: {wire}");

    let ts = Plain::ts_definition();
    assert!(ts.contains("export type Plain = string;"), "Got: {ts}");
    assert!(!ts.contains(": string;"), "Got: {ts}");
}

#[cfg(feature = "zod")]
#[test]
fn test_a_newtype_struct_describes_the_bare_inner_type_in_zod() {
    let wire = serde_json::to_value(Plain("hello".to_owned())).unwrap();
    assert!(wire.is_string(), "wire: {wire}");

    let zod = Plain::zod_schema();
    assert!(zod.contains("Plain$Schema"), "Got: {zod}");
    assert!(zod.contains("= z.string();"), "Got: {zod}");
    assert!(!zod.contains("{ : "), "Got: {zod}");
    assert!(!zod.contains("strictObject"), "Got: {zod}");
}

#[cfg(feature = "jsonschema")]
#[test]
fn test_a_newtype_struct_describes_the_bare_inner_type_in_json_schema() {
    let wire = serde_json::to_value(Plain("hello".to_owned())).unwrap();
    assert!(wire.is_string(), "wire: {wire}");

    assert_eq!(
        Plain::json_schema(),
        serde_json::json!({ "type": "string" })
    );
}

#[cfg(feature = "typescript")]
#[test]
fn test_a_wider_tuple_struct_describes_a_fixed_tuple_in_typescript() {
    let wire = serde_json::to_value(Pair("hello".to_owned(), 1_u32)).unwrap();
    assert!(wire.is_array(), "wire: {wire}");

    let ts = Pair::ts_definition();
    assert!(
        ts.contains("export type Pair = [string, number];"),
        "Got: {ts}"
    );
    assert!(!ts.contains(": string;"), "Got: {ts}");
}

#[cfg(feature = "zod")]
#[test]
fn test_a_wider_tuple_struct_describes_a_fixed_tuple_in_zod() {
    let wire = serde_json::to_value(Pair("hello".to_owned(), 1_u32)).unwrap();
    assert!(wire.is_array(), "wire: {wire}");

    let zod = Pair::zod_schema();
    assert!(zod.contains("Pair$Schema"), "Got: {zod}");
    assert!(
        zod.contains("= z.tuple([z.string(), z.number().int()]);"),
        "Got: {zod}"
    );
    assert!(!zod.contains("{ : "), "Got: {zod}");
    assert!(!zod.contains("strictObject"), "Got: {zod}");
}

/// The `z.ZodType<...> = ...$RawSchema` framing only appears when typescript is also enabled.
#[cfg(all(feature = "zod", feature = "typescript"))]
#[test]
fn test_a_tuple_struct_zod_schema_is_annotated_with_its_typescript_type() {
    let zod = Pair::zod_schema();
    assert!(
        zod.contains("export const Pair$Schema: z.ZodType<Pair> = Pair$RawSchema;"),
        "Got: {zod}"
    );
}

#[cfg(feature = "jsonschema")]
#[test]
fn test_a_wider_tuple_struct_describes_a_fixed_tuple_in_json_schema() {
    let wire = serde_json::to_value(Pair("hello".to_owned(), 1_u32)).unwrap();
    assert!(wire.is_array(), "wire: {wire}");

    assert_eq!(
        Pair::json_schema(),
        serde_json::json!({
            "type": "array",
            "prefixItems": [{ "type": "string" }, { "type": "integer" }],
            "items": false,
            "minItems": 2_u32,
            "maxItems": 2_u32
        })
    );
}

#[cfg(feature = "typescript")]
#[test]
fn test_an_optional_slot_describes_the_null_it_writes_in_typescript() {
    let wire = serde_json::to_value(MaybeName(None)).unwrap();
    assert!(wire.is_null(), "wire: {wire}");

    let ts = MaybeName::ts_definition();
    assert!(
        ts.contains("export type MaybeName = string | null;"),
        "Got: {ts}"
    );
}

#[cfg(feature = "zod")]
#[test]
fn test_an_optional_slot_describes_the_null_it_writes_in_zod() {
    let wire = serde_json::to_value(MaybeName(None)).unwrap();
    assert!(wire.is_null(), "wire: {wire}");

    let zod = MaybeName::zod_schema();
    assert!(zod.contains("MaybeName$Schema"), "Got: {zod}");
    assert!(zod.contains("= z.nullable(z.string());"), "Got: {zod}");
}

#[cfg(feature = "jsonschema")]
#[test]
fn test_an_optional_slot_describes_the_null_it_writes_in_json_schema() {
    let wire = serde_json::to_value(MaybeName(None)).unwrap();
    assert!(wire.is_null(), "wire: {wire}");

    assert_eq!(
        MaybeName::json_schema(),
        serde_json::json!({ "anyOf": [{ "type": "string" }, { "type": "null" }] })
    );
}

/// The wire criterion every dropped-slot surface below is read against, in both directions: the
/// slot is absent from the array serde writes, and the full-arity array — the one the surfaces used
/// to describe — is refused on the way in.
#[test]
fn test_serde_writes_and_reads_a_dropped_slot_in_neither_direction() {
    let written =
        serde_json::to_value(SkippedLeadSlot(Some("s".to_owned()), "x".to_owned())).unwrap();
    assert_eq!(written, serde_json::json!(["x"]));

    assert_eq!(
        serde_json::from_value::<SkippedLeadSlot>(written).unwrap(),
        SkippedLeadSlot(None, "x".to_owned())
    );
    assert!(
        serde_json::from_str::<SkippedLeadSlot>(r#"["s","x"]"#).is_err(),
        "the full-arity array must not read back"
    );
}

/// The halves written side by side are that same wire, so they owe the same surfaces.
#[test]
fn test_the_two_halves_written_together_are_the_same_slot_wire() {
    assert_eq!(
        serde_json::to_value(BothHalvesDroppedSlot(Some("s".to_owned()), "x".to_owned())).unwrap(),
        serde_json::json!(["x"])
    );
    assert!(
        serde_json::from_str::<BothHalvesDroppedSlot>(r#"["s","x"]"#).is_err(),
        "the full-arity array must not read back"
    );
}

/// The wire criterion for the remaining positions: a trailing drop only shortens the array, a
/// middle one moves the slot behind it up, and dropping every slot still writes an array.
#[test]
fn test_serde_writes_the_slots_it_still_carries_in_their_new_places() {
    assert_eq!(
        serde_json::to_value(SkippedTrailingSlot("s".to_owned(), 1_u32)).unwrap(),
        serde_json::json!(["s"])
    );
    assert_eq!(
        serde_json::to_value(SkippedMiddleSlot(
            "a".to_owned(),
            Some("s".to_owned()),
            7_u32
        ))
        .unwrap(),
        serde_json::json!(["a", 7_u32])
    );
    assert_eq!(
        serde_json::to_value(EverySlotSkipped("s".to_owned(), 1_u32)).unwrap(),
        serde_json::json!([])
    );
}

/// The wire criterion the lone slot's surfaces are read against: serde writes and reads a newtype
/// struct's only slot whatever the attribute on it says, so nothing is dropped there.
#[test]
fn test_serde_writes_a_lone_slot_whatever_the_skip_says() {
    assert_eq!(
        serde_json::to_value(SkippedLoneSlot("s".to_owned())).unwrap(),
        serde_json::json!("s")
    );
    assert_eq!(
        serde_json::from_str::<SkippedLoneSlot>(r#""s""#).unwrap(),
        SkippedLoneSlot("s".to_owned())
    );
}

/// The described tuple is the slots the wire carries: the dropped one is not an element, and the
/// arity is the one serde writes rather than the one the struct declares.
#[cfg(feature = "typescript")]
#[test]
fn test_a_dropped_slot_is_no_element_of_the_typescript_tuple() {
    let wire = serde_json::to_value(SkippedLeadSlot(Some("s".to_owned()), "x".to_owned())).unwrap();
    assert_eq!(wire, serde_json::json!(["x"]));

    let ts = SkippedLeadSlot::ts_definition();
    assert!(
        ts.contains("export type SkippedLeadSlot = [string];"),
        "Got: {ts}"
    );
    assert!(!ts.contains("null"), "Got: {ts}");
}

/// The halves written apart are one wire, so they are one emission.
#[cfg(feature = "typescript")]
#[test]
fn test_typescript_answers_the_two_slot_halves_the_way_it_answers_the_word() {
    let ts = BothHalvesDroppedSlot::ts_definition();
    assert!(
        ts.contains("export type BothHalvesDroppedSlot = [string];"),
        "Got: {ts}"
    );
}

/// Every remaining position, each read against the array serde writes for it.
#[cfg(feature = "typescript")]
#[test]
fn test_typescript_describes_the_arity_each_drop_leaves() {
    for (ts, expected) in [
        (SkippedTrailingSlot::ts_definition(), "[string]"),
        (SkippedMiddleSlot::ts_definition(), "[string, number]"),
        (EverySlotSkipped::ts_definition(), "[]"),
        (SkippedLoneSlot::ts_definition(), "string"),
    ] {
        assert!(ts.contains(&format!("= {expected};")), "Got: {ts}");
    }
}

/// The Zod tuple is the one the payload serde writes satisfies. Recorded against zod 4.1.8 under
/// node v26.2.0: `z.tuple([z.string()])` accepts `["x"]` and rejects `["s","x"]` with `too_big`,
/// which is the pair serde writes and refuses. The two-element tuple this used to emit,
/// `z.tuple([z.nullable(z.string()), z.string()])`, rejects `["x"]` with `invalid_type` — the only
/// payload serde writes.
#[cfg(feature = "zod")]
#[test]
fn test_a_dropped_slot_is_no_element_of_the_zod_tuple() {
    let wire = serde_json::to_value(SkippedLeadSlot(Some("s".to_owned()), "x".to_owned())).unwrap();
    assert_eq!(wire, serde_json::json!(["x"]));

    let zod = SkippedLeadSlot::zod_schema();
    assert!(zod.contains("= z.tuple([z.string()]);"), "Got: {zod}");
    assert!(!zod.contains("nullable"), "Got: {zod}");
}

/// [`test_typescript_describes_the_arity_each_drop_leaves`] for the Zod surface.
#[cfg(feature = "zod")]
#[test]
fn test_zod_describes_the_arity_each_drop_leaves() {
    for (zod, expected) in [
        (SkippedTrailingSlot::zod_schema(), "z.tuple([z.string()])"),
        (
            SkippedMiddleSlot::zod_schema(),
            "z.tuple([z.string(), z.number().int()])",
        ),
        (EverySlotSkipped::zod_schema(), "z.tuple([])"),
        (SkippedLoneSlot::zod_schema(), "z.string()"),
    ] {
        assert!(zod.contains(&format!("= {expected};")), "Got: {zod}");
    }
}

/// The JSON surface carries the arity twice more, as its own bounds, and both move with the drop.
#[cfg(feature = "jsonschema")]
#[test]
fn test_a_dropped_slot_leaves_the_json_schema_prefix_and_its_bounds() {
    let wire = serde_json::to_value(SkippedLeadSlot(Some("s".to_owned()), "x".to_owned())).unwrap();
    assert_eq!(wire, serde_json::json!(["x"]));

    assert_eq!(
        SkippedLeadSlot::json_schema(),
        serde_json::json!({
            "type": "array",
            "prefixItems": [{ "type": "string" }],
            "items": false,
            "minItems": 1_u32,
            "maxItems": 1_u32
        })
    );
}

/// [`test_typescript_describes_the_arity_each_drop_leaves`] for the JSON-schema surface, where the
/// empty array is the one shape that keeps `prefixItems` and drops every element from it.
#[cfg(feature = "jsonschema")]
#[test]
fn test_the_json_schema_describes_the_arity_each_drop_leaves() {
    assert_eq!(
        SkippedMiddleSlot::json_schema(),
        serde_json::json!({
            "type": "array",
            "prefixItems": [{ "type": "string" }, { "type": "integer" }],
            "items": false,
            "minItems": 2_u32,
            "maxItems": 2_u32
        })
    );
    assert_eq!(
        EverySlotSkipped::json_schema(),
        serde_json::json!({
            "type": "array",
            "prefixItems": [],
            "items": false,
            "minItems": 0_u32,
            "maxItems": 0_u32
        })
    );
    assert_eq!(
        SkippedLoneSlot::json_schema(),
        serde_json::json!({ "type": "string" })
    );
}

/// The empty ident of an unnamed slot must not reach any surface as a key.
#[cfg(feature = "typescript")]
#[test]
fn test_typescript_never_names_a_slot_by_its_absent_ident() {
    for ts in [
        Plain::ts_definition(),
        Pair::ts_definition(),
        MaybeName::ts_definition(),
    ] {
        assert!(!ts.contains(": string;"), "Got: {ts}");
        assert!(!ts.contains("  : "), "Got: {ts}");
    }
}

/// [`test_typescript_never_names_a_slot_by_its_absent_ident`] for the Zod surface.
#[cfg(feature = "zod")]
#[test]
fn test_zod_never_names_a_slot_by_its_absent_ident() {
    for zod in [
        Plain::zod_schema(),
        Pair::zod_schema(),
        MaybeName::zod_schema(),
    ] {
        assert!(!zod.contains("{ : "), "Got: {zod}");
        assert!(!zod.contains("  : "), "Got: {zod}");
    }
}

/// [`test_typescript_never_names_a_slot_by_its_absent_ident`] for the JSON-schema surface, where
/// the empty ident used to arrive as a property literally named `""`.
#[cfg(feature = "jsonschema")]
#[test]
fn test_json_schema_never_names_a_slot_by_its_absent_ident() {
    for json in [
        Plain::json_schema(),
        Pair::json_schema(),
        MaybeName::json_schema(),
    ] {
        assert!(!json.to_string().contains("\"\""), "Got: {json}");
    }
}
