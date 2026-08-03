use super::*;

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
