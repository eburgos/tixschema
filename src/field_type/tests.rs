use super::{FieldDef, FieldDefType};

#[cfg(feature = "zod")]
use crate::utils::{AliasKind, register_alias_info};

/// The wrappers whose fields write a JSON array of their element, named here as they reach the
/// renderers — `Vec` excluded, the parser having collapsed it long before.
const SEQUENCE_WRAPPERS: [&str; 4] = ["BTreeSet", "BinaryHeap", "HashSet", "VecDeque"];

fn field(field_type: FieldDefType) -> FieldDef {
    FieldDef {
        array_lengths: Vec::new(),
        docs: String::new(),
        field_type,
        array_depth: 0,
        model_schema_prop_meta: None,
        nullable_levels: Vec::new(),
        name: "items".to_owned(),
        absent_from_wire: false,
        omits_value: false,
        #[cfg(feature = "jsonschema")]
        type_span: proc_macro2::Span::call_site(),
    }
}

/// The field a sequence-wrapper spelling normalizes to, as every renderer asks for it — `None`
/// when the spelling is not a wrapper around a single element.
fn normalized_sequence(spelling: &str) -> Option<FieldDef> {
    let ty: syn::Type = syn::parse_str(spelling).ok()?;
    let parsed = super::get_field_def("items", &ty, "");
    let FieldDefType::SiblingType(_, generics) = &parsed.field_type else {
        return None;
    };
    Some(parsed.collection_element_field(generics.first()?))
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
        optional.nullable_levels = vec![optional.array_depth];
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
    optional.nullable_levels = vec![optional.array_depth];
    assert_eq!(optional.typescript_typename(), "Array<string> | undefined");
    #[cfg(feature = "zod")]
    assert_eq!(
        optional.zod_type(),
        "z.union([z.null().transform(() => undefined), z.array(z.string()), z.undefined()]).prefault(undefined)"
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

/// A fixed-size array is written at one level and bounds that level alone, so the count lands on
/// the level the `[T; N]` spells rather than on the field. A count the expansion cannot read (a
/// const generic, a `const` item, a computed expression) or a slice leaves the level unbounded.
#[test]
fn test_the_parser_records_a_fixed_array_length_at_the_level_it_bounds() {
    for (spelling, array_lengths) in [
        ("u32", vec![]),
        ("[u32; 3]", vec![(0_u8, 3_usize)]),
        ("[[u32; 2]; 3]", vec![(0, 2), (1, 3)]),
        ("Vec<[u32; 3]>", vec![(0, 3)]),
        ("[Vec<u32>; 3]", vec![(1, 3)]),
        ("Option<[u32; 3]>", vec![(0, 3)]),
        ("[Option<u32>; 3]", vec![(0, 3)]),
        ("Box<[u32; 3]>", vec![(0, 3)]),
        ("[u32; SLOT_COUNT]", vec![]),
        ("[u32; N]", vec![]),
        ("[u32; 2 + 1]", vec![]),
        ("[u32]", vec![]),
        ("Vec<Vec<u32>>", vec![]),
    ] {
        let ty: syn::Type = syn::parse_str(spelling).unwrap();
        assert_eq!(
            super::get_field_def("items", &ty, "").array_lengths,
            array_lengths,
            "for: {spelling}"
        );
    }
}

/// A sequence wrapper normalizes onto its element before any surface reads it, and a bound has to
/// survive that move from either side of the wrapper: the array written inside it keeps its own
/// level, and the array it is written inside moves up by the level the wrapper adds.
#[test]
fn test_a_fixed_array_keeps_its_bound_across_the_sequence_normalization() {
    for wrapper in SEQUENCE_WRAPPERS {
        let inner = normalized_sequence(&format!("{wrapper}<[u32; 3]>")).unwrap();
        let outer = normalized_sequence(&format!("[{wrapper}<u32>; 3]")).unwrap();

        assert_eq!(inner.array_depth, 2, "for: {wrapper}");
        assert_eq!(inner.array_lengths, vec![(0, 3)], "for: {wrapper}");
        assert_eq!(outer.array_depth, 2, "for: {wrapper}");
        assert_eq!(outer.array_lengths, vec![(1, 3)], "for: {wrapper}");
    }
}

/// The covered list, stated once so it is a decision rather than a spelling that happened to work.
/// The names on the false side are not uncovered by omission: each is a type the parser answers for
/// in a way of its own.
#[test]
fn test_the_covered_transparent_wrappers_are_exactly_the_recorded_list() {
    for name in [
        "Arc", "Box", "Cell", "Cow", "Mutex", "Rc", "RefCell", "RwLock",
    ] {
        assert!(super::is_transparent_wrapper(name), "for: {name}");
    }
    for name in ["BTreeMap", "HashMap", "Option", "Vec"] {
        assert!(!super::is_transparent_wrapper(name), "for: {name}");
    }
}

/// The covered sequence list, stated once so it is a decision rather than a spelling that happened
/// to work. `LinkedList` is on the false side deliberately: it writes the array `Vec` writes, so
/// the covered spelling already describes it and a field written as one is refused instead.
#[test]
fn test_the_covered_sequence_wrappers_are_exactly_the_recorded_list() {
    for name in ["BTreeSet", "BinaryHeap", "HashSet", "Vec", "VecDeque"] {
        assert!(super::is_sequence_wrapper(name), "for: {name}");
    }
    for name in ["LinkedList", "BTreeMap", "HashMap", "Option", "Box"] {
        assert!(!super::is_sequence_wrapper(name), "for: {name}");
    }
}

/// The ownership/borrow split the covered list is built from: only the first four implement
/// `Deref`, which is what a constraint's generated validator needs to reach through one.
#[test]
fn test_ownership_and_interior_mutability_partition_the_covered_list() {
    for name in ["Arc", "Box", "Cow", "Rc"] {
        assert!(super::is_ownership_wrapper(name), "for: {name}");
        assert!(!super::is_interior_mutability_wrapper(name), "for: {name}");
    }
    for name in ["Cell", "Mutex", "RefCell", "RwLock"] {
        assert!(!super::is_ownership_wrapper(name), "for: {name}");
        assert!(super::is_interior_mutability_wrapper(name), "for: {name}");
    }
}

/// A covered wrapper writes nothing of its own, so the field it wraps is the field the parser
/// produces — name and docs included, both of which belong to where the field was written.
#[test]
fn test_a_transparent_wrapper_parses_as_the_field_it_wraps() {
    for spelling in [
        "Arc<u32>",
        "Box<u32>",
        "Cell<u32>",
        "Cow<'a, u32>",
        "Mutex<u32>",
        "Rc<u32>",
        "RefCell<u32>",
        "RwLock<u32>",
        "u32",
    ] {
        let ty: syn::Type = syn::parse_str(spelling).unwrap();
        let parsed = super::get_field_def("items", &ty, " * items");
        assert!(
            matches!(parsed.field_type, FieldDefType::U32),
            "for: {spelling}"
        );
        assert_eq!(parsed.name, "items", "for: {spelling}");
        assert_eq!(parsed.docs, " * items", "for: {spelling}");
        assert_eq!(parsed.array_depth, 0, "for: {spelling}");
        assert!(!parsed.is_optional(), "for: {spelling}");
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
        assert!(parsed.is_optional(), "for: {spelling}");
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

/// The borrowed form of a mapped owned type writes what the owned form writes, and a wrapper or a
/// reference is how a field reaches it at all.
#[test]
fn test_a_borrowed_string_parses_as_a_string() {
    for spelling in [
        "Arc<str>",
        "Box<str>",
        "Cow<'a, str>",
        "Rc<str>",
        "String",
        "&str",
        "&'a str",
        "Arc<Path>",
        "Box<Path>",
        "Cow<'a, Path>",
        "Rc<Path>",
        "PathBuf",
        "&Path",
        "&'a Path",
    ] {
        let ty: syn::Type = syn::parse_str(spelling).unwrap();
        let parsed = super::get_field_def("label", &ty, "");
        assert!(
            matches!(parsed.field_type, FieldDefType::String),
            "for: {spelling}"
        );
    }
}

/// `OsString`/`OsStr` are the owned/borrowed pair held out of the string mapping: serde writes
/// them as an externally tagged enum naming the target platform, so no portable schema describes
/// them. The parse leaves them as siblings and reports them from wherever they were written.
#[test]
fn test_an_os_string_is_reported_wherever_it_was_written() {
    for spelling in [
        "OsString",
        "OsStr",
        "&OsStr",
        "Box<OsStr>",
        "Cow<'a, OsStr>",
        "Rc<OsStr>",
        "Arc<OsStr>",
        "Option<OsString>",
        "Vec<OsString>",
        "HashMap<String, OsString>",
        "(String, OsString)",
        "Wrapper<OsString>",
    ] {
        let ty: syn::Type = syn::parse_str(spelling).unwrap();
        let parsed = super::get_field_def("location", &ty, "");
        assert!(
            parsed
                .os_string_name()
                .is_some_and(|name| name == "OsString" || name == "OsStr"),
            "for: {spelling}"
        );
    }
}

/// The report is for those two names alone: a path, a string, and a user type that merely carries
/// one of them inside its own name all describe a wire form and stay unreported.
#[test]
fn test_a_schematizable_field_is_not_reported_as_an_os_string() {
    for spelling in [
        "String",
        "PathBuf",
        "Box<Path>",
        "Option<String>",
        "Vec<PathBuf>",
        "HashMap<String, String>",
        "OsStringHolder",
        "Wrapper<PathBuf>",
    ] {
        let ty: syn::Type = syn::parse_str(spelling).unwrap();
        let parsed = super::get_field_def("location", &ty, "");
        assert_eq!(parsed.os_string_name(), None, "for: {spelling}");
    }
}

/// The cell/lock/lazy-init types and the borrow guards serde writes nothing for: the parse leaves
/// each a sibling, and the report names it from wherever it was written.
#[test]
fn test_an_unsupported_std_wrapper_is_reported_wherever_it_was_written() {
    for (spelling, expected) in [
        ("OnceLock<u32>", "OnceLock"),
        ("OnceCell<u32>", "OnceCell"),
        ("LazyLock<u32>", "LazyLock"),
        ("LazyCell<u32>", "LazyCell"),
        ("Ref<'a, u32>", "Ref"),
        ("RefMut<'a, u32>", "RefMut"),
        ("MutexGuard<'a, u32>", "MutexGuard"),
        ("RwLockReadGuard<'a, u32>", "RwLockReadGuard"),
        ("RwLockWriteGuard<'a, u32>", "RwLockWriteGuard"),
        ("Option<OnceLock<u32>>", "OnceLock"),
        ("Vec<LazyCell<u32>>", "LazyCell"),
        ("HashMap<String, OnceCell<u32>>", "OnceCell"),
        ("(String, MutexGuard<'a, u32>)", "MutexGuard"),
        ("Box<Ref<'a, u32>>", "Ref"),
        ("Wrapper<RefMut<'a, u32>>", "RefMut"),
    ] {
        let ty: syn::Type = syn::parse_str(spelling).unwrap();
        let parsed = super::get_field_def("guarded", &ty, "");
        assert_eq!(
            parsed.unsupported_std_wrapper_name(),
            Some(expected),
            "for: {spelling}"
        );
    }
}

/// The report is for those nine names alone: the wrappers the crate reads straight through, and a
/// user type that merely carries one of the names inside its own, all describe a wire form.
#[test]
fn test_a_schematizable_field_is_not_reported_as_an_unsupported_std_wrapper() {
    for spelling in [
        "String",
        "Box<u32>",
        "Rc<u32>",
        "Arc<u32>",
        "Cow<'a, str>",
        "RefCell<u32>",
        "Cell<u32>",
        "Mutex<u32>",
        "RwLock<u32>",
        "OnceLockHolder",
        "RefHolder",
        "Wrapper<String>",
    ] {
        let ty: syn::Type = syn::parse_str(spelling).unwrap();
        let parsed = super::get_field_def("guarded", &ty, "");
        assert_eq!(
            parsed.unsupported_std_wrapper_name(),
            None,
            "for: {spelling}"
        );
    }
}

/// `LinkedList` is on the false side because it is refused by a list of its own: serde writes it,
/// so the message it earns names the covered spelling rather than a missing wire form.
#[test]
fn test_the_unsupported_std_wrapper_list_is_exactly_the_nine_names() {
    for name in [
        "OnceLock",
        "OnceCell",
        "LazyLock",
        "LazyCell",
        "Ref",
        "RefMut",
        "MutexGuard",
        "RwLockReadGuard",
        "RwLockWriteGuard",
    ] {
        assert!(super::is_unsupported_std_wrapper(name), "for: {name}");
        assert!(!super::is_transparent_wrapper(name), "for: {name}");
    }
    for name in [
        "Arc",
        "Box",
        "Cow",
        "Rc",
        "Cell",
        "Mutex",
        "RefCell",
        "RwLock",
        "Inner",
        "LinkedList",
    ] {
        assert!(!super::is_unsupported_std_wrapper(name), "for: {name}");
    }
}

/// An `Option` written inside a sequence wrapper and one written around it are two different
/// values on the wire — `[null]` against `null` — so the parse keeps them apart: each is recorded
/// at the level it was written at, the element's below the array and the field's own at the top.
#[test]
fn test_the_parser_records_the_level_each_option_was_written_at() {
    for (spelling, array_depth, nullable_levels) in [
        ("u32", 0_u8, &[] as &[u8]),
        ("Option<u32>", 0, &[0]),
        ("Vec<u32>", 1, &[]),
        ("Vec<Option<u32>>", 1, &[0]),
        ("Option<Vec<u32>>", 1, &[1]),
        ("[Option<u32>; 2]", 1, &[0]),
        ("Vec<Vec<Option<u32>>>", 2, &[0]),
        ("Vec<Option<Vec<u32>>>", 2, &[1]),
        ("Option<Vec<Vec<u32>>>", 2, &[2]),
        ("Option<Vec<Option<u32>>>", 1, &[0, 1]),
        ("Vec<Option<Vec<Option<u32>>>>", 2, &[0, 1]),
    ] {
        let ty: syn::Type = syn::parse_str(spelling).unwrap();
        let parsed = super::get_field_def("items", &ty, "");
        assert_eq!(parsed.array_depth, array_depth, "for: {spelling}");
        let mut recorded = parsed.nullable_levels.clone();
        recorded.sort_unstable();
        assert_eq!(recorded, nullable_levels, "for: {spelling}");
    }
}

/// A covered wrapper writes the array its element decides, so the element's own `Option` is
/// recorded at the level the wrapper puts it at — the same level the `Vec` spelling of the same
/// field records it at, whichever wrapper name was written.
#[test]
fn test_a_covered_wrapper_records_its_element_option_where_the_vec_spelling_does() {
    let vec_spelling: syn::Type = syn::parse_str("Vec<Option<u32>>").unwrap();
    let expected = super::get_field_def("items", &vec_spelling, "");
    for wrapper in SEQUENCE_WRAPPERS {
        let ty: syn::Type = syn::parse_str(&format!("{wrapper}<Option<u32>>")).unwrap();
        let parsed = super::get_field_def("items", &ty, "");
        assert_eq!(
            parsed.typescript_typename(),
            expected.typescript_typename(),
            "for: {wrapper}"
        );
        assert_eq!(
            parsed.typescript_slot_typename(),
            expected.typescript_slot_typename(),
            "for: {wrapper}"
        );
        assert_eq!(
            parsed.is_optional(),
            expected.is_optional(),
            "for: {wrapper}"
        );
        #[cfg(feature = "zod")]
        assert_eq!(parsed.zod_type(), expected.zod_type(), "for: {wrapper}");
    }
}

/// The tokens a `macro_rules!` `$t:ty` substitution reaches an expansion as: the type's own tokens
/// inside an invisible group, which keeps the substitution one unit whatever the expansion writes
/// around it. Built here rather than expanded, so the depth can be chosen.
fn substituted(spelling: &str, depth: usize) -> proc_macro2::TokenStream {
    let mut tokens: proc_macro2::TokenStream = spelling.parse().unwrap();
    for _ in 0..depth {
        tokens = proc_macro2::TokenTree::Group(proc_macro2::Group::new(
            proc_macro2::Delimiter::None,
            tokens,
        ))
        .into();
    }
    tokens
}

/// What `parsed` and `expected` have to agree on for the two spellings to describe one field: the
/// span is deliberately left out, being the one thing about a substitution that legitimately
/// differs.
fn assert_reads_alike(parsed: &FieldDef, expected: &FieldDef, at: &str) {
    assert_eq!(parsed.field_type, expected.field_type, "{at}");
    assert_eq!(parsed.array_depth, expected.array_depth, "{at}");
    assert_eq!(parsed.array_lengths, expected.array_lengths, "{at}");
    assert_eq!(parsed.nullable_levels, expected.nullable_levels, "{at}");
}

/// A substituted type is the type it names. The grouping describes nothing about the value, so it
/// reaches no classification: every spelling reads as its written twin, however many expansions
/// passed it on.
#[test]
fn test_a_substituted_type_reads_as_the_written_one() {
    for spelling in [
        "String",
        "u32",
        "Vec<String>",
        "Option<u32>",
        "HashMap<String, u32>",
        "(String, u32)",
        "[u8; 4]",
        "&str",
    ] {
        let written: syn::Type = syn::parse_str(spelling).unwrap();
        let expected = super::get_field_def("items", &written, "");
        for depth in 1_usize..=3 {
            let ty: syn::Type = syn::parse2(substituted(spelling, depth)).unwrap();
            let parsed = super::get_field_def("items", &ty, "");
            assert_reads_alike(
                &parsed,
                &expected,
                &format!("for {spelling} at depth {depth}"),
            );
        }
    }
}

/// A substitution written inside a shape the author spelled out is read through on the way down:
/// the shape is theirs, the grouping is not, and the field is the one they would have written by
/// hand.
#[test]
fn test_a_substitution_inside_a_written_shape_is_read_through() {
    let grouped = proc_macro2::Group::new(
        proc_macro2::Delimiter::None,
        "String".parse::<proc_macro2::TokenStream>().unwrap(),
    );
    for (written, tokens) in [
        ("Vec<String>", quote::quote! { Vec<#grouped> }),
        ("Option<String>", quote::quote! { Option<#grouped> }),
        (
            "HashMap<String, String>",
            quote::quote! { HashMap<String, #grouped> },
        ),
        ("(String, u32)", quote::quote! { (#grouped, u32) }),
        ("[String; 2]", quote::quote! { [#grouped; 2] }),
        ("&String", quote::quote! { &#grouped }),
    ] {
        let expected = super::get_field_def("items", &syn::parse_str(written).unwrap(), "");
        let ty: syn::Type = syn::parse2(tokens).unwrap();
        let parsed = super::get_field_def("items", &ty, "");
        assert_reads_alike(&parsed, &expected, &format!("for {written}"));
    }
}

/// [`FieldDef::reaches_a_type_declared_later`]'s zero-argument arm: a bare sibling reference to an
/// unregistered `#[model_schema]` item defers exactly as a generic forward reference already does;
/// a registered name stays the eager baseline.
#[cfg(feature = "zod")]
#[test]
fn test_reaches_a_type_declared_later_answers_for_a_zero_argument_sibling() {
    let unregistered = field(FieldDefType::SiblingType(
        "NotYetDeclaredSibling".to_owned(),
        Vec::new(),
    ));
    assert!(
        unregistered.reaches_a_type_declared_later(),
        "an unregistered zero-argument sibling is a forward reference"
    );

    register_alias_info(
        "AlreadyDeclaredSibling",
        "AlreadyDeclaredSibling",
        "already_declared_sibling_schema",
        AliasKind::NoEnumMembers,
    );
    let registered = field(FieldDefType::SiblingType(
        "AlreadyDeclaredSibling".to_owned(),
        Vec::new(),
    ));
    assert!(
        !registered.reaches_a_type_declared_later(),
        "a registered zero-argument sibling is not a forward reference"
    );
}

/// A sequence wrapper's own name is never itself a forward reference, registered or not — it
/// renders as the array its element describes rather than a binding of its own — while an
/// unregistered element inside it still is, through the `any()` clause beside the exclusion.
#[cfg(feature = "zod")]
#[test]
fn test_a_sequence_wrapper_name_is_never_itself_a_forward_reference() {
    let wrapper_around_a_primitive = sequence_of("HashSet", FieldDefType::String);
    assert!(!wrapper_around_a_primitive.reaches_a_type_declared_later());

    let wrapper_around_a_forward_element = field(FieldDefType::SiblingType(
        "HashSet".to_owned(),
        vec![field(FieldDefType::SiblingType(
            "StillUnregisteredSibling".to_owned(),
            Vec::new(),
        ))],
    ));
    assert!(wrapper_around_a_forward_element.reaches_a_type_declared_later());
}

#[test]
fn test_boolean_literal_typescript() {
    assert_eq!(
        field(FieldDefType::BooleanLiteral(true)).typescript_typename(),
        "true"
    );
    assert_eq!(
        field(FieldDefType::BooleanLiteral(false)).typescript_typename(),
        "false"
    );
}

/// A whole value renders without the trailing `.0` `f64`'s own `Display` carries; a fractional one
/// keeps its digits.
#[test]
fn test_number_literal_typescript_has_no_trailing_zero() {
    assert_eq!(
        field(FieldDefType::NumberLiteral(214.0)).typescript_typename(),
        "214"
    );
    assert_eq!(
        field(FieldDefType::NumberLiteral(-5.0)).typescript_typename(),
        "-5"
    );
    assert_eq!(
        field(FieldDefType::NumberLiteral(3.5)).typescript_typename(),
        "3.5"
    );
}

#[cfg(feature = "zod")]
#[test]
fn test_boolean_literal_zod() {
    assert_eq!(
        field(FieldDefType::BooleanLiteral(true)).zod_type(),
        "z.literal(true)"
    );
    assert_eq!(
        field(FieldDefType::BooleanLiteral(false)).zod_type(),
        "z.literal(false)"
    );
}

#[cfg(feature = "zod")]
#[test]
fn test_number_literal_zod_has_no_trailing_zero() {
    assert_eq!(
        field(FieldDefType::NumberLiteral(214.0)).zod_type(),
        "z.literal(214)"
    );
    assert_eq!(
        field(FieldDefType::NumberLiteral(3.5)).zod_type(),
        "z.literal(3.5)"
    );
}
