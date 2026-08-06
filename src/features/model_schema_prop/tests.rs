use super::*;
use syn::parse_quote;

#[test]
fn test_parse_empty_attributes() {
    let attrs: Vec<Attribute> = vec![];
    let meta = parse_model_schema_prop_attributes(&attrs);
    assert!(meta.as_type.is_none());
    assert!(meta.literal.is_none());
    assert!(meta.min_length.is_none());
}

#[test]
fn test_parse_as_type() {
    let attr: Attribute = parse_quote! { #[model_schema_prop(as = String)] };
    let meta = parse_model_schema_prop_attributes(&[attr]);
    assert!(meta.as_type.is_some());
    assert_eq!(meta.as_type.unwrap(), parse_quote!(String));
    assert!(meta.literal.is_none());
    assert!(meta.min_length.is_none());
}

#[test]
fn test_parse_literal() {
    let attr: Attribute = parse_quote! { #[model_schema_prop(literal = "Tixena")] };
    let meta = parse_model_schema_prop_attributes(&[attr]);
    assert!(meta.as_type.is_none());
    assert_eq!(meta.literal, Some(LiteralValue::Str("Tixena".to_owned())));
    assert!(meta.min_length.is_none());
}

#[test]
fn test_parse_literal_bool_true() {
    let attr: Attribute = parse_quote! { #[model_schema_prop(literal = true)] };
    let meta = parse_model_schema_prop_attributes(&[attr]);
    assert_eq!(meta.literal, Some(LiteralValue::Bool(true)));
}

#[test]
fn test_parse_literal_bool_false() {
    let attr: Attribute = parse_quote! { #[model_schema_prop(literal = false)] };
    let meta = parse_model_schema_prop_attributes(&[attr]);
    assert_eq!(meta.literal, Some(LiteralValue::Bool(false)));
}

#[test]
fn test_parse_literal_number_integer() {
    let attr: Attribute = parse_quote! { #[model_schema_prop(literal = 214)] };
    let meta = parse_model_schema_prop_attributes(&[attr]);
    assert_eq!(meta.literal, Some(LiteralValue::Number(214.0)));
}

#[test]
fn test_parse_literal_number_float() {
    let attr: Attribute = parse_quote! { #[model_schema_prop(literal = 3.5)] };
    let meta = parse_model_schema_prop_attributes(&[attr]);
    assert_eq!(meta.literal, Some(LiteralValue::Number(3.5)));
}

#[test]
fn test_parse_both_as_and_literal() {
    let attr: Attribute = parse_quote! { #[model_schema_prop(as = String, literal = "Tixena")] };
    let meta = parse_model_schema_prop_attributes(&[attr]);
    assert!(meta.as_type.is_some());
    assert_eq!(meta.as_type.unwrap(), parse_quote!(String));
    assert_eq!(meta.literal, Some(LiteralValue::Str("Tixena".to_owned())));
    assert!(meta.min_length.is_none());
}

#[test]
fn test_parse_min_length() {
    let attr: Attribute = parse_quote! { #[model_schema_prop(minLength = 1)] };
    let meta = parse_model_schema_prop_attributes(&[attr]);
    assert!(meta.as_type.is_none());
    assert!(meta.literal.is_none());
    assert!(meta.min_length.is_some());
    assert_eq!(meta.min_length.unwrap(), 1);
}

#[test]
fn test_parse_as_and_min_length() {
    let attr: Attribute = parse_quote! { #[model_schema_prop(as = String, minLength = 5)] };
    let meta = parse_model_schema_prop_attributes(&[attr]);
    assert!(meta.as_type.is_some());
    assert_eq!(meta.as_type.unwrap(), parse_quote!(String));
    assert!(meta.literal.is_none());
    assert!(meta.min_length.is_some());
    assert_eq!(meta.min_length.unwrap(), 5);
}

#[test]
fn test_parse_ts_optional() {
    let attr: Attribute = parse_quote! { #[model_schema_prop(ts_optional)] };
    let meta = parse_model_schema_prop_attributes(&[attr]);
    assert!(meta.ts_optional);
    assert!(meta.as_type.is_none());
    assert!(meta.literal.is_none());
}

#[test]
fn test_parse_both_as_and_ts_optional() {
    let attr: Attribute = parse_quote! { #[model_schema_prop(as = SomeBrand, ts_optional)] };
    let meta = parse_model_schema_prop_attributes(&[attr]);
    assert!(meta.as_type.is_some());
    assert_eq!(meta.as_type.unwrap(), parse_quote!(SomeBrand));
    assert!(meta.ts_optional);
}

#[test]
fn test_parse_no_ts_optional_by_default() {
    let attr: Attribute = parse_quote! { #[model_schema_prop(minLength = 1)] };
    let meta = parse_model_schema_prop_attributes(&[attr]);
    assert!(!meta.ts_optional);
}

#[test]
fn test_parse_nullable() {
    let attr: Attribute = parse_quote! { #[model_schema_prop(nullable)] };
    let meta = parse_model_schema_prop_attributes(&[attr]);
    assert!(meta.nullable);
    assert!(meta.as_type.is_none());
    assert!(meta.literal.is_none());
}

#[test]
fn test_parse_no_nullable_by_default() {
    let attr: Attribute = parse_quote! { #[model_schema_prop(minLength = 1)] };
    let meta = parse_model_schema_prop_attributes(&[attr]);
    assert!(!meta.nullable);
}

#[test]
fn test_parse_all_attributes() {
    let attr: Attribute =
        parse_quote! { #[model_schema_prop(as = String, literal = "test", minLength = 3)] };
    let meta = parse_model_schema_prop_attributes(&[attr]);
    assert!(meta.as_type.is_some());
    assert_eq!(meta.as_type.unwrap(), parse_quote!(String));
    assert_eq!(meta.literal, Some(LiteralValue::Str("test".to_owned())));
    assert!(meta.min_length.is_some());
    assert_eq!(meta.min_length.unwrap(), 3);
}

/// The parser's refusal of `attr`, rendered, or `None` when it read the attribute whole.
fn attr_rejection(attr: Attribute) -> Option<String> {
    parse_model_schema_prop_attributes(&[attr])
        .attr_rejection
        .as_ref()
        .map(ToString::to_string)
}

/// The reported repro: a misspelled `pattern` compiled clean and emitted an unconstrained string.
/// The refusal names the key as written and the one that was meant.
#[test]
fn a_misspelled_string_constraint_key_is_refused_by_the_name_as_written() {
    let rejection =
        attr_rejection(parse_quote! { #[model_schema_prop(patern = "^[a-z]+$")] }).unwrap();
    assert!(rejection.contains("patern"), "got: {rejection}");
    assert!(rejection.contains("pattern"), "got: {rejection}");
}

#[test]
fn a_misspelled_length_constraint_key_is_refused_by_the_name_as_written() {
    let rejection = attr_rejection(parse_quote! { #[model_schema_prop(minLenght = 3)] }).unwrap();
    assert!(rejection.contains("minLenght"), "got: {rejection}");
    assert!(rejection.contains("minLength"), "got: {rejection}");
}

/// The refusal offers every key the parser reads, and the probes below prove each offered name is
/// one it actually reads — the list and the arms cannot drift apart while both hold.
#[test]
fn no_key_the_parser_reads_is_rejected() {
    let attrs: [Attribute; 11] = [
        parse_quote! { #[model_schema_prop(as = String)] },
        parse_quote! { #[model_schema_prop(literal = "Tixena")] },
        parse_quote! { #[model_schema_prop(minLength = 1)] },
        parse_quote! { #[model_schema_prop(maxLength = 50)] },
        parse_quote! { #[model_schema_prop(minimum = 0)] },
        parse_quote! { #[model_schema_prop(maximum = 1.5)] },
        parse_quote! { #[model_schema_prop(pattern = "^[a-z]+$")] },
        parse_quote! { #[model_schema_prop(preprocess = ["trim"])] },
        parse_quote! { #[model_schema_prop(ts_optional)] },
        parse_quote! { #[model_schema_prop(as_number)] },
        parse_quote! { #[model_schema_prop(nullable)] },
    ];
    assert_eq!(attrs.len(), KNOWN_KEYS.len());

    let offered = attr_rejection(parse_quote! { #[model_schema_prop(nonsense)] }).unwrap();
    for key in KNOWN_KEYS {
        assert!(offered.contains(key), "{key} not offered: {offered}");
    }
    for attr in attrs {
        let rendered = quote::quote!(#attr).to_string();
        assert_eq!(attr_rejection(attr), None, "for {rendered}");
    }
}

/// Every value the old parser dropped on the floor: a wrong literal kind, a length the target type
/// cannot hold, and a `preprocess` element that names no function.
#[test]
fn a_value_the_parser_cannot_read_is_refused() {
    let attrs: [Attribute; 9] = [
        parse_quote! { #[model_schema_prop(as = 3)] },
        parse_quote! { #[model_schema_prop(literal = Tixena)] },
        parse_quote! { #[model_schema_prop(minLength = "3")] },
        parse_quote! { #[model_schema_prop(minLength = -1)] },
        parse_quote! { #[model_schema_prop(maxLength = 99999999999999999999999999999999999999999)] },
        parse_quote! { #[model_schema_prop(minimum = "0")] },
        parse_quote! { #[model_schema_prop(maximum = "0")] },
        parse_quote! { #[model_schema_prop(pattern = 3)] },
        parse_quote! { #[model_schema_prop(preprocess = ["trim", 3])] },
    ];
    for attr in attrs {
        let rendered = quote::quote!(#attr).to_string();
        assert!(attr_rejection(attr).is_some(), "for {rendered}");
    }
}

/// A key the parser reads before the refused one still lands: the refusal reports the attribute,
/// it does not discard what was already read.
#[test]
fn a_refusal_keeps_what_the_parser_had_already_read() {
    let meta = parse_model_schema_prop_attributes(&[parse_quote! {
        #[model_schema_prop(minLength = 3, patern = "^[a-z]+$")]
    }]);
    assert_eq!(meta.min_length, Some(3));
    assert!(meta.attr_rejection.is_some());
}
