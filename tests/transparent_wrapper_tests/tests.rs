use alloc::borrow::Cow;
use alloc::rc::Rc;
use alloc::sync::Arc;
use core::cell::{Cell, RefCell};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, RwLock};
use tixschema::model_schema;

/// The covered wrappers by the name a generated surface could leak.
#[cfg(any(feature = "typescript", feature = "zod"))]
const TRANSPARENT_WRAPPERS: [&str; 8] = [
    "Arc", "Box", "Cell", "Cow", "Mutex", "Rc", "RefCell", "RwLock",
];

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Tag {
    label: String,
}

// The spelling every wrapper twin below is held against: the inner types written bare.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct PlainFields {
    count: u64,
    labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    maybe_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    maybe_tag: Option<Tag>,
    tag: Tag,
    text: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct BoxedFields {
    count: Box<u64>,
    labels: Box<[String]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    maybe_count: Box<Option<u64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    maybe_tag: Box<Option<Tag>>,
    tag: Box<Tag>,
    text: Box<str>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct RcFields {
    count: Rc<u64>,
    labels: Rc<[String]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    maybe_count: Rc<Option<u64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    maybe_tag: Rc<Option<Tag>>,
    tag: Rc<Tag>,
    text: Rc<str>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct ArcFields {
    count: Arc<u64>,
    labels: Arc<[String]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    maybe_count: Arc<Option<u64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    maybe_tag: Arc<Option<Tag>>,
    tag: Arc<Tag>,
    text: Arc<str>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct CowFields {
    count: Cow<'static, u64>,
    labels: Cow<'static, [String]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    maybe_count: Cow<'static, Option<u64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    maybe_tag: Cow<'static, Option<Tag>>,
    tag: Cow<'static, Tag>,
    text: Cow<'static, str>,
}

// `RefCell::new` takes a `Sized` value, so unlike `Box`/`Rc`/`Arc`/`Cow` above it cannot hold an
// unsized `[String]`/`str` directly — the owned `Vec<String>`/`String` spellings stand in, and
// still collapse onto the same field `PlainFields` does.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct RefCellFields {
    count: RefCell<u64>,
    labels: RefCell<Vec<String>>,
    #[serde(default, skip_serializing_if = "ref_cell_option_is_none")]
    maybe_count: RefCell<Option<u64>>,
    #[serde(default, skip_serializing_if = "ref_cell_option_is_none")]
    maybe_tag: RefCell<Option<Tag>>,
    tag: RefCell<Tag>,
    text: RefCell<String>,
}

// `Mutex` implements neither `Clone` nor `PartialEq` regardless of what it holds, unlike the other
// covered wrappers.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
struct MutexFields {
    count: Mutex<u64>,
    labels: Mutex<Vec<String>>,
    #[serde(default, skip_serializing_if = "mutex_option_is_none")]
    maybe_count: Mutex<Option<u64>>,
    #[serde(default, skip_serializing_if = "mutex_option_is_none")]
    maybe_tag: Mutex<Option<Tag>>,
    tag: Mutex<Tag>,
    text: Mutex<String>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
struct RwLockFields {
    count: RwLock<u64>,
    labels: RwLock<Vec<String>>,
    #[serde(default, skip_serializing_if = "rwlock_option_is_none")]
    maybe_count: RwLock<Option<u64>>,
    #[serde(default, skip_serializing_if = "rwlock_option_is_none")]
    maybe_tag: RwLock<Option<Tag>>,
    tag: RwLock<Tag>,
    text: RwLock<String>,
}

// `Cell<T>` requires `T: Copy` for serde's own `Serialize` impl, so its fixture -- and the plain
// spelling held against it -- are reduced to the `PlainFields` fields that are `Copy`.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct CellFields {
    count: Cell<u64>,
    #[serde(default, skip_serializing_if = "cell_option_is_none")]
    maybe_count: Cell<Option<u64>>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct PlainCopyFields {
    count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    maybe_count: Option<u64>,
}

// A wrapper written under something else: inside an `Option`, inside a sequence, in a map's value
// slot. Each position reads the collapsed field, so none of them sees a wrapper at all.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct WrappedInsideFields {
    counts: HashMap<String, Rc<u64>>,
    elements: Vec<Rc<Tag>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    maybe_tag: Option<Box<Tag>>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct PlainInsideFields {
    counts: HashMap<String, u64>,
    elements: Vec<Tag>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    maybe_tag: Option<Tag>,
}

// One boxed field among unboxed ones: the collapse is the field's own, so its siblings are
// untouched by it.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct MixedBoxedFields {
    boxed_tag: Box<Tag>,
    count: u64,
    plain_tag: Tag,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct MixedPlainFields {
    boxed_tag: Tag,
    count: u64,
    plain_tag: Tag,
}

// The reason `Box` exists in a schema type at all: a struct that holds itself.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct TreeNode {
    label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    next: Option<Box<Self>>,
}

// None of the four is `Deref`, so `skip_serializing_if = "Option::is_none"` -- which reaches an
// `Option` field under `Box`/`Rc`/`Arc`/`Cow` by deref coercion -- has no field to coerce to here.
// Each predicate reaches the guarded `Option` through the wrapper's own accessor instead.
fn ref_cell_option_is_none<T>(value: &RefCell<Option<T>>) -> bool {
    value.borrow().is_none()
}

fn mutex_option_is_none<T>(value: &Mutex<Option<T>>) -> bool {
    value.lock().unwrap().is_none()
}

fn rwlock_option_is_none<T>(value: &RwLock<Option<T>>) -> bool {
    value.read().unwrap().is_none()
}

fn cell_option_is_none<T>(value: &Cell<Option<T>>) -> bool
where
    T: Copy,
{
    value.get().is_none()
}

fn tag() -> Tag {
    Tag {
        label: "a".to_owned(),
    }
}

fn plain_fields() -> PlainFields {
    PlainFields {
        count: 7,
        labels: vec!["x".to_owned()],
        maybe_count: Some(3),
        maybe_tag: Some(tag()),
        tag: tag(),
        text: "t".to_owned(),
    }
}

fn boxed_fields() -> BoxedFields {
    BoxedFields {
        count: Box::new(7),
        labels: vec!["x".to_owned()].into_boxed_slice(),
        maybe_count: Box::new(Some(3)),
        maybe_tag: Box::new(Some(tag())),
        tag: Box::new(tag()),
        text: "t".into(),
    }
}

fn rc_fields() -> RcFields {
    RcFields {
        count: Rc::new(7),
        labels: Rc::from(vec!["x".to_owned()]),
        maybe_count: Rc::new(Some(3)),
        maybe_tag: Rc::new(Some(tag())),
        tag: Rc::new(tag()),
        text: Rc::from("t"),
    }
}

fn arc_fields() -> ArcFields {
    ArcFields {
        count: Arc::new(7),
        labels: Arc::from(vec!["x".to_owned()]),
        maybe_count: Arc::new(Some(3)),
        maybe_tag: Arc::new(Some(tag())),
        tag: Arc::new(tag()),
        text: Arc::from("t"),
    }
}

fn cow_fields() -> CowFields {
    CowFields {
        count: Cow::Owned(7),
        labels: Cow::Owned(vec!["x".to_owned()]),
        maybe_count: Cow::Owned(Some(3)),
        maybe_tag: Cow::Owned(Some(tag())),
        tag: Cow::Owned(tag()),
        text: Cow::Borrowed("t"),
    }
}

fn ref_cell_fields() -> RefCellFields {
    RefCellFields {
        count: RefCell::new(7),
        labels: RefCell::new(vec!["x".to_owned()]),
        maybe_count: RefCell::new(Some(3)),
        maybe_tag: RefCell::new(Some(tag())),
        tag: RefCell::new(tag()),
        text: RefCell::new("t".to_owned()),
    }
}

fn mutex_fields() -> MutexFields {
    MutexFields {
        count: Mutex::new(7),
        labels: Mutex::new(vec!["x".to_owned()]),
        maybe_count: Mutex::new(Some(3)),
        maybe_tag: Mutex::new(Some(tag())),
        tag: Mutex::new(tag()),
        text: Mutex::new("t".to_owned()),
    }
}

fn rwlock_fields() -> RwLockFields {
    RwLockFields {
        count: RwLock::new(7),
        labels: RwLock::new(vec!["x".to_owned()]),
        maybe_count: RwLock::new(Some(3)),
        maybe_tag: RwLock::new(Some(tag())),
        tag: RwLock::new(tag()),
        text: RwLock::new("t".to_owned()),
    }
}

fn cell_fields() -> CellFields {
    CellFields {
        count: Cell::new(7),
        maybe_count: Cell::new(Some(3)),
    }
}

fn plain_copy_fields() -> PlainCopyFields {
    PlainCopyFields {
        count: 7,
        maybe_count: Some(3),
    }
}

/// The field declarations of a generated `TypeScript` definition, without the `JSDoc` around them.
#[cfg(feature = "typescript")]
fn ts_field_declarations(definition: &str) -> Vec<String> {
    definition
        .lines()
        .filter(|line| line.starts_with("  ") && line.ends_with(';'))
        .map(ToOwned::to_owned)
        .collect()
}

/// One populated instance of every wrapper twin, serialized, beside the bare spelling's.
///
/// `Cell` is not among them: its fixture holds a different, reduced field set (see
/// `test_cell_field_writes_its_inner_value` and its neighbors below).
fn covered_wrapper_payloads() -> [(&'static str, serde_json::Value); 7] {
    [
        ("Box", serde_json::to_value(boxed_fields()).unwrap()),
        ("Rc", serde_json::to_value(rc_fields()).unwrap()),
        ("Arc", serde_json::to_value(arc_fields()).unwrap()),
        ("Cow", serde_json::to_value(cow_fields()).unwrap()),
        ("RefCell", serde_json::to_value(ref_cell_fields()).unwrap()),
        ("Mutex", serde_json::to_value(mutex_fields()).unwrap()),
        ("RwLock", serde_json::to_value(rwlock_fields()).unwrap()),
    ]
}

/// Every wrapper twin's generated `TypeScript`, under the bare spelling's type name.
#[cfg(feature = "typescript")]
fn covered_wrapper_ts_definitions() -> [(&'static str, String); 7] {
    [
        (
            "Box",
            BoxedFields::ts_definition().replace("BoxedFields", "PlainFields"),
        ),
        (
            "Rc",
            RcFields::ts_definition().replace("RcFields", "PlainFields"),
        ),
        (
            "Arc",
            ArcFields::ts_definition().replace("ArcFields", "PlainFields"),
        ),
        (
            "Cow",
            CowFields::ts_definition().replace("CowFields", "PlainFields"),
        ),
        (
            "RefCell",
            RefCellFields::ts_definition().replace("RefCellFields", "PlainFields"),
        ),
        (
            "Mutex",
            MutexFields::ts_definition().replace("MutexFields", "PlainFields"),
        ),
        (
            "RwLock",
            RwLockFields::ts_definition().replace("RwLockFields", "PlainFields"),
        ),
    ]
}

/// The same for Zod.
#[cfg(feature = "zod")]
fn covered_wrapper_zod_schemas() -> [(&'static str, String); 7] {
    [
        (
            "Box",
            BoxedFields::zod_schema().replace("BoxedFields", "PlainFields"),
        ),
        (
            "Rc",
            RcFields::zod_schema().replace("RcFields", "PlainFields"),
        ),
        (
            "Arc",
            ArcFields::zod_schema().replace("ArcFields", "PlainFields"),
        ),
        (
            "Cow",
            CowFields::zod_schema().replace("CowFields", "PlainFields"),
        ),
        (
            "RefCell",
            RefCellFields::zod_schema().replace("RefCellFields", "PlainFields"),
        ),
        (
            "Mutex",
            MutexFields::zod_schema().replace("MutexFields", "PlainFields"),
        ),
        (
            "RwLock",
            RwLockFields::zod_schema().replace("RwLockFields", "PlainFields"),
        ),
    ]
}

#[cfg(feature = "jsonschema")]
fn covered_wrapper_json_schemas() -> [(&'static str, serde_json::Value); 7] {
    [
        ("Box", BoxedFields::json_schema()),
        ("Rc", RcFields::json_schema()),
        ("Arc", ArcFields::json_schema()),
        ("Cow", CowFields::json_schema()),
        ("RefCell", RefCellFields::json_schema()),
        ("Mutex", MutexFields::json_schema()),
        ("RwLock", RwLockFields::json_schema()),
    ]
}

#[test]
fn test_transparent_wrapper_structs_constructible() {
    assert_eq!(*boxed_fields().count, plain_fields().count);
    assert_eq!(*rc_fields().count, plain_fields().count);
    assert_eq!(*arc_fields().count, plain_fields().count);
    assert_eq!(*cow_fields().count, plain_fields().count);
    // Not `Deref`: each reads its guarded value through its own accessor instead of `*value`.
    assert_eq!(*ref_cell_fields().count.borrow(), plain_fields().count);
    assert_eq!(
        mutex_fields().count.into_inner().unwrap(),
        plain_fields().count
    );
    assert_eq!(
        rwlock_fields().count.into_inner().unwrap(),
        plain_fields().count
    );
    assert_eq!(cell_fields().count.get(), plain_copy_fields().count);

    let node = TreeNode {
        label: "root".to_owned(),
        next: Some(Box::new(TreeNode {
            label: "leaf".to_owned(),
            next: None,
        })),
    };
    assert_eq!(node.next.unwrap().label, "leaf");
}

/// A wrapper is covered when serde writes it as its inner value — the whole criterion, and the
/// reason the twins may be held against the bare spelling at all. Read off what serde actually
/// produces, so the covered list answers to the wire rather than to a name.
#[test]
fn test_every_covered_wrapper_writes_its_inner_value() {
    let plain = serde_json::to_value(plain_fields()).unwrap();
    for (wrapper, payload) in covered_wrapper_payloads() {
        assert_eq!(payload, plain, "for: {wrapper}");
    }
}

/// The criterion holds wherever the wrapper was written, not only in field position: inside an
/// `Option`, inside a sequence, in a map's value slot, and beside fields wrapped in nothing.
#[test]
fn test_a_wrapper_written_inside_another_type_writes_the_inner_value() {
    let inside = WrappedInsideFields {
        counts: HashMap::from([("k".to_owned(), Rc::new(1_u64))]),
        elements: vec![Rc::new(tag())],
        maybe_tag: Some(Box::new(tag())),
    };
    let plain_inside = PlainInsideFields {
        counts: HashMap::from([("k".to_owned(), 1_u64)]),
        elements: vec![tag()],
        maybe_tag: Some(tag()),
    };
    assert_eq!(
        serde_json::to_value(inside).unwrap(),
        serde_json::to_value(plain_inside).unwrap()
    );

    let mixed = MixedBoxedFields {
        boxed_tag: Box::new(tag()),
        count: 7,
        plain_tag: tag(),
    };
    let mixed_plain = MixedPlainFields {
        boxed_tag: tag(),
        count: 7,
        plain_tag: tag(),
    };
    assert_eq!(
        serde_json::to_value(mixed).unwrap(),
        serde_json::to_value(mixed_plain).unwrap()
    );
}

/// Writing the same value as the bare spelling, every covered wrapper describes as it does — whole
/// schema against whole schema, not one field at a time.
#[test]
#[cfg(feature = "jsonschema")]
fn test_every_covered_wrapper_describes_as_the_inner_spelling() {
    let plain_schema = PlainFields::json_schema();
    for (wrapper, schema) in covered_wrapper_json_schemas() {
        assert_eq!(
            schema["properties"], plain_schema["properties"],
            "for: {wrapper}"
        );
        assert_eq!(
            schema["required"], plain_schema["required"],
            "for: {wrapper}"
        );
    }
}

/// The same holding on the `TypeScript` surface, with the fixture's own name set aside.
/// Declarations rather than whole definitions, because the surrounding `JSDoc` differs for a reason
/// of its own: a field written under an `Option` or a `Vec` has its doc comment dropped, so the
/// twins' `Option`- and `Vec`-inner fields carry a doc comment the bare spelling has lost.
#[test]
#[cfg(feature = "typescript")]
fn test_every_covered_wrapper_types_as_the_inner_spelling() {
    let plain_declarations = ts_field_declarations(&PlainFields::ts_definition());
    for (wrapper, definition) in covered_wrapper_ts_definitions() {
        assert_eq!(
            ts_field_declarations(&definition),
            plain_declarations,
            "for: {wrapper}"
        );
    }
}

/// And on the Zod surface.
#[test]
#[cfg(feature = "zod")]
fn test_every_covered_wrapper_validates_as_the_inner_spelling() {
    let plain_schema = PlainFields::zod_schema();
    for (wrapper, schema) in covered_wrapper_zod_schemas() {
        assert_eq!(schema, plain_schema, "for: {wrapper}");
    }
}

/// Whatever a wrapped field renders as, it is not the Rust wrapper's name: neither surface has any
/// meaning for it, so the name surviving anywhere into the output is a syntax error in the output.
#[test]
#[cfg(any(feature = "typescript", feature = "zod"))]
fn test_no_transparent_wrapper_name_survives_into_generated_output() {
    let mut generated: Vec<(&str, String)> = Vec::new();
    #[cfg(feature = "typescript")]
    generated.extend(covered_wrapper_ts_definitions());
    #[cfg(feature = "zod")]
    generated.extend(covered_wrapper_zod_schemas());

    for (wrapper, output) in generated {
        for name in TRANSPARENT_WRAPPERS {
            assert!(!output.contains(name), "{wrapper} generated: {output}");
        }
    }
}

/// A wrapper holding an `Option` is optional on the field's behalf: the wrapper is not on the wire,
/// so what a `None` costs is decided by the `Option` alone, at either side of the wrapper.
#[test]
#[cfg(feature = "jsonschema")]
fn test_wrapped_option_is_required_exactly_as_the_bare_option() {
    let expected = serde_json::json!(["count", "labels", "tag", "text"]);
    assert_eq!(PlainFields::json_schema()["required"], expected);
    for (wrapper, schema) in covered_wrapper_json_schemas() {
        assert_eq!(schema["required"], expected, "for: {wrapper}");
    }
}

/// A wrapper written inside an `Option`, a sequence or a map's value slot describes as the bare
/// element does in that same position.
#[test]
#[cfg(feature = "jsonschema")]
fn test_wrapper_inside_another_type_describes_as_the_inner_spelling() {
    let wrapped = WrappedInsideFields::json_schema();
    let plain = PlainInsideFields::json_schema();
    assert_eq!(wrapped["properties"], plain["properties"]);
    assert_eq!(wrapped["required"], plain["required"]);
}

#[test]
#[cfg(feature = "typescript")]
fn test_wrapper_inside_another_type_types_as_the_inner_spelling() {
    assert_eq!(
        WrappedInsideFields::ts_definition().replace("WrappedInsideFields", "PlainInsideFields"),
        PlainInsideFields::ts_definition()
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_wrapper_inside_another_type_validates_as_the_inner_spelling() {
    assert_eq!(
        WrappedInsideFields::zod_schema().replace("WrappedInsideFields", "PlainInsideFields"),
        PlainInsideFields::zod_schema()
    );
}

/// The collapse is one field's, so the fields written beside it render as they always did.
#[test]
#[cfg(feature = "jsonschema")]
fn test_boxed_field_leaves_its_unboxed_siblings_alone() {
    let mixed = MixedBoxedFields::json_schema();
    let plain = MixedPlainFields::json_schema();
    assert_eq!(mixed["properties"], plain["properties"]);
    assert_eq!(mixed["required"], plain["required"]);
}

/// Whole definitions here, `JSDoc` and all: no field is written under an `Option` or a `Vec`, so
/// the boxed field's documentation is the unboxed field's, down to the comment above it. What a
/// field documents as is decided by where it was written, not by what it was written under.
#[test]
#[cfg(feature = "typescript")]
fn test_boxed_field_types_beside_its_unboxed_siblings() {
    assert_eq!(
        MixedBoxedFields::ts_definition().replace("MixedBoxedFields", "MixedPlainFields"),
        MixedPlainFields::ts_definition()
    );
}

/// A `Box` is what makes a self-holding struct a type at all, and it is still not on the wire: the
/// field is the self-reference it wraps, so the recursion is seen through it and gets the deferred
/// spelling a self-reference needs.
#[test]
#[cfg(feature = "zod")]
fn test_boxed_self_reference_validates_as_the_self_reference() {
    let schema = TreeNode::zod_schema();
    assert!(schema.contains("get next()"), "Got: {schema}");
    assert!(schema.contains("label: z.string()"), "Got: {schema}");
    for name in TRANSPARENT_WRAPPERS {
        assert!(!schema.contains(name), "Got: {schema}");
    }
}

/// `Cell` writes its inner value exactly as the other covered wrappers do, over its own reduced
/// (`Copy`-only) field set.
#[test]
fn test_cell_field_writes_its_inner_value() {
    assert_eq!(
        serde_json::to_value(cell_fields()).unwrap(),
        serde_json::to_value(plain_copy_fields()).unwrap()
    );
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_cell_field_describes_as_the_inner_spelling() {
    let cell_schema = CellFields::json_schema();
    let plain_schema = PlainCopyFields::json_schema();
    assert_eq!(cell_schema["properties"], plain_schema["properties"]);
    assert_eq!(cell_schema["required"], plain_schema["required"]);
}

#[test]
#[cfg(feature = "typescript")]
fn test_cell_field_types_as_the_inner_spelling() {
    assert_eq!(
        CellFields::ts_definition().replace("CellFields", "PlainCopyFields"),
        PlainCopyFields::ts_definition()
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_cell_field_validates_as_the_inner_spelling() {
    assert_eq!(
        CellFields::zod_schema().replace("CellFields", "PlainCopyFields"),
        PlainCopyFields::zod_schema()
    );
}

#[test]
#[cfg(any(feature = "typescript", feature = "zod"))]
fn test_no_cell_name_survives_into_generated_output() {
    #[cfg(feature = "typescript")]
    {
        let ts = CellFields::ts_definition().replace("CellFields", "PlainCopyFields");
        for name in TRANSPARENT_WRAPPERS {
            assert!(!ts.contains(name), "Got: {ts}");
        }
    }
    #[cfg(feature = "zod")]
    {
        let zod = CellFields::zod_schema().replace("CellFields", "PlainCopyFields");
        for name in TRANSPARENT_WRAPPERS {
            assert!(!zod.contains(name), "Got: {zod}");
        }
    }
}
