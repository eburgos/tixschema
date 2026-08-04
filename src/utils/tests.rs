use super::*;

/// The pattern shapes the shipped tests write, none of which the two grammars spell differently.
/// Every one of them has to come back byte for byte: the guard is there to stop a pattern only one
/// grammar reads, not to touch the ones both already read the same way.
const PORTABLE_PATTERNS: [&str; 9] = [
    "^[a-z]+$",
    "^[0-9a-fA-F]{24}$",
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
const UNPORTABLE_PATTERNS: [(&str, &str); 32] = [
    ("(?i)abc", "inline flag directive"),
    ("abc(?i)def", "inline flag directive"),
    ("(?i:abc)", "case-insensitive flag on a `(?i:...)` group"),
    ("(?m:^a)", "multi-line flag on a `(?m:...)` group"),
    ("(?s:a)", "dot-matches-newline flag on a `(?s:...)` group"),
    ("(?-i:abc)", "case-insensitive flag on a `(?i:...)` group"),
    ("(?i-s:abc)", "case-insensitive flag on a `(?i:...)` group"),
    ("^.$", "the `.` any-character class"),
    (r"^\D$", r"the `\D` negated digit class"),
    (r"^\W$", r"the `\W` negated word class"),
    (r"^\S$", r"the `\S` negated whitespace class"),
    (r"[a\D]", r"the `\D` negated digit class"),
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

/// The haystacks the two engines are compared over, chosen so every way `\d`, `\w`, `\s` and `.`
/// were found to part ways is represented: an ASCII sample of each class, the ARABIC-INDIC digit
/// and the accented and Greek letters the `regex` crate counts as word characters and a flagless
/// literal does not, the two whitespace characters the engines disagree over in opposite
/// directions (NEL, which only the `regex` crate spaces, and the byte-order mark, which only
/// JavaScript does), and the astral characters a flagless literal sees as two code units.
const CROSS_ENGINE_HAYSTACKS: [&str; 20] = [
    "",
    "5",
    "a",
    "_",
    "-",
    " ",
    "\t",
    "\n",
    "\u{b}",
    "\r",
    "\u{661}",
    "\u{e9}",
    "\u{3b1}",
    "\u{85}",
    "\u{a0}",
    "\u{feff}",
    "\u{1f600}",
    "\u{1d7d9}",
    "12",
    "\u{661}\u{662}",
];

/// What a flagless JavaScript regex literal makes of each spelling the guard can hand one, over
/// [`CROSS_ENGINE_HAYSTACKS`] in order.
///
/// Read off `new RegExp(source).test(haystack)` under node v26.2.0 (V8 14.6). The JavaScript side
/// is recorded rather than executed because the crate's tests must not need a JavaScript runtime
/// to run; what is asserted against it — the `regex` crate's verdict on the very string the guard
/// emits — is executed. Both the authored spellings and the spellings they translate to are here,
/// so the table shows the divergence and its closure side by side.
const JAVASCRIPT_VERDICTS: [(&str, [bool; 20]); 22] = [
    (
        r"^\d$",
        [
            false, true, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false,
        ],
    ),
    (
        r"^\d+$",
        [
            false, true, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, true, false,
        ],
    ),
    (
        r"^\w$",
        [
            false, true, true, true, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false,
        ],
    ),
    (
        r"^\w+$",
        [
            false, true, true, true, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, true, false,
        ],
    ),
    (
        r"^\s$",
        [
            false, false, false, false, false, true, true, true, true, true, false, false, false,
            false, true, true, false, false, false, false,
        ],
    ),
    (
        r"^\s+$",
        [
            false, false, false, false, false, true, true, true, true, true, false, false, false,
            false, true, true, false, false, false, false,
        ],
    ),
    (
        r"^[a\d]$",
        [
            false, true, true, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false,
        ],
    ),
    (
        r"^[a\s]$",
        [
            false, false, true, false, false, true, true, true, true, true, false, false, false,
            false, true, true, false, false, false, false,
        ],
    ),
    (
        r"^[\w-]$",
        [
            false, true, true, true, true, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false,
        ],
    ),
    (
        r"^\d{3}\.\d{3}-\d{2}$",
        [
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false,
        ],
    ),
    (
        "^[0-9]$",
        [
            false, true, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false,
        ],
    ),
    (
        "^[0-9]+$",
        [
            false, true, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, true, false,
        ],
    ),
    (
        "^[0-9A-Za-z_]$",
        [
            false, true, true, true, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false,
        ],
    ),
    (
        "^[0-9A-Za-z_]+$",
        [
            false, true, true, true, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, true, false,
        ],
    ),
    (
        r"^[\t\n\v\f\r ]$",
        [
            false, false, false, false, false, true, true, true, true, true, false, false, false,
            false, false, false, false, false, false, false,
        ],
    ),
    (
        r"^[\t\n\v\f\r ]+$",
        [
            false, false, false, false, false, true, true, true, true, true, false, false, false,
            false, false, false, false, false, false, false,
        ],
    ),
    (
        "^[a0-9]$",
        [
            false, true, true, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false,
        ],
    ),
    (
        r"^[a\t\n\v\f\r ]$",
        [
            false, false, true, false, false, true, true, true, true, true, false, false, false,
            false, false, false, false, false, false, false,
        ],
    ),
    (
        "^[0-9A-Za-z_-]$",
        [
            false, true, true, true, true, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false,
        ],
    ),
    (
        r"^[0-9]{3}\.[0-9]{3}-[0-9]{2}$",
        [
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false,
        ],
    ),
    (
        "^[0-9a-fA-F]{24}$",
        [
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false,
        ],
    ),
    (
        "^[a-z]+$",
        [
            false, false, true, false, false, false, false, false, false, false, false, false,
            false, false, false, false, false, false, false, false,
        ],
    ),
];

/// Every construct whose value set the guard equalises, beside the spelling it equalises it to.
const EQUALISED_PATTERNS: [(&str, &str); 10] = [
    (r"^\d$", "^[0-9]$"),
    (r"^\d+$", "^[0-9]+$"),
    (r"^\w$", "^[0-9A-Za-z_]$"),
    (r"^\w+$", "^[0-9A-Za-z_]+$"),
    (r"^\s$", r"^[\t\n\v\f\r ]$"),
    (r"^\s+$", r"^[\t\n\v\f\r ]+$"),
    // Inside a class the members go in bare: a nested class is a construct the guard refuses.
    (r"^[a\d]$", "^[a0-9]$"),
    (r"^[a\s]$", r"^[a\t\n\v\f\r ]$"),
    (r"^[\w-]$", "^[0-9A-Za-z_-]$"),
    (r"^\d{3}\.\d{3}-\d{2}$", r"^[0-9]{3}\.[0-9]{3}-[0-9]{2}$"),
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

/// A word boundary is spelled and read alike by both grammars over every haystack the divergence
/// hunt covered, so it stays byte-identical where the classes beside it do not.
#[test]
fn test_portable_pattern_admits_the_boundary_both_grammars_agree_on() {
    for pattern in [r"\ba", r"\Ba"] {
        assert_eq!(portable(pattern).as_deref(), Ok(pattern), "for {pattern}");
    }
}

/// `\d`, `\w` and `\s` are in both grammars under one spelling and cover different characters by
/// it: the `regex` crate reads them as Unicode classes, and a flagless JavaScript literal reads
/// the narrower ASCII ones. Neither engine can be told to mean the other, so the guard writes the
/// set out in the members both engines do agree on and hands that one string to all three
/// surfaces.
#[test]
fn test_portable_pattern_equalises_the_classes_the_engines_cover_differently() {
    for (written, equalised) in EQUALISED_PATTERNS {
        assert_eq!(portable(written).as_deref(), Ok(equalised), "for {written}");
    }
}

/// The whole point, asserted end to end: the string the guard emits picks out the same haystacks
/// in the `regex` crate — which is what the generated Rust validator runs it through — as in a
/// flagless JavaScript regex literal, which is what the Zod schema and the JSON Schema `pattern`
/// keyword are. Run over the constructs that used to diverge and over the ones that never did.
#[test]
fn test_the_emitted_pattern_picks_out_the_same_haystacks_in_both_engines() {
    for (written, _) in EQUALISED_PATTERNS
        .into_iter()
        .chain([("^[0-9a-fA-F]{24}$", ""), ("^[a-z]+$", "")])
    {
        let emitted = portable(written).unwrap();
        let recorded = JAVASCRIPT_VERDICTS
            .into_iter()
            .find_map(|(source, verdicts)| (source == emitted).then_some(verdicts));
        assert!(
            recorded.is_some(),
            "{written} emits {emitted}, which no JavaScript verdict was recorded for"
        );
        let javascript = recorded.unwrap();
        let rust = regex::Regex::new(&emitted).unwrap();
        for (haystack, expected) in CROSS_ENGINE_HAYSTACKS.into_iter().zip(javascript) {
            assert_eq!(
                rust.is_match(haystack),
                expected,
                "{written} emits {emitted}, which parts ways with JavaScript over {haystack:?}"
            );
        }
    }
}

/// The constructs with no equalising spelling at all, and why: a flagless JavaScript literal
/// matches one UTF-16 code unit where the `regex` crate matches one character, so anything that
/// can match a character outside the Basic Multilingual Plane parts ways over every one of them.
/// `[^0-9]` is no better than `\D` here, which is why these are refused rather than rewritten.
#[test]
fn test_portable_pattern_refuses_what_no_spelling_equalises() {
    for pattern in ["^.$", r"^\D$", r"^\W$", r"^\S$", r"[a\D]", r"[\s\S]"] {
        let rejection = portable(pattern).unwrap_err();
        assert!(
            rejection.contains("cover different characters"),
            "{pattern} is not refused for a value-set divergence: {rejection}"
        );
    }
}

/// Held against the engines rather than restated: for the dot, no candidate spelling agrees with
/// JavaScript over the astral haystacks, which is what makes refusal the only honest verdict.
#[test]
fn test_no_spelling_of_the_dot_agrees_across_the_engines() {
    for candidate in ["^.$", r"^[^\n]$", r"^[\s\S]$", r"^[^\n\r\u{2028}\u{2029}]$"] {
        let rust = regex::Regex::new(candidate).unwrap();
        // A flagless literal tests one code unit at a time, so a lone astral character can never
        // fill a one-character pattern there.
        assert!(
            rust.is_match("\u{1f600}"),
            "{candidate} was expected to match an astral character in the `regex` crate"
        );
    }
}

/// The engine baseline is a recorded decision, not a reading of whichever runtime is installed:
/// this machine's node parses the ES2025 modifier groups, and the guard refuses them anyway
/// because the baseline the emitted schemas target is older.
#[test]
fn test_portable_pattern_refuses_a_modifier_group_the_baseline_predates() {
    for pattern in ["(?i:abc)", "(?m:^a)", "(?s:a)", "(?-i:abc)", "(?i-s:abc)"] {
        let rejection = portable(pattern).unwrap_err();
        for needle in [
            JS_ENGINE_BASELINE,
            "modifier",
            "regular expression modifiers",
        ] {
            assert!(
                rejection.contains(needle),
                "{needle} missing for {pattern}: {rejection}"
            );
        }
    }
}

/// What the baseline admits stays admitted, so the refusal above is a floor rather than a ban on
/// everything recent: named groups are ES2018 and are still translated and emitted.
#[test]
fn test_the_engine_baseline_still_admits_what_it_dates_from() {
    assert_eq!(JS_ENGINE_BASELINE, "ES2018");
    assert_eq!(portable("(?P<w>[a-z]+)").unwrap(), "(?<w>[a-z]+)");
    assert_eq!(portable("(?<w>[a-z]+)").unwrap(), "(?<w>[a-z]+)");
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

/// Renaming a group is a change of spelling and not of meaning: what the pattern matched before
/// it, it matches after — read off `regex::Regex` itself rather than restated here.
///
/// Equalising a perl class is the other kind of rewrite and deliberately does narrow what the
/// `regex` crate accepts, down to the set JavaScript was going to enforce anyway. The haystacks
/// here are ASCII, where the two engines already agree, so a `\d` sitting beside a renamed group
/// still has to come through matching exactly what it matched.
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
