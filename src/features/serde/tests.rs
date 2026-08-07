use super::*;

#[test]
fn test_rename_all_transformations() {
    assert_eq!(apply_rename_all("user_name", Some("camelCase")), "userName");
    assert_eq!(
        apply_rename_all("first_name", Some("camelCase")),
        "firstName"
    );

    assert_eq!(
        apply_rename_all("user_name", Some("PascalCase")),
        "UserName"
    );

    assert_eq!(
        apply_rename_all("user_name", Some("kebab-case")),
        "user-name"
    );

    assert_eq!(apply_rename_all("user_name", None), "user_name");
}

#[test]
fn test_final_field_name() {
    let type_meta = SerdeTypeMeta {
        cfg_attr_rejection: None,
        tag: None,
        content: None,
        rename_all: Some("camelCase".to_owned()),
        untagged: false,
    };

    let field_meta_with_rename = SerdeFieldMeta {
        cfg_attr_rejection: None,
        rename: Some("customName".to_owned()),
        skip: false,
        flatten: false,
    };
    assert_eq!(
        get_final_field_name("field_name", &field_meta_with_rename, &type_meta),
        "customName"
    );

    let field_meta_no_rename = SerdeFieldMeta {
        cfg_attr_rejection: None,
        rename: None,
        skip: false,
        flatten: false,
    };
    assert_eq!(
        get_final_field_name("field_name", &field_meta_no_rename, &type_meta),
        "fieldName"
    );
}

#[test]
fn test_parse_flatten_field_attribute() {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct S {
            #[serde(flatten)]
            variant: SomeVariant,
        }
    };
    let field = item.fields.iter().next().unwrap();
    let meta = parse_serde_field_attributes(&field.attrs);
    assert!(meta.flatten);
    assert!(!meta.skip);
    assert!(meta.rename.is_none());
}

#[test]
fn test_parse_non_flatten_field_attribute() {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct S {
            #[serde(rename = "renamed")]
            foo: String,
        }
    };
    let field = item.fields.iter().next().unwrap();
    let meta = parse_serde_field_attributes(&field.attrs);
    assert!(!meta.flatten);
    assert_eq!(meta.rename.as_deref(), Some("renamed"));
}

#[test]
fn test_flatten_default_is_false() {
    let meta = SerdeFieldMeta::default();
    assert!(!meta.flatten);
}

fn field_meta(item: &syn::ItemStruct) -> SerdeFieldMeta {
    parse_serde_field_attributes(field_attrs(item))
}

fn field_attrs(item: &syn::ItemStruct) -> &[syn::Attribute] {
    &item.fields.iter().next().unwrap().attrs
}

#[test]
fn test_skip_deserializing_belongs_to_the_skip_lump() {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct S {
            #[serde(skip_deserializing)]
            note: Option<String>,
        }
    };
    assert!(field_meta(&item).skip);
}

#[test]
fn test_cfg_attr_wrapped_serde_field_attribute_is_rejected() {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct S {
            #[cfg_attr(feature = "serde", serde(rename = "renamed"))]
            foo: String,
        }
    };
    let meta = field_meta(&item);
    let message = meta.cfg_attr_rejection.unwrap().to_string();
    assert!(message.contains("cfg_attr"));
    assert!(message.contains("#[serde(...)]"));
    // The wrapper is precisely why the rename never reached the meta.
    assert!(meta.rename.is_none());
}

#[test]
fn test_cfg_attr_wrapped_serde_type_attribute_is_rejected() {
    let item: syn::ItemEnum = syn::parse_quote! {
        #[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
        enum E {
            FirstOne,
        }
    };
    let meta = parse_serde_type_attributes(&item.attrs);
    let message = meta.cfg_attr_rejection.unwrap().to_string();
    assert!(message.contains("cfg_attr"));
    assert!(message.contains("#[serde(...)]"));
    assert!(meta.rename_all.is_none());
}

#[test]
fn test_cfg_attr_without_serde_payload_is_accepted() {
    let item: syn::ItemStruct = syn::parse_quote! {
        #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
        struct S {
            #[cfg_attr(feature = "serde", doc = "only documented in serde builds")]
            foo: String,
        }
    };
    assert!(
        parse_serde_type_attributes(&item.attrs)
            .cfg_attr_rejection
            .is_none()
    );
    assert!(field_meta(&item).cfg_attr_rejection.is_none());
}

#[test]
fn test_plain_serde_attributes_carry_no_rejection() {
    let item: syn::ItemStruct = syn::parse_quote! {
        #[serde(rename_all = "camelCase")]
        struct S {
            #[serde(rename = "renamed")]
            foo: String,
        }
    };
    let type_meta = parse_serde_type_attributes(&item.attrs);
    let meta = field_meta(&item);
    assert_eq!(type_meta.rename_all.as_deref(), Some("camelCase"));
    assert!(type_meta.cfg_attr_rejection.is_none());
    assert_eq!(meta.rename.as_deref(), Some("renamed"));
    assert!(meta.cfg_attr_rejection.is_none());
}

#[test]
fn test_parse_untagged_type_attribute() {
    let item: syn::ItemEnum = syn::parse_quote! {
        #[serde(untagged)]
        enum E {
            S(String),
            N(i64),
        }
    };
    let meta = parse_serde_type_attributes(&item.attrs);
    assert!(meta.untagged);
    assert!(meta.tag.is_none());
    assert!(meta.content.is_none());
}

#[test]
fn test_untagged_default_is_false() {
    let meta = SerdeTypeMeta::default();
    assert!(!meta.untagged);
}

/// An unread `key = value` this parser has no use for still has to be consumed: it ends the walk
/// on the comma after it, and everything written past that point would go unseen.
#[test]
fn test_attributes_after_an_unread_value_are_still_read() {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct S {
            #[serde(skip_serializing_if = "Option::is_none", default, flatten, rename = "renamed")]
            note: Option<String>,
        }
    };
    let meta = field_meta(&item);
    assert!(meta.flatten, "flatten is written after an unread value");
    assert_eq!(
        meta.rename.as_deref(),
        Some("renamed"),
        "rename is written last of all"
    );
}

/// The type walk carries the same obligation as the field walk, and a heavier consequence: a tag
/// left unread describes a discriminated union as something else on every surface at once, while
/// serde goes on writing the tag.
#[test]
fn test_type_attributes_after_an_unread_value_are_still_read() {
    let item: syn::ItemEnum = syn::parse_quote! {
        #[serde(expecting = "an action", tag = "kind", content = "payload", rename_all = "camelCase")]
        enum E {
            FirstOne(String),
        }
    };
    let meta = parse_serde_type_attributes(&item.attrs);
    assert_eq!(
        meta.tag.as_deref(),
        Some("kind"),
        "tag is written after an unread value"
    );
    assert_eq!(meta.content.as_deref(), Some("payload"));
    assert_eq!(meta.rename_all.as_deref(), Some("camelCase"));
}

#[test]
fn test_untagged_after_an_unread_value_is_still_read() {
    let item: syn::ItemEnum = syn::parse_quote! {
        #[serde(bound = "T: Clone", untagged)]
        enum E<T> {
            FirstOne(T),
        }
    };
    assert!(parse_serde_type_attributes(&item.attrs).untagged);
}

/// Every value-carrying key the type walk ignores has to be survivable, not only the one that was
/// measured — otherwise which of them a type may be written with is a matter of luck.
#[test]
fn test_every_ignored_value_carrying_type_key_is_survived() {
    let expecting: syn::ItemEnum = syn::parse_quote! {
        #[serde(expecting = "an action", tag = "kind")]
        enum E { FirstOne(String) }
    };
    let bound: syn::ItemEnum = syn::parse_quote! {
        #[serde(bound = "T: Clone", tag = "kind")]
        enum E<T> { FirstOne(T) }
    };
    let serde_crate: syn::ItemEnum = syn::parse_quote! {
        #[serde(crate = "serde", tag = "kind")]
        enum E { FirstOne(String) }
    };
    let from: syn::ItemEnum = syn::parse_quote! {
        #[serde(from = "Other", tag = "kind")]
        enum E { FirstOne(String) }
    };
    let try_from: syn::ItemEnum = syn::parse_quote! {
        #[serde(try_from = "Other", tag = "kind")]
        enum E { FirstOne(String) }
    };
    let into: syn::ItemEnum = syn::parse_quote! {
        #[serde(into = "Other", tag = "kind")]
        enum E { FirstOne(String) }
    };
    for (key, item) in [
        ("expecting", &expecting),
        ("bound", &bound),
        ("crate", &serde_crate),
        ("from", &from),
        ("try_from", &try_from),
        ("into", &into),
    ] {
        assert_eq!(
            parse_serde_type_attributes(&item.attrs).tag.as_deref(),
            Some("kind"),
            "tag written after `{key} = ...` should still be read"
        );
    }
}

#[test]
fn test_type_attributes_without_unread_values_are_unchanged() {
    let item: syn::ItemEnum = syn::parse_quote! {
        #[serde(tag = "type", content = "data", rename_all = "camelCase")]
        enum E {
            FirstOne(String),
        }
    };
    let meta = parse_serde_type_attributes(&item.attrs);
    assert_eq!(meta.tag.as_deref(), Some("type"));
    assert_eq!(meta.content.as_deref(), Some("data"));
    assert_eq!(meta.rename_all.as_deref(), Some("camelCase"));
    assert!(!meta.untagged);
    assert!(meta.cfg_attr_rejection.is_none());
}

/// `bound(...)` has no dedicated branch, so it was always meant to fall through to
/// `consume_unread_value` — but the helper only knew how to step over `key = value`, not a
/// parenthesised list, so the tag written after it was lost.
#[test]
fn test_tag_after_a_list_form_bound_is_still_read() {
    let item: syn::ItemEnum = syn::parse_quote! {
        #[serde(bound(serialize = "T: Clone", deserialize = "T: Clone"), tag = "kind")]
        enum E<T> {
            A(T),
        }
    };
    let meta = parse_serde_type_attributes(&item.attrs);
    assert_eq!(
        meta.tag.as_deref(),
        Some("kind"),
        "tag is written after a list-form bound(...)"
    );
}

/// `rename_all` has a dedicated branch, and a list form only one direction is written in names no
/// single rule — only the tag written after it must survive, the same as the `bound(...)` case
/// above.
#[test]
fn test_tag_after_a_list_form_rename_all_is_still_read() {
    let item: syn::ItemEnum = syn::parse_quote! {
        #[serde(rename_all(serialize = "camelCase"), tag = "kind")]
        enum E {
            A(String),
        }
    };
    let meta = parse_serde_type_attributes(&item.attrs);
    assert_eq!(
        meta.rename_all, None,
        "one direction alone leaves the other at the name it would otherwise use"
    );
    assert_eq!(
        meta.tag.as_deref(),
        Some("kind"),
        "tag is written after a list-form rename_all(...)"
    );
}

/// `rename` has a dedicated branch too, and its list form must not swallow `flatten` written
/// after it.
#[test]
fn test_flatten_after_a_list_form_rename_is_still_read() {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct S {
            #[serde(rename(serialize = "ser", deserialize = "de"), flatten)]
            foo: String,
        }
    };
    let meta = field_meta(&item);
    assert_eq!(
        meta.rename, None,
        "two directions naming two keys leave no single name to record"
    );
    assert!(
        meta.flatten,
        "flatten is written after a list-form rename(...)"
    );
}

#[test]
fn test_list_form_rename_reads_the_name_both_directions_share() {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct S {
            #[serde(rename(serialize = "same_name", deserialize = "same_name"))]
            value: u32,
        }
    };
    assert_eq!(field_meta(&item).rename.as_deref(), Some("same_name"));
}

/// The two sub-keys are a set, not a sequence: the deserialize-first spelling names the same key.
#[test]
fn test_list_form_rename_reads_its_directions_in_either_order() {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct S {
            #[serde(rename(deserialize = "same_name", serialize = "same_name"))]
            value: u32,
        }
    };
    assert_eq!(field_meta(&item).rename.as_deref(), Some("same_name"));
}

#[test]
fn test_list_form_rename_all_reads_the_rule_both_directions_share() {
    let item: syn::ItemStruct = syn::parse_quote! {
        #[serde(rename_all(serialize = "camelCase", deserialize = "camelCase"))]
        struct S {
            my_field: u32,
        }
    };
    let meta = parse_serde_type_attributes(&item.attrs);
    assert_eq!(meta.rename_all.as_deref(), Some("camelCase"));
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn test_list_form_rename_naming_two_keys_is_refused() {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct S {
            #[serde(rename(serialize = "out_name", deserialize = "in_name"))]
            value: u32,
        }
    };
    let message = rename_direction_rejection(field_attrs(&item))
        .unwrap()
        .to_string();
    assert!(message.contains("`out_name` when serializing"), "{message}");
    assert!(
        message.contains("`in_name` when deserializing"),
        "{message}"
    );
    assert!(field_meta(&item).rename.is_none());
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn test_list_form_rename_all_naming_two_rules_is_refused() {
    let item: syn::ItemStruct = syn::parse_quote! {
        #[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
        struct S {
            my_field: u32,
        }
    };
    let message = rename_direction_rejection(&item.attrs).unwrap().to_string();
    assert!(
        message.contains("`camelCase` when serializing"),
        "{message}"
    );
    assert!(
        message.contains("`snake_case` when deserializing"),
        "{message}"
    );
    assert!(
        parse_serde_type_attributes(&item.attrs)
            .rename_all
            .is_none()
    );
}

/// serde was measured to leave the unwritten direction at the name it would otherwise use, so one
/// direction alone splits the two apart exactly as two different values do.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn test_list_form_rename_written_for_one_direction_only_is_refused() {
    let serializing: syn::ItemStruct = syn::parse_quote! {
        struct S {
            #[serde(rename(serialize = "out_name"))]
            value: u32,
        }
    };
    let written_out = rename_direction_rejection(field_attrs(&serializing))
        .unwrap()
        .to_string();
    assert!(
        written_out.contains("`out_name` when serializing only"),
        "{written_out}"
    );

    let deserializing: syn::ItemStruct = syn::parse_quote! {
        struct S {
            #[serde(rename(deserialize = "in_name"))]
            value: u32,
        }
    };
    let read_in = rename_direction_rejection(field_attrs(&deserializing))
        .unwrap()
        .to_string();
    assert!(
        read_in.contains("`in_name` when deserializing only"),
        "{read_in}"
    );
}

/// `bound(...)` writes the same two sub-keys and says nothing about the wire, so the walk that
/// reads them out of a renaming must not read them out of it.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn test_list_form_bound_earns_no_rename_refusal() {
    let item: syn::ItemStruct = syn::parse_quote! {
        #[serde(bound(serialize = "T: Clone", deserialize = "T: Clone"))]
        struct S<T> {
            #[serde(bound(serialize = "T: Clone", deserialize = "T: Clone"))]
            value: T,
        }
    };
    assert!(rename_direction_rejection(&item.attrs).is_none());
    assert!(rename_direction_rejection(field_attrs(&item)).is_none());
}
