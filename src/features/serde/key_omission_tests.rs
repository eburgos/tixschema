//! The omission walk is compiled in every build, so its tests are too — nothing here may reach
//! anything the `serde` feature gates.

use super::{SerdeKeyOmission, has_serde_default, parse_serde_key_omission};

fn field_attrs(item: &syn::ItemStruct) -> &[syn::Attribute] {
    &item.fields.iter().next().unwrap().attrs
}

fn omission(item: &syn::ItemStruct) -> SerdeKeyOmission {
    parse_serde_key_omission(field_attrs(item))
}

/// The three spellings that leave the key out of the output, and what each one costs the read
/// side: only a bare `skip` also stops serde looking for the key.
#[test]
fn test_key_omission_set_by_serialization_skips() {
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
    assert!(omission(&skip).omits_key);
    assert!(omission(&skip).skips_deserializing);
    assert!(omission(&skip_serializing).omits_key);
    assert!(!omission(&skip_serializing).skips_deserializing);
    assert!(omission(&skip_serializing_if).omits_key);
    assert!(!omission(&skip_serializing_if).skips_deserializing);
}

#[test]
fn test_key_omission_default_is_false() {
    let omission = SerdeKeyOmission::default();
    assert!(!omission.omits_key);
    assert!(!omission.skips_deserializing);
    assert!(!omission.defaulted);
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

/// `skip_deserializing` stops serde reading the field but never suppresses the key on the way
/// out, so it is the one member of the `skip` lump that omits nothing.
#[test]
fn test_key_omission_not_set_by_skip_deserializing() {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct S {
            #[serde(skip_deserializing)]
            note: Option<String>,
        }
    };
    assert!(!omission(&item).omits_key);
    assert!(omission(&item).skips_deserializing);
}

/// The walk reads every attribute in the list, wherever it sits. A `key = value` this parser has
/// no use for still has to be consumed: an unread value ends the walk on the comma after it, and
/// everything written past that point would go unseen — which is a key read or not read according
/// to the order someone happened to write the attributes in.
#[test]
fn test_omission_keys_after_an_unread_value_are_still_read() {
    let item: syn::ItemStruct = syn::parse_quote! {
        struct S {
            #[serde(skip_serializing_if = "Option::is_none", default)]
            note: Option<String>,
        }
    };
    assert!(
        omission(&item).omits_key,
        "skip_serializing_if is the attribute read first"
    );
    assert!(
        omission(&item).defaulted,
        "default is written after an unread value"
    );
}
