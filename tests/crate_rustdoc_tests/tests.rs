//! The crate rustdoc opens with four declarations and prints, beside each, what that declaration
//! emits. Each is expanded here as written and held to the run: the emission the generator
//! answers with has to still appear in `src/lib.rs` verbatim, field and variant order included,
//! since every surface writes in declaration order. Drift fails here rather than on the docs page.

#[cfg(all(
    feature = "jsonschema",
    feature = "serde",
    feature = "typescript",
    feature = "zod"
))]
use serde::{Deserialize, Serialize};
#[cfg(all(
    feature = "jsonschema",
    feature = "object_id",
    feature = "serde",
    feature = "typescript",
    feature = "zod"
))]
use std::collections::HashMap;
#[cfg(all(
    feature = "jsonschema",
    feature = "serde",
    feature = "typescript",
    feature = "zod"
))]
use tixschema::model_schema;

/// The JSON Schema each emission carries in its leading `JSDoc` block is written only while
/// `jsonschema` is on, so every pin here reads the rustdoc under the feature set the shown blocks
/// were taken from.
#[cfg(all(
    feature = "jsonschema",
    feature = "serde",
    feature = "typescript",
    feature = "zod"
))]
fn assert_rustdoc_shows(surface: &str) {
    let rustdoc = include_str!("../../src/lib.rs");
    assert!(
        rustdoc.contains(surface),
        "src/lib.rs no longer shows this emission verbatim:\n{surface}"
    );
}

#[cfg(all(
    feature = "jsonschema",
    feature = "serde",
    feature = "typescript",
    feature = "zod"
))]
#[test]
fn test_usage_section_shows_what_the_struct_emits() {
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[model_schema()]
    pub struct User {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub age: Option<u32>,
        pub first_name: String,
        pub id: String,
        pub last_name: String,
        pub roles: Vec<String>,
    }

    assert_rustdoc_shows(&User::ts_definition());
    assert_rustdoc_shows(&User::zod_schema());
}

#[cfg(all(
    feature = "jsonschema",
    feature = "serde",
    feature = "typescript",
    feature = "zod"
))]
#[test]
fn test_enum_section_shows_what_the_enum_emits() {
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "lowercase")]
    #[model_schema()]
    pub enum Status {
        Active,
        Inactive,
        Pending,
    }

    assert_rustdoc_shows(&Status::ts_definition());
    assert_rustdoc_shows(&Status::zod_schema());
}

#[cfg(all(
    feature = "jsonschema",
    feature = "serde",
    feature = "typescript",
    feature = "zod"
))]
#[test]
fn test_tagged_union_section_shows_what_the_tagged_enum_emits() {
    #[derive(Serialize, Deserialize)]
    #[serde(tag = "type", rename_all = "camelCase")]
    #[model_schema()]
    pub enum Event {
        UserCreated {
            timestamp: String,
            user_id: String,
        },
        UserDeleted {
            #[serde(skip_serializing_if = "Option::is_none")]
            reason: Option<String>,
            user_id: String,
        },
    }

    assert_rustdoc_shows(&Event::ts_definition());
    assert_rustdoc_shows(&Event::zod_schema());
}

/// The rustdoc block this pins stands in a dummy `ObjectId` rather than pulling `mongodb` in, the
/// type being recognised by name, and so does this.
#[cfg(all(
    feature = "jsonschema",
    feature = "object_id",
    feature = "serde",
    feature = "typescript",
    feature = "zod"
))]
#[test]
fn test_object_id_section_shows_what_the_object_id_struct_emits() {
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ObjectId(pub String);

    #[derive(Serialize, Deserialize)]
    #[model_schema()]
    pub struct Document {
        pub author_id: ObjectId,
        pub id: ObjectId,
        pub metadata: HashMap<String, ObjectId>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub parent_id: Option<ObjectId>,
        pub tags: Vec<ObjectId>,
        pub title: String,
    }

    assert_rustdoc_shows(&Document::ts_definition());
    assert_rustdoc_shows(&Document::zod_schema());
}
