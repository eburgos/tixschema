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
        tag: None,
        content: None,
        rename_all: Some("camelCase".to_owned()),
    };

    // Test field with explicit rename
    let field_meta_with_rename = SerdeFieldMeta {
        rename: Some("customName".to_owned()),
        skip: false,
        flatten: false,
    };
    assert_eq!(
        get_final_field_name("field_name", &field_meta_with_rename, &type_meta),
        "customName"
    );

    // Test field with rename_all
    let field_meta_no_rename = SerdeFieldMeta {
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
