use super::{
    FieldDefType, check_os_string_field, collect_discriminated_variants, field_label,
    get_field_def, render_discriminated_variants, validate_as_number_flag,
    validate_ts_optional_flag,
};

#[cfg(feature = "serde")]
use super::{
    ConstraintLeaf, ModelSchemaPropMeta, build_field_validation, cfg_attr_guard_error,
    check_optional_field_serialization, collect_untagged_members, constrained_shape,
    enum_cfg_attr_guard_errors, generate_field_validation, generate_numeric_validation_code,
    generate_string_validation_code, helper_name_stem, internally_tagged_guard_errors,
    needs_injected_default, parse_serde_field_attributes, parse_serde_type_attributes,
};

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use super::{AliasKind, branded_guard_errors, register_alias_info};

#[cfg(feature = "typescript")]
use super::tuple_struct_ts_body;

#[cfg(feature = "zod")]
use super::tuple_struct_zod_body;

#[cfg(feature = "jsonschema")]
use super::tuple_struct_json_body;

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use syn::spanned::Spanned as _;

/// The variants of [`rendered_discriminated_union`]'s enum, in the order they are declared.
const DECLARED_VARIANTS: [&str; 6] = ["Upload", "Generate", "Delete", "Rename", "Move", "Archive"];

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
    let meta = parse_serde_field_attributes(&field.attrs);
    check_optional_field_serialization(field, field_def.is_optional(), &meta)
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

/// Collects the untagged-path guard failures as rendered `compile_error!` token strings.
#[cfg(feature = "serde")]
fn untagged_guard_errors(mut item: syn::ItemEnum) -> Vec<String> {
    collect_untagged_members(&mut item)
        .3
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

/// Runs the field walk the way [`super::process_field`] does and renders the guard failures its
/// `model_schema_prop` attributes earn, so an unparseable `pattern` is read off the same channel
/// that carries it to the emitted item.
fn field_pattern_errors(item: &syn::ItemStruct) -> Vec<String> {
    let field = item.fields.iter().next().unwrap();
    let name = field
        .ident
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();
    let field_def = get_field_def(&name, &field.ty, "");
    let meta = super::parse_model_schema_prop_attributes(&field.attrs);
    super::collect_field_guard_errors(
        field,
        &field_def,
        &name,
        meta.pattern_rejection.as_ref(),
        Vec::new(),
    )
    .iter()
    .map(ToString::to_string)
    .collect()
}

/// The guard's verdict is the `regex` crate's verdict: the parse the generated validator's
/// `Regex::new` would run, moved to expansion. Driving the expectation off `Regex::new` itself is
/// what keeps the two from drifting as the crate's grammar changes.
#[test]
fn the_field_pattern_guard_follows_the_regex_crate() {
    for pattern in PROBE_PATTERNS {
        let rejected = regex::Regex::new(pattern).is_err();
        let errors = field_pattern_errors(&syn::parse_quote! {
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
    let errors = field_pattern_errors(&syn::parse_quote! {
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

/// A field carrying no `pattern` at all must not acquire one of these errors.
#[test]
fn an_unpatterned_field_is_left_alone() {
    let errors = field_pattern_errors(&syn::parse_quote! {
        struct Report {
            #[model_schema_prop(minLength = 3)]
            name: String,
        }
    });
    assert!(errors.is_empty(), "got: {errors:?}");
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
    let tokens =
        super::build_struct_schema_example(Some(&"Report { id: 1 }".to_owned()), &name).unwrap();
    assert_no_cfg_attribute(&tokens, "build_struct_schema_example");
}

#[cfg(feature = "jsonschema")]
#[test]
fn branded_json_schema_method_carries_no_cfg_attribute() {
    let args = super::ModelSchemaArgs::default();
    let tokens = super::build_branded_json_schema_method(&args, "string", "DocumentId");
    assert_no_cfg_attribute(&tokens, "build_branded_json_schema_method");
}

#[cfg(feature = "zod")]
#[test]
fn branded_schema_example_carries_no_cfg_attribute() {
    let name: syn::Ident = syn::parse_quote!(DocumentId);
    let example = "DocumentId(\"abc\".to_string())".to_owned();
    for is_generic in [false, true] {
        let tokens = super::build_branded_schema_example(Some(&example), &name, is_generic);
        assert_no_cfg_attribute(&tokens, "build_branded_schema_example");
    }
}

#[cfg(feature = "typescript")]
#[test]
fn plain_enum_ts_definition_carries_no_cfg_attribute() {
    let tokens = super::generate_plain_enum_ts_definition_method(" * Status", "Status", "  'a'");
    assert_no_cfg_attribute(&tokens, "generate_plain_enum_ts_definition_method");
}

#[cfg(feature = "typescript")]
#[test]
fn discriminated_enum_ts_definition_carries_no_cfg_attribute() {
    let tokens =
        super::generate_discriminated_enum_ts_definition_method(" * Shape", "Shape", "  'a'");
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
    let ty: syn::Type = syn::parse_quote!(String);
    let field_def = super::get_field_def("AliasType", &ty, "");
    let tokens = super::generate_alias_zod_method("AliasType", &field_def);
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

/// The inners that carry the constraints faithfully. A `SiblingType` — another brand, an
/// unresolved user type, or a bare generic parameter — is admitted because expansion cannot know
/// its shape; the constrained path's `Display` assertion is what covers it.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn string_constraints_over_a_string_shaped_inner_pass() {
    for inner in ["String", "PathBuf", "ObjectId", "SomeOtherBrand", "T"] {
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

/// The guard reads the constraints, not the inner type: an unconstrained brand over any of the
/// rejected shapes is the shipped `no_display` contract and stays accepted.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn an_unconstrained_brand_over_a_non_string_inner_passes() {
    for inner in ["u64", "bool", "Vec<String>", "serde_json::Value"] {
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

/// The generic arm renders the type parameter and drops the `Option` wrapper outright, so the
/// shape is no more representable there than in the concrete case.
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
    super::enum_key_map_json_schema_value("Slot", &value)
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

/// An `ObjectId` is the one value the two key paths spell differently: a `String`-keyed member
/// carries a closed, unpatterned `$oid` object where an enum-keyed member carries the field-position
/// form. Pinned so the divergence stays a decision rather than a drift.
#[cfg(all(feature = "object_id", feature = "jsonschema"))]
#[test]
fn an_object_id_enum_keyed_map_value_keeps_the_field_position_oid_object() {
    let tokens = enum_key_map_value_binding(FieldDefType::ObjectId);
    assert!(
        tokens.contains(
            r#"let value_schema = serde_json :: json ! ({ "type" : "object" , "properties" : { "$oid" : { "type" : "string" , "pattern" : "^[a-f\\d]{24}$" } } , "required" : ["$oid"] }) ;"#
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
/// alias. `Vec<Slot>` is the collection, not the enum it holds; a target this expansion has not
/// seen registered is `Unknown`, which is not a negative.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn an_alias_registers_the_kind_of_what_it_targets() {
    register_alias_info("Slot", "Slot", "slot_schema", AliasKind::EnumMembers);
    register_alias_info("Doc", "Doc", "doc_schema", AliasKind::NoEnumMembers);
    for (target, expected) in [
        ("Slot", AliasKind::EnumMembers),
        ("Doc", AliasKind::NoEnumMembers),
        ("String", AliasKind::NoEnumMembers),
        ("u32", AliasKind::NoEnumMembers),
        ("Vec<Slot>", AliasKind::NoEnumMembers),
        ("HashMap<Slot, String>", AliasKind::NoEnumMembers),
        ("Wrapper<Slot>", AliasKind::NoEnumMembers),
        ("Ghost", AliasKind::Unknown),
    ] {
        let ty: syn::Type = syn::parse_str(target).unwrap();
        let kind = super::alias_target_kind(&super::get_field_def("AliasType", &ty, ""));
        assert_eq!(kind, expected, "for alias target {target}");
    }
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

/// A nested `ObjectId` member carries the closed `$oid` object the outer member carries.
#[cfg(all(feature = "object_id", feature = "jsonschema"))]
#[test]
fn a_nested_object_id_map_value_keeps_its_oid_object() {
    let inner_member = r#"{ "type" : "object" , "additionalProperties" : { "type" : "object" , "properties" : { "$oid" : { "type" : "string" } } , "required" : ["$oid"] , "additionalProperties" : false } }"#;
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

/// An alias's schema module is named after its registered export name, which the raw ident does
/// not reproduce — the reference has to come from the registry or it names a module that was never
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

/// The `ObjectId` divergence is frozen by position, so routing the tuple element through the map
/// path's dispatch must not migrate it onto the `String`-keyed member's closed, unpatterned `$oid`
/// object: the element keeps the patterned, open field-position form it has always carried.
#[cfg(all(feature = "object_id", feature = "jsonschema"))]
#[test]
fn an_object_id_tuple_element_keeps_the_field_position_oid_object() {
    let parsed = super::get_field_def("id", &syn::parse_quote!(ObjectId), "");
    assert_eq!(
        super::build_tuple_element_json_schema(&parsed)
            .unwrap()
            .to_string(),
        r#"serde_json :: json ! ({ "type" : "object" , "properties" : { "$oid" : { "type" : "string" , "pattern" : "^[a-f\\d]{24}$" } } , "required" : ["$oid"] })"#
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

/// The `validate()` contribution for a field spelled `spelling`, as tokens.
#[cfg(feature = "serde")]
fn emitted_validation(spelling: &str) -> String {
    let ty: syn::Type = syn::parse_str(spelling).unwrap();
    let shape = constrained_shape(&ty).unwrap();
    let field = proc_macro2::Ident::new("field", proc_macro2::Span::call_site());
    let checker = proc_macro2::Ident::new("check", proc_macro2::Span::call_site());
    build_field_validation(&shape.wraps, &field, &checker).to_string()
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
    )
    .module_items
    .to_string()
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
    build_field_validation(&shape.wraps, &field, &checker)
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

/// One slot is the slot's own type — serde writes a newtype struct as that value alone — and every
/// other arity is the fixed tuple serde writes as an array.
#[cfg(feature = "typescript")]
#[test]
fn a_tuple_struct_describes_as_its_arity_in_typescript() {
    assert_eq!(tuple_struct_ts_body(&tuple_slots(&[])), "[]");
    assert_eq!(tuple_struct_ts_body(&tuple_slots(&["String"])), "string");
    assert_eq!(
        tuple_struct_ts_body(&tuple_slots(&["String", "u32"])),
        "[string, number]"
    );
}

/// [`a_tuple_struct_describes_as_its_arity_in_typescript`] for the Zod surface.
#[cfg(feature = "zod")]
#[test]
fn a_tuple_struct_describes_as_its_arity_in_zod() {
    assert_eq!(tuple_struct_zod_body(&tuple_slots(&[])), "z.tuple([])");
    assert_eq!(
        tuple_struct_zod_body(&tuple_slots(&["String"])),
        "z.string()"
    );
    assert_eq!(
        tuple_struct_zod_body(&tuple_slots(&["String", "u32"])),
        "z.tuple([z.string(), z.number().int()])"
    );
}

/// [`a_tuple_struct_describes_as_its_arity_in_typescript`] for the JSON-schema surface, whose
/// fixed array carries the arity as its own bounds.
#[cfg(feature = "jsonschema")]
#[test]
fn a_tuple_struct_describes_as_its_arity_in_json_schema() {
    let empty = tuple_struct_json_body("Nothing", &tuple_slots(&[])).to_string();
    assert!(empty.contains("prefixItems"), "Got: {empty}");
    assert!(empty.contains("minItems"), "Got: {empty}");

    let single = tuple_struct_json_body("Plain", &tuple_slots(&["String"])).to_string();
    assert!(single.contains("string"), "Got: {single}");
    assert!(!single.contains("prefixItems"), "Got: {single}");

    let pair = tuple_struct_json_body("Pair", &tuple_slots(&["String", "u32"])).to_string();
    assert!(pair.contains("prefixItems"), "Got: {pair}");
    assert!(pair.contains("maxItems"), "Got: {pair}");
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
