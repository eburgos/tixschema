use super::{
    FieldDefType, check_os_string_field, collect_discriminated_variants, field_label,
    get_field_def, render_discriminated_variants, validate_as_number_flag,
    validate_ts_optional_flag,
};

#[cfg(feature = "serde")]
use super::{
    ConstraintLeaf, MemberAccess, ModelSchemaPropMeta, build_field_validation,
    cfg_attr_guard_error, check_omitted_key_is_readable, check_optional_field_serialization,
    collect_untagged_members, constrained_shape, enum_cfg_attr_guard_errors,
    generate_field_validation, generate_numeric_validation_code, generate_string_validation_code,
    has_serde_default, helper_name_stem, internally_tagged_guard_errors, needs_injected_default,
    parse_serde_field_attributes, parse_serde_type_attributes, render_untagged_variant,
};

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use super::{
    AliasKind, PublishedShape, alias_map_key_guard_error, branded_guard_errors, check_map_key,
    ident_schema_module_name, record_value_shape, register_alias_info,
};

#[cfg(all(feature = "serde", feature = "zod"))]
use super::{WireLeaf, flatten_edge_guard_error, record_wire_leaves, record_zod_union_members};

#[cfg(feature = "typescript")]
use super::tuple_struct_ts_body;

#[cfg(feature = "zod")]
use super::tuple_struct_zod_body;

#[cfg(feature = "jsonschema")]
use super::tuple_struct_json_body;

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use super::{check_slot_wire_is_readable, tuple_struct_shape};

use super::{
    VariantKind, check_variant_slot_wire_is_readable, parse_serde_key_omission, variant_wire_kind,
};

use syn::spanned::Spanned as _;

/// The variants of [`rendered_discriminated_union`]'s enum, in the order they are declared.
const DECLARED_VARIANTS: [&str; 6] = ["Upload", "Generate", "Delete", "Rename", "Move", "Archive"];

/// The doc attributes carrying a ` ```rust example ` block that the const-parameter probes are
/// written under. Held apart from them so every probe writes the same block and only the
/// declaration beneath it varies.
#[cfg(feature = "zod")]
const EXAMPLE_DOC_BLOCK: &str = "/// An item carrying an example block.\n\
                                 ///\n\
                                 /// ```rust example\n\
                                 /// Probe::Held\n\
                                 /// ```\n";

/// Every pattern the `pattern` guards must decide, invalid ones first, then the valid shapes the
/// shipped tests write.
const PROBE_PATTERNS: [&str; 10] = [
    r"^ab\",
    "^ab(",
    "*ab",
    "^[a-",
    r"\p{NotAClass}",
    "^[a-z]+$",
    r"^\d{3}\.\d{3}$",
    r"^a\n[a-z]+$",
    "^/[a-z]+$",
    r"^\/[a-z]+$",
];

/// Patterns the `regex` crate parses that no JavaScript regex literal carries, one per family the
/// guard sorts them into, beside the words the refusal names each by. Both splice points reach the
/// Zod literal and the JSON Schema `pattern`, so both have to answer for them.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
const UNPORTABLE_PROBE_PATTERNS: [(&str, &str); 6] = [
    ("(?i)abc", "inline flag directive"),
    (r"^\p{L}+$", "Unicode class"),
    // The needle is doubled because the guard error is read back off rendered `compile_error!`
    // tokens, where the string literal's own escaping is still in place.
    (r"\Aabc", r"`\\A` anchor"),
    ("^[[:alpha:]]+$", "POSIX class"),
    (r"[\w&&\d]", "`&&` class intersection"),
    (r"\x{41}", "braced code point escape"),
];

/// Every slot spelling the refusal reads, beside whether it is refused. A slot dropped from one of
/// serde's directions and not the other is; the pair that drops both is the wire the description
/// already answers for, and everything else is a slot written in its place.
///
/// Ungated because the variant seam reads the same list in every build.
const SLOT_OMISSION_SPELLINGS: [(&str, bool); 6] = [
    ("skip_serializing", true),
    ("skip_serializing_if = \"Option::is_none\"", true),
    ("skip_deserializing", true),
    ("skip", false),
    ("skip_serializing, skip_deserializing", false),
    ("default", false),
];

/// The covered wrappers, under the names a dispatch reads them by.
#[cfg(any(feature = "jsonschema", feature = "serde"))]
const SEQUENCE_WRAPPERS: [&str; 6] = [
    "BTreeSet",
    "BinaryHeap",
    "HashSet",
    "LinkedList",
    "Vec",
    "VecDeque",
];

/// The module name a generated hook is written against, standing in for the one the enum's own
/// expansion registers.
#[cfg(feature = "serde")]
const UNTAGGED_MODULE: Option<&str> = Some("choice_schema");

#[test]
fn ts_optional_ok_on_option_field() {
    validate_ts_optional_flag(true, true).unwrap();
}

#[test]
fn ts_optional_ok_when_flag_unset() {
    validate_ts_optional_flag(true, false).unwrap();
    validate_ts_optional_flag(false, false).unwrap();
}

#[test]
fn ts_optional_err_on_non_option_field() {
    let err = validate_ts_optional_flag(false, true).unwrap_err();
    assert!(err.contains("ts_optional"));
    assert!(err.contains("Option<T>"));
}

#[cfg(feature = "chrono")]
#[test]
fn as_number_ok_on_datetime_field() {
    validate_as_number_flag(&FieldDefType::DateTime, true).unwrap();
}

#[test]
fn as_number_ok_when_flag_unset() {
    validate_as_number_flag(&FieldDefType::String, false).unwrap();
}

#[test]
fn as_number_err_on_non_datetime_field() {
    let err = validate_as_number_flag(&FieldDefType::String, true).unwrap_err();
    assert!(err.contains("as_number"));
    assert!(err.contains("DateTime<Tz>"));
}

/// Runs the guard over the sole field of `item`, deriving `is_optional` the way the generator
/// does rather than re-sniffing the type.
#[cfg(feature = "serde")]
fn guard_result(item: &syn::ItemStruct) -> Result<(), syn::Error> {
    let field = item.fields.iter().next().unwrap();
    let field_name = field
        .ident
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();
    let field_def = get_field_def(&field_name, &field.ty, "");
    check_optional_field_serialization(field, field_def.is_optional())
}

/// Runs the omitted-key readability guard over the sole field of `item`, with the container's own
/// `default` read off the item the way the generator reads it.
#[cfg(feature = "serde")]
fn omitted_key_guard_result(item: &syn::ItemStruct) -> Result<(), syn::Error> {
    let field = item.fields.iter().next().unwrap();
    let field_name = field
        .ident
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();
    let field_def = get_field_def(&field_name, &field.ty, "");
    check_omitted_key_is_readable(
        field,
        field_def.is_optional(),
        has_serde_default(&item.attrs),
    )
}

#[cfg(feature = "serde")]
#[test]
fn bare_option_field_is_rejected() {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct Report {
            note: Option<String>,
        }
    };
    let message = guard_result(&item).unwrap_err().to_string();
    assert!(message.contains("note"));
    assert!(message.contains("skip_serializing_if = \"Option::is_none\""));
}

/// Read off serde itself: a `Vec` behind a `skip_serializing_if` and nothing else serializes to
/// `{"id":"1"}` and then fails to deserialize that payload, reporting the field as missing. The
/// surfaces now admit the absent key, so the spelling that writes a payload it cannot read back is
/// refused rather than described.
#[cfg(feature = "serde")]
#[test]
fn a_non_option_omitted_key_with_no_default_is_rejected() {
    for spelling in [
        "skip_serializing_if = \"Vec::is_empty\"",
        "skip_serializing",
    ] {
        let item: syn::ItemStruct = syn::parse_str(&format!(
            "struct Report {{ #[serde({spelling})] roles: Vec<String> }}"
        ))
        .unwrap();
        let message = omitted_key_guard_result(&item).unwrap_err().to_string();
        assert!(message.contains("roles"), "{spelling}: {message}");
        assert!(message.contains("default"), "{spelling}: {message}");
    }
}

/// Every spelling serde can already read a missing key back from. Each is left alone, because the
/// guard's subject is a payload with no reader — not an omitted key as such.
#[cfg(feature = "serde")]
#[test]
fn an_omitted_key_serde_can_read_back_is_left_alone() {
    for item_source in [
        // `default` on the field writes the value the missing key does not carry.
        "struct Report { #[serde(default, skip_serializing_if = \"Vec::is_empty\")] roles: Vec<String> }",
        "struct Report { #[serde(default = \"mk\", skip_serializing_if = \"Vec::is_empty\")] roles: Vec<String> }",
        // A bare `skip` stops serde reading the field at all, so it supplies `Default` itself.
        "struct Report { #[serde(skip)] roles: Vec<String> }",
        // serde reads a missing `Option` field back as `None` with no `default` written.
        "struct Report { #[serde(skip_serializing_if = \"Option::is_none\")] note: Option<String> }",
        // A `default` on the container answers for every field under it.
        "#[serde(default)] struct Report { #[serde(skip_serializing_if = \"Vec::is_empty\")] roles: Vec<String> }",
        // No omission at all.
        "struct Report { roles: Vec<String> }",
    ] {
        let item: syn::ItemStruct = syn::parse_str(item_source).unwrap();
        let refusal = omitted_key_guard_result(&item)
            .err()
            .map(|err| err.to_string());
        assert_eq!(refusal, None, "for: {item_source}");
    }
}

/// A positional slot has no key to drop — it is written by its place in the tuple — so the guard
/// has no subject there whatever the attribute says.
#[cfg(feature = "serde")]
#[test]
fn a_positional_slot_is_not_subject_to_the_omitted_key_guard() {
    let item: syn::ItemStruct = syn::parse_str(
        "struct Report(#[serde(skip_serializing_if = \"Vec::is_empty\")] Vec<String>);",
    )
    .unwrap();
    omitted_key_guard_result(&item).unwrap();
}

/// The single-field `Report` written with `notes` at the given spelling.
#[cfg(feature = "serde")]
fn report_with(spelling: &str) -> syn::ItemStruct {
    syn::parse_str(&format!("struct Report {{ notes: {spelling} }}")).unwrap()
}

/// The guard's subject is the `None` that reaches the wire as a bare `null` under the key, which is
/// the one an `Option` around the whole field writes. A covered wrapper is such a field, so the
/// wrapper spellings are refused exactly where the `Vec` spelling is.
#[cfg(feature = "serde")]
#[test]
fn an_option_around_a_covered_wrapper_is_rejected_as_the_vec_spelling_is() {
    for spelling in SEQUENCE_WRAPPERS
        .iter()
        .map(|wrapper| format!("Option<{wrapper}<String>>"))
        .chain(["Option<Vec<String>>".to_owned()])
    {
        let message = guard_result(&report_with(&spelling))
            .unwrap_err()
            .to_string();
        assert!(message.contains("notes"), "{spelling}: {message}");
        assert!(
            message.contains("skip_serializing_if"),
            "{spelling}: {message}"
        );
    }
}

/// An `Option` the wrapper holds is a different `None`: the array around it is always written, so
/// the key is always present and the `null` lands among the items, which is a value the field's
/// schema describes rather than one it has no way to admit. The guard has no subject there and
/// says nothing — at every depth, and whichever spelling each level takes.
#[cfg(feature = "serde")]
#[test]
fn an_option_inside_a_covered_wrapper_leaves_the_guard_nothing_to_refuse() {
    for spelling in SEQUENCE_WRAPPERS
        .iter()
        .map(|wrapper| format!("{wrapper}<Option<String>>"))
        .chain(
            [
                "Vec<Option<String>>",
                "Vec<Vec<Option<String>>>",
                "HashSet<Vec<Option<String>>>",
                "Vec<HashSet<Option<String>>>",
                "BTreeSet<VecDeque<Option<String>>>",
                "Vec<Option<Vec<String>>>",
            ]
            .map(ToOwned::to_owned),
        )
    {
        let refusal = guard_result(&report_with(&spelling))
            .err()
            .map(|err| err.to_string());
        assert_eq!(refusal, None, "for: {spelling}");
    }
}

/// The optionality read through a wrapper is the element's own and nothing else: a plain element
/// leaves the field non-optional, so no wrapper name alone can trip the guard.
#[cfg(feature = "serde")]
#[test]
fn a_covered_wrapper_of_a_plain_element_satisfies_the_guard() {
    for wrapper in SEQUENCE_WRAPPERS {
        guard_result(&report_with(&format!("{wrapper}<String>"))).unwrap();
    }
}

#[cfg(feature = "serde")]
#[test]
fn skip_serializing_if_satisfies_the_guard() {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct Report {
            #[serde(skip_serializing_if = "Option::is_none")]
            note: Option<String>,
        }
    };
    guard_result(&item).unwrap();
}

#[cfg(feature = "serde")]
#[test]
fn any_skip_serializing_if_predicate_satisfies_the_guard() {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct Report {
            #[serde(skip_serializing_if = "crate::is_boring")]
            note: Option<String>,
        }
    };
    guard_result(&item).unwrap();
}

#[cfg(feature = "serde")]
#[test]
fn skip_satisfies_the_guard() {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct Report {
            #[serde(skip)]
            note: Option<String>,
        }
    };
    guard_result(&item).unwrap();
}

#[cfg(feature = "serde")]
#[test]
fn skip_serializing_satisfies_the_guard() {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct Report {
            #[serde(skip_serializing)]
            note: Option<String>,
        }
    };
    guard_result(&item).unwrap();
}

#[cfg(feature = "serde")]
#[test]
fn skip_deserializing_alone_does_not_satisfy_the_guard() {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct Report {
            #[serde(skip_deserializing)]
            note: Option<String>,
        }
    };
    let message = guard_result(&item).unwrap_err().to_string();
    assert!(message.contains("note"));
}

#[cfg(feature = "serde")]
#[test]
fn non_option_field_is_unaffected() {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct Report {
            name: String,
        }
    };
    guard_result(&item).unwrap();
}

#[cfg(feature = "serde")]
#[test]
fn positional_option_field_is_exempt() {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct Report(Option<String>);
    };
    guard_result(&item).unwrap();
}

/// Collects the untagged-path guard failures as rendered `compile_error!` token streams.
#[cfg(feature = "serde")]
fn untagged_guard_error_tokens(item: &mut syn::ItemEnum) -> Vec<proc_macro2::TokenStream> {
    collect_untagged_members(item, UNTAGGED_MODULE).5
}

/// Collects the untagged-path guard failures as rendered `compile_error!` token strings.
#[cfg(feature = "serde")]
fn untagged_guard_errors(mut item: syn::ItemEnum) -> Vec<String> {
    untagged_guard_error_tokens(&mut item)
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[cfg(feature = "serde")]
#[test]
fn untagged_named_variant_bare_option_is_rejected() {
    let errors = untagged_guard_errors(syn::parse_quote! {
        enum Choice {
            Report { note: Option<String> },
            Plain(i64),
        }
    });
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    assert!(errors[0].contains("note"), "got: {}", errors[0]);
    assert!(
        errors[0].contains("skip_serializing_if"),
        "got: {}",
        errors[0]
    );
}

#[cfg(feature = "serde")]
#[test]
fn untagged_named_variant_omitting_none_is_accepted() {
    let errors = untagged_guard_errors(syn::parse_quote! {
        enum Choice {
            Report {
                #[serde(skip_serializing_if = "Option::is_none")]
                note: Option<String>,
            },
        }
    });
    assert!(errors.is_empty(), "got: {errors:?}");
}

#[cfg(feature = "serde")]
#[test]
fn untagged_tuple_variant_option_is_exempt() {
    let errors = untagged_guard_errors(syn::parse_quote! {
        enum Choice {
            Maybe(Option<i64>),
        }
    });
    assert!(errors.is_empty(), "got: {errors:?}");
}

/// Every shape the untagged rendering has no member spelling for is refused the way every other
/// misuse is — as an error the enum reports for each offender, rather than a panic that stops the
/// expansion at the first one and demotes its sentence to a `help:` note.
#[cfg(feature = "serde")]
#[test]
fn untagged_unsupported_variant_shapes_are_all_reported() {
    let errors = untagged_guard_errors(syn::parse_quote! {
        enum Choice {
            Bare,
            Pair(String, String),
            Note { id: String },
            Empty(),
        }
    });
    assert_eq!(errors.len(), 3, "got: {errors:?}");
    for (error, needles) in errors.iter().zip([
        ["`Bare`", "a unit variant"],
        ["`Pair`", "a tuple variant with 2 fields"],
        ["`Empty`", "a unit variant"],
    ]) {
        assert!(error.contains("compile_error"), "got: {error}");
        for needle in needles {
            assert!(error.contains(needle), "got: {error}");
        }
        assert!(
            error.contains("supports newtype (`V(T)`) and struct"),
            "got: {error}"
        );
    }
}

/// The refusal points at the variant it is about, not at the attribute on the enum: an enum with
/// many variants otherwise sends its author to the wrong line.
#[cfg(feature = "serde")]
#[test]
fn untagged_unsupported_variant_refusal_points_at_the_variant() {
    use syn::spanned::Spanned as _;

    let mut item: syn::ItemEnum =
        syn::parse_str("enum Choice { Note { id: String }, Pair(String, String) }").unwrap();
    let errors = untagged_guard_error_tokens(&mut item);
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    assert_eq!(
        errors[0].span().source_text().as_deref(),
        Some("Pair(String, String)")
    );
}

/// The supported shapes are untouched: a newtype and a struct variant still render, and neither
/// earns a word from the shape guard.
#[cfg(feature = "serde")]
#[test]
fn untagged_supported_variant_shapes_are_left_alone() {
    let errors = untagged_guard_errors(syn::parse_quote! {
        enum Choice {
            Note { id: String },
            Plain(i64),
        }
    });
    assert!(errors.is_empty(), "got: {errors:?}");
}

/// A variant's member renders the map its written type earns exactly as a struct field does, so the
/// key the registry rules out is refused in this position too rather than naming keys nothing can
/// supply.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn untagged_member_reaching_a_map_key_with_no_members_is_refused() {
    register_alias_info(
        "Ledger",
        "Ledger",
        "ledger_schema",
        AliasKind::NoEnumMembers,
    );
    for member_type in [
        quote::quote! { HashMap<Ledger, u32> },
        quote::quote! { Vec<HashMap<Ledger, u32>> },
        quote::quote! { HashMap<String, HashMap<Ledger, u32>> },
    ] {
        let errors = untagged_guard_errors(syn::parse_quote! {
            enum Untagged {
                Counts { counts: #member_type },
            }
        });
        assert_eq!(errors.len(), 1, "for {member_type}, got: {errors:?}");
        assert!(
            errors[0].contains("compile_error"),
            "for {member_type}: {}",
            errors[0]
        );
        assert!(
            errors[0].contains("field `counts`"),
            "for {member_type}: {}",
            errors[0]
        );
        assert!(
            errors[0].contains("a map key must be a plain"),
            "for {member_type}: {}",
            errors[0]
        );
        assert!(
            errors[0].contains("Ledger"),
            "for {member_type}: {}",
            errors[0]
        );
    }
}

/// A member's `model_schema_prop` reaches the surfaces exactly as the same field written in a
/// tagged variant does — the constraint was previously refused by rustc as an attribute that does
/// not exist, the untagged walk never having read or stripped it.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn untagged_member_carries_its_constraint_to_the_surfaces() {
    let mut item: syn::ItemEnum = syn::parse_quote! {
        enum Choice {
            Named {
                #[model_schema_prop(minLength = 2, pattern = "^[a-z]+$")]
                name: String,
            },
        }
    };
    let (_, _, zod_parts, _, _, errors, _, _) =
        collect_untagged_members(&mut item, UNTAGGED_MODULE);
    assert!(errors.is_empty(), "got: {errors:?}");
    assert!(
        zod_parts[0].contains("z.string().min(2).check(z.regex(/^[a-z]+$/))"),
        "got: {}",
        zod_parts[0]
    );
}

/// The same member reaches the Rust side through the generation the tagged twin uses: the validator
/// and its deserializer, named for the variant, hung on the member by the injected attribute.
#[cfg(feature = "serde")]
#[test]
fn untagged_member_constraint_generates_the_validator_and_hangs_it_on_the_member() {
    let mut item: syn::ItemEnum = syn::parse_quote! {
        enum Choice {
            Named {
                #[model_schema_prop(minLength = 2)]
                name: String,
            },
        }
    };
    let (_, _, _, _, _, errors, validation_fns, _) =
        collect_untagged_members(&mut item, UNTAGGED_MODULE);
    assert!(errors.is_empty(), "got: {errors:?}");
    assert_eq!(validation_fns.len(), 1, "got: {validation_fns:?}");
    let rendered = validation_fns[0].to_string();
    assert!(
        rendered.contains("fn validate_named_name_value"),
        "got: {rendered}"
    );
    assert!(
        rendered.contains("fn deserialize_named_name"),
        "got: {rendered}"
    );

    let attrs = &item.variants[0].fields.iter().next().unwrap().attrs;
    let rendered_attrs = quote::quote!(#(#attrs)*).to_string();
    assert!(
        rendered_attrs.contains(r#"deserialize_with = "choice_schema::deserialize_named_name""#),
        "got: {rendered_attrs}"
    );
}

/// Without a schema module there is nothing for a `deserialize_with` to name, so the member is left
/// exactly as written — the same subset in which a struct field generates no validator either.
#[cfg(feature = "serde")]
#[test]
fn untagged_member_constraint_generates_nothing_without_a_schema_module() {
    let mut item: syn::ItemEnum = syn::parse_quote! {
        enum Choice {
            Named {
                #[model_schema_prop(minLength = 2)]
                name: String,
            },
        }
    };
    let (_, _, _, _, _, errors, validation_fns, _) = collect_untagged_members(&mut item, None);
    assert!(errors.is_empty(), "got: {errors:?}");
    assert!(validation_fns.is_empty(), "got: {validation_fns:?}");
    let attrs = &item.variants[0].fields.iter().next().unwrap().attrs;
    assert!(attrs.is_empty(), "got: {}", quote::quote!(#(#attrs)*));
}

/// A newtype member has no ident for the two helpers and the accessor to be named from, so the
/// bound is refused here for the reason it is refused on a tuple field — the position the generation
/// this path now shares has always answered for.
#[cfg(feature = "serde")]
#[test]
fn untagged_newtype_member_constraint_is_refused() {
    let errors = untagged_guard_errors(syn::parse_quote! {
        enum Choice {
            Slug(#[model_schema_prop(minLength = 2)] String),
        }
    });
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    assert!(errors[0].contains("tuple field"), "got: {}", errors[0]);
    assert!(
        errors[0].contains("unsupported on a positional field"),
        "got: {}",
        errors[0]
    );
}

/// The attribute is stripped off the member the way [`super::process_field`] strips it off a struct
/// field: it is this crate's own and inert to every derive, so a copy left on the emitted item is
/// one rustc reports as an attribute that does not exist.
#[cfg(feature = "serde")]
#[test]
fn untagged_member_prop_attribute_is_stripped_from_the_emitted_item() {
    let mut item: syn::ItemEnum = syn::parse_quote! {
        enum Choice {
            Named {
                #[model_schema_prop(minLength = 2)]
                #[serde(rename = "label")]
                name: String,
            },
        }
    };
    collect_untagged_members(&mut item, UNTAGGED_MODULE);
    let attrs = &item.variants[0].fields.iter().next().unwrap().attrs;
    assert!(
        !attrs
            .iter()
            .any(|attr| attr.path().is_ident("model_schema_prop")),
        "got: {}",
        quote::quote!(#(#attrs)*)
    );
    assert!(
        attrs.iter().any(|attr| attr.path().is_ident("serde")),
        "the serde attribute must survive the strip"
    );
}

/// The whole `model_schema_prop` guard chain reaches this position too, so a member's misspelled
/// key is named where it was written instead of emitting an unconstrained member.
#[cfg(feature = "serde")]
#[test]
fn untagged_member_prop_guards_apply() {
    for (member, needle) in [
        (
            quote::quote! { #[model_schema_prop(patern = "^[a-z]+$")] name: String },
            "patern",
        ),
        (
            quote::quote! { #[model_schema_prop(ts_optional)] name: String },
            "requires an Option<T> field",
        ),
        (
            quote::quote! { #[model_schema_prop(as = String)] name: u64 },
            "as = String",
        ),
    ] {
        let errors = untagged_guard_errors(syn::parse_quote! {
            enum Choice {
                Named { #member },
            }
        });
        assert_eq!(errors.len(), 1, "for {member}: {errors:?}");
        assert!(
            errors[0].contains(needle),
            "{needle} missing for {member}: {}",
            errors[0]
        );
    }
}

/// The guard turns away only what the registry proves has no members: a member keyed by a plain
/// enum, or by a `String`, keeps the variant it had.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn untagged_member_with_an_enumerable_map_key_is_left_alone() {
    register_alias_info("Bucket", "Bucket", "bucket_schema", AliasKind::EnumMembers);
    for member_type in [
        quote::quote! { HashMap<Bucket, u32> },
        quote::quote! { HashMap<String, u32> },
    ] {
        let errors = untagged_guard_errors(syn::parse_quote! {
            enum Untagged {
                Counts { counts: #member_type },
            }
        });
        assert!(errors.is_empty(), "for {member_type}, got: {errors:?}");
    }
}

/// The refusal every map-key spelling no surface can write earns, read off the untagged walk.
/// Field position refuses each of these; a member is the same map, so it is refused here too — and
/// the walk is the only thing that has to say so, the guard failure dropping the schema surface
/// before any member rendering reaches the author.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn untagged_member_reaching_an_unwritable_map_key_is_refused() {
    for (member_type, needle) in [
        (quote::quote! { HashMap<Vec<String>, u32> }, "String"),
        (quote::quote! { HashMap<[String; 2], u32> }, "String"),
        (quote::quote! { HashMap<Option<String>, u32> }, "String"),
        (
            quote::quote! { HashMap<(String, u32), u32> },
            "`(_, _)` as a JSON array",
        ),
        (
            quote::quote! { HashMap<HashMap<String, u32>, u32> },
            "`HashMap<_, _>` as a JSON object",
        ),
        (
            quote::quote! { Vec<HashMap<Option<String>, u32>> },
            "String",
        ),
    ] {
        let errors = untagged_guard_errors(syn::parse_quote! {
            enum Untagged {
                Counts { counts: #member_type },
            }
        });
        assert_eq!(errors.len(), 1, "for {member_type}, got: {errors:?}");
        assert!(
            errors[0].contains("compile_error"),
            "for {member_type}: {}",
            errors[0]
        );
        assert!(
            errors[0].contains("field `counts`"),
            "for {member_type}: {}",
            errors[0]
        );
        assert!(
            errors[0].contains(needle),
            "for {member_type}: {}",
            errors[0]
        );
    }
}

/// The JSON-schema values the untagged walk renders its members as.
#[cfg(all(feature = "serde", feature = "jsonschema"))]
fn untagged_member_values(mut item: syn::ItemEnum) -> Vec<String> {
    collect_untagged_members(&mut item, UNTAGGED_MODULE)
        .4
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// A member holding a map is the map its key classification earns, at the depth it is written —
/// the renderings field position produces from the same types, reached through the same dispatch.
#[cfg(all(feature = "serde", feature = "jsonschema"))]
#[test]
fn untagged_member_holding_a_map_renders_the_field_position_map() {
    register_alias_info("Bucket", "Bucket", "bucket_schema", AliasKind::EnumMembers);
    for map_type in [
        "HashMap<Bucket, u32>",
        "HashMap<String, u32>",
        "Vec<HashMap<String, u32>>",
        "HashMap<String, HashMap<Bucket, u32>>",
    ] {
        let member_type: syn::Type = syn::parse_str(map_type).unwrap();
        let values = untagged_member_values(syn::parse_quote! {
            enum Untagged {
                Counts { m: #member_type },
            }
        });
        assert!(
            values[0].contains(&inserted_field_value(map_type)),
            "for {map_type}, got: {}",
            values[0]
        );
    }
}

/// A tuple member is the fixed-arity array its own field position writes, arity bounds included:
/// without them a shorter array serde can neither write nor read back still validates.
#[cfg(all(feature = "serde", feature = "jsonschema"))]
#[test]
fn untagged_member_holding_a_tuple_renders_the_arity_bounds() {
    let values = untagged_member_values(syn::parse_quote! {
        enum Untagged {
            Pair { pair: (i64, String) },
        }
    });
    assert!(
        values[0].contains(r#""minItems" : 2usize , "maxItems" : 2usize"#),
        "got: {}",
        values[0]
    );
    assert!(
        values[0].contains(r#""items" : false"#),
        "got: {}",
        values[0]
    );
}

/// An opaque member keeps the permissive empty schema: no type name reaches it to narrow with,
/// which is the reason field position leaves it open too.
#[cfg(all(feature = "serde", feature = "jsonschema"))]
#[test]
fn untagged_member_holding_an_opaque_value_stays_permissive() {
    let values = untagged_member_values(syn::parse_quote! {
        enum Untagged {
            Raw { raw: serde_json::Value },
        }
    });
    assert!(
        values[0].contains("serde_json :: json ! ({ })"),
        "got: {}",
        values[0]
    );
}

/// A map value the member dispatch cannot render replaces the member's whole rendering with the
/// diagnostic, as it replaces the insertion in field position: a schema the expansion has already
/// rejected is not one to hand the author. No guard answers for this shape, so the rendering is
/// where it has to be said — once, naming the field and the reason, with no rendered map left
/// beside it.
#[cfg(all(feature = "serde", feature = "jsonschema"))]
#[test]
fn untagged_member_holding_an_unsupported_map_value_emits_only_the_compile_error() {
    let values = untagged_member_values(syn::parse_quote! {
        enum Untagged {
            Rows { rows: HashMap<String, (String, u32)> },
        }
    });
    assert_eq!(
        values[0].matches("compile_error !").count(),
        1,
        "got: {}",
        values[0]
    );
    assert!(values[0].contains("field `rows`"), "got: {}", values[0]);
    assert!(
        values[0].contains("a tuple is not supported as a map value"),
        "got: {}",
        values[0]
    );
    assert!(
        !values[0].contains("additionalProperties\" :"),
        "got: {}",
        values[0]
    );
}

/// Collects the internally tagged path's guard failures as rendered `compile_error!` token strings.
#[cfg(feature = "serde")]
fn internal_guard_errors(item: &syn::ItemEnum) -> Vec<String> {
    internally_tagged_guard_errors(item, "type")
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// The arms serde writes beside a bare tag: a struct variant's fields, a named type's own members,
/// and a unit variant's nothing at all.
#[cfg(feature = "serde")]
#[test]
fn internally_tagged_serializable_variants_are_accepted() {
    let errors = internal_guard_errors(&syn::parse_quote! {
        enum TagOnly {
            Bare,
            Fields { a: String },
            Wrapped(Payload),
            Boxed(Box<Payload>),
        }
    });
    assert!(errors.is_empty(), "got: {errors:?}");
}

/// Every scalar shape serde refuses to write beside the tag, named the way serde's own error names
/// it.
#[cfg(feature = "serde")]
#[test]
fn internally_tagged_newtype_over_a_scalar_is_rejected() {
    for (source, shape) in [
        ("enum E { V(String) }", "a string"),
        ("enum E { V(bool) }", "a boolean"),
        ("enum E { V(i64) }", "an integer"),
        ("enum E { V(f64) }", "a float"),
        ("enum E { V((u32, u32)) }", "a tuple"),
        ("enum E { V(Vec<Payload>) }", "a sequence"),
        ("enum E { V(Option<Payload>) }", "an optional"),
    ] {
        let errors = internal_guard_errors(&syn::parse_str(source).unwrap());
        assert_eq!(errors.len(), 1, "got: {errors:?} for {source}");
        assert!(errors[0].contains("compile_error"), "got: {}", errors[0]);
        assert!(errors[0].contains("variant `V`"), "got: {}", errors[0]);
        assert!(
            errors[0].contains(&format!("containing {shape}")),
            "expected serde's own wording for {source}. Got: {}",
            errors[0]
        );
    }
}

/// An `Option` around a sequence is refused as an optional: serde's serializer meets the wrappers
/// in that order, and reports the outermost one.
#[cfg(feature = "serde")]
#[test]
fn internally_tagged_newtype_names_the_outermost_wrapper() {
    let errors = internal_guard_errors(&syn::parse_quote! {
        enum E { V(Option<Vec<Payload>>) }
    });
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    assert!(
        errors[0].contains("containing an optional"),
        "got: {}",
        errors[0]
    );
}

/// A map's members are written beside the tag, but the expansion cannot name them, so no schema
/// closed around the tag admits them. serde's restriction is not what is quoted here — serde writes
/// this one.
#[cfg(feature = "serde")]
#[test]
fn internally_tagged_newtype_over_a_map_is_rejected_as_unnameable() {
    let errors = internal_guard_errors(&syn::parse_quote! {
        enum E { V(std::collections::HashMap<String, u32>) }
    });
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    assert!(errors[0].contains("wraps a map"), "got: {}", errors[0]);
    assert!(
        !errors[0].contains("serde refuses"),
        "serde writes a map beside the tag. Got: {}",
        errors[0]
    );
}

/// A multi-element tuple variant is a declaration serde's own derive refuses; the guard names that
/// rather than describing elements that have no key to sit under.
#[cfg(feature = "serde")]
#[test]
fn internally_tagged_tuple_variant_is_rejected() {
    let errors = internal_guard_errors(&syn::parse_quote! {
        enum E { V(String, u32) }
    });
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    assert!(
        errors[0].contains("cannot be used with tuple variants"),
        "got: {}",
        errors[0]
    );
}

/// An empty tuple variant carries nothing: serde writes the tag alone, which is the unit arm.
#[cfg(feature = "serde")]
#[test]
fn internally_tagged_empty_tuple_variant_is_accepted() {
    let errors = internal_guard_errors(&syn::parse_quote! {
        enum E { V() }
    });
    assert!(errors.is_empty(), "got: {errors:?}");
}

/// A name is not the criterion — what serde writes for it is. A plain enum writes its own variant
/// name, which joins no object, and the registry is where the expansion learns that.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn internally_tagged_newtype_over_a_registered_plain_enum_is_rejected() {
    register_alias_info("Hue", "Hue", "hue_schema", AliasKind::EnumMembers);
    let errors = internal_guard_errors(&syn::parse_quote! {
        enum E { V(Hue) }
    });
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    assert!(errors[0].contains("variant `V`"), "got: {}", errors[0]);
    assert!(errors[0].contains("`Hue`"), "got: {}", errors[0]);
    assert!(
        errors[0].contains("does not write as an object"),
        "got: {}",
        errors[0]
    );
    assert!(
        errors[0].contains("Name a `content` key"),
        "got: {}",
        errors[0]
    );
}

/// The two answers that leave the declaration alone: a type the registry rules out, and one it has
/// never seen. Neither is a plain enum as far as this expansion can tell, and an `Unknown` is not a
/// negative — it reaches the merge, which reads the schema instead of the name.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn internally_tagged_newtype_over_a_non_enum_or_unknown_name_is_accepted() {
    register_alias_info(
        "Payload",
        "Payload",
        "payload_schema",
        AliasKind::NoEnumMembers,
    );
    for source in ["enum E { V(Payload) }", "enum E { V(NeverRegistered) }"] {
        let errors = internal_guard_errors(&syn::parse_str(source).unwrap());
        assert!(errors.is_empty(), "got: {errors:?} for {source}");
    }
}

/// The same criterion at the other flattened position: a `#[serde(flatten)]` field puts what its
/// type writes into the object being written, so a plain enum has nothing to put there either.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn flattening_a_registered_plain_enum_is_rejected() {
    register_alias_info("Hue", "Hue", "hue_schema", AliasKind::EnumMembers);
    register_alias_info(
        "Payload",
        "Payload",
        "payload_schema",
        AliasKind::NoEnumMembers,
    );

    let rejected: syn::Field = syn::parse_quote! { pub tone: Hue };
    let error = super::flattened_field_guard_error(&rejected, "Holder")
        .map(|tokens| tokens.to_string())
        .unwrap_or_default();
    assert!(error.contains("field `tone`"), "got: {error}");
    assert!(error.contains("`Holder`"), "got: {error}");
    assert!(error.contains("`Hue`"), "got: {error}");
    assert!(error.contains("#[serde(flatten)]"), "got: {error}");
    assert!(
        error.contains("does not write as an object"),
        "got: {error}"
    );

    for accepted in [
        syn::parse_quote! { pub body: Payload },
        syn::parse_quote! { pub body: NeverRegistered },
        syn::parse_quote! { pub tones: Vec<Hue> },
    ] {
        let field: syn::Field = accepted;
        assert!(
            super::flattened_field_guard_error(&field, "Holder").is_none(),
            "got a rejection for {}",
            quote::ToTokens::to_token_stream(&field)
        );
    }
}

/// Collects an enum's `cfg_attr` guard failures as rendered `compile_error!` token strings.
#[cfg(feature = "serde")]
fn enum_cfg_attr_errors(item: &syn::ItemEnum) -> Vec<String> {
    let type_meta = parse_serde_type_attributes(&item.attrs);
    enum_cfg_attr_guard_errors(item, &type_meta)
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[cfg(feature = "serde")]
#[test]
fn cfg_attr_wrapped_serde_on_a_type_is_rejected() {
    let errors = enum_cfg_attr_errors(&syn::parse_quote! {
        #[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
        enum Status {
            Active,
        }
    });
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    assert!(errors[0].contains("compile_error"), "got: {}", errors[0]);
    assert!(errors[0].contains("type `Status`"), "got: {}", errors[0]);
    assert!(errors[0].contains("cfg_attr"), "got: {}", errors[0]);
}

#[cfg(feature = "serde")]
#[test]
fn cfg_attr_wrapped_serde_on_a_variant_is_rejected() {
    let errors = enum_cfg_attr_errors(&syn::parse_quote! {
        enum Status {
            #[cfg_attr(feature = "serde", serde(rename = "active"))]
            Active,
        }
    });
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    assert!(errors[0].contains("compile_error"), "got: {}", errors[0]);
    assert!(errors[0].contains("variant `Active`"), "got: {}", errors[0]);
}

#[cfg(feature = "serde")]
#[test]
fn cfg_attr_without_serde_leaves_an_enum_alone() {
    let errors = enum_cfg_attr_errors(&syn::parse_quote! {
        #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
        #[serde(rename_all = "lowercase")]
        enum Status {
            #[cfg_attr(feature = "serde", doc = "only documented in serde builds")]
            Active,
        }
    });
    assert!(errors.is_empty(), "got: {errors:?}");
}

/// Runs the field walk the way [`process_field`] does and renders the `cfg_attr` guard failure.
#[cfg(feature = "serde")]
fn field_cfg_attr_error(item: &syn::ItemStruct) -> Option<String> {
    let field = item.fields.iter().next()?;
    let name = field
        .ident
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();
    parse_serde_field_attributes(&field.attrs)
        .cfg_attr_rejection
        .as_ref()
        .map(|rejection| cfg_attr_guard_error(rejection, &field_label(&name)).to_string())
}

#[cfg(feature = "serde")]
#[test]
fn cfg_attr_wrapped_serde_on_a_field_is_rejected() {
    let error = field_cfg_attr_error(&syn::parse_quote! {
        struct Report {
            #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
            note: Option<String>,
        }
    })
    .unwrap();
    assert!(error.contains("compile_error"), "got: {error}");
    assert!(error.contains("field `note`"), "got: {error}");
    assert!(error.contains("#[serde(...)]"), "got: {error}");
}

/// Runs the field walk the way [`process_field`] does and renders the `OsString` guard failure.
fn field_os_string_error(item: &syn::ItemStruct) -> Option<String> {
    let field = item.fields.iter().next()?;
    let name = field
        .ident
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();
    let field_def = get_field_def(&name, &field.ty, "");
    check_os_string_field(field, &field_def, &field_label(&name))
        .err()
        .map(|err| err.to_compile_error().to_string())
}

#[test]
fn an_os_string_field_is_rejected_by_name() {
    let error = field_os_string_error(&syn::parse_quote! {
        struct Report {
            location: OsString,
        }
    })
    .unwrap();
    assert!(error.contains("compile_error"), "got: {error}");
    assert!(error.contains("field `location`"), "got: {error}");
    assert!(error.contains("`OsString`"), "got: {error}");
    assert!(error.contains("externally tagged enum"), "got: {error}");
}

/// The guard reads through the wrappers the parser reads through, so a borrowed `OsStr` is named
/// as itself rather than as the wrapper it was written behind.
#[test]
fn a_wrapped_os_str_field_is_rejected_by_its_own_name() {
    let error = field_os_string_error(&syn::parse_quote! {
        struct Report {
            location: Box<OsStr>,
        }
    })
    .unwrap();
    assert!(error.contains("`OsStr`"), "got: {error}");
}

/// The path types the borrowed-form rule takes in are the ones this guard must not catch.
#[test]
fn a_path_field_is_left_alone() {
    for ty in [
        quote::quote! { PathBuf },
        quote::quote! { Box<Path> },
        quote::quote! { Cow<'static, Path> },
        quote::quote! { String },
    ] {
        let error = field_os_string_error(&syn::parse_quote! {
            struct Report {
                location: #ty,
            }
        });
        assert!(error.is_none(), "for {ty}, got: {error:?}");
    }
}

/// Runs the field walk the way [`super::process_field`] does and renders the map-key guard failure
/// the field's written type earns, or the empty string when it earns none.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn field_map_key_error(field_type: &proc_macro2::TokenStream) -> String {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct Report {
            counts: #field_type,
        }
    };
    let field = item.fields.iter().next().unwrap();
    let field_def = get_field_def("counts", &field.ty, "");
    check_map_key(field, &field_def, &field_label("counts"))
        .err()
        .map_or_else(String::new, |err| err.to_compile_error().to_string())
}

/// The registry proves a struct-keyed map has no members to name, and it proves it whatever surface
/// is being generated: the key is read off the field every one of them renders from, so the same
/// source cannot be a schema under one feature set and a refusal under another.
///
/// Every depth the key can be written at is covered, those being the positions the surfaces reach
/// it through: a field's own map, a map nested under either key flavor, a tuple slot, a sequence
/// wrapper, and a sibling's generic argument.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_map_key_proved_to_lack_enum_members_is_refused_wherever_it_is_written() {
    register_alias_info("Doc", "Doc", "doc_schema", AliasKind::NoEnumMembers);
    register_alias_info("Slot", "Slot", "slot_schema", AliasKind::EnumMembers);
    for field_type in [
        quote::quote! { HashMap<Doc, u32> },
        quote::quote! { HashMap<String, HashMap<Doc, u32>> },
        quote::quote! { HashMap<Slot, HashMap<Doc, u32>> },
        quote::quote! { Vec<HashMap<Doc, u32>> },
        quote::quote! { Option<HashMap<Doc, u32>> },
        quote::quote! { (String, HashMap<Doc, u32>) },
        quote::quote! { Wrapper<HashMap<Doc, u32>> },
        quote::quote! { HashMap<Doc, HashMap<Doc, u32>> },
    ] {
        let error = field_map_key_error(&field_type);
        assert!(error.contains("compile_error"), "for {field_type}: {error}");
        assert!(
            error.contains("field `counts`"),
            "for {field_type}: {error}"
        );
        assert!(
            error.contains("a map key must be a plain"),
            "for {field_type}: {error}"
        );
        assert!(error.contains("Doc"), "for {field_type}: {error}");
    }
}

/// A sequence-wrapped key writes a JSON array, which serde refuses as an object key outright, so
/// no surface has an object to describe and the field is refused instead — wherever the map is
/// written, and whichever sequence spelling wrote it, the parser having collapsed them all onto the
/// same array levels. The element the wrapper holds is named, that being the one part of the
/// spelling the levels leave recoverable.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_sequence_wrapped_map_key_is_refused_wherever_it_is_written() {
    register_alias_info("Slot", "Slot", "slot_schema", AliasKind::EnumMembers);
    for (field_type, element) in [
        (quote::quote! { HashMap<Vec<Slot>, u32> }, "Slot"),
        (quote::quote! { HashMap<[Slot; 2], u32> }, "Slot"),
        (quote::quote! { HashMap<HashSet<Slot>, u32> }, "Slot"),
        (quote::quote! { HashMap<Vec<Vec<Slot>>, u32> }, "Slot"),
        (quote::quote! { HashMap<Vec<String>, u32> }, "String"),
        (quote::quote! { HashMap<Vec<u32>, u64> }, "u32"),
        (
            quote::quote! { HashMap<String, HashMap<Vec<Slot>, u32>> },
            "Slot",
        ),
        (
            quote::quote! { HashMap<Slot, HashMap<Vec<Slot>, u32>> },
            "Slot",
        ),
        (quote::quote! { Vec<HashMap<Vec<Slot>, u32>> }, "Slot"),
        (quote::quote! { Option<HashMap<Vec<Slot>, u32>> }, "Slot"),
        (quote::quote! { (String, HashMap<Vec<Slot>, u32>) }, "Slot"),
        (quote::quote! { Wrapper<HashMap<Vec<Slot>, u32>> }, "Slot"),
    ] {
        let error = field_map_key_error(&field_type);
        assert!(error.contains("compile_error"), "for {field_type}: {error}");
        assert!(
            error.contains("field `counts`"),
            "for {field_type}: {error}"
        );
        assert!(
            error.contains("a map key must be a value serde writes as a string"),
            "for {field_type}: {error}"
        );
        assert!(error.contains(element), "for {field_type}: {error}");
    }
}

/// A key whose own type writes a JSON array or a JSON object earns the same refusal a
/// sequence-wrapped one does, and for the same reason: serde raises `key must be a string` and
/// refuses the whole map, so there is no object for any surface to describe. The key is named by the
/// shape it was written as, and what serde writes for it is named beside it.
///
/// Every depth is covered, the position a map sits in not being what decides whether its key can be
/// written.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn assert_unwritable_map_key(field_type: &proc_macro2::TokenStream, key_name: &str) {
    let error = field_map_key_error(field_type);
    assert!(error.contains("compile_error"), "for {field_type}: {error}");
    assert!(
        error.contains("field `counts`"),
        "for {field_type}: {error}"
    );
    assert!(
        error.contains("a map key must be a value serde writes as a string"),
        "for {field_type}: {error}"
    );
    assert!(error.contains(key_name), "for {field_type}: {error}");
    assert!(
        error.contains("refuses to serialize a map keyed by one"),
        "for {field_type}: {error}"
    );
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_map_key_serde_refuses_to_write_is_refused_wherever_it_is_written() {
    register_alias_info("Slot", "Slot", "slot_schema", AliasKind::EnumMembers);
    for (field_type, key_name) in [
        (quote::quote! { HashMap<(Slot, Slot), u32> }, "(_, _)"),
        (quote::quote! { HashMap<(String, u32), u32> }, "(_, _)"),
        (
            quote::quote! { HashMap<HashMap<String, u32>, u32> },
            "HashMap<_, _>",
        ),
        (
            quote::quote! { HashMap<BTreeMap<String, u32>, u32> },
            "HashMap<_, _>",
        ),
        (
            quote::quote! { HashMap<String, HashMap<(Slot, Slot), u32>> },
            "(_, _)",
        ),
        (
            quote::quote! { HashMap<Slot, HashMap<(Slot, Slot), u32>> },
            "(_, _)",
        ),
        (quote::quote! { Vec<HashMap<(Slot, Slot), u32>> }, "(_, _)"),
        (
            quote::quote! { Option<HashMap<(Slot, Slot), u32>> },
            "(_, _)",
        ),
        (
            quote::quote! { (String, HashMap<(Slot, Slot), u32>) },
            "(_, _)",
        ),
        (
            quote::quote! { Wrapper<HashMap<(Slot, Slot), u32>> },
            "(_, _)",
        ),
    ] {
        assert_unwritable_map_key(&field_type, key_name);
    }
}

/// An `ObjectId` writes a `{"$oid": ...}` object, so it joins the tuple and the nested map: serde
/// refuses a map keyed by one exactly as it refuses those.
#[cfg(all(
    feature = "object_id",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn an_object_id_map_key_is_refused_wherever_it_is_written() {
    register_alias_info("Slot", "Slot", "slot_schema", AliasKind::EnumMembers);
    for field_type in [
        quote::quote! { HashMap<ObjectId, u32> },
        quote::quote! { HashMap<String, HashMap<ObjectId, u32>> },
        quote::quote! { HashMap<Slot, HashMap<ObjectId, u32>> },
        quote::quote! { Vec<HashMap<ObjectId, u32>> },
        quote::quote! { (String, HashMap<ObjectId, u32>) },
        quote::quote! { Wrapper<HashMap<ObjectId, u32>> },
    ] {
        assert_unwritable_map_key(&field_type, "ObjectId");
    }
}

/// An `Option`-wrapped key writes what its inner writes for a `Some` and nothing a key can be for a
/// `None` — serde refuses the whole map the moment one is present — so the map is refused rather
/// than described by the half that serializes. The inner is named, that being the remedy.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn an_optional_map_key_is_refused_wherever_it_is_written() {
    register_alias_info("Slot", "Slot", "slot_schema", AliasKind::EnumMembers);
    for (field_type, inner) in [
        (quote::quote! { HashMap<Option<Slot>, u32> }, "Slot"),
        (quote::quote! { HashMap<Option<String>, u32> }, "String"),
        (quote::quote! { HashMap<Option<u32>, u64> }, "u32"),
        (
            quote::quote! { HashMap<String, HashMap<Option<Slot>, u32>> },
            "Slot",
        ),
        (
            quote::quote! { HashMap<Slot, HashMap<Option<Slot>, u32>> },
            "Slot",
        ),
        (quote::quote! { Vec<HashMap<Option<Slot>, u32>> }, "Slot"),
        (quote::quote! { Option<HashMap<Option<Slot>, u32>> }, "Slot"),
        (
            quote::quote! { (String, HashMap<Option<Slot>, u32>) },
            "Slot",
        ),
        (
            quote::quote! { Wrapper<HashMap<Option<Slot>, u32>> },
            "Slot",
        ),
    ] {
        let error = field_map_key_error(&field_type);
        assert!(error.contains("compile_error"), "for {field_type}: {error}");
        assert!(
            error.contains("field `counts`"),
            "for {field_type}: {error}"
        );
        assert!(
            error.contains("a map key must be a value serde writes as a string"),
            "for {field_type}: {error}"
        );
        assert!(
            error.contains(&format!("Option<{inner}>")),
            "for {field_type}: {error}"
        );
    }
}

/// The wrapper spellings answer in the order they were written, so a key wearing both keeps the
/// diagnostic of its outermost one: an optional sequence is still refused as the sequence its array
/// levels make it, and only a key whose outermost wrapper is the `Option` earns the `None`-key
/// wording.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_key_wrapped_twice_is_named_by_its_outermost_wrapper() {
    register_alias_info("Slot", "Slot", "slot_schema", AliasKind::EnumMembers);
    let sequenced = field_map_key_error(&quote::quote! { HashMap<Option<Vec<Slot>>, u32> });
    assert!(sequenced.contains("is a sequence of"), "got: {sequenced}");
    let optional = field_map_key_error(&quote::quote! { HashMap<Option<(Slot, Slot)>, u32> });
    assert!(optional.contains("Option<(_, _)>"), "got: {optional}");
}

/// A brand is `#[serde(transparent)]`, so a brand over a string writes the bare string a JSON object
/// key is — it keys a map exactly as `String` does and is left alone, at every depth. A brand over
/// anything else keeps the refusal it had: its wire is no key.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_string_wire_brand_keys_a_map_the_way_a_string_does() {
    register_alias_info("Slot", "Slot", "slot_schema", AliasKind::EnumMembers);
    register_alias_info(
        "CorrelationId",
        "CorrelationId",
        "correlation_id_schema",
        AliasKind::StringWire,
    );
    register_alias_info("Tick", "Tick", "tick_schema", AliasKind::NoEnumMembers);
    for field_type in [
        quote::quote! { HashMap<CorrelationId, u32> },
        quote::quote! { HashMap<String, HashMap<CorrelationId, u32>> },
        quote::quote! { HashMap<Slot, HashMap<CorrelationId, u32>> },
        quote::quote! { HashMap<CorrelationId, HashMap<CorrelationId, u32>> },
        quote::quote! { Vec<HashMap<CorrelationId, u32>> },
        quote::quote! { (String, HashMap<CorrelationId, u32>) },
        quote::quote! { Wrapper<HashMap<CorrelationId, u32>> },
    ] {
        let error = field_map_key_error(&field_type);
        assert!(error.is_empty(), "for {field_type}, got: {error}");
    }

    let refused = field_map_key_error(&quote::quote! { HashMap<Tick, u32> });
    assert!(
        refused.contains("a map key must be a plain"),
        "got: {refused}"
    );
    assert!(refused.contains("Tick"), "got: {refused}");
}

/// The guard is a filter, never a rewrite: a key the registry names as a plain enum, one it never
/// saw registered, and one no position enumerates all keep the field they had. A sequence in the
/// *value* is no key at all, so the sequence refusal does not reach across the map to it — nor does
/// any of its siblings, an `Option` value and a tuple value being ordinary members.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_map_key_that_may_have_enum_members_is_left_alone() {
    register_alias_info("Slot", "Slot", "slot_schema", AliasKind::EnumMembers);
    for field_type in [
        quote::quote! { HashMap<Slot, u32> },
        quote::quote! { HashMap<String, u32> },
        quote::quote! { HashMap<Ghost, u32> },
        quote::quote! { HashMap<u32, u64> },
        quote::quote! { HashMap<bool, u64> },
        quote::quote! { HashMap<String, HashMap<Slot, u32>> },
        quote::quote! { HashMap<Slot, Vec<u32>> },
        quote::quote! { HashMap<String, Vec<Slot>> },
        quote::quote! { HashMap<String, Option<Slot>> },
        quote::quote! { HashMap<String, (Slot, Slot)> },
        quote::quote! { HashMap<String, HashMap<String, u32>> },
        quote::quote! { Vec<HashMap<Slot, u32>> },
    ] {
        let error = field_map_key_error(&field_type);
        assert!(error.is_empty(), "for {field_type}, got: {error}");
    }
}

/// Every key serde stringifies for the author keeps the open object it has always described as: the
/// rule is refuse-what-serde-refuses, never refuse-what-is-not-a-`String`.
#[cfg(all(
    feature = "chrono",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn a_chrono_map_key_is_left_alone() {
    for field_type in [
        quote::quote! { HashMap<NaiveDate, u32> },
        quote::quote! { HashMap<NaiveTime, u32> },
        quote::quote! { HashMap<NaiveDateTime, u32> },
        quote::quote! { HashMap<DateTime<Utc>, u32> },
    ] {
        let error = field_map_key_error(&field_type);
        assert!(error.is_empty(), "for {field_type}, got: {error}");
    }
}

/// An alias publishes the target type's own schema, so a target reaching a key with no members
/// leaves every surface naming keys nothing can supply — the same refusal a field of that type
/// earns, named for the alias the author wrote.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn an_alias_targeting_a_map_key_with_no_members_is_refused() {
    register_alias_info("Doc", "Doc", "doc_schema", AliasKind::NoEnumMembers);
    let alias: syn::ItemType = syn::parse_quote! {
        pub type CountsByDoc = HashMap<Doc, u32>;
    };
    let field_def = get_field_def("CountsByDocType", &alias.ty, "");
    let error = alias_map_key_guard_error(&alias, "CountsByDocType", &field_def)
        .unwrap_or_default()
        .to_string();
    assert!(error.contains("compile_error"), "got: {error}");
    assert!(
        error.contains("type alias `CountsByDocType`"),
        "got: {error}"
    );
    assert!(error.contains("a map key must be a plain"), "got: {error}");
    assert!(error.contains("Doc"), "got: {error}");
}

/// A refused item still publishes the schema module every reference to it addresses.
///
/// The address is derived from the Rust ident and nothing else, so it is the same whatever became
/// of the item — which is what lets a reference stand before it. An expansion that emitted no
/// module left every referencing type with an `E0433` naming a module the author never wrote,
/// sitting on top of the refusal they can act on.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_refused_item_publishes_the_module_a_reference_to_it_resolves_to() {
    let ident = syn::Ident::new("CountsByDoc", proc_macro2::Span::call_site());
    let module = super::refused_item_schema_module(&ident).to_string();
    assert!(
        module.contains(&format!(
            "pub mod {}",
            ident_schema_module_name("CountsByDoc")
        )),
        "got: {module}"
    );
    // The refusal is the one diagnostic the author reads, so the module adds none of its own.
    assert!(!module.contains("compile_error"), "got: {module}");
}

/// And it publishes the call a reference emits: a sibling in field position asks the module it
/// resolves to for `json_schema_within`, so that is the method that has to be there for the
/// reference to compile.
#[cfg(feature = "jsonschema")]
#[test]
fn a_refused_items_module_answers_the_call_a_reference_emits() {
    let span = proc_macro2::Span::call_site();
    let ident = syn::Ident::new("CountsByRefusedDoc", span);
    let module = super::refused_item_schema_module(&ident).to_string();
    let addressed = super::sibling_schema_module_ident("CountsByRefusedDoc", span).to_string();
    assert!(
        module.contains(&format!("pub mod {addressed}")),
        "got: {module}"
    );
    assert!(module.contains("json_schema_within"), "got: {module}");
}

/// Every attribute the walked fields are left carrying, rendered as the emitted item carries them.
#[cfg(feature = "serde")]
fn walked_field_attrs<'field>(fields: impl Iterator<Item = &'field syn::Field>) -> String {
    let attrs: Vec<&syn::Attribute> = fields.flat_map(|field| field.attrs.iter()).collect();
    quote::quote!(#(#attrs)*).to_string()
}

/// Every field of an enum, in the order the walks visit them.
#[cfg(feature = "serde")]
fn enum_fields(item: &syn::ItemEnum) -> impl Iterator<Item = &syn::Field> {
    item.variants
        .iter()
        .flat_map(|variant| variant.fields.iter())
}

/// A refused pattern leaves the item without the `deserialize_with` naming the module the refusal
/// drops.
///
/// A pattern is recorded as written whether or not a guard refused it, so the constraint is still
/// a string constraint by the time the hook is generated: the item was emitted carrying
/// `#[serde(deserialize_with = "probe_caret_schema::deserialize_anything")]` beside the
/// `compile_error!`, while the module holding that function was replaced by the absorbing one —
/// an `E0425` pointing at the attribute rather than at anything the author wrote.
#[cfg(feature = "serde")]
#[test]
fn a_refused_pattern_leaves_no_hook_naming_the_dropped_module() {
    let mut item: syn::ItemStruct = syn::parse_quote! {
        struct ProbeCaret {
            #[model_schema_prop(pattern = "^.$")]
            anything: String,
        }
    };
    let errors = super::collect_struct_fields(
        &mut item.fields,
        None,
        Some("probe_caret_schema"),
        "ProbeCaret",
        &syn::Generics::default(),
        false,
    )
    .4;
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    assert!(
        errors[0].to_string().contains("any-character class"),
        "got: {}",
        errors[0]
    );
    let rendered = walked_field_attrs(item.fields.iter());
    assert_eq!(rendered, "", "got: {rendered}");
}

/// The same for a constraint written where the position cannot carry it: the refused slot takes
/// none, and the member that would have earned one on its own is held back with it, the whole
/// item's surface having been dropped.
#[cfg(feature = "serde")]
#[test]
fn a_constraint_refused_its_placement_leaves_no_hook_naming_the_dropped_module() {
    let mut item: syn::ItemEnum = syn::parse_quote! {
        enum Action {
            Slug(#[model_schema_prop(minLength = 2)] String),
            Named {
                #[model_schema_prop(minLength = 2)]
                name: String,
            },
        }
    };
    let errors = collect_discriminated_variants(&mut item, None, Some("action_schema")).2;
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    assert!(
        errors[0]
            .to_string()
            .contains("unsupported on a positional field"),
        "got: {}",
        errors[0]
    );
    let rendered = walked_field_attrs(enum_fields(&item));
    assert_eq!(rendered, "", "got: {rendered}");
}

/// And for a map key no surface can write: the key is refused, and the constrained member beside
/// it keeps no hook naming a module that is no longer published.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn a_refused_map_key_leaves_no_hook_naming_the_dropped_module() {
    register_alias_info("Doc", "Doc", "doc_schema", AliasKind::NoEnumMembers);
    let mut item: syn::ItemEnum = syn::parse_quote! {
        enum Choice {
            Named {
                #[model_schema_prop(minLength = 2)]
                name: String,
                counts: HashMap<Doc, u32>,
            },
        }
    };
    let errors = collect_untagged_members(&mut item, UNTAGGED_MODULE).5;
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    assert!(
        errors[0].to_string().contains("a map key must be a plain"),
        "got: {}",
        errors[0]
    );
    let rendered = walked_field_attrs(enum_fields(&item));
    assert_eq!(rendered, "", "got: {rendered}");
}

/// A field that clears every guard still carries the hook, written after the attributes the
/// declaration itself had.
#[cfg(feature = "serde")]
#[test]
fn a_field_that_clears_the_guards_carries_the_hook_it_earned() {
    let mut item: syn::ItemStruct = syn::parse_quote! {
        struct Report {
            #[serde(rename = "label")]
            #[model_schema_prop(minLength = 2)]
            name: String,
        }
    };
    let errors = super::collect_struct_fields(
        &mut item.fields,
        None,
        Some("report_schema"),
        "Report",
        &syn::Generics::default(),
        false,
    )
    .4;
    assert!(errors.is_empty(), "got: {errors:?}");
    let rendered = walked_field_attrs(item.fields.iter());
    assert_eq!(
        rendered,
        quote::quote! {
            #[serde(rename = "label")]
            #[serde(deserialize_with = "report_schema::deserialize_name")]
        }
        .to_string(),
        "got: {rendered}"
    );
}

/// The type the parser reads a field's written spelling as, rendered the way every surface receives
/// it: one `FieldDef`, so a spelling that parses alike describes alike wherever it is dispatched.
fn parsed_field_type(field_type: &proc_macro2::TokenStream) -> String {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct Report {
            counts: #field_type,
        }
    };
    let field = item.fields.iter().next().unwrap();
    format!("{:?}", get_field_def("counts", &field.ty, "").field_type)
}

/// The reported failure: std's `HashMap<K, V, S>` and `HashSet<T, S>` carry a hasher past the types
/// they write, and the arity-keyed arms read that argument as a type of its own — demoting the
/// container to a sibling naming a schema module the expansion never writes. serde writes the same
/// bytes whichever hasher is named, so a container is read by its own name and the arguments past
/// its wire form are dropped before any arm is consulted.
#[test]
fn a_container_written_with_a_hasher_parses_as_the_container_without_one() {
    for (written, implied) in [
        (
            quote::quote! { HashMap<String, u32, FxBuildHasher> },
            quote::quote! { HashMap<String, u32> },
        ),
        (
            quote::quote! { HashSet<String, FxBuildHasher> },
            quote::quote! { HashSet<String> },
        ),
        (
            quote::quote! { HashMap<String, HashSet<u32, FxBuildHasher>> },
            quote::quote! { HashMap<String, HashSet<u32>> },
        ),
        (
            quote::quote! { Option<HashSet<u32, FxBuildHasher>> },
            quote::quote! { Option<HashSet<u32>> },
        ),
    ] {
        assert_eq!(
            parsed_field_type(&written),
            parsed_field_type(&implied),
            "for {written}"
        );
    }
}

/// A container named with fewer arguments than its wire form is written from is not that container,
/// and is left to fall through as the sibling it was written as — where the schema module it names
/// is reported unresolvable against the type the author wrote, rather than quietly read as a map.
#[test]
fn a_container_short_of_its_wire_arity_still_falls_through_as_a_sibling() {
    assert_eq!(
        parsed_field_type(&quote::quote! { HashMap<String> }),
        parsed_field_type(&quote::quote! { Wrapper<String> }).replace("Wrapper", "HashMap"),
    );
}

/// Runs the field walk the way [`super::process_field`] does and renders the guard failures its
/// `model_schema_prop` attributes earn, so a refused key and an unparseable `pattern` are read off
/// the same channel that carries them to the emitted item.
///
/// `parameters` names what the enclosing item declares, which is what decides whether a name the
/// field is written with is a reference to another type or one of the item's own parameters — the
/// same list [`super::process_field`] hands the walk.
fn field_prop_guard_errors_in_scope(item: &syn::ItemStruct, parameters: &[String]) -> Vec<String> {
    let field = item.fields.iter().next().unwrap();
    let name = field
        .ident
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();
    let written = get_field_def(&name, &field.ty, "");
    let mut rendered = written.clone();
    rendered.erase_type_parameters(parameters);
    let meta = super::parse_model_schema_prop_attributes(&field.attrs);
    super::collect_field_guard_errors(field, &rendered, &written, &name, &meta, Vec::new())
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn field_prop_guard_errors(item: &syn::ItemStruct) -> Vec<String> {
    field_prop_guard_errors_in_scope(item, &[])
}

/// The guard's verdict is the `regex` crate's verdict: the parse the generated validator's
/// `Regex::new` would run, moved to expansion. Driving the expectation off `Regex::new` itself is
/// what keeps the two from drifting as the crate's grammar changes.
#[test]
fn the_field_pattern_guard_follows_the_regex_crate() {
    for pattern in PROBE_PATTERNS {
        let rejected = regex::Regex::new(pattern).is_err();
        let errors = field_prop_guard_errors(&syn::parse_quote! {
            struct Report {
                #[model_schema_prop(pattern = #pattern)]
                name: String,
            }
        });
        assert_eq!(
            errors.len(),
            usize::from(rejected),
            "for {pattern}, got: {errors:?}"
        );
    }
}

/// The trailing backslash from the report: it terminates no escape, so `Regex::new` fails and the
/// Zod literal it would otherwise feed swallows its own closing delimiter.
#[test]
fn a_field_pattern_the_regex_crate_rejects_names_the_field_and_quotes_the_parse_error() {
    let errors = field_prop_guard_errors(&syn::parse_quote! {
        struct Report {
            #[model_schema_prop(pattern = r"^ab\")]
            name: String,
        }
    });
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    for needle in [
        "compile_error",
        "field `name`",
        "pattern",
        "regex parse error",
        "incomplete escape sequence",
    ] {
        assert!(
            errors[0].contains(needle),
            "{needle} missing: {}",
            errors[0]
        );
    }
}

/// A pattern the `regex` crate parses is still refused when no JavaScript regex literal carries
/// it: the field's Zod schema and JSON Schema are generated from the same string the validator
/// gets, so a construct only one of the two grammars reads reaches a surface that cannot say it.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_field_pattern_javascript_cannot_carry_names_the_field_and_the_construct() {
    for (pattern, construct) in UNPORTABLE_PROBE_PATTERNS {
        assert!(
            regex::Regex::new(pattern).is_ok(),
            "{pattern} is not a pattern the `regex` crate accepts, so it probes nothing"
        );
        let errors = field_prop_guard_errors(&syn::parse_quote! {
            struct Report {
                #[model_schema_prop(pattern = #pattern)]
                name: String,
            }
        });
        assert_eq!(errors.len(), 1, "for {pattern}, got: {errors:?}");
        for needle in ["compile_error", "field `name`", construct] {
            assert!(
                errors[0].contains(needle),
                "{needle} missing for {pattern}: {}",
                errors[0]
            );
        }
    }
}

/// `(?P<name>...)` is the one construct the two grammars merely spell differently, so it clears the
/// guard rather than tripping it.
#[test]
fn a_field_pattern_naming_a_group_the_rust_way_clears_the_guard() {
    let errors = field_prop_guard_errors(&syn::parse_quote! {
        struct Report {
            #[model_schema_prop(pattern = r"^(?P<word>[a-z]+)$")]
            name: String,
        }
    });
    assert!(errors.is_empty(), "got: {errors:?}");
}

/// A pattern every string satisfies is refused where it is written, naming the field, the way a
/// bound written where no surface reads one is. The alternative — taking it and emitting no check
/// — would leave the author a contract nothing enforces, and would leave the emitted
/// `validate_..._value(value: &str)` with `value` unread, which the consumer's own deny set turns
/// into a second failure it has no edit for.
#[test]
fn a_field_pattern_admitting_every_value_names_the_field_and_says_so() {
    for pattern in ["", "^", "$", "|", "a*", "^a*"] {
        let errors = field_prop_guard_errors(&syn::parse_quote! {
            struct Report {
                #[model_schema_prop(pattern = #pattern)]
                name: String,
            }
        });
        assert_eq!(errors.len(), 1, "for {pattern:?}, got: {errors:?}");
        for needle in [
            "compile_error",
            "field `name`",
            "admits every value",
            "constrains nothing",
        ] {
            assert!(
                errors[0].contains(needle),
                "{needle} missing for {pattern:?}: {}",
                errors[0]
            );
        }
    }
}

/// The shapes written out of the same pieces that still turn a value away clear the guard: `^$`
/// asks for the empty string, `^a*$` for a run of `a`, and `\b` for a word boundary.
#[test]
fn a_field_pattern_written_out_of_the_same_pieces_that_still_constrains_clears_the_guard() {
    for pattern in ["^$", "^a*$", r"\b"] {
        let errors = field_prop_guard_errors(&syn::parse_quote! {
            struct Report {
                #[model_schema_prop(pattern = #pattern)]
                name: String,
            }
        });
        assert!(errors.is_empty(), "for {pattern:?}, got: {errors:?}");
    }
}

/// A field carrying no `pattern` at all must not acquire one of these errors.
#[test]
fn an_unpatterned_field_is_left_alone() {
    let errors = field_prop_guard_errors(&syn::parse_quote! {
        struct Report {
            #[model_schema_prop(minLength = 3)]
            name: String,
        }
    });
    assert!(errors.is_empty(), "got: {errors:?}");
}

/// The reported repro: two misspelled keys, which emitted `z.string()` with nothing on it. The
/// misspelling reaches the author on the same channel the `pattern` guard uses, naming the field
/// and the key as written; parsing stops there, so the second misspelling is not reached.
#[test]
fn a_misspelled_field_prop_key_names_the_field_and_the_key_as_written() {
    let errors = field_prop_guard_errors(&syn::parse_quote! {
        struct Report {
            #[model_schema_prop(patern = "^[a-z]+$", minLenght = 3)]
            name: String,
        }
    });
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    for needle in ["compile_error", "field `name`", "patern", "pattern"] {
        assert!(
            errors[0].contains(needle),
            "{needle} missing: {}",
            errors[0]
        );
    }
}

/// A value the parser cannot read is the same class of loss as a key it cannot read — the
/// constraint reaches no surface — and leaves by the same channel.
#[test]
fn a_field_prop_value_the_parser_cannot_read_names_the_field() {
    let errors = field_prop_guard_errors(&syn::parse_quote! {
        struct Report {
            #[model_schema_prop(minLength = "3")]
            name: String,
        }
    });
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    assert!(errors[0].contains("field `name`"), "got: {}", errors[0]);
}

/// The two `model_schema_prop` guards are independent: a refused key does not swallow the
/// unparseable `pattern` the same attribute already carried.
#[test]
fn a_refused_key_and_an_unparseable_pattern_are_both_reported() {
    let errors = field_prop_guard_errors(&syn::parse_quote! {
        struct Report {
            #[model_schema_prop(pattern = r"^ab\", patern = "^[a-z]+$")]
            name: String,
        }
    });
    assert_eq!(errors.len(), 2, "got: {errors:?}");
    assert!(errors[0].contains("patern"), "got: {}", errors[0]);
    assert!(
        errors[1].contains("regex parse error"),
        "got: {}",
        errors[1]
    );
}

/// The types whose schema this crate writes whole read no bound off the meta, on any surface, so a
/// bound written on one is refused where it is written instead of accepted and dropped.
#[cfg(feature = "chrono")]
#[test]
fn a_bound_on_a_chrono_field_is_refused() {
    for (constraint, key) in [
        (quote::quote! { minLength = 30 }, "minLength"),
        (quote::quote! { maxLength = 30 }, "maxLength"),
        (quote::quote! { pattern = "^[0-9-]+$" }, "pattern"),
        (quote::quote! { minimum = 5 }, "minimum"),
        (quote::quote! { maximum = 5 }, "maximum"),
    ] {
        for field_type in [
            quote::quote! { chrono::NaiveDate },
            quote::quote! { Option<chrono::NaiveTime> },
            quote::quote! { Vec<chrono::NaiveDateTime> },
            quote::quote! { chrono::DateTime<chrono::Utc> },
        ] {
            let errors = field_prop_guard_errors(&syn::parse_quote! {
                struct Report {
                    #[model_schema_prop(#constraint)]
                    when: #field_type,
                }
            });
            assert_eq!(errors.len(), 1, "for {key} on {field_type}: {errors:?}");
            for needle in ["compile_error", "field `when`", key, "chrono::"] {
                assert!(
                    errors[0].contains(needle),
                    "{needle} missing for {key} on {field_type}: {}",
                    errors[0]
                );
            }
        }
    }
}

/// An `ObjectId` writes an object, not the string a length or a pattern measures, so it answers as
/// the chrono types do.
#[cfg(feature = "object_id")]
#[test]
fn a_bound_on_an_object_id_field_is_refused() {
    let errors = field_prop_guard_errors(&syn::parse_quote! {
        struct Report {
            #[model_schema_prop(minLength = 30, pattern = "^[a-f]+$")]
            id: ObjectId,
        }
    });
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    for needle in [
        "compile_error",
        "field `id`",
        "minLength",
        "pattern",
        "ObjectId",
    ] {
        assert!(
            errors[0].contains(needle),
            "{needle} missing: {}",
            errors[0]
        );
    }
}

/// A chrono or `ObjectId` field carrying no bound must not acquire one of these errors, and neither
/// must the keys that name the type rather than constrain the value.
#[cfg(all(feature = "chrono", feature = "object_id"))]
#[test]
fn a_fixed_shape_field_without_a_bound_is_left_alone() {
    for field in [
        quote::quote! { when: chrono::NaiveDate },
        quote::quote! { #[model_schema_prop(as_number)] when: chrono::DateTime<chrono::Utc> },
        quote::quote! { #[model_schema_prop(preprocess = ["trim"])] when: chrono::NaiveDate },
        quote::quote! { id: ObjectId },
    ] {
        let errors = field_prop_guard_errors(&syn::parse_quote! {
            struct Report {
                #field
            }
        });
        assert!(errors.is_empty(), "for {field}: {errors:?}");
    }
}

/// A map and a tuple render their members, never themselves, so a bound written beside one reaches
/// no surface either — the same loss the whole-schema types answer for, refused where it is written.
#[test]
fn a_bound_on_a_map_or_tuple_field_is_refused() {
    for (constraint, key) in [
        (quote::quote! { minLength = 30 }, "minLength"),
        (quote::quote! { maxLength = 30 }, "maxLength"),
        (quote::quote! { pattern = "^[a-z]+$" }, "pattern"),
        (quote::quote! { minimum = 5 }, "minimum"),
        (quote::quote! { maximum = 5 }, "maximum"),
    ] {
        for (field_type, shape) in [
            (quote::quote! { HashMap<String, String> }, "a map"),
            (quote::quote! { Option<HashMap<String, u32>> }, "a map"),
            (quote::quote! { Vec<HashMap<String, u32>> }, "a map"),
            (quote::quote! { (String, String) }, "a tuple"),
            (quote::quote! { Option<(String, u32)> }, "a tuple"),
        ] {
            let errors = field_prop_guard_errors(&syn::parse_quote! {
                struct Report {
                    #[model_schema_prop(#constraint)]
                    labels: #field_type,
                }
            });
            assert_eq!(errors.len(), 1, "for {key} on {field_type}: {errors:?}");
            for needle in ["compile_error", "field `labels`", key, shape, "brand"] {
                assert!(
                    errors[0].contains(needle),
                    "{needle} missing for {key} on {field_type}: {}",
                    errors[0]
                );
            }
        }
    }
}

/// A map or tuple field carrying no bound must not acquire one of these errors, and neither must the
/// keys that name or wrap the rendering rather than constrain a value.
#[test]
fn a_map_or_tuple_field_without_a_bound_is_left_alone() {
    for field in [
        quote::quote! { labels: HashMap<String, String> },
        quote::quote! { pair: (String, u32) },
        quote::quote! { #[model_schema_prop(preprocess = ["trim"])] pair: (String, u32) },
    ] {
        let errors = field_prop_guard_errors(&syn::parse_quote! {
            struct Report {
                #field
            }
        });
        assert!(errors.is_empty(), "for {field}: {errors:?}");
    }
}

/// A parameter names no type until the item is instantiated, so a bound written on a field typed
/// with one is held by nothing at all: the two validating surfaces describe the value as opaque,
/// which takes no length, no pattern and no range, and the generated validator and serde read
/// whatever type the instantiation supplied. That is the map's and the tuple's loss under another
/// spelling, and it is refused where it is written for the same reason — at every depth the
/// parameter can be reached through, the wrappers collapsing onto the value a bound would measure.
#[test]
fn a_bound_on_a_parameter_typed_field_is_refused() {
    for (constraint, key) in [
        (quote::quote! { minLength = 30 }, "minLength"),
        (quote::quote! { maxLength = 30 }, "maxLength"),
        (quote::quote! { pattern = "^[a-z]+$" }, "pattern"),
        (quote::quote! { minimum = 5 }, "minimum"),
        (quote::quote! { maximum = 5 }, "maximum"),
    ] {
        for field_type in [
            quote::quote! { IdType },
            quote::quote! { Option<IdType> },
            quote::quote! { Vec<IdType> },
            quote::quote! { Option<Vec<IdType>> },
        ] {
            let errors = field_prop_guard_errors_in_scope(
                &syn::parse_quote! {
                    struct Report {
                        #[model_schema_prop(#constraint)]
                        labels: #field_type,
                    }
                },
                &["IdType".to_owned()],
            );
            assert_eq!(errors.len(), 1, "for {key} on {field_type}: {errors:?}");
            for needle in [
                "compile_error",
                "field `labels`",
                key,
                "type parameter",
                "IdType",
                "brand",
            ] {
                assert!(
                    errors[0].contains(needle),
                    "{needle} missing for {key} on {field_type}: {}",
                    errors[0]
                );
            }
        }
    }
}

/// The refusal turns on the bound and on the name being the item's own, so a parameter-typed field
/// carrying none clears it — and so does the same bound on a concrete field standing beside the
/// parameter, which every surface still reads it for. A name the item does not declare is a
/// reference to another type and keeps whatever that type's own rendering earns.
#[test]
fn a_parameter_in_scope_only_refuses_the_field_that_carries_a_bound() {
    for (field, parameters) in [
        (quote::quote! { labels: IdType }, &["IdType".to_owned()][..]),
        (
            quote::quote! { #[model_schema_prop(preprocess = ["trim"])] labels: IdType },
            &["IdType".to_owned()][..],
        ),
        (
            quote::quote! { #[model_schema_prop(minLength = 30)] labels: String },
            &["IdType".to_owned()][..],
        ),
        (
            quote::quote! { #[model_schema_prop(minLength = 30)] labels: IdType },
            &[][..],
        ),
    ] {
        let errors = field_prop_guard_errors_in_scope(
            &syn::parse_quote! {
                struct Report {
                    #field
                }
            },
            parameters,
        );
        assert!(errors.is_empty(), "for {field}: {errors:?}");
    }
}

/// The docs a field's meta earns, read off the walk that writes them, inside an item declaring
/// `parameters`.
fn field_docs_after_meta_in_scope(item: &syn::ItemStruct, parameters: &[String]) -> String {
    let field = item.fields.iter().next().unwrap();
    let name = field
        .ident
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();
    let mut field_def = get_field_def(&name, &field.ty, "");
    field_def.erase_type_parameters(parameters);
    let meta = super::parse_model_schema_prop_attributes(&field.attrs);
    super::apply_model_schema_prop_meta(&mut field_def, meta, &name);
    field_def.docs
}

fn field_docs_after_meta(item: &syn::ItemStruct) -> String {
    field_docs_after_meta_in_scope(item, &[])
}

/// The `JSDoc` states the bound as a rule the value is held to, so it is written only where
/// something holds the value to it — never for a placement the guard refuses, which was the one
/// place the sentence appeared over nothing at all.
#[test]
fn the_constraint_docs_are_written_only_where_the_bound_is_kept() {
    assert!(
        field_docs_after_meta(&syn::parse_quote! {
            struct Report {
                #[model_schema_prop(minLength = 30)]
                name: String,
            }
        })
        .contains("Minimum length: 30"),
        "a bound the surfaces render says so in the docs"
    );
    for field in [
        quote::quote! { #[model_schema_prop(minLength = 30)] labels: HashMap<String, String> },
        quote::quote! { #[model_schema_prop(maximum = 5)] pair: (u32, u32) },
    ] {
        let docs = field_docs_after_meta(&syn::parse_quote! {
            struct Report {
                #field
            }
        });
        assert!(docs.is_empty(), "for {field}, got: {docs}");
    }
}

/// The `JSDoc` was the one place a bound on a parameter-typed field appeared at all — every gate it
/// named was silent — so the sentence goes where the refusal does, off the same question both are
/// written from. A concrete field standing in the same generic item keeps its own.
#[test]
fn the_constraint_docs_are_silent_for_a_parameter_typed_field() {
    let parameters = ["IdType".to_owned()];
    for field in [
        quote::quote! { #[model_schema_prop(minLength = 30)] id: IdType },
        quote::quote! { #[model_schema_prop(maximum = 5)] id: Option<IdType> },
        quote::quote! { #[model_schema_prop(minimum = 5)] id: Vec<IdType> },
    ] {
        let docs = field_docs_after_meta_in_scope(
            &syn::parse_quote! {
                struct Report {
                    #field
                }
            },
            &parameters,
        );
        assert!(docs.is_empty(), "for {field}, got: {docs}");
    }

    assert!(
        field_docs_after_meta_in_scope(
            &syn::parse_quote! {
                struct Report {
                    #[model_schema_prop(minLength = 30)]
                    id: String,
                }
            },
            &parameters,
        )
        .contains("Minimum length: 30"),
        "a bound the surfaces render says so in the docs, parameters in scope or not"
    );
}

/// `as` names the type the field already renders or it names nothing the expansion can honor: the
/// surfaces are written from the declared type, and no second reading of the wire exists here.
#[test]
fn an_as_naming_another_type_is_refused() {
    let errors = field_prop_guard_errors(&syn::parse_quote! {
        struct Report {
            #[model_schema_prop(as = String)]
            id: u64,
        }
    });
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    for needle in ["compile_error", "field `id`", "as = String", "u64"] {
        assert!(
            errors[0].contains(needle),
            "{needle} missing: {}",
            errors[0]
        );
    }
}

/// The target may name the field itself or the value under its wrappers — the two readings the
/// shipped uses of the key are written in — and neither is an override of anything.
#[test]
fn an_as_naming_the_rendered_type_is_accepted() {
    for field in [
        quote::quote! { #[model_schema_prop(as = String)] name: String },
        quote::quote! { #[model_schema_prop(as = String)] name: Option<String> },
        quote::quote! { #[model_schema_prop(as = String)] name: Vec<String> },
        quote::quote! { #[model_schema_prop(as = Vec<String>)] name: Vec<String> },
        quote::quote! { #[model_schema_prop(as = Inner)] name: Option<Inner> },
        quote::quote! { #[model_schema_prop(as = String)] name: PathBuf },
    ] {
        let errors = field_prop_guard_errors(&syn::parse_quote! {
            struct Report {
                #field
            }
        });
        assert!(errors.is_empty(), "for {field}: {errors:?}");
    }
}

/// The three misuses that aborted expansion with `custom attribute panicked`, spanned on the field
/// that carries them and carrying the message their validator already spelled.
#[test]
fn the_field_prop_misuses_leave_by_the_guard_channel() {
    for (field, needle) in [
        (
            quote::quote! { #[model_schema_prop(ts_optional)] name: String },
            "requires an Option<T> field",
        ),
        (
            quote::quote! { #[model_schema_prop(as_number)] name: u32 },
            "requires a chrono DateTime<Tz> field",
        ),
        (
            quote::quote! { #[model_schema_prop(as = String, preprocess = ["trim"])] name: String },
            "cannot be written on the same field",
        ),
    ] {
        let errors = field_prop_guard_errors(&syn::parse_quote! {
            struct Report {
                #field
            }
        });
        assert_eq!(errors.len(), 1, "for {field}: {errors:?}");
        for expected in ["compile_error", "field `name`", needle] {
            assert!(
                errors[0].contains(expected),
                "{expected} missing for {field}: {}",
                errors[0]
            );
        }
    }
}

/// The valid spellings of the same three keys stay valid.
#[test]
fn the_field_prop_flags_on_the_shapes_they_fit_are_accepted() {
    for field in [
        quote::quote! { #[model_schema_prop(ts_optional)] name: Option<String> },
        quote::quote! { #[model_schema_prop(preprocess = ["trim"])] name: String },
        quote::quote! { #[model_schema_prop(as = String, minLength = 1)] name: String },
    ] {
        let errors = field_prop_guard_errors(&syn::parse_quote! {
            struct Report {
                #field
            }
        });
        assert!(errors.is_empty(), "for {field}: {errors:?}");
    }
}

#[cfg(feature = "serde")]
#[test]
fn cfg_attr_without_serde_leaves_a_field_alone() {
    let error = field_cfg_attr_error(&syn::parse_quote! {
        struct Report {
            #[cfg_attr(feature = "serde", doc = "only documented in serde builds")]
            #[serde(skip_serializing_if = "Option::is_none")]
            note: Option<String>,
        }
    });
    assert!(error.is_none(), "got: {error:?}");
}

/// Reports whether `tokens` contains a `#[cfg(...)]` / `#![cfg(...)]` attribute at any nesting
/// depth.
fn contains_cfg_attribute(tokens: proc_macro2::TokenStream) -> bool {
    let mut in_attr_prefix = false;
    for tree in tokens {
        match tree {
            proc_macro2::TokenTree::Punct(punct) => {
                let ch = punct.as_char();
                in_attr_prefix = ch == '#' || (in_attr_prefix && ch == '!');
            }
            proc_macro2::TokenTree::Group(group) => {
                let is_cfg_attr = in_attr_prefix
                    && group.delimiter() == proc_macro2::Delimiter::Bracket
                    && matches!(
                        group.stream().into_iter().next(),
                        Some(proc_macro2::TokenTree::Ident(ident)) if ident == "cfg"
                    );
                if is_cfg_attr || contains_cfg_attribute(group.stream()) {
                    return true;
                }
                in_attr_prefix = false;
            }
            proc_macro2::TokenTree::Ident(_) | proc_macro2::TokenTree::Literal(_) => {
                in_attr_prefix = false;
            }
        }
    }
    false
}

/// A `cfg` attribute in the macro's output is resolved against the *consumer's* feature table,
/// not tixschema's, so an item emitted per tixschema's features can call a method the consumer
/// cfg'd away. Every feature decision must be made while building the tokens, never emitted.
fn assert_no_cfg_attribute(tokens: &proc_macro2::TokenStream, what: &str) {
    assert!(
        !contains_cfg_attribute(tokens.clone()),
        "{what} emitted a cfg attribute into generated code: {tokens}"
    );
}

#[test]
fn the_cfg_probe_sees_an_emitted_cfg_attribute() {
    assert!(contains_cfg_attribute(quote::quote! {
        #[cfg(feature = "zod")]
        pub fn zod_schema() -> String { String::new() }
    }));
    assert!(contains_cfg_attribute(quote::quote! {
        pub fn wrapper() {
            #[cfg(feature = "zod")]
            let _ = 1;
        }
    }));
    assert_no_cfg_attribute(
        &quote::quote! {
            pub fn zod_schema() -> String { String::new() }
        },
        "a cfg-free token stream",
    );
}

#[cfg(feature = "zod")]
#[test]
fn struct_schema_example_carries_no_cfg_attribute() {
    let name: syn::Ident = syn::parse_quote!(Report);
    let generics = syn::Generics::default();
    let tokens = super::item_schema_example_method(
        Some(&"Report { id: 1 }".to_owned()),
        &name,
        &generics,
        &super::ModelSchemaArgs::default(),
    )
    .unwrap();
    assert_no_cfg_attribute(&tokens, "item_schema_example_method");
}

/// The type the example is bound at carries one argument per declared parameter, the way a brand's
/// already does — a bare ident on a generic item is `E0107` before the example is ever read. A
/// lifetime and a const are not parameters a filling is chosen for, so neither reaches the list.
/// With nothing declared, every argument is the `String` fallback.
#[cfg(feature = "zod")]
#[test]
fn struct_schema_example_instantiates_every_type_parameter() {
    let name: syn::Ident = syn::parse_quote!(Report);
    let example = "Report { id: 1 }".to_owned();
    for (generics, expected) in [
        (syn::Generics::default(), "let value : Report ="),
        (syn::parse_quote!(<A>), "let value : Report < String > ="),
        (
            syn::parse_quote!(<A, B>),
            "let value : Report < String , String > =",
        ),
        (syn::parse_quote!(<'a>), "let value : Report ="),
    ] {
        let rendered = super::item_schema_example_method(
            Some(&example),
            &name,
            &generics,
            &super::ModelSchemaArgs::default(),
        )
        .unwrap()
        .to_string();
        assert!(rendered.contains(expected), "Got: {rendered}");
    }
}

/// Each argument is read off the `default_types` entry naming that parameter, so an item is
/// annotated at the concrete types its author declared and in the order the parameters were
/// written. A parameter no entry names keeps the `String` fallback, which is why a partly declared
/// item mixes the two.
#[cfg(feature = "zod")]
#[test]
fn struct_schema_example_instantiates_each_parameter_at_its_declared_filling() {
    let name: syn::Ident = syn::parse_quote!(Report);
    let example = "Report { id: 1 }".to_owned();
    let generics: syn::Generics = syn::parse_quote!(<A, B>);
    let count: (syn::Ident, syn::Type) = (syn::parse_quote!(A), syn::parse_quote!(u32));
    let held: (syn::Ident, syn::Type) = (syn::parse_quote!(B), syn::parse_quote!(Vec<u8>));
    for (default_types, expected) in [
        (vec![count.clone()], "let value : Report < u32 , String > ="),
        (
            vec![held.clone()],
            "let value : Report < String , Vec < u8 > > =",
        ),
        (
            vec![held, count],
            "let value : Report < u32 , Vec < u8 > > =",
        ),
    ] {
        let args = super::ModelSchemaArgs {
            default_types,
            ..Default::default()
        };
        let rendered = super::item_schema_example_method(Some(&example), &name, &generics, &args)
            .unwrap()
            .to_string();
        assert!(rendered.contains(expected), "Got: {rendered}");
    }
}

/// A filling written as `String` is exactly what an unfilled parameter falls back to, so an item
/// declaring one and an item declaring none are annotated with the same tokens — reading the
/// declaration leaves every item the old convention already got right byte for byte as it was.
#[cfg(feature = "zod")]
#[test]
fn a_string_filling_annotates_the_example_as_no_filling_does() {
    let name: syn::Ident = syn::parse_quote!(Report);
    let example = "Report { id: 1 }".to_owned();
    let generics: syn::Generics = syn::parse_quote!(<A>);
    let filled = super::ModelSchemaArgs {
        default_types: vec![(syn::parse_quote!(A), syn::parse_quote!(String))],
        ..Default::default()
    };
    let render = |args: &super::ModelSchemaArgs| {
        super::item_schema_example_method(Some(&example), &name, &generics, args)
            .unwrap()
            .to_string()
    };
    assert_eq!(render(&filled), render(&super::ModelSchemaArgs::default()));
}

#[cfg(feature = "jsonschema")]
#[test]
fn branded_json_schema_method_carries_no_cfg_attribute() {
    let args = super::ModelSchemaArgs::default();
    for inner in branded_json_inners() {
        let tokens = super::build_branded_json_schema_method(&args, &inner, "DocumentId");
        assert_no_cfg_attribute(&tokens, "build_branded_json_schema_method");
    }
}

/// Every shape [`super::branded_json_inner`] resolves to, so the dispatch is covered whole.
///
/// The `Slot` and `Chrono` shapes build their bodies through [`super::branded_slot_json_schema`]
/// and [`super::branded_chrono_schema`], which is where a stray `cfg` attribute would land unseen,
/// so they are walked here beside the two that render inline.
#[cfg(feature = "jsonschema")]
fn branded_json_inners() -> Vec<super::BrandedJsonInner> {
    let composite: syn::Type = syn::parse_quote!(Vec<String>);
    vec![
        #[cfg(feature = "chrono")]
        super::BrandedJsonInner::Chrono("date-time"),
        #[cfg(feature = "object_id")]
        super::BrandedJsonInner::ObjectId,
        super::BrandedJsonInner::Scalar("string".to_owned()),
        super::BrandedJsonInner::Slot(Box::new(super::get_field_def("_inner", &composite, ""))),
    ]
}

#[cfg(feature = "zod")]
#[test]
fn branded_schema_example_carries_no_cfg_attribute() {
    let name: syn::Ident = syn::parse_quote!(DocumentId);
    let example = "DocumentId(\"abc\".to_string())".to_owned();
    for generic_params in [
        Vec::new(),
        vec!["A".to_owned()],
        vec!["A".to_owned(), "B".to_owned()],
    ] {
        let tokens = super::build_branded_schema_example(
            Some(&example),
            &name,
            &generic_params,
            &super::ModelSchemaArgs::default(),
        );
        assert_no_cfg_attribute(&tokens, "build_branded_schema_example");
    }
}

/// The type the example is bound at carries one argument per declared parameter, and none at all
/// where the brand declares none — so a brand of any arity annotates a type it can be built as.
#[cfg(feature = "zod")]
#[test]
fn branded_schema_example_instantiates_every_parameter() {
    let name: syn::Ident = syn::parse_quote!(DocumentId);
    let example = "DocumentId(\"abc\".to_string())".to_owned();
    for (generic_params, expected) in [
        (Vec::new(), "let value : DocumentId ="),
        (vec!["A".to_owned()], "let value : DocumentId < String > ="),
        (
            vec!["A".to_owned(), "B".to_owned()],
            "let value : DocumentId < String , String > =",
        ),
    ] {
        let rendered = super::build_branded_schema_example(
            Some(&example),
            &name,
            &generic_params,
            &super::ModelSchemaArgs::default(),
        )
        .to_string();
        assert!(rendered.contains(expected), "Got: {rendered}");
    }
}

/// A brand reads its declaration through the same seam a declared struct does, so its example is
/// annotated at the fillings its author wrote rather than at the fallback.
#[cfg(feature = "zod")]
#[test]
fn branded_schema_example_instantiates_each_parameter_at_its_declared_filling() {
    let name: syn::Ident = syn::parse_quote!(DocumentId);
    let example = "DocumentId(\"abc\".to_string())".to_owned();
    let args = super::ModelSchemaArgs {
        default_types: vec![(syn::parse_quote!(A), syn::parse_quote!(u32))],
        ..Default::default()
    };
    let rendered = super::build_branded_schema_example(
        Some(&example),
        &name,
        &["A".to_owned(), "B".to_owned()],
        &args,
    )
    .to_string();
    assert!(
        rendered.contains("let value : DocumentId < u32 , String > ="),
        "Got: {rendered}"
    );
}

#[cfg(feature = "typescript")]
#[test]
fn plain_enum_ts_definition_carries_no_cfg_attribute() {
    let tokens = super::generate_plain_enum_ts_definition_method(
        " * Status",
        "Status",
        "Status",
        "",
        "  'a'",
    );
    assert_no_cfg_attribute(&tokens, "generate_plain_enum_ts_definition_method");
}

#[cfg(feature = "typescript")]
#[test]
fn discriminated_enum_ts_definition_carries_no_cfg_attribute() {
    let tokens = super::generate_discriminated_enum_ts_definition_method(
        " * Shape", "Shape", "Shape", "", "  'a'",
    );
    assert_no_cfg_attribute(&tokens, "generate_discriminated_enum_ts_definition_method");
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn alias_json_schema_method_carries_no_cfg_attribute() {
    let alias: syn::ItemType = syn::parse_quote!(
        pub type AliasIdent = String;
    );
    let field_def = super::get_field_def("AliasType", &alias.ty, "");
    let tokens = super::generate_alias_json_schema_method(&alias, "AliasType", &field_def);
    assert_no_cfg_attribute(&tokens, "generate_alias_json_schema_method");
}

/// An alias whose target the dispatch cannot render fails the way a field of that target does: one
/// diagnostic, naming the alias and the reason, in place of the whole body. A schema left there
/// would be carried by every slot the alias fills.
#[cfg(feature = "jsonschema")]
#[test]
fn an_alias_of_an_unrenderable_target_emits_only_the_compile_error() {
    for alias_source in [
        "pub type Rows = HashMap<String, (u32, u32)>;",
        "pub type Rows = Vec<HashMap<String, (u32, u32)>>;",
        "pub type Rows = (String, HashMap<String, (u32, u32)>);",
    ] {
        let alias: syn::ItemType = syn::parse_str(alias_source).unwrap();
        let field_def = super::get_field_def("RowsType", &alias.ty, "");
        let tokens =
            super::generate_alias_json_schema_method(&alias, "RowsType", &field_def).to_string();
        assert!(
            tokens.contains("compile_error !"),
            "for {alias_source}, got: {tokens}"
        );
        assert!(
            tokens.contains("type alias `RowsType`"),
            "for {alias_source}, got: {tokens}"
        );
        assert!(
            tokens.contains("a tuple is not supported as a map value"),
            "for {alias_source}, got: {tokens}"
        );
        assert!(
            tokens.contains("= compile_error !"),
            "the description is the diagnostic, not a schema: for {alias_source}, got: {tokens}"
        );
        assert!(
            !tokens.contains("\"type\""),
            "no schema for the rejected target is left beside the diagnostic: \
             for {alias_source}, got: {tokens}"
        );
    }
}

/// A type parameter reaches the mapping as a named type, and a name is carried by a reference to
/// the schema module it registered — a module no expansion emits for a parameter. So the parameter
/// is erased wherever it can be written, or the alias names a module that does not exist.
#[cfg(feature = "jsonschema")]
#[test]
fn an_alias_type_parameter_is_erased_at_every_depth() {
    for alias_source in [
        "pub type Holder<V> = V;",
        "pub type Holder<V> = Vec<V>;",
        "pub type Holder<V> = Option<V>;",
        "pub type Holder<V> = (String, V);",
        "pub type Holder<V> = HashMap<String, V>;",
        "pub type Holder<V> = HashMap<String, Vec<V>>;",
    ] {
        let alias: syn::ItemType = syn::parse_str(alias_source).unwrap();
        let field_def = super::get_field_def("HolderType", &alias.ty, "");
        let tokens =
            super::generate_alias_json_schema_method(&alias, "HolderType", &field_def).to_string();
        assert!(
            !tokens.contains("_schema :: Schema ::"),
            "for {alias_source}, got: {tokens}"
        );
    }
}

/// The same erasure at the same depths on the value surface, where the consequence of skipping it
/// is louder: a parameter left to render names a `$Schema` binding no emitted module declares, and
/// the pasted output throws before a payload is read. What it renders as instead is the argument
/// the alias's own factory binds for it, at whatever depth it was written. Asserted over the
/// identical alias list the JSON test walks, so the two surfaces cannot erase at different depths.
#[cfg(feature = "zod")]
#[test]
fn an_alias_type_parameter_is_erased_at_every_depth_on_the_value_surface() {
    for alias_source in [
        "pub type Holder<V> = V;",
        "pub type Holder<V> = Vec<V>;",
        "pub type Holder<V> = Option<V>;",
        "pub type Holder<V> = (String, V);",
        "pub type Holder<V> = HashMap<String, V>;",
        "pub type Holder<V> = HashMap<String, Vec<V>>;",
    ] {
        let alias: syn::ItemType = syn::parse_str(alias_source).unwrap();
        let field_def = super::get_field_def("HolderType", &alias.ty, "");
        let tokens = super::generate_alias_zod_method(&alias, "HolderType", "Holder", &field_def)
            .to_string();
        assert!(
            !tokens.contains("V$Schema"),
            "for {alias_source}, got: {tokens}"
        );
        assert!(
            tokens.contains("HolderType$SchemaFactory"),
            "for {alias_source}, got: {tokens}"
        );
    }
}

/// The stub this replaced answered every alias with an object carrying a lone `warning` key, which
/// under JSON Schema constrains nothing — every slot naming an alias accepted every payload. No
/// emission may carry one again.
#[test]
fn no_json_schema_emission_carries_a_warning_key() {
    for (file, source) in [
        ("model_schema.rs", include_str!("../model_schema.rs")),
        (
            "features/jsonschema.rs",
            include_str!("../features/jsonschema.rs"),
        ),
    ] {
        assert!(!source.contains("\"warning\""), "in: {file}");
    }
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn alias_zod_method_carries_no_cfg_attribute() {
    let alias: syn::ItemType = syn::parse_quote!(
        pub type Alias = String;
    );
    let ty: syn::Type = syn::parse_quote!(String);
    let field_def = super::get_field_def("AliasType", &ty, "");
    let tokens = super::generate_alias_zod_method(&alias, "AliasType", "Alias", &field_def);
    assert_no_cfg_attribute(&tokens, "generate_alias_zod_method");
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn alias_ts_definition_method_carries_no_cfg_attribute() {
    let alias: syn::ItemType = syn::parse_quote!(
        /// An aliased identifier.
        pub type AliasIdent = String;
    );
    let ty: syn::Type = syn::parse_quote!(String);
    let field_def = super::get_field_def("AliasType", &ty, "");
    let tokens = super::generate_alias_ts_definition_method(&alias, "AliasType", &field_def);
    assert_no_cfg_attribute(&tokens, "generate_alias_ts_definition_method");
}

/// Collects a branded newtype's guard failures as rendered `compile_error!` token strings.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn branded_errors(item: &syn::ItemStruct) -> Vec<String> {
    branded_errors_with(item, &super::ModelSchemaArgs::default())
}

/// [`branded_errors`] for a brand carrying `model_schema` arguments.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn branded_errors_with(item: &syn::ItemStruct, args: &super::ModelSchemaArgs) -> Vec<String> {
    branded_guard_errors(item, args)
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// The `model_schema` arguments of a brand carrying a lone `pattern`.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn pattern_args() -> super::ModelSchemaArgs {
    super::parse_model_schema_args(quote::quote! { pattern = "^[a-z]+$" })
}

/// A brand's guard failures for the given `pattern`, over a `String` inner that clears every other
/// guard.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn brand_pattern_errors(pattern: &str) -> Vec<String> {
    branded_errors_with(
        &syn::parse_quote! {
            #[serde(transparent)]
            struct UserId(pub String);
        },
        &super::parse_model_schema_args(quote::quote! { pattern = #pattern }),
    )
}

/// The brand splice reaches the same three surfaces the field splice does, so it answers to the
/// same parse.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn the_brand_pattern_guard_follows_the_regex_crate() {
    for pattern in PROBE_PATTERNS {
        let rejected = regex::Regex::new(pattern).is_err();
        let errors = brand_pattern_errors(pattern);
        assert_eq!(
            errors.len(),
            usize::from(rejected),
            "for {pattern}, got: {errors:?}"
        );
    }
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_brand_pattern_the_regex_crate_rejects_names_the_type_and_quotes_the_parse_error() {
    let errors = brand_pattern_errors(r"^ab\");
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    for needle in [
        "compile_error",
        "type `UserId`",
        "pattern",
        "regex parse error",
        "incomplete escape sequence",
    ] {
        assert!(
            errors[0].contains(needle),
            "{needle} missing: {}",
            errors[0]
        );
    }
}

/// The brand splices the same string into the Zod literal and the JSON Schema `pattern` that the
/// field splice does, so the portability verdict has to reach it too.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_brand_pattern_javascript_cannot_carry_names_the_type_and_the_construct() {
    for (pattern, construct) in UNPORTABLE_PROBE_PATTERNS {
        let errors = brand_pattern_errors(pattern);
        assert_eq!(errors.len(), 1, "for {pattern}, got: {errors:?}");
        for needle in ["compile_error", "type `UserId`", construct] {
            assert!(
                errors[0].contains(needle),
                "{needle} missing for {pattern}: {}",
                errors[0]
            );
        }
    }
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_brand_pattern_naming_a_group_the_rust_way_clears_the_guard() {
    let errors = brand_pattern_errors("^(?P<word>[a-z]+)$");
    assert!(errors.is_empty(), "got: {errors:?}");
}

/// The brand carries the same `pattern` to the same three surfaces a field does, so a pattern that
/// says nothing has to be refused here too — and it is the only string constraint the brand has,
/// so taking it would publish a `validate()` that turns nothing away.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_brand_pattern_admitting_every_value_names_the_type_and_says_so() {
    for pattern in ["", "^", "$", "|", "a*", "^a*"] {
        let errors = brand_pattern_errors(pattern);
        assert_eq!(errors.len(), 1, "for {pattern:?}, got: {errors:?}");
        for needle in [
            "compile_error",
            "type `UserId`",
            "admits every value",
            "constrains nothing",
        ] {
            assert!(
                errors[0].contains(needle),
                "{needle} missing for {pattern:?}: {}",
                errors[0]
            );
        }
    }
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_brand_pattern_that_still_constrains_clears_the_guard() {
    for pattern in ["^$", "^a*$", r"\b"] {
        let errors = brand_pattern_errors(pattern);
        assert!(errors.is_empty(), "for {pattern:?}, got: {errors:?}");
    }
}

/// The brand renders its inner into the brand rather than walking it as a field, so the map-key
/// guard has to be run here or the inner escapes it entirely — leaving TypeScript to write a
/// `Record` keyed by a type that supplies no keys. The brand earns the same diagnostic a field of
/// the inner's type earns, whichever reason the key has.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_brand_over_a_map_with_an_unwritable_key_is_refused() {
    register_alias_info("Doc", "Doc", "doc_schema", AliasKind::NoEnumMembers);
    for (inner, needle) in [
        (
            quote::quote! { HashMap<Doc, u32> },
            "a map key must be a plain",
        ),
        (quote::quote! { HashMap<Vec<Doc>, u32> }, "is a sequence of"),
        (
            quote::quote! { HashMap<(String, u32), u32> },
            "serde writes `(_, _)`",
        ),
        (quote::quote! { HashMap<Option<Doc>, u32> }, "Option<Doc>"),
        (
            quote::quote! { Vec<HashMap<Doc, u32>> },
            "a map key must be a plain",
        ),
    ] {
        let errors = branded_errors(&syn::parse_quote! {
            #[serde(transparent)]
            struct Wrap(pub #inner);
        });
        assert_eq!(errors.len(), 1, "for {inner}, got: {errors:?}");
        assert!(
            errors[0].contains("compile_error"),
            "for {inner}: {}",
            errors[0]
        );
        assert!(
            errors[0].contains("type `Wrap`"),
            "for {inner}: {}",
            errors[0]
        );
        assert!(errors[0].contains(needle), "for {inner}: {}", errors[0]);
    }
}

/// The guard is a filter here too: a brand over a map whose key can be written, and a brand over no
/// map at all, keep the brand they had.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_brand_over_a_writable_map_key_clears_the_guard() {
    register_alias_info("Slot", "Slot", "slot_schema", AliasKind::EnumMembers);
    for inner in [
        quote::quote! { String },
        quote::quote! { HashMap<String, u32> },
        quote::quote! { HashMap<Slot, u32> },
        quote::quote! { Vec<HashMap<String, u32>> },
    ] {
        let errors = branded_errors(&syn::parse_quote! {
            #[serde(transparent)]
            struct Wrap(pub #inner);
        });
        assert!(errors.is_empty(), "for {inner}, got: {errors:?}");
    }
}

/// Every constraint the guard reacts to, applied one at a time: `has_string_constraints` is an
/// or over three independent fields, so a guard wired to only one of them would still pass a
/// pattern-only probe.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn each_string_constraint_alone_rejects_a_numeric_inner() {
    for args in [
        quote::quote! { pattern = "^[a-z]+$" },
        quote::quote! { minLength = 3 },
        quote::quote! { maxLength = 8 },
    ] {
        let rendered = format!("{args}");
        let errors = branded_errors_with(
            &syn::parse_quote! {
                #[serde(transparent)]
                struct BadNum(pub u64);
            },
            &super::parse_model_schema_args(args),
        );
        assert_eq!(errors.len(), 1, "for {rendered}, got: {errors:?}");
        assert!(errors[0].contains("numeric"), "got: {}", errors[0]);
    }
}

/// The shapes whose surfaces read the string constraints as something other than a string check.
///
/// Every sequence spelling stands beside the `Vec` it writes the same array as. A wrapper name is
/// what the parser leaves for all but `Vec` and `[T; N]`, and reading it as a name rather than as
/// the array it writes is what let a set through: the JSON schema then dropped `minLength` outside
/// a string, while Zod read `.min` as a bound on how many items the array holds.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn string_constraints_over_a_non_string_inner_are_rejected() {
    for (inner, shape) in [
        ("u64", "numeric"),
        ("i32", "numeric"),
        ("f64", "numeric"),
        ("bool", "boolean"),
        ("Vec<String>", "container"),
        ("[u8; 4]", "container"),
        ("BTreeSet<String>", "container"),
        ("BinaryHeap<String>", "container"),
        ("HashSet<String>", "container"),
        ("LinkedList<String>", "container"),
        ("VecDeque<String>", "container"),
        ("HashMap<String, String>", "container"),
        ("(String, String)", "container"),
        ("serde_json::Value", "opaque"),
    ] {
        let ty: syn::Type = syn::parse_str(inner).unwrap();
        let errors = branded_errors_with(
            &syn::parse_quote! {
                #[serde(transparent)]
                struct Branded(pub #ty);
            },
            &pattern_args(),
        );
        assert_eq!(errors.len(), 1, "for {inner}, got: {errors:?}");
        assert!(errors[0].contains("compile_error"), "got: {}", errors[0]);
        assert!(errors[0].contains("`Branded`"), "got: {}", errors[0]);
        assert!(errors[0].contains(shape), "for {inner}, got: {}", errors[0]);
    }
}

/// The inners that carry the constraints faithfully. A `SiblingType` — another brand, or an
/// unresolved user type — is admitted because expansion cannot know its shape; the constrained
/// path's `Display` assertion is what covers it. A name carrying one argument that is not a
/// sequence wrapper is such a name too, and stays admitted.
///
/// `U` is written where the brand declares no such parameter, so it is one of those unresolved
/// names rather than a parameter: that is the whole of the line the classifier draws, and the
/// brand's own `T` is on the other side of it in the test below.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn string_constraints_over_a_string_shaped_inner_pass() {
    for inner in [
        "String",
        "PathBuf",
        "ObjectId",
        "SomeOtherBrand",
        "U",
        "SomeWrapper<String>",
    ] {
        let ty: syn::Type = syn::parse_str(inner).unwrap();
        let errors = branded_errors_with(
            &syn::parse_quote! {
                #[serde(transparent)]
                struct Branded<T>(pub #ty);
            },
            &pattern_args(),
        );
        assert!(errors.is_empty(), "for {inner}, got: {errors:?}");
    }
}

/// Registers a name carrying the value surface a `#[model_schema()]` item's own expansion would
/// have recorded for it, standing in for that expansion.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn seed_value_shape(rust_ident: &str, shape: Option<&'static str>) {
    seed_published_shape(rust_ident, PublishedShape::Flat(shape));
}

/// The same, for a name whose own target is one of its parameters and which therefore records a
/// position rather than a word.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn seed_published_shape(rust_ident: &str, shape: PublishedShape) {
    register_alias_info(
        rust_ident,
        rust_ident,
        &ident_schema_module_name(rust_ident),
        AliasKind::NoEnumMembers,
    );
    record_value_shape(rust_ident, shape);
}

/// A brand's guard failures for a `pattern` over the named inner, spelled as a bare name.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn brand_over_named_inner_errors(inner: &str) -> Vec<String> {
    let ty: syn::Type = syn::parse_str(inner).unwrap();
    branded_errors_with(
        &syn::parse_quote! {
            #[serde(transparent)]
            struct Branded(pub #ty);
        },
        &pattern_args(),
    )
}

/// A named inner is where the checks actually land — the brand emits `Inner$Schema.min(3)` — so a
/// name the registry says publishes something other than a string takes the refusal the same shape
/// spelled directly takes, and names both the brand and the inner.
///
/// Spelled directly, `serde_json::Value` is refused through the opaque arm; one module out it was
/// admitted, and `Blob$Schema.min(3)` over `const Blob$Schema = z.unknown().brand<"Blob">()` is a
/// `TypeError` at load, before a payload is read.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn string_constraints_over_a_named_inner_the_registry_answers_for_are_rejected() {
    for (inner, shape) in [
        ("OpaqueSibling", "opaque"),
        ("NumericSibling", "numeric"),
        ("BooleanSibling", "boolean"),
        ("ContainerSibling", "container"),
        ("ObjectSibling", "object"),
        ("EnumeratedSibling", "enumerated"),
        ("UnionSibling", "union"),
        ("NullableSibling", "nullable"),
    ] {
        seed_value_shape(inner, Some(shape));
        let errors = brand_over_named_inner_errors(inner);
        assert_eq!(errors.len(), 1, "for {inner}, got: {errors:?}");
        assert!(errors[0].contains("compile_error"), "got: {}", errors[0]);
        assert!(errors[0].contains("`Branded`"), "got: {}", errors[0]);
        assert!(errors[0].contains(inner), "got: {}", errors[0]);
        assert!(errors[0].contains(shape), "for {inner}, got: {}", errors[0]);
    }
}

/// A name the registry says publishes a string carries the checks, so it stays admitted — the
/// working case, unchanged.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn string_constraints_over_a_named_inner_the_registry_calls_a_string_pass() {
    seed_value_shape("StringSibling", None);
    let errors = brand_over_named_inner_errors("StringSibling");
    assert!(errors.is_empty(), "got: {errors:?}");
}

/// A name the registry has no answer for keeps the emission it has always had.
///
/// The two names that reach this are the same absence: one written above the item that registers
/// it, and one this crate never expands at all — an unresolved user type whose schema the author
/// supplies. Refusing on absence would refuse the second for the sake of the first, and would make
/// a diagnostic out of declaration order: moving a declaration would turn a compiling program into
/// a refused one without changing what it means. The `Display` assertion still bounds the Rust
/// surface either way.
///
/// Both of them name a type the declaration has already fixed, which is what the admission rests
/// on; a name written over one of the brand's own parameters has not, and is refused below.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn string_constraints_over_a_name_the_registry_cannot_answer_for_pass() {
    let bare = brand_over_named_inner_errors("SiblingRegisteredNowhere");
    assert!(bare.is_empty(), "got: {bare:?}");

    for inner in ["UnregisteredGeneric<String>", "UnregisteredGeneric<u32>"] {
        let ty: syn::Type = syn::parse_str(inner).unwrap();
        let carrying_a_fixed_argument = branded_errors_with(
            &syn::parse_quote! {
                #[serde(transparent)]
                struct Branded<T>(pub #ty);
            },
            &pattern_args(),
        );
        assert!(
            carrying_a_fixed_argument.is_empty(),
            "for {inner}, got: {carrying_a_fixed_argument:?}"
        );
    }
}

/// A name written over one of the brand's own type parameters is refused, wherever the parameter
/// sits inside it.
///
/// The registry's silence is not consent here. The brand composes the named type's schema from the
/// argument the caller supplies, so the checks land on whatever that argument turns out to be: Zod
/// appends them to a shape the call site decides, the one JSON document written for every
/// instantiation still holds the `{}` a parameter describes as, and `validate()` measures the
/// inner's `Display` — a numeric filling rejected for its digit count rather than for its value.
/// The same two items written in the other order already refuse through the registry, so admitting
/// this one puts the diagnostic back on declaration order the long way round.
///
/// The refusal names the brand, the inner and the parameter, so the author reads which declaration
/// to fix and which name in it.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn string_constraints_over_a_name_written_over_the_brands_own_parameter_are_rejected() {
    for inner in [
        "UnregisteredGeneric<T>",
        "UnregisteredGeneric<Vec<T>>",
        "UnregisteredGeneric<String, U>",
        "UnregisteredGeneric<HashMap<String, T>>",
        "UnregisteredGeneric<(u32, U)>",
    ] {
        let ty: syn::Type = syn::parse_str(inner).unwrap();
        let errors = branded_errors_with(
            &syn::parse_quote! {
                #[serde(transparent)]
                struct Branded<T, U>(pub #ty);
            },
            &pattern_args(),
        );
        assert_eq!(errors.len(), 1, "for {inner}, got: {errors:?}");
        assert!(errors[0].contains("compile_error"), "got: {}", errors[0]);
        assert!(errors[0].contains("`Branded`"), "got: {}", errors[0]);
        assert!(
            errors[0].contains("UnregisteredGeneric"),
            "for {inner}, got: {}",
            errors[0]
        );
        assert!(
            errors[0].contains("type parameter"),
            "for {inner}, got: {}",
            errors[0]
        );
    }
}

/// A name the registry calls a string publisher is refused too, once one of the brand's own
/// parameters is written into it.
///
/// That registration answers for the declaration, and the declaration here is one whose value the
/// filling supplies: `Later<T>` publishes a string for a `Later<String>` and something else for
/// every other filling, so the checks still land on whatever the call site handed over. The
/// registry's own arm cannot tell this case from an unregistered one — a string publisher records
/// no shape, exactly as an absence records none — which is why it is the parameter that decides.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn string_constraints_over_a_registered_name_carrying_the_brands_parameter_are_rejected() {
    seed_value_shape("RegisteredStringGeneric", None);
    let errors = branded_errors_with(
        &syn::parse_quote! {
            #[serde(transparent)]
            struct Branded<T>(pub RegisteredStringGeneric<T>);
        },
        &pattern_args(),
    );
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    assert!(
        errors[0].contains("RegisteredStringGeneric"),
        "got: {}",
        errors[0]
    );
}

/// What a brand records for the next brand written over it: whatever its own inner publishes, read
/// through the same call the guard reads it through — including one link through a name, which is
/// what carries a chain of brands to its end.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_brand_records_the_value_surface_its_inner_publishes() {
    seed_value_shape("RecordedOpaqueSibling", Some("opaque"));
    seed_value_shape("RecordedStringSibling", None);
    for (inner, expected) in [
        ("String", None),
        ("PathBuf", None),
        ("serde_json::Value", Some("opaque")),
        ("u32", Some("numeric")),
        ("bool", Some("boolean")),
        ("Vec<String>", Some("container")),
        ("(String, String)", Some("container")),
        ("RecordedOpaqueSibling", Some("opaque")),
        ("RecordedStringSibling", None),
        ("SiblingRegisteredNowhere", None),
    ] {
        let ty: syn::Type = syn::parse_str(inner).unwrap();
        let item: syn::ItemStruct = syn::parse_quote! {
            #[serde(transparent)]
            struct Branded(pub #ty);
        };
        assert_eq!(
            brand_surface(&item).shape,
            PublishedShape::Flat(expected),
            "for {inner}"
        );
    }
}

/// A brand whose inner *is* one of its own parameters records that parameter's position, not a
/// word: what it publishes is settled by the argument a reference writes, and no word available at
/// the declaration says that. A parameter reached under a wrapper keeps the wrapper's own shape,
/// which no filling changes.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_brand_over_its_own_parameter_records_the_position_it_publishes() {
    for (inner, expected) in [
        ("TagType", PublishedShape::Parameter(0)),
        ("ValueType", PublishedShape::Parameter(1)),
        ("Vec<TagType>", PublishedShape::Flat(Some("container"))),
        (
            "HashMap<String, ValueType>",
            PublishedShape::Flat(Some("container")),
        ),
        (
            "(TagType, ValueType)",
            PublishedShape::Flat(Some("container")),
        ),
        ("Option<TagType>", PublishedShape::Flat(Some("nullable"))),
        ("String", PublishedShape::Flat(None)),
    ] {
        let ty: syn::Type = syn::parse_str(inner).unwrap();
        let item: syn::ItemStruct = syn::parse_quote! {
            #[serde(transparent)]
            struct Branded<TagType, ValueType>(pub #ty);
        };
        assert_eq!(brand_surface(&item).shape, expected, "for {inner}");
    }
}

/// The surface a brand's own registration records, built the way `register_branded_newtype` builds
/// it.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn brand_surface(item: &syn::ItemStruct) -> super::Surface {
    super::Surface::written(
        &super::branded_inner_value_surface(&item.generics, item.fields.iter().next().unwrap()),
        &super::type_parameters_in_scope(&item.generics),
    )
}

/// A recorded position is filled with the argument the reference writes, so one declaration answers
/// per instantiation: the checks compose onto that argument's schema, and the guard names the shape
/// the argument resolves to rather than the opaque one the declaration alone could say.
///
/// The same answer whichever way the two declarations are written — the record is the same record —
/// which is what takes the guard's verdict off declaration order for every reference that reaches
/// it at all.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn string_constraints_over_a_parameter_publisher_read_the_argument_written_for_it() {
    seed_published_shape("PublishesItsParameter", PublishedShape::Parameter(0));
    let admitted = brand_over_named_inner_errors("PublishesItsParameter<String>");
    assert!(admitted.is_empty(), "got: {admitted:?}");

    for (inner, shape) in [
        ("PublishesItsParameter<u32>", "numeric"),
        ("PublishesItsParameter<bool>", "boolean"),
        ("PublishesItsParameter<Vec<String>>", "container"),
        ("PublishesItsParameter<serde_json::Value>", "opaque"),
    ] {
        let errors = brand_over_named_inner_errors(inner);
        assert_eq!(errors.len(), 1, "for {inner}, got: {errors:?}");
        assert!(errors[0].contains("`Branded`"), "got: {}", errors[0]);
        assert!(
            errors[0].contains("PublishesItsParameter"),
            "for {inner}, got: {}",
            errors[0]
        );
        assert!(errors[0].contains(shape), "for {inner}, got: {}", errors[0]);
    }
}

/// A position the reference writes no argument at, and one a second parameter is published at, are
/// both read off the same list the declaration numbered.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_recorded_position_is_read_off_the_arguments_the_reference_writes() {
    seed_published_shape("PublishesItsSecond", PublishedShape::Parameter(1));
    let errors = brand_over_named_inner_errors("PublishesItsSecond<String, u32>");
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    assert!(errors[0].contains("numeric"), "got: {}", errors[0]);

    let unwritten = brand_over_named_inner_errors("PublishesItsSecond<String>");
    assert!(unwritten.is_empty(), "got: {unwritten:?}");
}

/// What a tuple struct records: serde writes one slot as that slot's value alone, so the schema is
/// the slot's and carries what the slot carries; every other arity is the fixed array `z.tuple`
/// writes, which takes no string check. An optional slot is `z.nullable(...)`, which takes none
/// either.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_tuple_struct_records_the_value_surface_its_slots_publish() {
    for (decl, expected) in [
        ("struct Slots(pub String);", PublishedShape::Flat(None)),
        (
            "struct Slots(pub Option<String>);",
            PublishedShape::Flat(Some("nullable")),
        ),
        (
            "struct Slots(pub u32);",
            PublishedShape::Flat(Some("numeric")),
        ),
        (
            "struct Slots(pub Vec<String>);",
            PublishedShape::Flat(Some("container")),
        ),
        (
            "struct Slots(pub String, pub u32);",
            PublishedShape::Flat(Some("container")),
        ),
        ("struct Slots();", PublishedShape::Flat(Some("container"))),
        ("struct Slots<T>(pub T);", PublishedShape::Parameter(0)),
        (
            "struct Slots<T>(pub Vec<T>);",
            PublishedShape::Flat(Some("container")),
        ),
    ] {
        let item: syn::ItemStruct = syn::parse_str(decl).unwrap();
        assert_eq!(
            super::tuple_struct_surface(
                &item.fields,
                &super::type_parameters_in_scope(&item.generics)
            )
            .shape,
            expected,
            "for {decl}"
        );
    }
}

/// A brand constraining one of its own type parameters has nothing to hang the checks on.
///
/// Both validating surfaces read a parameter as the opaque value, and an opaque value takes no
/// string checks: Zod 4's `z.unknown()` carries no `.min`/`.max`, and `.brand()` hands back that
/// same instance rather than a wrapper that could; JSON Schema's string keywords go inert beside
/// the `{}` a parameter describes as; and `validate()` still measures `Display`. So the parameter
/// reaches the same refusal `serde_json::Value` reaches, through the same opaque arm — the erasure
/// is what puts it there, and is why the guard does not have to name parameters itself.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn string_constraints_over_the_brands_own_type_parameter_are_rejected() {
    for inner in ["T", "U"] {
        let ty: syn::Type = syn::parse_str(inner).unwrap();
        let errors = branded_errors_with(
            &syn::parse_quote! {
                #[serde(transparent)]
                struct Branded<T, U>(pub #ty);
            },
            &pattern_args(),
        );
        assert_eq!(errors.len(), 1, "for {inner}, got: {errors:?}");
        assert!(errors[0].contains("`Branded`"), "got: {}", errors[0]);
        assert!(
            errors[0].contains("opaque"),
            "for {inner}, got: {}",
            errors[0]
        );
    }
}

/// The guard reads the constraints, not the inner type: an unconstrained brand over any of the
/// rejected shapes is the shipped `no_display` contract and stays accepted — a sequence wrapper
/// included, which describes the array it writes on every surface and needs no refusal.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn an_unconstrained_brand_over_a_non_string_inner_passes() {
    for inner in [
        "u64",
        "bool",
        "Vec<String>",
        "BTreeSet<String>",
        "VecDeque<String>",
        "serde_json::Value",
    ] {
        let ty: syn::Type = syn::parse_str(inner).unwrap();
        let errors = branded_errors(&syn::parse_quote! {
            #[serde(transparent)]
            struct Branded(pub #ty);
        });
        assert!(errors.is_empty(), "for {inner}, got: {errors:?}");
    }
}

/// The message has to name the surfaces that disagree, or it reads as an arbitrary restriction.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn the_constraint_guard_message_names_the_constraints_and_the_surfaces() {
    let errors = branded_errors_with(
        &syn::parse_quote! {
            #[serde(transparent)]
            struct BadNum(pub u64);
        },
        &pattern_args(),
    );
    for needle in ["pattern", "minLength", "maxLength", "Zod", "JSON Schema"] {
        assert!(
            errors[0].contains(needle),
            "{needle} missing: {}",
            errors[0]
        );
    }
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn cfg_attr_wrapped_serde_on_a_branded_type_is_rejected() {
    let errors = branded_errors(&syn::parse_quote! {
        #[serde(transparent)]
        #[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
        struct UserId(pub String);
    });
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    assert!(errors[0].contains("compile_error"), "got: {}", errors[0]);
    assert!(errors[0].contains("type `UserId`"), "got: {}", errors[0]);
    assert!(errors[0].contains("cfg_attr"), "got: {}", errors[0]);
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn cfg_attr_wrapped_serde_on_a_branded_inner_slot_is_rejected() {
    let errors = branded_errors(&syn::parse_quote! {
        #[serde(transparent)]
        struct UserId(#[cfg_attr(feature = "serde", serde(rename = "inner"))] pub String);
    });
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    assert!(errors[0].contains("compile_error"), "got: {}", errors[0]);
    assert!(errors[0].contains("tuple field"), "got: {}", errors[0]);
    assert!(errors[0].contains("#[serde(...)]"), "got: {}", errors[0]);
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn branded_newtype_over_option_is_rejected() {
    let errors = branded_errors(&syn::parse_quote! {
        #[serde(transparent)]
        struct MaybeUserId(pub Option<String>);
    });
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    assert!(errors[0].contains("compile_error"), "got: {}", errors[0]);
    assert!(errors[0].contains("Option"), "got: {}", errors[0]);
    assert!(errors[0].contains("null"), "got: {}", errors[0]);
}

/// An inner naming a type parameter is read exactly as a concrete one is, so the `Option`
/// collapses onto what it holds there too and the shape is no more representable than in the
/// concrete case.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn generic_branded_newtype_over_option_is_rejected() {
    let errors = branded_errors(&syn::parse_quote! {
        #[serde(transparent)]
        struct MaybeId<T>(pub Option<T>);
    });
    assert_eq!(errors.len(), 1, "got: {errors:?}");
    assert!(errors[0].contains("Option"), "got: {}", errors[0]);
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn compliant_branded_newtype_passes_every_guard() {
    let errors = branded_errors(&syn::parse_quote! {
        #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
        #[serde(transparent)]
        struct UserId(#[cfg_attr(feature = "serde", doc = "documented in serde builds")] pub String);
    });
    assert!(errors.is_empty(), "got: {errors:?}");
}

#[test]
fn no_display_is_accepted_as_a_bare_flag_and_as_a_named_bool() {
    assert!(super::parse_model_schema_args(quote::quote! { no_display }).no_display);
    assert!(super::parse_model_schema_args(quote::quote! { no_display = true }).no_display);
    assert!(!super::parse_model_schema_args(quote::quote! { no_display = false }).no_display);
    assert!(!super::parse_model_schema_args(proc_macro2::TokenStream::new()).no_display);
}

/// The bare flag shares the argument list with the `key = value` args, so parsing it must not
/// cost the others: a parse failure here silently drops every argument.
#[test]
fn no_display_coexists_with_the_named_args() {
    let args = super::parse_model_schema_args(quote::quote! {
        name = "Slug", pattern = "^[a-z]+$", minLength = 1, maxLength = 8, no_display
    });
    assert_eq!(args.name_override.as_deref(), Some("Slug"));
    assert_eq!(args.pattern.as_deref(), Some("^[a-z]+$"));
    assert_eq!(args.min_length, Some(1));
    assert_eq!(args.max_length, Some(8));
    assert!(args.no_display);
}

/// The parser's refusal of `args`, rendered, or `None` when it read them whole.
fn args_rejection(args: proc_macro2::TokenStream) -> Option<String> {
    super::parse_model_schema_args(args)
        .arg_rejection
        .as_ref()
        .map(ToString::to_string)
}

/// The reported repro: a misspelled `name` compiled clean and emitted the unrenamed schema. The
/// refusal names the argument as written and the one that was meant.
#[test]
fn a_misspelled_name_argument_is_refused_by_the_name_as_written() {
    let rejection = args_rejection(quote::quote! { nme = "Renamed" }).unwrap();
    assert!(rejection.contains("nme"), "got: {rejection}");
    assert!(rejection.contains("name"), "got: {rejection}");
}

/// A name the item paths splice into `Ident::new` — the schema module every reference to the item
/// resolves through — so one no identifier can be spelled from panicked the macro, reporting no
/// span and naming no argument.
#[test]
fn a_name_no_identifier_can_be_spelled_from_is_refused() {
    for value in [
        "",
        " ",
        "Not Valid",
        "9Leading",
        "Foo$Bar",
        "Foo::Bar",
        "\u{dc}n\u{ef}code",
    ] {
        let rejection = args_rejection(quote::quote! { name = #value });
        assert!(rejection.is_some(), "accepted `name = {value:?}`");
    }
}

#[test]
fn a_name_an_identifier_can_be_spelled_from_is_read() {
    for value in ["Slug", "_Slug", "Slug2", "slug_case", "S"] {
        let args = super::parse_model_schema_args(quote::quote! { name = #value });
        assert_eq!(
            args.arg_rejection.map(|e| e.to_string()),
            None,
            "for {value}"
        );
        assert_eq!(args.name_override.as_deref(), Some(value));
    }
}

/// An item with no docs falls back to the name it is exported under on both surfaces, and the
/// description surface escapes a double quote where the `JSDoc` one leaves it alone — so the two
/// fallbacks can only be spelled from one body while no exported name can carry that character.
/// The name has exactly two sources and neither can produce one: an override is refused unless it
/// spells an identifier, and an unrenamed item takes the Rust ident, which the grammar refuses to
/// tokenize with a quote in it — raw spelling included, which reaches the name as `r#`.
#[test]
fn an_exported_name_can_carry_no_double_quote() {
    for value in ["Weird\"Name", "\"", "\"Quoted\""] {
        assert!(
            args_rejection(quote::quote! { name = #value }).is_some(),
            "accepted `name = {value:?}`"
        );
        assert!(
            syn::parse_str::<syn::Ident>(value).is_err(),
            "tokenized {value:?} as an ident"
        );
    }
    assert_eq!(
        syn::Ident::new_raw("type", proc_macro2::Span::call_site()).to_string(),
        "r#type"
    );
    // The unrenamed path takes the ident whole but for a `Json` suffix, which drops characters and
    // adds none; the renamed path takes the refused-unless-identifier override verbatim.
    assert_eq!(
        super::compute_item_export_name("PayloadJson", None),
        "Payload"
    );
    assert_eq!(
        super::compute_item_export_name("Payload", Some("Renamed")),
        "Renamed"
    );
}

/// The refusal offers every argument the parser reads, and the probes below prove each offered
/// name is one it actually reads — the list and the arms cannot drift apart while both hold.
#[test]
fn no_argument_the_parser_reads_is_rejected() {
    let probes: [proc_macro2::TokenStream; 6] = [
        quote::quote! { name = "Renamed" },
        quote::quote! { pattern = "^[a-z]+$" },
        quote::quote! { minLength = 1 },
        quote::quote! { maxLength = 50 },
        quote::quote! { no_display },
        quote::quote! { default_types(IdType = String) },
    ];
    assert_eq!(probes.len(), super::KNOWN_ARGS.len());

    let offered = args_rejection(quote::quote! { bogus_flag }).unwrap();
    for name in super::KNOWN_ARGS {
        assert!(offered.contains(name), "{name} not offered: {offered}");
    }
    for probe in probes {
        let rendered = probe.to_string();
        assert_eq!(args_rejection(probe), None, "for {rendered}");
    }
}

/// Every shape the old parser dropped on the floor: a wrong literal kind, a value that is no
/// literal at all, a length the target type cannot hold, a known argument written as a list or as
/// a bare flag, and a bare path the parser does not read.
#[test]
fn a_shape_the_parser_cannot_read_is_refused() {
    let probes: [proc_macro2::TokenStream; 9] = [
        quote::quote! { name = 3 },
        quote::quote! { name("Nested") },
        quote::quote! { name },
        quote::quote! { pattern = 3 },
        quote::quote! { minLength = "3" },
        quote::quote! { minLength = -1 },
        quote::quote! { maxLength = 99999999999999999999999999999999999999999 },
        quote::quote! { no_display = 3 },
        quote::quote! { bogus_flag },
    ];
    for probe in probes {
        let rendered = probe.to_string();
        assert!(args_rejection(probe).is_some(), "for {rendered}");
    }
}

/// An argument list `syn` itself cannot parse took the whole list down with it, silently.
#[test]
fn an_unparseable_argument_list_is_refused() {
    let rejection = args_rejection(quote::quote! { name = }).unwrap();
    assert!(!rejection.is_empty());
}

/// An argument the parser reads before the refused one still lands: the refusal reports the
/// attribute, it does not discard what was already read.
#[test]
fn a_refusal_keeps_what_the_parser_had_already_read() {
    let args = super::parse_model_schema_args(quote::quote! { name = "Slug", nme = "Renamed" });
    assert_eq!(args.name_override.as_deref(), Some("Slug"));
    assert!(args.arg_rejection.is_some());
}

/// The refusal reaches the expansion as a `compile_error!` naming the type it was written on.
#[test]
fn a_refused_argument_reaches_the_expansion_as_a_named_compile_error() {
    let rejection = super::parse_model_schema_args(quote::quote! { nme = "Renamed" })
        .arg_rejection
        .unwrap();
    let item: syn::Item = syn::parse_quote! {
        struct TypeLevelUnknown {
            name: String,
        }
    };
    let error = super::attr_guard_error(&rejection, &super::item_label(&item)).to_string();
    for needle in ["compile_error", "type `TypeLevelUnknown`", "nme"] {
        assert!(error.contains(needle), "{needle} missing: {error}");
    }
}

/// The three shapes `model_schema` expands name themselves; anything else has no ident to name.
#[test]
fn every_expanded_shape_names_itself_in_a_guard_message() {
    for (item, label) in [
        (
            syn::parse_quote! { struct Carrier { name: String } },
            "type `Carrier`",
        ),
        (syn::parse_quote! { enum Carrier { One } }, "type `Carrier`"),
        (
            syn::parse_quote! { type Carrier = String; },
            "type `Carrier`",
        ),
        (syn::parse_quote! { fn carrier() {} }, "item"),
    ] {
        assert_eq!(super::item_label(&item), label);
    }
}

/// `default_types(IdType = String, DateType = f64)` reads as the pairs it was written as, in the
/// order they were written, each type kept whole — the order is what a reader of the declaration
/// lines up against the parameter list.
#[test]
fn default_types_reads_its_pairs_in_declaration_order() {
    let args = super::parse_model_schema_args(quote::quote! {
        default_types(IdType = String, DateType = f64, Nested = Vec<Option<u8>>)
    });
    assert_eq!(args.arg_rejection.as_ref().map(ToString::to_string), None);
    let read: Vec<(String, String)> = args
        .default_types
        .iter()
        .map(|(name, ty)| (name.to_string(), quote::quote!(#ty).to_string()))
        .collect();
    assert_eq!(
        read,
        vec![
            ("IdType".to_owned(), "String".to_owned()),
            ("DateType".to_owned(), "f64".to_owned()),
            ("Nested".to_owned(), "Vec < Option < u8 > >".to_owned()),
        ]
    );
}

/// Every shape the argument is not: a value where the list belongs, a bare flag, an entry with no
/// type beside it, a name no parameter can be spelled as, a trailing hole, and a list that
/// declares nothing at all.
#[test]
fn a_default_types_shape_the_parser_cannot_read_is_refused() {
    let probes: [proc_macro2::TokenStream; 6] = [
        quote::quote! { default_types = "IdType" },
        quote::quote! { default_types },
        quote::quote! { default_types() },
        quote::quote! { default_types(IdType) },
        quote::quote! { default_types("IdType" = String) },
        quote::quote! { default_types(IdType = String, , DateType = f64) },
    ];
    for probe in probes {
        let rendered = probe.to_string();
        assert!(args_rejection(probe).is_some(), "for {rendered}");
    }
}

/// A parameter named twice is refused as written, and the refusal names the parameter and both
/// fillings — a reader shown only that there is a duplicate still has to go and find the other
/// entry to know what was dropped.
#[test]
fn a_parameter_declared_twice_is_refused() {
    let rejection = args_rejection(quote::quote! {
        default_types(IdType = String, IdType = f64)
    })
    .unwrap();
    for needle in ["default_types", "IdType", "String", "f64"] {
        assert!(rejection.contains(needle), "{needle} missing: {rejection}");
    }
}

/// The duplicate is refused whatever surrounds it: the second of two identical entries says no
/// more than the second of two different ones, and a repeat past the first pair is still a repeat.
#[test]
fn a_repeated_parameter_is_refused_wherever_it_is_written() {
    let probes: [proc_macro2::TokenStream; 3] = [
        quote::quote! { default_types(IdType = String, IdType = String) },
        quote::quote! { default_types(IdType = String, DateType = f64, IdType = u8) },
        quote::quote! { default_types(IdType = String, DateType = f64, DateType = f64) },
    ];
    for probe in probes {
        let rendered = probe.to_string();
        assert!(args_rejection(probe).is_some(), "for {rendered}");
    }
}

/// The refusal points at the entry that earned it — the second spelling of the name, not the first,
/// which on its own declares exactly what the author meant.
#[test]
fn a_duplicate_entry_refusal_is_spanned_on_the_second_spelling() {
    let source = "default_types(IdType = String, DateType = f64, IdType = u8)";
    let args: proc_macro2::TokenStream = syn::parse_str(source).unwrap();
    let rejection = super::parse_model_schema_args(args).arg_rejection.unwrap();
    let span = rejection.span();
    assert_eq!(span.source_text().as_deref(), Some("IdType"));
    assert_eq!(span.start().column, source.rfind("IdType").unwrap());
    assert_ne!(span.start().column, source.find("IdType").unwrap());
}

/// A list that names each parameter once is read exactly as before, however many entries it
/// carries and whatever the fillings are — the duplicate check costs a distinct declaration
/// nothing.
#[test]
fn distinct_entries_are_read_unchanged() {
    let args = super::parse_model_schema_args(quote::quote! {
        default_types(AType = u32, BType = String, CType = u32, DType = Vec<Option<u8>>)
    });
    assert_eq!(args.arg_rejection.as_ref().map(ToString::to_string), None);
    let read: Vec<(String, String)> = args
        .default_types
        .iter()
        .map(|(name, ty)| (name.to_string(), quote::quote!(#ty).to_string()))
        .collect();
    assert_eq!(
        read,
        vec![
            ("AType".to_owned(), "u32".to_owned()),
            ("BType".to_owned(), "String".to_owned()),
            ("CType".to_owned(), "u32".to_owned()),
            ("DType".to_owned(), "Vec < Option < u8 > >".to_owned()),
        ]
    );
}

/// The argument shares the list with the string constraints and the name override, and reading it
/// costs none of them. This is the only place the coexistence can be asked: a type-level string
/// constraint is a brand's alone, and a brand over a type parameter is already refused at its
/// inner field, so no one item reaches an emitter carrying both.
#[test]
fn default_types_coexists_with_every_other_argument() {
    let args = super::parse_model_schema_args(quote::quote! {
        name = "Slug", pattern = "^[a-z]+$", minLength = 1, maxLength = 8, no_display,
        default_types(IdType = String)
    });
    assert_eq!(args.arg_rejection.as_ref().map(ToString::to_string), None);
    assert_eq!(args.name_override.as_deref(), Some("Slug"));
    assert_eq!(args.pattern.as_deref(), Some("^[a-z]+$"));
    assert_eq!(args.min_length, Some(1));
    assert_eq!(args.max_length, Some(8));
    assert!(args.no_display);
    assert_eq!(args.default_types.len(), 1);
}

/// The `compile_error!` tokens `default_types` earns when `args` is written on `source`. Both are
/// parsed from text so their tokens carry file locations and each refusal's span can be read back
/// as the source it points at.
fn default_types_refusals(source: &str, args: &str) -> Vec<proc_macro2::TokenStream> {
    let item: syn::Item = syn::parse_str(source).unwrap();
    let parsed = super::parse_model_schema_args(syn::parse_str(args).unwrap());
    assert_eq!(
        parsed.arg_rejection.as_ref().map(ToString::to_string),
        None,
        "for {args}"
    );
    super::default_types_guard_errors(&item, &parsed)
}

/// The refusals of `args` on `source`, rendered.
fn default_types_messages(source: &str, args: &str) -> Vec<String> {
    default_types_refusals(source, args)
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// An entry naming nothing the item declares fills nothing, so it is refused in every build: no
/// surface reads the default it carries, and the parameter it was meant for is left without one.
#[test]
fn an_entry_naming_no_declared_parameter_is_refused_in_every_build() {
    let messages = default_types_messages(
        "pub struct Renamed<IdType, DateType> { pub id: IdType, pub at: DateType }",
        "default_types(IdType = String, DateType = f64, WrongName = String)",
    );
    assert_eq!(messages.len(), 1, "got: {messages:?}");
    for needle in [
        "compile_error",
        "WrongName",
        "IdType",
        "DateType",
        "type `Renamed`",
    ] {
        assert!(
            messages[0].contains(needle),
            "{needle} missing: {}",
            messages[0]
        );
    }
}

/// An item with no type parameter has nothing for a default to fill, whatever the entry names and
/// whichever shape carries the attribute — a lifetime and a const name no type either.
#[test]
fn default_types_on_an_item_declaring_no_type_parameter_is_refused() {
    for source in [
        "pub struct Plain { pub id: String }",
        "pub enum Plain { One }",
        "pub type Plain = String;",
        "pub struct Borrowed<'label> { pub id: &'label str }",
        "pub struct Fixed<const WIDTH: usize> { pub id: [u8; WIDTH] }",
    ] {
        let messages = default_types_messages(source, "default_types(IdType = String)");
        assert_eq!(messages.len(), 1, "for {source}: {messages:?}");
        assert!(
            messages[0].contains("IdType"),
            "for {source}: {}",
            messages[0]
        );
    }
}

/// A declaration that answers for every parameter earns nothing, in every shape the attribute
/// expands and beside the lifetimes and consts that name no type.
#[test]
fn an_item_declaring_a_default_for_every_parameter_earns_no_refusal() {
    for (source, args) in [
        (
            "pub struct Both<IdType, DateType> { pub id: IdType, pub at: DateType }",
            "default_types(IdType = String, DateType = f64)",
        ),
        (
            "pub enum Tagged<IdType> { Named { id: IdType } }",
            "default_types(IdType = String)",
        ),
        (
            "pub type Boxed<ValueType> = Vec<ValueType>;",
            "default_types(ValueType = String)",
        ),
        (
            "pub struct Mixed<'label, IdType, const WIDTH: usize> { pub id: IdType }",
            "default_types(IdType = String)",
        ),
        ("pub struct Plain { pub id: String }", ""),
    ] {
        let messages = default_types_messages(source, args);
        assert!(messages.is_empty(), "for {source}: {messages:?}");
    }
}

/// The refusal points at what earned it: the entry, for a name the item does not declare, and the
/// parameter itself, for one left with no default.
#[test]
fn a_default_types_refusal_is_spanned_on_what_earned_it() {
    let entry = default_types_refusals(
        "pub struct Renamed<IdType> { pub id: IdType }",
        "default_types(IdType = String, WrongName = String)",
    );
    assert_eq!(entry.len(), 1, "got: {}", entry.len());
    assert_eq!(entry[0].span().source_text().as_deref(), Some("WrongName"));

    #[cfg(feature = "jsonschema")]
    {
        let parameter = default_types_refusals(
            "pub struct EcmDocument<IdType, DateType> { pub id: IdType, pub at: DateType }",
            "default_types(IdType = String)",
        );
        assert_eq!(parameter.len(), 1, "got: {}", parameter.len());
        assert_eq!(
            parameter[0].span().source_text().as_deref(),
            Some("DateType")
        );
    }
}

/// The JSON document is built from the declared default, so a parameter left without one is
/// refused wherever that document is written — and the refusal says what the default is for, that
/// the feature is what requires it, and the attribute to write for this item's own parameters.
#[cfg(feature = "jsonschema")]
#[test]
fn a_parameter_with_no_default_is_refused_where_the_json_document_is_built() {
    let messages = default_types_messages(
        "pub struct EcmDocument<IdType, DateType> { pub id: IdType, pub at: DateType }",
        "",
    );
    assert_eq!(messages.len(), 2, "got: {messages:?}");
    let joined = messages.join("\n");
    for needle in [
        "IdType",
        "DateType",
        "silently rejects valid payloads",
        "`jsonschema` feature",
        "default_types(IdType = String, DateType = String)",
        "type `EcmDocument`",
    ] {
        assert!(joined.contains(needle), "{needle} missing: {joined}");
    }
}

/// Without the feature that reads it, nothing is generated from a default type, so an item that
/// declares none is left alone — the same item that is refused above.
#[cfg(not(feature = "jsonschema"))]
#[test]
fn a_parameter_with_no_default_is_accepted_where_no_json_document_is_built() {
    for args in ["", "default_types(IdType = String)"] {
        let messages = default_types_messages(
            "pub struct EcmDocument<IdType, DateType> { pub id: IdType, pub at: DateType }",
            args,
        );
        assert!(messages.is_empty(), "for {args:?}: {messages:?}");
    }
}

/// The bound checks `args` earns on `source`. Both are parsed from text so the emitted tokens carry
/// file locations and the span each check hands the compiler can be read back as the source it was
/// written as.
fn filling_bound_checks(source: &str, args: &str) -> Vec<proc_macro2::TokenStream> {
    let item: syn::Item = syn::parse_str(source).unwrap();
    let parsed = super::parse_model_schema_args(syn::parse_str(args).unwrap());
    assert_eq!(
        parsed.arg_rejection.as_ref().map(ToString::to_string),
        None,
        "for {args}"
    );
    super::default_types_bound_checks(&item, &parsed)
}

/// The checks `args` earns on `source`, rendered.
fn filling_bound_check_text(source: &str, args: &str) -> Vec<String> {
    filling_bound_checks(source, args)
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// Whether any token of `tokens`, at any depth, was written as `written` in the source it was
/// parsed from. A token the expansion synthesised was written nowhere and answers `None`.
fn some_token_was_written_as(tokens: &proc_macro2::TokenStream, written: &str) -> bool {
    tokens.clone().into_iter().any(|tree| match tree {
        proc_macro2::TokenTree::Group(group) => some_token_was_written_as(&group.stream(), written),
        leaf @ (proc_macro2::TokenTree::Ident(_)
        | proc_macro2::TokenTree::Punct(_)
        | proc_macro2::TokenTree::Literal(_)) => {
            leaf.span().source_text().as_deref() == Some(written)
        }
    })
}

/// Whether a filling satisfies the bounds its parameter declares is a question about trait impls,
/// which the macro cannot answer — so it hands the filling to a function carrying those bounds and
/// lets the compiler answer. The parameter keeps the ident it was declared under, so a bound
/// written in terms of it still reads.
#[test]
fn a_bounded_parameters_filling_is_handed_to_a_function_carrying_that_bound() {
    let checks = filling_bound_check_text(
        "pub struct Counted<CountType: Copy> { pub count: CountType }",
        "default_types(CountType = String)",
    );
    assert_eq!(checks.len(), 1, "got: {checks:?}");
    for needle in [
        "fn default_type_filling < CountType : Copy > ()",
        "default_type_filling :: < String > ()",
    ] {
        assert!(
            checks[0].contains(needle),
            "{needle} missing: {}",
            checks[0]
        );
    }
    assert!(
        !checks[0].contains("compile_error"),
        "the compiler answers this, not the macro: {}",
        checks[0]
    );
}

/// The bound and the filling keep the spans they were written at, so the compiler points at the
/// entry that earned the refusal and at the declaration that required it — neither at anything the
/// expansion synthesised.
#[test]
fn a_bound_check_carries_the_spans_the_entry_and_the_bound_were_written_at() {
    let checks = filling_bound_checks(
        "pub struct Counted<CountType: Copy> { pub count: CountType }",
        "default_types(CountType = String)",
    );
    assert_eq!(checks.len(), 1, "got: {}", checks.len());
    for written in ["String", "Copy", "CountType"] {
        assert!(
            some_token_was_written_as(&checks[0], written),
            "{written} was not carried at the span it was written at: {}",
            checks[0]
        );
    }
}

/// A parameter declares its bounds in either of two places, and a filling answers for both alike.
#[test]
fn a_bound_written_in_the_where_clause_is_read_like_one_written_beside_the_parameter() {
    let beside = filling_bound_check_text(
        "pub struct Counted<CountType: Copy> { pub count: CountType }",
        "default_types(CountType = String)",
    );
    let clause = filling_bound_check_text(
        "pub struct Counted<CountType> where CountType: Copy { pub count: CountType }",
        "default_types(CountType = String)",
    );
    assert_eq!(clause.len(), 1, "got: {clause:?}");
    assert_eq!(clause, beside);
}

/// A parameter bounded in both places is checked against every bound at once, so a filling has to
/// answer for all of them.
#[test]
fn a_parameter_bounded_in_both_places_is_checked_against_every_bound() {
    let checks = filling_bound_check_text(
        "pub struct Counted<CountType: Copy> where CountType: Clone { pub count: CountType }",
        "default_types(CountType = String)",
    );
    assert_eq!(checks.len(), 1, "got: {checks:?}");
    assert!(
        checks[0].contains("fn default_type_filling < CountType : Copy + Clone > ()"),
        "got: {}",
        checks[0]
    );
}

/// A parameter declaring no bound admits every filling, so there is nothing to ask and the
/// expansion is left exactly as it was — in every shape the attribute expands, and beside the
/// lifetimes and consts that name no type.
#[test]
fn an_unbounded_parameter_earns_no_check() {
    for (source, args) in [
        (
            "pub struct Both<IdType, DateType> { pub id: IdType, pub at: DateType }",
            "default_types(IdType = String, DateType = f64)",
        ),
        (
            "pub enum Tagged<IdType> { Named { id: IdType } }",
            "default_types(IdType = String)",
        ),
        (
            "pub type Boxed<ValueType> = Vec<ValueType>;",
            "default_types(ValueType = String)",
        ),
        (
            "pub struct Mixed<'label, IdType, const WIDTH: usize> { pub id: IdType }",
            "default_types(IdType = String)",
        ),
        ("pub struct Plain { pub id: String }", ""),
    ] {
        let checks = filling_bound_check_text(source, args);
        assert!(checks.is_empty(), "for {source}: {checks:?}");
    }
}

/// A bound naming another parameter of the item holds only where that one is filled too, which is a
/// joint statement this per-filling check does not make — and the neighbour's name reproduced
/// beside a single filling would resolve to nothing. So the bound is left to the item's own use
/// sites, whether it names a type parameter, a lifetime or a const.
#[test]
fn a_bound_naming_another_parameter_of_the_item_earns_no_check() {
    for (source, args) in [
        (
            "pub struct Pair<AType: From<BType>, BType> { pub a: AType, pub b: BType }",
            "default_types(AType = String, BType = char)",
        ),
        (
            "pub struct Held<'label, ValueType: Into<&'label str>> { pub held: ValueType }",
            "default_types(ValueType = String)",
        ),
        (
            "pub struct Bounded<ValueType: Fits<WIDTH>, const WIDTH: usize> { pub held: ValueType }",
            "default_types(ValueType = String)",
        ),
    ] {
        let checks = filling_bound_check_text(source, args);
        assert!(checks.is_empty(), "for {source}: {checks:?}");
    }
}

/// A bound written in terms of the parameter it bounds names no neighbour, so it is checked like
/// any other.
#[test]
fn a_bound_naming_only_the_parameter_it_bounds_is_still_checked() {
    let checks = filling_bound_check_text(
        "pub struct Held<ValueType: Iterator<Item = ValueType>> { pub held: ValueType }",
        "default_types(ValueType = String)",
    );
    assert_eq!(checks.len(), 1, "got: {checks:?}");
    assert!(
        checks[0].contains("Iterator < Item = ValueType >"),
        "got: {}",
        checks[0]
    );
}

/// One check per bounded filling, in the order the entries were written, and none for the unbounded
/// parameters beside them.
#[test]
fn every_bounded_filling_earns_its_own_check() {
    let checks = filling_bound_check_text(
        "pub struct Trio<AType: Copy, BType, CType: Clone> { pub a: AType, pub b: BType, pub c: CType }",
        "default_types(AType = u8, BType = String, CType = String)",
    );
    assert_eq!(checks.len(), 2, "got: {checks:?}");
    assert!(
        checks[0].contains("Copy") && checks[0].contains("u8"),
        "got: {}",
        checks[0]
    );
    assert!(
        checks[1].contains("Clone") && checks[1].contains("String"),
        "got: {}",
        checks[1]
    );
}

/// The three shapes the attribute expands answer alike: the check is read off the item's own
/// parameters, which every one of them binds the same way.
#[test]
fn every_expanded_shape_checks_its_fillings_alike() {
    let expected = filling_bound_check_text(
        "pub struct Held<ValueType: Copy> { pub held: ValueType }",
        "default_types(ValueType = String)",
    );
    assert_eq!(expected.len(), 1, "got: {expected:?}");
    for source in [
        "pub enum Held<ValueType: Copy> { Named { held: ValueType } }",
        "pub type Held<ValueType: Copy> = Vec<ValueType>;",
    ] {
        assert_eq!(
            filling_bound_check_text(source, "default_types(ValueType = String)"),
            expected,
            "for {source}"
        );
    }
}

/// The `compile_error!` tokens `source` earns for the example it carries against the parameters it
/// declares. Parsed from text so the tokens carry file locations and each refusal's span can be
/// read back as the source it points at.
#[cfg(feature = "zod")]
fn const_example_refusals(source: &str) -> Vec<proc_macro2::TokenStream> {
    super::const_parameter_example_errors(&syn::parse_str(source).unwrap())
}

/// The refusals `source` earns, rendered.
#[cfg(feature = "zod")]
fn const_example_messages(source: &str) -> Vec<String> {
    const_example_refusals(source)
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// A doc example is Rust compiled at one instantiation, and no value is the one every
/// const-parameterised example is written at, so an item that writes one while declaring a const
/// is refused instead of expanded into a `schema_example()` that cannot compile. Both shapes that
/// publish an example answer alike, the branded newtype among them.
#[cfg(feature = "zod")]
#[test]
fn a_doc_example_on_a_const_declaring_item_is_refused() {
    for (source, label) in [
        (
            format!("{EXAMPLE_DOC_BLOCK}pub enum Probe<const WIDTH: usize> {{ Held }}"),
            "type `Probe`",
        ),
        (
            format!("{EXAMPLE_DOC_BLOCK}pub struct Probe<const WIDTH: usize>(pub String);"),
            "type `Probe`",
        ),
    ] {
        let messages = const_example_messages(&source);
        assert_eq!(messages.len(), 1, "for {source}: {messages:?}");
        for needle in [
            "compile_error",
            "WIDTH",
            "const parameter",
            "```rust example",
            "`zod` feature",
            label,
        ] {
            assert!(
                messages[0].contains(needle),
                "{needle} missing for {source}: {}",
                messages[0]
            );
        }
    }
}

/// The refusal is the one the item earned, not one per parameter: an item writes a single example,
/// so a second const adds a name to the message rather than a second diagnostic. It points at the
/// first const declared, the example itself having no one token to sit on.
#[cfg(feature = "zod")]
#[test]
fn a_doc_example_is_refused_once_and_names_every_const_declared() {
    let source = format!(
        "{EXAMPLE_DOC_BLOCK}pub struct Probe<'label, ValueType, const WIDTH: usize, const DEPTH: \
         usize> {{ pub value: ValueType }}"
    );
    let refusals = const_example_refusals(&source);
    assert_eq!(refusals.len(), 1, "got: {refusals:?}");
    assert_eq!(refusals[0].span().source_text().as_deref(), Some("WIDTH"));
    let rendered = refusals[0].to_string();
    for needle in ["WIDTH", "DEPTH"] {
        assert!(rendered.contains(needle), "{needle} missing: {rendered}");
    }
}

/// What a const costs is the example, not the declaration: an item that writes none is expanded
/// exactly as before, and so is one whose parameters are all kinds a filling exists for — a
/// lifetime elides in the annotation and a type parameter takes `String`.
#[cfg(feature = "zod")]
#[test]
fn an_item_the_example_convention_covers_earns_no_refusal() {
    for source in [
        "pub enum Probe<const WIDTH: usize> { Held }".to_owned(),
        "pub struct Probe<const WIDTH: usize>(pub String);".to_owned(),
        format!("{EXAMPLE_DOC_BLOCK}pub struct Probe<'label> {{ pub label: &'label str }}"),
        format!("{EXAMPLE_DOC_BLOCK}pub struct Probe<ValueType> {{ pub value: ValueType }}"),
        format!("{EXAMPLE_DOC_BLOCK}pub enum Probe {{ Held }}"),
        format!("{EXAMPLE_DOC_BLOCK}pub struct Probe {{ pub value: String }}"),
    ] {
        let messages = const_example_messages(&source);
        assert!(messages.is_empty(), "for {source}: {messages:?}");
    }
}

/// An alias publishes no `schema_example()` — the expansion never reads its example — so a const
/// on one costs nothing and is left alone. The refusal is owed exactly where the method is built.
#[cfg(feature = "zod")]
#[test]
fn a_const_declaring_alias_is_left_alone() {
    let source = format!("{EXAMPLE_DOC_BLOCK}pub type Probe<const WIDTH: usize> = [u8; WIDTH];");
    let messages = const_example_messages(&source);
    assert!(messages.is_empty(), "got: {messages:?}");
}

/// Builds the `Display` assertion for the sole field of `source`, parsed from text so its spans
/// carry file locations and `source_text()` can report what they point at.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn display_assertion(source: &str) -> proc_macro2::TokenStream {
    let item: syn::ItemStruct = syn::parse_str(source).unwrap();
    let field = item.fields.iter().next().unwrap();
    super::build_branded_display_assertion(field, &item.generics)
}

/// Collects the source text each token in `tokens` points at, skipping the macro-synthesized
/// tokens that carry no location.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn located_source_texts(tokens: &proc_macro2::TokenStream) -> Vec<String> {
    let mut texts = Vec::new();
    for tree in tokens.clone() {
        match &tree {
            proc_macro2::TokenTree::Group(group) => {
                texts.extend(located_source_texts(&group.stream()));
            }
            proc_macro2::TokenTree::Ident(_)
            | proc_macro2::TokenTree::Punct(_)
            | proc_macro2::TokenTree::Literal(_) => texts.extend(tree.span().source_text()),
        }
    }
    texts
}

/// Without a span carried over from the user's source there is no source text to report, which is
/// what the assertion below would silently degrade into.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn the_span_probe_sees_an_unlocated_token_stream() {
    let tokens = quote::quote! { const _: () = {}; };
    assert!(tokens.span().source_text().is_none());
    assert!(located_source_texts(&tokens).is_empty());
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn display_assertion_names_the_trait_and_points_at_the_inner_field() {
    let tokens = display_assertion("pub struct Tags(pub Vec<String>);");
    let rendered = tokens.to_string();
    assert!(
        rendered.contains("std :: fmt :: Display"),
        "got: {rendered}"
    );
    assert!(rendered.contains("Vec < String >"), "got: {rendered}");
    assert_eq!(tokens.span().source_text().as_deref(), Some("Vec<String>"));
}

/// A `const` item cannot name the struct's generic parameters, and the `Display` bound the impl
/// adds to each type parameter already reports the violation at the instantiation site.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn display_assertion_is_skipped_when_the_inner_names_a_generic_param() {
    for source in [
        "pub struct DocumentId<IdType>(pub IdType);",
        "pub struct Wrapped<T>(pub Vec<T>);",
        "pub struct Borrowed<'a>(pub &'a str);",
        "pub struct Fixed<const N: usize>(pub [u8; N]);",
    ] {
        let tokens = display_assertion(source);
        assert!(tokens.is_empty(), "expected no assertion for {source}");
    }
}

/// Locks the delegating impl: the tokens are the ones branded newtypes have always carried, and
/// every located one points at the inner field so a non-`Display` inner is blamed there.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn display_impl_delegates_from_the_inner_field_span() {
    let item: syn::ItemStruct = syn::parse_str("pub struct UserId(pub String);").unwrap();
    let field = item.fields.iter().next().unwrap();
    let tokens = super::build_branded_display_impl(&item.generics, &item.ident, field);
    assert_eq!(
        tokens.to_string(),
        "impl std :: fmt :: Display for UserId { fn fmt (& self , f : & mut std :: fmt :: Formatter < '_ >) -> std :: fmt :: Result { self . 0 . fmt (f) } }"
    );
    assert_eq!(
        located_source_texts(&tokens).join(" "),
        "UserId String String String String String String",
        "the interpolated type name, then `self . 0 . fmt (f)` on the inner field"
    );
}

/// The generic impl keeps its own `Display` bound on every type parameter; that bound, not the
/// skipped assertion, is what carries the requirement.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn generic_display_impl_bounds_every_type_parameter() {
    let item: syn::ItemStruct =
        syn::parse_str("pub struct DocumentId<IdType>(pub IdType);").unwrap();
    let field = item.fields.iter().next().unwrap();
    let tokens = super::build_branded_display_impl(&item.generics, &item.ident, field);
    assert_eq!(
        tokens.to_string(),
        "impl < IdType : std :: fmt :: Display > std :: fmt :: Display for DocumentId < IdType > { fn fmt (& self , f : & mut std :: fmt :: Formatter < '_ >) -> std :: fmt :: Result { self . 0 . fmt (f) } }"
    );
}

/// Builds the whole `Display` block — assertion plus impl — for the sole field of `source`.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn display_tokens(source: &str, args: &proc_macro2::TokenStream) -> String {
    let item: syn::ItemStruct = syn::parse_str(source).unwrap();
    let field = item.fields.iter().next().unwrap();
    super::build_branded_display_tokens(
        &item.generics,
        &item.ident,
        field,
        &super::parse_model_schema_args(args.clone()),
    )
    .to_string()
}

/// `no_display` drops the `Display` impl, never the requirement: the constrained path validates
/// through `value.to_string()`, so a brand that opted out still has to prove the inner is
/// `Display` — at the field, not at the attribute.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn constraints_keep_the_display_assertion_when_the_brand_opts_out_of_the_impl() {
    let tokens = display_tokens(
        "pub struct Slugs(pub Tags);",
        &quote::quote! { no_display, pattern = "^[a-z]+$" },
    );
    assert!(
        tokens.contains("assert_display :: < Tags > ()"),
        "got: {tokens}"
    );
    assert!(
        !tokens.contains("impl std :: fmt :: Display"),
        "got: {tokens}"
    );
}

/// The three combinations that predate the constrained-path assertion keep their exact tokens.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn the_display_block_is_unchanged_for_every_pre_existing_combination() {
    let assertion_and_impl = display_tokens("pub struct UserId(pub String);", &quote::quote! {});
    assert!(
        assertion_and_impl.contains("assert_display :: < String > ()")
            && assertion_and_impl.contains("impl std :: fmt :: Display for UserId"),
        "got: {assertion_and_impl}"
    );
    assert_eq!(
        display_tokens(
            "pub struct SlugId(pub String);",
            &quote::quote! { pattern = "^[a-z]+$" }
        ),
        assertion_and_impl.replace("UserId", "SlugId"),
        "a constrained brand that kept its impl emits what an unconstrained one does"
    );
    assert_eq!(
        display_tokens(
            "pub struct Tags(pub Vec<String>);",
            &quote::quote! { no_display }
        ),
        "",
        "an unconstrained opt-out emits neither half"
    );
}

/// How many tokens of each `to_string()` call site a constrained brand expands to point back at
/// the inner field. Built from parsed source, so a token that carries the inner type's span has a
/// location to report and a macro-synthesized one does not.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
fn constrained_brand_inner_spanned_tokens(source: &str, inner: &str) -> (usize, usize) {
    let item: syn::ItemStruct = syn::parse_str(source).unwrap();
    let args = super::parse_model_schema_args(quote::quote! { pattern = "^[a-z]+$" });
    let validation =
        super::build_branded_validation(&args, false, &item.fields.iter().next().unwrap().ty)
            .unwrap();
    let module_ident = syn::Ident::new("slug_id_schema", proc_macro2::Span::call_site());
    let (_, _, validate_method) = super::inject_branded_serde_attrs(
        item,
        Some(&validation),
        false,
        &[],
        "slug_id_schema",
        &module_ident,
    );
    let count = |tokens| {
        located_source_texts(tokens)
            .iter()
            .filter(|text| text.as_str() == inner)
            .count()
    };
    (count(&validation.deserialize_fn), count(&validate_method))
}

/// Neither `to_string()` the constrained path emits may carry the inner field's span. Spanned
/// tokens are judged by the consumer's lints as if hand-written, and on a `String` inner
/// `self.0.to_string()` is a redundant clone — the whole test suite fails on it. The `Display`
/// requirement is blamed by the static assertion instead, whose tokens are inert wherever they
/// land. Only the interpolated `#inner_ty` may point at the field here.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn neither_constrained_to_string_call_carries_the_inner_fields_span() {
    let (deserializer, validate_method) =
        constrained_brand_inner_spanned_tokens("pub struct SlugId(pub Tags);", "Tags");
    assert_eq!(
        deserializer, 2,
        "the two interpolated type names and nothing else"
    );
    assert_eq!(
        validate_method, 0,
        "`validate()` interpolates no inner type"
    );
}

/// A constrained brand's emitted text for an inner spelled `inner`, as the three streams it is made
/// of: the validator, the deserializer, and the `validate()` method that calls into the first.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
fn constrained_brand_emission(inner: &str, module: &str) -> (String, String, String) {
    let ty: syn::Type = syn::parse_str(inner).unwrap();
    let item: syn::ItemStruct = syn::parse_quote!(
        pub struct Branded(pub #ty);
    );
    let args = super::parse_model_schema_args(quote::quote! { pattern = "^[a-z]+$" });
    let validation =
        super::build_branded_validation(&args, false, &item.fields.iter().next().unwrap().ty)
            .unwrap();
    let module_ident = syn::Ident::new(module, proc_macro2::Span::call_site());
    let (_, _, validate_method) = super::inject_branded_serde_attrs(
        item,
        Some(&validation),
        false,
        &[],
        module,
        &module_ident,
    );
    (
        validation.validate_fn.to_string(),
        validation.deserialize_fn.to_string(),
        validate_method.to_string(),
    )
}

/// The constrained path's generated text is what it has always been. A wrapper that is not
/// transparent stays here too: only a deref reaches a path from outside it, and an `Option` or a
/// sequence has none to offer.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn the_constrained_path_renders_the_same_to_string_calls_it_always_has() {
    for spelling in ["String", "Option<PathBuf>", "Vec<PathBuf>"] {
        let (validate_fn, deserialize_fn, validate_method) =
            constrained_brand_emission(spelling, "slug_id_schema");
        assert!(
            validate_fn
                .starts_with("pub fn validate_value (value : & str) -> Result < () , String > {"),
            "for {spelling}, got: {validate_fn}"
        );
        assert!(
            deserialize_fn.contains("validate_value (& v . to_string ())"),
            "for {spelling}, got: {deserialize_fn}"
        );
        assert!(
            validate_method
                .contains("slug_id_schema :: validate_value (& self . 0 . to_string ())"),
            "for {spelling}, got: {validate_method}"
        );
    }
}

/// A brand's constrained value is its inner field, so a path inner is reached the way a path field
/// is: the validator takes the borrowed path and renders it once, and neither call site names a
/// `to_string()` a path has none of. A transparent wrapper adds no call of its own — the borrow the
/// bare spelling already writes is what deref coercion carries through it.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn a_path_brand_is_checked_through_its_lossy_rendering() {
    for spelling in [
        "PathBuf",
        "std::path::PathBuf",
        "Arc<Path>",
        "Box<Path>",
        "Cow<'static, Path>",
        "Rc<std::path::Path>",
        "Box<Arc<Path>>",
    ] {
        let (validate_fn, deserialize_fn, validate_method) =
            constrained_brand_emission(spelling, "asset_path_schema");
        assert!(
            validate_fn.starts_with(
                "pub fn validate_value (path : & std :: path :: Path) -> Result < () , String > { \
                 let rendered = path . to_string_lossy () ; let value : & str = & rendered ;"
            ),
            "for {spelling}, got: {validate_fn}"
        );
        assert!(
            deserialize_fn.contains("validate_value (& v)"),
            "for {spelling}, got: {deserialize_fn}"
        );
        assert!(
            validate_method.contains("asset_path_schema :: validate_value (& self . 0)"),
            "for {spelling}, got: {validate_method}"
        );
    }
}

/// The JSON schema statements a map-typed field expands to, parsed from the map type's source and
/// dispatched through the field entry point, so a wrapper spelling reaches the map arm the way a
/// declared field reaches it.
#[cfg(feature = "jsonschema")]
fn map_field_schema(map_type: &str) -> proc_macro2::TokenStream {
    let ty: syn::Type = syn::parse_str(map_type).unwrap();
    super::build_field_type_schema(&super::get_field_def("m", &ty, ""), "m")
}

/// The value a field's `properties` insertion carries, lifted out of the statement so a wrapped
/// spelling can be held against the map it holds.
#[cfg(feature = "jsonschema")]
fn inserted_field_value(field_type: &str) -> String {
    const PREFIX: &str = r#"properties . insert ("m" . to_string () , "#;
    const SUFFIX: &str = ") ;";
    let tokens = map_field_schema(field_type).to_string();
    assert!(
        tokens.starts_with(PREFIX) && tokens.ends_with(SUFFIX),
        "for {field_type}, not a plain insertion: {tokens}"
    );
    tokens[PREFIX.len()..tokens.len() - SUFFIX.len()].to_owned()
}

/// A value type the enum-key branch cannot render must yield the `compile_error!` *instead of* the
/// per-member insertion loop: leaving the loop in place adds an E0425 on the `value_schema` the
/// failed arm never bound, and that second error names macro-internal state the author cannot act on.
/// The diagnostic names the field and the type, as the `String`-key branch's does — a message that
/// names neither leaves the author nothing to act on.
#[cfg(feature = "jsonschema")]
#[test]
fn an_unsupported_enum_keyed_map_value_emits_only_the_compile_error() {
    for map_type in [
        "HashMap<Slot, (String, u32)>",
        "HashMap<Slot, HashMap<String, (String, u32)>>",
        "HashMap<Slot, Vec<HashMap<String, (String, u32)>>>",
    ] {
        let tokens = map_field_schema(map_type).to_string();
        assert!(
            tokens.starts_with("compile_error !"),
            "for {map_type}, got: {tokens}"
        );
        assert!(
            !tokens.contains("enum_members"),
            "for {map_type}, got: {tokens}"
        );
        assert!(
            !tokens.contains("properties . insert"),
            "for {map_type}, got: {tokens}"
        );
        assert!(
            tokens.contains("field `m`"),
            "for {map_type}, got: {tokens}"
        );
        assert!(
            tokens.contains("a tuple is not supported as a map value"),
            "for {map_type}, got: {tokens}"
        );
    }
}

/// The enum-key branch's rendering for a map value built by hand, for the value types no source type
/// produces.
#[cfg(feature = "jsonschema")]
fn enum_key_map_value_binding(field_type: FieldDefType) -> String {
    let ty: syn::Type = syn::parse_str("String").unwrap();
    let mut value = super::get_field_def("m", &ty, "");
    value.field_type = field_type;
    super::enum_key_map_json_schema_value("Slot", proc_macro2::Span::call_site(), &value)
        .unwrap()
        .to_string()
}

/// A key that enumerates its members says nothing about what each member holds, so an enum-keyed
/// member is the member the `String`-key path renders — materialized as a `serde_json::Value` for
/// the insertion loop, and recursing to the same depth.
#[cfg(feature = "jsonschema")]
#[test]
fn a_nested_enum_keyed_map_value_renders_its_inner_members() {
    for (map_type, expected) in [
        (
            "HashMap<Slot, HashMap<String, String>>",
            r#"serde_json :: json ! ({ "type" : "object" , "additionalProperties" : { "type" : "string" } })"#,
        ),
        (
            "HashMap<Slot, HashMap<String, Vec<u64>>>",
            r#"serde_json :: json ! ({ "type" : "object" , "additionalProperties" : { "type" : "array" , "items" : { "type" : "integer" } } })"#,
        ),
        (
            "HashMap<Slot, HashMap<String, HashMap<String, f64>>>",
            r#"serde_json :: json ! ({ "type" : "object" , "additionalProperties" : { "type" : "object" , "additionalProperties" : { "type" : "number" } } })"#,
        ),
        (
            "HashMap<Slot, HashMap<String, Inner>>",
            r#"serde_json :: json ! ({ "type" : "object" , "additionalProperties" : inner_schema :: Schema :: json_schema_within (in_flight , hoisted_defs) })"#,
        ),
        (
            "HashMap<Slot, Vec<HashMap<String, String>>>",
            r#"serde_json :: json ! ({ "type" : "array" , "items" : serde_json :: json ! ({ "type" : "object" , "additionalProperties" : { "type" : "string" } }) })"#,
        ),
        (
            "HashMap<Slot, Option<HashMap<String, String>>>",
            r#"serde_json :: json ! ({ "anyOf" : [serde_json :: json ! ({ "type" : "object" , "additionalProperties" : { "type" : "string" } }) , { "type" : "null" }] })"#,
        ),
    ] {
        let tokens = map_field_schema(map_type).to_string();
        assert!(
            tokens.contains(&format!("let value_schema = {expected} ;")),
            "for {map_type}, got: {tokens}"
        );
        assert!(
            tokens.contains("Slot :: enum_members ()"),
            "for {map_type}, got: {tokens}"
        );
    }
}

/// A generic sibling is a map value the `String`-key path renders through its schema module, so the
/// enum-key path renders it there too.
#[cfg(feature = "jsonschema")]
#[test]
fn a_generic_sibling_enum_keyed_map_value_emits_the_sibling_schema() {
    let tokens = map_field_schema("HashMap<Slot, Wrapper<String>>").to_string();
    assert!(
        tokens.contains("let value_schema = wrapper_schema :: Schema :: json_schema_within (in_flight , hoisted_defs) ;"),
        "got: {tokens}"
    );
}

/// An opaque value has no type name to narrow with on either key path, so the member stays
/// permissive rather than collapsing the whole field to a diagnostic.
#[cfg(feature = "jsonschema")]
#[test]
fn an_opaque_enum_keyed_map_value_stays_permissive() {
    let tokens = enum_key_map_value_binding(FieldDefType::Unknown);
    assert!(
        tokens.contains("let value_schema = serde_json :: json ! ({ }) ;"),
        "got: {tokens}"
    );
}

/// A string literal keeps the `const` it carries on the `String`-key path.
#[cfg(feature = "jsonschema")]
#[test]
fn a_string_literal_enum_keyed_map_value_keeps_its_const() {
    let tokens = enum_key_map_value_binding(FieldDefType::StringLiteral("Tixena".to_owned()));
    assert!(
        tokens.contains(
            r#"let value_schema = serde_json :: json ! ({ "type" : "string" , "const" : "Tixena" }) ;"#
        ),
        "got: {tokens}"
    );
}

/// A chrono value keeps the format it carries in field position, as it does under a `String` key.
#[cfg(all(feature = "chrono", feature = "jsonschema"))]
#[test]
fn a_chrono_enum_keyed_map_value_keeps_its_format() {
    for (map_type, format) in [
        ("HashMap<Slot, NaiveDate>", "date"),
        ("HashMap<Slot, NaiveTime>", "time"),
        ("HashMap<Slot, NaiveDateTime>", "date-time"),
        ("HashMap<Slot, DateTime<Utc>>", "date-time"),
    ] {
        let tokens = map_field_schema(map_type).to_string();
        assert!(
            tokens.contains(&format!(
                r#"let value_schema = serde_json :: json ! ({{ "type" : "string" , "format" : "{format}" }}) ;"#
            )),
            "for {map_type}, got: {tokens}"
        );
    }
}

/// An enum-keyed member binds the one `$oid` object every position spells — the same one a
/// `String`-keyed member carries. Pinned so neither key path can grow a spelling of its own.
#[cfg(all(feature = "object_id", feature = "jsonschema"))]
#[test]
fn an_object_id_enum_keyed_map_value_binds_the_one_oid_object() {
    let tokens = enum_key_map_value_binding(FieldDefType::ObjectId);
    assert!(
        tokens.contains(
            r#"let value_schema = serde_json :: json ! ({ "type" : "object" , "properties" : { "$oid" : (serde_json :: json ! ({ "type" : "string" , "pattern" : "^[a-f0-9]{24}$" })) } , "required" : ["$oid"] , "additionalProperties" : false }) ;"#
        ),
        "got: {tokens}"
    );
}

#[cfg(feature = "jsonschema")]
#[test]
fn a_scalar_enum_keyed_map_value_expands_to_the_per_member_loop() {
    let tokens = map_field_schema("HashMap<Slot, String>").to_string();
    assert!(!tokens.contains("compile_error"), "got: {tokens}");
    assert!(tokens.contains("Slot :: enum_members ()"), "got: {tokens}");
    assert!(
        tokens.contains(r#"serde_json :: json ! ({ "type" : "string" })"#),
        "got: {tokens}"
    );
}

/// A `Vec` of siblings is the inner sibling with an array level counted onto it, so the member schema has to array
/// the sibling's own schema — bound bare, it types the member as one sibling and turns away every
/// payload serde produces.
#[cfg(feature = "jsonschema")]
#[test]
fn a_vec_sibling_enum_keyed_map_value_arrays_the_sibling_schema() {
    let arrayed = map_field_schema("HashMap<Slot, Vec<Inner>>").to_string();
    assert!(
        arrayed.contains(
            r#"let value_schema = serde_json :: json ! ({ "type" : "array" , "items" : inner_schema :: Schema :: json_schema_within (in_flight , hoisted_defs) }) ;"#
        ),
        "got: {arrayed}"
    );

    let single = map_field_schema("HashMap<Slot, Inner>").to_string();
    assert!(
        single.contains("let value_schema = inner_schema :: Schema :: json_schema_within (in_flight , hoisted_defs) ;"),
        "got: {single}"
    );
}

/// A map entry cannot be dropped the way an object key can, so serde writes an `Option` value's
/// `None` as JSON `null`. Both key paths have to admit it, and through the same seam: the enum-key
/// branch materializes the nullable form as a `serde_json::Value`, the `String`-key branch inlines
/// it as the map's `additionalProperties`.
#[cfg(feature = "jsonschema")]
#[test]
fn an_optional_map_value_is_nullable_on_both_key_paths() {
    let enum_keyed = map_field_schema("HashMap<Slot, Option<String>>").to_string();
    assert!(
        enum_keyed.contains(
            r#"let value_schema = serde_json :: json ! ({ "anyOf" : [serde_json :: json ! ({ "type" : "string" }) , { "type" : "null" }] }) ;"#
        ),
        "got: {enum_keyed}"
    );

    let string_keyed = map_field_schema("HashMap<String, Option<String>>").to_string();
    assert!(
        string_keyed.contains(
            r#""additionalProperties" : { "anyOf" : [{ "type" : "string" } , { "type" : "null" }] }"#
        ),
        "got: {string_keyed}"
    );
}

/// A non-`Option` map value is untouched by the nullable seam — on either key path the tokens are
/// the ones the value type has always produced.
#[cfg(feature = "jsonschema")]
#[test]
fn a_required_map_value_carries_no_nullable_wrap() {
    for map_type in [
        "HashMap<Slot, String>",
        "HashMap<Slot, Vec<Inner>>",
        "HashMap<String, String>",
        "HashMap<String, Vec<u64>>",
    ] {
        let tokens = map_field_schema(map_type).to_string();
        assert!(!tokens.contains("anyOf"), "for {map_type}, got: {tokens}");
    }
}

/// The kind an alias registers is its *target's* answer, because a type path resolves through the
/// alias — the same four verdicts a brand carries up from its inner, and for the same reason. A
/// target serde writes as a bare string makes the alias one too, whatever spelling that target
/// wears; a target serde stringifies for the author makes the alias key the open object that bare
/// target keys; a target serde writes as no key at all leaves the alias refused, under its own name.
/// `Vec<Slot>` is the collection, not the enum it holds; a target this expansion has not seen
/// registered is `Unknown`, which is not a negative.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn an_alias_registers_the_kind_of_what_it_targets() {
    register_alias_info("Slot", "Slot", "slot_schema", AliasKind::EnumMembers);
    register_alias_info("Doc", "Doc", "doc_schema", AliasKind::NoEnumMembers);
    register_alias_info(
        "CorrelationId",
        "CorrelationId",
        "correlation_id_schema",
        AliasKind::StringWire,
    );
    register_alias_info("Tick", "Tick", "tick_schema", AliasKind::Stringified);
    for (target, expected) in [
        ("Slot", AliasKind::EnumMembers),
        ("Doc", AliasKind::NoEnumMembers),
        ("String", AliasKind::StringWire),
        ("PathBuf", AliasKind::StringWire),
        ("CorrelationId", AliasKind::StringWire),
        ("u32", AliasKind::Stringified),
        ("bool", AliasKind::Stringified),
        ("f64", AliasKind::Stringified),
        ("Tick", AliasKind::Stringified),
        ("Wrapper<Slot>", AliasKind::Stringified),
        ("Vec<String>", AliasKind::NoEnumMembers),
        ("Option<String>", AliasKind::NoEnumMembers),
        ("Vec<Slot>", AliasKind::NoEnumMembers),
        ("HashMap<Slot, String>", AliasKind::NoEnumMembers),
        ("(String, String)", AliasKind::NoEnumMembers),
        ("Ghost", AliasKind::Unknown),
    ] {
        let ty: syn::Type = syn::parse_str(target).unwrap();
        let kind = super::alias_target_kind(&super::get_field_def("AliasType", &ty, ""));
        assert_eq!(kind, expected, "for alias target {target}");
    }
}

/// The alias path and the brand path answer the one map-key question the same way, target for
/// inner: both spellings wrap a value whose wire form is the whole of what they carry, so a
/// disagreement between them would be a key that opens an object under one spelling and is refused
/// under the other. `EnumMembers` is the one verdict only the alias can reach — a type path resolves
/// to the enum, where a brand publishes no `enum_members()` of its own — so the enum spellings are
/// held apart rather than in this list.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn an_alias_and_a_brand_answer_alike_for_the_same_target() {
    register_alias_info("Doc", "Doc", "doc_schema", AliasKind::NoEnumMembers);
    register_alias_info(
        "CorrelationId",
        "CorrelationId",
        "correlation_id_schema",
        AliasKind::StringWire,
    );
    register_alias_info("Tick", "Tick", "tick_schema", AliasKind::Stringified);
    for target in [
        "String",
        "PathBuf",
        "CorrelationId",
        "u32",
        "bool",
        "f64",
        "Tick",
        "Doc",
        "Vec<String>",
        "(String, String)",
        "HashMap<String, u32>",
    ] {
        let ty: syn::Type = syn::parse_str(target).unwrap();
        let alias = super::alias_target_kind(&super::get_field_def("AliasType", &ty, ""));
        assert_eq!(alias, brand_kind(target), "for target {target}");
    }
}

/// A chrono value is one serde stringifies into a key, so an alias of one keys the open object its
/// bare target keys — the verdict the brand over the same target already carries.
#[cfg(all(
    feature = "chrono",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn an_alias_of_a_chrono_target_is_stringified() {
    for target in ["NaiveDate", "NaiveTime", "NaiveDateTime", "DateTime<Utc>"] {
        let ty: syn::Type = syn::parse_str(target).unwrap();
        let kind = super::alias_target_kind(&super::get_field_def("AliasType", &ty, ""));
        assert_eq!(kind, AliasKind::Stringified, "for alias target {target}");
    }
}

/// An `ObjectId` writes a JSON object, which serde uses as no key at all, so an alias of one stays
/// refused where the stringifying targets are let through.
#[cfg(all(
    feature = "object_id",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn an_alias_of_an_object_id_stays_refused() {
    let ty: syn::Type = syn::parse_str("ObjectId").unwrap();
    let kind = super::alias_target_kind(&super::get_field_def("AliasType", &ty, ""));
    assert_eq!(kind, AliasKind::NoEnumMembers);
}

/// An alias of an alias of a value serde stringifies is still that value at the type path, so the
/// chain carries `Stringified` through every link — and so does a chain ending at a stringified
/// brand.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn an_alias_chain_carries_the_stringified_kind_to_its_end() {
    register_alias_info("Tick", "Tick", "tick_schema", AliasKind::Stringified);
    for end in ["u32", "Tick"] {
        let first_target: syn::Type = syn::parse_str(end).unwrap();
        let first = super::alias_target_kind(&super::get_field_def("FirstType", &first_target, ""));
        assert_eq!(first, AliasKind::Stringified, "for a chain ending at {end}");
        register_alias_info("First", "FirstType", "first_type_schema", first);

        let second_target: syn::Type = syn::parse_str("First").unwrap();
        let second =
            super::alias_target_kind(&super::get_field_def("SecondType", &second_target, ""));
        assert_eq!(
            second,
            AliasKind::Stringified,
            "for a chain ending at {end}"
        );
    }
}

/// A refused target keeps the diagnostic naming the *alias*, not the target's own rejection reason:
/// the alias is what the author wrote at the key, and it is what they can act on. So a field keyed
/// by an alias of a tuple reads as a name with no `enum_members()` rather than as an array wire, and
/// so does one keyed by an alias of the wrapper spellings — a target the key dispatch refuses is
/// refused however it was spelled, including the `Option` and the sequence around a plain enum,
/// neither of which the alias can supply members for.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn an_alias_of_a_refused_target_is_refused_under_its_own_name() {
    register_alias_info("Slot", "Slot", "slot_schema", AliasKind::EnumMembers);
    for target in ["(String, String)", "Option<Slot>", "Vec<Slot>"] {
        register_alias_info(
            "RefusedKey",
            "RefusedKeyType",
            "refused_key_type_schema",
            super::alias_target_kind(&super::get_field_def(
                "RefusedKeyType",
                &syn::parse_str(target).unwrap(),
                "",
            )),
        );
        let error = field_map_key_error(&quote::quote! { HashMap<RefusedKey, u32> });
        assert!(
            error.contains("a map key must be a plain"),
            "for {target}, got: {error}"
        );
        assert!(error.contains("RefusedKey"), "for {target}, got: {error}");
        assert!(
            !error.contains("serde writes"),
            "for {target}, got: {error}"
        );
    }
}

/// An alias of an alias of a string is still that bare string at the type path, so the chain carries
/// `StringWire` through every link — and so does a chain ending at a string-wire brand.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn an_alias_chain_carries_the_string_wire_kind_to_its_end() {
    register_alias_info(
        "CorrelationId",
        "CorrelationId",
        "correlation_id_schema",
        AliasKind::StringWire,
    );
    for end in ["String", "CorrelationId"] {
        let first_target: syn::Type = syn::parse_str(end).unwrap();
        let first = super::alias_target_kind(&super::get_field_def("FirstType", &first_target, ""));
        assert_eq!(first, AliasKind::StringWire, "for a chain ending at {end}");
        register_alias_info("First", "FirstType", "first_type_schema", first);

        let second_target: syn::Type = syn::parse_str("First").unwrap();
        let second =
            super::alias_target_kind(&super::get_field_def("SecondType", &second_target, ""));
        assert_eq!(second, AliasKind::StringWire, "for a chain ending at {end}");
    }
}

/// [`super::branded_alias_kind`] read off the one field a brand is written with.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn brand_kind(inner: &str) -> AliasKind {
    let inner_ty: syn::Type = syn::parse_str(inner).unwrap();
    let item: syn::ItemStruct = syn::parse_quote! { struct Brand(#inner_ty); };
    super::branded_alias_kind(item.fields.iter().next().unwrap())
}

/// The kind a brand registers is what serde writes for its inner, the brand being
/// `#[serde(transparent)]` over it. A string-shaped inner is written as the bare string a JSON
/// object key is; a plain enum's variant name is that bare string too, and the brand carries no
/// `enum_members()` of its own to close an object over; a value serde stringifies is written as the
/// object its bare inner writes; and an inner serde refuses as a key, or one this expansion has not
/// classified, leaves the brand refused.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_brand_registers_what_serde_writes_for_its_inner() {
    register_alias_info("Slot", "Slot", "slot_schema", AliasKind::EnumMembers);
    register_alias_info("Doc", "Doc", "doc_schema", AliasKind::NoEnumMembers);
    register_alias_info(
        "CorrelationId",
        "CorrelationId",
        "correlation_id_schema",
        AliasKind::StringWire,
    );
    register_alias_info("Tick", "Tick", "tick_schema", AliasKind::Stringified);
    for (inner, expected) in [
        ("String", AliasKind::StringWire),
        ("PathBuf", AliasKind::StringWire),
        ("CorrelationId", AliasKind::StringWire),
        ("Slot", AliasKind::StringWire),
        ("u32", AliasKind::Stringified),
        ("bool", AliasKind::Stringified),
        ("f64", AliasKind::Stringified),
        ("Tick", AliasKind::Stringified),
        ("Doc", AliasKind::NoEnumMembers),
        ("Vec<String>", AliasKind::NoEnumMembers),
        ("(String, String)", AliasKind::NoEnumMembers),
        ("HashMap<String, u32>", AliasKind::NoEnumMembers),
        ("Ghost", AliasKind::NoEnumMembers),
    ] {
        assert_eq!(brand_kind(inner), expected, "for brand inner {inner}");
    }
}

/// The chrono renderings are keys serde stringifies, so a brand over one is written as the object
/// its bare inner is written as.
#[cfg(all(
    feature = "chrono",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn a_brand_over_a_chrono_inner_is_stringified() {
    for inner in ["NaiveDate", "NaiveTime", "NaiveDateTime", "DateTime<Utc>"] {
        assert_eq!(
            brand_kind(inner),
            AliasKind::Stringified,
            "for brand inner {inner}"
        );
    }
}

/// An `ObjectId` writes a JSON object, which serde uses as no key at all, so a brand over one stays
/// refused where the stringifying inners are let through.
#[cfg(all(
    feature = "object_id",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn a_brand_over_an_object_id_stays_refused() {
    assert_eq!(brand_kind("ObjectId"), AliasKind::NoEnumMembers);
}

/// A key the registry proves serde stringifies keeps the open object its bare inner describes as,
/// at every depth a map is written at — and the refusals around it are untouched, a brand over a
/// container or a struct still writing no key at all.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_stringified_key_is_left_alone_wherever_it_is_written() {
    register_alias_info("Slot", "Slot", "slot_schema", AliasKind::EnumMembers);
    register_alias_info("Tick", "Tick", "tick_schema", AliasKind::Stringified);
    register_alias_info("Tags", "Tags", "tags_schema", AliasKind::NoEnumMembers);
    for field_type in [
        quote::quote! { HashMap<Tick, u32> },
        quote::quote! { HashMap<String, HashMap<Tick, u32>> },
        quote::quote! { HashMap<Slot, HashMap<Tick, u32>> },
        quote::quote! { HashMap<Tick, HashMap<Tick, u32>> },
        quote::quote! { Vec<HashMap<Tick, u32>> },
        quote::quote! { (String, HashMap<Tick, u32>) },
        quote::quote! { Wrapper<HashMap<Tick, u32>> },
    ] {
        let error = field_map_key_error(&field_type);
        assert!(error.is_empty(), "for {field_type}, got: {error}");
    }

    let refused = field_map_key_error(&quote::quote! { HashMap<Tags, u32> });
    assert!(
        refused.contains("a map key must be a plain"),
        "got: {refused}"
    );
    assert!(refused.contains("Tags"), "got: {refused}");
}

/// An alias of an alias of a plain enum is still a plain enum at the type path, so the chain
/// carries `EnumMembers` through every link.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn an_alias_chain_carries_the_enum_kind_to_its_end() {
    register_alias_info("Slot", "Slot", "slot_schema", AliasKind::EnumMembers);
    let first_target: syn::Type = syn::parse_str("Slot").unwrap();
    let first = super::alias_target_kind(&super::get_field_def("FirstType", &first_target, ""));
    register_alias_info("First", "FirstType", "first_type_schema", first);

    let second_target: syn::Type = syn::parse_str("First").unwrap();
    let second = super::alias_target_kind(&super::get_field_def("SecondType", &second_target, ""));
    assert_eq!(second, AliasKind::EnumMembers);
}

/// A key the registry positively rules out never reaches the emitting path: `enum_members()` on it
/// resolves through the alias to a type that has no such method, and rustc blames the attribute for
/// a method the author never wrote.
#[cfg(feature = "jsonschema")]
#[test]
fn a_map_key_known_to_lack_enum_members_names_the_requirement() {
    register_alias_info(
        "KeyAlias",
        "KeyAliasType",
        "key_alias_type_schema",
        AliasKind::NoEnumMembers,
    );
    let tokens = map_field_schema("HashMap<KeyAlias, String>").to_string();
    assert!(
        !tokens.contains("KeyAlias :: enum_members"),
        "got: {tokens}"
    );
    assert!(tokens.starts_with("compile_error !"), "got: {tokens}");
    assert!(
        tokens.contains("a map key must be a plain"),
        "got: {tokens}"
    );
    assert!(tokens.contains("KeyAlias"), "got: {tokens}");
}

/// The registry extension is a filter, never a rewrite: for every key that compiled before it —
/// a plain enum, an alias of one, or a name this expansion cannot classify — the emitted tokens
/// are the ones an unregistered key has always produced.
#[cfg(feature = "jsonschema")]
#[test]
fn a_map_key_that_may_have_enum_members_expands_exactly_as_before() {
    let unregistered = map_field_schema("HashMap<Slot, String>").to_string();
    for kind in [AliasKind::EnumMembers, AliasKind::Unknown] {
        register_alias_info("Slot", "Slot", "slot_schema", kind);
        assert_eq!(
            map_field_schema("HashMap<Slot, String>").to_string(),
            unregistered
        );
    }
}

/// The `String`-key branch's output for a map value built by hand, for the value types no source
/// type produces: the `literal` override rewrites a *field's* type, never a map value's.
#[cfg(feature = "jsonschema")]
fn string_key_map_value_schema(field_type: FieldDefType) -> String {
    let ty: syn::Type = syn::parse_str("String").unwrap();
    let mut value = super::get_field_def("m", &ty, "");
    value.field_type = field_type;
    super::string_key_map_json_schema_value(&value)
        .unwrap()
        .to_string()
}

/// A `String` key says nothing about the value it holds, so a value type the crate renders is
/// rendered here too — the same mapping the field position uses, not an open member schema.
#[cfg(all(feature = "chrono", feature = "jsonschema"))]
#[test]
fn a_chrono_string_keyed_map_value_keeps_its_format() {
    for (map_type, format) in [
        ("HashMap<String, NaiveDate>", "date"),
        ("HashMap<String, NaiveTime>", "time"),
        ("HashMap<String, NaiveDateTime>", "date-time"),
        ("HashMap<String, DateTime<Utc>>", "date-time"),
    ] {
        let tokens = map_field_schema(map_type).to_string();
        assert!(
            tokens.contains(&format!(
                r#""additionalProperties" : {{ "type" : "string" , "format" : "{format}" }}"#
            )),
            "for {map_type}, got: {tokens}"
        );
    }
}

#[cfg(feature = "jsonschema")]
#[test]
fn a_string_literal_string_keyed_map_value_keeps_its_const() {
    let tokens = string_key_map_value_schema(FieldDefType::StringLiteral("Tixena".to_owned()));
    assert!(
        tokens.contains(r#""additionalProperties" : { "type" : "string" , "const" : "Tixena" }"#),
        "got: {tokens}"
    );
}

/// An opaque value has no type name to narrow with, so the member schema stays permissive — the
/// empty schema the crate settled on, in both the bare and the collection form.
#[cfg(feature = "jsonschema")]
#[test]
fn an_opaque_string_keyed_map_value_stays_permissive() {
    let tokens = string_key_map_value_schema(FieldDefType::Unknown);
    assert!(
        tokens.contains(r#""additionalProperties" : { }"#),
        "got: {tokens}"
    );
}

/// A value the branch cannot render must yield the `compile_error!` *instead of* the property
/// insertion, so exactly one diagnostic reaches the author — and it names the field, which is all
/// the author can act on.
#[cfg(feature = "jsonschema")]
#[test]
fn an_unsupported_string_keyed_map_value_emits_only_the_compile_error() {
    let tokens = map_field_schema("HashMap<String, (String, u32)>").to_string();
    assert!(tokens.starts_with("compile_error !"), "got: {tokens}");
    assert!(!tokens.contains("properties . insert"), "got: {tokens}");
    assert!(tokens.contains("field `m`"), "got: {tokens}");
    assert!(
        tokens.contains("a tuple is not supported as a map value"),
        "got: {tokens}"
    );
}

/// The value types the branch already rendered keep the tokens they have always produced: the
/// shared mapping is a reuse of the same renderings, not a rewrite of them.
#[cfg(feature = "jsonschema")]
#[test]
fn a_scalar_string_keyed_map_value_expands_exactly_as_before() {
    for (map_type, expected) in [
        ("HashMap<String, String>", r#"{ "type" : "string" }"#),
        ("HashMap<String, u64>", r#"{ "type" : "integer" }"#),
        ("HashMap<String, i8>", r#"{ "type" : "integer" }"#),
        ("HashMap<String, f32>", r#"{ "type" : "number" }"#),
        ("HashMap<String, bool>", r#"{ "type" : "boolean" }"#),
        (
            "HashMap<String, Vec<String>>",
            r#"{ "type" : "array" , "items" : { "type" : "string" } }"#,
        ),
    ] {
        let tokens = map_field_schema(map_type).to_string();
        assert!(
            tokens.contains(&format!(r#""additionalProperties" : {expected}"#)),
            "for {map_type}, got: {tokens}"
        );
    }
}

/// The `String`-key branch's output for a nested map value built by hand: a `String`-keyed inner
/// map holding `inner_type`, bare or behind a `Vec`, for the inner value types no source type
/// produces.
#[cfg(feature = "jsonschema")]
fn nested_string_key_map_value_schema(inner_type: FieldDefType, array_depth: u8) -> String {
    let ty: syn::Type = syn::parse_str("String").unwrap();
    let inner_key = super::get_field_def("m", &ty, "");
    let mut inner_value = inner_key.clone();
    inner_value.field_type = inner_type;
    let mut value = inner_key.clone();
    value.field_type = FieldDefType::Map(Box::new(inner_key), Box::new(inner_value));
    value.array_depth = array_depth;
    super::string_key_map_json_schema_value(&value)
        .unwrap()
        .to_string()
}

/// A map value that is itself a map is dispatched the same way at every depth: the members of the
/// inner map carry the inner value type's own rendering. A member schema that stops at the outer
/// map describes nothing about what the map holds.
#[cfg(feature = "jsonschema")]
#[test]
fn a_nested_string_keyed_map_value_renders_its_inner_members() {
    for (map_type, expected) in [
        (
            "HashMap<String, HashMap<String, String>>",
            r#"{ "type" : "object" , "additionalProperties" : { "type" : "string" } }"#,
        ),
        (
            "HashMap<String, HashMap<String, Vec<u64>>>",
            r#"{ "type" : "object" , "additionalProperties" : { "type" : "array" , "items" : { "type" : "integer" } } }"#,
        ),
        (
            "HashMap<String, HashMap<String, Option<bool>>>",
            r#"{ "type" : "object" , "additionalProperties" : { "anyOf" : [{ "type" : "boolean" } , { "type" : "null" }] } }"#,
        ),
        (
            "HashMap<String, HashMap<String, HashMap<String, f64>>>",
            r#"{ "type" : "object" , "additionalProperties" : { "type" : "object" , "additionalProperties" : { "type" : "number" } } }"#,
        ),
        (
            "HashMap<String, HashMap<String, Inner>>",
            r#"{ "type" : "object" , "additionalProperties" : inner_schema :: Schema :: json_schema_within (in_flight , hoisted_defs) }"#,
        ),
        (
            "HashMap<String, Option<HashMap<String, String>>>",
            r#"{ "anyOf" : [{ "type" : "object" , "additionalProperties" : { "type" : "string" } } , { "type" : "null" }] }"#,
        ),
    ] {
        let tokens = map_field_schema(map_type).to_string();
        assert!(
            tokens.contains(&format!(r#""additionalProperties" : {expected}"#)),
            "for {map_type}, got: {tokens}"
        );
        assert!(
            !tokens.contains(r#""additionalProperties" : true"#),
            "for {map_type}, got: {tokens}"
        );
    }
}

/// A `Vec` of maps arrays the same member schema the bare inner map renders — the array wrap sits
/// between the two dispatches, it does not replace the inner one.
#[cfg(feature = "jsonschema")]
#[test]
fn a_vec_of_maps_string_keyed_map_value_renders_its_inner_members() {
    for (map_type, expected) in [
        (
            "HashMap<String, Vec<HashMap<String, String>>>",
            r#"{ "type" : "array" , "items" : { "type" : "object" , "additionalProperties" : { "type" : "string" } } }"#,
        ),
        (
            "HashMap<String, Vec<HashMap<String, u64>>>",
            r#"{ "type" : "array" , "items" : { "type" : "object" , "additionalProperties" : { "type" : "integer" } } }"#,
        ),
        (
            "HashMap<String, Vec<HashMap<String, Inner>>>",
            r#"{ "type" : "array" , "items" : { "type" : "object" , "additionalProperties" : inner_schema :: Schema :: json_schema_within (in_flight , hoisted_defs) } }"#,
        ),
    ] {
        let tokens = map_field_schema(map_type).to_string();
        assert!(
            tokens.contains(&format!(r#""additionalProperties" : {expected}"#)),
            "for {map_type}, got: {tokens}"
        );
        assert!(
            !tokens.contains(r#""additionalProperties" : true"#),
            "for {map_type}, got: {tokens}"
        );
    }
}

/// A chrono value keeps the format it carries in field position however deep the nesting goes: the
/// depth is the map's, never the value type's.
#[cfg(all(feature = "chrono", feature = "jsonschema"))]
#[test]
fn a_nested_chrono_map_value_keeps_its_format() {
    for (map_type, expected) in [
        (
            "HashMap<String, HashMap<String, NaiveDate>>",
            r#"{ "type" : "object" , "additionalProperties" : { "type" : "string" , "format" : "date" } }"#,
        ),
        (
            "HashMap<String, HashMap<String, NaiveTime>>",
            r#"{ "type" : "object" , "additionalProperties" : { "type" : "string" , "format" : "time" } }"#,
        ),
        (
            "HashMap<String, Vec<HashMap<String, DateTime<Utc>>>>",
            r#"{ "type" : "array" , "items" : { "type" : "object" , "additionalProperties" : { "type" : "string" , "format" : "date-time" } } }"#,
        ),
    ] {
        let tokens = map_field_schema(map_type).to_string();
        assert!(
            tokens.contains(&format!(r#""additionalProperties" : {expected}"#)),
            "for {map_type}, got: {tokens}"
        );
        assert!(
            !tokens.contains(r#""additionalProperties" : true"#),
            "for {map_type}, got: {tokens}"
        );
    }
}

/// The inner value types no source type produces reach the same mapping as the outer ones, whether
/// the inner map stands alone or behind a `Vec`.
#[cfg(feature = "jsonschema")]
#[test]
fn a_nested_string_literal_map_value_keeps_its_const() {
    let inner_member = r#"{ "type" : "object" , "additionalProperties" : { "type" : "string" , "const" : "Tixena" } }"#;
    let bare =
        nested_string_key_map_value_schema(FieldDefType::StringLiteral("Tixena".to_owned()), 0);
    assert!(
        bare.contains(&format!(r#""additionalProperties" : {inner_member}"#)),
        "got: {bare}"
    );

    let arrayed =
        nested_string_key_map_value_schema(FieldDefType::StringLiteral("Tixena".to_owned()), 1);
    assert!(
        arrayed.contains(&format!(
            r#""additionalProperties" : {{ "type" : "array" , "items" : {inner_member} }}"#
        )),
        "got: {arrayed}"
    );
}

/// A nested `ObjectId` member carries the `$oid` object the outer member carries.
#[cfg(all(feature = "object_id", feature = "jsonschema"))]
#[test]
fn a_nested_object_id_map_value_keeps_its_oid_object() {
    let inner_member = r#"{ "type" : "object" , "additionalProperties" : { "type" : "object" , "properties" : { "$oid" : (serde_json :: json ! ({ "type" : "string" , "pattern" : "^[a-f0-9]{24}$" })) } , "required" : ["$oid"] , "additionalProperties" : false } }"#;
    let bare = nested_string_key_map_value_schema(FieldDefType::ObjectId, 0);
    assert!(
        bare.contains(&format!(r#""additionalProperties" : {inner_member}"#)),
        "got: {bare}"
    );

    let arrayed = nested_string_key_map_value_schema(FieldDefType::ObjectId, 1);
    assert!(
        arrayed.contains(&format!(
            r#""additionalProperties" : {{ "type" : "array" , "items" : {inner_member} }}"#
        )),
        "got: {arrayed}"
    );
}

/// The rendering an enum-keyed map carries in every position, spelled out for a key of the given
/// name and a member of the given tokens.
#[cfg(feature = "jsonschema")]
fn enum_key_map_rendering(key_type_name: &str, member: &str) -> String {
    format!(
        r#"serde_json :: json ! ({{ "type" : "object" , "properties" : ({{ let value_schema = {member} ; let mut map_properties = serde_json :: Map :: new () ; for enum_key in {key_type_name} :: enum_members () {{ map_properties . insert (enum_key . to_string () , value_schema . clone ()) ; }} map_properties }}) , "additionalProperties" : false }})"#
    )
}

/// Which keys a map has is the key type's answer wherever the map is written, so an inner key that
/// enumerates its members enumerates them under an outer `String` key too — the position the map
/// sits in cannot decide whether its keys are known.
#[cfg(feature = "jsonschema")]
#[test]
fn a_nested_map_under_an_enumerating_key_expands_its_members() {
    let tokens = map_field_schema("HashMap<String, HashMap<Slot, String>>").to_string();
    let inner = enum_key_map_rendering("Slot", r#"serde_json :: json ! ({ "type" : "string" })"#);
    assert!(
        tokens.contains(&format!(r#""additionalProperties" : {inner}"#)),
        "got: {tokens}"
    );
    assert!(
        !tokens.contains(r#""additionalProperties" : true"#),
        "got: {tokens}"
    );
}

/// And under an outer key that enumerates too: each level asks its own key type, so a two-level map
/// spells both member sets out rather than stopping at the first.
#[cfg(feature = "jsonschema")]
#[test]
fn a_nested_map_under_two_enumerating_keys_expands_both_member_sets() {
    let tokens = map_field_schema("HashMap<Slot, HashMap<Bucket, u64>>").to_string();
    let inner =
        enum_key_map_rendering("Bucket", r#"serde_json :: json ! ({ "type" : "integer" })"#);
    assert!(
        tokens.contains(&enum_key_map_rendering("Slot", &inner)),
        "got: {tokens}"
    );
    assert!(
        !tokens.contains(r#""additionalProperties" : true"#),
        "got: {tokens}"
    );
}

/// The slot wraps sit outside the map's own rendering, as they do for every other member: a `Vec` of
/// enum-keyed maps is an array of the object each one describes as, and an `Option` admits `null`
/// beside it.
#[cfg(feature = "jsonschema")]
#[test]
fn a_wrapped_nested_enum_keyed_map_keeps_its_members_inside_the_slot_wrap() {
    let inner = enum_key_map_rendering("Slot", r#"serde_json :: json ! ({ "type" : "string" })"#);
    for (map_type, expected) in [
        (
            "HashMap<String, Vec<HashMap<Slot, String>>>",
            format!(r#"serde_json :: json ! ({{ "type" : "array" , "items" : {inner} }})"#),
        ),
        (
            "HashMap<String, Option<HashMap<Slot, String>>>",
            format!(r#"serde_json :: json ! ({{ "anyOf" : [{inner} , {{ "type" : "null" }}] }})"#),
        ),
    ] {
        let tokens = map_field_schema(map_type).to_string();
        assert!(
            tokens.contains(&format!(r#""additionalProperties" : {expected}"#)),
            "for {map_type}, got: {tokens}"
        );
    }
}

/// An enum-keyed map in a tuple slot is the same map, so it carries the same rendering: the slot
/// dispatch reaches the one emission rather than falling back to the open object.
#[cfg(feature = "jsonschema")]
#[test]
fn an_enum_keyed_map_tuple_element_expands_its_members() {
    let expected =
        enum_key_map_rendering("Slot", r#"serde_json :: json ! ({ "type" : "integer" })"#);
    for field_type in [
        "(String, HashMap<Slot, u32>)",
        "(String, HashMap<String, HashMap<Slot, u32>>)",
    ] {
        let tokens = tuple_field_schema(field_type);
        assert!(
            tokens.contains(&expected),
            "for {field_type}, got: {tokens}"
        );
        assert!(
            !tokens.contains(r#""additionalProperties" : true"#),
            "for {field_type}, got: {tokens}"
        );
    }
}

/// An inner key the registry positively rules out is rejected where the outer one is: reaching the
/// emitting path at depth resolves `enum_members()` through the alias onto a type that has no such
/// method, and rustc blames the attribute for a method the author never wrote.
#[cfg(feature = "jsonschema")]
#[test]
fn a_nested_map_key_known_to_lack_enum_members_names_the_requirement() {
    register_alias_info(
        "KeyAlias",
        "KeyAliasType",
        "key_alias_type_schema",
        AliasKind::NoEnumMembers,
    );
    for map_type in [
        "HashMap<String, HashMap<KeyAlias, String>>",
        "HashMap<Slot, HashMap<KeyAlias, String>>",
        "HashMap<String, Vec<HashMap<KeyAlias, String>>>",
    ] {
        let tokens = map_field_schema(map_type).to_string();
        assert!(
            !tokens.contains("KeyAlias :: enum_members"),
            "for {map_type}, got: {tokens}"
        );
        assert!(
            tokens.starts_with("compile_error !"),
            "for {map_type}, got: {tokens}"
        );
        assert!(
            !tokens.contains("properties . insert"),
            "for {map_type}, got: {tokens}"
        );
        assert!(
            tokens.contains("a map key must be a plain"),
            "for {map_type}, got: {tokens}"
        );
        assert!(tokens.contains("KeyAlias"), "for {map_type}, got: {tokens}");
    }
}

/// The guard reaches a tuple slot too, that being another position the map is dispatched through.
#[cfg(feature = "jsonschema")]
#[test]
fn a_tuple_element_map_key_known_to_lack_enum_members_names_the_requirement() {
    register_alias_info(
        "KeyAlias",
        "KeyAliasType",
        "key_alias_type_schema",
        AliasKind::NoEnumMembers,
    );
    let tokens = tuple_field_schema("(String, HashMap<KeyAlias, String>)");
    assert!(tokens.starts_with("compile_error !"), "got: {tokens}");
    assert!(
        tokens.contains("a map key must be a plain"),
        "got: {tokens}"
    );
    assert!(tokens.contains("field `t`"), "got: {tokens}");
}

/// The one emission answers for every position a map can sit in, so the object `HashMap<Slot, T>`
/// describes as in field position is the object it describes as nested under either key flavor and
/// in a tuple slot. Held against the field-position rendering, which is the one that has always
/// enumerated — depth cannot widen what the key already settled.
#[cfg(feature = "jsonschema")]
#[test]
fn an_enum_keyed_map_renders_the_same_in_every_position() {
    let expected =
        enum_key_map_rendering("Slot", r#"serde_json :: json ! ({ "type" : "string" })"#);
    assert!(
        map_field_schema("HashMap<Slot, String>")
            .to_string()
            .contains(&expected),
        "field position lost its enumeration"
    );
    for position in [
        map_field_schema("HashMap<String, HashMap<Slot, String>>").to_string(),
        map_field_schema("HashMap<Slot, HashMap<Slot, String>>").to_string(),
        tuple_field_schema("(String, HashMap<Slot, String>)"),
    ] {
        assert!(position.contains(&expected), "got: {position}");
    }
}

/// An inner key this expansion cannot narrow leaves the inner members open — the member is still
/// known to be an object, which is what the map guarantees, and it is what the same key states in
/// field position.
#[cfg(feature = "jsonschema")]
#[test]
fn a_nested_map_under_an_unenumerable_key_still_renders_as_an_object() {
    for map_type in [
        "HashMap<String, HashMap<u32, String>>",
        "HashMap<String, HashMap<Wrapper<Slot>, String>>",
    ] {
        let tokens = map_field_schema(map_type).to_string();
        assert!(
            tokens.contains(
                r#""additionalProperties" : { "type" : "object" , "additionalProperties" : true }"#
            ),
            "for {map_type}, got: {tokens}"
        );
    }
}

/// A value the mapping cannot render fails wherever it sits: nested behind a map, the tuple is
/// still a map value, and widening it to an open member would hide the rejection.
#[cfg(feature = "jsonschema")]
#[test]
fn an_unsupported_nested_map_value_emits_only_the_compile_error() {
    for map_type in [
        "HashMap<String, HashMap<String, (String, u32)>>",
        "HashMap<String, Vec<HashMap<String, (String, u32)>>>",
    ] {
        let tokens = map_field_schema(map_type).to_string();
        assert!(
            tokens.starts_with("compile_error !"),
            "for {map_type}, got: {tokens}"
        );
        assert!(
            !tokens.contains("properties . insert"),
            "for {map_type}, got: {tokens}"
        );
        assert!(
            tokens.contains("a tuple is not supported as a map value"),
            "for {map_type}, got: {tokens}"
        );
    }
}

/// The value half of a map type, parsed from the map type's source.
#[cfg(feature = "jsonschema")]
fn map_value_def(map_type: &str) -> super::FieldDef {
    let ty: syn::Type = syn::parse_str(map_type).unwrap();
    let field = super::get_field_def("m", &ty, "");
    let value = if let FieldDefType::Map(_, value) = &field.field_type {
        Some(value.as_ref().clone())
    } else {
        None
    };
    value.unwrap()
}

/// `Vec`-ness rides on the `FieldDef`, never in the type name: the parser collapses `Vec<T>` to
/// `T` with an array level counted onto it. A map value's type name is therefore the sibling's own
/// at every nesting, so a member schema predicated on a `Vec` type name can never fire.
#[cfg(feature = "jsonschema")]
#[test]
fn a_vec_sibling_map_value_parses_as_the_sibling_at_the_depth_it_is_written() {
    for (map_type, array_depth) in [
        ("HashMap<String, Inner>", 0_u8),
        ("HashMap<String, Vec<Inner>>", 1),
        ("HashMap<String, Vec<Vec<Inner>>>", 2),
    ] {
        let value = map_value_def(map_type);
        assert_eq!(value.array_depth, array_depth, "for {map_type}");
        assert!(
            matches!(&value.field_type, FieldDefType::SiblingType(name, args) if name == "Inner" && args.is_empty()),
            "for {map_type}, got: {:?}",
            value.field_type
        );
    }
}

/// A `String` key enumerates nothing, so the member schema is the value type's own — for a sibling
/// that is its schema module, arrayed when the value is a `Vec` and nullable when it is an
/// `Option`, exactly as the enum-key path binds its member.
#[cfg(feature = "jsonschema")]
#[test]
fn a_sibling_string_keyed_map_value_emits_the_sibling_schema() {
    for (map_type, expected) in [
        (
            "HashMap<String, Inner>",
            "inner_schema :: Schema :: json_schema_within (in_flight , hoisted_defs)",
        ),
        (
            "HashMap<String, Vec<Inner>>",
            r#"serde_json :: json ! ({ "type" : "array" , "items" : inner_schema :: Schema :: json_schema_within (in_flight , hoisted_defs) })"#,
        ),
        (
            "HashMap<String, Option<Inner>>",
            r#"serde_json :: json ! ({ "anyOf" : [inner_schema :: Schema :: json_schema_within (in_flight , hoisted_defs) , { "type" : "null" }] })"#,
        ),
        (
            "HashMap<String, Option<Vec<Inner>>>",
            r#"serde_json :: json ! ({ "anyOf" : [serde_json :: json ! ({ "type" : "array" , "items" : inner_schema :: Schema :: json_schema_within (in_flight , hoisted_defs) }) , { "type" : "null" }] })"#,
        ),
    ] {
        let tokens = map_field_schema(map_type).to_string();
        assert!(
            tokens.contains(&format!(r#""additionalProperties" : {expected}"#)),
            "for {map_type}, got: {tokens}"
        );
        assert!(
            !tokens.contains(r#""additionalProperties" : true"#),
            "for {map_type}, got: {tokens}"
        );
    }
}

/// A sibling's type arguments do not reach its schema, which lives on the wrapper itself, so a
/// generic value is the same schema-module reference a bare one is. Pinned on this key path as it
/// is on the enum-key one: the two share a dispatcher, and a narrowing that reintroduces the
/// divergence has to fail on the key path it is written for.
#[cfg(feature = "jsonschema")]
#[test]
fn a_generic_sibling_string_keyed_map_value_emits_the_sibling_schema() {
    let tokens = map_field_schema("HashMap<String, Wrapper<String>>").to_string();
    assert!(
        tokens.contains(
            r#""additionalProperties" : wrapper_schema :: Schema :: json_schema_within (in_flight , hoisted_defs)"#
        ),
        "got: {tokens}"
    );
}

/// A map is a `Map` wherever it is written, never a sibling named after the container: the parser
/// claims both 2-argument map idents ahead of the sibling fallback, and the wrappers a map can be
/// written under either collapse onto it or hold it as a value. The sibling dispatch therefore
/// renders no map of its own — a rendering there answers for nothing, and is free to drift from the
/// one the map arm states.
#[test]
fn a_map_never_parses_as_a_sibling_named_after_its_container() {
    for spelling in [
        "HashMap<String, u32>",
        "BTreeMap<String, u32>",
        "std::collections::BTreeMap<String, u32>",
        "Option<HashMap<String, u32>>",
        "Box<HashMap<String, u32>>",
        "Vec<BTreeMap<String, u32>>",
    ] {
        let ty: syn::Type = syn::parse_str(spelling).unwrap();
        let field_type = get_field_def("m", &ty, "").field_type;
        assert!(
            matches!(field_type, FieldDefType::Map(..)),
            "for {spelling}, got: {field_type:?}"
        );
    }
}

/// A renamed item's schema module is named after its exported name, which the raw ident does not
/// reproduce — the reference has to come from the registry or it names a module that was never
/// emitted.
#[cfg(feature = "jsonschema")]
#[test]
fn an_aliased_string_keyed_map_value_resolves_its_module_through_the_registry() {
    register_alias_info(
        "Inner",
        "InnerType",
        "inner_type_schema",
        AliasKind::NoEnumMembers,
    );
    for map_type in ["HashMap<String, Inner>", "HashMap<String, Vec<Inner>>"] {
        let tokens = map_field_schema(map_type).to_string();
        assert!(
            tokens.contains(
                "inner_type_schema :: Schema :: json_schema_within (in_flight , hoisted_defs)"
            ),
            "for {map_type}, got: {tokens}"
        );
    }
}

/// A value spelled as a wrapper around `u32`, built rather than parsed: the parser collapses a
/// `Vec` onto its element before any wrapper name is read, so the `Vec` spelling of a wrapper name
/// can only be put in front of a dispatch this way.
#[cfg(feature = "jsonschema")]
fn wrapped_u32_value(wrapper: &str) -> super::FieldDef {
    let element = super::get_field_def("", &syn::parse_quote!(u32), "");
    super::FieldDef {
        array_lengths: Vec::new(),
        docs: String::new(),
        field_type: FieldDefType::SiblingType(wrapper.to_owned(), vec![element]),
        array_depth: 0,
        model_schema_prop_meta: None,
        nullable_levels: Vec::new(),
        name: "items".to_owned(),
        absent_from_wire: false,
        omits_value: false,
        type_span: proc_macro2::Span::call_site(),
    }
}

/// The `Vec<u32>` value every wrapper spelling of the same thing is held against.
#[cfg(feature = "jsonschema")]
fn parsed_u32_vec_value() -> super::FieldDef {
    super::get_field_def("items", &syn::parse_quote!(Vec<u32>), "")
}

/// Every wrapper serde writes as a JSON array describes as the `Vec` of its element does, that
/// being the whole reason each is covered. One list answers for all of them, so none can fall
/// through to a schema module of its own — a module the expansion never emits.
#[cfg(feature = "jsonschema")]
#[test]
fn every_sequence_wrapper_describes_as_the_vec_of_its_element() {
    let expected = super::build_field_type_schema(&parsed_u32_vec_value(), "items").to_string();
    for wrapper in SEQUENCE_WRAPPERS {
        assert_eq!(
            super::build_field_type_schema(&wrapped_u32_value(wrapper), "items").to_string(),
            expected,
            "for: {wrapper}"
        );
    }
}

/// And in the two slot positions, where a value is dispatched instead of a field: a map member and
/// a tuple element each hold whatever the value writes, so each describes a covered wrapper as the
/// `Vec` of its element too — a slot reached by a name the field position covers alone is a slot
/// that renders an array on one surface and a schema module of its own on another.
#[cfg(feature = "jsonschema")]
#[test]
fn every_sequence_wrapper_describes_as_the_vec_of_its_element_in_a_slot() {
    let parsed = parsed_u32_vec_value();
    let expected_member = super::build_map_member_schema(&parsed).unwrap().to_string();
    let expected_element = super::build_tuple_element_json_schema(&parsed)
        .unwrap()
        .to_string();
    for wrapper in SEQUENCE_WRAPPERS {
        let value = wrapped_u32_value(wrapper);
        assert_eq!(
            super::build_map_member_schema(&value).unwrap().to_string(),
            expected_member,
            "for: {wrapper}"
        );
        assert_eq!(
            super::build_tuple_element_json_schema(&value)
                .unwrap()
                .to_string(),
            expected_element,
            "for: {wrapper}"
        );
    }
}

/// And in an untagged variant's member, the third position that dispatches a value. It reads the
/// wrappers through the seam the other positions read them through, so a member describes a covered
/// wrapper as the `Vec` of its element too — and none of them can name a schema module after a
/// wrapper, which is a module the expansion never declares and rustc reports at the member's type.
#[cfg(all(feature = "jsonschema", feature = "serde"))]
#[test]
fn every_sequence_wrapper_describes_as_the_vec_of_its_element_in_an_untagged_member() {
    let expected = super::field_json_schema_value(&parsed_u32_vec_value()).to_string();
    for wrapper in SEQUENCE_WRAPPERS {
        assert_eq!(
            super::field_json_schema_value(&wrapped_u32_value(wrapper)).to_string(),
            expected,
            "for: {wrapper}"
        );
    }
}

/// One value per arm of the untagged-member dispatch, labelled as it was written. Built at no array
/// level, so each holds exactly the tokens its arm emits before the array wrap sees them.
#[cfg(all(feature = "jsonschema", feature = "serde"))]
fn untagged_member_dispatch_values() -> Vec<(&'static str, super::FieldDef)> {
    let parsed: [(&'static str, syn::Type); 6] = [
        ("MetricTag", syn::parse_quote!(MetricTag)),
        ("String", syn::parse_quote!(String)),
        ("u32", syn::parse_quote!(u32)),
        ("f64", syn::parse_quote!(f64)),
        ("bool", syn::parse_quote!(bool)),
        ("serde_json::Value", syn::parse_quote!(serde_json::Value)),
    ];
    let mut values: Vec<(&'static str, super::FieldDef)> = parsed
        .iter()
        .map(|(label, ty)| (*label, super::get_field_def("items", ty, "")))
        .collect();

    let mut bounded = super::get_field_def("items", &syn::parse_quote!(String), "");
    bounded.model_schema_prop_meta = Some(ModelSchemaPropMeta {
        min_length: Some(2),
        pattern: Some("^[a-z]+$".to_owned()),
        ..Default::default()
    });
    values.push(("String under a bound", bounded));

    let mut literal = super::get_field_def("items", &syn::parse_quote!(String), "");
    literal.field_type = FieldDefType::StringLiteral("north".to_owned());
    values.push(("a string literal", literal));

    values.push((
        "HashMap<String, u32>",
        super::get_field_def("items", &syn::parse_quote!(HashMap<String, u32>), ""),
    ));
    values.push((
        "(i64, String)",
        super::get_field_def("items", &syn::parse_quote!((i64, String)), ""),
    ));
    #[cfg(feature = "object_id")]
    values.push((
        "ObjectId",
        super::get_field_def("items", &syn::parse_quote!(ObjectId), ""),
    ));
    #[cfg(feature = "chrono")]
    values.push((
        "NaiveDate",
        super::get_field_def("items", &syn::parse_quote!(NaiveDate), ""),
    ));

    values
}

/// Every arm of the untagged-member dispatch hands the array wrap a value the wrap can carry. The
/// wrap writes it into a `serde_json::json!` literal, where a value opening with a brace is read as
/// a JSON object rather than as a Rust block and the macro dies inside its own array expansion — so
/// an arm that opens with one is an arm no member holding it under a `Vec` can compile.
#[cfg(all(feature = "jsonschema", feature = "serde"))]
#[test]
fn every_untagged_member_value_is_one_the_array_wrap_can_carry() {
    for (label, value) in untagged_member_dispatch_values() {
        let tokens = super::field_json_schema_value(&value);
        let opens_a_block = matches!(
            tokens.clone().into_iter().next(),
            Some(proc_macro2::TokenTree::Group(group))
                if group.delimiter() == proc_macro2::Delimiter::Brace
        );
        assert!(!opens_a_block, "for: {label}, got: {tokens}");
    }
}

/// And the wrap carries each arm's own tokens through unchanged: the array level is written around
/// the value the arm emitted, with nothing reshaped at the wrap. An arm the wrap had to special-case
/// is one whose member rendering could drift from the field rendering built from the same tokens.
#[cfg(all(feature = "jsonschema", feature = "serde"))]
#[test]
fn the_array_wrap_carries_each_untagged_member_arms_own_tokens() {
    for (label, value) in untagged_member_dispatch_values() {
        let item = super::field_json_schema_value(&value).to_string();
        let mut arrayed = value.clone();
        arrayed.array_depth = 1;
        assert_eq!(
            super::field_json_schema_value(&arrayed).to_string(),
            format!("serde_json :: json ! ({{ \"type\" : \"array\" , \"items\" : {item} }})"),
            "for: {label}"
        );
    }
}

/// A sibling is carried by reference in every position that holds one, so the two slot positions
/// name one schema module and wrap it the same way — a tuple element that fell back to the open
/// object would admit values the same type in a map member rejects.
#[cfg(feature = "jsonschema")]
#[test]
fn a_sibling_slot_carries_the_schema_module_reference() {
    let spellings: [syn::Type; 4] = [
        syn::parse_quote!(MetricTag),
        syn::parse_quote!(Vec<MetricTag>),
        syn::parse_quote!(HashSet<MetricTag>),
        syn::parse_quote!(Option<BTreeSet<MetricTag>>),
    ];
    for value in &spellings {
        let parsed = super::get_field_def("tag", value, "");
        let element = super::build_tuple_element_json_schema(&parsed)
            .unwrap()
            .to_string();
        assert!(element.contains("metric_tag_schema"), "Got: {element}");
        assert_eq!(
            element,
            super::build_map_member_schema(&parsed).unwrap().to_string()
        );
    }
}

/// The `json_schema()` body a brand over `inner_ty` carries, read off the same dispatch the
/// branded expansion runs.
#[cfg(feature = "jsonschema")]
fn brand_json_schema_over(inner_ty: &syn::Type) -> String {
    super::build_branded_json_schema_method(
        &super::ModelSchemaArgs::default(),
        &super::branded_json_inner(&super::get_field_def("_inner", inner_ty, "")),
        "Wrapped",
    )
    .to_string()
}

/// A named type resolves to one schema module wherever it is written, and a brand is one of those
/// places: it carries its inner by the same reference a field carries it by, so what the module
/// name is derived from cannot differ between the two. A name the registry knows resolves through
/// the registry in both, and a name it does not know assumes the module that name's own
/// `#[model_schema()]` would publish — which for a type declared without the attribute is a module
/// nothing emits, and rustc reports the `E0433` in either position. Nothing the expansion holds can
/// tell those two apart, so that report is the whole contract, and it has to be one contract.
#[cfg(feature = "jsonschema")]
#[test]
fn a_named_type_resolves_to_the_same_module_in_field_and_brand_position() {
    register_alias_info(
        "Renamed",
        "RenamedType",
        "renamed_type_schema",
        AliasKind::NoEnumMembers,
    );
    for (name, module) in [
        ("Foreign", "foreign_schema"),
        ("Renamed", "renamed_type_schema"),
    ] {
        let inner_ty: syn::Type = syn::parse_str(name).unwrap();
        let reference =
            format!("{module} :: Schema :: json_schema_within (in_flight , hoisted_defs)");

        let field =
            super::build_field_type_schema(&super::get_field_def("id", &inner_ty, ""), "id")
                .to_string();
        assert!(field.contains(&reference), "for {name}, got: {field}");

        let brand = brand_json_schema_over(&inner_ty);
        assert!(brand.contains(&reference), "for {name}, got: {brand}");
    }
}

/// The JSON-schema insertion for the sole field of `source`, parsed from text so its spans carry
/// file locations and `source_text()` can report what they point at.
#[cfg(feature = "jsonschema")]
fn sole_field_json_schema(source: &str) -> proc_macro2::TokenStream {
    let item: syn::ItemStruct = syn::parse_str(source).unwrap();
    let field = item.fields.iter().next().unwrap();
    let field_name = field.ident.as_ref().unwrap().to_string();
    let def = get_field_def(&field_name, &field.ty, "");
    super::build_field_type_schema(&def, &field_name)
}

/// The source text each occurrence of the ident `name` points at, `None` for an occurrence carrying
/// no location.
#[cfg(feature = "jsonschema")]
fn ident_source_texts(tokens: &proc_macro2::TokenStream, name: &str) -> Vec<Option<String>> {
    let mut found = Vec::new();
    for tree in tokens.clone() {
        match &tree {
            proc_macro2::TokenTree::Group(group) => {
                found.extend(ident_source_texts(&group.stream(), name));
            }
            proc_macro2::TokenTree::Ident(ident) if ident == name => {
                found.push(tree.span().source_text());
            }
            proc_macro2::TokenTree::Ident(_)
            | proc_macro2::TokenTree::Punct(_)
            | proc_macro2::TokenTree::Literal(_) => {}
        }
    }
    found
}

/// A generated module reaches its siblings through `use super::*`, which a type declared inside a
/// function body never joins, and nothing the macro can read says whether it will resolve. So the
/// whole reference is spanned on the name the module was built from — the module ident included,
/// that being the one the `E0433` blames — and the failure is reported at the user's type instead
/// of at `#[model_schema()]`. Every position that carries a sibling names it the same way.
#[cfg(feature = "jsonschema")]
#[test]
fn a_sibling_reference_points_at_the_type_the_field_names() {
    for source in [
        "struct Outer { inner: Inner }",
        "struct Outer { inner: Vec<Inner> }",
        "struct Outer { inner: Box<Inner> }",
        "struct Outer { inner: HashMap<String, Inner> }",
        "struct Outer { inner: (Inner, u32) }",
    ] {
        let tokens = sole_field_json_schema(source);
        for named in ["inner_schema", "Schema", "json_schema_within"] {
            assert_eq!(
                ident_source_texts(&tokens, named),
                vec![Some("Inner".to_owned())],
                "for {source}, at `{named}`, got: {tokens}"
            );
        }
    }
}

/// The reference an item-scope sibling emits is the one it has always emitted — only the spans its
/// tokens carry are new.
#[cfg(feature = "jsonschema")]
#[test]
fn a_sibling_reference_emits_the_tokens_it_always_has() {
    assert_eq!(
        sole_field_json_schema("struct Outer { inner: Inner }").to_string(),
        "properties . insert (\"inner\" . to_string () , inner_schema :: Schema :: json_schema_within (in_flight , hoisted_defs)) ;"
    );
}

/// The registry is filled as items expand, so a key declared after the type that writes the map —
/// like a key foreign to this crate — reads as unclassified and keeps the emitting path. Its
/// `enum_members()` call is therefore spanned on the key the field names, so a key that carries no
/// such method is blamed at the user's type instead of at `#[model_schema()]`. Every position a
/// map is written in names the key the same way.
#[cfg(feature = "jsonschema")]
#[test]
fn an_enum_keyed_map_points_its_members_call_at_the_key_the_field_names() {
    for source in [
        "struct Outer { m: HashMap<Slot, String> }",
        "struct Outer { m: Vec<HashMap<Slot, String>> }",
        "struct Outer { m: HashMap<String, HashMap<Slot, String>> }",
        "struct Outer { m: (HashMap<Slot, String>, u32) }",
    ] {
        let tokens = sole_field_json_schema(source);
        for named in ["Slot", "enum_members"] {
            assert_eq!(
                ident_source_texts(&tokens, named),
                vec![Some("Slot".to_owned())],
                "for {source}, at `{named}`, got: {tokens}"
            );
        }
    }
}

/// The tuple-field insertion for a field type built by hand.
#[cfg(feature = "jsonschema")]
fn tuple_field_schema(field_type: &str) -> String {
    let ty: syn::Type = syn::parse_str(field_type).unwrap();
    super::build_field_type_schema(&super::get_field_def("t", &ty, ""), "t").to_string()
}

/// A tuple element reaching a map the dispatch cannot render fails the way the map field itself
/// does: one diagnostic naming the field and the type, in place of the whole insertion. An open
/// object left there would describe a field the expansion has already rejected.
#[cfg(feature = "jsonschema")]
#[test]
fn a_tuple_element_holding_an_unrenderable_map_emits_only_the_compile_error() {
    for field_type in [
        "(String, HashMap<String, (u32, u32)>)",
        "(String, Vec<HashMap<String, (u32, u32)>>)",
        "(String, (u32, HashMap<String, (u32, u32)>))",
    ] {
        let tokens = tuple_field_schema(field_type);
        assert!(
            tokens.starts_with("compile_error !"),
            "for {field_type}, got: {tokens}"
        );
        assert!(
            !tokens.contains("properties . insert"),
            "for {field_type}, got: {tokens}"
        );
        assert!(
            tokens.contains("field `t`"),
            "for {field_type}, got: {tokens}"
        );
        assert!(
            tokens.contains("a tuple is not supported as a map value"),
            "for {field_type}, got: {tokens}"
        );
    }
}

/// A sequence wrapper around a map is the field's array, not the map's, so the field position
/// applies the wrap every other field type applies — and the item it wraps is the map's own
/// rendering, unchanged. Without it the same type describes as a bare object in a field and as an
/// array in the slot positions that already wrap it, and the field schema rejects what serde writes.
#[cfg(feature = "jsonschema")]
#[test]
fn a_sequence_wrapped_map_field_describes_as_the_array_of_the_map_it_holds() {
    for (map_type, wrapped_type) in [
        ("HashMap<String, u64>", "Vec<HashMap<String, u64>>"),
        ("HashMap<String, u64>", "VecDeque<HashMap<String, u64>>"),
        ("HashMap<String, u64>", "Option<Vec<HashMap<String, u64>>>"),
        ("HashMap<Slot, u64>", "Vec<HashMap<Slot, u64>>"),
        (
            "HashMap<Slot, Vec<u64>>",
            "BTreeSet<HashMap<Slot, Vec<u64>>>",
        ),
        ("HashMap<u32, u64>", "Vec<HashMap<u32, u64>>"),
        (
            "HashMap<String, HashMap<Slot, u64>>",
            "Vec<HashMap<String, HashMap<Slot, u64>>>",
        ),
    ] {
        let item = inserted_field_value(map_type);
        assert_eq!(
            inserted_field_value(wrapped_type),
            format!(r#"serde_json :: json ! ({{ "type" : "array" , "items" : {item} }})"#),
            "for {wrapped_type}"
        );
    }
}

/// An `Option` the sequence holds is not the field's: the array is written either way, and the
/// `None` lands among its items — so the map's own rendering is what admits the `null`, one level
/// inside the array wrap rather than around it.
#[cfg(feature = "jsonschema")]
#[test]
fn a_sequence_of_optional_maps_admits_the_null_among_its_items() {
    let item = inserted_field_value("HashMap<String, u64>");
    assert_eq!(
        inserted_field_value("Vec<Option<HashMap<String, u64>>>"),
        format!(
            r#"serde_json :: json ! ({{ "type" : "array" , "items" : serde_json :: json ! ({{ "anyOf" : [{item} , {{ "type" : "null" }}] }}) }})"#
        )
    );
}

/// A map named without a sequence wrapper describes exactly as it always has, on every key path.
/// An `Option` around one is not such a wrapper: field position spells optionality by leaving the
/// name out of `required`, as it does for every other field type, so the map itself is untouched.
#[cfg(feature = "jsonschema")]
#[test]
fn an_unwrapped_map_field_keeps_the_object_it_has_always_described_as() {
    let string_keyed = r#"serde_json :: json ! ({ "type" : "object" , "additionalProperties" : { "type" : "integer" } })"#;
    for (map_type, expected) in [
        ("HashMap<String, u64>", string_keyed),
        ("BTreeMap<String, u64>", string_keyed),
        ("Option<HashMap<String, u64>>", string_keyed),
        (
            "HashMap<u32, u64>",
            r#"serde_json :: json ! ({ "type" : "object" , "additionalProperties" : true })"#,
        ),
        (
            "Option<HashMap<u32, u64>>",
            r#"serde_json :: json ! ({ "type" : "object" , "additionalProperties" : true })"#,
        ),
    ] {
        assert_eq!(inserted_field_value(map_type), expected, "for {map_type}");
    }
    assert_eq!(
        inserted_field_value("Option<HashMap<Slot, u64>>"),
        inserted_field_value("HashMap<Slot, u64>")
    );
}

/// A tuple element is a slot, and a slot spells the `$oid` object the way every other position
/// spells it. Pinned so the element cannot be handed a rendering of its own again.
#[cfg(all(feature = "object_id", feature = "jsonschema"))]
#[test]
fn an_object_id_tuple_element_spells_the_one_oid_object() {
    let parsed = super::get_field_def("id", &syn::parse_quote!(ObjectId), "");
    assert_eq!(
        super::build_tuple_element_json_schema(&parsed)
            .unwrap()
            .to_string(),
        r#"serde_json :: json ! ({ "type" : "object" , "properties" : { "$oid" : (serde_json :: json ! ({ "type" : "string" , "pattern" : "^[a-f0-9]{24}$" })) } , "required" : ["$oid"] , "additionalProperties" : false })"#
    );
}

/// A slot cannot be dropped the way an object key can, so a `None` in one is written as `null` —
/// and the wrapper the `None` stands around does not change that. The nullability belongs to the
/// slot, and survives the wrapper being normalized away.
#[cfg(feature = "jsonschema")]
#[test]
fn an_optional_sequence_wrapper_member_stays_nullable() {
    let mut optional_set = wrapped_u32_value("HashSet");
    optional_set.nullable_levels = vec![optional_set.array_depth];
    let mut optional_vec = parsed_u32_vec_value();
    optional_vec.nullable_levels = vec![optional_vec.array_depth];
    assert_eq!(
        super::build_map_member_schema(&optional_set)
            .unwrap()
            .to_string(),
        super::build_map_member_schema(&optional_vec)
            .unwrap()
            .to_string()
    );
}

/// Runs a fixed discriminated enum through the real collect/render pipeline, returning its
/// TypeScript member fragments, Zod member fragments, and JSON-schema member fragments.
///
/// Rebuilt from scratch on every call so repeated calls exercise whatever ordering the collection
/// stage imposes, not a single cached traversal.
fn rendered_discriminated_union() -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut item: syn::ItemEnum = syn::parse_quote! {
        enum Action {
            Upload { path: String },
            Generate,
            Delete(String),
            Rename { from: String, to: String },
            Move(String, String),
            Archive,
        }
    };
    let variants = collect_discriminated_variants(&mut item, None, Some("action_schema"));
    let rendered = render_discriminated_variants("type", "value", "Action", &variants.0);
    (
        rendered.0,
        rendered
            .1
            .into_iter()
            .map(|(schema_code, _optional)| schema_code)
            .collect(),
        rendered.2.iter().map(ToString::to_string).collect(),
    )
}

/// Union member order is semantic, not cosmetic: serde tries untagged members in declaration
/// order, so the emitted union must carry that same order on every surface.
#[test]
fn discriminated_union_members_follow_declaration_order() {
    let (ts_members, zod_members, json_members) = rendered_discriminated_union();
    assert_eq!(ts_members.len(), DECLARED_VARIANTS.len());
    assert_eq!(zod_members.len(), DECLARED_VARIANTS.len());
    assert_eq!(json_members.len(), DECLARED_VARIANTS.len());

    for (position, declared) in DECLARED_VARIANTS.iter().enumerate() {
        assert!(
            ts_members[position].contains(&format!("type: \"{declared}\";")),
            "TypeScript member {position} is not `{declared}`: {}",
            ts_members[position]
        );
        assert!(
            zod_members[position].contains(&format!("type: z.literal(\"{declared}\")")),
            "Zod member {position} is not `{declared}`: {}",
            zod_members[position]
        );
        #[cfg(feature = "jsonschema")]
        assert!(
            json_members[position].contains(&format!("\"const\" : \"{declared}\"")),
            "JSON-schema member {position} is not `{declared}`: {}",
            json_members[position]
        );
    }
}

/// Every per-variant collection feeding emission must be order-preserving. A hash-ordered one
/// reseeds per instance, so the same source expands to a different union on each build — which
/// this catches by rendering the same enum repeatedly inside one process.
#[test]
fn discriminated_union_rendering_is_stable_across_runs() {
    const RUNS: usize = 32;

    let first = rendered_discriminated_union();
    for run in 1..RUNS {
        assert_eq!(
            rendered_discriminated_union(),
            first,
            "run {run} rendered a different union than run 0"
        );
    }
}

/// The `validate()` contribution for a field spelled `spelling`, reached from `access`, as tokens.
#[cfg(feature = "serde")]
fn emitted_validation_from(spelling: &str, access: MemberAccess) -> String {
    let ty: syn::Type = syn::parse_str(spelling).unwrap();
    let shape = constrained_shape(&ty).unwrap();
    let field = proc_macro2::Ident::new("field", proc_macro2::Span::call_site());
    let checker = proc_macro2::Ident::new("check", proc_macro2::Span::call_site());
    build_field_validation(&shape.wraps, access, &field, &checker).to_string()
}

/// The `validate()` contribution for a struct's field spelled `spelling`, as tokens.
#[cfg(feature = "serde")]
fn emitted_validation(spelling: &str) -> String {
    emitted_validation_from(spelling, MemberAccess::SelfField)
}

/// The two positions differ in one place and nowhere else: where the checked value is reached.
/// Everything downstream of that — the walk through the wrappers, the check, the push — is the
/// same body, which is what makes a variant's member answer for its bound in a struct's words.
#[cfg(feature = "serde")]
#[test]
fn a_variant_member_is_reached_through_the_binding_its_arm_made() {
    for spelling in [
        "String",
        "u32",
        "Option<String>",
        "Vec<String>",
        "Option<Arc<[String]>>",
    ] {
        assert_eq!(
            emitted_validation_from(spelling, MemberAccess::VariantBinding),
            emitted_validation(spelling).replace("& self . field", "member_field"),
            "spelling {spelling}"
        );
    }
}

/// The one spelling whose emitted body predates the reach-through and must not move: anything else
/// would change what every already-generated bare field validates.
#[cfg(feature = "serde")]
#[test]
fn a_bare_field_is_checked_in_place() {
    assert_eq!(
        emitted_validation("String"),
        "if let Err (e) = check (& self . field) { errors . push (e) ; }"
    );
    assert_eq!(emitted_validation("u32"), emitted_validation("String"));
}

/// A constraint describes the value on the wire, and a `None` puts none there.
#[cfg(feature = "serde")]
#[test]
fn an_option_is_checked_inside_its_some() {
    assert_eq!(
        emitted_validation("Option<String>"),
        "{ let value_0 = & self . field ; if let Some (value_1) = value_0 { if let Err (e) = check (value_1) { errors . push (e) ; } } }"
    );
}

/// A transparent wrapper writes its inner value and nothing else, so reaching through it is a
/// deref and no check of its own.
#[cfg(feature = "serde")]
#[test]
fn a_transparent_wrapper_is_dereferenced_through() {
    let expected = "{ let value_0 = & self . field ; let value_1 = & * * value_0 ; if let Err (e) = check (value_1) { errors . push (e) ; } }";
    for spelling in ["Arc<str>", "Box<String>", "Cow<'a, str>", "Rc<str>"] {
        assert_eq!(
            emitted_validation(spelling),
            expected,
            "spelling {spelling}"
        );
    }
}

/// Every sequence spelling writes an array of its element, so each element answers for the
/// constraint — one level per depth, the innermost being where it lands.
#[cfg(feature = "serde")]
#[test]
fn a_sequence_is_checked_per_element() {
    let expected = "{ let value_0 = & self . field ; for value_1 in value_0 { if let Err (e) = check (value_1) { errors . push (e) ; } } }";
    for wrapper in SEQUENCE_WRAPPERS {
        assert_eq!(
            emitted_validation(&format!("{wrapper}<String>")),
            expected,
            "wrapper {wrapper}"
        );
    }
    assert_eq!(emitted_validation("[String ; 2]"), expected);

    assert_eq!(
        emitted_validation("Vec<Vec<String>>"),
        "{ let value_0 = & self . field ; for value_1 in value_0 { for value_2 in value_1 { if let Err (e) = check (value_2) { errors . push (e) ; } } } }"
    );
}

/// The wrappers compose in the order they were written, each reaching exactly one level.
#[cfg(feature = "serde")]
#[test]
fn mixed_wrappers_compose_in_written_order() {
    assert_eq!(
        emitted_validation("Option<Arc<[String]>>"),
        "{ let value_0 = & self . field ; if let Some (value_1) = value_0 { let value_2 = & * * value_1 ; for value_3 in value_2 { if let Err (e) = check (value_3) { errors . push (e) ; } } } }"
    );
}

/// The schema-module items emitted for a `minLength` field spelled `spelling`, as tokens.
#[cfg(feature = "serde")]
fn emitted_string_module(spelling: &str) -> String {
    let ty: syn::Type = syn::parse_str(spelling).unwrap();
    let shape = constrained_shape(&ty).unwrap();
    let meta = ModelSchemaPropMeta {
        min_length: Some(3),
        ..ModelSchemaPropMeta::default()
    };
    generate_string_validation_code(
        "field",
        &helper_name_stem("field", None),
        &meta,
        &shape,
        &ty,
        MemberAccess::SelfField,
    )
    .module_items
    .to_string()
}

/// The validator emitted for a `String` field constrained by `pattern`, as tokens — everything the
/// schema module holds ahead of the deserializer.
#[cfg(feature = "serde")]
fn emitted_pattern_validator(pattern: &str) -> String {
    let ty: syn::Type = syn::parse_str("String").unwrap();
    let meta = ModelSchemaPropMeta {
        pattern: Some(pattern.to_owned()),
        ..ModelSchemaPropMeta::default()
    };
    let module = generate_string_validation_code(
        "field",
        &helper_name_stem("field", None),
        &meta,
        &constrained_shape(&ty).unwrap(),
        &ty,
        MemberAccess::SelfField,
    )
    .module_items
    .to_string();
    module[..module.find("pub fn deserialize_field").unwrap()].to_owned()
}

/// Just the deserializer of [`emitted_string_module`], which is everything after the validator.
#[cfg(feature = "serde")]
fn emitted_string_deserializer(spelling: &str) -> String {
    let module = emitted_string_module(spelling);
    assert!(
        module.contains("pub fn deserialize_field"),
        "no deserializer emitted for {spelling}: {module}"
    );
    module[module.find("pub fn deserialize_field").unwrap()..].to_owned()
}

/// The one deserializer whose body predates the reach-through and must not move: it is what every
/// already-generated bare field is gated by.
#[cfg(feature = "serde")]
#[test]
fn a_bare_field_deserializes_the_constrained_value_itself() {
    assert_eq!(
        emitted_string_deserializer("String"),
        "pub fn deserialize_field < 'de , D > (deserializer : D) -> Result < String , D :: Error > \
         where D : serde :: Deserializer < 'de > , { use serde :: Deserialize ; \
         let s = String :: deserialize (deserializer) ? ; \
         validate_field_value (& s) . map_err (serde :: de :: Error :: custom) ? ; Ok (s) }"
    );

    let numeric_ty: syn::Type = syn::parse_str("u32").unwrap();
    let numeric_meta = ModelSchemaPropMeta {
        minimum: Some(5.0_f64),
        ..ModelSchemaPropMeta::default()
    };
    let numeric = generate_numeric_validation_code(
        "field",
        &helper_name_stem("field", None),
        "u32",
        &numeric_meta,
        &constrained_shape(&numeric_ty).unwrap(),
        &numeric_ty,
        MemberAccess::SelfField,
    )
    .module_items
    .to_string();
    assert!(
        numeric.ends_with(
            "pub fn deserialize_field < 'de , D > (deserializer : D) -> Result < u32 , D :: Error > \
             where D : serde :: Deserializer < 'de > , { use serde :: Deserialize ; \
             let v = u32 :: deserialize (deserializer) ? ; \
             validate_field_value (& v) . map_err (serde :: de :: Error :: custom) ? ; Ok (v) }"
        ),
        "bare numeric deserializer moved: {numeric}"
    );
}

/// A struct field names its helpers for the field alone — the spelling every already-generated
/// struct is gated by — while a variant's field names them for its variant too, which is what keeps
/// two variants naming one field from colliding in the single schema module that holds both.
#[cfg(feature = "serde")]
#[test]
fn a_variant_field_names_its_helpers_for_its_variant() {
    assert_eq!(helper_name_stem("note", None), "note");
    assert_eq!(helper_name_stem("note", Some("Upload")), "upload_note");
    assert_eq!(
        helper_name_stem("note", Some("DeleteForever")),
        "delete_forever_note"
    );
}

/// The sole field of `item`, run through the constraint generator under `constraint` as the field
/// of variant `One`, returning (`module_items`, `validate_body`, `guard_error`, injected attribute
/// count).
#[cfg(feature = "serde")]
fn generated_field_validation(
    item: &syn::ItemStruct,
    constraint: &ModelSchemaPropMeta,
) -> (bool, bool, Option<String>, usize) {
    let field = item.fields.iter().next().unwrap();
    let raw_field_ident = field
        .ident
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();
    let mut new_attrs = Vec::new();
    let (module_items, validate_body, guard_error) = generate_field_validation(
        field,
        Some("probe_schema"),
        &raw_field_ident,
        Some("One"),
        constraint,
        &mut new_attrs,
    );
    (
        module_items.is_some(),
        validate_body.is_some(),
        guard_error.map(|tokens| tokens.to_string()),
        new_attrs.len(),
    )
}

/// A length or range constraint spells three names from the field ident — the validator, the
/// deserializer, and the `validate()` accessor — so a slot that has no ident is refused before the
/// first of them is built, where the `Ident` made from the empty name used to abort the expansion.
/// A named field is reached by none of this and generates what it always has.
#[cfg(feature = "serde")]
#[test]
fn a_constraint_on_a_positional_field_is_refused_before_a_name_is_spelled() {
    let positional: syn::ItemStruct = syn::parse_quote! { struct Probe(String); };
    let named: syn::ItemStruct = syn::parse_quote! { struct Probe { note: String } };

    for constraint in [
        ModelSchemaPropMeta {
            min_length: Some(3),
            ..ModelSchemaPropMeta::default()
        },
        ModelSchemaPropMeta {
            maximum: Some(5.0_f64),
            ..ModelSchemaPropMeta::default()
        },
    ] {
        let (module_items, validate_body, guard_error, injected_attrs) =
            generated_field_validation(&positional, &constraint);
        let error = guard_error.unwrap();
        assert!(error.contains("compile_error"), "got: {error}");
        assert!(error.contains("tuple field"), "got: {error}");
        assert!(!module_items, "a refused slot generated helpers: {error}");
        assert!(!validate_body, "a refused slot reached validate(): {error}");
        assert_eq!(injected_attrs, 0, "a refused slot was given a serde hook");
    }

    let named_string = generated_field_validation(
        &named,
        &ModelSchemaPropMeta {
            min_length: Some(3),
            ..ModelSchemaPropMeta::default()
        },
    );
    assert_eq!(named_string, (true, true, None, 1));

    let unconstrained = generated_field_validation(&positional, &ModelSchemaPropMeta::default());
    assert_eq!(unconstrained, (false, false, None, 0));
}

/// The whole expansion is what a panicking `Ident` cost, so the enum the bug was found on must
/// come back as diagnostics — one per offending slot, and none for the slot that carries no
/// constraint.
#[cfg(feature = "serde")]
#[test]
fn a_constrained_tuple_variant_yields_diagnostics_rather_than_helpers() {
    let mut item: syn::ItemEnum = syn::parse_quote! {
        enum Probe {
            One(#[model_schema_prop(minLength = 3)] String),
            Two(#[model_schema_prop(minLength = 5)] String),
            Three(String),
        }
    };
    let variants = collect_discriminated_variants(&mut item, None, Some("probe_schema"));

    let validation_fns: Vec<String> = variants.1.iter().map(ToString::to_string).collect();
    assert!(validation_fns.is_empty(), "got: {validation_fns:?}");

    let errors: Vec<String> = variants.2.iter().map(ToString::to_string).collect();
    assert_eq!(errors.len(), 2, "got: {errors:?}");
    for error in &errors {
        assert!(error.contains("compile_error"), "got: {error}");
        assert!(error.contains("tuple field"), "got: {error}");
    }
}

/// A wrapped field is gated on the way in by the walk that gates it in `validate()`, run over the
/// field's own declared type — the one thing a hook attached to that field can answer for.
#[cfg(feature = "serde")]
#[test]
fn a_wrapped_field_deserializes_its_declared_type() {
    assert_eq!(
        emitted_string_deserializer("Option<String>"),
        "pub fn deserialize_field < 'de , D > (deserializer : D) -> Result < Option < String > , D :: Error > \
         where D : serde :: Deserializer < 'de > , \
         { fn deserialize_validated < 'de , D , T , F > (deserializer : D , check : F) -> Result < T , D :: Error > \
         where D : serde :: Deserializer < 'de > , T : serde :: Deserialize < 'de > , F : FnOnce (& T) -> Result < () , String > , \
         { use serde :: Deserialize ; let value = T :: deserialize (deserializer) ? ; \
         check (& value) . map_err (serde :: de :: Error :: custom) ? ; Ok (value) } \
         deserialize_validated (deserializer , | value_0 : & Option < String > | \
         { if let Some (value_1) = value_0 { validate_field_value (value_1) ? ; } Ok (()) }) }"
    );
}

/// The `validate()` walk for `spelling` rewritten into the ending the wire needs: the collected
/// violation becomes the answered one, and nothing else about the walk is touched.
#[cfg(feature = "serde")]
fn wire_walk_of(spelling: &str) -> String {
    let ty: syn::Type = syn::parse_str(spelling).unwrap();
    let shape = constrained_shape(&ty).unwrap();
    let field = proc_macro2::Ident::new("field", proc_macro2::Span::call_site());
    let checker = proc_macro2::Ident::new("validate_field_value", proc_macro2::Span::call_site());
    build_field_validation(&shape.wraps, MemberAccess::SelfField, &field, &checker)
        .to_string()
        .replace(
            "if let Err (e) = validate_field_value (",
            "validate_field_value (",
        )
        .replace(") { errors . push (e) ; }", ") ? ;")
        .trim_start_matches("{ let value_0 = & self . field ; ")
        .trim_end_matches(" }")
        .to_owned()
}

/// The walk inside the hook is the walk `validate()` runs — same reach, same bindings, same order,
/// differing only where it ends: a `Deserializer` answers with one error, so the wire walk stops at
/// the first violation instead of collecting every one.
#[cfg(feature = "serde")]
#[test]
fn the_wire_walk_is_the_validate_walk_shape_for_shape() {
    for spelling in [
        "Box<String>",
        "Vec<String>",
        "Cow<'static, str>",
        "Option<Vec<String>>",
        "Option<Arc<[String]>>",
        "Vec<Vec<String>>",
    ] {
        let walk = wire_walk_of(spelling);
        let deserializer = emitted_string_deserializer(spelling);
        assert!(
            deserializer.contains(&walk),
            "spelling {spelling} walks differently on the wire than in validate(): \
             expected {walk} within {deserializer}"
        );
    }
}

/// A lifetime the field spells is declared by the hook that returns that type: a free function is
/// handed none of the struct's generics. `'static` needs no declaration and gets none.
#[cfg(feature = "serde")]
#[test]
fn a_borrowed_field_type_carries_its_lifetime_into_the_hook() {
    assert!(
        emitted_string_deserializer("Cow<'a, str>").starts_with(
            "pub fn deserialize_field < 'de , 'a , D > (deserializer : D) -> Result < Cow < 'a , str > , D :: Error >"
        ),
        "{}",
        emitted_string_deserializer("Cow<'a, str>")
    );
    assert!(
        emitted_string_deserializer("Cow<'static, str>").starts_with(
            "pub fn deserialize_field < 'de , D > (deserializer : D) -> Result < Cow < 'static , str > , D :: Error >"
        ),
        "{}",
        emitted_string_deserializer("Cow<'static, str>")
    );

    let ty: syn::Type = syn::parse_str("Option<Cow<'a, Cow<'a, str>>>").unwrap();
    let lifetimes = constrained_shape(&ty).unwrap().lifetimes;
    assert_eq!(
        lifetimes.len(),
        1,
        "a lifetime spelled twice must still be declared once"
    );
}

/// serde reads a missing key for an `Option` as a `None` only while the field deserializes itself,
/// so the hook that replaces that reading is given the default which restores it — and only there.
#[cfg(feature = "serde")]
#[test]
fn only_an_outermost_option_without_a_default_gets_one_injected() {
    let defaulted = true;
    let plain = false;

    for spelling in [
        "Option<String>",
        "Option<Vec<String>>",
        "Box<Option<String>>",
        "Arc<Rc<Option<String>>>",
    ] {
        let ty: syn::Type = syn::parse_str(spelling).unwrap();
        let wraps = constrained_shape(&ty).unwrap().wraps;
        assert!(
            needs_injected_default(&wraps, plain),
            "{spelling} would answer a missing key with an error"
        );
        assert!(
            !needs_injected_default(&wraps, defaulted),
            "{spelling} already has a default of its own"
        );
    }

    for spelling in [
        "String",
        "Vec<Option<String>>",
        "Box<String>",
        "Box<Vec<Option<String>>>",
    ] {
        let ty: syn::Type = syn::parse_str(spelling).unwrap();
        let wraps = constrained_shape(&ty).unwrap().wraps;
        assert!(
            !needs_injected_default(&wraps, plain),
            "{spelling} has no optional key to restore"
        );
    }
}

/// A field with no value for a length or a range to describe emits nothing at all.
#[cfg(feature = "serde")]
#[test]
fn a_field_without_a_constrainable_value_has_no_shape() {
    for spelling in [
        "Tag",
        "Option<Tag>",
        "HashMap<String, String>",
        "(String, String)",
    ] {
        let ty: syn::Type = syn::parse_str(spelling).unwrap();
        assert!(
            constrained_shape(&ty).is_none(),
            "spelling {spelling} should reach no constrainable value"
        );
    }
}

/// The slots a tuple struct is read from, at the three arities the dispatch answers for. The empty
/// one has no declaration in this crate — `struct Nothing();` is refused by the lint table — so it
/// is read here, where the slots are built rather than parsed off an item.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn tuple_slots(spellings: &[&str]) -> Vec<super::FieldDef> {
    spellings
        .iter()
        .map(|spelling| get_field_def("", &syn::parse_str(spelling).unwrap(), ""))
        .collect()
}

/// The shape a struct declaring exactly the given slots publishes, none of them off the wire.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn whole_tuple(spellings: &[&str]) -> super::TupleStructShape {
    tuple_struct_shape(spellings.len(), tuple_slots(spellings))
}

/// One slot is the slot's own type — serde writes a newtype struct as that value alone — and every
/// other arity is the fixed tuple serde writes as an array.
#[cfg(feature = "typescript")]
#[test]
fn a_tuple_struct_describes_as_its_arity_in_typescript() {
    assert_eq!(tuple_struct_ts_body(&whole_tuple(&[])), "[]");
    assert_eq!(tuple_struct_ts_body(&whole_tuple(&["String"])), "string");
    assert_eq!(
        tuple_struct_ts_body(&whole_tuple(&["String", "u32"])),
        "[string, number]"
    );
}

/// [`a_tuple_struct_describes_as_its_arity_in_typescript`] for the Zod surface.
#[cfg(feature = "zod")]
#[test]
fn a_tuple_struct_describes_as_its_arity_in_zod() {
    assert_eq!(tuple_struct_zod_body(&whole_tuple(&[])), "z.tuple([])");
    assert_eq!(
        tuple_struct_zod_body(&whole_tuple(&["String"])),
        "z.string()"
    );
    assert_eq!(
        tuple_struct_zod_body(&whole_tuple(&["String", "u32"])),
        "z.tuple([z.string(), z.number().int()])"
    );
}

/// [`a_tuple_struct_describes_as_its_arity_in_typescript`] for the JSON-schema surface, whose
/// fixed array carries the arity as its own bounds.
#[cfg(feature = "jsonschema")]
#[test]
fn a_tuple_struct_describes_as_its_arity_in_json_schema() {
    let empty = tuple_struct_json_body("Nothing", &whole_tuple(&[])).to_string();
    assert!(empty.contains("prefixItems"), "Got: {empty}");
    assert!(empty.contains("minItems"), "Got: {empty}");

    let single = tuple_struct_json_body("Plain", &whole_tuple(&["String"])).to_string();
    assert!(single.contains("string"), "Got: {single}");
    assert!(!single.contains("prefixItems"), "Got: {single}");

    let pair = tuple_struct_json_body("Pair", &whole_tuple(&["String", "u32"])).to_string();
    assert!(pair.contains("prefixItems"), "Got: {pair}");
    assert!(pair.contains("maxItems"), "Got: {pair}");
}

/// The bare value is the *declared* arity's, not the described list's. Captured from serde: a
/// struct declaring two slots with the first one taken off the wire writes `["x"]` — a one-element
/// array, not the bare `"x"` a struct declaring one slot writes — so a described list that has
/// shrunk to one still describes as an array.
#[cfg(feature = "typescript")]
#[test]
fn a_slot_dropped_off_the_wire_leaves_the_tuple_an_array() {
    let shrunk = tuple_struct_shape(2, tuple_slots(&["String"]));
    assert_eq!(tuple_struct_ts_body(&shrunk), "[string]");
    assert_eq!(
        tuple_struct_ts_body(&tuple_struct_shape(1, tuple_slots(&["String"]))),
        "string"
    );
}

/// The same reading on the JSON surface, where the arity is written twice as its own bounds.
#[cfg(feature = "jsonschema")]
#[test]
fn a_slot_dropped_off_the_wire_shrinks_the_described_arity() {
    let shrunk = tuple_struct_json_body("Pair", &tuple_struct_shape(2, tuple_slots(&["u32"])));
    let rendered = shrunk.to_string();
    assert!(rendered.contains("prefixItems"), "Got: {rendered}");
    assert!(rendered.contains("1usize"), "Got: {rendered}");
    assert!(!rendered.contains("2usize"), "Got: {rendered}");
}

/// A two-slot struct whose second slot is written at the given spelling.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn slot_pair(spelling: &str) -> syn::ItemStruct {
    syn::parse_str(&format!(
        "struct Pair(String, #[serde({spelling})] Option<String>);"
    ))
    .unwrap()
}

/// Runs the slot refusal over that struct's second slot, with the declared arity supplied rather
/// than counted, so the lone-slot exemption can be read off the same declaration.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn slot_guard_result(item: &syn::ItemStruct, declared_slots: usize) -> Result<(), syn::Error> {
    let field = item.fields.iter().nth(1).unwrap();
    check_slot_wire_is_readable(
        field,
        1,
        declared_slots,
        "Pair",
        parse_serde_key_omission(&field.attrs),
    )
}

/// The refusal message for the slot at that spelling, and `None` where it is left alone.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn slot_refusal(spelling: &str, declared_slots: usize) -> Option<String> {
    slot_guard_result(&slot_pair(spelling), declared_slots)
        .err()
        .map(|err| err.to_string())
}

/// Captured from serde on `struct S(#[serde(...)] Option<String>, String)`: `skip_serializing`
/// alone writes `["x"]` and reads only `["s","x"]`, `skip_deserializing` alone writes `["s","x"]`
/// and reads only `["x"]`. The array serde writes is not an array serde reads, and a slot has no
/// optional spelling to describe both, so the declaration is refused.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_slot_dropped_from_one_direction_only_is_refused() {
    for (spelling, refused) in SLOT_OMISSION_SPELLINGS {
        let refusal = slot_refusal(spelling, 2);
        assert_eq!(refusal.is_some(), refused, "{spelling}: {refusal:?}");
        if let Some(message) = refusal {
            assert!(message.contains("slot 1"), "{spelling}: {message}");
            assert!(message.contains("`Pair`"), "{spelling}: {message}");
        }
    }
}

/// Captured from serde: a struct declaring exactly one slot writes and reads that slot's value
/// whatever the skip spellings say, so none of them has a wire to be refused for there.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_lone_slot_is_refused_for_no_spelling() {
    for (spelling, _) in SLOT_OMISSION_SPELLINGS {
        assert_eq!(slot_refusal(spelling, 1), None, "for: {spelling}");
    }
}

/// The variant a declaration's first (and here only) variant parses to.
fn declared_variant(declaration: &str) -> syn::Variant {
    let item: syn::ItemEnum = syn::parse_str(declaration).unwrap();
    item.variants.into_iter().next().unwrap()
}

/// Every refusal that declaration's variant earns, one per slot that earns one.
fn variant_slot_refusals(declaration: &str) -> Vec<String> {
    let variant = declared_variant(declaration);
    let variant_name = variant.ident.to_string();
    variant
        .fields
        .iter()
        .enumerate()
        .filter_map(|(index, field)| {
            check_variant_slot_wire_is_readable(
                field,
                index,
                &variant_name,
                "Wire",
                parse_serde_key_omission(&field.attrs),
            )
            .err()
            .map(|err| err.to_string())
        })
        .collect()
}

/// Captured from serde on `enum E { One(#[serde(...)] String, u32) }`: `skip_serializing` alone
/// writes `{"One":[7]}` and reads only `{"One":["s",7]}`, `skip_deserializing` alone writes
/// `{"One":["s",7]}` and reads only `{"One":[7]}`. What serde writes is not what serde reads, and
/// a slot has no optional spelling to describe both, so the declaration is refused.
#[test]
fn a_variant_slot_dropped_from_one_direction_only_is_refused() {
    for (spelling, refused) in SLOT_OMISSION_SPELLINGS {
        let refusals = variant_slot_refusals(&format!(
            "enum Wire {{ One(#[serde({spelling})] Option<String>, u32) }}"
        ));
        assert_eq!(
            refusals.len(),
            usize::from(refused),
            "{spelling}: {refusals:?}"
        );
        for message in refusals {
            assert!(message.contains("slot 0"), "{spelling}: {message}");
            assert!(message.contains("`One`"), "{spelling}: {message}");
            assert!(message.contains("`Wire`"), "{spelling}: {message}");
        }
    }
}

/// A named member of a struct variant is left alone by the same walk: its key is absent from one
/// payload and present in the other, which an optional key describes.
#[test]
fn a_named_variant_member_is_refused_for_no_spelling() {
    for (spelling, _) in SLOT_OMISSION_SPELLINGS {
        let refusals = variant_slot_refusals(&format!(
            "enum Wire {{ One {{ #[serde({spelling})] a: Option<String> }} }}"
        ));
        assert!(refusals.is_empty(), "{spelling}: {refusals:?}");
    }
}

/// The lone slot of a variant is asked the same question, which is where this seam parts from the
/// tuple-struct one. Captured: `One(#[serde(skip_serializing)] String)` writes the bare name
/// `"One"` and reads only `{"One":"s"}`, so the halves split a variant at every declared arity
/// where a newtype struct ignored them outright.
#[test]
fn a_lone_variant_slot_is_refused_for_the_same_spellings() {
    for (spelling, refused) in SLOT_OMISSION_SPELLINGS {
        let refusals = variant_slot_refusals(&format!(
            "enum Wire {{ One(#[serde({spelling})] Option<String>) }}"
        ));
        assert_eq!(
            refusals.len(),
            usize::from(refused),
            "{spelling}: {refusals:?}"
        );
    }
}

/// Captured from serde: a variant declaring one slot and taking it off the wire is written as a
/// unit variant — `"One"` externally, `{"type":"One"}` under a tag, `null` untagged — which are the
/// payloads a declared unit variant writes in the same three places.
#[test]
fn a_variant_taking_its_lone_slot_off_the_wire_publishes_a_unit() {
    for spelling in ["skip", "skip_serializing, skip_deserializing"] {
        let variant =
            declared_variant(&format!("enum Wire {{ One(#[serde({spelling})] String) }}"));
        assert_eq!(variant_wire_kind(&variant), VariantKind::Unit, "{spelling}");
    }
}

/// Every other declared arity keeps the kind it declared. Captured: a two-slot variant with one
/// slot off the wire writes `{"One":[7]}` and with both off writes `{"One":[]}` — a shorter array
/// and then an empty one, never the bare name a unit writes.
#[test]
fn every_other_declared_arity_keeps_the_kind_it_declared() {
    for (declaration, expected) in [
        (
            "enum Wire { One(#[serde(skip)] String, u32) }",
            VariantKind::TupleMultiple,
        ),
        (
            "enum Wire { One(#[serde(skip)] String, #[serde(skip)] u32) }",
            VariantKind::TupleMultiple,
        ),
        ("enum Wire { One(String) }", VariantKind::TupleSingle),
        (
            "enum Wire { One { #[serde(skip)] a: String } }",
            VariantKind::Named,
        ),
        ("enum Wire { One }", VariantKind::Unit),
    ] {
        assert_eq!(
            variant_wire_kind(&declared_variant(declaration)),
            expected,
            "for: {declaration}"
        );
    }
}

/// The member spelling an untagged variant is refused with, run over the kind it publishes.
#[cfg(feature = "serde")]
fn untagged_refusal(declaration: &str, members: &[super::FieldDef]) -> String {
    let variant = declared_variant(declaration);
    render_untagged_variant(&variant_wire_kind(&variant), &variant, members, "Wire")
        .map_or_else(|err| err.to_string(), |_| String::new())
}

/// Captured from serde: an untagged variant whose lone slot is off the wire writes and reads `null`
/// — the payload a declared unit variant writes there — so it takes the refusal a unit variant
/// takes rather than describing the value nothing carries.
#[cfg(feature = "serde")]
#[test]
fn an_untagged_variant_whose_lone_slot_is_dropped_is_refused_as_a_unit() {
    let refusal = untagged_refusal("enum Wire { One(#[serde(skip)] String) }", &[]);
    assert!(refusal.contains("is a unit variant"), "Got: {refusal}");
}

/// A refused tuple variant is named by the arity the author declared, not by the slots that reached
/// the wire: a slot dropped from the description is still a slot the union has no spelling for.
#[cfg(feature = "serde")]
#[test]
fn a_refused_untagged_tuple_variant_is_named_by_its_declared_arity() {
    let carried = get_field_def("_1", &syn::parse_str::<syn::Type>("u32").unwrap(), "");
    let refusal = untagged_refusal("enum Wire { One(#[serde(skip)] String, u32) }", &[carried]);
    assert!(
        refusal.contains("a tuple variant with 2 fields"),
        "Got: {refusal}"
    );
}

/// A path writes a string on the wire, which is the value the rendered constraint describes, so
/// every spelling of one reaches a leaf the checks can land on — the borrowed form included.
#[cfg(feature = "serde")]
#[test]
fn every_path_spelling_reaches_a_constrainable_value() {
    for spelling in [
        "PathBuf",
        "std::path::PathBuf",
        "Box<Path>",
        "Cow<'a, Path>",
        "Option<PathBuf>",
        "Vec<PathBuf>",
    ] {
        let ty: syn::Type = syn::parse_str(spelling).unwrap();
        let shape = constrained_shape(&ty).unwrap();
        assert!(
            matches!(shape.leaf, ConstraintLeaf::Path),
            "spelling {spelling} should reach the path leaf"
        );
    }
}

/// The path leaf changes what the validator is handed and nothing else: it takes the borrowed path
/// every wrap of the walk already ends at, and the checks read the string serde writes for it.
#[cfg(feature = "serde")]
#[test]
fn a_path_is_checked_through_its_lossy_rendering() {
    let module = emitted_string_module("PathBuf");
    assert!(
        module.starts_with(
            "pub fn validate_field_value (path : & std :: path :: Path) -> Result < () , String > \
             { let rendered = path . to_string_lossy () ; let value : & str = & rendered ;"
        ),
        "got: {module}"
    );
    assert!(
        module.contains("if value . len () < 3usize"),
        "the checks are the ones a string field is held to: {module}"
    );
}

/// A bare path field is declared as the owned form — the borrowed one is unsized — so that is what
/// its deserializer reads before the check runs.
#[cfg(feature = "serde")]
#[test]
fn a_bare_path_field_deserializes_the_owned_path() {
    assert_eq!(
        emitted_string_deserializer("PathBuf"),
        "pub fn deserialize_field < 'de , D > (deserializer : D) -> Result < std :: path :: PathBuf , D :: Error > \
         where D : serde :: Deserializer < 'de > , { use serde :: Deserialize ; \
         let s = std :: path :: PathBuf :: deserialize (deserializer) ? ; \
         validate_field_value (& s) . map_err (serde :: de :: Error :: custom) ? ; Ok (s) }"
    );
}

/// A wrapped path is read as its declared type and checked by the same walk a wrapped string is,
/// the deref of each wrapper landing on the borrowed path the validator takes.
#[cfg(feature = "serde")]
#[test]
fn a_wrapped_path_field_deserializes_its_declared_type() {
    assert_eq!(
        emitted_string_deserializer("Box<Path>"),
        emitted_string_deserializer("Box<str>").replace("Box < str >", "Box < Path >")
    );
}

/// A `pattern` a regex engine is avoidable work for is emitted as the `str` call
/// `clippy::trivial_regex` names for it, with a one-character needle as a `char` so the emitted
/// call also answers `clippy::single_char_pattern`. Both lints are denied by the crates this code
/// is written into, and neither has an edit available at the `#[model_schema]` attribute the
/// diagnostic lands on.
#[cfg(feature = "serde")]
#[test]
fn a_trivial_pattern_is_emitted_as_the_call_it_says_the_same_thing_as() {
    assert_eq!(
        emitted_pattern_validator("^/"),
        "pub fn validate_field_value (value : & str) -> Result < () , String > \
         { if ! value . starts_with ('/') { \
         return Err (format ! (\"'{}' does not match pattern '{}'\" , \"field\" , \"^/\")) ; } Ok (()) } "
    );
    assert_eq!(
        emitted_pattern_validator("^abc$"),
        "pub fn validate_field_value (value : & str) -> Result < () , String > \
         { if value != \"abc\" { \
         return Err (format ! (\"'{}' does not match pattern '{}'\" , \"field\" , \"^abc$\")) ; } Ok (()) } "
    );
}

/// A `pattern` of any real shape keeps the regex it has always been checked by, built once per
/// process, and the words it is turned away with do not move either way.
#[cfg(feature = "serde")]
#[test]
fn a_pattern_of_any_real_shape_keeps_its_regex() {
    assert_eq!(
        emitted_pattern_validator("^[a-z]+$"),
        "pub fn validate_field_value (value : & str) -> Result < () , String > \
         { { use std :: sync :: LazyLock ; \
         static RE : LazyLock < regex :: Regex > = LazyLock :: new (|| { regex :: Regex :: new (\"^[a-z]+$\") . unwrap () }) ; \
         if ! RE . is_match (value) { \
         return Err (format ! (\"'{}' does not match pattern '{}'\" , \"field\" , \"^[a-z]+$\")) ; } } Ok (()) } "
    );
}

/// The recording a merge reads the union's members off says which of them serde writes as something
/// other than an object, that being the one thing the spelling does not carry and the one thing a
/// merge has to know: an intersection built on such a member is an object joined to a scalar, which
/// no payload satisfies.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_scalar_union_member_is_recorded_as_the_type_serde_writes_it_as() {
    let mut item: syn::ItemEnum = syn::parse_quote! {
        enum Choice {
            Obj(Holder),
            Text(String),
            Count(u32),
            Many(Vec<Holder>),
        }
    };
    let (_, _, _, merge_parts, _, errors, _, _) =
        collect_untagged_members(&mut item, UNTAGGED_MODULE);
    assert!(errors.is_empty(), "got: {errors:?}");
    let recorded: Vec<(String, Option<&str>)> = merge_parts
        .iter()
        .map(|member| (member.branch_path(), member.non_object))
        .collect();
    assert_eq!(
        recorded,
        vec![
            ("1".to_owned(), None),
            ("2".to_owned(), Some("string")),
            ("3".to_owned(), Some("integer")),
            ("4".to_owned(), Some("array")),
        ]
    );
}

/// So an object flattening that union is refused where the field was written, in the words the
/// JSON-schema merge refuses the same declaration with. Before, the branch for the scalar member
/// was emitted as the object intersected with it and nothing said so.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn flattening_a_union_with_a_scalar_member_is_refused_naming_the_branch() {
    let error = recorded_union_flatten_error(
        "ScalarChoice",
        syn::parse_quote! {
            enum ScalarChoice {
                Obj(Holder),
                Text(String),
            }
        },
        &syn::parse_quote! { #[serde(flatten)] either: ScalarChoice },
    )
    .unwrap();
    assert!(error.contains("compile_error"), "got: {error}");
    assert!(
        error.contains(
            "`#[serde(flatten)]` of `ScalarChoice` writes a union member that is not an object"
        ),
        "got: {error}"
    );
    assert!(
        error.contains("its branch 2 describes a `string`"),
        "got: {error}"
    );
    assert!(
        error.contains("write the field as a named member so the value gets a key of its own"),
        "got: {error}"
    );
}

/// A member reached through a nesting is named by the trail that reaches it, which is the position
/// the JSON-schema merge names the same member by — the recording is multiplied out where that
/// merge descends, so the trail is what keeps the two answers the same sentence.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_nested_scalar_union_member_is_refused_by_its_trail() {
    recorded_union_flatten_error(
        "NestedInner",
        syn::parse_quote! {
            enum NestedInner {
                Obj(Holder),
                Text(String),
            }
        },
        &syn::parse_quote! { #[serde(flatten)] either: NestedInner },
    );
    let error = recorded_union_flatten_error(
        "NestedOuter",
        syn::parse_quote! {
            enum NestedOuter {
                Inner(NestedInner),
                Other(Holder),
            }
        },
        &syn::parse_quote! { #[serde(flatten)] either: NestedOuter },
    )
    .unwrap();
    assert!(
        error.contains("its branch 1.2 describes a `string`"),
        "got: {error}"
    );
}

/// An object flattening a union every member of which serde writes as an object is untouched, and
/// so is one naming a type the recording holds nothing for.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn flattening_a_union_of_objects_is_not_refused() {
    assert!(
        recorded_union_flatten_error(
            "ObjectChoice",
            syn::parse_quote! {
                enum ObjectChoice {
                    First(Holder),
                    Second(Other),
                }
            },
            &syn::parse_quote! { #[serde(flatten)] either: ObjectChoice },
        )
        .is_none()
    );
    let unrecorded: syn::Field = syn::parse_quote! { #[serde(flatten)] base: NeverRecorded };
    assert!(flatten_edge_guard_error(&unrecorded, "Host").is_none());
}

/// Records an untagged enum's members the way its own expansion does, then asks the flatten guard
/// what an object naming it would be told. Returns the rendered `compile_error!`, or `None` where
/// the merge has nothing to refuse.
#[cfg(all(feature = "serde", feature = "zod"))]
fn recorded_union_flatten_error(
    rust_ident: &str,
    mut item: syn::ItemEnum,
    field: &syn::Field,
) -> Option<String> {
    let (_, _, _, merge_parts, _, errors, _, _) =
        collect_untagged_members(&mut item, UNTAGGED_MODULE);
    assert!(errors.is_empty(), "got: {errors:?}");
    register_alias_info(
        rust_ident,
        rust_ident,
        &ident_schema_module_name(rust_ident),
        AliasKind::NoEnumMembers,
    );
    record_zod_union_members(rust_ident, &merge_parts);
    flatten_edge_guard_error(field, "Host").map(|error| error.to_string())
}

/// The branch trails one untagged enum's members are recorded at, beside what each is proved to
/// write, in declaration order.
#[cfg(all(feature = "serde", feature = "zod"))]
fn recorded_member_trails(mut item: syn::ItemEnum) -> Vec<(String, Option<&'static str>)> {
    let (_, _, _, merge_parts, _, errors, _, _) =
        collect_untagged_members(&mut item, UNTAGGED_MODULE);
    assert!(errors.is_empty(), "got: {errors:?}");
    merge_parts
        .iter()
        .map(|member| (member.branch_path(), member.non_object))
        .collect()
}

/// Registers a name carrying both answers a `#[model_schema()]` item's own expansion records for
/// it: what serde writes it as, and the JSON type keyword its published wire describes as where
/// that wire is proved to be no object.
#[cfg(all(feature = "serde", feature = "zod"))]
fn seed_registered_wire(rust_ident: &str, kind: AliasKind, wire: Option<&'static str>) {
    register_alias_info(
        rust_ident,
        rust_ident,
        &ident_schema_module_name(rust_ident),
        kind,
    );
    record_wire_leaves(
        rust_ident,
        &[WireLeaf {
            branch: Vec::new(),
            non_object: wire,
        }],
    );
}

/// A member reached through an `Option` is two choices and not one: serde writes the value's own
/// wire or writes nothing, and the JSON-schema merge descends into both — naming the value `n.1`
/// and the absence `n.2`. The recording carries the same two, so the merge that reads it names a
/// member by the position the other surface names the same member by.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn an_optional_union_member_is_recorded_as_its_value_beside_the_absence() {
    assert_eq!(
        recorded_member_trails(syn::parse_quote! {
            enum Choice {
                Obj(Holder),
                Maybe(Option<Holder>),
                Text(Option<String>),
            }
        }),
        vec![
            ("1".to_owned(), None),
            ("2.1".to_owned(), None),
            ("2.2".to_owned(), Some("null")),
            ("3.1".to_owned(), Some("string")),
            ("3.2".to_owned(), Some("null")),
        ]
    );
}

/// A member serde writes as an object is recorded exactly as it was: one entry at its own position,
/// with no level below it. The `Option` is what adds a level, and nothing else does.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_member_written_without_an_option_keeps_the_one_position_it_had() {
    assert_eq!(
        recorded_member_trails(syn::parse_quote! {
            enum Choice {
                Obj(Holder),
                Other(Second),
            }
        }),
        vec![("1".to_owned(), None), ("2".to_owned(), None)]
    );
}

/// So an object flattening a union with an optional member is refused where the field was written,
/// naming the null leaf in the words the JSON-schema merge names it with. The absence is no key
/// set: serde writes the object's own keys alone for it and then refuses to read those same keys
/// back, so no branch a multiplication could write describes the type.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn flattening_a_union_with_an_optional_member_is_refused_naming_the_null_leaf() {
    let error = recorded_union_flatten_error(
        "NullableChoice",
        syn::parse_quote! {
            enum NullableChoice {
                Obj(Holder),
                Maybe(Option<Holder>),
            }
        },
        &syn::parse_quote! { #[serde(flatten)] either: NullableChoice },
    )
    .unwrap();
    assert!(
        error.contains(
            "`#[serde(flatten)]` of `NullableChoice` writes a union member that is not an object"
        ),
        "got: {error}"
    );
    assert!(
        error.contains("its branch 2.2 describes a `null`"),
        "got: {error}"
    );
}

/// And an optional member whose value serde already writes as a scalar is named at the value's own
/// trail — the choice below the `Option`, which is where the merge descending the same document
/// stops first.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn an_optional_scalar_union_member_is_refused_below_the_option_it_was_written_under() {
    let error = recorded_union_flatten_error(
        "NullableScalarChoice",
        syn::parse_quote! {
            enum NullableScalarChoice {
                Obj(Holder),
                Maybe(Option<String>),
            }
        },
        &syn::parse_quote! { #[serde(flatten)] either: NullableScalarChoice },
    )
    .unwrap();
    assert!(
        error.contains("its branch 2.1 describes a `string`"),
        "got: {error}"
    );
}

/// A member that names another item is asked of the registry rather than left unanswered: the named
/// item recorded the JSON type keyword its own published document carries, which is the word the
/// other surface writes for the same member.
///
/// An object, a union and a name the registry cannot rule out are the three that stay unanswered,
/// and each keeps the emission it has always had.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_union_member_naming_a_registered_non_object_wire_is_recorded_as_that_wire() {
    seed_registered_wire("StringBrand", AliasKind::StringWire, Some("string"));
    seed_registered_wire("SwitchBrand", AliasKind::Stringified, Some("boolean"));
    seed_registered_wire("PlainEnum", AliasKind::EnumMembers, Some("string"));
    seed_registered_wire("NamedStruct", AliasKind::NoEnumMembers, None);
    seed_registered_wire("TaggedEnum", AliasKind::NoEnumMembers, None);
    seed_registered_wire("CountBrand", AliasKind::Stringified, Some("integer"));
    assert_eq!(
        recorded_member_trails(syn::parse_quote! {
            enum Choice {
                Named(NamedStruct),
                Slug(StringBrand),
                Switch(SwitchBrand),
                Hue(PlainEnum),
                Tagged(TaggedEnum),
                Count(CountBrand),
                Foreign(NeverRegistered),
            }
        }),
        vec![
            ("1".to_owned(), None),
            ("2".to_owned(), Some("string")),
            ("3".to_owned(), Some("boolean")),
            ("4".to_owned(), Some("string")),
            ("5".to_owned(), None),
            ("6".to_owned(), Some("integer")),
            ("7".to_owned(), None),
        ]
    );
}

/// So flattening a union whose member names a brand over a string is refused in the branch-naming
/// words, where before it emitted the object intersected with that brand — a branch no payload
/// satisfies, and one serde refuses to write for the same reason it refuses a directly flattened
/// brand.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn flattening_a_union_with_a_named_string_wire_member_is_refused_naming_the_branch() {
    seed_registered_wire("Slug", AliasKind::StringWire, Some("string"));
    let error = recorded_union_flatten_error(
        "SlugChoice",
        syn::parse_quote! {
            enum SlugChoice {
                Obj(Holder),
                Slug(Slug),
            }
        },
        &syn::parse_quote! { #[serde(flatten)] either: SlugChoice },
    )
    .unwrap();
    assert!(
        error.contains("its branch 2 describes a `string`"),
        "got: {error}"
    );
}

/// The same for a brand serde stringifies and for a plain unit enum, each named by the keyword its
/// own published document carries: a brand over a `bool` describes as a `boolean`, and a unit enum
/// describes as the `string` its member name is written as.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn flattening_a_union_with_a_named_stringified_or_enumerated_member_is_refused() {
    seed_registered_wire("Switch", AliasKind::Stringified, Some("boolean"));
    let switch = recorded_union_flatten_error(
        "SwitchChoice",
        syn::parse_quote! {
            enum SwitchChoice {
                Obj(Holder),
                Switch(Switch),
            }
        },
        &syn::parse_quote! { #[serde(flatten)] either: SwitchChoice },
    )
    .unwrap();
    assert!(
        switch.contains("its branch 2 describes a `boolean`"),
        "got: {switch}"
    );

    seed_registered_wire("Hue", AliasKind::EnumMembers, Some("string"));
    let hue = recorded_union_flatten_error(
        "HueChoice",
        syn::parse_quote! {
            enum HueChoice {
                Obj(Holder),
                Hue(Hue),
            }
        },
        &syn::parse_quote! { #[serde(flatten)] either: HueChoice },
    )
    .unwrap();
    assert!(
        hue.contains("its branch 2 describes a `string`"),
        "got: {hue}"
    );
}

/// A member naming an item the registry says publishes an object, and one naming a type the
/// registry has never seen, are both left alone — the second being the declaration-order fallback,
/// which answers for a name written above the union no differently than for a foreign type.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_union_member_naming_an_object_or_an_unregistered_type_stays_admitted() {
    seed_registered_wire("Doc", AliasKind::NoEnumMembers, None);
    assert!(
        recorded_union_flatten_error(
            "NamedChoice",
            syn::parse_quote! {
                enum NamedChoice {
                    Obj(Holder),
                    Doc(Doc),
                    Foreign(NeverRegistered),
                }
            },
            &syn::parse_quote! { #[serde(flatten)] either: NamedChoice },
        )
        .is_none()
    );
}

/// What the flatten guard is asked about a field naming one item directly, with no union between.
#[cfg(all(feature = "serde", feature = "zod"))]
fn direct_flatten_error(field: &syn::Field) -> Option<String> {
    flatten_edge_guard_error(field, "Host").map(|error| error.to_string())
}

/// The same refusal one position further out. A `#[serde(flatten)]` field naming an item whose own
/// published wire is no object is the shape the member-position refusal was landed to stop, with the
/// intersection written directly rather than through a union: serde refuses the value at runtime and
/// the JSON-schema merge refuses the declaration, so the guard names it in the words that merge uses
/// for a source at no position of its own.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn flattening_a_registered_scalar_wire_is_refused_where_the_field_was_written() {
    seed_registered_wire("Counted", AliasKind::NoEnumMembers, Some("integer"));
    let error = direct_flatten_error(&syn::parse_quote! { #[serde(flatten)] c: Counted }).unwrap();
    assert!(
        error.contains("`#[serde(flatten)]` of `Counted` is not written as an object"),
        "got: {error}"
    );
    assert!(
        error.contains("its schema describes a `integer`"),
        "got: {error}"
    );
    assert!(
        error.contains("write the field as a named member so the value gets a key of its own"),
        "got: {error}"
    );
}

/// Every keyword a registration can prove reaches the same refusal, each named by the word its own
/// published document carries — the array a fixed-arity tuple struct writes among them, which serde
/// refuses to flatten for the reason it refuses the scalar.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn flattening_a_registered_string_boolean_or_array_wire_is_refused_by_its_own_keyword() {
    for (rust_ident, kind, keyword) in [
        ("DirectSlug", AliasKind::StringWire, "string"),
        ("DirectSwitch", AliasKind::Stringified, "boolean"),
        ("DirectPair", AliasKind::NoEnumMembers, "array"),
    ] {
        seed_registered_wire(rust_ident, kind, Some(keyword));
        let named: syn::Type = syn::parse_str(rust_ident).unwrap();
        let error =
            direct_flatten_error(&syn::parse_quote! { #[serde(flatten)] v: #named }).unwrap();
        assert!(
            error.contains(&format!("its schema describes a `{keyword}`")),
            "got: {error}"
        );
    }
}

/// The three the direct position leaves exactly as they stand: an item the registry says publishes
/// an object, a name it has never seen — the declaration-order fallback, which answers for a source
/// written below the object no differently than for a foreign type — and an array of a proved
/// scalar, where the array is what the field wrote rather than anything the name proves.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_direct_flatten_of_an_object_an_unregistered_name_or_an_array_stays_admitted() {
    seed_registered_wire("DirectDoc", AliasKind::NoEnumMembers, None);
    seed_registered_wire("DirectCount", AliasKind::NoEnumMembers, Some("integer"));
    for admitted in [
        syn::parse_quote! { #[serde(flatten)] base: DirectDoc },
        syn::parse_quote! { #[serde(flatten)] base: NeverRegisteredDirectly },
        syn::parse_quote! { #[serde(flatten)] counts: Vec<DirectCount> },
    ] {
        let field: syn::Field = admitted;
        assert!(
            direct_flatten_error(&field).is_none(),
            "got a rejection for {}",
            quote::ToTokens::to_token_stream(&field)
        );
    }
}

/// A plain enum proves the same `string` and keeps the refusal written for it: those words name the
/// variant key serde writes into the object, which is what the author of that declaration acts on.
/// Two guards firing on one field would put two diagnostics on one line, each answering for the same
/// thing in different words.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_direct_flatten_of_a_plain_enum_is_left_to_the_guard_written_for_it() {
    seed_registered_wire("DirectHue", AliasKind::EnumMembers, Some("string"));
    let field: syn::Field = syn::parse_quote! { #[serde(flatten)] tone: DirectHue };
    assert!(direct_flatten_error(&field).is_none());
    let written = super::flattened_field_guard_error(&field, "Host")
        .map(|error| error.to_string())
        .unwrap();
    assert!(
        written.contains("a plain enum writes its"),
        "got: {written}"
    );
}

/// A registration publishing a choice reaches the same refusal, named by the branch its value sits
/// at rather than at no position at all — the wording the JSON-schema merge, which reads that same
/// choice back as a union, already refuses the declaration in.
///
/// The absence beside it is not what is refused: serde writes the object's own keys alone for it and
/// reads them back as the absent value, which is the branch the merge already writes. What no
/// spelling describes is the value, and one declaration cannot carry a branch that round-trips and a
/// branch no payload satisfies.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_direct_flatten_of_a_nullable_scalar_registration_is_refused_at_its_value_branch() {
    seed_slot_registration(&syn::parse_quote! { struct DirectMaybeCount(Option<i64>); });
    let field: syn::Field = syn::parse_quote! { #[serde(flatten)] c: DirectMaybeCount };
    let error = direct_flatten_error(&field).unwrap();
    assert!(
        error.contains(
            "`#[serde(flatten)]` of `DirectMaybeCount` writes a union member that is not an object"
        ),
        "got: {error}"
    );
    assert!(
        error.contains("its branch 1 describes a `integer`"),
        "got: {error}"
    );
    assert!(
        error.contains("write the field as a named member so the value gets a key of its own"),
        "got: {error}"
    );
}

/// Every keyword the value side can prove reaches that same refusal, each named by the word its own
/// published document carries — the array a nullable sequence writes among them, which is the one
/// the name proves rather than the one a field wrote around it.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_nullable_registration_is_refused_by_the_keyword_its_value_side_proves() {
    seed_slot_registration(&syn::parse_quote! { struct DirectMaybeName(Option<String>); });
    seed_slot_registration(&syn::parse_quote! { struct DirectMaybeFlag(Option<bool>); });
    seed_slot_registration(&syn::parse_quote! { struct DirectMaybeList(Option<Vec<String>>); });
    for (rust_ident, keyword) in [
        ("DirectMaybeName", "string"),
        ("DirectMaybeFlag", "boolean"),
        ("DirectMaybeList", "array"),
    ] {
        let named: syn::Type = syn::parse_str(rust_ident).unwrap();
        let error =
            direct_flatten_error(&syn::parse_quote! { #[serde(flatten)] v: #named }).unwrap();
        assert!(
            error.contains(&format!("its branch 1 describes a `{keyword}`")),
            "got: {error}"
        );
    }
}

/// And a registration whose value side is an object keeps the absence multiplication it was landed
/// with: nothing beside the `null` is proved to be no object, so both branches are ones serde writes
/// and reads back. A name publishing a `null` at no top level of its own is not this shape at all —
/// there the `null` sits under a choice serde matched a member on, which the member position already
/// answers for.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_direct_flatten_of_a_nullable_object_registration_stays_admitted() {
    seed_registered_wire("DirectPlainDoc", AliasKind::NoEnumMembers, None);
    seed_slot_registration(&syn::parse_quote! { struct DirectMaybeDoc(Option<DirectPlainDoc>); });
    let field: syn::Field = syn::parse_quote! { #[serde(flatten)] base: DirectMaybeDoc };
    assert!(
        direct_flatten_error(&field).is_none(),
        "got a rejection for {}",
        quote::ToTokens::to_token_stream(&field)
    );
}

/// Runs the registration a tuple struct's own expansion runs, so what the registry answers for the
/// name is what a declaration put there rather than a word written by hand.
#[cfg(all(feature = "serde", feature = "zod"))]
fn seed_slot_registration(item: &syn::ItemStruct) {
    let _: (String, String, syn::Ident) = super::struct_module_idents(
        &item.ident,
        None,
        super::tuple_struct_surface(
            &item.fields,
            &super::type_parameters_in_scope(&item.generics),
        ),
    );
}

/// The same for a branded newtype, whose wire is its inner's.
#[cfg(all(feature = "serde", feature = "zod"))]
fn seed_brand_registration(item: &syn::ItemStruct) {
    let rust_ident = item.ident.to_string();
    super::register_branded_newtype(
        item,
        &rust_ident,
        &rust_ident,
        &ident_schema_module_name(&rust_ident),
    );
}

/// One `u32` is one wire, and every spelling of it publishes the one JSON type keyword that wire
/// describes as. A field, the slot of a one-slot tuple struct and a brand all reach the same value,
/// so a keyword read off one of them and not the others is the same wire named twice — and the
/// merge that repeats the keyword cannot pick between two producers that disagree.
#[cfg(feature = "jsonschema")]
#[test]
fn every_spelling_of_one_value_publishes_one_json_type_keyword() {
    for (inner, keyword) in [
        ("u32", "integer"),
        ("i64", "integer"),
        ("usize", "integer"),
        ("f32", "number"),
        ("f64", "number"),
        ("bool", "boolean"),
        ("String", "string"),
    ] {
        let ty: syn::Type = syn::parse_str(inner).unwrap();
        let written = super::get_field_def("v", &ty, "");
        let named = format!("\"{keyword}\"");
        let field = super::build_field_type_schema(&written, "v").to_string();
        let slot = super::scalar_field_json_schema_item(&written)
            .unwrap()
            .to_string();
        let brand = brand_json_schema_over(&ty);
        assert!(field.contains(&named), "field {inner}, got: {field}");
        assert!(slot.contains(&named), "slot {inner}, got: {slot}");
        assert!(brand.contains(&named), "brand {inner}, got: {brand}");
    }
}

/// A member naming a brand over an integer is recorded as the `integer` that brand publishes, and
/// one naming a brand over a float as the `number` its own publishes. The two are one word to the
/// shape vocabulary and two documents on the wire, which is the disagreement that left both
/// unanswered.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_union_member_naming_a_numeric_registration_is_recorded_by_its_own_keyword() {
    seed_brand_registration(&syn::parse_quote! {
        #[serde(transparent)]
        struct WireTicks(u32);
    });
    seed_brand_registration(&syn::parse_quote! {
        #[serde(transparent)]
        struct WireRatio(f64);
    });
    seed_slot_registration(&syn::parse_quote! { struct WireSlotTicks(u32); });
    assert_eq!(
        recorded_member_trails(syn::parse_quote! {
            enum WireNumericChoice {
                Obj(Holder),
                Ticks(WireTicks),
                Ratio(WireRatio),
                Slot(WireSlotTicks),
            }
        }),
        vec![
            ("1".to_owned(), None),
            ("2".to_owned(), Some("integer")),
            ("3".to_owned(), Some("number")),
            ("4".to_owned(), Some("integer")),
        ]
    );
}

/// So flattening a union whose member names one is refused where the field was written, naming the
/// keyword the JSON-schema merge names the same member by.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn flattening_a_union_with_a_named_integer_wire_member_is_refused_naming_the_keyword() {
    seed_brand_registration(&syn::parse_quote! {
        #[serde(transparent)]
        struct FlatWireTicks(u32);
    });
    let error = recorded_union_flatten_error(
        "WireTickChoice",
        syn::parse_quote! {
            enum WireTickChoice {
                Obj(Holder),
                Ticks(FlatWireTicks),
            }
        },
        &syn::parse_quote! { #[serde(flatten)] either: WireTickChoice },
    )
    .unwrap();
    assert!(
        error.contains("its branch 2 describes a `integer`"),
        "got: {error}"
    );
}

/// An array-shaped registration and a map-shaped one are one word to the shape vocabulary and
/// opposite answers to the merge: serde flattens a map and writes an array as an array, which no
/// object can be merged with. Each is recorded as the keyword its own published document carries.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn an_array_shaped_registration_is_recorded_apart_from_a_map_shaped_one() {
    seed_slot_registration(&syn::parse_quote! { struct WireBag(Vec<String>); });
    seed_slot_registration(
        &syn::parse_quote! { struct WireBucket(std::collections::HashMap<String, String>); },
    );
    seed_slot_registration(&syn::parse_quote! { struct WirePair(String, u32); });
    assert_eq!(
        recorded_member_trails(syn::parse_quote! {
            enum WireContainerChoice {
                Obj(Holder),
                Bag(WireBag),
                Bucket(WireBucket),
                Pair(WirePair),
            }
        }),
        vec![
            ("1".to_owned(), None),
            ("2".to_owned(), Some("array")),
            ("3".to_owned(), None),
            ("4".to_owned(), Some("array")),
        ]
    );
}

/// So the array-shaped member is refused at the flatten site naming `array`, and the map-shaped one
/// stays admitted: serde writes a map's keys straight into the object, which is what flattening is.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn flattening_a_union_with_a_named_array_wire_member_is_refused_while_a_map_stays_admitted() {
    seed_slot_registration(&syn::parse_quote! { struct FlatWireBag(Vec<String>); });
    seed_slot_registration(
        &syn::parse_quote! { struct FlatWireBucket(std::collections::HashMap<String, String>); },
    );
    let bag = recorded_union_flatten_error(
        "WireBagChoice",
        syn::parse_quote! {
            enum WireBagChoice {
                Obj(Holder),
                Bag(FlatWireBag),
            }
        },
        &syn::parse_quote! { #[serde(flatten)] either: WireBagChoice },
    )
    .unwrap();
    assert!(
        bag.contains("its branch 2 describes a `array`"),
        "got: {bag}"
    );
    assert!(
        recorded_union_flatten_error(
            "WireBucketChoice",
            syn::parse_quote! {
                enum WireBucketChoice {
                    Obj(Holder),
                    Bucket(FlatWireBucket),
                }
            },
            &syn::parse_quote! { #[serde(flatten)] either: WireBucketChoice },
        )
        .is_none()
    );
}

/// A member naming a registration whose own published surface is nullable carries that surface's
/// null leaf, at the branch behind the name — the same two positions the member written
/// `Option<T>` is recorded at, one module further in.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_union_member_naming_a_nullable_registration_carries_its_null_leaf() {
    seed_slot_registration(&syn::parse_quote! { struct WireMaybeDoc(Option<Holder>); });
    seed_slot_registration(&syn::parse_quote! { struct WireMaybeCount(Option<u32>); });
    assert_eq!(
        recorded_member_trails(syn::parse_quote! {
            enum WireNullableChoice {
                Obj(Holder),
                Maybe(WireMaybeDoc),
                Count(WireMaybeCount),
            }
        }),
        vec![
            ("1".to_owned(), None),
            ("2.1".to_owned(), None),
            ("2.2".to_owned(), Some("null")),
            ("3.1".to_owned(), Some("integer")),
            ("3.2".to_owned(), Some("null")),
        ]
    );
}

/// So flattening a union whose member names one is refused in the words the directly written
/// `Option` member is refused in: serde writes the absent form and then refuses to read it back,
/// whether the null sits on the member or one name away.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn flattening_a_union_with_a_named_nullable_member_is_refused_naming_the_trail() {
    seed_slot_registration(&syn::parse_quote! { struct FlatWireMaybeDoc(Option<Holder>); });
    let named = recorded_union_flatten_error(
        "WireNamedNullableChoice",
        syn::parse_quote! {
            enum WireNamedNullableChoice {
                Obj(Holder),
                Maybe(FlatWireMaybeDoc),
            }
        },
        &syn::parse_quote! { #[serde(flatten)] either: WireNamedNullableChoice },
    )
    .unwrap();
    let written = recorded_union_flatten_error(
        "WireWrittenNullableChoice",
        syn::parse_quote! {
            enum WireWrittenNullableChoice {
                Obj(Holder),
                Maybe(Option<Holder>),
            }
        },
        &syn::parse_quote! { #[serde(flatten)] either: WireWrittenNullableChoice },
    )
    .unwrap();
    assert!(
        named.contains("its branch 2.2 describes a `null`"),
        "got: {named}"
    );
    assert_eq!(
        named.replace("WireNamedNullableChoice", "CHOICE"),
        written.replace("WireWrittenNullableChoice", "CHOICE")
    );
}

/// A member naming an externally tagged enum carries one leaf per variant, at the positions the
/// JSON-schema merge names the same variants by. serde writes a data-carrying variant as the
/// single-key object its name tags and writes a unit variant as that name alone — a bare string —
/// so the choice behind the member holds a leaf no object can be merged with, one level in from
/// where the member stands.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_union_member_naming_a_tagged_enum_carries_one_leaf_per_variant() {
    seed_external_registration(&syn::parse_quote! {
        enum WireExtBare {
            Bare,
            Wrapped(Holder),
        }
    });
    assert_eq!(
        recorded_member_trails(syn::parse_quote! {
            enum WireExtBareChoice {
                Obj(Holder),
                Ext(WireExtBare),
            }
        }),
        vec![
            ("1".to_owned(), None),
            ("2.1".to_owned(), Some("string")),
            ("2.2".to_owned(), None),
        ]
    );
}

/// And a tagged enum whose every variant carries data keeps the one unmarked leaf it always had.
/// Every branch of that choice is an object the merge joins, and the operand it would join is the
/// name whichever branch matched — so writing one member per branch would put three where one stood
/// and say nothing the single leaf did not.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_union_member_naming_an_all_object_tagged_enum_keeps_its_one_leaf() {
    seed_external_registration(&syn::parse_quote! {
        enum WireExtObjects {
            One(Holder),
            Two(Other),
        }
    });
    assert_eq!(
        recorded_member_trails(syn::parse_quote! {
            enum WireExtObjChoice {
                Obj(Holder),
                Ext(WireExtObjects),
            }
        }),
        vec![("1".to_owned(), None), ("2".to_owned(), None)]
    );
}

/// So flattening a union whose member names one is refused at the leaf the bare string sits at —
/// `2.1`, a position below the member, which is where the enum's own choice puts it and not where
/// the member stands — and in the words the JSON-schema merge refuses the same declaration in.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn flattening_a_union_with_a_tagged_enum_member_is_refused_naming_the_trail() {
    seed_external_registration(&syn::parse_quote! {
        enum FlatWireExtBare {
            Bare,
            Wrapped(Holder),
        }
    });
    let refusal = recorded_union_flatten_error(
        "WireExtFlatChoice",
        syn::parse_quote! {
            enum WireExtFlatChoice {
                Obj(Holder),
                Ext(FlatWireExtBare),
            }
        },
        &syn::parse_quote! { #[serde(flatten)] either: WireExtFlatChoice },
    )
    .unwrap();
    assert!(
        refusal
            .contains("`#[serde(flatten)]` of `WireExtFlatChoice` writes a union member that is"),
        "got: {refusal}"
    );
    assert!(
        refusal.contains("its branch 2.1 describes a `string`, which has no members to merge"),
        "got: {refusal}"
    );
    seed_external_registration(&syn::parse_quote! {
        enum FlatWireExtObjects {
            One(Holder),
            Two(Other),
        }
    });
    assert!(
        recorded_union_flatten_error(
            "WireExtObjFlatChoice",
            syn::parse_quote! {
                enum WireExtObjFlatChoice {
                    Obj(Holder),
                    Ext(FlatWireExtObjects),
                }
            },
            &syn::parse_quote! { #[serde(flatten)] either: WireExtObjFlatChoice },
        )
        .is_none()
    );
}

/// Runs the registration an externally tagged enum's own expansion runs, so the leaves the registry
/// answers with for the name are ones a declaration put there rather than words written by hand.
#[cfg(all(feature = "serde", feature = "zod"))]
fn seed_external_registration(item: &syn::ItemEnum) {
    let rust_ident = item.ident.to_string();
    let _: (String, syn::Ident) = super::enum_module_idents(
        &item.ident,
        &rust_ident,
        AliasKind::NoEnumMembers,
        super::Surface::externally_tagged(&item.variants),
    );
}

/// `^$` is the empty-string check, not a degenerate pattern: it pins both ends of the value to one
/// position. It keeps the `is_empty()` call it has been emitted as, byte for byte, now that the
/// shapes written out of the same two anchors are refused.
#[cfg(feature = "serde")]
#[test]
fn the_empty_string_pattern_keeps_the_call_it_was_already_emitted_as() {
    assert_eq!(
        emitted_pattern_validator("^$"),
        "pub fn validate_field_value (value : & str) -> Result < () , String > \
         { if ! value . is_empty () { \
         return Err (format ! (\"'{}' does not match pattern '{}'\" , \"field\" , \"^$\")) ; } Ok (()) } "
    );
}

/// `\b` is trivial to `clippy::trivial_regex` and names no `str` call, so it keeps the regex — and
/// the lint keeps firing on it in the consumer, which is the one case left standing here.
///
/// The verdict is from a probe, not an assumption: `#[model_schema_prop(pattern = r"\b")]` run
/// through `cargo clippy --all-targets -- -D warnings` in a crate denying `clippy::nursery`
/// reported `error: trivial regex ... the regex is unlikely to be useful as it is`, against the
/// `#[model_schema()]` attribute. It is left standing because `\b` turns a value away — the empty
/// string holds no word boundary — so there is a check here to keep, and answering the lint would
/// mean dropping it.
#[cfg(feature = "serde")]
#[test]
fn a_word_boundary_pattern_keeps_its_regex() {
    assert_eq!(
        emitted_pattern_validator(r"\b"),
        "pub fn validate_field_value (value : & str) -> Result < () , String > \
         { { use std :: sync :: LazyLock ; \
         static RE : LazyLock < regex :: Regex > = LazyLock :: new (|| { regex :: Regex :: new (\"\\\\b\") . unwrap () }) ; \
         if ! RE . is_match (value) { \
         return Err (format ! (\"'{}' does not match pattern '{}'\" , \"field\" , \"\\\\b\")) ; } } Ok (()) } "
    );
}
