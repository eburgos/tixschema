use super::*;

#[test]
fn test_rename_all_transformations() {
    // Test camelCase
    assert_eq!(apply_rename_all("user_name", Some("camelCase")), "userName");
    assert_eq!(
        apply_rename_all("first_name", Some("camelCase")),
        "firstName"
    );

    // Test PascalCase
    assert_eq!(
        apply_rename_all("user_name", Some("PascalCase")),
        "UserName"
    );

    // Test kebab-case
    assert_eq!(
        apply_rename_all("user_name", Some("kebab-case")),
        "user-name"
    );

    // Test no transformation
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

    // Test field with explicit rename
    let field_meta_with_rename = SerdeFieldMeta {
        cfg_attr_rejection: None,
        rename: Some("customName".to_owned()),
        skip: false,
        omits_none: false,
        flatten: false,
    };
    assert_eq!(
        get_final_field_name("field_name", &field_meta_with_rename, &type_meta),
        "customName"
    );

    // Test field with rename_all
    let field_meta_no_rename = SerdeFieldMeta {
        cfg_attr_rejection: None,
        rename: None,
        skip: false,
        omits_none: false,
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
fn test_omits_none_set_by_serialization_skips() {
    let skip: syn::ItemStruct = syn::parse_quote! {
        struct S {
            #[serde(skip)]
            note: Option<String>,
        }
    };
    let skip_serializing: syn::ItemStruct = syn::parse_quote! {
        struct S {
            #[serde(skip_serializing)]
            note: Option<String>,
        }
    };
    let skip_serializing_if: syn::ItemStruct = syn::parse_quote! {
        struct S {
            #[serde(skip_serializing_if = "Option::is_none")]
            note: Option<String>,
        }
    };
    assert!(field_meta(&skip).omits_none);
    assert!(field_meta(&skip_serializing).omits_none);
    assert!(field_meta(&skip_serializing_if).omits_none);
}

#[test]
fn test_omits_none_not_set_by_skip_deserializing() {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct S {
            #[serde(skip_deserializing)]
            note: Option<String>,
        }
    };
    let meta = field_meta(&item);
    assert!(!meta.omits_none);
    assert!(meta.skip);
}

#[test]
fn test_omits_none_default_is_false() {
    let meta = SerdeFieldMeta::default();
    assert!(!meta.omits_none);
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

/// A field's default is read in either spelling, since either one answers for a missing key.
#[test]
fn test_has_serde_default_reads_either_spelling() {
    let bare: syn::ItemStruct = syn::parse_quote! {
        struct S {
            #[serde(default)]
            note: Option<String>,
        }
    };
    let named: syn::ItemStruct = syn::parse_quote! {
        struct S {
            #[serde(default = "make_note")]
            note: Option<String>,
        }
    };
    let neither: syn::ItemStruct = syn::parse_quote! {
        struct S {
            #[serde(skip_serializing_if = "Option::is_none")]
            note: Option<String>,
        }
    };
    assert!(has_serde_default(field_attrs(&bare)));
    assert!(has_serde_default(field_attrs(&named)));
    assert!(!has_serde_default(field_attrs(&neither)));
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
    assert!(
        meta.omits_none,
        "skip_serializing_if is the attribute read first"
    );
    assert!(
        has_serde_default(field_attrs(&item)),
        "default is written after an unread value"
    );
    assert!(meta.flatten, "flatten is written after an unread value");
    assert_eq!(
        meta.rename.as_deref(),
        Some("renamed"),
        "rename is written last of all"
    );
}
