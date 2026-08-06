use super::*;

#[cfg(feature = "object_id")]
use crate::features::object_id::OBJECT_ID_HEX_PATTERN;

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
const UNPORTABLE_PATTERNS: [(&str, &str); 34] = [
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
    // Negated, so the class is refused for being negated before its members are read at all — the
    // `]` rule still answers for the two spellings above it.
    ("[^]]", "negated character class"),
    ("[]-a]", "unescaped `]` opening a character class"),
    ("[^a]", "negated character class"),
    (r"[^\w]", "negated character class"),
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

#[cfg(feature = "serde")]
/// Patterns a regex engine is avoidable work for, in every shape `clippy::trivial_regex` proves
/// one is — a bare literal, and a literal run under a leading `^`, a trailing `$`, or both.
const TRIVIAL_PATTERNS: [&str; 10] = [
    "^/",
    "^abc",
    r"^foo\.bar",
    "^\u{e9}",
    "abc$",
    "/$",
    "^abc$",
    "^/$",
    "^$",
    "abc",
];

#[cfg(feature = "serde")]
/// Patterns that keep their regex. The first eight are the shapes the shipped tests and the
/// reports write; the rest are the near misses — a literal with one non-literal part in it, and
/// the constructs whose trivial reading the lint offers no call for and this crate therefore
/// declines to make one up for.
const NON_TRIVIAL_PATTERNS: [&str; 16] = [
    "^[a-z]+$",
    "^/[a-z]+$",
    "^[0-9a-fA-F]{24}$",
    "^[a-z0-9_]+$",
    r"^\s*$",
    "(?<word>[a-z]+)",
    r"^[0-9]{3}\.[0-9]{3}-[0-9]{2}$",
    "^[a-z]+",
    "^a[0-9]",
    "[0-9]a$",
    "^a[0-9]b$",
    "",
    "^",
    "$",
    r"\b",
    "a|b",
];

/// Every shape the guard proves admits every value, written the ways an author reaches one: the
/// four the report names, then the same emptiness reached through a capture, a repetition that may
/// run zero times, an alternative that may be skipped, and a single text anchor with nothing but
/// those beside it.
const UNCONSTRAINING_PATTERNS: [&str; 12] = [
    "", "^", "$", "|", "()", "(^)", "a*", "a?", "a|", "^a*", "a*$", "(?:ab)*",
];

/// Patterns that turn some value away, kept beside [`UNCONSTRAINING_PATTERNS`] as the near misses
/// that decide where the line sits.
const CONSTRAINING_PATTERNS: [&str; 8] = [
    "^$", "^a*$", r"\b", r"\B", "a+", "(?:ab)+", "^[a-z]+$", r"^\s*$",
];

/// The negated classes the verdict was decided over: a single member, the ranges an author reaches
/// for to bound one to ASCII, an escape, and the `\d` the guard writes out to `0-9` on its way in.
/// The last is the one that shows rewriting cannot help — its members come out ASCII and the
/// complement is still taken two different ways.
const NEGATED_CLASS_PATTERNS: [&str; 6] = [
    "^[^a]$",
    "^[^0-9]$",
    r"^[^\n]$",
    "^[^a-z]$",
    r"^[^\x00-\x7F]$",
    r"^[^\d]$",
];

/// Every regex this crate writes into a generated schema itself, rather than carrying over from an
/// author's `pattern`.
///
/// An author's pattern reaches the three surfaces through the guard, which equalises what it can
/// and refuses the rest. One the crate writes reaches them directly, so without this list there
/// are two contracts and only one of them is enforced.
#[cfg(feature = "object_id")]
const CRATE_EMITTED_PATTERNS: [&str; 1] = [OBJECT_ID_HEX_PATTERN];

/// The strings a classification is proved against — every character the equalised classes are
/// compared over, plus the delimiters and near-miss prefixes the rewrite tests carry.
fn classification_haystacks() -> Vec<&'static str> {
    CROSS_ENGINE_HAYSTACKS
        .into_iter()
        .chain(REWRITE_HAYSTACKS)
        .chain(["ab", "abab", "xay", "word boundary", "  "])
        .collect()
}

#[cfg(feature = "serde")]
/// The haystacks a classified pattern is held to, in the two senses that matter: every character
/// the equalised classes are compared over, and the strings the rewrite tests use, which carry the
/// delimiters and the near-miss prefixes a `^/` sort of pattern turns on.
fn trivial_haystacks() -> Vec<String> {
    CROSS_ENGINE_HAYSTACKS
        .iter()
        .chain(REWRITE_HAYSTACKS.iter())
        .map(|haystack| (*haystack).to_owned())
        .chain(
            [
                "/",
                "/var",
                "var/",
                "/var/log",
                "abc",
                "xabc",
                "abcx",
                "xabcx",
                "foo.bar",
                "fooxbar",
                "\u{e9}t\u{e9}",
                "t\u{e9}",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .collect()
}

#[cfg(feature = "serde")]
/// What a classified pattern says about one haystack — the verdict the emitted call reaches,
/// reached here instead so it can be held against the regex the call replaced.
fn accepts(trivial: &TrivialPattern, haystack: &str) -> bool {
    match trivial {
        TrivialPattern::Contains(needle) => haystack.contains(needle),
        TrivialPattern::EndsWith(needle) => haystack.ends_with(needle),
        TrivialPattern::Equals(needle) => haystack == needle,
        TrivialPattern::IsEmpty => haystack.is_empty(),
        TrivialPattern::StartsWith(needle) => haystack.starts_with(needle),
    }
}

/// Parses `doc_lines` as consecutive `///` lines ahead of a minimal struct, so the resulting
/// attributes carry the real source spans a `#[model_schema]`-annotated item's would.
#[cfg(feature = "zod")]
fn struct_attrs_with_docs(doc_lines: &[&str]) -> Vec<syn::Attribute> {
    let doc_block = doc_lines.iter().fold(String::new(), |mut block, line| {
        use core::fmt::Write as _;
        writeln!(block, "/// {line}").unwrap();
        block
    });
    let source = format!("{doc_block}struct Probe;");
    let item: syn::ItemStruct = syn::parse_str(&source).unwrap();
    item.attrs
}

/// The source text each top-level token's span reports, in order.
#[cfg(feature = "zod")]
fn top_level_source_texts(tokens: &proc_macro2::TokenStream) -> Vec<Option<String>> {
    tokens
        .clone()
        .into_iter()
        .map(|tree| tree.span().source_text())
        .collect()
}

#[cfg(feature = "zod")]
#[test]
fn test_extract_example_simple() {
    let attrs = struct_attrs_with_docs(&[
        "User profile",
        "```rust example",
        "User { name: \"John\".to_string(), age: 25 }",
        "```",
    ]);
    let tokens = extract_example_tokens(&attrs);
    assert!(tokens.is_some());
    let expected: proc_macro2::TokenStream = "User { name: \"John\".to_string(), age: 25 }"
        .parse()
        .unwrap();
    assert_eq!(tokens.unwrap().to_string(), expected.to_string());
}

#[cfg(feature = "zod")]
#[test]
fn test_extract_example_multiline() {
    let attrs = struct_attrs_with_docs(&[
        "Complex example",
        "```rust example",
        "let x = 5;",
        "let y = 10;",
        "User { age: x + y }",
        "```",
    ]);
    let tokens = extract_example_tokens(&attrs);
    assert!(tokens.is_some());
    let expected: proc_macro2::TokenStream = "let x = 5; let y = 10; User { age: x + y }"
        .parse()
        .unwrap();
    assert_eq!(tokens.unwrap().to_string(), expected.to_string());
}

#[cfg(feature = "zod")]
#[test]
fn test_extract_example_first_only() {
    let attrs = struct_attrs_with_docs(&[
        "Multiple examples",
        "```rust example",
        "User { age: 25 }",
        "```",
        "Another description",
        "```rust example",
        "User { age: 30 }",
        "```",
    ]);
    let tokens = extract_example_tokens(&attrs);
    assert!(tokens.is_some());
    let expected: proc_macro2::TokenStream = "User { age: 25 }".parse().unwrap();
    assert_eq!(tokens.unwrap().to_string(), expected.to_string());
}

#[cfg(feature = "zod")]
#[test]
fn test_extract_example_none() {
    let attrs = struct_attrs_with_docs(&["User profile", "No examples here"]);
    assert!(extract_example_tokens(&attrs).is_none());
}

#[cfg(feature = "zod")]
#[test]
fn test_extract_example_empty_docs() {
    let attrs = struct_attrs_with_docs(&[]);
    assert!(extract_example_tokens(&attrs).is_none());
}

#[cfg(feature = "zod")]
#[test]
fn test_extract_example_regular_code_fence_ignored() {
    let attrs = struct_attrs_with_docs(&[
        "Example with regular fence",
        "```rust",
        "User { age: 25 }",
        "```",
    ]);
    assert!(extract_example_tokens(&attrs).is_none());
}

/// The bug this seam exists to fix: each line's tokens point at that line, not at the whole
/// example or the enclosing attribute.
#[cfg(feature = "zod")]
#[test]
fn extract_example_tokens_respans_each_line_onto_itself() {
    let attrs = struct_attrs_with_docs(&[
        "```rust example",
        "let ok = 1;",
        "Counted { count: ok }",
        "```",
    ]);
    let tokens = extract_example_tokens(&attrs).unwrap();
    let texts = top_level_source_texts(&tokens);

    // `source_text()` reports the whole physical line the doc literal spans, `///` included.
    assert_eq!(texts.first().unwrap().as_deref(), Some("/// let ok = 1;"));
    assert_eq!(
        texts.last().unwrap().as_deref(),
        Some("/// Counted { count: ok }")
    );
}

/// A value split across lines has no single line of its own to point at, so the whole run is
/// spanned on the line it starts on — an ambiguous run take the first line's span, per design.
#[cfg(feature = "zod")]
#[test]
fn extract_example_tokens_spans_a_multiline_value_on_its_first_line() {
    let attrs = struct_attrs_with_docs(&["```rust example", "Counted {", "count: 1,", "}", "```"]);
    let tokens = extract_example_tokens(&attrs).unwrap();
    let texts = top_level_source_texts(&tokens);

    assert!(
        texts
            .iter()
            .all(|text| text.as_deref() == Some("/// Counted {")),
        "{texts:?}"
    );
}

/// The `println!`/`let _` unwrapping `transform_example_code` performs still applies — it is
/// only the span that changed, not the emitted tokens.
#[cfg(feature = "zod")]
#[test]
fn extract_example_tokens_still_unwraps_the_doctest_println_pattern() {
    let attrs = struct_attrs_with_docs(&[
        "```rust example",
        "let data_type = 1;",
        concat!("println!(\"{", ":?}\", data_type);"),
        "```",
    ]);
    let tokens = extract_example_tokens(&attrs).unwrap();
    assert_eq!(tokens.to_string(), "let data_type = 1 ; data_type");
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
        ident_reexport_zod("LaterAlias", "LaterAliasType", "$Schema"),
        "\n\nexport const LaterAlias$Schema = LaterAliasType$Schema;"
    );
    for (ident, export) in [("PlainItem", "PlainItem"), ("PlainItemJson", "PlainItem")] {
        assert_eq!(
            ident_reexport_zod(ident, export, "$Schema"),
            "",
            "for: {ident}"
        );
    }
}

/// A generic type publishes a factory, so both names it is reached by have to name that factory:
/// a re-export written to `$Schema` would bind a name no emitted module declares.
#[cfg(feature = "zod")]
#[test]
fn a_zod_reexport_carries_the_suffix_the_item_published_under() {
    assert_eq!(
        ident_reexport_zod("Holder", "RenamedHolder", "$SchemaFactory"),
        "\n\nexport const Holder$SchemaFactory = RenamedHolder$SchemaFactory;"
    );
}

/// The argument a factory binds for a parameter reads as the parameter it fills while staying a
/// name of its own beside it.
#[cfg(feature = "zod")]
#[test]
fn a_factory_argument_is_the_lower_camel_of_its_parameter() {
    for (parameter, argument) in [
        ("IdType", "idType"),
        ("T", "t"),
        ("DateType", "dateType"),
        ("", ""),
    ] {
        assert_eq!(
            zod_factory_argument(parameter),
            argument,
            "for: {parameter}"
        );
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

/// Every regex this crate writes into a generated schema itself, rather than carrying over from an
/// author's `pattern`.
#[test]
#[cfg(feature = "object_id")]
fn test_every_pattern_the_crate_emits_is_a_fixed_point_of_the_guard() {
    for pattern in CRATE_EMITTED_PATTERNS {
        assert_eq!(
            portable(pattern).as_deref(),
            Ok(pattern),
            "the crate emits {pattern}, which the guard does not admit unchanged"
        );
    }
}

/// The `$oid` hex, run over the haystacks that tell the engines apart, on the same recorded-
/// JavaScript discipline as the table above: twenty-four ARABIC-INDIC digits are what a `\d` in
/// that class admits in the `regex` crate and a flagless literal refuses, so they are the case the
/// spelling turns on. The real `ObjectId` and the uppercase hex are there to say the value set the
/// constant is *for* did not move.
#[test]
#[cfg(feature = "object_id")]
fn test_the_emitted_object_id_hex_agrees_with_javascript_over_every_hex_shaped_haystack() {
    let emitted = regex::Regex::new(OBJECT_ID_HEX_PATTERN).unwrap();
    for (haystack, javascript) in [
        ("507f1f77bcf86cd799439011".to_owned(), true),
        ("\u{661}".repeat(24), false),
        ("507F1F77BCF86CD799439011".to_owned(), false),
    ] {
        assert_eq!(
            emitted.is_match(&haystack),
            javascript,
            "the emitted `$oid` pattern parts ways with JavaScript over {haystack:?}"
        );
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

/// A negated class is the last construct that was admitted while covering different characters in
/// the two engines, and it is refused the same way `\D`, `\W` and `\S` already are — which are the
/// same class negated, so admitting the bracketed spelling was refusing a construct under one name
/// and taking it under another.
#[test]
fn test_portable_pattern_refuses_a_negated_class_whatever_its_members() {
    for pattern in NEGATED_CLASS_PATTERNS {
        let rejection = portable(pattern).unwrap_err();
        for needle in ["negated character class", "cover different characters"] {
            assert!(
                rejection.contains(needle),
                "{needle} missing for {pattern}: {rejection}"
            );
        }
    }
}

/// Held against the engines rather than restated, as the dot's verdict was: no spelling of a
/// negated class agrees with JavaScript, so refusing every one of them is the verdict rather than
/// an admission table of the ones that survive.
#[test]
fn test_no_spelling_of_a_negated_class_agrees_across_the_engines() {
    for candidate in NEGATED_CLASS_PATTERNS.into_iter().chain(["^[^\u{1f601}]$"]) {
        let rust = regex::Regex::new(candidate).unwrap();
        assert!(
            rust.is_match("\u{1f600}"),
            "{candidate} was expected to match an astral character in the `regex` crate"
        );
    }
    let astral_bounded = regex::Regex::new("^[^\u{10000}-\u{10ffff}]$").unwrap();
    assert!(
        !astral_bounded.is_match("\u{1f600}"),
        "the astral-bounded class was expected to be the one whose `regex` reading excludes them"
    );
    assert!(
        astral_bounded.is_match("\u{661}"),
        "the astral-bounded class was expected to keep every character below the astral range"
    );
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

#[cfg(feature = "serde")]
/// Every shape the lint proves a regex is avoidable work for is classified as the call the lint
/// names for it, with the needle the pattern's own escapes resolve to.
#[test]
fn test_trivial_pattern_names_the_call_the_lint_names() {
    let expected = [
        ("^/", TrivialPattern::StartsWith("/".to_owned())),
        ("^abc", TrivialPattern::StartsWith("abc".to_owned())),
        (
            r"^foo\.bar",
            TrivialPattern::StartsWith("foo.bar".to_owned()),
        ),
        ("^\u{e9}", TrivialPattern::StartsWith("\u{e9}".to_owned())),
        ("abc$", TrivialPattern::EndsWith("abc".to_owned())),
        ("/$", TrivialPattern::EndsWith("/".to_owned())),
        ("^abc$", TrivialPattern::Equals("abc".to_owned())),
        ("^/$", TrivialPattern::Equals("/".to_owned())),
        ("^$", TrivialPattern::IsEmpty),
        ("abc", TrivialPattern::Contains("abc".to_owned())),
    ];
    assert_eq!(
        expected.len(),
        TRIVIAL_PATTERNS.len(),
        "every classified pattern needs the call it is classified as written down"
    );
    for (pattern, call) in expected {
        assert_eq!(
            trivial_pattern(pattern),
            Some(call),
            "{pattern} was not classified as the call the lint names for it"
        );
    }
}

#[cfg(feature = "serde")]
/// A pattern of any real shape keeps its regex, and so do the two the lint calls trivial without
/// naming a call: what a wrong reading would cost is a constraint that admits a different set of
/// values than it was written to, and there is nothing to gain by guessing at one.
#[test]
fn test_trivial_pattern_leaves_every_other_pattern_its_regex() {
    for pattern in NON_TRIVIAL_PATTERNS {
        assert!(
            trivial_pattern(pattern).is_none(),
            "{pattern} was classified as trivial and is not"
        );
    }
}

#[cfg(feature = "serde")]
/// The classified call and the regex it replaces accept the same haystacks and reject the same
/// haystacks — which is the whole of what a `pattern` constraint says.
#[test]
fn test_trivial_pattern_accepts_exactly_what_its_regex_accepts() {
    let haystacks = trivial_haystacks();
    for pattern in TRIVIAL_PATTERNS {
        let trivial = trivial_pattern(pattern).unwrap();
        let regex = regex::Regex::new(pattern).unwrap();
        for haystack in &haystacks {
            assert_eq!(
                accepts(&trivial, haystack),
                regex.is_match(haystack),
                "{pattern} and the call it was classified as part ways over {haystack:?}"
            );
        }
    }
}

/// Runs both `pattern` guards the way the attribute parsers run them, and hands back the refusal
/// as the author reads it.
fn constraining(pattern: &str) -> Result<String, String> {
    let lit = LitStr::new(pattern, proc_macro2::Span::call_site());
    portable_pattern(&lit)
        .and_then(|portable| constraining_pattern(&lit, portable))
        .map_err(|rejection| rejection.to_string())
}

/// The verdict is proved against the regex the pattern would have been checked by, not asserted
/// beside it: a pattern the guard calls unconstraining has to match every haystack in the corpus,
/// and one it lets through has to turn at least one of them away.
#[test]
fn test_the_unconstraining_verdict_is_the_regex_crate_s_own() {
    let haystacks = classification_haystacks();
    for pattern in UNCONSTRAINING_PATTERNS {
        let regex = regex::Regex::new(pattern).unwrap();
        for haystack in &haystacks {
            assert!(
                regex.is_match(haystack),
                "{pattern} is called unconstraining and turns {haystack:?} away"
            );
        }
    }
    for pattern in CONSTRAINING_PATTERNS {
        let regex = regex::Regex::new(pattern).unwrap();
        assert!(
            haystacks.iter().any(|haystack| !regex.is_match(haystack)),
            "{pattern} is left on the regex path and turns nothing in the corpus away"
        );
    }
}

/// A pattern that matches at some position of every string is refused where it is written, and the
/// refusal says what is wrong with it rather than naming a construct.
#[test]
fn test_a_pattern_admitting_every_value_is_refused() {
    for pattern in UNCONSTRAINING_PATTERNS {
        let rejection = constraining(pattern).unwrap_err();
        for needle in ["pattern", "admits every value", "constrains nothing"] {
            assert!(
                rejection.contains(needle),
                "{needle} missing for {pattern}: {rejection}"
            );
        }
    }
}

/// A pattern that turns some value away keeps its place, `\b` included.
#[test]
fn test_a_pattern_turning_some_value_away_clears_the_guard() {
    for pattern in CONSTRAINING_PATTERNS {
        assert!(
            constraining(pattern).is_ok(),
            "{pattern} constrains something and was refused: {:?}",
            constraining(pattern)
        );
    }
}

/// The patterns the shipped tests and the report write all say something, so none of them changes
/// verdict under the new guard — the classified trivial shapes above all.
#[cfg(feature = "serde")]
#[test]
fn test_the_shapes_already_classified_still_clear_the_guard() {
    for pattern in TRIVIAL_PATTERNS.into_iter().chain(PORTABLE_PATTERNS) {
        assert_eq!(
            constraining(pattern).as_deref(),
            Ok(pattern),
            "{pattern} was refused, or came back in a different spelling"
        );
    }
}
