#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
use serde::{Deserialize, Serialize};
#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
use tixschema::model_schema;

#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct AccountContext {
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
    #[model_schema_prop(as = String, literal = "Tixena")]
    pub iss: String,
    #[model_schema_prop(as = String, minLength = 1)]
    pub jti: String,
    pub nbf: i64,
    #[model_schema_prop(as = String, minLength = 1)]
    pub sub: String,
}

#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct ArrayLiteral {
    pub id: String,
    #[model_schema_prop(literal = "array_item")]
    pub literal_array: Vec<String>,
}

#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct CombinedTest {
    #[model_schema_prop(as = String, literal = "fixed", minLength = 10)]
    pub fixed_field: String,
    #[model_schema_prop(as = String, minLength = 1)]
    pub normal_field: String,
}

#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct MinLengthTest {
    pub description: String,
    #[model_schema_prop(as = String, minLength = 1)]
    pub name: String,
    #[model_schema_prop(as = String, minLength = 3)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    #[model_schema_prop(as = String, minLength = 10)]
    pub password: String,
    #[model_schema_prop(as = String, minLength = 2)]
    pub tags: Vec<String>,
    #[model_schema_prop(as = String, minLength = 5)]
    pub username: String,
}

#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct MultipleLiterals {
    pub id: String,
    pub name: String,
    #[model_schema_prop(literal = "fixed_type")]
    pub type_field: String,
    #[model_schema_prop(literal = "v1.0")]
    pub version: String,
}

#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct OptionalLiteral {
    pub id: String,
    #[model_schema_prop(literal = "optional_literal")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional_type: Option<String>,
}

#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Inner {
    pub value: String,
}

#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct TsOptionalStruct {
    #[model_schema_prop(ts_optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub f: Option<Inner>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub g: Option<Inner>,
    // `as` is currently a no-op override; this field guards their coexistence.
    #[model_schema_prop(as = Inner, ts_optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h: Option<Inner>,
}

#[cfg(all(
    test,
    any(feature = "typescript", feature = "jsonschema", feature = "zod")
))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type")]
enum TsOptionalVariant {
    FilterPart {
        #[model_schema_prop(ts_optional)]
        #[serde(skip_serializing_if = "Option::is_none")]
        filter: Option<Inner>,
    },
}

/// `f` and `h` carry no key-dropping serde attribute: `nullable` requires the key always be
/// written, so serde writes `null` for a `None` rather than omitting the key. `g` is the control,
/// left in the default flavor with the key-dropping attribute the default guard requires.
#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NullableStruct {
    #[model_schema_prop(nullable)]
    pub f: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub g: Option<String>,
    #[model_schema_prop(nullable, preprocess = ["trim"])]
    pub h: Option<String>,
}

#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
#[model_schema(default_types(IdType = String))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NullableGeneric<IdType> {
    #[model_schema_prop(nullable)]
    pub id: Option<IdType>,
    pub name: String,
}

/// The member an `Option` field written with a `skip_serializing_if` renders as. The attribute
/// decides the wire, not the spelling, and none of these fields carries `ts_optional`.
#[cfg(feature = "typescript")]
fn omitted_member(name: &str, ts_type: &str) -> String {
    format!("{name}: {ts_type} | undefined;")
}

#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
#[test]
fn test_model_schema_prop_structs_constructible() {
    let account = AccountContext {
        aud: String::new(),
        exp: 0,
        iat: 0,
        iss: String::new(),
        jti: String::new(),
        nbf: 0,
        sub: String::new(),
    };
    assert!(account.aud.is_empty());
    let array_literal = ArrayLiteral {
        id: String::new(),
        literal_array: Vec::new(),
    };
    assert!(array_literal.id.is_empty());
    let combined = CombinedTest {
        fixed_field: String::new(),
        normal_field: String::new(),
    };
    assert!(combined.fixed_field.is_empty());
    let min_length = MinLengthTest {
        description: String::new(),
        name: String::new(),
        nickname: None,
        password: String::new(),
        tags: Vec::new(),
        username: String::new(),
    };
    assert!(min_length.description.is_empty());
    let multiple = MultipleLiterals {
        id: String::new(),
        name: String::new(),
        type_field: String::new(),
        version: String::new(),
    };
    assert!(multiple.id.is_empty());
    let optional_literal = OptionalLiteral {
        id: String::new(),
        optional_type: None,
    };
    assert!(optional_literal.id.is_empty());
    let inner = Inner {
        value: String::new(),
    };
    assert!(inner.value.is_empty());
    let ts_optional = TsOptionalStruct {
        f: None,
        g: None,
        h: None,
    };
    assert!(ts_optional.f.is_none());
    let nullable = NullableStruct {
        f: None,
        g: None,
        h: None,
    };
    assert!(nullable.f.is_none());
    let nullable_generic = NullableGeneric {
        name: String::new(),
        id: None::<String>,
    };
    assert!(nullable_generic.id.is_none());
}

#[cfg(all(
    test,
    any(feature = "typescript", feature = "jsonschema", feature = "zod")
))]
#[test]
fn test_ts_optional_variant_constructible() {
    let variant = TsOptionalVariant::FilterPart { filter: None };
    assert!(matches!(variant, TsOptionalVariant::FilterPart { .. }));
}

#[test]
#[cfg(feature = "typescript")]
fn test_string_literal_typescript() {
    let ts_definition = AccountContext::ts_definition();

    assert!(ts_definition.contains("iss: \"Tixena\";"));

    assert!(ts_definition.contains("aud: string;"));
    assert!(ts_definition.contains("sub: string;"));
    assert!(ts_definition.contains("jti: string;"));

    assert!(ts_definition.contains("exp: number;"));
    assert!(ts_definition.contains("iat: number;"));
    assert!(ts_definition.contains("nbf: number;"));

    assert!(ts_definition.contains("Minimum length: 1"));
}

#[test]
#[cfg(feature = "zod")]
fn test_string_literal_zod() {
    let zod_schema = AccountContext::zod_schema();

    assert!(zod_schema.contains("iss: z.literal(\"Tixena\")"));

    assert!(zod_schema.contains("aud: z.string()"));

    assert!(zod_schema.contains("sub: z.string().min(1)"));
    assert!(zod_schema.contains("jti: z.string().min(1)"));

    assert!(zod_schema.contains("exp: z.number().int()"));
    assert!(zod_schema.contains("iat: z.number().int()"));
    assert!(zod_schema.contains("nbf: z.number().int()"));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_string_literal_json_schema() {
    let schema = AccountContext::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    let iss_prop = &properties["iss"];
    assert_eq!(iss_prop["type"], "string");
    assert_eq!(iss_prop["const"], "Tixena");

    let aud_prop = &properties["aud"];
    assert_eq!(aud_prop["type"], "string");
    assert!(aud_prop.get("const").is_none());
    assert!(aud_prop.get("minLength").is_none());

    let sub_prop = &properties["sub"];
    assert_eq!(sub_prop["type"], "string");
    assert!(sub_prop.get("const").is_none());
    assert_eq!(sub_prop["minLength"], 1_i32);

    let jti_prop = &properties["jti"];
    assert_eq!(jti_prop["type"], "string");
    assert!(jti_prop.get("const").is_none());
    assert_eq!(jti_prop["minLength"], 1_i32);
}

#[test]
#[cfg(feature = "typescript")]
fn test_multiple_literals_typescript() {
    let ts_definition = MultipleLiterals::ts_definition();

    assert!(ts_definition.contains("type_field: \"fixed_type\";"));
    assert!(ts_definition.contains("version: \"v1.0\";"));

    assert!(ts_definition.contains("id: string;"));
    assert!(ts_definition.contains("name: string;"));
}

#[test]
#[cfg(feature = "zod")]
fn test_multiple_literals_zod() {
    let zod_schema = MultipleLiterals::zod_schema();

    assert!(zod_schema.contains("type_field: z.literal(\"fixed_type\")"));
    assert!(zod_schema.contains("version: z.literal(\"v1.0\")"));

    assert!(zod_schema.contains("id: z.string()"));
    assert!(zod_schema.contains("name: z.string()"));
}

#[test]
#[cfg(feature = "typescript")]
fn test_optional_literal_typescript() {
    let ts_definition = OptionalLiteral::ts_definition();

    assert!(ts_definition.contains(&omitted_member("optional_type", "\"optional_literal\"")));
    assert!(ts_definition.contains("id: string;"));
}

#[test]
#[cfg(feature = "zod")]
fn test_optional_literal_zod() {
    let zod_schema = OptionalLiteral::zod_schema();

    assert!(zod_schema.contains(
        "optional_type: z.union([z.null().transform(() => undefined), \
         z.literal(\"optional_literal\"), z.undefined()])"
    ));
    assert!(zod_schema.contains("id: z.string()"));
}

#[test]
#[cfg(feature = "typescript")]
fn test_array_literal_typescript() {
    let ts_definition = ArrayLiteral::ts_definition();

    assert!(ts_definition.contains("literal_array: Array<\"array_item\">;"));
    assert!(ts_definition.contains("id: string;"));
}

#[test]
#[cfg(feature = "zod")]
fn test_array_literal_zod() {
    let zod_schema = ArrayLiteral::zod_schema();

    assert!(zod_schema.contains("literal_array: z.array(z.literal(\"array_item\"))"));
    assert!(zod_schema.contains("id: z.string()"));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_array_literal_json_schema() {
    let schema = ArrayLiteral::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    let literal_array_prop = &properties["literal_array"];
    assert_eq!(literal_array_prop["type"], "array");
    assert_eq!(literal_array_prop["items"]["type"], "string");
    assert_eq!(literal_array_prop["items"]["const"], "array_item");
}

#[test]
#[cfg(feature = "typescript")]
fn test_min_length_typescript() {
    let ts_definition = MinLengthTest::ts_definition();

    assert!(ts_definition.contains("name: string;"));
    assert!(ts_definition.contains("username: string;"));
    assert!(ts_definition.contains("password: string;"));
    assert!(ts_definition.contains("description: string;"));
    assert!(ts_definition.contains(&omitted_member("nickname", "string")));
    assert!(ts_definition.contains("tags: Array<string>;"));

    assert!(ts_definition.contains("Minimum length: 1"));
    assert!(ts_definition.contains("Minimum length: 5"));
    assert!(ts_definition.contains("Minimum length: 10"));
    assert!(ts_definition.contains("Minimum length: 3"));
    assert!(ts_definition.contains("Minimum length: 2"));
}

#[test]
#[cfg(feature = "zod")]
fn test_min_length_zod() {
    let zod_schema = MinLengthTest::zod_schema();

    assert!(zod_schema.contains("name: z.string().min(1)"));
    assert!(zod_schema.contains("username: z.string().min(5)"));
    assert!(zod_schema.contains("password: z.string().min(10)"));
    assert!(zod_schema.contains("tags: z.array(z.string().min(2))"));

    assert!(zod_schema.contains("description: z.string(),"));

    assert!(zod_schema.contains(
        "nickname: z.union([z.null().transform(() => undefined), z.string().min(3), z.undefined()])"
    ));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_min_length_json_schema() {
    let schema = MinLengthTest::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    let name_prop = &properties["name"];
    assert_eq!(name_prop["type"], "string");
    assert_eq!(name_prop["minLength"], 1_i32);

    let username_prop = &properties["username"];
    assert_eq!(username_prop["type"], "string");
    assert_eq!(username_prop["minLength"], 5_i32);

    let password_prop = &properties["password"];
    assert_eq!(password_prop["type"], "string");
    assert_eq!(password_prop["minLength"], 10_i32);

    let nickname_prop = &properties["nickname"];
    assert_eq!(nickname_prop["anyOf"][0]["type"], "string");
    assert_eq!(nickname_prop["anyOf"][0]["minLength"], 3_i32);
    assert_eq!(nickname_prop["anyOf"][1]["type"], "null");

    let description_prop = &properties["description"];
    assert_eq!(description_prop["type"], "string");
    assert!(description_prop.get("minLength").is_none());

    let tags_prop = &properties["tags"];
    assert_eq!(tags_prop["type"], "array");
    assert_eq!(tags_prop["items"]["type"], "string");
    assert_eq!(tags_prop["items"]["minLength"], 2_i32);
}

#[test]
#[cfg(feature = "typescript")]
fn test_combined_literal_minlength_typescript() {
    let ts_definition = CombinedTest::ts_definition();

    assert!(ts_definition.contains("fixed_field: \"fixed\";"));
    assert!(ts_definition.contains("normal_field: string;"));

    assert!(ts_definition.contains("Minimum length: 1"));
}

#[test]
#[cfg(feature = "zod")]
fn test_combined_literal_minlength_zod() {
    let zod_schema = CombinedTest::zod_schema();

    assert!(zod_schema.contains("fixed_field: z.literal(\"fixed\")"));
    assert!(zod_schema.contains("normal_field: z.string().min(1)"));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_combined_literal_minlength_json_schema() {
    let schema = CombinedTest::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    let fixed_prop = &properties["fixed_field"];
    assert_eq!(fixed_prop["type"], "string");
    assert_eq!(fixed_prop["const"], "fixed");
    assert!(fixed_prop.get("minLength").is_none()); // Should not have minLength when literal

    let normal_prop = &properties["normal_field"];
    assert_eq!(normal_prop["type"], "string");
    assert_eq!(normal_prop["minLength"], 1_i32);
    assert!(normal_prop.get("const").is_none());
}

#[test]
#[cfg(feature = "typescript")]
fn test_ts_optional_struct_typescript() {
    let ts = TsOptionalStruct::ts_definition();

    assert!(ts.contains("f?: Inner;"), "expected `f?: Inner;` in:\n{ts}");
    assert!(
        !ts.contains("f: Inner | undefined"),
        "did not expect required-key form for `f` in:\n{ts}"
    );

    let g = omitted_member("g", "Inner");
    assert!(ts.contains(&g), "expected control `{g}` in:\n{ts}");

    assert!(ts.contains("h?: Inner;"), "expected `h?: Inner;` in:\n{ts}");
}

#[test]
#[cfg(feature = "zod")]
fn test_ts_optional_struct_zod_unchanged() {
    let zod = TsOptionalStruct::zod_schema();

    assert!(
        zod.contains(
            "f: z.union([z.null().transform(() => undefined), Inner$Schema, z.undefined()]).prefault(undefined)"
        ),
        "expected unchanged Zod for `f` in:\n{zod}"
    );
    assert!(
        zod.contains(
            "g: z.union([z.null().transform(() => undefined), Inner$Schema, z.undefined()]).prefault(undefined)"
        ),
        "expected unchanged Zod for `g` in:\n{zod}"
    );
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_ts_optional_struct_json_schema_unchanged() {
    let schema = TsOptionalStruct::json_schema();
    let required_arr = schema["required"].as_array().unwrap();
    let required: Vec<&str> = required_arr.iter().filter_map(|v| v.as_str()).collect();

    assert!(
        !required.contains(&"f"),
        "`f` should not be required: {required:?}"
    );
    assert!(
        !required.contains(&"g"),
        "`g` should not be required: {required:?}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_ts_optional_variant_typescript() {
    let ts = TsOptionalVariant::ts_definition();

    // Covers the named-variant render path (write_named_variant_fields).
    assert!(
        ts.contains("filter?: Inner;"),
        "expected `filter?: Inner;` in variant TS:\n{ts}"
    );
    assert!(
        !ts.contains("filter: Inner | undefined"),
        "did not expect required-key form for variant `filter` in:\n{ts}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_nullable_struct_typescript() {
    let ts = NullableStruct::ts_definition();

    assert!(ts.contains("f: string | null;"), "Got: {ts}");
    assert!(
        !ts.contains("f: string | undefined"),
        "nullable should not fall back to the coercing spelling: {ts}"
    );

    let g = omitted_member("g", "string");
    assert!(ts.contains(&g), "expected control `{g}` in:\n{ts}");

    assert!(
        ts.contains("h: string | null;"),
        "preprocess should not change the TypeScript type: {ts}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_nullable_struct_zod() {
    let zod = NullableStruct::zod_schema();

    assert!(
        zod.contains("f: z.union([z.string(), z.null()])"),
        "Got: {zod}"
    );
    assert!(
        !zod.contains("f: z.union([z.null().transform"),
        "nullable should not coerce null away: {zod}"
    );

    assert!(
        zod.contains(
            "g: z.union([z.null().transform(() => undefined), z.string(), z.undefined()]).prefault(undefined)"
        ),
        "expected unchanged Zod for control field `g` in:\n{zod}"
    );

    assert!(
        zod.contains("h: z.preprocess(trim, z.union([z.string(), z.null()]))"),
        "preprocess should wrap the whole nullable union: {zod}"
    );
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_nullable_struct_json_schema() {
    let schema = NullableStruct::json_schema();
    let required_arr = schema["required"].as_array().unwrap();
    let required: Vec<&str> = required_arr.iter().filter_map(|v| v.as_str()).collect();

    assert!(
        required.contains(&"f"),
        "`f` should be required: {required:?}"
    );
    assert!(
        !required.contains(&"g"),
        "`g` should not be required: {required:?}"
    );
    assert!(
        required.contains(&"h"),
        "`h` should be required: {required:?}"
    );

    let properties = schema["properties"].as_object().unwrap();
    let f_prop = &properties["f"];
    assert_eq!(f_prop["anyOf"][0]["type"], "string");
    assert_eq!(f_prop["anyOf"][1]["type"], "null");
}

#[test]
#[cfg(feature = "typescript")]
fn test_nullable_generic_typescript() {
    let ts = NullableGeneric::<String>::ts_definition();
    assert!(ts.contains("id: IdType | null;"), "Got: {ts}");
}

#[test]
#[cfg(feature = "zod")]
fn test_nullable_generic_zod() {
    let zod = NullableGeneric::<String>::zod_schema();

    assert!(
        zod.contains("export function NullableGeneric$SchemaFactory"),
        "expected a factory for a generic type: {zod}"
    );
    assert!(
        zod.contains("id: z.union([idType, z.null()])"),
        "nullable should compose with the factory argument: {zod}"
    );
}
