use super::*;

/// The pattern shapes the shipped tests write, none of which the two grammars spell differently.
/// Every one of them has to come back byte for byte: the guard is there to stop a pattern only one
/// grammar reads, not to touch the ones both already read the same way.
const PORTABLE_PATTERNS: [&str; 10] = [
    "^[a-z]+$",
    "^[0-9a-fA-F]{24}$",
    r"^\d{3}\.\d{3}\.\d{3}-\d{2}$",
    r"^\/[a-z]+$",
    "^/[a-z]+$",
    r"^a\n[a-z]+$",
    "(?<word>[a-z]+)",
    // `(?P<` inside a class is four ordinary members, and the rename must not reach into one.
    "[(?P<]",
    "[--/]",
    r"[a\]b]",
];

/// Every construct the guard refuses, beside the words the refusal has to name it by.
///
/// The list is the inventory of what `regex::Regex::new` accepts and a JavaScript regex literal
/// either fails to parse or reads as something else, so it doubles as the record of what was
/// checked against both grammars.
const UNPORTABLE_PATTERNS: [(&str, &str); 22] = [
    ("(?i)abc", "inline flag directive"),
    ("abc(?i)def", "inline flag directive"),
    ("(?x:a b)", "ignore-whitespace flag"),
    ("(?U:a+)", "swap-greed flag"),
    ("(?u:a)", "Unicode flag"),
    ("(?R:a)", "CRLF flag"),
    (r"\Aabc", r"`\A` anchor"),
    (r"abc\z", r"`\z` anchor"),
    (r"\b{start}a", "word boundary"),
    (r"\<a", "word boundary"),
    (r"\p{L}", "Unicode class"),
    (r"\pL", "Unicode class"),
    (r"[\p{L}]", "Unicode class"),
    ("[[:alpha:]]", "POSIX class"),
    (r"[\w&&\d]", "`&&` class intersection"),
    ("[+--]", "`--` class difference"),
    (r"[\d~~\w]", "`~~` class symmetric difference"),
    ("[a[b]]", "class nested inside another class"),
    (r"\x{41}", "braced code point escape"),
    ("[]]", "unescaped `]` opening a character class"),
    ("[^]]", "unescaped `]` opening a character class"),
    ("[]-a]", "unescaped `]` opening a character class"),
];

/// The haystacks a rewritten pattern is held to: whatever the original matched among them, the
/// rewrite has to match too.
const REWRITE_HAYSTACKS: [&str; 12] = [
    "", "a", "abc", "]", "]]", "a]", "-", "/", "a-b", "word-42", "AB", "[",
];

#[cfg(feature = "zod")]
#[test]
fn test_extract_example_simple() {
    let docs = vec![
        "User profile".to_owned(),
        "```rust example".to_owned(),
        "User { name: \"John\".to_string(), age: 25 }".to_owned(),
        "```".to_owned(),
    ];

    let example = extract_example_from_docs(&docs);
    assert!(example.is_some());
    assert_eq!(
        example.unwrap(),
        "User { name: \"John\".to_string(), age: 25 }"
    );
}

#[cfg(feature = "zod")]
#[test]
fn test_extract_example_multiline() {
    let docs = vec![
        "Complex example".to_owned(),
        "```rust example".to_owned(),
        "let x = 5;".to_owned(),
        "let y = 10;".to_owned(),
        "User { age: x + y }".to_owned(),
        "```".to_owned(),
    ];

    let example = extract_example_from_docs(&docs);
    assert!(example.is_some());
    let code = example.unwrap();
    assert!(code.contains("let x = 5;"));
    assert!(code.contains("let y = 10;"));
    assert!(code.contains("User { age: x + y }"));
}

#[cfg(feature = "zod")]
#[test]
fn test_extract_example_first_only() {
    let docs = vec![
        "Multiple examples".to_owned(),
        "```rust example".to_owned(),
        "User { age: 25 }".to_owned(),
        "```".to_owned(),
        "Another description".to_owned(),
        "```rust example".to_owned(),
        "User { age: 30 }".to_owned(),
        "```".to_owned(),
    ];

    let example = extract_example_from_docs(&docs);
    assert!(example.is_some());
    assert_eq!(example.unwrap(), "User { age: 25 }");
}

#[cfg(feature = "zod")]
#[test]
fn test_extract_example_none() {
    let docs = vec!["User profile".to_owned(), "No examples here".to_owned()];

    let example = extract_example_from_docs(&docs);
    assert!(example.is_none());
}

#[cfg(feature = "zod")]
#[test]
fn test_extract_example_empty_docs() {
    let docs: Vec<String> = vec![];
    let example = extract_example_from_docs(&docs);
    assert!(example.is_none());
}

#[cfg(feature = "zod")]
#[test]
fn test_extract_example_regular_code_fence_ignored() {
    let docs = vec![
        "Example with regular fence".to_owned(),
        "```rust".to_owned(),
        "User { age: 25 }".to_owned(),
        "```".to_owned(),
    ];

    let example = extract_example_from_docs(&docs);
    assert!(example.is_none());
}

#[cfg(feature = "zod")]
#[test]
fn test_transform_println_pattern() {
    let code = concat!(
        "let data_type = DataType::Integer;\n",
        "println!(\"data_type: {",
        ":?}\", data_type);"
    );
    let result = transform_example_code(code);
    assert_eq!(result, "let data_type = DataType::Integer;\ndata_type");
}

#[cfg(feature = "zod")]
#[test]
fn test_transform_let_underscore_pattern() {
    let code = "let _: DataType = DataType::Integer;";
    let result = transform_example_code(code);
    assert_eq!(result, "DataType::Integer");
}

#[cfg(feature = "zod")]
#[test]
fn test_transform_let_underscore_no_type() {
    let code = "let _ = DataType::Integer;";
    let result = transform_example_code(code);
    assert_eq!(result, "DataType::Integer");
}

#[cfg(feature = "zod")]
#[test]
fn test_transform_no_pattern_match() {
    let code = "DataType::Integer";
    let result = transform_example_code(code);
    assert_eq!(result, "DataType::Integer");
}

#[cfg(feature = "zod")]
#[test]
fn test_transform_strips_use_statements() {
    let code = concat!(
        "use crate::definition::DataType;\n\n",
        "let data_type = DataType::Integer;\n",
        "println!(\"data_type: {",
        ":?}\", data_type);"
    );
    let result = transform_example_code(code);
    assert_eq!(result, "let data_type = DataType::Integer;\ndata_type");
}

#[cfg(feature = "zod")]
#[test]
fn test_transform_strips_multiple_use_statements() {
    let code = concat!(
        "use crate::definition::DataType;\n",
        "use std::collections::HashMap;\n\n",
        "let data_type = DataType::Integer;\n",
        "println!(\"data_type: {",
        ":?}\", data_type);"
    );
    let result = transform_example_code(code);
    assert_eq!(result, "let data_type = DataType::Integer;\ndata_type");
}

#[test]
fn test_strip_examples_removes_example_blocks() {
    let docs = vec![
        "User profile".to_owned(),
        "Some description".to_owned(),
        "```rust example".to_owned(),
        "User { name: \"John\".to_string() }".to_owned(),
        "```".to_owned(),
        "More description".to_owned(),
    ];

    let result = strip_examples_from_docs(&docs);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0], "User profile");
    assert_eq!(result[1], "Some description");
    assert_eq!(result[2], "More description");
}

#[test]
fn test_strip_examples_multiple_blocks() {
    let docs = vec![
        "Description".to_owned(),
        "```rust example".to_owned(),
        "Example 1".to_owned(),
        "```".to_owned(),
        "Between".to_owned(),
        "```rust example".to_owned(),
        "Example 2".to_owned(),
        "```".to_owned(),
        "End".to_owned(),
    ];

    let result = strip_examples_from_docs(&docs);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0], "Description");
    assert_eq!(result[1], "Between");
    assert_eq!(result[2], "End");
}

#[test]
fn test_strip_examples_no_examples() {
    let docs = vec!["User profile".to_owned(), "Some description".to_owned()];

    let result = strip_examples_from_docs(&docs);
    assert_eq!(result.len(), 2);
    assert_eq!(result, docs);
}

#[cfg(feature = "zod")]
#[test]
fn test_escape_js_regex_literal_escapes_a_bare_delimiter() {
    assert_eq!(escape_js_regex_literal("^/[a-z]+$"), r"^\/[a-z]+$");
    assert_eq!(escape_js_regex_literal("a/b/c"), r"a\/b\/c");
}

#[cfg(feature = "zod")]
#[test]
fn test_escape_js_regex_literal_leaves_a_slashless_pattern_untouched() {
    assert_eq!(
        escape_js_regex_literal(r"^\d{3}\.\d{3}-\d{2}$"),
        r"^\d{3}\.\d{3}-\d{2}$"
    );
}

#[cfg(feature = "zod")]
#[test]
fn test_escape_js_regex_literal_does_not_double_escape() {
    assert_eq!(escape_js_regex_literal(r"^\/[a-z]+$"), r"^\/[a-z]+$");
}

#[cfg(feature = "zod")]
#[test]
fn test_escape_js_regex_literal_consumes_a_backslash_escape_whole() {
    assert_eq!(escape_js_regex_literal(r"^\\/x$"), r"^\\\/x$");
    assert_eq!(escape_js_regex_literal(r"trailing\"), r"trailing\");
}

#[cfg(feature = "zod")]
#[test]
fn test_escape_js_regex_literal_escapes_every_raw_line_terminator() {
    assert_eq!(escape_js_regex_literal("^a\nb$"), r"^a\nb$");
    assert_eq!(escape_js_regex_literal("^a\rb$"), r"^a\rb$");
    assert_eq!(escape_js_regex_literal("^a\u{2028}b$"), r"^a\u2028b$");
    assert_eq!(escape_js_regex_literal("^a\u{2029}b$"), r"^a\u2029b$");
    assert_eq!(escape_js_regex_literal("a\r\nb"), r"a\r\nb");
}

#[cfg(feature = "zod")]
#[test]
fn test_escape_js_regex_literal_leaves_an_authored_line_terminator_escape_alone() {
    assert_eq!(escape_js_regex_literal(r"^a\nb$"), r"^a\nb$");
    assert_eq!(escape_js_regex_literal(r"^a\u2028b$"), r"^a\u2028b$");
}

#[cfg(feature = "zod")]
#[test]
fn test_escape_js_regex_literal_rewrites_an_identity_escaped_line_terminator() {
    assert_eq!(escape_js_regex_literal("^a\\\nb$"), r"^a\nb$");
    assert_eq!(escape_js_regex_literal("^a\\\\\nb$"), r"^a\\\nb$");
}

/// The module name a reference assumes for a name it has not seen and the one an alias goes on to
/// publish are the same call, so nothing the alias is written with can pull them apart: the `Type`
/// suffix an alias exports under and a `name = "…"` override both land on the export name only.
/// The `Json` suffix is still read through, that being what every reference already spells.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn ident_schema_module_name_is_derived_from_the_ident_alone() {
    assert_eq!(ident_schema_module_name("LaterAlias"), "later_alias_schema");
    assert_eq!(
        ident_schema_module_name("LaterAliasJson"),
        "later_alias_schema"
    );
    assert_eq!(
        compute_alias_export_name("LaterAlias", None),
        "LaterAliasType"
    );
    assert_eq!(
        compute_alias_export_name("LaterAlias", Some("Renamed")),
        "Renamed"
    );
}

/// A declared item's module used to be named from its export name, so the move is a move only for
/// an item carrying an override: without one the export name *is* the ident, and both spellings
/// land on the same module. That is what keeps every unrenamed item's published path unchanged.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn an_unrenamed_item_publishes_the_same_module_under_either_derivation() {
    for ident in ["LaterItem", "LaterItemJson", "HTTPHeader", "A"] {
        assert_eq!(
            ident_schema_module_name(ident),
            format!(
                "{}_schema",
                to_snake_case(&compute_item_export_name(ident, None))
            ),
            "for: {ident}"
        );
    }
    assert_ne!(
        ident_schema_module_name("LaterItem"),
        format!(
            "{}_schema",
            to_snake_case(&compute_item_export_name("LaterItem", Some("RenamedLater")))
        ),
    );
}

/// The ident re-export answers at the same spelling the module seam answers at: whatever a
/// reference falls back to when the registry cannot help it, which is the ident with the `Json`
/// suffix read through. An item exported under that spelling already answers there and publishes
/// nothing.
#[cfg(feature = "typescript")]
#[test]
fn a_ts_reexport_is_written_only_where_the_export_moved_off_the_ident() {
    assert_eq!(
        ident_reexport_ts("LaterAlias", "LaterAliasType", ""),
        "\n\nexport type LaterAlias = LaterAliasType;"
    );
    assert_eq!(
        ident_reexport_ts("LaterItem", "RenamedLater", ""),
        "\n\nexport type LaterItem = RenamedLater;"
    );
    assert_eq!(
        ident_reexport_ts("Pair", "PairType", "<A, B>"),
        "\n\nexport type Pair<A, B> = PairType<A, B>;"
    );
    for (ident, export) in [("PlainItem", "PlainItem"), ("PlainItemJson", "PlainItem")] {
        assert_eq!(ident_reexport_ts(ident, export, ""), "", "for: {ident}");
    }
}

#[cfg(feature = "zod")]
#[test]
fn a_zod_reexport_is_written_only_where_the_export_moved_off_the_ident() {
    assert_eq!(
        ident_reexport_zod("LaterAlias", "LaterAliasType"),
        "\n\nexport const LaterAlias$Schema = LaterAliasType$Schema;"
    );
    for (ident, export) in [("PlainItem", "PlainItem"), ("PlainItemJson", "PlainItem")] {
        assert_eq!(ident_reexport_zod(ident, export), "", "for: {ident}");
    }
}

/// Runs the `pattern` guard over `pattern` written as the literal an author would have written,
/// which is what the guard spans its refusals on.
fn portable(pattern: &str) -> Result<String, String> {
    let lit = LitStr::new(pattern, proc_macro2::Span::call_site());
    portable_pattern(&lit).map_err(|rejection| rejection.to_string())
}

/// `(?P<name>` is Rust's and Python's spelling of the group JavaScript spells `(?<name>`, and the
/// `regex` crate reads both, so the one spelling that reaches every surface is the shared one.
#[test]
fn test_portable_pattern_translates_the_rust_named_group_spelling() {
    assert_eq!(portable("(?P<w>[a-z]+)").unwrap(), "(?<w>[a-z]+)");
    assert_eq!(
        portable("^(?P<w>[a-z]+)-(?P<n>[0-9]+)$").unwrap(),
        "^(?<w>[a-z]+)-(?<n>[0-9]+)$"
    );
    assert_eq!(
        portable("(?P<outer>(?P<inner>a))").unwrap(),
        "(?<outer>(?<inner>a))"
    );
    // The group's span is a byte offset, so a multi-byte character ahead of it must not shift the
    // `P` the rewrite drops.
    assert_eq!(portable("\u{e9}(?P<w>a)").unwrap(), "\u{e9}(?<w>a)");
}

/// A class-opening `]` is refused rather than escaped because escaping it is not local: `[]-a]` is
/// the three members `]`, `-` and `a`, and `[\]-a]` is the range `]` to `a`. Pinned here so a
/// later attempt to "just escape it" fails instead of quietly widening the constraint.
#[test]
fn test_portable_pattern_does_not_escape_a_class_opening_bracket() {
    let escaped = regex::Regex::new(r"[\]-a]").unwrap();
    let written = regex::Regex::new("[]-a]").unwrap();
    assert!(escaped.is_match("_"));
    assert!(!written.is_match("_"));
    portable("[]-a]").unwrap_err();
}

/// A pattern both grammars already read the same way comes back untouched, byte for byte.
#[test]
fn test_portable_pattern_leaves_a_shared_pattern_byte_identical() {
    for pattern in PORTABLE_PATTERNS {
        assert_eq!(portable(pattern).as_deref(), Ok(pattern), "for {pattern}");
    }
}

/// `.`, `\d`, `\w`, `\s` and `\b` are in both grammars and part ways only over what counts as a
/// digit, a word character or a boundary. That is a divergence in what they match, not in what
/// they are, and the guard deliberately leaves it alone.
#[test]
fn test_portable_pattern_admits_the_classes_both_grammars_spell() {
    for pattern in [r"^\d+$", r"^\w+$", r"^\s+$", "^.$", r"\ba", r"\Ba"] {
        assert_eq!(portable(pattern).as_deref(), Ok(pattern), "for {pattern}");
    }
}

/// Every refusal names the construct that earned it and the surface that cannot carry it.
#[test]
fn test_portable_pattern_names_the_construct_javascript_cannot_carry() {
    for (pattern, construct) in UNPORTABLE_PATTERNS {
        assert!(
            regex::Regex::new(pattern).is_ok(),
            "{pattern} is not a pattern the `regex` crate accepts, so it probes nothing"
        );
        let rejection = portable(pattern).unwrap_err();
        for needle in [construct, "JavaScript regex literal", "Zod", "JSON Schema"] {
            assert!(
                rejection.contains(needle),
                "{needle} missing for {pattern}: {rejection}"
            );
        }
    }
}

/// A rewrite is a change of spelling, not of meaning: what the pattern matched before it, it
/// matches after — read off `regex::Regex` itself rather than restated here.
#[test]
fn test_portable_pattern_rewrite_keeps_what_the_pattern_matched() {
    for pattern in [
        "(?P<w>[a-z]+)",
        "^(?P<w>[a-z]+)-(?P<n>[0-9]+)$",
        "(?P<outer>(?P<inner>a))",
        r"(?P<w>[a-z]+)|(?P<n>\d+)",
    ] {
        let rewritten = portable(pattern).unwrap();
        let before = regex::Regex::new(pattern).unwrap();
        let after = regex::Regex::new(&rewritten).unwrap();
        for haystack in REWRITE_HAYSTACKS {
            assert_eq!(
                before.is_match(haystack),
                after.is_match(haystack),
                "{pattern} and {rewritten} part ways over {haystack:?}"
            );
        }
    }
}

/// A pattern the `regex` crate cannot parse is still refused with the crate's own words, ahead of
/// any question of what JavaScript would make of it.
#[test]
fn test_portable_pattern_quotes_the_regex_crate_on_a_pattern_it_refuses() {
    let rejection = portable(r"^ab\").unwrap_err();
    for needle in [
        "pattern",
        "regex parse error",
        "incomplete escape sequence",
        "panic",
    ] {
        assert!(rejection.contains(needle), "{needle} missing: {rejection}");
    }
}
