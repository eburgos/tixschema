use super::{FieldDef, FieldDefType};

/// The wrappers whose fields write a JSON array of their element, named here as they reach the
/// renderers — `Vec` excluded, the parser having collapsed it long before.
const SEQUENCE_WRAPPERS: [&str; 5] = [
    "BTreeSet",
    "BinaryHeap",
    "HashSet",
    "LinkedList",
    "VecDeque",
];

fn field(field_type: FieldDefType) -> FieldDef {
    FieldDef {
        array_num: None,
        docs: String::new(),
        field_type,
        array_depth: 0,
        is_optional: false,
        model_schema_prop_meta: None,
        name: "items".to_owned(),
    }
}

fn sequence_of(wrapper: &str, element: FieldDefType) -> FieldDef {
    field(FieldDefType::SiblingType(
        wrapper.to_owned(),
        vec![field(element)],
    ))
}

#[test]
fn test_sequence_wrappers_render_as_typescript_arrays() {
    for wrapper in SEQUENCE_WRAPPERS {
        assert_eq!(
            sequence_of(wrapper, FieldDefType::U32).typescript_typename(),
            "Array<number>",
            "for: {wrapper}"
        );
        assert_eq!(
            sequence_of(wrapper, FieldDefType::String).typescript_typename(),
            "Array<string>",
            "for: {wrapper}"
        );
        assert_eq!(
            sequence_of(
                wrapper,
                FieldDefType::SiblingType("MetricTag".to_owned(), vec![])
            )
            .typescript_typename(),
            "Array<MetricTag>",
            "for: {wrapper}"
        );
    }
}

#[test]
#[cfg(feature = "zod")]
fn test_sequence_wrappers_render_as_zod_arrays() {
    for wrapper in SEQUENCE_WRAPPERS {
        assert_eq!(
            sequence_of(wrapper, FieldDefType::U32).zod_type(),
            "z.array(z.number().int())",
            "for: {wrapper}"
        );
        assert_eq!(
            sequence_of(wrapper, FieldDefType::String).zod_type(),
            "z.array(z.string())",
            "for: {wrapper}"
        );
        assert_eq!(
            sequence_of(
                wrapper,
                FieldDefType::SiblingType("MetricTag".to_owned(), vec![])
            )
            .zod_type(),
            "z.array(MetricTag$Schema)",
            "for: {wrapper}"
        );
    }
}

/// A slot — a tuple element or a map entry — holds the same array a field does, and an `Option`
/// there is the null-flavored one the slot always uses, applied to that array rather than inside it.
#[test]
fn test_sequence_in_a_slot_renders_as_the_array_it_writes() {
    for wrapper in SEQUENCE_WRAPPERS {
        let sequence = sequence_of(wrapper, FieldDefType::String);
        assert_eq!(
            sequence.typescript_slot_typename(),
            "Array<string>",
            "for: {wrapper}"
        );
        #[cfg(feature = "zod")]
        assert_eq!(
            sequence.zod_slot_type(),
            "z.array(z.string())",
            "for: {wrapper}"
        );

        let mut optional = sequence;
        optional.is_optional = true;
        assert_eq!(
            optional.typescript_slot_typename(),
            "Array<string> | null",
            "for: {wrapper}"
        );
        #[cfg(feature = "zod")]
        assert_eq!(
            optional.zod_slot_type(),
            "z.nullable(z.array(z.string()))",
            "for: {wrapper}"
        );
    }
}

/// A sequence wrapper inside a `Vec` is two arrays on the wire, so it is two on the surfaces: the wrapper's own
/// array wrap is the element's, and the field it sits in keeps the wrap the parser gave it.
#[test]
fn test_sequence_within_an_array_field_nests_both_wraps() {
    for wrapper in SEQUENCE_WRAPPERS {
        let mut nested = sequence_of(wrapper, FieldDefType::U32);
        nested.array_depth = 1;
        assert_eq!(
            nested.typescript_typename(),
            "Array<Array<number>>",
            "for: {wrapper}"
        );
        #[cfg(feature = "zod")]
        assert_eq!(
            nested.zod_type(),
            "z.array(z.array(z.number().int()))",
            "for: {wrapper}"
        );
    }
}

/// An `Option` around a sequence wrapper is the field's own, not the array's: the wrapper answers for what the
/// array holds and nothing about whether the key is there.
#[test]
fn test_optional_sequence_field_keeps_the_field_level_optionality() {
    let mut optional = sequence_of("HashSet", FieldDefType::String);
    optional.is_optional = true;
    assert_eq!(optional.typescript_typename(), "Array<string> | undefined");
    #[cfg(feature = "zod")]
    assert_eq!(
        optional.zod_type(),
        "z.union([z.array(z.string()), z.undefined()]).prefault(undefined)"
    );
}

/// A `Vec` inside a sequence wrapper is the mirror of the case above: the element already carries a
/// level and the wrapper adds its own, so both survive rather than one standing in for the other.
#[test]
fn test_an_array_element_within_a_sequence_nests_both_wraps() {
    for wrapper in SEQUENCE_WRAPPERS {
        let mut element = field(FieldDefType::U32);
        element.array_depth = 1;
        let nested = field(FieldDefType::SiblingType(wrapper.to_owned(), vec![element]));
        assert_eq!(
            nested.typescript_typename(),
            "Array<Array<number>>",
            "for: {wrapper}"
        );
        #[cfg(feature = "zod")]
        assert_eq!(
            nested.zod_type(),
            "z.array(z.array(z.number().int()))",
            "for: {wrapper}"
        );
    }
}

/// The parser counts a level per wrapper written rather than setting a flag one wrapper can only
/// set again, so the depth a field was written at is the depth the surfaces are handed.
#[test]
fn test_the_parser_counts_one_array_level_per_wrapper_written() {
    for (spelling, array_depth) in [
        ("u32", 0_u8),
        ("Vec<u32>", 1),
        ("Vec<Vec<u32>>", 2),
        ("Vec<Vec<Vec<u32>>>", 3),
        ("[[u32; 2]; 2]", 2),
        ("Option<Vec<Vec<u32>>>", 2),
    ] {
        let ty: syn::Type = syn::parse_str(spelling).unwrap();
        assert_eq!(
            super::get_field_def("items", &ty, "").array_depth,
            array_depth,
            "for: {spelling}"
        );
    }
}

/// The covered list, stated once so it is a decision rather than a spelling that happened to work.
/// The names on the false side are not uncovered by omission: each is a type the parser answers for
/// in a way of its own.
#[test]
fn test_the_covered_transparent_wrappers_are_exactly_the_recorded_list() {
    for name in ["Arc", "Box", "Cow", "Rc"] {
        assert!(super::is_transparent_wrapper(name), "for: {name}");
    }
    for name in ["BTreeMap", "HashMap", "Option", "Vec"] {
        assert!(!super::is_transparent_wrapper(name), "for: {name}");
    }
}

/// A covered wrapper writes nothing of its own, so the field it wraps is the field the parser
/// produces — name and docs included, both of which belong to where the field was written.
#[test]
fn test_a_transparent_wrapper_parses_as_the_field_it_wraps() {
    for spelling in ["Arc<u32>", "Box<u32>", "Cow<'a, u32>", "Rc<u32>", "u32"] {
        let ty: syn::Type = syn::parse_str(spelling).unwrap();
        let parsed = super::get_field_def("items", &ty, " * items");
        assert!(
            matches!(parsed.field_type, FieldDefType::U32),
            "for: {spelling}"
        );
        assert_eq!(parsed.name, "items", "for: {spelling}");
        assert_eq!(parsed.docs, " * items", "for: {spelling}");
        assert_eq!(parsed.array_depth, 0, "for: {spelling}");
        assert!(!parsed.is_optional, "for: {spelling}");
    }
}

/// What a `None` costs is the `Option`'s answer at either side of a wrapper that is not on the
/// wire: the flag is lifted onto the field exactly as the bare spelling lifts it.
#[test]
fn test_a_transparent_wrapper_carries_the_optionality_of_what_it_wraps() {
    for spelling in [
        "Arc<Option<u32>>",
        "Box<Option<u32>>",
        "Cow<'a, Option<u32>>",
        "Option<Box<u32>>",
        "Option<u32>",
        "Rc<Option<u32>>",
    ] {
        let ty: syn::Type = syn::parse_str(spelling).unwrap();
        let parsed = super::get_field_def("items", &ty, "");
        assert!(
            matches!(parsed.field_type, FieldDefType::U32),
            "for: {spelling}"
        );
        assert!(parsed.is_optional, "for: {spelling}");
        assert_eq!(parsed.array_depth, 0, "for: {spelling}");
    }
}

/// The array levels a field was written at are counted the same through a wrapper, whichever side
/// of it the sequence was written on — a boxed slice included, being the sequence a `Box` holds.
#[test]
fn test_a_transparent_wrapper_keeps_the_array_levels_of_what_it_wraps() {
    for spelling in [
        "Arc<[u32]>",
        "Box<Vec<u32>>",
        "Box<[u32]>",
        "Cow<'a, [u32]>",
        "Rc<Vec<u32>>",
        "Vec<Box<u32>>",
        "Vec<u32>",
    ] {
        let ty: syn::Type = syn::parse_str(spelling).unwrap();
        let parsed = super::get_field_def("items", &ty, "");
        assert!(
            matches!(parsed.field_type, FieldDefType::U32),
            "for: {spelling}"
        );
        assert_eq!(parsed.array_depth, 1, "for: {spelling}");
    }
}

/// The borrowed form of a mapped owned type writes what the owned form writes, and a wrapper is how
/// a field reaches it at all.
#[test]
fn test_a_borrowed_string_parses_as_a_string() {
    for spelling in ["Arc<str>", "Box<str>", "Cow<'a, str>", "Rc<str>", "String"] {
        let ty: syn::Type = syn::parse_str(spelling).unwrap();
        let parsed = super::get_field_def("label", &ty, "");
        assert!(
            matches!(parsed.field_type, FieldDefType::String),
            "for: {spelling}"
        );
    }
}
