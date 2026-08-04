//! The README's `Option` examples, expanded here as the README declares them.
//!
//! An `Option` field serde writes as `null` for a `None` is refused where it is declared, so an
//! example carrying one is not a declaration a reader can paste — and the block beside it, showing
//! the key that declaration would render, describes a build the declaration cannot be written in.
//! Each example is therefore held to two things: the README still declares it exactly as it is
//! declared here, and the emission the README shows beside it is what the generator answers with.
//! Drift on either side fails here rather than in someone's editor.
//!
//! Each type is declared with its fields ordered alphabetically, as this crate's lints require of
//! Rust source; the README orders its examples for reading. Only the order the members are written
//! in differs, so every member is held as a whole line rather than as part of a block — what is
//! pinned is the spelling of each, which is what an omitted key changes. A derive list is widened
//! here for the same reason and to the same effect on the emission, which is none: the generator
//! reads no derive but serde's.
//!
//! The README's `ts_optional` example is not here. The flag decides the key only where no serde
//! attribute is read, so the section shows a declaration this module's build cannot compile; the
//! same declares-and-shows pin sits in `optional_key_flag_tests`, gated the other way.

#![cfg(all(feature = "serde", feature = "typescript"))]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tixschema::model_schema;

fn readme() -> &'static str {
    include_str!("../../README.md")
}

/// The README declares this, and the generator writes that.
///
/// A member is matched as a whole line on both sides, so a spelling that is merely a prefix of the
/// emitted one cannot pass for it.
fn assert_readme_declares_and_shows(declaration: &str, emission: &str, members: &[&str]) {
    assert!(
        readme().contains(declaration),
        "the README no longer declares this verbatim:\n{declaration}"
    );
    for member in members {
        assert!(
            emission.lines().any(|line| line == *member),
            "the generator no longer writes this member: {member}\nGot: {emission}"
        );
        assert!(
            readme().lines().any(|line| line == *member),
            "the README no longer shows this member verbatim: {member}"
        );
    }
}

/// The "Optional Fields" example: three keys the payload may omit, beside two it may not.
#[test]
fn test_the_optional_fields_example_is_declarable_and_shows_what_it_emits() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct UserWithOptionals {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub avatar_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub email: Option<String>,
        pub id: String,
        pub name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub phone: Option<String>,
    }

    assert_readme_declares_and_shows(
        "    #[serde(default, skip_serializing_if = \"Option::is_none\")]\n    pub email: Option<String>,\n",
        &UserWithOptionals::ts_definition(),
        &[
            "export type UserWithOptionals = {",
            "  id: string;",
            "  name: string;",
            "  email?: string;",
            "  phone?: string;",
            "  avatar_url?: string;",
            "};",
        ],
    );
}

/// The "Collections and Maps" example. It shows no emission of its own — the section is about what
/// each container describes as — so what is held here is that it is a declaration at all.
#[test]
fn test_the_collections_example_is_declarable() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct UserWithCollections {
        pub id: String,
        pub metadata: HashMap<String, String>,
        pub scores: Vec<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub settings: Option<HashMap<String, String>>,
        pub tags: Vec<String>,
    }

    assert_readme_declares_and_shows(
        "    #[serde(default, skip_serializing_if = \"Option::is_none\")]\n    pub settings: Option<HashMap<String, String>>,\n",
        &UserWithCollections::ts_definition(),
        &[],
    );
}

/// The `ObjectId` example. The block it stands in a dummy `ObjectId` for the reason the crate
/// rustdoc's does: the type is recognised by name, so nothing here needs `mongodb` pulled in.
#[test]
#[cfg(feature = "object_id")]
fn test_the_object_id_example_is_declarable_and_shows_what_it_emits() {
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ObjectId(pub String);

    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct Document {
        pub author_id: ObjectId,
        pub id: ObjectId,
        pub metadata: HashMap<String, ObjectId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub parent_id: Option<ObjectId>,
        pub related_docs: HashMap<String, Vec<ObjectId>>,
        pub tags: Vec<ObjectId>,
        pub title: String,
    }

    assert_readme_declares_and_shows(
        "    #[serde(default, skip_serializing_if = \"Option::is_none\")]\n    pub parent_id: Option<ObjectId>,\n",
        &Document::ts_definition(),
        &[
            "export type Document = {",
            "  id: ObjectId;",
            "  title: string;",
            "  author_id: ObjectId;",
            "  tags: Array<ObjectId>;",
            "  metadata: Partial<Record<string, ObjectId>>;",
            "  parent_id?: ObjectId;",
            "  related_docs: Partial<Record<string, Array<ObjectId>>>;",
            "};",
        ],
    );

    #[cfg(feature = "zod")]
    assert_readme_declares_and_shows(
        "pub struct Document {",
        &Document::zod_schema(),
        &[
            "  parent_id: z.union([z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: \"Invalid ObjectId\" }) }), z.undefined()]).prefault(undefined),",
        ],
    );
}

/// The chrono example, whose `DateTime<Tz>` renders as a native `Date` and whose `as_number` field
/// renders as the epoch-milliseconds number beside it.
#[test]
#[cfg(feature = "chrono")]
fn test_the_chrono_example_is_declarable_and_shows_what_it_emits() {
    use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Event {
        pub created_at: DateTime<Utc>,
        pub date: NaiveDate,
        #[model_schema_prop(as_number)]
        pub epoch_ms: DateTime<Utc>,
        pub local_datetime: NaiveDateTime,
        pub name: String,
        pub time: NaiveTime,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub updated_at: Option<DateTime<Utc>>,
    }

    assert_readme_declares_and_shows(
        "    #[serde(default, skip_serializing_if = \"Option::is_none\")]\n    pub updated_at: Option<DateTime<Utc>>,\n",
        &Event::ts_definition(),
        &[
            "export type Event = {",
            "  name: string;",
            "  date: string;",
            "  time: string;",
            "  local_datetime: string;",
            "  created_at: Date;",
            "  epoch_ms: number;",
            "  updated_at?: Date;",
            "};",
        ],
    );

    #[cfg(feature = "zod")]
    assert_readme_declares_and_shows(
        "pub struct Event {",
        &Event::zod_schema(),
        &[
            "  created_at: z.coerce.date(),",
            "  updated_at: z.union([z.coerce.date(), z.undefined()]).prefault(undefined),",
        ],
    );
}

/// The README declares this, and the block beside it is the emission whole.
///
/// Held as one block rather than line by line, which is what a truncated paste needs: an emission
/// shown down to its seventh line matches every line it shows and still leaves the reader without
/// the factory the rest of the block declares.
fn assert_readme_declares_and_shows_whole(declaration: &str, emission: &str) {
    assert!(
        readme().contains(declaration),
        "the README no longer declares this verbatim:\n{declaration}"
    );
    assert!(
        readme().contains(emission.trim_end()),
        "the README no longer shows this emission whole:\n{emission}"
    );
}

/// The "Branded Newtypes" example: a generic brand beside a non-generic one, and the whole of what
/// each publishes.
///
/// A generic brand publishes a factory, so its emission runs well past the builder the block used
/// to stop at — the type alias reading the builder's return back, the two-method cache interface,
/// the cache itself and the exported factory. All of it is what a reader pastes.
#[test]
#[cfg(feature = "zod")]
fn test_the_branded_newtype_example_is_declarable_and_shows_what_it_emits() {
    #[model_schema(default_types(IdType = String))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct UserId<IdType>(pub IdType);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct CorrelationId(pub String);

    assert_readme_declares_and_shows_whole(
        "pub struct UserId<IdType>(pub IdType);",
        &UserId::<String>::ts_definition(),
    );
    assert_readme_declares_and_shows_whole(
        "pub struct UserId<IdType>(pub IdType);",
        &UserId::<String>::zod_schema(),
    );
    assert_readme_declares_and_shows_whole(
        "pub struct CorrelationId(pub String);",
        &CorrelationId::ts_definition(),
    );
    assert_readme_declares_and_shows_whole(
        "pub struct CorrelationId(pub String);",
        &CorrelationId::zod_schema(),
    );
}

/// The same two declarations in a build with no `zod`, where the brand is a `unique symbol` the
/// type is intersected with rather than a marker Zod's `.brand()` carries — the block the README
/// shows under "without `zod` feature", and the one this build is the only one that can run.
#[test]
#[cfg(not(feature = "zod"))]
fn test_the_branded_newtype_example_shows_what_a_build_without_zod_emits() {
    #[model_schema(default_types(IdType = String))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct UserId<IdType>(pub IdType);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct CorrelationId(pub String);

    assert_readme_declares_and_shows_whole(
        "pub struct UserId<IdType>(pub IdType);",
        &UserId::<String>::ts_definition(),
    );
    assert_readme_declares_and_shows_whole(
        "pub struct CorrelationId(pub String);",
        &CorrelationId::ts_definition(),
    );
}

/// The "Doc Comments and Examples on Branded Newtypes" example, whose docs and `rust example` block
/// reach the factory's builder as a description and an example — and whose factory below them is
/// the same whole block the plain generic brand publishes.
#[test]
#[cfg(feature = "zod")]
fn test_the_documented_branded_newtype_example_is_declarable_and_shows_what_it_emits() {
    /// Generic document identifier.
    ///
    /// - `DocumentId<String>` for API/HTTP layer
    /// - `DocumentId<ObjectId>` for MongoDB layer
    ///
    /// ```rust example
    /// DocumentId("64de3d95ff45b119e5b53a7e".to_string())
    /// ```
    #[model_schema(default_types(IdType = String))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct DocumentId<IdType>(pub IdType);

    assert_readme_declares_and_shows_whole(
        "pub struct DocumentId<IdType>(pub IdType);",
        &DocumentId::<String>::zod_schema(),
    );
}

/// The rejected half of the constrained-brand rule for an inner written over a parameter. Only its
/// spelling can be held here — the declaration itself is a `compile_error!` by construction, which
/// is what the rule says it should be — so what this pins is that the README still shows the two
/// declarations the refusal is written about.
#[test]
fn test_the_constrained_brand_over_a_parameterised_inner_example_is_still_shown() {
    assert!(
        readme().contains("pub struct TaggedSlug<T>(pub Tagged<T>);"),
        "the README no longer shows the rejected declaration"
    );
    assert!(
        readme().contains("pub struct Tagged<T>(pub T);"),
        "the README no longer shows the inner the rejected declaration is written over"
    );
}
