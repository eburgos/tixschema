use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use tixschema::model_schema;

/// A length no expansion can read: the macro runs long before a `const` has a value.
const SLOT_COUNT: usize = 3;

/// Every spelling that mixes a fixed-size array with the wrappers around it, so the bound is read
/// for the level it was written at rather than for the field it was written on.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct FixedArrayFields {
    const_ids: [u32; SLOT_COUNT],
    grid: [[u32; 2]; 2],
    ids: [u32; 3],
    ids_of_rows: [Vec<u32>; 2],
    optional_elements: [Option<u32>; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    optional_ids: Option<[u32; 3]>,
    rows_of_ids: Vec<[u32; 3]>,
    set_of_ids: [HashSet<u32>; 2],
    slice_ids: Box<[u32]>,
}

/// The same array in the slots that cannot be dropped, where the wrapper chain is normalized onto
/// the element before the slot wraps go on.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct FixedArraySlots {
    by_key: HashMap<String, [u32; 3]>,
    entry: (String, [u32; 3]),
}

/// The member spelling an `Option` field written with a `skip_serializing_if` renders as.
///
/// serde drops the key for a `None`, so the payload has no such key and the member is written with
/// an optional one. The attribute is on the field under every toggle, so this is one spelling and
/// not two.
#[cfg(feature = "typescript")]
fn omitted_member(name: &str, ts_type: &str) -> String {
    format!("{name}?: {ts_type};")
}

fn fixed_array_fields() -> FixedArrayFields {
    FixedArrayFields {
        const_ids: [1, 2, 3],
        grid: [[1, 2], [3, 4]],
        ids: [1, 2, 3],
        ids_of_rows: [vec![1], vec![2]],
        optional_elements: [Some(1), None, Some(3)],
        optional_ids: Some([1, 2, 3]),
        rows_of_ids: vec![[1, 2, 3]],
        set_of_ids: [HashSet::from([1]), HashSet::from([2])],
        slice_ids: vec![1, 2].into_boxed_slice(),
    }
}

fn fixed_array_slots() -> FixedArraySlots {
    FixedArraySlots {
        by_key: HashMap::from([("k".to_owned(), [1, 2, 3])]),
        entry: ("t".to_owned(), [1, 2, 3]),
    }
}

/// The wire criterion every surface below is read against: the array is written at one length and
/// read back at that length alone, so a payload of any other length is one serde rejects.
#[test]
fn test_serde_reads_a_fixed_array_back_only_at_the_length_it_writes() {
    let three = serde_json::json!([1_u32, 2_u32, 3_u32]);
    assert_eq!(
        serde_json::to_value(fixed_array_fields()).unwrap()["ids"],
        three
    );
    let slots = serde_json::to_value(fixed_array_slots()).unwrap();
    assert_eq!(slots["by_key"]["k"], three);
    assert_eq!(slots["entry"][1], three);

    assert_eq!(
        serde_json::from_value::<[u32; 3]>(three).ok(),
        Some([1_u32, 2_u32, 3_u32])
    );
    for wrong_length in [
        serde_json::json!([1_u32, 2_u32]),
        serde_json::json!([1_u32, 2_u32, 3_u32, 4_u32]),
    ] {
        assert_eq!(
            serde_json::from_value::<[u32; 3]>(wrong_length.clone()).ok(),
            None,
            "for: {wrong_length}"
        );
    }
}

/// The JSON schema pins that length at the level that carries it, and leaves every other level as
/// open as the spelling that wrote it — a `Vec`, a slice, or a count the expansion cannot read.
#[test]
#[cfg(feature = "jsonschema")]
fn test_a_fixed_array_describes_its_length_at_the_level_it_bounds() {
    let properties = FixedArrayFields::json_schema()["properties"].clone();
    let three_integers = serde_json::json!({
        "type": "array",
        "items": { "type": "integer" },
        "minItems": 3_u32,
        "maxItems": 3_u32
    });
    let open_integers = serde_json::json!({ "type": "array", "items": { "type": "integer" } });

    for field in ["ids", "optional_ids"] {
        assert_eq!(properties[field], three_integers, "for: {field}");
    }
    for field in ["const_ids", "slice_ids"] {
        assert_eq!(properties[field], open_integers, "for: {field}");
    }
    for field in ["ids_of_rows", "set_of_ids"] {
        assert_eq!(
            properties[field],
            serde_json::json!({
                "type": "array",
                "items": open_integers,
                "minItems": 2_u32,
                "maxItems": 2_u32
            }),
            "for: {field}"
        );
    }
    assert_eq!(
        properties["rows_of_ids"],
        serde_json::json!({ "type": "array", "items": three_integers })
    );
    assert_eq!(
        properties["grid"],
        serde_json::json!({
            "type": "array",
            "items": {
                "type": "array",
                "items": { "type": "integer" },
                "minItems": 2_u32,
                "maxItems": 2_u32
            },
            "minItems": 2_u32,
            "maxItems": 2_u32
        })
    );
    assert_eq!(
        properties["optional_elements"],
        serde_json::json!({
            "type": "array",
            "items": { "anyOf": [{ "type": "integer" }, { "type": "null" }] },
            "minItems": 3_u32,
            "maxItems": 3_u32
        })
    );
}

/// A slot carries the bound the same way: the array is the value the slot holds, whatever holds it.
#[test]
#[cfg(feature = "jsonschema")]
fn test_a_fixed_array_in_a_slot_describes_its_length() {
    let properties = FixedArraySlots::json_schema()["properties"].clone();
    let three_integers = serde_json::json!({
        "type": "array",
        "items": { "type": "integer" },
        "minItems": 3_u32,
        "maxItems": 3_u32
    });

    assert_eq!(properties["by_key"]["additionalProperties"], three_integers);
    assert_eq!(properties["entry"]["prefixItems"][1], three_integers);
}

/// Zod is the other validator, so it says what the JSON schema says: `.length(N)` on the level the
/// `[T; N]` was written at, and nothing on the levels no count was written for.
#[test]
#[cfg(feature = "zod")]
fn test_a_fixed_array_validates_its_length_in_zod() {
    let zod_schema = FixedArrayFields::zod_schema();
    for spelling in [
        "const_ids: z.array(z.number().int()),",
        "grid: z.array(z.array(z.number().int()).length(2)).length(2),",
        "ids: z.array(z.number().int()).length(3),",
        "ids_of_rows: z.array(z.array(z.number().int())).length(2),",
        "optional_elements: z.array(z.nullable(z.number().int())).length(3),",
        "rows_of_ids: z.array(z.array(z.number().int()).length(3)),",
        "set_of_ids: z.array(z.array(z.number().int())).length(2),",
        "slice_ids: z.array(z.number().int()),",
    ] {
        assert!(zod_schema.contains(spelling), "Got: {zod_schema}");
    }
    // The field's own optionality wraps the bounded array rather than replacing its bound.
    assert!(
        zod_schema.contains(
            "optional_ids: z.union([z.array(z.number().int()).length(3), z.undefined()])"
        ),
        "Got: {zod_schema}"
    );

    let slot_schema = FixedArraySlots::zod_schema();
    for spelling in [
        "by_key: z.record(z.string(), z.array(z.number().int()).length(3)),",
        "entry: z.tuple([z.string(), z.array(z.number().int()).length(3)]),",
    ] {
        assert!(slot_schema.contains(spelling), "Got: {slot_schema}");
    }
}

/// TypeScript takes the other answer and stays `Array<T>` at every level. The fixed-length form its
/// type system has is the N-element tuple, which has to be written out element by element and stops
/// being readable long before `N` stops being legal; the two validating surfaces are where a
/// wrong-length payload is caught.
#[test]
#[cfg(feature = "typescript")]
fn test_a_fixed_array_types_as_an_unbounded_array_in_typescript() {
    let ts_definition = FixedArrayFields::ts_definition();
    let optional_ids = omitted_member("optional_ids", "Array<number>");
    for spelling in [
        "const_ids: Array<number>;",
        "grid: Array<Array<number>>;",
        "ids: Array<number>;",
        "ids_of_rows: Array<Array<number>>;",
        "optional_elements: Array<number | null>;",
        optional_ids.as_str(),
        "rows_of_ids: Array<Array<number>>;",
        "set_of_ids: Array<Array<number>>;",
        "slice_ids: Array<number>;",
    ] {
        assert!(ts_definition.contains(spelling), "Got: {ts_definition}");
    }

    let slot_definition = FixedArraySlots::ts_definition();
    for spelling in [
        "by_key: Partial<Record<string, Array<number>>>;",
        "entry: [string, Array<number>];",
    ] {
        assert!(slot_definition.contains(spelling), "Got: {slot_definition}");
    }
}
