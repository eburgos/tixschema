//! An `Option` inside a covered sequence wrapper and an `Option` around one are two different
//! values on the wire, and each surface has to say which it describes.
//!
//! Every expectation here is read off what serde writes, asserted first in this file and then
//! described by the three surfaces in the same order: a `None` the wrapper holds is a `null` among
//! the array's items, so the items admit `null` and the array itself does not; a `None` around the
//! wrapper replaces the whole array, so the array admits `null` and its items do not.

use alloc::collections::BTreeSet;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tixschema::model_schema;

/// The `Option` inside the wrapper. Serde always writes the array, so the key is always present and
/// the `None` reaches the wire as a `null` among the items.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct ElementNullFields {
    set_items: BTreeSet<Option<u32>>,
    vec_items: Vec<Option<u32>>,
}

/// The `Option` around the wrapper. In field position a `None` writes a bare `null` under the key,
/// which the generated contract does not admit — so this shape carries the omission the guard
/// demands, and the key is dropped instead.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct SlotNullFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    set_items: Option<BTreeSet<u32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vec_items: Option<Vec<u32>>,
}

/// The same four nestings in the two slots that cannot be dropped, plus the one holding both
/// `Option`s at once — the levels are independent, and neither swallows the other.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NullNestingSlots {
    map_both: HashMap<String, Option<Vec<Option<u32>>>>,
    map_set_element: HashMap<String, BTreeSet<Option<u32>>>,
    map_set_slot: HashMap<String, Option<BTreeSet<u32>>>,
    map_vec_element: HashMap<String, Vec<Option<u32>>>,
    map_vec_slot: HashMap<String, Option<Vec<u32>>>,
    map_wrapped: HashMap<String, Vec<Option<BTreeSet<Option<u32>>>>>,
    tuple_both: (String, Option<Vec<Option<u32>>>),
    tuple_set_element: (String, BTreeSet<Option<u32>>),
    tuple_set_slot: (String, Option<BTreeSet<u32>>),
    tuple_vec_element: (String, Vec<Option<u32>>),
    tuple_vec_slot: (String, Option<Vec<u32>>),
}

/// The member spelling an `Option` field written with a `skip_serializing_if` renders as.
///
/// serde drops the key for a `None`, so the payload has no such key and the member is written with
/// an optional one. Only a build that reads the attribute knows that: without the `serde` feature
/// none is read, and the key stays written with an `undefined` value.
#[cfg(feature = "typescript")]
fn omitted_member(name: &str, ts_type: &str) -> String {
    if cfg!(feature = "serde") {
        format!("{name}?: {ts_type};")
    } else {
        format!("{name}: {ts_type} | undefined;")
    }
}

fn element_null_fields() -> ElementNullFields {
    ElementNullFields {
        set_items: BTreeSet::from([None]),
        vec_items: vec![None],
    }
}

fn slot_null_fields() -> SlotNullFields {
    SlotNullFields {
        set_items: None,
        vec_items: None,
    }
}

fn null_nesting_slots() -> NullNestingSlots {
    NullNestingSlots {
        map_both: HashMap::from([("k".to_owned(), Some(vec![None]))]),
        map_set_element: HashMap::from([("k".to_owned(), BTreeSet::from([None]))]),
        map_set_slot: HashMap::from([("k".to_owned(), None)]),
        map_vec_element: HashMap::from([("k".to_owned(), vec![None])]),
        map_vec_slot: HashMap::from([("k".to_owned(), None)]),
        map_wrapped: HashMap::from([("k".to_owned(), vec![Some(BTreeSet::from([None])), None])]),
        tuple_both: ("t".to_owned(), Some(vec![None])),
        tuple_set_element: ("t".to_owned(), BTreeSet::from([None])),
        tuple_set_slot: ("t".to_owned(), None),
        tuple_vec_element: ("t".to_owned(), vec![None]),
        tuple_vec_slot: ("t".to_owned(), None),
    }
}

/// A `None` the wrapper holds is a `null` inside the array, and the array is written either way.
#[test]
fn test_an_option_inside_a_wrapper_writes_a_null_among_the_items() {
    let payload = serde_json::to_value(element_null_fields()).unwrap();
    assert_eq!(payload["set_items"], serde_json::json!([null]));
    assert_eq!(payload["vec_items"], serde_json::json!([null]));
}

/// A `None` around the wrapper stands in place of the whole array — and in field position, where
/// the key can be dropped, the omission is what the schema is written against.
#[test]
fn test_an_option_around_a_wrapper_stands_for_the_whole_array() {
    let payload = serde_json::to_value(slot_null_fields()).unwrap();
    assert_eq!(payload, serde_json::json!({}));
}

/// In a slot the key cannot be dropped, so each nesting writes exactly what its own `Option` says:
/// the items for the inner one, the slot itself for the outer, both when both are written.
#[test]
fn test_each_slot_writes_the_null_its_own_option_puts_there() {
    let payload = serde_json::to_value(null_nesting_slots()).unwrap();
    for field in ["map_set_element", "map_vec_element"] {
        assert_eq!(payload[field]["k"], serde_json::json!([null]), "{field}");
    }
    for field in ["map_set_slot", "map_vec_slot"] {
        assert_eq!(payload[field]["k"], serde_json::json!(null), "{field}");
    }
    assert_eq!(payload["map_both"]["k"], serde_json::json!([null]));
    assert_eq!(
        payload["map_wrapped"]["k"],
        serde_json::json!([[null], null])
    );
    for field in ["tuple_set_element", "tuple_vec_element"] {
        assert_eq!(payload[field][1], serde_json::json!([null]), "{field}");
    }
    for field in ["tuple_set_slot", "tuple_vec_slot"] {
        assert_eq!(payload[field][1], serde_json::json!(null), "{field}");
    }
    assert_eq!(payload["tuple_both"][1], serde_json::json!([null]));
}

#[cfg(feature = "jsonschema")]
fn integer_or_null() -> serde_json::Value {
    serde_json::json!({ "anyOf": [{ "type": "integer" }, { "type": "null" }] })
}

#[cfg(feature = "jsonschema")]
fn array_of(items: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "type": "array", "items": items })
}

#[cfg(feature = "jsonschema")]
fn or_null(base: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "anyOf": [base, { "type": "null" }] })
}

/// The array is always written, so the field describes as an array whose items admit `null` —
/// required, and never `null` itself.
#[test]
#[cfg(feature = "jsonschema")]
fn test_an_element_null_field_describes_as_an_array_of_nullable_items() {
    let schema = ElementNullFields::json_schema();
    let expected = array_of(&integer_or_null());
    for field in ["set_items", "vec_items"] {
        assert_eq!(schema["properties"][field], expected, "{field}");
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!(field)),
            "{field} must stay required: {schema}"
        );
    }
}

/// The key is dropped rather than written `null`, so the field describes as the plain array and
/// says nothing about `null` at all — it is the absence that the schema admits, by not requiring it.
#[test]
#[cfg(feature = "jsonschema")]
fn test_a_slot_null_field_describes_as_the_array_it_writes_when_present() {
    let schema = SlotNullFields::json_schema();
    let expected = array_of(&serde_json::json!({ "type": "integer" }));
    for field in ["set_items", "vec_items"] {
        assert_eq!(schema["properties"][field], expected, "{field}");
        assert!(
            !schema["required"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!(field)),
            "{field} must not be required: {schema}"
        );
    }
}

/// Each slot describes the `null` its own `Option` writes, and no other: the items for the inner
/// one, the member for the outer, both wraps when both were written.
#[test]
#[cfg(feature = "jsonschema")]
fn test_each_slot_describes_the_null_its_own_option_writes() {
    let properties = NullNestingSlots::json_schema()["properties"].clone();
    let plain_array = array_of(&serde_json::json!({ "type": "integer" }));
    let nullable_items = array_of(&integer_or_null());

    for field in ["map_set_element", "map_vec_element"] {
        assert_eq!(
            properties[field]["additionalProperties"], nullable_items,
            "{field}"
        );
    }
    for field in ["map_set_slot", "map_vec_slot"] {
        assert_eq!(
            properties[field]["additionalProperties"],
            or_null(&plain_array),
            "{field}"
        );
    }
    assert_eq!(
        properties["map_both"]["additionalProperties"],
        or_null(&nullable_items)
    );

    for field in ["tuple_set_element", "tuple_vec_element"] {
        assert_eq!(
            properties[field]["prefixItems"][1], nullable_items,
            "{field}"
        );
    }
    for field in ["tuple_set_slot", "tuple_vec_slot"] {
        assert_eq!(
            properties[field]["prefixItems"][1],
            or_null(&plain_array),
            "{field}"
        );
    }
    assert_eq!(
        properties["tuple_both"]["prefixItems"][1],
        or_null(&nullable_items)
    );

    // A wrapper under an array is two levels, and each one answers for the `Option` written at it:
    // the inner array's items, then the inner array itself, both inside the outer array.
    assert_eq!(
        properties["map_wrapped"]["additionalProperties"],
        array_of(&or_null(&nullable_items))
    );
}

/// The TypeScript surface says the same thing in its own spelling: the `| null` sits inside the
/// `Array<…>` for an element `Option` and outside it for a slot `Option`.
#[test]
#[cfg(feature = "typescript")]
fn test_the_typescript_surface_puts_the_null_at_the_level_it_was_written() {
    for spelling in [
        "set_items: Array<number | null>;",
        "vec_items: Array<number | null>;",
    ] {
        let definition = ElementNullFields::ts_definition();
        assert!(definition.contains(spelling), "Got: {definition}");
    }
    for spelling in [
        omitted_member("set_items", "Array<number>"),
        omitted_member("vec_items", "Array<number>"),
    ] {
        let definition = SlotNullFields::ts_definition();
        assert!(definition.contains(&spelling), "Got: {definition}");
    }
    let definition = NullNestingSlots::ts_definition();
    for spelling in [
        "map_both: Partial<Record<string, Array<number | null> | null>>;",
        "map_wrapped: Partial<Record<string, Array<Array<number | null> | null>>>;",
        "map_set_element: Partial<Record<string, Array<number | null>>>;",
        "map_set_slot: Partial<Record<string, Array<number> | null>>;",
        "map_vec_element: Partial<Record<string, Array<number | null>>>;",
        "map_vec_slot: Partial<Record<string, Array<number> | null>>;",
        "tuple_both: [string, Array<number | null> | null];",
        "tuple_set_element: [string, Array<number | null>];",
        "tuple_set_slot: [string, Array<number> | null];",
        "tuple_vec_element: [string, Array<number | null>];",
        "tuple_vec_slot: [string, Array<number> | null];",
    ] {
        assert!(definition.contains(spelling), "Got: {definition}");
    }
}

/// And the Zod surface: `z.nullable` inside the `z.array` for an element `Option`, around it for a
/// slot `Option`.
#[test]
#[cfg(feature = "zod")]
fn test_the_zod_surface_puts_the_nullable_at_the_level_it_was_written() {
    let element_null = ElementNullFields::zod_schema();
    for spelling in [
        "set_items: z.array(z.nullable(z.number().int())),",
        "vec_items: z.array(z.nullable(z.number().int())),",
    ] {
        assert!(element_null.contains(spelling), "Got: {element_null}");
    }
    let slot_null = SlotNullFields::zod_schema();
    for spelling in [
        "set_items: z.union([z.array(z.number().int()), z.undefined()]).prefault(undefined),",
        "vec_items: z.union([z.array(z.number().int()), z.undefined()]).prefault(undefined),",
    ] {
        assert!(slot_null.contains(spelling), "Got: {slot_null}");
    }
    let schema = NullNestingSlots::zod_schema();
    for spelling in [
        "map_both: z.record(z.string(), z.nullable(z.array(z.nullable(z.number().int())))),",
        "map_wrapped: z.record(z.string(), z.array(z.nullable(z.array(z.nullable(z.number().int()))))),",
        "map_set_element: z.record(z.string(), z.array(z.nullable(z.number().int()))),",
        "map_set_slot: z.record(z.string(), z.nullable(z.array(z.number().int()))),",
        "map_vec_element: z.record(z.string(), z.array(z.nullable(z.number().int()))),",
        "map_vec_slot: z.record(z.string(), z.nullable(z.array(z.number().int()))),",
        "tuple_both: z.tuple([z.string(), z.nullable(z.array(z.nullable(z.number().int())))]),",
        "tuple_set_element: z.tuple([z.string(), z.array(z.nullable(z.number().int()))]),",
        "tuple_set_slot: z.tuple([z.string(), z.nullable(z.array(z.number().int()))]),",
        "tuple_vec_element: z.tuple([z.string(), z.array(z.nullable(z.number().int()))]),",
        "tuple_vec_slot: z.tuple([z.string(), z.nullable(z.array(z.number().int()))]),",
    ] {
        assert!(schema.contains(spelling), "Got: {schema}");
    }
}

/// The set spelling and the `Vec` spelling of the same nesting write one value, so they describe as
/// one — the parity the wrapper list exists to hold, now read at each level separately.
#[test]
#[cfg(feature = "jsonschema")]
fn test_a_set_nesting_describes_as_the_vec_nesting_it_writes() {
    let properties = NullNestingSlots::json_schema()["properties"].clone();
    for (set_field, vec_field) in [
        ("map_set_element", "map_vec_element"),
        ("map_set_slot", "map_vec_slot"),
        ("tuple_set_element", "tuple_vec_element"),
        ("tuple_set_slot", "tuple_vec_slot"),
    ] {
        assert_eq!(
            properties[set_field], properties[vec_field],
            "{set_field} against {vec_field}"
        );
    }
    assert_eq!(
        ElementNullFields::json_schema()["properties"]["set_items"],
        ElementNullFields::json_schema()["properties"]["vec_items"]
    );
}
