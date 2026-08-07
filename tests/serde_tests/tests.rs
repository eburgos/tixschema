use serde::{Deserialize, Serialize};
use tixschema::model_schema;

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
enum Color {
    Blue,
    #[serde(rename = "dark_green")]
    Green,
    Red,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Event {
    #[serde(rename = "userCreated")]
    Created { user_id: String, user_name: String },
    #[serde(rename = "userDeleted")]
    Deleted { user_id: String },
    #[serde(rename = "userUpdated")]
    Updated {
        #[serde(rename = "newEmail")]
        email: String,
        user_id: String,
    },
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
struct Lowercase {
    field_one: String,
    field_two: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct OptionalFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "customOptional")]
    another_optional: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_field: Option<String>,
    required_field: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct CompliantOptionals {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    #[model_schema_prop(ts_optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    tag: Option<String>,
}

/// The two `Option` flavors side by side: `generation_token` drops its key for a `None` (the
/// default), `template_id` writes `null` instead (`nullable`).
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct IndexEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_token: Option<String>,
    pub name: String,
    #[model_schema_prop(nullable)]
    pub template_id: Option<String>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
enum Status {
    Active,
    Inactive,
    Pending,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct UserWithSerde {
    created_at: String,
    #[serde(rename = "emailAddress")]
    email: String,
    first_name: String,
    is_verified: bool,
    last_name: String,
    user_id: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum Body {
    Fresh {
        subject: String,
    },
    Reply {
        #[serde(rename = "reply-to")]
        reply_to: String,
    },
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "kind")]
enum Delivery {
    Sent {
        #[serde(rename = "sent-at")]
        sent_at: String,
    },
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Envelope {
    #[serde(flatten)]
    body: Body,
    id: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Message {
    id: String,
    #[serde(rename = "reply-to", default, skip_serializing_if = "Option::is_none")]
    reply_to: Option<String>,
}

/// A key an identifier can hold is written as it always was, on both text surfaces.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct PlainKeys {
    #[serde(rename = "$ref")]
    reference: String,
    #[serde(rename = "user_id2")]
    user_id: String,
}

/// The list form of `rename`, with one name in both of serde's directions.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct ListFormRename {
    #[serde(rename(serialize = "same_name", deserialize = "same_name"))]
    value: u32,
}

/// The list form of `rename_all`, with one rule in both of serde's directions.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all(serialize = "camelCase", deserialize = "camelCase"))]
struct ListFormRenameAll {
    my_field: u32,
}

/// `bound(...)` in both places serde accepts it. It replaces the trait bounds serde's derive
/// writes on its own impls, which no generated surface describes.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(bound(serialize = "", deserialize = ""))]
struct BoundCarrying {
    #[serde(bound(serialize = "", deserialize = ""))]
    reading: u32,
    writing: u32,
}

/// The rendered line naming `key`, with the key itself removed, so two members rendered the same
/// way compare equal whatever they are called.
#[cfg(any(feature = "typescript", feature = "zod"))]
fn member_rendering(surface: &str, key: &str) -> String {
    let member = surface
        .lines()
        .find(|line| line.trim_start().starts_with(&format!("{key}:")));
    assert!(member.is_some(), "no member `{key}` in:\n{surface}");
    member.unwrap().replacen(key, "", 1)
}

#[test]
fn test_serde_types_constructible() {
    let color = Color::Red;
    assert!(matches!(color, Color::Red));
    let event = Event::Deleted {
        user_id: String::new(),
    };
    assert!(matches!(event, Event::Deleted { .. }));
    let lowercase = Lowercase {
        field_one: String::new(),
        field_two: String::new(),
    };
    assert!(lowercase.field_one.is_empty());
    let optional = OptionalFields {
        another_optional: None,
        optional_field: None,
        required_field: String::new(),
    };
    assert!(optional.required_field.is_empty());
    let status = Status::Active;
    assert!(matches!(status, Status::Active));
    let user = UserWithSerde {
        created_at: String::new(),
        email: String::new(),
        first_name: String::new(),
        is_verified: false,
        last_name: String::new(),
        user_id: String::new(),
    };
    assert!(user.user_id.is_empty());
    let message = Message {
        id: String::new(),
        reply_to: None,
    };
    assert!(message.reply_to.is_none());
    let delivery = Delivery::Sent {
        sent_at: String::new(),
    };
    assert!(matches!(delivery, Delivery::Sent { .. }));
    let envelope = Envelope {
        body: Body::Reply {
            reply_to: String::new(),
        },
        id: String::new(),
    };
    assert!(matches!(envelope.body, Body::Reply { .. }));
    let plain = PlainKeys {
        reference: String::new(),
        user_id: String::new(),
    };
    assert!(plain.reference.is_empty());
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_serde_rename_all_camel_case_json_schema() {
    let schema = UserWithSerde::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    assert!(properties.contains_key("userId")); // user_id -> userId
    assert!(properties.contains_key("firstName")); // first_name -> firstName
    assert!(properties.contains_key("lastName")); // last_name -> lastName
    assert!(properties.contains_key("emailAddress")); // email -> emailAddress (manual rename)
    assert!(properties.contains_key("createdAt")); // created_at -> createdAt
    assert!(properties.contains_key("isVerified")); // is_verified -> isVerified

    assert!(!properties.contains_key("user_id"));
    assert!(!properties.contains_key("first_name"));
    assert!(!properties.contains_key("last_name"));
    assert!(!properties.contains_key("email"));
    assert!(!properties.contains_key("created_at"));
    assert!(!properties.contains_key("is_verified"));

    assert_eq!(properties["userId"]["type"], "string");
    assert_eq!(properties["firstName"]["type"], "string");
    assert_eq!(properties["lastName"]["type"], "string");
    assert_eq!(properties["emailAddress"]["type"], "string");
    assert_eq!(properties["createdAt"]["type"], "string");
    assert_eq!(properties["isVerified"]["type"], "boolean");
}

#[test]
#[cfg(feature = "typescript")]
fn test_serde_rename_all_camel_case_typescript() {
    let ts_definition = UserWithSerde::ts_definition();

    assert!(ts_definition.contains("userId: string;"));
    assert!(ts_definition.contains("firstName: string;"));
    assert!(ts_definition.contains("lastName: string;"));
    assert!(ts_definition.contains("emailAddress: string;"));
    assert!(ts_definition.contains("createdAt: string;"));
    assert!(ts_definition.contains("isVerified: boolean;"));
}

#[test]
#[cfg(feature = "zod")]
fn test_serde_rename_all_camel_case_zod() {
    let zod_schema = UserWithSerde::zod_schema();

    assert!(zod_schema.contains("userId: z.string()"));
    assert!(zod_schema.contains("firstName: z.string()"));
    assert!(zod_schema.contains("lastName: z.string()"));
    assert!(zod_schema.contains("emailAddress: z.string()"));
    assert!(zod_schema.contains("createdAt: z.string()"));
    assert!(zod_schema.contains("isVerified: z.boolean()"));
}

#[test]
#[cfg(feature = "typescript")]
fn test_rename_all_lowercase() {
    let ts = Lowercase::ts_definition();

    // lowercase converts to lowercase but keeps underscores
    assert!(ts.contains("field_one: string;"));
    assert!(ts.contains("field_two: string;"));
}

#[test]
#[cfg(feature = "typescript")]
fn test_serde_with_optional_fields_typescript() {
    let ts = OptionalFields::ts_definition();

    assert!(ts.contains("requiredField: string;"));
    assert!(ts.contains("optionalField: string | undefined;"));
    assert!(ts.contains("customOptional: number | undefined;"));
}

#[test]
#[cfg(feature = "zod")]
fn test_serde_with_optional_fields_zod() {
    let zod = OptionalFields::zod_schema();

    assert!(zod.contains("requiredField: z.string()"));
    assert!(zod.contains(
        "optionalField: z.union([z.null().transform(() => undefined), z.string(), z.undefined()])"
    ));
    assert!(zod.contains(
        "customOptional: z.union([z.null().transform(() => undefined), z.number().int(), z.undefined()])"
    ));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_serde_with_optional_fields_json_schema() {
    let schema = OptionalFields::json_schema();

    let properties = schema["properties"].as_object().unwrap();
    assert!(properties.contains_key("requiredField"));
    assert!(properties.contains_key("optionalField"));
    assert!(properties.contains_key("customOptional"));

    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&serde_json::Value::String("requiredField".to_owned())));
    assert!(!required.contains(&serde_json::Value::String("optionalField".to_owned())));
    assert!(!required.contains(&serde_json::Value::String("customOptional".to_owned())));
}

#[test]
#[cfg(feature = "typescript")]
fn test_enum_rename_all_lowercase() {
    let ts = Status::ts_definition();

    assert!(ts.contains("\"active\""));
    assert!(ts.contains("\"inactive\""));
    assert!(ts.contains("\"pending\""));
}

#[test]
#[cfg(feature = "zod")]
fn test_enum_rename_all_zod() {
    let zod = Status::zod_schema();

    assert!(zod.contains("\"active\""));
    assert!(zod.contains("\"inactive\""));
    assert!(zod.contains("\"pending\""));
}

#[test]
#[cfg(feature = "typescript")]
fn test_enum_field_rename() {
    let ts = Color::ts_definition();

    assert!(ts.contains("\"Red\""));
    assert!(ts.contains("\"dark_green\""));
    assert!(ts.contains("\"Blue\""));
    assert!(!ts.contains("\"Green\""));
}

#[test]
#[cfg(feature = "typescript")]
fn test_discriminated_union_with_serde_typescript() {
    let ts = Event::ts_definition();

    assert!(ts.contains("type: \"userCreated\""));
    assert!(ts.contains("type: \"userDeleted\""));
    assert!(ts.contains("type: \"userUpdated\""));

    // The enum's own rename_all cases the discriminator alone; `user_id`/`user_name` carry no
    // rename of their own and stay as declared, while `email`'s own field-level rename still
    // applies regardless of the container.
    assert!(ts.contains("user_id: string;"));
    assert!(ts.contains("user_name: string;"));
    assert!(ts.contains("newEmail: string;"));
}

#[test]
#[cfg(feature = "zod")]
fn test_discriminated_union_with_serde_zod() {
    let zod = Event::zod_schema();

    assert!(zod.contains("z.discriminatedUnion"));
    assert!(zod.contains("\"type\""));
    assert!(zod.contains("user_id"));
    assert!(zod.contains("user_name"));
    assert!(zod.contains("newEmail"));
}

#[test]
#[cfg(feature = "typescript")]
fn test_compliant_optionals_typescript() {
    let ts = CompliantOptionals::ts_definition();

    // Same `skip_serializing_if` on both; the flag on `tag` is the only difference.
    assert!(
        ts.contains("tag?: string;"),
        "expected `tag?: string;`:\n{ts}"
    );
    assert!(
        ts.contains("note: string | undefined;"),
        "expected `note: string | undefined;`:\n{ts}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_compliant_optionals_zod_keeps_undefined_union() {
    let zod = CompliantOptionals::zod_schema();

    assert!(zod.contains("z.strictObject"));
    assert!(zod.contains(
        "z.union([z.null().transform(() => undefined), z.string(), z.undefined()]).prefault(undefined)"
    ));
    assert!(!zod.contains("z.nullable"));
}

/// The guard exists so the wire form matches the schema: `None` must leave the key out.
#[test]
fn test_compliant_optionals_serialize_absent_and_parse_absent() {
    let none = CompliantOptionals {
        name: "n".to_owned(),
        note: None,
        tag: None,
    };
    let json = serde_json::to_string(&none).unwrap();
    assert_eq!(json, r#"{"name":"n"}"#);

    let parsed: CompliantOptionals = serde_json::from_str(r#"{"name":"n"}"#).unwrap();
    assert_eq!(parsed, none);

    let some = CompliantOptionals {
        name: "n".to_owned(),
        note: Some("here".to_owned()),
        tag: None,
    };
    assert_eq!(
        serde_json::to_string(&some).unwrap(),
        r#"{"name":"n","note":"here"}"#
    );
}

/// Each `Option` flavor writes exactly one shape for a `None`: the default drops the key, `nullable`
/// writes `null` and keeps it. Both read every shape either flavor writes, plus the one it does not.
#[test]
fn test_each_option_flavor_writes_one_shape_and_reads_both() {
    let entry = IndexEntry {
        name: "invoice.pdf".to_owned(),
        generation_token: None,
        template_id: None,
    };
    assert_eq!(
        serde_json::to_string(&entry).unwrap(),
        r#"{"name":"invoice.pdf","templateId":null}"#
    );

    let from_absent: IndexEntry =
        serde_json::from_str(r#"{"name":"invoice.pdf","templateId":null}"#).unwrap();
    assert_eq!(from_absent, entry);

    let from_null: IndexEntry =
        serde_json::from_str(r#"{"name":"invoice.pdf","generationToken":null,"templateId":null}"#)
            .unwrap();
    assert_eq!(from_null, entry);

    let both_keys_absent: IndexEntry = serde_json::from_str(r#"{"name":"invoice.pdf"}"#).unwrap();
    assert_eq!(both_keys_absent, entry);

    let both_some = IndexEntry {
        name: "invoice.pdf".to_owned(),
        generation_token: Some("tok".to_owned()),
        template_id: Some("tmpl-1".to_owned()),
    };
    assert_eq!(
        serde_json::to_string(&both_some).unwrap(),
        r#"{"generationToken":"tok","name":"invoice.pdf","templateId":"tmpl-1"}"#
    );
    let round_tripped: IndexEntry =
        serde_json::from_str(&serde_json::to_string(&both_some).unwrap()).unwrap();
    assert_eq!(round_tripped, both_some);
}

/// The struct-field seam: a renamed key is the string serde writes, which is what the object needs
/// to still close after it.
#[test]
#[cfg(feature = "typescript")]
fn test_a_hyphenated_field_key_is_written_as_a_string() {
    let ts = Message::ts_definition();

    assert!(
        ts.lines()
            .any(|line| line == r#"  "reply-to": string | undefined;"#),
        "expected the key as a string member:\n{ts}"
    );
    assert!(
        ts.lines().any(|line| line == "  id: string;"),
        "an identifier-legal key stays bare:\n{ts}"
    );
}

/// The same key on the Zod surface, which is object-literal syntax and refuses a bare hyphen for
/// the same reason the type does.
#[test]
#[cfg(feature = "zod")]
fn test_a_hyphenated_field_key_is_written_as_a_string_in_zod() {
    let zod = Message::zod_schema();

    assert!(
        zod.contains(r#"  "reply-to": "#),
        "expected the key as a string member:\n{zod}"
    );
    assert!(
        zod.contains("  id: z.string(),"),
        "an identifier-legal key stays bare:\n{zod}"
    );
}

/// The struct-variant seam: a variant's own fields are members of the variant's object and are
/// written by their own seam, which needs the same rule.
#[test]
#[cfg(feature = "typescript")]
fn test_a_hyphenated_variant_field_key_is_written_as_a_string() {
    let ts = Delivery::ts_definition();

    assert!(
        ts.lines().any(|line| line == r#"  "sent-at": string;"#),
        "expected the key as a string member:\n{ts}"
    );
    assert!(
        ts.lines().any(|line| line == r#"  kind: "Sent";"#),
        "an identifier-legal tag stays bare:\n{ts}"
    );
}

/// The untagged-member seam: a renamed field inside a `Named` untagged variant is written as the key
/// serde writes it as, not the Rust ident it never reaches the wire under. `Fresh`'s untouched field
/// stays exactly as it was.
#[test]
#[cfg(feature = "typescript")]
fn test_a_renamed_untagged_member_field_is_written_as_a_string() {
    let ts = Body::ts_definition();

    assert!(
        ts.contains(r#"export type Body = { subject: string } | { "reply-to": string };"#),
        "expected the renamed member key quoted, the untouched one bare:\n{ts}"
    );
}

/// The same key on the Zod surface.
#[test]
#[cfg(feature = "zod")]
fn test_a_renamed_untagged_member_field_is_written_as_a_string_in_zod() {
    let zod = Body::zod_schema();

    assert!(
        zod.contains(r#""reply-to": z.string()"#),
        "expected the key as a string member:\n{zod}"
    );
    assert!(
        zod.contains("subject: z.string()"),
        "an identifier-legal key stays bare:\n{zod}"
    );
}

/// The same key in the JSON schema, where quoting is moot — a property name is a plain string either
/// way — but the key itself must still be the one serde writes.
#[test]
#[cfg(feature = "jsonschema")]
fn test_a_renamed_untagged_member_field_reaches_the_json_schema() {
    let schema = Body::json_schema();
    let any_of = schema["anyOf"].as_array().unwrap();
    let reply_branch = any_of
        .iter()
        .find(|branch| {
            branch["properties"]
                .as_object()
                .unwrap()
                .contains_key("reply-to")
        })
        .unwrap();
    assert_eq!(reply_branch["required"], serde_json::json!(["reply-to"]));
    assert!(
        !reply_branch["properties"]
            .as_object()
            .unwrap()
            .contains_key("reply_to"),
        "the Rust ident leaked into the schema:\n{schema}"
    );
}

/// serde's own wire, beside the schema above: the closed document — every leaf `additionalProperties:
/// false` — accepts exactly the payload serde writes for the renamed member, and the value round-trips
/// through it.
#[test]
#[cfg(feature = "jsonschema")]
fn test_a_renamed_untagged_member_round_trips_through_its_closed_schema() {
    let value = Body::Reply {
        reply_to: "x".to_owned(),
    };
    let payload = serde_json::to_value(&value).unwrap();
    assert_eq!(payload, serde_json::json!({ "reply-to": "x" }));

    let schema = Body::json_schema();
    let any_of = schema["anyOf"].as_array().unwrap();
    let reply_branch = any_of
        .iter()
        .find(|branch| {
            branch["properties"]
                .as_object()
                .unwrap()
                .contains_key("reply-to")
        })
        .unwrap();
    let named = reply_branch["properties"].as_object().unwrap();
    let required = reply_branch["required"].as_array().unwrap();
    let written = payload.as_object().unwrap();
    assert!(written.keys().all(|key| named.contains_key(key)));
    assert!(
        required
            .iter()
            .all(|key| written.contains_key(key.as_str().unwrap()))
    );

    let back: Body = serde_json::from_value(payload).unwrap();
    assert_eq!(back, value);
}

/// The flatten-operand seam and the sibling exclusion beside it: `Reply`'s renamed field reaches
/// both as the key serde writes, quoted, while `Fresh`'s untouched field stays bare in both.
#[test]
#[cfg(feature = "typescript")]
fn test_a_flatten_operand_and_its_sibling_exclusions_carry_a_renamed_key() {
    let ts = Envelope::ts_definition();

    assert!(
        ts.contains(
            "} & ({ subject: string; \"reply-to\"?: never } | { \"reply-to\": string; subject?: never });"
        ),
        "expected the renamed key quoted on both sides of the union:\n{ts}"
    );
}

/// The rule reads the key, not the rename: `$` and a trailing digit are identifier-legal and are
/// left exactly as they were written.
#[test]
#[cfg(feature = "typescript")]
fn test_identifier_legal_renamed_keys_stay_bare() {
    let ts = PlainKeys::ts_definition();

    assert!(
        ts.lines().any(|line| line == "  $ref: string;"),
        "expected a bare `$ref`:\n{ts}"
    );
    assert!(
        ts.lines().any(|line| line == "  user_id2: string;"),
        "expected a bare `user_id2`:\n{ts}"
    );
}

/// The key serde writes for a list-form `rename`, which the surfaces below all describe.
#[test]
fn test_a_list_form_rename_writes_the_name_both_directions_share() {
    let value = ListFormRename { value: 5 };
    let payload = serde_json::to_value(&value).unwrap();
    assert_eq!(payload, serde_json::json!({ "same_name": 5_u32 }));

    let back: ListFormRename = serde_json::from_value(payload).unwrap();
    assert_eq!(back, value);
}

#[test]
#[cfg(feature = "typescript")]
fn test_a_list_form_rename_reaches_the_typescript_type() {
    let ts = ListFormRename::ts_definition();

    assert!(
        ts.lines().any(|line| line == "  same_name: number;"),
        "expected the renamed key:\n{ts}"
    );
    assert!(
        !ts.contains("value:"),
        "the Rust ident leaked into the type:\n{ts}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_a_list_form_rename_reaches_the_zod_schema() {
    let zod = ListFormRename::zod_schema();

    assert!(
        zod.contains("same_name: z.number().int()"),
        "expected the renamed key:\n{zod}"
    );
    assert!(
        !zod.contains("value:"),
        "the Rust ident leaked into the schema:\n{zod}"
    );
}

/// The closed document, read against the payload serde actually writes: every key written is named,
/// and every key required is written.
#[test]
#[cfg(feature = "jsonschema")]
fn test_a_list_form_rename_document_accepts_what_serde_writes() {
    let payload = serde_json::to_value(ListFormRename { value: 5 }).unwrap();
    let schema = ListFormRename::json_schema();
    let named = schema["properties"].as_object().unwrap();
    let required = schema["required"].as_array().unwrap();
    let written = payload.as_object().unwrap();

    assert!(named.contains_key("same_name"), "{schema}");
    assert!(
        written.keys().all(|key| named.contains_key(key)),
        "{schema}"
    );
    assert!(
        required
            .iter()
            .all(|key| written.contains_key(key.as_str().unwrap())),
        "{schema}"
    );
}

/// The same, for the rule a list-form `rename_all` names.
#[test]
fn test_a_list_form_rename_all_writes_the_rule_both_directions_share() {
    let value = ListFormRenameAll { my_field: 5 };
    let payload = serde_json::to_value(&value).unwrap();
    assert_eq!(payload, serde_json::json!({ "myField": 5_u32 }));

    let back: ListFormRenameAll = serde_json::from_value(payload).unwrap();
    assert_eq!(back, value);
}

#[test]
#[cfg(feature = "typescript")]
fn test_a_list_form_rename_all_reaches_the_typescript_type() {
    let ts = ListFormRenameAll::ts_definition();

    assert!(
        ts.lines().any(|line| line == "  myField: number;"),
        "expected the transformed key:\n{ts}"
    );
    assert!(
        !ts.contains("my_field"),
        "the Rust ident leaked into the type:\n{ts}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_a_list_form_rename_all_reaches_the_zod_schema() {
    let zod = ListFormRenameAll::zod_schema();

    assert!(
        zod.contains("myField: z.number().int()"),
        "expected the transformed key:\n{zod}"
    );
    assert!(
        !zod.contains("my_field"),
        "the Rust ident leaked into the schema:\n{zod}"
    );
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_a_list_form_rename_all_document_accepts_what_serde_writes() {
    let payload = serde_json::to_value(ListFormRenameAll { my_field: 5 }).unwrap();
    let schema = ListFormRenameAll::json_schema();
    let named = schema["properties"].as_object().unwrap();
    let required = schema["required"].as_array().unwrap();
    let written = payload.as_object().unwrap();

    assert!(named.contains_key("myField"), "{schema}");
    assert!(
        written.keys().all(|key| named.contains_key(key)),
        "{schema}"
    );
    assert!(
        required
            .iter()
            .all(|key| written.contains_key(key.as_str().unwrap())),
        "{schema}"
    );
}

/// `bound(...)` names trait bounds, not keys, so the member carrying it is on the wire exactly as
/// its plain sibling is.
#[test]
fn test_a_bound_carrying_member_is_on_the_wire_as_its_plain_sibling_is() {
    let value = BoundCarrying {
        reading: 1,
        writing: 2,
    };
    let payload = serde_json::to_value(&value).unwrap();
    assert_eq!(
        payload,
        serde_json::json!({ "reading": 1_u32, "writing": 2_u32 })
    );

    let back: BoundCarrying = serde_json::from_value(payload).unwrap();
    assert_eq!(back, value);
}

#[test]
#[cfg(feature = "typescript")]
fn test_a_bound_carrying_member_is_typed_as_its_plain_sibling_is() {
    let ts = BoundCarrying::ts_definition();

    assert_eq!(
        member_rendering(&ts, "reading"),
        member_rendering(&ts, "writing"),
        "{ts}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_a_bound_carrying_member_is_validated_as_its_plain_sibling_is() {
    let zod = BoundCarrying::zod_schema();

    assert_eq!(
        member_rendering(&zod, "reading"),
        member_rendering(&zod, "writing"),
        "{zod}"
    );
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_a_bound_carrying_member_is_described_as_its_plain_sibling_is() {
    let schema = BoundCarrying::json_schema();

    assert_eq!(
        schema["properties"]["reading"], schema["properties"]["writing"],
        "{schema}"
    );
    assert_eq!(
        schema["required"],
        serde_json::json!(["reading", "writing"])
    );
}
