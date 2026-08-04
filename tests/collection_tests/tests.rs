use alloc::collections::{BTreeSet, BinaryHeap, VecDeque};
use core::hash::BuildHasher;
use core::iter::once;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::DefaultHasher;
use tixschema::model_schema;

/// The covered wrappers by the name a generated surface could leak. `Vec` is covered too but
/// never arrives under its own name: the parser collapses it onto its element long before.
#[cfg(any(feature = "typescript", feature = "zod"))]
const SEQUENCE_WRAPPERS: [&str; 5] = [
    "BTreeSet",
    "BinaryHeap",
    "HashSet",
    "LinkedList",
    "VecDeque",
];

// Test comprehensive HashMap scenarios with various value types
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct ComprehensiveHashMapTest {
    bool_array: HashMap<String, Vec<bool>>,
    bool_value: HashMap<String, bool>,
    f64_array: HashMap<String, Vec<f64>>,
    f64_value: HashMap<String, f64>,
    i64_array: HashMap<String, Vec<i64>>,
    i64_value: HashMap<String, i64>,
    optional_u64: HashMap<String, Option<u64>>,
    optional_u64_array: HashMap<String, Option<Vec<u64>>>,
    string_array: HashMap<String, Vec<String>>,
    string_value: HashMap<String, String>,
    u64_array: HashMap<String, Vec<u64>>,
    u64_value: HashMap<String, u64>,
}

// Test potential edge case with HashMap containing 64-bit integers
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct HashMapWith64Bit {
    i64_map: HashMap<String, i64>,
    id: String,
    mixed_map: HashMap<String, Vec<u64>>,
    u64_map: HashMap<String, u64>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum MetricSlot {
    Daily,
    Weekly,
}

// An enum-keyed map enumerates its keys, so every member carries the value schema outright
// instead of the open `additionalProperties` a String-keyed map uses.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct EnumKeyedScalarValueMaps {
    bool_value: HashMap<MetricSlot, bool>,
    f64_value: HashMap<MetricSlot, f64>,
    i64_value: HashMap<MetricSlot, i64>,
    string_array: HashMap<MetricSlot, Vec<String>>,
    string_value: HashMap<MetricSlot, String>,
    u64_value: HashMap<MetricSlot, u64>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct MetricSample {
    label: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct EnumKeyedSiblingValueMaps {
    sample_array: HashMap<MetricSlot, Vec<MetricSample>>,
    sample_value: HashMap<MetricSlot, MetricSample>,
}

// A key that enumerates its members says nothing about what each member holds, so a value that is
// itself a map is described at every level here exactly as it is under the String key of the twin
// below — the key path decides which keys exist, never which values are renderable.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct EnumKeyedNestedMapValues {
    counts: HashMap<MetricSlot, HashMap<String, u64>>,
    labels: HashMap<MetricSlot, HashMap<String, String>>,
    rows: HashMap<MetricSlot, Vec<HashMap<String, String>>>,
    samples: HashMap<MetricSlot, HashMap<String, MetricSample>>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct StringKeyedNestedMapValues {
    counts: HashMap<String, HashMap<String, u64>>,
    labels: HashMap<String, HashMap<String, String>>,
    rows: HashMap<String, Vec<HashMap<String, String>>>,
    samples: HashMap<String, HashMap<String, MetricSample>>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum MetricBucket {
    High,
    Low,
}

/// The enum-keyed map every position below is held against: the field-position rendering, which is
/// the one that has always enumerated its key.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct BucketKeyedMap {
    counts: HashMap<MetricBucket, u64>,
    samples: HashMap<MetricBucket, MetricSample>,
}

// Which keys a map has is its key type's answer wherever the map is written, so an enum-keyed map
// enumerates its members nested under either outer key flavor and behind either slot wrap — the
// depth a map sits at cannot decide whether its keys are known.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NestedEnumKeyedMapValues {
    arrayed: HashMap<String, Vec<HashMap<MetricBucket, u64>>>,
    enum_keyed_outer: HashMap<MetricSlot, HashMap<MetricBucket, u64>>,
    optional: HashMap<String, Option<HashMap<MetricBucket, u64>>>,
    siblings: HashMap<MetricSlot, HashMap<MetricBucket, MetricSample>>,
    string_keyed_outer: HashMap<String, HashMap<MetricBucket, u64>>,
}

/// A `#[serde(transparent)]` brand over a `String`: serde writes it as the bare string its inner
/// is, which is exactly what a JSON object key is.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(transparent)]
struct CorrelationId(String);

/// A brand over such a brand. The wire form is the same bare string at every link of the chain.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(transparent)]
struct TraceId(CorrelationId);

// A brand key is the open case wearing a name: serde writes the brand as the bare string its inner
// is, so the map is an object keyed by arbitrary strings — held below against the `String`-keyed
// twin, which is what the map describes as once the brand's name is spent.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct BrandKeyedMaps {
    chained: HashMap<TraceId, u64>,
    counts: HashMap<CorrelationId, u64>,
    nested: HashMap<CorrelationId, HashMap<CorrelationId, u64>>,
    samples: HashMap<CorrelationId, MetricSample>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct StringKeyedBrandTwin {
    chained: HashMap<String, u64>,
    counts: HashMap<String, u64>,
    nested: HashMap<String, HashMap<String, u64>>,
    samples: HashMap<String, MetricSample>,
}

// A String key enumerates nothing, so one `additionalProperties` schema stands for every member —
// and it is the value type's own, which is what the enum-keyed twin above spells out per key.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct StringKeyedSiblingValueMaps {
    optional_sample: HashMap<String, Option<MetricSample>>,
    sample_array: HashMap<String, Vec<MetricSample>>,
    sample_value: HashMap<String, MetricSample>,
}

/// An alias of a sibling: its schema module is named after the registered export name, not the
/// alias ident, so the member reference has to be resolved through the registry.
#[model_schema()]
type MetricSampleRef = MetricSample;

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct StringKeyedAliasValueMaps {
    sample_array: HashMap<String, Vec<MetricSampleRef>>,
    sample_value: HashMap<String, MetricSampleRef>,
}

// A map entry cannot be dropped the way an object key can, so an `Option` value is spelled `null`
// on the wire rather than omitted — the same twin type on both key paths pins that both agree.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct OptionalMapValues {
    enum_keyed: HashMap<MetricSlot, Option<String>>,
    string_keyed: HashMap<String, Option<String>>,
}

// A sequence wrapper around a map is the field's array, not the map's, so each wrapped field
// describes as the array of the map its unwrapped twin describes as — the wrap every other field
// type applies, and the one the slot positions already apply to the same type. An `Option` is not
// such a wrapper: field position spells optionality by leaving the name out of `required`.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct WrappedMapFields {
    bucket_counts: HashMap<MetricBucket, u64>,
    labels: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_labels: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_wrapped_labels: Option<Vec<HashMap<String, String>>>,
    raw_keyed_counts: HashMap<u32, u64>,
    wrapped_bucket_counts: Vec<HashMap<MetricBucket, u64>>,
    wrapped_labels: Vec<HashMap<String, String>>,
    wrapped_raw_keyed_counts: VecDeque<HashMap<u32, u64>>,
}

// Test struct with collections
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct UserWithCollections {
    id: String,
    metadata: HashMap<String, String>,
    scores: Vec<u32>,
    tags: Vec<String>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct MetricTag {
    label: String,
}

/// An alias of a set element: the element's schema module is named after the registered export
/// name, so the reference has to be resolved through the registry rather than from the ident.
#[model_schema()]
type MetricTagRef = MetricTag;

// Every std wrapper serde writes as a JSON array of `T` — the criterion for covering one — puts
// the element in charge of what the array holds. The twin structs below carry the same element
// types under each covered spelling; the tests hold every field of one against its `Vec` twin.
// `LinkedList` has no twin here, the crate's own lints forbidding it a value of one; it is covered
// by name at the dispatch and in the renderers' unit tests instead.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct SetElementFields {
    aliased_tags: HashSet<MetricTagRef>,
    big_ids: HashSet<u64>,
    #[model_schema_prop(minLength = 3)]
    constrained_labels: HashSet<String>,
    labels: HashSet<String>,
    #[model_schema_prop(preprocess = ["trim"])]
    preprocessed_labels: HashSet<String>,
    sibling_tags: HashSet<MetricTag>,
    small_ids: HashSet<u32>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct BTreeSetElementFields {
    aliased_tags: BTreeSet<MetricTagRef>,
    big_ids: BTreeSet<u64>,
    #[model_schema_prop(minLength = 3)]
    constrained_labels: BTreeSet<String>,
    labels: BTreeSet<String>,
    #[model_schema_prop(preprocess = ["trim"])]
    preprocessed_labels: BTreeSet<String>,
    sibling_tags: BTreeSet<MetricTag>,
    small_ids: BTreeSet<u32>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct VecDequeElementFields {
    aliased_tags: VecDeque<MetricTagRef>,
    big_ids: VecDeque<u64>,
    #[model_schema_prop(minLength = 3)]
    constrained_labels: VecDeque<String>,
    labels: VecDeque<String>,
    #[model_schema_prop(preprocess = ["trim"])]
    preprocessed_labels: VecDeque<String>,
    sibling_tags: VecDeque<MetricTag>,
    small_ids: VecDeque<u32>,
}

// `BinaryHeap` is the one covered wrapper with no `PartialEq`, so this twin cannot derive it.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct BinaryHeapElementFields {
    aliased_tags: BinaryHeap<MetricTagRef>,
    big_ids: BinaryHeap<u64>,
    #[model_schema_prop(minLength = 3)]
    constrained_labels: BinaryHeap<String>,
    labels: BinaryHeap<String>,
    #[model_schema_prop(preprocess = ["trim"])]
    preprocessed_labels: BinaryHeap<String>,
    sibling_tags: BinaryHeap<MetricTag>,
    small_ids: BinaryHeap<u32>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct VecElementFields {
    aliased_tags: Vec<MetricTagRef>,
    big_ids: Vec<u64>,
    #[model_schema_prop(minLength = 3)]
    constrained_labels: Vec<String>,
    labels: Vec<String>,
    #[model_schema_prop(preprocess = ["trim"])]
    preprocessed_labels: Vec<String>,
    sibling_tags: Vec<MetricTag>,
    small_ids: Vec<u32>,
}

/// A `BuildHasher` written where std implies one, so a field can name the hasher parameter both
/// `HashMap` and `HashSet` carry past the types they write. What it hashes with is beside the
/// point — serde writes the same bytes whichever hasher a container is built with, which is why the
/// argument is not part of the wire form the surfaces render.
#[derive(Clone, Default)]
struct NamedHasher;

impl BuildHasher for NamedHasher {
    type Hasher = DefaultHasher;

    fn build_hasher(&self) -> Self::Hasher {
        DefaultHasher::new()
    }
}

// The twin pair the hasher is read through: the same containers at the same element and key types,
// one spelling naming the hasher parameter and one leaving it implied. Every surface holds the two
// against each other, the way the sequence wrappers are held against their `Vec` spelling.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct HasherNamedFields {
    counts: HashMap<String, u32, NamedHasher>,
    enum_keyed_counts: HashMap<MetricSlot, u32, NamedHasher>,
    nested_ids: HashMap<String, HashSet<u32, NamedHasher>>,
    small_ids: HashSet<u32, NamedHasher>,
    tags: HashSet<MetricTag, NamedHasher>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct HasherImpliedFields {
    counts: HashMap<String, u32>,
    enum_keyed_counts: HashMap<MetricSlot, u32>,
    nested_ids: HashMap<String, HashSet<u32>>,
    small_ids: HashSet<u32>,
    tags: HashSet<MetricTag>,
}

// A slot — a map member, a tuple element — cannot be dropped the way an object key can, so it
// holds whatever the value writes, and a covered wrapper writes the array its element decides. The
// twin below carries the `Vec` spelling of the same slots, which is what each set slot is held
// against: the key path, the element type and the nesting are the same on both.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct SetSlotValues {
    aliased_tags: HashMap<String, BTreeSet<MetricTagRef>>,
    enum_keyed_ids: HashMap<MetricSlot, HashSet<u32>>,
    enum_keyed_tags: HashMap<MetricSlot, BTreeSet<MetricTag>>,
    labels: HashMap<String, BTreeSet<String>>,
    optional_element_ids: HashMap<String, HashSet<Option<u32>>>,
    optional_ids: HashMap<String, Option<HashSet<u32>>>,
    sibling_tags: HashMap<String, HashSet<MetricTag>>,
    small_ids: HashMap<String, HashSet<u32>>,
    tuple_ids: (String, HashSet<u32>),
    tuple_labels: (u32, BTreeSet<String>),
    tuple_optional_ids: (String, BTreeSet<Option<u32>>),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct VecSlotValues {
    aliased_tags: HashMap<String, Vec<MetricTagRef>>,
    enum_keyed_ids: HashMap<MetricSlot, Vec<u32>>,
    enum_keyed_tags: HashMap<MetricSlot, Vec<MetricTag>>,
    labels: HashMap<String, Vec<String>>,
    optional_element_ids: HashMap<String, Vec<Option<u32>>>,
    optional_ids: HashMap<String, Option<Vec<u32>>>,
    sibling_tags: HashMap<String, Vec<MetricTag>>,
    small_ids: HashMap<String, Vec<u32>>,
    tuple_ids: (String, Vec<u32>),
    tuple_labels: (u32, Vec<String>),
    tuple_optional_ids: (String, Vec<Option<u32>>),
}

// A sibling is carried by reference wherever it sits, so a tuple element names the schema module a
// field and a map member name — under a sequence wrapper, the array of that reference, the wrapper
// having normalized onto the element like any other.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct SiblingSlotValues {
    bare: (String, MetricTag),
    setted: (String, HashSet<MetricTag>),
    vecced: (String, Vec<MetricTag>),
}

/// An alias whose target is a nested sequence: the alias publishes what a field written as the
/// target publishes, so the levels have to survive the alias too.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[model_schema()]
type MetricGrid = Vec<Vec<u32>>;

// A sequence holding a sequence writes an array of arrays, so it describes as one: every level the
// field is written at is a level of the description, whichever covered wrapper spells each level.
// A constraint still has only the innermost element to land on — the levels above hold arrays, not
// values a `minLength` could reach.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NestedSequenceFields {
    aliased_rows: Vec<Vec<MetricTagRef>>,
    #[model_schema_prop(minLength = 3)]
    constrained_rows: Vec<Vec<String>>,
    deep_ids: Vec<Vec<Vec<u32>>>,
    fixed_grid: [[u32; 2]; 2],
    labels: Vec<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    optional_rows: Option<Vec<Vec<u32>>>,
    set_of_rows: HashSet<Vec<u32>>,
    sibling_rows: Vec<Vec<MetricTag>>,
    small_ids: Vec<Vec<u32>>,
    vec_of_sets: Vec<BTreeSet<u32>>,
}

// The same nesting in the slots that cannot be dropped — a map member, a tuple element — where the
// wrapper chain is normalized onto the element before the slot wraps go on.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NestedSequenceSlots {
    enum_keyed_rows: HashMap<MetricSlot, Vec<Vec<u32>>>,
    optional_rows: HashMap<String, Option<Vec<Vec<u32>>>>,
    rows: HashMap<String, Vec<Vec<u32>>>,
    set_rows: HashMap<String, HashSet<Vec<u32>>>,
    sibling_rows: HashMap<String, Vec<Vec<MetricTag>>>,
    tuple_rows: (String, Vec<Vec<u32>>),
}

// A doc comment is written on the field, not on the shape its type spells, so the wrappers the
// parser collapses onto their element describe with the same comment the spellings it leaves alone
// describe with.
#[cfg(feature = "typescript")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct DocumentedSequenceFields {
    /// Documented nested field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    documented_nested: Option<Vec<u32>>,
    /// Documented optional field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    documented_optional: Option<String>,
    /// Documented set field.
    documented_set: HashSet<String>,
    /// Documented vec field.
    documented_vec: Vec<String>,
    undocumented_vec: Vec<String>,
}

fn one<T, C>(item: T) -> C
where
    C: FromIterator<T>,
{
    once(item).collect()
}

fn metric_tag() -> MetricTag {
    MetricTag {
        label: "a".to_owned(),
    }
}

/// One populated `SetSlotValues`, with a single member in every slot the fixture opens — one
/// member, so the wire it writes is a value the `Vec` twin's wire can be held against outright
/// rather than a shape a set's iteration order could reorder.
fn set_slot_values() -> SetSlotValues {
    SetSlotValues {
        aliased_tags: HashMap::from([("k".to_owned(), one(metric_tag()))]),
        enum_keyed_ids: HashMap::from([(MetricSlot::Daily, one(7_u32))]),
        enum_keyed_tags: HashMap::from([(MetricSlot::Daily, one(metric_tag()))]),
        labels: HashMap::from([("k".to_owned(), one("t".to_owned()))]),
        optional_element_ids: HashMap::from([("k".to_owned(), one(None))]),
        optional_ids: HashMap::from([("k".to_owned(), Some(one(7_u32)))]),
        sibling_tags: HashMap::from([("k".to_owned(), one(metric_tag()))]),
        small_ids: HashMap::from([("k".to_owned(), one(7_u32))]),
        tuple_ids: ("t".to_owned(), one(7_u32)),
        tuple_labels: (7, one("t".to_owned())),
        tuple_optional_ids: ("t".to_owned(), one(None)),
    }
}

/// The same members under the `Vec` spelling of every slot.
fn vec_slot_values() -> VecSlotValues {
    VecSlotValues {
        aliased_tags: HashMap::from([("k".to_owned(), one(metric_tag()))]),
        enum_keyed_ids: HashMap::from([(MetricSlot::Daily, one(7_u32))]),
        enum_keyed_tags: HashMap::from([(MetricSlot::Daily, one(metric_tag()))]),
        labels: HashMap::from([("k".to_owned(), one("t".to_owned()))]),
        optional_element_ids: HashMap::from([("k".to_owned(), one(None))]),
        optional_ids: HashMap::from([("k".to_owned(), Some(one(7_u32)))]),
        sibling_tags: HashMap::from([("k".to_owned(), one(metric_tag()))]),
        small_ids: HashMap::from([("k".to_owned(), one(7_u32))]),
        tuple_ids: ("t".to_owned(), one(7_u32)),
        tuple_labels: (7, one("t".to_owned())),
        tuple_optional_ids: ("t".to_owned(), one(None)),
    }
}

/// The same sibling in each slot the fixture opens: bare, and under each wrapper spelling.
fn sibling_slot_values() -> SiblingSlotValues {
    SiblingSlotValues {
        bare: ("t".to_owned(), metric_tag()),
        setted: ("t".to_owned(), one(metric_tag())),
        vecced: ("t".to_owned(), vec![metric_tag()]),
    }
}

/// One populated `NestedSequenceFields`, every field holding a single innermost value so the wire
/// it writes is read for its nesting rather than for its order.
fn nested_sequence_fields() -> NestedSequenceFields {
    NestedSequenceFields {
        aliased_rows: vec![vec![metric_tag()]],
        constrained_rows: vec![vec!["abc".to_owned()]],
        deep_ids: vec![vec![vec![7_u32]]],
        fixed_grid: [[7_u32, 8_u32], [9_u32, 10_u32]],
        labels: vec![vec!["t".to_owned()]],
        optional_rows: Some(vec![vec![7_u32]]),
        set_of_rows: one(vec![7_u32]),
        sibling_rows: vec![vec![metric_tag()]],
        small_ids: vec![vec![7_u32]],
        vec_of_sets: vec![one(7_u32)],
    }
}

/// The same nesting in every slot the slot fixture opens.
fn nested_sequence_slots() -> NestedSequenceSlots {
    NestedSequenceSlots {
        enum_keyed_rows: HashMap::from([(MetricSlot::Daily, vec![vec![7_u32]])]),
        optional_rows: HashMap::from([("k".to_owned(), Some(vec![vec![7_u32]]))]),
        rows: HashMap::from([("k".to_owned(), vec![vec![7_u32]])]),
        set_rows: HashMap::from([("k".to_owned(), one(vec![7_u32]))]),
        sibling_rows: HashMap::from([("k".to_owned(), vec![vec![metric_tag()]])]),
        tuple_rows: ("t".to_owned(), vec![vec![7_u32]]),
    }
}

/// The `name: Type;` lines of a generated TypeScript definition, the `JSDoc` blocks around them
/// left out — a member line is indented and terminated, where every comment line carries a `*`.
#[cfg(feature = "typescript")]
fn ts_field_declarations(definition: &str) -> Vec<String> {
    definition
        .lines()
        .filter(|line| line.starts_with("  ") && line.ends_with(';'))
        .map(ToOwned::to_owned)
        .collect()
}

/// One populated instance of every wrapper twin the crate may hold, serialized.
fn covered_wrapper_payloads() -> [(&'static str, serde_json::Value); 4] {
    let tag = MetricTag {
        label: "a".to_owned(),
    };
    [
        (
            "HashSet",
            serde_json::to_value(SetElementFields {
                aliased_tags: one(tag.clone()),
                big_ids: one(9_u64),
                constrained_labels: one("abc".to_owned()),
                labels: one("t".to_owned()),
                preprocessed_labels: one("t".to_owned()),
                sibling_tags: one(tag.clone()),
                small_ids: one(7_u32),
            })
            .unwrap(),
        ),
        (
            "BTreeSet",
            serde_json::to_value(BTreeSetElementFields {
                aliased_tags: one(tag.clone()),
                big_ids: one(9_u64),
                constrained_labels: one("abc".to_owned()),
                labels: one("t".to_owned()),
                preprocessed_labels: one("t".to_owned()),
                sibling_tags: one(tag.clone()),
                small_ids: one(7_u32),
            })
            .unwrap(),
        ),
        (
            "BinaryHeap",
            serde_json::to_value(BinaryHeapElementFields {
                aliased_tags: one(tag.clone()),
                big_ids: one(9_u64),
                constrained_labels: one("abc".to_owned()),
                labels: one("t".to_owned()),
                preprocessed_labels: one("t".to_owned()),
                sibling_tags: one(tag.clone()),
                small_ids: one(7_u32),
            })
            .unwrap(),
        ),
        (
            "VecDeque",
            serde_json::to_value(VecDequeElementFields {
                aliased_tags: one(tag.clone()),
                big_ids: one(9_u64),
                constrained_labels: one("abc".to_owned()),
                labels: one("t".to_owned()),
                preprocessed_labels: one("t".to_owned()),
                sibling_tags: one(tag),
                small_ids: one(7_u32),
            })
            .unwrap(),
        ),
    ]
}

#[cfg(feature = "jsonschema")]
fn assert_hashmap_primitive_type(
    properties: &serde_json::Map<String, serde_json::Value>,
    field_name: &str,
    expected_type: &str,
) {
    assert_eq!(properties[field_name]["type"], "object");
    assert_eq!(
        properties[field_name]["additionalProperties"]["type"],
        expected_type
    );
}

#[cfg(feature = "jsonschema")]
fn assert_hashmap_array_type(
    properties: &serde_json::Map<String, serde_json::Value>,
    field_name: &str,
    expected_item_type: &str,
) {
    assert_eq!(properties[field_name]["type"], "object");
    assert_eq!(
        properties[field_name]["additionalProperties"]["type"],
        "array"
    );
    assert_eq!(
        properties[field_name]["additionalProperties"]["items"]["type"],
        expected_item_type
    );
}

/// The JSON Schema `type` a value carries on the wire, so an emitted schema can be held against
/// what serde actually produces for it.
#[cfg(feature = "jsonschema")]
const fn json_type_name(value: &serde_json::Value) -> &'static str {
    match *value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

#[cfg(feature = "jsonschema")]
fn assert_enum_keyed_map_value(
    properties: &serde_json::Map<String, serde_json::Value>,
    field_name: &str,
    expected_value_schema: &serde_json::Value,
) {
    let field = &properties[field_name];
    assert_eq!(field["type"], "object", "in: {field}");
    assert_eq!(field["additionalProperties"], false, "in: {field}");
    let members = MetricSlot::enum_members();
    assert_eq!(
        field["properties"].as_object().unwrap().len(),
        members.len(),
        "in: {field}"
    );
    for member in members {
        assert_eq!(
            &field["properties"][&member], expected_value_schema,
            "member {member} in: {field}"
        );
    }
}

#[test]
fn test_collection_structs_constructible() {
    let comprehensive = ComprehensiveHashMapTest {
        bool_array: HashMap::new(),
        bool_value: HashMap::new(),
        f64_array: HashMap::new(),
        f64_value: HashMap::new(),
        i64_array: HashMap::new(),
        i64_value: HashMap::new(),
        optional_u64: HashMap::new(),
        optional_u64_array: HashMap::new(),
        string_array: HashMap::new(),
        string_value: HashMap::new(),
        u64_array: HashMap::new(),
        u64_value: HashMap::new(),
    };
    assert!(comprehensive.bool_value.is_empty());
    let with_64bit = HashMapWith64Bit {
        i64_map: HashMap::new(),
        id: String::new(),
        mixed_map: HashMap::new(),
        u64_map: HashMap::new(),
    };
    assert!(with_64bit.id.is_empty());
    let with_collections = UserWithCollections {
        id: String::new(),
        metadata: HashMap::new(),
        scores: Vec::new(),
        tags: Vec::new(),
    };
    assert!(with_collections.id.is_empty());
    let optional_values = OptionalMapValues {
        enum_keyed: HashMap::from([(MetricSlot::Daily, None)]),
        string_keyed: HashMap::from([("k".to_owned(), None)]),
    };
    assert_eq!(optional_values.enum_keyed[&MetricSlot::Daily], None);
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_collections_json_schema() {
    let schema = UserWithCollections::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    // Check array properties
    assert_eq!(properties["tags"]["type"], "array");
    assert_eq!(properties["tags"]["items"]["type"], "string");

    assert_eq!(properties["scores"]["type"], "array");
    assert_eq!(properties["scores"]["items"]["type"], "integer");

    // Check map properties
    assert_eq!(properties["metadata"]["type"], "object");
    assert_eq!(
        properties["metadata"]["additionalProperties"]["type"],
        "string"
    );
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_collections_ts_definition() {
    let ts_definition = UserWithCollections::ts_definition();

    // Check TypeScript array types
    assert!(ts_definition.contains("tags: Array<string>;"));
    assert!(ts_definition.contains("scores: Array<number>;"));
    // HashMap becomes Partial<Record<...>> in the generated output
    assert!(ts_definition.contains("metadata: Partial<Record<string, string>>;"));

    // Check Zod schema - now in separate method
    let zod_schema = UserWithCollections::zod_schema();
    assert!(zod_schema.contains("tags: z.array(z.string())"));
    assert!(zod_schema.contains("scores: z.array(z.number().int())"));
    assert!(zod_schema.contains("metadata: z.record(z.string(), z.string())"));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_comprehensive_hashmap_json_schema() {
    let schema = ComprehensiveHashMapTest::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    // Test simple primitive values
    assert_hashmap_primitive_type(properties, "string_value", "string");
    assert_hashmap_primitive_type(properties, "u64_value", "integer");
    assert_hashmap_primitive_type(properties, "i64_value", "integer");
    assert_hashmap_primitive_type(properties, "f64_value", "number");
    assert_hashmap_primitive_type(properties, "bool_value", "boolean");

    // Test array values
    assert_hashmap_array_type(properties, "string_array", "string");
    assert_hashmap_array_type(properties, "u64_array", "integer");
    assert_hashmap_array_type(properties, "i64_array", "integer");
    assert_hashmap_array_type(properties, "f64_array", "number");
    assert_hashmap_array_type(properties, "bool_array", "boolean");

    // An `Option` value is nullable rather than absent — the array wrap sits inside the union,
    // because the `Option` is the outer one in `Option<Vec<T>>`.
    assert_eq!(
        properties["optional_u64"]["additionalProperties"],
        serde_json::json!({ "anyOf": [{ "type": "integer" }, { "type": "null" }] })
    );
    assert_eq!(
        properties["optional_u64_array"]["additionalProperties"],
        serde_json::json!({
            "anyOf": [
                { "type": "array", "items": { "type": "integer" } },
                { "type": "null" }
            ]
        })
    );
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_comprehensive_hashmap_typescript_generation() {
    let ts_definition = ComprehensiveHashMapTest::ts_definition();

    // Test TypeScript type generation for simple values
    assert!(ts_definition.contains("string_value: Partial<Record<string, string>>;"));
    assert!(ts_definition.contains("u64_value: Partial<Record<string, number>>;"));
    assert!(ts_definition.contains("i64_value: Partial<Record<string, number>>;"));
    assert!(ts_definition.contains("f64_value: Partial<Record<string, number>>;"));
    assert!(ts_definition.contains("bool_value: Partial<Record<string, boolean>>;"));

    // Test TypeScript type generation for array values
    assert!(ts_definition.contains("string_array: Partial<Record<string, Array<string>>>;"));
    assert!(ts_definition.contains("u64_array: Partial<Record<string, Array<number>>>;"));
    assert!(ts_definition.contains("i64_array: Partial<Record<string, Array<number>>>;"));
    assert!(ts_definition.contains("f64_array: Partial<Record<string, Array<number>>>;"));
    assert!(ts_definition.contains("bool_array: Partial<Record<string, Array<boolean>>>;"));

    // Test Zod schema generation for simple values - now in separate method
    let zod_schema = ComprehensiveHashMapTest::zod_schema();
    assert!(zod_schema.contains("string_value: z.record(z.string(), z.string())"));
    assert!(zod_schema.contains("u64_value: z.record(z.string(), z.number().int())"));
    assert!(zod_schema.contains("i64_value: z.record(z.string(), z.number().int())"));
    assert!(zod_schema.contains("f64_value: z.record(z.string(), z.number())"));
    assert!(zod_schema.contains("bool_value: z.record(z.string(), z.boolean())"));

    // Test Zod schema generation for array values
    assert!(zod_schema.contains("string_array: z.record(z.string(), z.array(z.string()))"));
    assert!(zod_schema.contains("u64_array: z.record(z.string(), z.array(z.number().int()))"));
    assert!(zod_schema.contains("i64_array: z.record(z.string(), z.array(z.number().int()))"));
    assert!(zod_schema.contains("f64_array: z.record(z.string(), z.array(z.number()))"));
    assert!(zod_schema.contains("bool_array: z.record(z.string(), z.array(z.boolean()))"));

    // An `Option` map value is null-flavored, not undefined-flavored: the entry it sits in cannot
    // be dropped, so serde writes the `None` as `null`.
    assert!(
        ts_definition.contains("optional_u64: Partial<Record<string, number | null>>;"),
        "Got: {ts_definition}"
    );
    assert!(
        ts_definition
            .contains("optional_u64_array: Partial<Record<string, Array<number> | null>>;"),
        "Got: {ts_definition}"
    );
    assert!(
        zod_schema.contains("optional_u64: z.record(z.string(), z.nullable(z.number().int()))"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema.contains(
            "optional_u64_array: z.record(z.string(), z.nullable(z.array(z.number().int())))"
        ),
        "Got: {zod_schema}"
    );
}

#[test]
fn test_enum_keyed_scalar_value_maps_constructible() {
    let maps = EnumKeyedScalarValueMaps {
        bool_value: HashMap::new(),
        f64_value: HashMap::new(),
        i64_value: HashMap::new(),
        string_array: HashMap::new(),
        string_value: HashMap::from([
            (MetricSlot::Daily, "d".to_owned()),
            (MetricSlot::Weekly, "w".to_owned()),
        ]),
        u64_value: HashMap::new(),
    };
    assert_eq!(
        maps.string_value.get(&MetricSlot::Daily),
        Some(&"d".to_owned())
    );
    assert_eq!(
        maps.string_value.get(&MetricSlot::Weekly),
        Some(&"w".to_owned())
    );
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_enum_keyed_scalar_value_maps_json_schema() {
    let schema = EnumKeyedScalarValueMaps::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    assert_enum_keyed_map_value(
        properties,
        "string_value",
        &serde_json::json!({ "type": "string" }),
    );
    assert_enum_keyed_map_value(
        properties,
        "u64_value",
        &serde_json::json!({ "type": "integer" }),
    );
    assert_enum_keyed_map_value(
        properties,
        "i64_value",
        &serde_json::json!({ "type": "integer" }),
    );
    assert_enum_keyed_map_value(
        properties,
        "f64_value",
        &serde_json::json!({ "type": "number" }),
    );
    assert_enum_keyed_map_value(
        properties,
        "bool_value",
        &serde_json::json!({ "type": "boolean" }),
    );
    assert_enum_keyed_map_value(
        properties,
        "string_array",
        &serde_json::json!({ "type": "array", "items": { "type": "string" } }),
    );
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_enum_keyed_scalar_value_maps_typescript_generation() {
    let ts_definition = EnumKeyedScalarValueMaps::ts_definition();

    assert!(
        ts_definition.contains("string_value: Partial<Record<MetricSlot, string>>;"),
        "Got: {ts_definition}"
    );
    assert!(
        ts_definition.contains("u64_value: Partial<Record<MetricSlot, number>>;"),
        "Got: {ts_definition}"
    );
    assert!(
        ts_definition.contains("bool_value: Partial<Record<MetricSlot, boolean>>;"),
        "Got: {ts_definition}"
    );
    assert!(
        ts_definition.contains("string_array: Partial<Record<MetricSlot, Array<string>>>;"),
        "Got: {ts_definition}"
    );

    let zod_schema = EnumKeyedScalarValueMaps::zod_schema();
    assert!(
        zod_schema.contains("string_value: z.record(MetricSlot$Schema, z.string())"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema.contains("u64_value: z.record(MetricSlot$Schema, z.number().int())"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema.contains("bool_value: z.record(MetricSlot$Schema, z.boolean())"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema.contains("string_array: z.record(MetricSlot$Schema, z.array(z.string()))"),
        "Got: {zod_schema}"
    );
}

#[test]
fn test_enum_keyed_sibling_value_maps_constructible() {
    let maps = EnumKeyedSiblingValueMaps {
        sample_array: HashMap::from([(
            MetricSlot::Daily,
            vec![MetricSample {
                label: "d".to_owned(),
            }],
        )]),
        sample_value: HashMap::new(),
    };
    assert_eq!(maps.sample_array[&MetricSlot::Daily].len(), 1);
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_enum_keyed_sibling_value_maps_json_schema() {
    let sample_schema = MetricSample::json_schema();
    let schema = EnumKeyedSiblingValueMaps::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    assert_enum_keyed_map_value(properties, "sample_value", &sample_schema);
    assert_enum_keyed_map_value(
        properties,
        "sample_array",
        &serde_json::json!({ "type": "array", "items": sample_schema }),
    );
}

/// A `Vec` map value is an array of siblings on the wire, so the member schema has to admit that
/// form and turn away the single sibling object a dropped array wrap would have accepted.
#[test]
#[cfg(feature = "jsonschema")]
fn test_enum_keyed_sibling_array_member_matches_the_serialized_form() {
    let sample = MetricSample {
        label: "d".to_owned(),
    };
    let maps = EnumKeyedSiblingValueMaps {
        sample_array: HashMap::from([(MetricSlot::Daily, vec![sample.clone()])]),
        sample_value: HashMap::new(),
    };
    let payload = serde_json::to_value(&maps).unwrap();
    let member = &EnumKeyedSiblingValueMaps::json_schema()["properties"]["sample_array"]["properties"]
        ["Daily"];

    assert_eq!(
        member["type"],
        json_type_name(&payload["sample_array"]["Daily"]),
        "in: {member}"
    );
    assert_eq!(member["items"], MetricSample::json_schema(), "in: {member}");
    assert_ne!(
        member["type"],
        json_type_name(&serde_json::to_value(&sample).unwrap()),
        "in: {member}"
    );
}

/// What serde writes for a map whose key is wrapped in a sequence: nothing at all. A JSON object
/// key is a string, a sequence has no string form, and `serde_json` refuses the whole value rather
/// than falling back to an array of pairs — the refusal is the wrapper's, not the element's, so the
/// fixed-size spelling earns it as squarely as the `Vec` one. The bare key beside them is the form
/// that does write, and the one `#[model_schema()]` describes; a sequence-wrapped key describes no
/// wire at all, which is why the expansion refuses the spelling instead of enumerating its element.
#[test]
fn test_a_sequence_wrapped_map_key_has_no_wire_form() {
    let vec_keyed = HashMap::from([(vec![MetricSlot::Daily], 1_u32)]);
    assert_eq!(
        serde_json::to_value(&vec_keyed).unwrap_err().to_string(),
        "key must be a string"
    );

    let array_keyed = HashMap::from([([MetricSlot::Daily; 1], 1_u32)]);
    assert_eq!(
        serde_json::to_value(&array_keyed).unwrap_err().to_string(),
        "key must be a string"
    );

    let bare_keyed = HashMap::from([(MetricSlot::Daily, 1_u32)]);
    assert_eq!(
        serde_json::to_value(&bare_keyed).unwrap(),
        serde_json::json!({ "Daily": 1_u32 })
    );
}

#[test]
fn test_nested_map_values_constructible_on_both_key_paths() {
    let enum_keyed = EnumKeyedNestedMapValues {
        counts: HashMap::new(),
        labels: HashMap::from([(
            MetricSlot::Daily,
            HashMap::from([("a".to_owned(), "one".to_owned())]),
        )]),
        rows: HashMap::new(),
        samples: HashMap::new(),
    };
    assert_eq!(enum_keyed.labels[&MetricSlot::Daily]["a"], "one");

    let string_keyed = StringKeyedNestedMapValues {
        counts: HashMap::new(),
        labels: HashMap::from([(
            "daily".to_owned(),
            HashMap::from([("a".to_owned(), "one".to_owned())]),
        )]),
        rows: HashMap::new(),
        samples: HashMap::new(),
    };
    assert_eq!(string_keyed.labels["daily"]["a"], "one");
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_enum_keyed_nested_map_values_json_schema() {
    let schema = EnumKeyedNestedMapValues::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    assert_enum_keyed_map_value(
        properties,
        "counts",
        &serde_json::json!({
            "type": "object",
            "additionalProperties": { "type": "integer" }
        }),
    );
    assert_enum_keyed_map_value(
        properties,
        "labels",
        &serde_json::json!({
            "type": "object",
            "additionalProperties": { "type": "string" }
        }),
    );
    assert_enum_keyed_map_value(
        properties,
        "rows",
        &serde_json::json!({
            "type": "array",
            "items": {
                "type": "object",
                "additionalProperties": { "type": "string" }
            }
        }),
    );
    assert_enum_keyed_map_value(
        properties,
        "samples",
        &serde_json::json!({
            "type": "object",
            "additionalProperties": MetricSample::json_schema()
        }),
    );
}

/// Both key paths render a map value through the same member dispatcher, so the member an enum key
/// spells out per key and the one a `String` key states once are the same schema — the twin types
/// hold that to the value types, field by field.
#[test]
#[cfg(feature = "jsonschema")]
fn test_enum_keyed_nested_map_values_match_their_string_keyed_members() {
    let string_keyed = StringKeyedNestedMapValues::json_schema();
    let enum_keyed = EnumKeyedNestedMapValues::json_schema();
    let properties = enum_keyed["properties"].as_object().unwrap();

    for field_name in ["counts", "labels", "rows", "samples"] {
        let member = &string_keyed["properties"][field_name]["additionalProperties"];
        assert_ne!(*member, serde_json::json!(true), "for {field_name}");
        assert_enum_keyed_map_value(properties, field_name, member);
    }
}

/// TypeScript and Zod recurse through a map value whatever the key is, so the nesting they render
/// under an enum key is the one the JSON schema now describes — pinned so the three surfaces stay
/// in step.
#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_enum_keyed_nested_map_values_typescript_generation() {
    let ts_definition = EnumKeyedNestedMapValues::ts_definition();
    for expected in [
        "counts: Partial<Record<MetricSlot, Partial<Record<string, number>>>>;",
        "labels: Partial<Record<MetricSlot, Partial<Record<string, string>>>>;",
        "rows: Partial<Record<MetricSlot, Array<Partial<Record<string, string>>>>>;",
        "samples: Partial<Record<MetricSlot, Partial<Record<string, MetricSample>>>>;",
    ] {
        assert!(
            ts_definition.contains(expected),
            "missing {expected}, got: {ts_definition}"
        );
    }

    let zod_schema = EnumKeyedNestedMapValues::zod_schema();
    for expected in [
        "counts: z.record(MetricSlot$Schema, z.record(z.string(), z.number().int())),",
        "labels: z.record(MetricSlot$Schema, z.record(z.string(), z.string())),",
        "rows: z.record(MetricSlot$Schema, z.array(z.record(z.string(), z.string()))),",
        "samples: z.record(MetricSlot$Schema, z.record(z.string(), MetricSample$Schema)),",
    ] {
        assert!(
            zod_schema.contains(expected),
            "missing {expected}, got: {zod_schema}"
        );
    }
}

/// The rendering `HashMap<MetricBucket, u64>` carries in field position — the one every nested
/// position is held against, because field position is where an enum key has always enumerated.
#[cfg(feature = "jsonschema")]
fn bucket_keyed_count_map() -> serde_json::Value {
    BucketKeyedMap::json_schema()["properties"]["counts"].clone()
}

#[test]
fn test_nested_enum_keyed_map_values_constructible() {
    let nested = NestedEnumKeyedMapValues {
        arrayed: HashMap::new(),
        enum_keyed_outer: HashMap::from([(
            MetricSlot::Daily,
            HashMap::from([(MetricBucket::Low, 3_u64)]),
        )]),
        optional: HashMap::from([("a".to_owned(), None)]),
        siblings: HashMap::new(),
        string_keyed_outer: HashMap::from([(
            "a".to_owned(),
            HashMap::from([(MetricBucket::High, 7_u64)]),
        )]),
    };
    assert_eq!(
        nested.enum_keyed_outer[&MetricSlot::Daily][&MetricBucket::Low],
        3
    );
    assert_eq!(nested.string_keyed_outer["a"][&MetricBucket::High], 7);
    assert_eq!(nested.optional["a"], None);

    let field_position = BucketKeyedMap {
        counts: HashMap::from([(MetricBucket::High, 7_u64)]),
        samples: HashMap::new(),
    };
    assert_eq!(field_position.counts[&MetricBucket::High], 7);
}

/// An enum-keyed map is the same object wherever it is written: nested under a `String` key, under
/// an enum key, and behind either slot wrap, it carries the members its own key enumerates rather
/// than the open object a position that could not reach the key would leave.
#[test]
#[cfg(feature = "jsonschema")]
fn test_nested_enum_keyed_map_values_enumerate_their_inner_key() {
    let expected = bucket_keyed_count_map();
    assert_eq!(expected["additionalProperties"], false, "in: {expected}");
    assert_eq!(
        expected["properties"].as_object().unwrap().len(),
        MetricBucket::enum_members().len(),
        "in: {expected}"
    );

    let schema = NestedEnumKeyedMapValues::json_schema();
    let properties = &schema["properties"];

    assert_eq!(
        properties["string_keyed_outer"]["additionalProperties"], expected,
        "in: {}",
        properties["string_keyed_outer"]
    );
    assert_eq!(
        properties["arrayed"]["additionalProperties"]["items"], expected,
        "in: {}",
        properties["arrayed"]
    );
    assert_eq!(
        properties["optional"]["additionalProperties"],
        serde_json::json!({ "anyOf": [expected, { "type": "null" }] }),
        "in: {}",
        properties["optional"]
    );
    for member in MetricSlot::enum_members() {
        assert_eq!(
            properties["enum_keyed_outer"]["properties"][&member], expected,
            "member {member} in: {}",
            properties["enum_keyed_outer"]
        );
        assert_eq!(
            properties["siblings"]["properties"][&member],
            BucketKeyedMap::json_schema()["properties"]["samples"],
            "member {member} in: {}",
            properties["siblings"]
        );
    }
}

/// The enumerated members are the keys serde actually writes, so a payload's inner keys are named
/// by the schema rather than admitted by an open member set.
#[test]
#[cfg(feature = "jsonschema")]
fn test_nested_enum_keyed_map_members_match_the_serialized_keys() {
    let nested = NestedEnumKeyedMapValues {
        arrayed: HashMap::new(),
        enum_keyed_outer: HashMap::new(),
        optional: HashMap::new(),
        siblings: HashMap::new(),
        string_keyed_outer: HashMap::from([(
            "a".to_owned(),
            HashMap::from([(MetricBucket::High, 7_u64)]),
        )]),
    };
    let payload = serde_json::to_value(&nested).unwrap();
    let written = payload["string_keyed_outer"]["a"].as_object().unwrap();
    let member = &NestedEnumKeyedMapValues::json_schema()["properties"]["string_keyed_outer"]["additionalProperties"];

    for key in written.keys() {
        assert_eq!(
            member["properties"][key],
            serde_json::json!({ "type": "integer" }),
            "key {key} in: {member}"
        );
    }
}

/// TypeScript and Zod recurse through a map value whatever the key is, at whatever depth, so the
/// members the JSON schema now spells out under a nested enum key are the ones those two surfaces
/// have always named — pinned so the three stay in step.
#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_nested_enum_keyed_map_values_typescript_generation() {
    let ts_definition = NestedEnumKeyedMapValues::ts_definition();
    for expected in [
        "arrayed: Partial<Record<string, Array<Partial<Record<MetricBucket, number>>>>>;",
        "enum_keyed_outer: Partial<Record<MetricSlot, Partial<Record<MetricBucket, number>>>>;",
        "optional: Partial<Record<string, Partial<Record<MetricBucket, number>> | null>>;",
        "siblings: Partial<Record<MetricSlot, Partial<Record<MetricBucket, MetricSample>>>>;",
        "string_keyed_outer: Partial<Record<string, Partial<Record<MetricBucket, number>>>>;",
    ] {
        assert!(
            ts_definition.contains(expected),
            "missing {expected}, got: {ts_definition}"
        );
    }

    let zod_schema = NestedEnumKeyedMapValues::zod_schema();
    for expected in [
        "arrayed: z.record(z.string(), z.array(z.record(MetricBucket$Schema, z.number().int()))),",
        "enum_keyed_outer: z.record(MetricSlot$Schema, z.record(MetricBucket$Schema, z.number().int())),",
        "optional: z.record(z.string(), z.nullable(z.record(MetricBucket$Schema, z.number().int()))),",
        "siblings: z.record(MetricSlot$Schema, z.record(MetricBucket$Schema, MetricSample$Schema)),",
        "string_keyed_outer: z.record(z.string(), z.record(MetricBucket$Schema, z.number().int())),",
    ] {
        assert!(
            zod_schema.contains(expected),
            "missing {expected}, got: {zod_schema}"
        );
    }
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_enum_keyed_sibling_value_maps_typescript_generation() {
    let ts_definition = EnumKeyedSiblingValueMaps::ts_definition();

    assert!(
        ts_definition.contains("sample_value: Partial<Record<MetricSlot, MetricSample>>;"),
        "Got: {ts_definition}"
    );
    assert!(
        ts_definition.contains("sample_array: Partial<Record<MetricSlot, Array<MetricSample>>>;"),
        "Got: {ts_definition}"
    );

    let zod_schema = EnumKeyedSiblingValueMaps::zod_schema();
    assert!(
        zod_schema.contains("sample_value: z.record(MetricSlot$Schema, MetricSample$Schema)"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema
            .contains("sample_array: z.record(MetricSlot$Schema, z.array(MetricSample$Schema))"),
        "Got: {zod_schema}"
    );
}

#[test]
fn test_string_keyed_sibling_value_maps_constructible() {
    let maps = StringKeyedSiblingValueMaps {
        optional_sample: HashMap::from([("m".to_owned(), None)]),
        sample_array: HashMap::from([(
            "d".to_owned(),
            vec![MetricSample {
                label: "d".to_owned(),
            }],
        )]),
        sample_value: HashMap::new(),
    };
    assert_eq!(maps.sample_array["d"].len(), 1);
    assert_eq!(maps.optional_sample["m"], None);
}

/// A `String` key never widens the value: the member is the sibling's own schema, arrayed when the
/// value is a `Vec` and nullable when it is an `Option` — the same schema the enum-key path writes
/// under each key, never the open object that admits every payload alike.
#[test]
#[cfg(feature = "jsonschema")]
fn test_string_keyed_sibling_value_maps_json_schema() {
    let sample_schema = MetricSample::json_schema();
    let schema = StringKeyedSiblingValueMaps::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    for (field_name, expected) in [
        ("sample_value", sample_schema.clone()),
        (
            "sample_array",
            serde_json::json!({ "type": "array", "items": sample_schema }),
        ),
        (
            "optional_sample",
            serde_json::json!({ "anyOf": [sample_schema, { "type": "null" }] }),
        ),
    ] {
        let field = &properties[field_name];
        assert_eq!(field["type"], "object", "in: {field}");
        assert_eq!(field["additionalProperties"], expected, "in: {field}");
    }
}

/// The member schema is held against what serde writes: an array of siblings for the `Vec` field,
/// a single sibling object for the plain one. An open member would have accepted either.
#[test]
#[cfg(feature = "jsonschema")]
fn test_string_keyed_sibling_members_match_the_serialized_form() {
    let sample = MetricSample {
        label: "d".to_owned(),
    };
    let maps = StringKeyedSiblingValueMaps {
        optional_sample: HashMap::from([("m".to_owned(), None)]),
        sample_array: HashMap::from([("d".to_owned(), vec![sample.clone()])]),
        sample_value: HashMap::from([("s".to_owned(), sample)]),
    };
    let payload = serde_json::to_value(&maps).unwrap();
    let schema = StringKeyedSiblingValueMaps::json_schema();

    let arrayed = &schema["properties"]["sample_array"]["additionalProperties"];
    assert_eq!(
        arrayed["type"],
        json_type_name(&payload["sample_array"]["d"]),
        "in: {arrayed}"
    );
    assert_eq!(
        arrayed["items"],
        MetricSample::json_schema(),
        "in: {arrayed}"
    );

    let single = &schema["properties"]["sample_value"]["additionalProperties"];
    assert_eq!(
        single["type"],
        json_type_name(&payload["sample_value"]["s"]),
        "in: {single}"
    );
    assert_eq!(*single, MetricSample::json_schema(), "in: {single}");

    let optional = &schema["properties"]["optional_sample"]["additionalProperties"];
    assert_eq!(
        optional["anyOf"][1]["type"],
        json_type_name(&payload["optional_sample"]["m"]),
        "in: {optional}"
    );
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_string_keyed_sibling_value_maps_typescript_generation() {
    let ts_definition = StringKeyedSiblingValueMaps::ts_definition();

    assert!(
        ts_definition.contains("sample_value: Partial<Record<string, MetricSample>>;"),
        "Got: {ts_definition}"
    );
    assert!(
        ts_definition.contains("sample_array: Partial<Record<string, Array<MetricSample>>>;"),
        "Got: {ts_definition}"
    );
    assert!(
        ts_definition.contains("optional_sample: Partial<Record<string, MetricSample | null>>;"),
        "Got: {ts_definition}"
    );

    let zod_schema = StringKeyedSiblingValueMaps::zod_schema();
    assert!(
        zod_schema.contains("sample_value: z.record(z.string(), MetricSample$Schema)"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema.contains("sample_array: z.record(z.string(), z.array(MetricSample$Schema))"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema
            .contains("optional_sample: z.record(z.string(), z.nullable(MetricSample$Schema))"),
        "Got: {zod_schema}"
    );
}

/// A brand key clears the derive under every feature set that reads one, none of them excepted:
/// the key is read off the field all three surfaces render from, so the same source cannot be a
/// schema under one toggle and a refusal under another.
#[test]
fn test_brand_keyed_maps_constructible() {
    let key = CorrelationId("abc".to_owned());
    let maps = BrandKeyedMaps {
        chained: HashMap::from([(TraceId(key.clone()), 2)]),
        counts: HashMap::from([(key.clone(), 1)]),
        nested: HashMap::from([(key.clone(), HashMap::from([(key.clone(), 3)]))]),
        samples: HashMap::from([(
            key.clone(),
            MetricSample {
                label: "s".to_owned(),
            },
        )]),
    };
    assert_eq!(maps.counts[&key], 1);
    assert_eq!(maps.nested[&key][&key], 3);

    let twin = StringKeyedBrandTwin {
        chained: HashMap::from([("abc".to_owned(), 2)]),
        counts: HashMap::from([("abc".to_owned(), 1)]),
        nested: HashMap::from([("abc".to_owned(), HashMap::from([("abc".to_owned(), 3)]))]),
        samples: HashMap::new(),
    };
    assert_eq!(twin.counts["abc"], maps.counts[&key]);
}

/// A brand key builds the same object a `String` key builds. The JSON schema is the structural
/// surface and has no brand to say, so it describes the open object outright — field for field the
/// `String`-keyed twin's.
#[test]
#[cfg(feature = "jsonschema")]
fn test_brand_keyed_maps_describe_as_their_string_keyed_twin() {
    assert_eq!(
        BrandKeyedMaps::json_schema()["properties"],
        StringKeyedBrandTwin::json_schema()["properties"]
    );
}

/// The nominal surfaces keep the brand's own spelling as the key type, the way the enum-key path
/// keeps the enum's — a `Record` and a `z.record` keyed by the brand, not by bare `string`.
#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_brand_keyed_maps_name_the_brand_as_the_key_type() {
    let ts_definition = BrandKeyedMaps::ts_definition();
    for expected in [
        "counts: Partial<Record<CorrelationId, number>>;",
        "chained: Partial<Record<TraceId, number>>;",
        "samples: Partial<Record<CorrelationId, MetricSample>>;",
        "nested: Partial<Record<CorrelationId, Partial<Record<CorrelationId, number>>>>;",
    ] {
        assert!(
            ts_definition.contains(expected),
            "{expected} missing: {ts_definition}"
        );
    }

    let zod_schema = BrandKeyedMaps::zod_schema();
    for expected in [
        "counts: z.record(CorrelationId$Schema, z.number().int())",
        "chained: z.record(TraceId$Schema, z.number().int())",
        "samples: z.record(CorrelationId$Schema, MetricSample$Schema)",
        "nested: z.record(CorrelationId$Schema, z.record(CorrelationId$Schema, z.number().int()))",
    ] {
        assert!(
            zod_schema.contains(expected),
            "{expected} missing: {zod_schema}"
        );
    }
}

/// The whole reason the brand keys a map: serde writes it as the bare string, so what the schema
/// describes as an open object is the object the value actually writes — and reads back.
#[test]
#[cfg(feature = "jsonschema")]
fn test_brand_keyed_maps_match_the_serialized_form() {
    let key = CorrelationId("abc".to_owned());
    let maps = BrandKeyedMaps {
        chained: HashMap::from([(TraceId(key.clone()), 2)]),
        counts: HashMap::from([(key.clone(), 1)]),
        nested: HashMap::from([(key.clone(), HashMap::from([(key.clone(), 3)]))]),
        samples: HashMap::from([(
            key,
            MetricSample {
                label: "s".to_owned(),
            },
        )]),
    };
    let payload = serde_json::to_value(&maps).unwrap();
    assert_eq!(payload["counts"], serde_json::json!({ "abc": 1_u64 }));
    assert_eq!(payload["chained"], serde_json::json!({ "abc": 2_u64 }));
    assert_eq!(
        payload["nested"],
        serde_json::json!({ "abc": { "abc": 3_u64 } })
    );

    let schema = BrandKeyedMaps::json_schema();
    for field_name in ["counts", "chained", "nested", "samples"] {
        let field = &schema["properties"][field_name];
        assert_eq!(field["type"], json_type_name(&payload[field_name]));
        assert!(field["additionalProperties"].is_object(), "in: {field}");
    }
    assert_eq!(
        schema["properties"]["samples"]["additionalProperties"],
        MetricSample::json_schema()
    );

    let read_back: BrandKeyedMaps = serde_json::from_value(payload).unwrap();
    assert_eq!(read_back, maps);
}

#[test]
fn test_string_keyed_alias_value_maps_constructible() {
    let maps = StringKeyedAliasValueMaps {
        sample_array: HashMap::new(),
        sample_value: HashMap::from([(
            "s".to_owned(),
            MetricSampleRef {
                label: "s".to_owned(),
            },
        )]),
    };
    assert_eq!(maps.sample_value["s"].label, "s");
}

/// An alias's schema module is named after its registered export name, so the member reference is
/// only resolvable through the registry — deriving it from the alias ident names a module that was
/// never emitted, and the expansion no longer compiles.
#[test]
#[cfg(feature = "jsonschema")]
fn test_string_keyed_alias_value_maps_resolve_the_registered_module() {
    let alias_schema = metric_sample_ref_schema::Schema::json_schema();
    let schema = StringKeyedAliasValueMaps::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    assert_eq!(
        properties["sample_value"]["additionalProperties"], alias_schema,
        "in: {schema}"
    );
    assert_eq!(
        properties["sample_array"]["additionalProperties"],
        serde_json::json!({ "type": "array", "items": alias_schema }),
        "in: {schema}"
    );
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_string_keyed_alias_value_maps_typescript_generation() {
    let ts_definition = StringKeyedAliasValueMaps::ts_definition();

    assert!(
        ts_definition.contains("sample_value: Partial<Record<string, MetricSampleRefType>>;"),
        "Got: {ts_definition}"
    );
    assert!(
        ts_definition
            .contains("sample_array: Partial<Record<string, Array<MetricSampleRefType>>>;"),
        "Got: {ts_definition}"
    );

    let zod_schema = StringKeyedAliasValueMaps::zod_schema();
    assert!(
        zod_schema.contains("sample_value: z.record(z.string(), MetricSampleRefType$Schema)"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema
            .contains("sample_array: z.record(z.string(), z.array(MetricSampleRefType$Schema))"),
        "Got: {zod_schema}"
    );
}

/// A map entry carries its `None` as JSON `null`: unlike an object key, an entry cannot be dropped,
/// so the schema has to admit the null serde writes. Both key paths render the same nullable form,
/// the one a tuple slot already uses for the same reason.
#[test]
#[cfg(feature = "jsonschema")]
fn test_optional_map_values_admit_the_null_serde_writes() {
    let values = OptionalMapValues {
        enum_keyed: HashMap::from([(MetricSlot::Daily, None)]),
        string_keyed: HashMap::from([("k".to_owned(), None)]),
    };
    let payload = serde_json::to_value(&values).unwrap();
    assert_eq!(json_type_name(&payload["enum_keyed"]["Daily"]), "null");
    assert_eq!(json_type_name(&payload["string_keyed"]["k"]), "null");

    let nullable_string = serde_json::json!({
        "anyOf": [{ "type": "string" }, { "type": "null" }]
    });
    let schema = OptionalMapValues::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    assert_enum_keyed_map_value(properties, "enum_keyed", &nullable_string);
    assert_eq!(properties["string_keyed"]["type"], "object");
    assert_eq!(
        properties["string_keyed"]["additionalProperties"], nullable_string,
        "in: {}",
        properties["string_keyed"]
    );
}

/// A map value is null-flavored rather than undefined-flavored, on both key paths: `Partial<Record>`
/// already lets a key be missing, but a key that *is* present carries the `null` serde writes for a
/// `None`, so the value type has to admit it.
#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_optional_map_values_are_null_flavored_on_both_key_paths() {
    let ts_definition = OptionalMapValues::ts_definition();
    assert!(
        ts_definition.contains("enum_keyed: Partial<Record<MetricSlot, string | null>>;"),
        "Got: {ts_definition}"
    );
    assert!(
        ts_definition.contains("string_keyed: Partial<Record<string, string | null>>;"),
        "Got: {ts_definition}"
    );

    let zod_schema = OptionalMapValues::zod_schema();
    assert!(
        zod_schema.contains("enum_keyed: z.record(MetricSlot$Schema, z.nullable(z.string()))"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema.contains("string_keyed: z.record(z.string(), z.nullable(z.string()))"),
        "Got: {zod_schema}"
    );
}

#[cfg(feature = "jsonschema")]
fn wrapped_map_fields_properties() -> serde_json::Map<String, serde_json::Value> {
    WrappedMapFields::json_schema()["properties"]
        .as_object()
        .unwrap()
        .clone()
}

fn wrapped_map_fields() -> WrappedMapFields {
    WrappedMapFields {
        bucket_counts: HashMap::from([(MetricBucket::High, 7_u64)]),
        labels: HashMap::from([("a".to_owned(), "one".to_owned())]),
        optional_labels: None,
        optional_wrapped_labels: Some(vec![HashMap::from([("a".to_owned(), "one".to_owned())])]),
        raw_keyed_counts: HashMap::from([(3_u32, 4_u64)]),
        wrapped_bucket_counts: vec![HashMap::from([(MetricBucket::Low, 2_u64)])],
        wrapped_labels: vec![HashMap::from([("b".to_owned(), "two".to_owned())])],
        wrapped_raw_keyed_counts: once(HashMap::from([(6_u32, 8_u64)])).collect(),
    }
}

#[test]
fn test_wrapped_map_fields_constructible() {
    let fields = wrapped_map_fields();
    assert_eq!(fields.wrapped_labels[0]["b"], "two");
    assert_eq!(fields.wrapped_bucket_counts[0][&MetricBucket::Low], 2);
    assert_eq!(fields.wrapped_raw_keyed_counts[0][&6], 8);
    assert_eq!(fields.optional_labels, None);
}

/// A field spelled with a sequence wrapper around a map writes a JSON array of the objects the map
/// writes, on every key path — so a schema that describes the bare object rejects the payload the
/// type serializes to.
#[test]
fn test_wrapped_map_fields_write_arrays_of_their_map() {
    let payload = serde_json::to_value(wrapped_map_fields()).unwrap();
    for field in [
        "optional_wrapped_labels",
        "wrapped_bucket_counts",
        "wrapped_labels",
        "wrapped_raw_keyed_counts",
    ] {
        let written = payload[field].as_array().unwrap();
        assert_eq!(written.len(), 1, "in: {}", payload[field]);
        assert!(written[0].is_object(), "in: {}", payload[field]);
    }
    for field in ["bucket_counts", "labels", "raw_keyed_counts"] {
        assert!(payload[field].is_object(), "in: {}", payload[field]);
    }
}

/// The array wrap is the field's, and what it wraps is the map's own rendering: each wrapped field
/// describes as `array` of exactly what its unwrapped twin describes as. Every key path is held
/// against its twin, so no path can lose the wrap or widen the map while applying it.
#[test]
#[cfg(feature = "jsonschema")]
fn test_wrapped_map_fields_describe_as_arrays_of_their_map() {
    let properties = wrapped_map_fields_properties();
    for (wrapped, unwrapped) in [
        ("optional_wrapped_labels", "labels"),
        ("wrapped_bucket_counts", "bucket_counts"),
        ("wrapped_labels", "labels"),
        ("wrapped_raw_keyed_counts", "raw_keyed_counts"),
    ] {
        assert_eq!(
            properties[wrapped],
            serde_json::json!({ "type": "array", "items": properties[unwrapped] }),
            "for {wrapped}, got: {}",
            properties[wrapped]
        );
    }
}

/// A map named without a sequence wrapper keeps the object it has always described as, on every key
/// path — including behind an `Option`, which field position spells by leaving the name out of
/// `required` rather than by admitting a `null`.
#[test]
#[cfg(feature = "jsonschema")]
fn test_unwrapped_map_fields_keep_the_object_they_describe_as() {
    let properties = wrapped_map_fields_properties();
    assert_eq!(
        properties["labels"],
        serde_json::json!({ "type": "object", "additionalProperties": { "type": "string" } })
    );
    assert_eq!(
        properties["raw_keyed_counts"],
        serde_json::json!({ "type": "object", "additionalProperties": true })
    );
    assert_eq!(properties["bucket_counts"], bucket_keyed_count_map());
    assert_eq!(properties["optional_labels"], properties["labels"]);

    let required = WrappedMapFields::json_schema()["required"]
        .as_array()
        .unwrap()
        .clone();
    for optional in ["optional_labels", "optional_wrapped_labels"] {
        assert!(
            !required.contains(&serde_json::Value::String(optional.to_owned())),
            "for {optional}, got: {required:?}"
        );
    }
}

/// TypeScript and Zod have always rendered the array a wrapped map writes, so the JSON schema's wrap
/// is pinned against theirs — the three surfaces describe the field one way or the divergence is
/// back.
#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_wrapped_map_fields_typescript_generation() {
    let ts_definition = WrappedMapFields::ts_definition();
    for expected in [
        "bucket_counts: Partial<Record<MetricBucket, number>>;",
        "labels: Partial<Record<string, string>>;",
        "optional_labels: Partial<Record<string, string>> | undefined;",
        "optional_wrapped_labels: Array<Partial<Record<string, string>>> | undefined;",
        "raw_keyed_counts: Partial<Record<number, number>>;",
        "wrapped_bucket_counts: Array<Partial<Record<MetricBucket, number>>>;",
        "wrapped_labels: Array<Partial<Record<string, string>>>;",
        "wrapped_raw_keyed_counts: Array<Partial<Record<number, number>>>;",
    ] {
        assert!(
            ts_definition.contains(expected),
            "missing {expected}, got: {ts_definition}"
        );
    }

    let zod_schema = WrappedMapFields::zod_schema();
    for expected in [
        "bucket_counts: z.record(MetricBucket$Schema, z.number().int()),",
        "labels: z.record(z.string(), z.string()),",
        "wrapped_bucket_counts: z.array(z.record(MetricBucket$Schema, z.number().int())),",
        "wrapped_labels: z.array(z.record(z.string(), z.string())),",
    ] {
        assert!(
            zod_schema.contains(expected),
            "missing {expected}, got: {zod_schema}"
        );
    }
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_hashmap_with_64bit_json_schema() {
    let schema = HashMapWith64Bit::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    // Check HashMap with u64 values
    assert_eq!(properties["u64_map"]["type"], "object");
    assert_eq!(
        properties["u64_map"]["additionalProperties"]["type"],
        "integer"
    );

    // Check HashMap with i64 values
    assert_eq!(properties["i64_map"]["type"], "object");
    assert_eq!(
        properties["i64_map"]["additionalProperties"]["type"],
        "integer"
    );

    // Check HashMap with Vec<u64> values
    assert_eq!(properties["mixed_map"]["type"], "object");
    assert_eq!(
        properties["mixed_map"]["additionalProperties"]["type"],
        "array"
    );
    assert_eq!(
        properties["mixed_map"]["additionalProperties"]["items"]["type"],
        "integer"
    );
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_hashmap_with_64bit_ts_definition() {
    let ts_definition = HashMapWith64Bit::ts_definition();

    // Check TypeScript HashMap types
    assert!(ts_definition.contains("u64_map: Partial<Record<string, number>>;"));
    assert!(ts_definition.contains("i64_map: Partial<Record<string, number>>;"));
    assert!(ts_definition.contains("mixed_map: Partial<Record<string, Array<number>>>;"));

    // Check Zod schema - now in separate method
    let zod_schema = HashMapWith64Bit::zod_schema();
    assert!(zod_schema.contains("u64_map: z.record(z.string(), z.number().int())"));
    assert!(zod_schema.contains("i64_map: z.record(z.string(), z.number().int())"));
    assert!(zod_schema.contains("mixed_map: z.record(z.string(), z.array(z.number().int()))"));
}

#[test]
fn test_element_fields_constructible_under_both_spellings() {
    let tag = MetricTag {
        label: "a".to_owned(),
    };
    let sets = SetElementFields {
        aliased_tags: HashSet::from([tag.clone()]),
        big_ids: HashSet::from([9_u64]),
        constrained_labels: HashSet::from(["abc".to_owned()]),
        labels: HashSet::from(["t".to_owned()]),
        preprocessed_labels: HashSet::from(["t".to_owned()]),
        sibling_tags: HashSet::from([tag.clone()]),
        small_ids: HashSet::from([7_u32]),
    };
    assert!(sets.small_ids.contains(&7));
    assert!(sets.big_ids.contains(&9));

    let vecs = VecElementFields {
        aliased_tags: vec![tag.clone()],
        big_ids: vec![9],
        constrained_labels: vec!["abc".to_owned()],
        labels: vec!["t".to_owned()],
        preprocessed_labels: vec!["t".to_owned()],
        sibling_tags: vec![tag],
        small_ids: vec![7],
    };
    assert_eq!(vecs.small_ids, [7]);
    assert_eq!(vecs.big_ids, [9]);
}

/// A set writes a JSON array of its element, so the element decides what the array holds — the
/// hardcoded `string` items every set once carried described only the sets of strings.
#[test]
#[cfg(feature = "jsonschema")]
fn test_set_element_json_schema() {
    let sets = SetElementFields {
        aliased_tags: HashSet::new(),
        big_ids: HashSet::from([9_u64]),
        constrained_labels: HashSet::new(),
        labels: HashSet::new(),
        preprocessed_labels: HashSet::new(),
        sibling_tags: HashSet::new(),
        small_ids: HashSet::from([7_u32]),
    };
    let payload = serde_json::to_value(&sets).unwrap();
    let schema = SetElementFields::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    for numeric in ["big_ids", "small_ids"] {
        let field = &properties[numeric];
        assert_eq!(
            field["type"],
            json_type_name(&payload[numeric]),
            "in: {field}"
        );
        assert_eq!(
            field["items"],
            serde_json::json!({ "type": "integer" }),
            "in: {field}"
        );
    }

    assert_eq!(
        properties["labels"],
        serde_json::json!({ "type": "array", "items": { "type": "string" } })
    );

    // A constraint written on the field constrains what the array holds, there being nothing else
    // for it to reach — the element is where it lands, as it does on the `Vec` spelling.
    assert_eq!(
        properties["constrained_labels"],
        serde_json::json!({
            "type": "array",
            "items": { "type": "string", "minLength": 3_u32 }
        })
    );

    assert_eq!(
        properties["sibling_tags"]["items"],
        MetricTag::json_schema(),
        "in: {}",
        properties["sibling_tags"]
    );
}

/// The twin values the hasher pair is read off: the same members holding the same things, one built
/// with the hasher named and one with it implied.
fn hasher_named_fields() -> HasherNamedFields {
    HasherNamedFields {
        counts: once(("alpha".to_owned(), 1)).collect(),
        enum_keyed_counts: once((MetricSlot::Daily, 2)).collect(),
        nested_ids: once(("beta".to_owned(), once(3).collect())).collect(),
        small_ids: once(4).collect(),
        tags: once(MetricTag {
            label: "gamma".to_owned(),
        })
        .collect(),
    }
}

fn hasher_implied_fields() -> HasherImpliedFields {
    HasherImpliedFields {
        counts: once(("alpha".to_owned(), 1)).collect(),
        enum_keyed_counts: once((MetricSlot::Daily, 2)).collect(),
        nested_ids: once(("beta".to_owned(), once(3).collect())).collect(),
        small_ids: once(4).collect(),
        tags: once(MetricTag {
            label: "gamma".to_owned(),
        })
        .collect(),
    }
}

/// The whole reason a named hasher may be held against the implied one: serde writes the same bytes
/// either way, the hasher deciding only the order of a bucket the wire form never exposes. Read off
/// what serde actually produces, so the claim answers to the wire rather than to a name.
#[test]
fn test_a_named_hasher_writes_what_the_implied_hasher_writes() {
    assert_eq!(
        serde_json::to_value(hasher_named_fields()).unwrap(),
        serde_json::to_value(hasher_implied_fields()).unwrap()
    );
}

/// The reported failure: naming the hasher carried an argument more than the container arms claimed,
/// so the type fell through to the sibling rendering and each surface published a name nothing
/// emits — a `HashMap<…>` TypeScript type, the same string as a Zod schema, and a schema module the
/// expansion never writes. Writing the same bytes, the two spellings describe the same.
#[test]
#[cfg(feature = "jsonschema")]
fn test_hasher_named_fields_describe_as_the_implied_spelling() {
    let named = HasherNamedFields::json_schema();
    let implied = HasherImpliedFields::json_schema();

    assert_eq!(named["properties"], implied["properties"]);
    assert_eq!(named["required"], implied["required"]);
}

/// The same holding on the TypeScript surface, with the fixture's own name set aside: no container
/// name and no hasher name reached the output.
#[test]
#[cfg(feature = "typescript")]
fn test_hasher_named_fields_type_as_the_implied_spelling() {
    let named = HasherNamedFields::ts_definition();
    for leaked in ["NamedHasher", "HashMap<", "HashSet<"] {
        assert!(!named.contains(leaked), "{leaked} reached: {named}");
    }
    assert_eq!(
        named.replace("HasherNamedFields", "HasherImpliedFields"),
        HasherImpliedFields::ts_definition()
    );
}

/// And on the Zod surface, where the sibling rendering published a bare type name that is not a
/// schema expression at all.
#[test]
#[cfg(feature = "zod")]
fn test_hasher_named_fields_validate_as_the_implied_spelling() {
    let named = HasherNamedFields::zod_schema();
    for leaked in ["NamedHasher", "HashMap<", "HashSet<"] {
        assert!(!named.contains(leaked), "{leaked} reached: {named}");
    }
    assert_eq!(
        named.replace("HasherNamedFields", "HasherImpliedFields"),
        HasherImpliedFields::zod_schema()
    );
}

/// A `HashSet<T>` and a `Vec<T>` serialize alike, so they describe alike — element by element, at
/// every element type. An alias element is the case the naming cannot be guessed for: its schema
/// module is the registered one, which is why the twin below compiles at all.
#[test]
#[cfg(feature = "jsonschema")]
fn test_set_elements_render_as_vec_elements() {
    let set_schema = SetElementFields::json_schema();
    let vec_schema = VecElementFields::json_schema();

    assert_eq!(set_schema["properties"], vec_schema["properties"]);
    assert_eq!(set_schema["required"], vec_schema["required"]);
}

/// A set writes a JSON array of its element, so the TypeScript type is the array of whatever the
/// element renders as — the Rust container name is not a TypeScript type at all. The aliased
/// element is the case the naming cannot be guessed for: it resolves through the registry.
#[test]
#[cfg(feature = "typescript")]
fn test_set_element_typescript_generation() {
    let ts_definition = SetElementFields::ts_definition();
    for spelling in [
        "aliased_tags: Array<MetricTagRefType>;",
        "big_ids: Array<number>;",
        "constrained_labels: Array<string>;",
        "labels: Array<string>;",
        "preprocessed_labels: Array<string>;",
        "sibling_tags: Array<MetricTag>;",
        "small_ids: Array<number>;",
    ] {
        assert!(ts_definition.contains(spelling), "Got: {ts_definition}");
    }
}

/// The Zod schema of a set is the array schema of its element, constraints and all — the container
/// name is not a Zod expression, and an element schema is what `z.array` has to be handed.
#[test]
#[cfg(feature = "zod")]
fn test_set_element_zod_generation() {
    let zod_schema = SetElementFields::zod_schema();
    for spelling in [
        "aliased_tags: z.array(MetricTagRefType$Schema),",
        "big_ids: z.array(z.number().int()),",
        "constrained_labels: z.array(z.string().min(3)),",
        "labels: z.array(z.string()),",
        // The preprocess wrap belongs once, outside the array — where the `Vec` spelling puts it.
        "preprocessed_labels: z.preprocess(trim, z.array(z.string())),",
        "sibling_tags: z.array(MetricTag$Schema),",
        "small_ids: z.array(z.number().int()),",
    ] {
        assert!(zod_schema.contains(spelling), "Got: {zod_schema}");
    }
}

/// Whatever a set field renders as, it is not the Rust wrapper's name: neither surface has any
/// meaning for it, so the name surviving anywhere into the output is a syntax error in the output.
/// The other covered wrappers name themselves in their own fixture's type name, so the equivalence
/// with the `Vec` twin below is what says no wrapper name reached their output.
#[test]
#[cfg(any(feature = "typescript", feature = "zod"))]
fn test_no_sequence_wrapper_name_survives_into_generated_output() {
    let mut generated = String::new();
    #[cfg(feature = "typescript")]
    generated.push_str(&SetElementFields::ts_definition());
    #[cfg(feature = "zod")]
    generated.push_str(&SetElementFields::zod_schema());

    for wrapper in SEQUENCE_WRAPPERS {
        assert!(!generated.contains(wrapper), "Got: {generated}");
    }
}

/// A `HashSet<T>` and a `Vec<T>` serialize alike, so their Zod schemas validate alike — field for
/// field, at every element type, once the type's own name is set aside.
#[test]
#[cfg(feature = "zod")]
fn test_set_fields_validate_as_vec_fields() {
    assert_eq!(
        SetElementFields::zod_schema().replace("SetElementFields", "VecElementFields"),
        VecElementFields::zod_schema()
    );
}

/// A wrapper is covered when serde writes it as a JSON array of its element — the whole criterion,
/// and the reason the twins below may be held against the `Vec` spelling at all. Read off what
/// serde actually produces, so the covered list answers to the wire rather than to a name.
#[test]
fn test_every_covered_wrapper_writes_a_json_array() {
    for (wrapper, payload) in covered_wrapper_payloads() {
        for (field, value) in payload.as_object().unwrap() {
            assert!(value.is_array(), "{wrapper}::{field} wrote: {value}");
        }
    }
}

/// Writing the same array as the `Vec` spelling, every covered wrapper describes as it does —
/// whole schema against whole schema, not one field at a time.
#[test]
#[cfg(feature = "jsonschema")]
fn test_every_covered_wrapper_describes_as_the_vec_spelling() {
    let vec_schema = VecElementFields::json_schema();
    for (wrapper, schema) in [
        ("SetElementFields", SetElementFields::json_schema()),
        (
            "BTreeSetElementFields",
            BTreeSetElementFields::json_schema(),
        ),
        (
            "BinaryHeapElementFields",
            BinaryHeapElementFields::json_schema(),
        ),
        (
            "VecDequeElementFields",
            VecDequeElementFields::json_schema(),
        ),
    ] {
        assert_eq!(
            schema["properties"], vec_schema["properties"],
            "for: {wrapper}"
        );
        assert_eq!(schema["required"], vec_schema["required"], "for: {wrapper}");
    }
}

/// The same holding on the TypeScript surface: with the fixture's own name set aside, a covered
/// wrapper's field declarations are the `Vec` spelling's, so no wrapper name reached the output.
/// Declarations rather than whole definitions, because the surrounding `JSDoc` differs for a reason
/// of its own: a `Vec` field's doc comment is dropped where every other spelling keeps it.
#[test]
#[cfg(feature = "typescript")]
fn test_every_covered_wrapper_types_as_the_vec_spelling() {
    let vec_definition = ts_field_declarations(&VecElementFields::ts_definition());
    for (wrapper, definition) in [
        ("SetElementFields", SetElementFields::ts_definition()),
        (
            "BTreeSetElementFields",
            BTreeSetElementFields::ts_definition(),
        ),
        (
            "BinaryHeapElementFields",
            BinaryHeapElementFields::ts_definition(),
        ),
        (
            "VecDequeElementFields",
            VecDequeElementFields::ts_definition(),
        ),
    ] {
        assert_eq!(
            ts_field_declarations(&definition.replace(wrapper, "VecElementFields")),
            vec_definition,
            "for: {wrapper}"
        );
    }
}

/// And on the Zod surface, constraints and preprocess wraps included.
#[test]
#[cfg(feature = "zod")]
fn test_every_covered_wrapper_validates_as_the_vec_spelling() {
    let vec_schema = VecElementFields::zod_schema();
    for (wrapper, schema) in [
        ("SetElementFields", SetElementFields::zod_schema()),
        ("BTreeSetElementFields", BTreeSetElementFields::zod_schema()),
        (
            "BinaryHeapElementFields",
            BinaryHeapElementFields::zod_schema(),
        ),
        ("VecDequeElementFields", VecDequeElementFields::zod_schema()),
    ] {
        assert_eq!(
            schema.replace(wrapper, "VecElementFields"),
            vec_schema,
            "for: {wrapper}"
        );
    }
}

/// A set in a slot writes the JSON array its element decides, exactly as the `Vec` spelling of the
/// same slot writes it — the whole reason a set slot may be held against that twin below. Read off
/// what serde produces: the two payloads are one value, arrays and all.
#[test]
fn test_a_set_slot_writes_what_the_vec_slot_writes() {
    let payload = serde_json::to_value(set_slot_values()).unwrap();
    assert_eq!(payload, serde_json::to_value(vec_slot_values()).unwrap());

    for field in [
        "aliased_tags",
        "enum_keyed_ids",
        "enum_keyed_tags",
        "labels",
        "optional_element_ids",
        "optional_ids",
        "sibling_tags",
        "small_ids",
    ] {
        for (key, member) in payload[field].as_object().unwrap() {
            assert!(member.is_array(), "{field}[{key}] wrote: {member}");
        }
    }
    for field in ["tuple_ids", "tuple_labels", "tuple_optional_ids"] {
        let slots = payload[field].as_array().unwrap();
        assert!(
            slots.last().unwrap().is_array(),
            "{field} wrote: {payload:?}"
        );
    }
}

/// A slot holding a value that is no sequence writes what that value writes — an object here — and
/// a wrapped one writes the array of those objects, which is what the schemas below describe.
#[test]
fn test_a_sibling_slot_writes_the_object_its_value_writes() {
    let payload = serde_json::to_value(sibling_slot_values()).unwrap();
    assert!(payload["bare"][1].is_object(), "Got: {payload}");
    for field in ["setted", "vecced"] {
        assert_eq!(
            payload[field][1],
            serde_json::json!([metric_tag()]),
            "{field} wrote: {payload}"
        );
    }
}

/// Writing the same array as the `Vec` spelling of the same slot, a set describes as one — on both
/// map key paths and in a tuple element, at every element type the twin carries.
#[test]
#[cfg(feature = "jsonschema")]
fn test_set_slots_describe_as_the_vec_slot_twin() {
    let set_schema = SetSlotValues::json_schema();
    let vec_schema = VecSlotValues::json_schema();
    assert_eq!(set_schema["properties"], vec_schema["properties"]);
    assert_eq!(set_schema["required"], vec_schema["required"]);
}

/// What that description is, spelled out: the array of the element, in the member schema a
/// `String`-keyed map carries and in the tuple element the same fixture opens.
#[test]
#[cfg(feature = "jsonschema")]
fn test_set_slots_describe_as_arrays_of_their_element() {
    let properties = SetSlotValues::json_schema()["properties"].clone();
    let integer_array = serde_json::json!({ "type": "array", "items": { "type": "integer" } });
    assert_eq!(
        properties["small_ids"]["additionalProperties"],
        integer_array
    );
    assert_eq!(properties["tuple_ids"]["prefixItems"][1], integer_array);
    assert_eq!(
        properties["labels"]["additionalProperties"],
        serde_json::json!({ "type": "array", "items": { "type": "string" } })
    );
    // A slot cannot be dropped, so a `None` there is `null` rather than an absent key.
    assert_eq!(
        properties["optional_ids"]["additionalProperties"],
        serde_json::json!({ "anyOf": [integer_array, { "type": "null" }] })
    );
    // A `None` the wrapper itself holds is a different `null`: the array is written either way, so
    // it lands among the items rather than in place of the slot. The `Vec` spelling writes exactly
    // that, and so describes as exactly this.
    let nullable_integer_array = serde_json::json!({
        "type": "array",
        "items": { "anyOf": [{ "type": "integer" }, { "type": "null" }] }
    });
    assert_eq!(
        properties["optional_element_ids"]["additionalProperties"],
        nullable_integer_array
    );
    assert_eq!(
        properties["tuple_optional_ids"]["prefixItems"][1],
        nullable_integer_array
    );
}

/// A sibling in a tuple element describes as the sibling does — its own schema, the one a field and
/// a map member reach for — rather than as the open object any value at all satisfies.
#[test]
#[cfg(feature = "jsonschema")]
fn test_a_sibling_tuple_slot_describes_as_the_sibling_it_holds() {
    let properties = SiblingSlotValues::json_schema()["properties"].clone();
    assert_eq!(
        properties["bare"]["prefixItems"][1],
        MetricTag::json_schema()
    );
    assert_ne!(
        MetricTag::json_schema(),
        serde_json::json!({ "type": "object" })
    );
}

/// And under a wrapper, the array of that schema — the same array the wrapper writes, so a slot
/// holding a sequence of siblings admits a sequence rather than any object at all.
#[test]
#[cfg(feature = "jsonschema")]
fn test_a_wrapped_sibling_tuple_slot_describes_as_the_array_of_that_sibling() {
    let properties = SiblingSlotValues::json_schema()["properties"].clone();
    let tag_array = serde_json::json!({ "type": "array", "items": MetricTag::json_schema() });
    for field in ["setted", "vecced"] {
        assert_eq!(
            properties[field]["prefixItems"][1], tag_array,
            "for: {field}"
        );
    }
}

/// The TypeScript surface, which names the sibling in every one of those slots already: pinned so
/// the JSON schema above is held against a rendering that does not move under it.
#[test]
#[cfg(feature = "typescript")]
fn test_sibling_tuple_slots_type_as_the_sibling_they_hold() {
    let ts_definition = SiblingSlotValues::ts_definition();
    for spelling in [
        "bare: [string, MetricTag];",
        "setted: [string, Array<MetricTag>];",
        "vecced: [string, Array<MetricTag>];",
    ] {
        assert!(ts_definition.contains(spelling), "Got: {ts_definition}");
    }
}

/// And the Zod surface, which validates each of them against the sibling's schema already.
#[test]
#[cfg(feature = "zod")]
fn test_sibling_tuple_slots_validate_against_the_sibling_they_hold() {
    let zod_schema = SiblingSlotValues::zod_schema();
    for spelling in [
        "bare: z.tuple([z.string(), MetricTag$Schema]),",
        "setted: z.tuple([z.string(), z.array(MetricTag$Schema)]),",
        "vecced: z.tuple([z.string(), z.array(MetricTag$Schema)]),",
    ] {
        assert!(zod_schema.contains(spelling), "Got: {zod_schema}");
    }
}

/// The TypeScript surface, which types a set slot as the array of its element already: pinned here
/// so the JSON schema above is held against a rendering that does not move under it.
#[test]
#[cfg(feature = "typescript")]
fn test_set_slots_type_as_the_vec_slot_twin() {
    let ts_definition = SetSlotValues::ts_definition();
    for spelling in [
        "aliased_tags: Partial<Record<string, Array<MetricTagRefType>>>;",
        "enum_keyed_ids: Partial<Record<MetricSlot, Array<number>>>;",
        "optional_element_ids: Partial<Record<string, Array<number | null>>>;",
        "optional_ids: Partial<Record<string, Array<number> | null>>;",
        "sibling_tags: Partial<Record<string, Array<MetricTag>>>;",
        "tuple_ids: [string, Array<number>];",
        "tuple_labels: [number, Array<string>];",
        "tuple_optional_ids: [string, Array<number | null>];",
    ] {
        assert!(ts_definition.contains(spelling), "Got: {ts_definition}");
    }
    assert_eq!(
        ts_field_declarations(&ts_definition.replace("SetSlotValues", "VecSlotValues")),
        ts_field_declarations(&VecSlotValues::ts_definition())
    );
}

/// And the Zod surface, which validates a set slot as that array already.
#[test]
#[cfg(feature = "zod")]
fn test_set_slots_validate_as_the_vec_slot_twin() {
    let zod_schema = SetSlotValues::zod_schema();
    for spelling in [
        "aliased_tags: z.record(z.string(), z.array(MetricTagRefType$Schema)),",
        "enum_keyed_tags: z.record(MetricSlot$Schema, z.array(MetricTag$Schema)),",
        "optional_element_ids: z.record(z.string(), z.array(z.nullable(z.number().int()))),",
        "optional_ids: z.record(z.string(), z.nullable(z.array(z.number().int()))),",
        "tuple_ids: z.tuple([z.string(), z.array(z.number().int())]),",
        "tuple_labels: z.tuple([z.number().int(), z.array(z.string())]),",
        "tuple_optional_ids: z.tuple([z.string(), z.array(z.nullable(z.number().int()))]),",
    ] {
        assert!(zod_schema.contains(spelling), "Got: {zod_schema}");
    }
    assert_eq!(
        zod_schema.replace("SetSlotValues", "VecSlotValues"),
        VecSlotValues::zod_schema()
    );
}

/// The wire the nested fixtures write, read for its nesting: a sequence of sequences is an array
/// whose members are arrays, which is what every description below is held against.
#[test]
fn test_a_nested_sequence_writes_an_array_of_arrays() {
    let payload = serde_json::to_value(nested_sequence_fields()).unwrap();
    for field in [
        "aliased_rows",
        "constrained_rows",
        "deep_ids",
        "fixed_grid",
        "labels",
        "optional_rows",
        "set_of_rows",
        "sibling_rows",
        "small_ids",
        "vec_of_sets",
    ] {
        assert!(payload[field][0].is_array(), "{field} wrote: {payload}");
    }
    assert!(payload["deep_ids"][0][0].is_array(), "Got: {payload}");

    let slots = serde_json::to_value(nested_sequence_slots()).unwrap();
    for field in ["enum_keyed_rows", "optional_rows", "rows", "set_rows"] {
        for (key, member) in slots[field].as_object().unwrap() {
            assert!(member[0].is_array(), "{field}[{key}] wrote: {member}");
        }
    }
    assert!(slots["tuple_rows"][1][0].is_array(), "Got: {slots}");
}

/// Each of those levels is a level of the JSON schema: the array wrap goes on once per level the
/// field is written at, whichever wrapper spells each one.
#[test]
#[cfg(feature = "jsonschema")]
fn test_nested_sequences_describe_at_the_depth_they_are_written() {
    let properties = NestedSequenceFields::json_schema()["properties"].clone();
    let integer_rows = serde_json::json!({
        "type": "array",
        "items": { "type": "array", "items": { "type": "integer" } }
    });

    for field in ["optional_rows", "set_of_rows", "small_ids"] {
        assert_eq!(properties[field], integer_rows, "for: {field}");
    }
    assert_eq!(properties["vec_of_sets"], integer_rows);
    // Every level of a fixed-size array is the same array of arrays, bounded at each level by the
    // length it was written with.
    assert_eq!(
        properties["fixed_grid"],
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
        properties["deep_ids"],
        serde_json::json!({ "type": "array", "items": integer_rows })
    );
    assert_eq!(
        properties["labels"],
        serde_json::json!({
            "type": "array",
            "items": { "type": "array", "items": { "type": "string" } }
        })
    );
    // A constraint has only the innermost element to land on: the levels above it hold arrays.
    assert_eq!(
        properties["constrained_rows"],
        serde_json::json!({
            "type": "array",
            "items": {
                "type": "array",
                "items": { "type": "string", "minLength": 3_u32 }
            }
        })
    );
    for field in ["aliased_rows", "sibling_rows"] {
        assert_eq!(
            properties[field],
            serde_json::json!({
                "type": "array",
                "items": { "type": "array", "items": MetricTag::json_schema() }
            }),
            "for: {field}"
        );
    }
}

/// A slot carries the nesting its value was written at too — the wrapper chain normalizes onto the
/// element before the member and element wraps go on, so no level is dropped on the way.
#[test]
#[cfg(feature = "jsonschema")]
fn test_nested_sequence_slots_describe_at_the_depth_they_are_written() {
    let properties = NestedSequenceSlots::json_schema()["properties"].clone();
    let integer_rows = serde_json::json!({
        "type": "array",
        "items": { "type": "array", "items": { "type": "integer" } }
    });

    for field in ["rows", "set_rows"] {
        assert_eq!(
            properties[field]["additionalProperties"], integer_rows,
            "for: {field}"
        );
    }
    assert_eq!(
        properties["optional_rows"]["additionalProperties"],
        serde_json::json!({ "anyOf": [integer_rows, { "type": "null" }] })
    );
    assert_eq!(
        properties["enum_keyed_rows"]["properties"]["Daily"],
        integer_rows
    );
    assert_eq!(properties["tuple_rows"]["prefixItems"][1], integer_rows);
    assert_eq!(
        properties["sibling_rows"]["additionalProperties"],
        serde_json::json!({
            "type": "array",
            "items": { "type": "array", "items": MetricTag::json_schema() }
        })
    );
}

/// The TypeScript surface at the same depth: one `Array<…>` per level written.
#[test]
#[cfg(feature = "typescript")]
fn test_nested_sequences_type_at_the_depth_they_are_written() {
    let ts_definition = NestedSequenceFields::ts_definition();
    for spelling in [
        "aliased_rows: Array<Array<MetricTagRefType>>;",
        "constrained_rows: Array<Array<string>>;",
        "deep_ids: Array<Array<Array<number>>>;",
        "fixed_grid: Array<Array<number>>;",
        "labels: Array<Array<string>>;",
        "optional_rows: Array<Array<number>> | undefined;",
        "set_of_rows: Array<Array<number>>;",
        "sibling_rows: Array<Array<MetricTag>>;",
        "small_ids: Array<Array<number>>;",
        "vec_of_sets: Array<Array<number>>;",
    ] {
        assert!(ts_definition.contains(spelling), "Got: {ts_definition}");
    }

    let slot_definition = NestedSequenceSlots::ts_definition();
    for spelling in [
        "enum_keyed_rows: Partial<Record<MetricSlot, Array<Array<number>>>>;",
        "optional_rows: Partial<Record<string, Array<Array<number>> | null>>;",
        "rows: Partial<Record<string, Array<Array<number>>>>;",
        "set_rows: Partial<Record<string, Array<Array<number>>>>;",
        "sibling_rows: Partial<Record<string, Array<Array<MetricTag>>>>;",
        "tuple_rows: [string, Array<Array<number>>];",
    ] {
        assert!(slot_definition.contains(spelling), "Got: {slot_definition}");
    }
}

/// And the Zod surface: one `z.array(…)` per level, the preprocess and constraint wraps landing
/// where the single-level spelling puts them.
#[test]
#[cfg(feature = "zod")]
fn test_nested_sequences_validate_at_the_depth_they_are_written() {
    let zod_schema = NestedSequenceFields::zod_schema();
    for spelling in [
        "aliased_rows: z.array(z.array(MetricTagRefType$Schema)),",
        "constrained_rows: z.array(z.array(z.string().min(3))),",
        "deep_ids: z.array(z.array(z.array(z.number().int()))),",
        "fixed_grid: z.array(z.array(z.number().int()).length(2)).length(2),",
        "labels: z.array(z.array(z.string())),",
        "set_of_rows: z.array(z.array(z.number().int())),",
        "sibling_rows: z.array(z.array(MetricTag$Schema)),",
        "small_ids: z.array(z.array(z.number().int())),",
        "vec_of_sets: z.array(z.array(z.number().int())),",
    ] {
        assert!(zod_schema.contains(spelling), "Got: {zod_schema}");
    }

    let slot_schema = NestedSequenceSlots::zod_schema();
    for spelling in [
        "enum_keyed_rows: z.record(MetricSlot$Schema, z.array(z.array(z.number().int()))),",
        "optional_rows: z.record(z.string(), z.nullable(z.array(z.array(z.number().int())))),",
        "rows: z.record(z.string(), z.array(z.array(z.number().int()))),",
        "set_rows: z.record(z.string(), z.array(z.array(z.number().int()))),",
        "sibling_rows: z.record(z.string(), z.array(z.array(MetricTag$Schema))),",
        "tuple_rows: z.tuple([z.string(), z.array(z.array(z.number().int()))]),",
    ] {
        assert!(slot_schema.contains(spelling), "Got: {slot_schema}");
    }
}

/// An alias of a nested sequence publishes the nesting its target was written at: an alias names a
/// type, and the type it names is an array of arrays on every surface.
#[test]
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn test_an_alias_of_a_nested_sequence_publishes_its_depth() {
    let grid: MetricGrid = vec![vec![7_u32]];
    assert!(
        serde_json::to_value(&grid).unwrap()[0].is_array(),
        "Got: {grid:?}"
    );

    #[cfg(feature = "typescript")]
    assert!(
        metric_grid_schema::Schema::ts_definition().contains("Array<Array<number>>"),
        "Got: {}",
        metric_grid_schema::Schema::ts_definition()
    );
    #[cfg(feature = "zod")]
    assert!(
        metric_grid_schema::Schema::zod_schema().contains("z.array(z.array(z.number().int()))"),
        "Got: {}",
        metric_grid_schema::Schema::zod_schema()
    );
    #[cfg(feature = "jsonschema")]
    assert_eq!(
        metric_grid_schema::Schema::json_schema(),
        serde_json::json!({
            "type": "array",
            "items": { "type": "array", "items": { "type": "integer" } }
        })
    );
}

/// A field's doc comment reaches the generated TypeScript from every spelling: the wrappers the
/// parser collapses onto their element carry the docs across the collapse, so a `Vec` and an
/// `Option` describe with the comment the set spelling they share a wire form with already carried.
#[test]
#[cfg(feature = "typescript")]
fn test_a_doc_comment_reaches_ts_from_a_collapsed_wrapper_field() {
    let ts = DocumentedSequenceFields::ts_definition();
    for spelling in [
        " * Documented nested field.\n * \n**/\n  documented_nested: Array<number> | undefined;",
        " * Documented optional field.\n * \n**/\n  documented_optional: string | undefined;",
        " * Documented set field.\n * \n**/\n  documented_set: Array<string>;",
        " * Documented vec field.\n * \n**/\n  documented_vec: Array<string>;",
        " * undocumented_vec\n * \n**/\n  undocumented_vec: Array<string>;",
    ] {
        assert!(ts.contains(spelling), "Got: {ts}");
    }
}
