use super::{RenameRule, resolve_rename_rule, unsupported_mode_message};

/// The expectation table from `serde_derive` 1.0.229 `src/internals/case.rs::rename_variants`,
/// extended with the multi-word, acronym-run and digit cases this crate has to get right.
#[test]
fn variant_rules_match_serde() {
    for &(original, lower, upper, camel, snake, screaming, kebab, screaming_kebab) in &[
        (
            "Outcome", "outcome", "OUTCOME", "outcome", "outcome", "OUTCOME", "outcome", "OUTCOME",
        ),
        (
            "VeryTasty",
            "verytasty",
            "VERYTASTY",
            "veryTasty",
            "very_tasty",
            "VERY_TASTY",
            "very-tasty",
            "VERY-TASTY",
        ),
        ("A", "a", "A", "a", "a", "A", "a", "A"),
        ("Z42", "z42", "Z42", "z42", "z42", "Z42", "z42", "Z42"),
        (
            "AgentKeyUnderUserSsh",
            "agentkeyunderuserssh",
            "AGENTKEYUNDERUSERSSH",
            "agentKeyUnderUserSsh",
            "agent_key_under_user_ssh",
            "AGENT_KEY_UNDER_USER_SSH",
            "agent-key-under-user-ssh",
            "AGENT-KEY-UNDER-USER-SSH",
        ),
        (
            "HttpSSHProxy",
            "httpsshproxy",
            "HTTPSSHPROXY",
            "httpSSHProxy",
            "http_s_s_h_proxy",
            "HTTP_S_S_H_PROXY",
            "http-s-s-h-proxy",
            "HTTP-S-S-H-PROXY",
        ),
        (
            "Base64Payload",
            "base64payload",
            "BASE64PAYLOAD",
            "base64Payload",
            "base64_payload",
            "BASE64_PAYLOAD",
            "base64-payload",
            "BASE64-PAYLOAD",
        ),
    ] {
        assert_eq!(RenameRule::None.apply_to_variant(original), original);
        assert_eq!(RenameRule::LowerCase.apply_to_variant(original), lower);
        assert_eq!(RenameRule::UpperCase.apply_to_variant(original), upper);
        assert_eq!(RenameRule::PascalCase.apply_to_variant(original), original);
        assert_eq!(RenameRule::CamelCase.apply_to_variant(original), camel);
        assert_eq!(RenameRule::SnakeCase.apply_to_variant(original), snake);
        assert_eq!(
            RenameRule::ScreamingSnakeCase.apply_to_variant(original),
            screaming
        );
        assert_eq!(RenameRule::KebabCase.apply_to_variant(original), kebab);
        assert_eq!(
            RenameRule::ScreamingKebabCase.apply_to_variant(original),
            screaming_kebab
        );
    }
}

/// The expectation table from `serde_derive` 1.0.229 `src/internals/case.rs::rename_fields`.
#[test]
fn field_rules_match_serde() {
    for &(original, upper, pascal, camel, screaming, kebab, screaming_kebab) in &[
        (
            "outcome", "OUTCOME", "Outcome", "outcome", "OUTCOME", "outcome", "OUTCOME",
        ),
        (
            "very_tasty",
            "VERY_TASTY",
            "VeryTasty",
            "veryTasty",
            "VERY_TASTY",
            "very-tasty",
            "VERY-TASTY",
        ),
        ("a", "A", "A", "a", "A", "a", "A"),
        ("z42", "Z42", "Z42", "z42", "Z42", "z42", "Z42"),
    ] {
        assert_eq!(RenameRule::None.apply_to_field(original), original);
        assert_eq!(RenameRule::LowerCase.apply_to_field(original), original);
        assert_eq!(RenameRule::SnakeCase.apply_to_field(original), original);
        assert_eq!(RenameRule::UpperCase.apply_to_field(original), upper);
        assert_eq!(RenameRule::PascalCase.apply_to_field(original), pascal);
        assert_eq!(RenameRule::CamelCase.apply_to_field(original), camel);
        assert_eq!(
            RenameRule::ScreamingSnakeCase.apply_to_field(original),
            screaming
        );
        assert_eq!(RenameRule::KebabCase.apply_to_field(original), kebab);
        assert_eq!(
            RenameRule::ScreamingKebabCase.apply_to_field(original),
            screaming_kebab
        );
    }
}

#[test]
fn every_serde_mode_parses() {
    for (mode, expected) in [
        ("lowercase", RenameRule::LowerCase),
        ("UPPERCASE", RenameRule::UpperCase),
        ("PascalCase", RenameRule::PascalCase),
        ("camelCase", RenameRule::CamelCase),
        ("snake_case", RenameRule::SnakeCase),
        ("SCREAMING_SNAKE_CASE", RenameRule::ScreamingSnakeCase),
        ("kebab-case", RenameRule::KebabCase),
        ("SCREAMING-KEBAB-CASE", RenameRule::ScreamingKebabCase),
    ] {
        assert_eq!(RenameRule::from_mode(mode), Some(expected));
        assert_eq!(resolve_rename_rule(Some(mode)), expected);
    }
}

#[test]
fn absent_rename_all_applies_no_rule() {
    assert_eq!(resolve_rename_rule(None), RenameRule::None);
}

#[test]
fn unsupported_mode_does_not_parse() {
    assert_eq!(RenameRule::from_mode("snakecase"), None);
    assert_eq!(RenameRule::from_mode("SnakeCase"), None);
    assert_eq!(RenameRule::from_mode(""), None);
}

#[test]
fn unsupported_mode_message_names_mode_and_alternatives() {
    let message = unsupported_mode_message("snakecase");
    assert!(message.contains("snakecase"), "Got: {message}");
    assert!(message.contains("snake_case"), "Got: {message}");
    assert!(message.contains("SCREAMING-KEBAB-CASE"), "Got: {message}");
}

#[test]
#[should_panic(expected = "unsupported serde `rename_all = \"snakecase\"`")]
fn unsupported_mode_is_rejected_loudly() {
    let _rule = resolve_rename_rule(Some("snakecase"));
}
