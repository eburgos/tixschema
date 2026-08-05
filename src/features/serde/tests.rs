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

/// The walk reads every attribute in the list, wherever it sits. A `key = value` this parser has
/// no use for still has to be consumed: an unread value ends the walk on the comma after it, and
/// everything written past that point would go unseen — which is a field diagnosed by the
/// attributes someone happened to write first.
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

/// A list with nothing to step over is read exactly as it was, key for key.
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
