use super::{FieldDefType, validate_as_number_flag, validate_ts_optional_flag};

#[cfg(feature = "serde")]
use super::{
    cfg_attr_guard_error, check_optional_field_serialization, collect_untagged_members,
    enum_cfg_attr_guard_errors, field_label, get_field_def, parse_serde_field_attributes,
    parse_serde_type_attributes,
};

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use super::{AliasKind, branded_guard_errors, register_alias_info};

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use syn::spanned::Spanned as _;

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
    check_optional_field_serialization(field, field_def.is_optional, &meta)
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
    let tokens = super::build_branded_json_schema_method(&args, "string");
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
fn alias_json_schema_stub_carries_no_cfg_attribute() {
    let tokens = super::generate_alias_json_schema_stub();
    assert_no_cfg_attribute(&tokens, "generate_alias_json_schema_stub");
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
    branded_guard_errors(item)
        .iter()
        .map(ToString::to_string)
        .collect()
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

/// The JSON schema statements a map-typed field expands to, parsed from the map type's source.
#[cfg(feature = "jsonschema")]
fn map_field_schema(map_type: &str) -> proc_macro2::TokenStream {
    let ty: syn::Type = syn::parse_str(map_type).unwrap();
    let field = super::get_field_def("m", &ty, "");
    let map_parts = if let FieldDefType::Map(key, value) = &field.field_type {
        Some((key, value))
    } else {
        None
    };
    let (key, value) = map_parts.unwrap();
    super::build_map_field_schema(key, value, "m")
}

/// A value type the enum-key branch cannot render must yield the `compile_error!` *instead of* the
/// per-member insertion loop: leaving the loop in place adds an E0425 on the `value_schema` the
/// failed arm never bound, and that second error names macro-internal state the author cannot act on.
#[cfg(feature = "jsonschema")]
#[test]
fn an_unsupported_enum_keyed_map_value_emits_only_the_compile_error() {
    for map_type in [
        "HashMap<Slot, HashMap<String, String>>",
        "HashMap<Slot, Wrapper<String>>",
        "HashMap<Slot, (String, u32)>",
    ] {
        assert_eq!(
            map_field_schema(map_type).to_string(),
            r#"compile_error ! ("Unsupported map value type") ;"#,
            "for {map_type}"
        );
    }
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
