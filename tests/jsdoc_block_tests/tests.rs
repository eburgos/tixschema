//! One `JSDoc` block, one shape, wherever a surface writes one.
//!
//! A block opens at the indent of the member it documents, carries its body at that same indent,
//! and closes with the delimiter JavaScript closes a block comment with. Nothing between the block
//! and what it documents, and nothing between the last member and the brace that closes the object
//! — the emitted text is what a caller writes to a `.ts` file, so a struct, a plain enum and a
//! tagged enum have to agree on all of it rather than each spelling its own.

#![cfg(feature = "typescript")]

use serde::{Deserialize, Serialize};
use tixschema::model_schema;

/// The block a documented member is written under, and the one an undocumented member falls back
/// to, both read off the same struct.
#[model_schema()]
#[derive(Serialize, Deserialize)]
struct Member {
    /// Documented member.
    documented: String,
    plain: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
enum Plain {
    Active,
    Inactive,
}

#[model_schema()]
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "camelCase"))]
enum Tagged {
    Created { at: String },
    Deleted { at: String },
}

#[model_schema()]
#[derive(Serialize, Deserialize)]
enum External {
    Nothing,
    Wrapped { at: String },
}

#[model_schema()]
#[derive(Serialize, Deserialize)]
struct Empty;

/// The one closer. `**/` closes nothing in JavaScript — it is a block comment already closed by the
/// `*/` inside it, followed by a stray `/`.
#[test]
fn no_surface_closes_a_block_with_anything_but_the_javascript_closer() {
    for emission in [
        Member::ts_definition(),
        Plain::ts_definition(),
        Tagged::ts_definition(),
        External::ts_definition(),
        Empty::ts_definition(),
    ] {
        assert!(!emission.contains("**/"), "Got: {emission}");
        assert!(emission.contains(" */"), "Got: {emission}");
    }
}

/// A member's block sits at the member's own indent, continuation lines included, so the block and
/// the member it documents read as one thing.
#[test]
fn a_members_block_is_written_at_the_members_indent() {
    let ts = Member::ts_definition();

    assert!(
        ts.contains("  /**\n   * Documented member.\n   * \n   */\n  documented: string;\n"),
        "Got: {ts}"
    );
    assert!(
        ts.contains("  /**\n   * plain\n   * \n   */\n  plain: string;\n"),
        "Got: {ts}"
    );
}

/// The item's own block is at column 0, and what it documents is the line straight beneath it — on
/// every item kind, rather than two blank lines on one and none on the others.
#[test]
fn every_item_kind_puts_its_export_on_the_line_below_its_block() {
    for emission in [
        Member::ts_definition(),
        Plain::ts_definition(),
        Tagged::ts_definition(),
        External::ts_definition(),
        Empty::ts_definition(),
    ] {
        assert!(emission.contains(" */\nexport type "), "Got: {emission}");
    }
}

/// A tagged variant's first member is a member like any other, so its block opens on its own line
/// rather than trailing the brace.
#[test]
#[cfg(feature = "serde")]
fn a_tagged_variants_first_member_opens_its_block_on_its_own_line() {
    let ts = Tagged::ts_definition();

    assert!(
        ts.contains("= {\n  /**\n   * created\n   * \n   */\n  type: \"created\";\n"),
        "Got: {ts}"
    );
    assert!(
        ts.contains("} | {\n  /**\n   * deleted\n   * \n   */\n  type: \"deleted\";\n"),
        "Got: {ts}"
    );
}

/// An externally tagged variant's key is a member too, whether the variant carries an object or is
/// the bare key a unit writes.
#[test]
#[cfg(feature = "serde")]
fn an_externally_tagged_variants_block_sits_at_its_keys_indent() {
    let ts = External::ts_definition();

    assert!(
        ts.contains("  /**\n   * Nothing\n   * \n   */\n  \"Nothing\""),
        "Got: {ts}"
    );
    assert!(
        ts.contains("{\n  /**\n   * Wrapped\n   * \n   */\n  \"Wrapped\":"),
        "Got: {ts}"
    );
}

/// Nothing stands between the last member and the brace that closes the object, on either surface.
#[test]
fn no_surface_leaves_a_blank_line_before_the_brace_that_closes_an_object() {
    let ts = Member::ts_definition();
    assert!(ts.contains("  plain: string;\n};"), "Got: {ts}");

    let tagged = Tagged::ts_definition();
    assert!(!tagged.contains(";\n\n}"), "Got: {tagged}");
}

/// The same holding on the Zod surface, where the struct emitter and the tagged-enum emitter
/// disagreed.
#[test]
#[cfg(feature = "zod")]
fn the_zod_object_closes_straight_after_its_last_member() {
    let zod = Member::zod_schema();
    assert!(zod.contains("  plain: z.string(),\n});"), "Got: {zod}");
    assert!(!zod.contains(",\n\n})"), "Got: {zod}");
}

/// Layout is all that this normalizes: what a block says is what the author wrote, and an
/// undocumented member still falls back to its own exported name.
#[test]
fn the_body_a_block_carries_is_the_one_the_author_wrote() {
    let ts = Member::ts_definition();

    assert!(ts.contains("   * Documented member.\n"), "Got: {ts}");
    assert!(ts.contains("   * plain\n"), "Got: {ts}");
}
