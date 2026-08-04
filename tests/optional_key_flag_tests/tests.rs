//! `ts_optional` writes the optional key for the one field nothing else says may be absent.
//!
//! An `Option<T>` whose serde attributes drop its key already renders `field?: T` off the wire, so
//! on such a field the flag decides nothing — every shape below carries a flagged member beside an
//! unflagged control of the same type and the same attributes, and the two render the same line.
//! What is left is the `Option<T>` carrying no key-dropping attribute at all, and that is exactly
//! the field the `serde` feature's `Option`-null guard refuses, because serde writes its `None` as
//! `null` while the generated schema admits only the absent key. So the flag has one live shape and
//! it is a build with the `serde` feature off, where no attribute is read and no such guard runs.
//!
//! That is what makes this a two-flavour question rather than a TypeScript one, and both halves are
//! held here so neither can drift into claiming the other's ground.

#[cfg(feature = "serde")]
mod already_decided {
    use super::member;
    use serde::{Deserialize, Serialize};
    use tixschema::model_schema;

    /// The key is dropped by a predicate. `a` asks for the optional key as well; `b` does not.
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct UnderAPredicate {
        #[model_schema_prop(ts_optional)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        a: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        b: Option<String>,
    }

    /// The key is dropped unconditionally on the way out.
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct UnderSkipSerializing {
        #[model_schema_prop(ts_optional)]
        #[serde(skip_serializing)]
        a: Option<String>,
        #[serde(skip_serializing)]
        b: Option<String>,
    }

    /// The key is dropped in both directions, which takes both members off the surface entirely —
    /// the flag has no member left to spell.
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct UnderSkip {
        #[model_schema_prop(ts_optional)]
        #[serde(skip)]
        a: Option<String>,
        #[serde(skip)]
        b: Option<String>,
        id: String,
    }

    /// On every shape the `serde` feature accepts, the flagged member and its control are one line.
    #[test]
    fn the_flag_writes_nothing_the_omission_attribute_has_not_written() {
        let predicate = UnderAPredicate::ts_definition();
        assert!(member(&predicate, "a?: string;"), "Got: {predicate}");
        assert!(member(&predicate, "b?: string;"), "Got: {predicate}");

        let skip_serializing = UnderSkipSerializing::ts_definition();
        assert!(
            member(&skip_serializing, "a?: string;"),
            "Got: {skip_serializing}"
        );
        assert!(
            member(&skip_serializing, "b?: string;"),
            "Got: {skip_serializing}"
        );
    }

    /// A key the wire carries in neither direction has no member for the flag to spell, flagged or
    /// not.
    #[test]
    fn a_key_off_the_wire_entirely_leaves_the_flag_nothing_to_spell() {
        let ts = UnderSkip::ts_definition();

        assert!(!ts.contains("a?"), "Got: {ts}");
        assert!(!ts.contains("b?"), "Got: {ts}");
        assert!(member(&ts, "id: string;"), "Got: {ts}");
    }
}

#[cfg(not(feature = "serde"))]
mod the_flag_decides {
    use super::member;
    use tixschema::model_schema;

    /// The shape the flag is for: an `Option<T>` no attribute says anything about. Declarable only
    /// here — under the `serde` feature the `Option`-null guard refuses both of these fields.
    ///
    /// Declared with its fields alphabetically, as this crate's lints require of Rust source; the
    /// README orders the same three for reading. Only the written order differs, so each member is
    /// held as a whole line rather than as part of a block.
    #[model_schema()]
    #[derive(Debug, Clone, PartialEq)]
    struct Profile {
        name: String,
        nick_handle: Option<String>,
        #[model_schema_prop(ts_optional)]
        nickname: Option<String>,
    }

    /// The flagged member takes the optional key and the control keeps the always-written one, so
    /// the flag is the whole of the difference between the two lines.
    #[test]
    fn the_flag_writes_the_optional_key_for_a_field_nothing_else_speaks_for() {
        let ts = Profile::ts_definition();

        assert!(member(&ts, "nickname?: string;"), "Got: {ts}");
        assert!(member(&ts, "nick_handle: string | undefined;"), "Got: {ts}");
        assert!(member(&ts, "name: string;"), "Got: {ts}");
    }

    /// The README documents the flag against this build, and shows the split above. Drift on either
    /// side fails here rather than in a reader's editor.
    #[test]
    fn the_readme_declares_the_shape_the_flag_decides_and_shows_what_it_emits() {
        let readme = include_str!("../../README.md");

        assert!(
            readme.contains(
                "    #[model_schema_prop(ts_optional)]\n    pub nickname: Option<String>,\n    \
                 pub nick_handle: Option<String>,\n"
            ),
            "the README no longer declares the no-serde shape verbatim"
        );
        for line in ["  nickname?: string;", "  nick_handle: string | undefined;"] {
            assert!(
                readme.lines().any(|written| written == line),
                "the README no longer shows this member verbatim: {line}"
            );
        }
    }

    /// Only TypeScript. Both members are already optional in the other two surfaces, and the flag
    /// leaves both exactly as they stand.
    #[test]
    #[cfg(feature = "zod")]
    fn the_flag_leaves_zod_as_it_stands() {
        let zod = Profile::zod_schema();

        assert!(
            zod.contains("nickname: z.union([z.string(), z.undefined()]).prefault(undefined),"),
            "Got: {zod}"
        );
        assert!(
            zod.contains("nick_handle: z.union([z.string(), z.undefined()]).prefault(undefined),"),
            "Got: {zod}"
        );
    }

    #[test]
    #[cfg(feature = "jsonschema")]
    fn the_flag_leaves_the_json_schema_as_it_stands() {
        let schema = Profile::json_schema();

        assert_eq!(
            schema["required"],
            serde_json::json!(["name"]),
            "Got: {schema}"
        );
        assert_eq!(
            schema["properties"]["nickname"], schema["properties"]["nick_handle"],
            "Got: {schema}"
        );
    }
}

/// Whether the emission writes this member, matched as a whole line so a spelling that merely
/// starts the same cannot pass for it — `nickname?: string;` and `nickname: string | undefined;`
/// are the two answers this file is about, and neither may be read off a prefix of the other.
fn member(ts: &str, line: &str) -> bool {
    ts.lines().any(|written| written.trim() == line)
}
