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

// ========================================================================
// Discriminated Union with Serde
// ========================================================================

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

// ========================================================================
// Different rename_all Conventions
// Note: Currently only camelCase and lowercase are fully implemented
// lowercase converts field names to lowercase while keeping underscores
// ========================================================================

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
struct Lowercase {
    field_one: String,
    field_two: String,
}

// ========================================================================
// Serde with Optional Fields
// ========================================================================

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

// ========================================================================
// Option Fields That Satisfy the None-Serialization Guard
// ========================================================================

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

// ========================================================================
// Enum Serde Tests
// ========================================================================

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
enum Status {
    Active,
    Inactive,
    Pending,
}

// ========================================================================
// Basic Serde Rename Tests
// ========================================================================

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

// ========================================================================
// Wire Names That No Identifier Can Hold
// ========================================================================

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
enum Body {
    Fresh { subject: String },
    Reply { reply_to: String },
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

    // Check that serde rename attributes are applied
    assert!(properties.contains_key("userId")); // user_id -> userId
    assert!(properties.contains_key("firstName")); // first_name -> firstName
    assert!(properties.contains_key("lastName")); // last_name -> lastName
    assert!(properties.contains_key("emailAddress")); // email -> emailAddress (manual rename)
    assert!(properties.contains_key("createdAt")); // created_at -> createdAt
    assert!(properties.contains_key("isVerified")); // is_verified -> isVerified

    // Check that original field names are NOT present
    assert!(!properties.contains_key("user_id"));
    assert!(!properties.contains_key("first_name"));
    assert!(!properties.contains_key("last_name"));
    assert!(!properties.contains_key("email"));
    assert!(!properties.contains_key("created_at"));
    assert!(!properties.contains_key("is_verified"));

    // Verify field types
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

    // Check that field names are converted in TypeScript
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
    assert!(ts.contains("optionalField?: string;"));
    assert!(ts.contains("customOptional?: number;"));
}

#[test]
#[cfg(feature = "zod")]
fn test_serde_with_optional_fields_zod() {
    let zod = OptionalFields::zod_schema();

    assert!(zod.contains("requiredField: z.string()"));
    assert!(zod.contains("optionalField: z.union([z.string(), z.undefined()])"));
    assert!(zod.contains("customOptional: z.union([z.number().int(), z.undefined()])"));
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

    // Check discriminator field with camelCase applied to variant names
    assert!(ts.contains("type: \"userCreated\""));
    assert!(ts.contains("type: \"userDeleted\""));
    assert!(ts.contains("type: \"userUpdated\""));

    // Check renamed fields
    assert!(ts.contains("userId: string;"));
    assert!(ts.contains("userName: string;"));
    assert!(ts.contains("newEmail: string;"));
}

#[test]
#[cfg(feature = "zod")]
fn test_discriminated_union_with_serde_zod() {
    let zod = Event::zod_schema();

    assert!(zod.contains("z.discriminatedUnion"));
    assert!(zod.contains("\"type\""));
    assert!(zod.contains("userId"));
    assert!(zod.contains("userName"));
    assert!(zod.contains("newEmail"));
}

#[test]
#[cfg(feature = "typescript")]
fn test_compliant_optionals_typescript() {
    let ts = CompliantOptionals::ts_definition();

    // The `skip_serializing_if` is what makes the key optional; `ts_optional` on `tag` asks for the
    // same spelling and so adds nothing on top of it.
    assert!(
        ts.contains("tag?: string;"),
        "expected `tag?: string;`:\n{ts}"
    );
    assert!(
        ts.contains("note?: string;"),
        "expected `note?: string;`:\n{ts}"
    );
    assert!(
        !ts.contains(" | undefined"),
        "no member of this type keeps a key serde may drop:\n{ts}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_compliant_optionals_zod_keeps_undefined_union() {
    let zod = CompliantOptionals::zod_schema();

    assert!(zod.contains("z.strictObject"));
    assert!(zod.contains("z.union([z.string(), z.undefined()]).prefault(undefined)"));
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

/// The struct-field seam: a renamed key is the string serde writes, which is what the object needs
/// to still close after it.
#[test]
#[cfg(feature = "typescript")]
fn test_a_hyphenated_field_key_is_written_as_a_string() {
    let ts = Message::ts_definition();

    assert!(
        ts.lines().any(|line| line == r#"  "reply-to"?: string;"#),
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

/// The flatten-operand seam and the sibling exclusion beside it, both written from keys an
/// identifier can hold: the whole intersection is spelled as it always was.
///
/// The quoted half of this seam is held in the unit tests, against the closer directly. It cannot be
/// reached from here: a key on this path is the field's own name whatever serde was told to rename
/// it to, so no rename spells one an identifier cannot hold.
#[test]
#[cfg(feature = "typescript")]
fn test_a_flatten_operand_and_its_sibling_exclusions_stay_bare_for_identifier_keys() {
    let ts = Envelope::ts_definition();

    assert!(
        ts.contains(
            "} & ({ subject: string; reply_to?: never } | { reply_to: string; subject?: never });"
        ),
        "expected the intersection unchanged:\n{ts}"
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
