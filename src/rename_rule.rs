//! serde's `rename_all` casing rules.
//!
//! Mirrors `RenameRule` from `serde_derive` 1.0.229 (`src/internals/case.rs`) so generated
//! TypeScript and Zod schemas carry exactly the strings serde puts on the wire. serde applies
//! different rules to struct fields (already `snake_case` in Rust source) than to enum variants
//! (`PascalCase` in Rust source), so the two stay separate here as well.

/// Every mode serde accepts, spelled as serde spells it. A mode serde accepts but this table
/// omits is rejected at macro-expansion time rather than emitted as the raw Rust identifier.
const RENAME_RULES: [(&str, RenameRule); 8] = [
    ("lowercase", RenameRule::LowerCase),
    ("UPPERCASE", RenameRule::UpperCase),
    ("PascalCase", RenameRule::PascalCase),
    ("camelCase", RenameRule::CamelCase),
    ("snake_case", RenameRule::SnakeCase),
    ("SCREAMING_SNAKE_CASE", RenameRule::ScreamingSnakeCase),
    ("kebab-case", RenameRule::KebabCase),
    ("SCREAMING-KEBAB-CASE", RenameRule::ScreamingKebabCase),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenameRule {
    CamelCase,
    KebabCase,
    LowerCase,
    None,
    PascalCase,
    ScreamingKebabCase,
    ScreamingSnakeCase,
    SnakeCase,
    UpperCase,
}

impl RenameRule {
    pub fn apply_to_field(self, field: &str) -> String {
        match self {
            Self::None | Self::LowerCase | Self::SnakeCase => field.to_owned(),
            Self::UpperCase | Self::ScreamingSnakeCase => field.to_ascii_uppercase(),
            Self::PascalCase => field_to_pascal(field),
            Self::CamelCase => lower_first(&field_to_pascal(field)),
            Self::KebabCase => field.replace('_', "-"),
            Self::ScreamingKebabCase => field.to_ascii_uppercase().replace('_', "-"),
        }
    }

    pub fn apply_to_variant(self, variant: &str) -> String {
        match self {
            Self::None | Self::PascalCase => variant.to_owned(),
            Self::LowerCase => variant.to_ascii_lowercase(),
            Self::UpperCase => variant.to_ascii_uppercase(),
            Self::CamelCase => lower_first(variant),
            Self::SnakeCase => variant_to_snake(variant),
            Self::ScreamingSnakeCase => variant_to_snake(variant).to_ascii_uppercase(),
            Self::KebabCase => variant_to_snake(variant).replace('_', "-"),
            Self::ScreamingKebabCase => variant_to_snake(variant)
                .to_ascii_uppercase()
                .replace('_', "-"),
        }
    }

    pub fn from_mode(mode: &str) -> Option<Self> {
        RENAME_RULES
            .iter()
            .find(|&&(name, _)| name == mode)
            .map(|&(_, rule)| rule)
    }
}

fn field_to_pascal(field: &str) -> String {
    let mut pascal = String::new();
    let mut capitalize = true;
    for ch in field.chars() {
        if ch == '_' {
            capitalize = true;
        } else if capitalize {
            pascal.push(ch.to_ascii_uppercase());
            capitalize = false;
        } else {
            pascal.push(ch);
        }
    }
    pascal
}

/// serde lowercases only the leading character, leaving the rest untouched.
fn lower_first(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map_or_else(String::new, |first| {
        format!("{}{}", first.to_ascii_lowercase(), chars.as_str())
    })
}

/// Resolves a `rename_all` mode string, rejecting any mode `RENAME_RULES` does not implement.
///
/// The failed assert surfaces as a compile error at macro-expansion time; falling through to the
/// raw Rust identifier instead would silently disagree with what serde writes on the wire.
pub fn resolve_rename_rule(rename_all: Option<&str>) -> RenameRule {
    let Some(mode) = rename_all else {
        return RenameRule::None;
    };
    let rule = RenameRule::from_mode(mode);
    assert!(rule.is_some(), "{}", unsupported_mode_message(mode));
    rule.unwrap()
}

pub fn unsupported_mode_message(mode: &str) -> String {
    let supported = RENAME_RULES.map(|(name, _)| name).join(", ");
    format!(
        "unsupported serde `rename_all = \"{mode}\"` in model_schema; supported modes: {supported}"
    )
}

/// serde inserts a separator before every uppercase character past the first, so an acronym run
/// yields one underscore per capital (`HttpSSHProxy` -> `http_s_s_h_proxy`) and digits, not being
/// uppercase, never introduce a word break (`Base64Payload` -> `base64_payload`).
fn variant_to_snake(variant: &str) -> String {
    let mut snake = String::new();
    for (index, ch) in variant.char_indices() {
        if index > 0 && ch.is_uppercase() {
            snake.push('_');
        }
        snake.push(ch.to_ascii_lowercase());
    }
    snake
}

#[cfg(test)]
mod tests;
