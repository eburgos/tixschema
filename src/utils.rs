use core::cell::RefCell;
#[cfg(feature = "typescript")]
use core::iter;
use regex_syntax::ast::parse::Parser as PatternParser;
use regex_syntax::ast::{
    Assertion, AssertionKind, Ast, ClassSet, ClassSetBinaryOpKind, ClassSetItem, Flag,
    FlagsItemKind, Group, GroupKind, HexLiteralKind, Literal, LiteralKind, SpecialLiteralKind,
};
use std::collections::HashMap;
use syn::{Attribute, Expr, Field, Lit, LitStr, Meta, Variant};

#[cfg(any(feature = "typescript", feature = "zod"))]
use syn::{ItemEnum, ItemStruct};

/// What a JavaScript regex literal makes of a flag the `(?...:...)` form cannot carry. ES2025's
/// regular expression modifiers spell `i`, `m` and `s`, and the parse fails on anything else.
const MODIFIER_GROUP_READ_AS: &str = "a group opening no JavaScript regex parses: its modifier \
                                      groups carry `i`, `m` and `s` and nothing else";

/// What a JavaScript regex literal makes of a `\p{...}` class, shared by the two places one can be
/// written.
const UNICODE_CLASS_READ_AS: &str = "an escaped `p` or `P` followed by a literal `{...}`, since a \
                                     Unicode class there needs the `u` flag and a spliced literal \
                                     carries no flags";

/// A Unicode class, in the three spellings the `regex` crate reads one by.
const UNICODE_CLASS_WRITTEN: &str = "a Unicode class -- `\\p{...}`, `\\pL` or `\\P{...}`";

/// What a registered Rust ident, *written as a type path*, resolves to — the two facts a map key
/// asks of it: whether it carries an inherent `enum_members()`, the enumeration the JSON-schema
/// map-key expansion calls, and whether serde writes it as a bare string, which is what a JSON
/// object key is. Only a plain unit enum gets that method, and a type path sees straight through an
/// alias, so an alias answers for whatever it targets rather than for itself.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AliasKind {
    /// A plain unit enum, or an alias chain ending in one.
    EnumMembers,
    /// Provably has neither: a struct, a brand over a non-string inner, a non-plain enum, or an
    /// alias whose target is a primitive, a collection, or one of those.
    NoEnumMembers,
    /// No `enum_members()`, but serde writes it as a bare string: a `#[serde(transparent)]` brand
    /// whose inner is itself string-shaped, or an alias chain ending in one. Such a type keys a map
    /// exactly as `String` does, under its own name.
    StringWire,
    /// Undecidable at this expansion — an alias naming a type that was not registered before it.
    Unknown,
}

#[derive(Clone)]
pub struct AliasInfo {
    pub export_name: String,
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    pub kind: AliasKind,
    #[cfg(feature = "jsonschema")]
    pub module_name: String,
}

/// The walk over a parsed `pattern` that collects the rewrites its JavaScript spelling needs and
/// the first construct that has no JavaScript spelling at all.
///
/// The AST walked is the one `regex::Regex::new` is itself built on, so the guard's reading of a
/// pattern is the crate's own reading of it. A second grammar written here would answer for the
/// crate's while drifting from it, and it would have to tell `[--/]`, three ordinary class members,
/// from `[+--]`, a set difference — which the bytes alone do not say.
#[derive(Default)]
struct JsSpelling {
    /// Byte offsets of the `P` in each `(?P<name>` the pattern opens.
    ///
    /// This is the only rewrite the guard makes, and it is safe because it is local: the bytes
    /// around a group's opening are fixed, so dropping the `P` cannot change how anything else in
    /// the pattern parses. Escaping a class-opening `]` looks like the same kind of fix and is
    /// not — `[]-a]` is three members and `[\]-a]` is a range — which is why that one is refused
    /// rather than rewritten.
    named_group_markers: Vec<usize>,
    refusal: Option<Unportable>,
}

/// A construct the `regex` crate reads that a JavaScript regex literal has no reading for.
struct Unportable {
    /// What the same bytes are inside a JavaScript regex literal instead.
    read_as: &'static str,
    /// The construct, as the rejection names it.
    written: &'static str,
}

impl JsSpelling {
    fn assertion(&mut self, assertion: &Assertion) {
        let (written, read_as) = match assertion.kind {
            AssertionKind::StartLine
            | AssertionKind::EndLine
            | AssertionKind::WordBoundary
            | AssertionKind::NotWordBoundary => return,
            AssertionKind::StartText => (
                "the `\\A` anchor",
                "an escaped `A`, matching that letter; a flagless literal spells the same anchor \
                 `^`",
            ),
            AssertionKind::EndText => (
                "the `\\z` anchor",
                "an escaped `z`, matching that letter; a flagless literal spells the same anchor \
                 `$`",
            ),
            AssertionKind::WordBoundaryStart | AssertionKind::WordBoundaryEnd => (
                "a `\\b{start}` or `\\b{end}` word boundary",
                "a plain word boundary followed by a literal `{start}` or `{end}`",
            ),
            AssertionKind::WordBoundaryStartHalf | AssertionKind::WordBoundaryEndHalf => (
                "a `\\b{start-half}` or `\\b{end-half}` word boundary",
                "a plain word boundary followed by a literal `{start-half}` or `{end-half}`",
            ),
            AssertionKind::WordBoundaryStartAngle | AssertionKind::WordBoundaryEndAngle => (
                "a `\\<` or `\\>` word boundary",
                "an escaped `<` or `>`, matching that character",
            ),
        };
        self.refuse(written, read_as);
    }

    fn ast(&mut self, ast: &Ast) {
        match ast {
            // `.`, `\d` and `\w` are in both grammars and differ only in whether they are
            // Unicode-aware, which divides what they match rather than what they are.
            Ast::Empty(_) | Ast::Dot(_) | Ast::ClassPerl(_) => {}
            Ast::Flags(_) => self.refuse(
                "an inline flag directive `(?...)`",
                "a group opening no JavaScript regex parses",
            ),
            Ast::Literal(literal) => self.literal(literal, false),
            Ast::Assertion(assertion) => self.assertion(assertion),
            Ast::ClassUnicode(_) => self.refuse(UNICODE_CLASS_WRITTEN, UNICODE_CLASS_READ_AS),
            Ast::ClassBracketed(class) => self.class_set(&class.kind),
            Ast::Repetition(repetition) => self.ast(&repetition.ast),
            Ast::Group(group) => self.group(group),
            Ast::Alternation(alternation) => self.asts(&alternation.asts),
            Ast::Concat(concat) => self.asts(&concat.asts),
        }
    }

    fn asts(&mut self, asts: &[Ast]) {
        for ast in asts {
            self.ast(ast);
        }
    }

    fn class_item(&mut self, item: &ClassSetItem) {
        match item {
            ClassSetItem::Empty(_) | ClassSetItem::Perl(_) => {}
            ClassSetItem::Literal(literal) => self.literal(literal, true),
            ClassSetItem::Range(range) => {
                self.literal(&range.start, true);
                self.literal(&range.end, true);
            }
            ClassSetItem::Ascii(_) => self.refuse(
                "a POSIX class `[:name:]`",
                "the characters `[`, `:` and the name, listed as members of the class",
            ),
            ClassSetItem::Unicode(_) => self.refuse(UNICODE_CLASS_WRITTEN, UNICODE_CLASS_READ_AS),
            ClassSetItem::Bracketed(class) => {
                self.refuse(
                    "a class nested inside another class",
                    "a literal `[` listed as a member of the outer class",
                );
                self.class_set(&class.kind);
            }
            ClassSetItem::Union(union) => {
                for member in &union.items {
                    self.class_item(member);
                }
            }
        }
    }

    fn class_set(&mut self, set: &ClassSet) {
        match set {
            ClassSet::Item(item) => self.class_item(item),
            ClassSet::BinaryOp(op) => {
                let written = match op.kind {
                    ClassSetBinaryOpKind::Intersection => "the `&&` class intersection",
                    ClassSetBinaryOpKind::Difference => "the `--` class difference",
                    ClassSetBinaryOpKind::SymmetricDifference => {
                        "the `~~` class symmetric difference"
                    }
                };
                self.refuse(
                    written,
                    "the operator's own characters, listed as members of the class",
                );
                self.class_set(&op.lhs);
                self.class_set(&op.rhs);
            }
        }
    }

    fn group(&mut self, group: &Group) {
        match &group.kind {
            GroupKind::CaptureIndex(_) => {}
            GroupKind::CaptureName { starts_with_p, .. } => {
                if *starts_with_p {
                    // `(?P<name>` and `(?<name>` are one construct under two spellings, and the
                    // `P` that tells them apart sits two bytes into the group's span.
                    self.named_group_markers.push(group.span.start.offset + 2);
                }
            }
            GroupKind::NonCapturing(flags) => {
                for item in &flags.items {
                    let FlagsItemKind::Flag(flag) = &item.kind else {
                        continue;
                    };
                    let written = match flag {
                        Flag::CaseInsensitive | Flag::MultiLine | Flag::DotMatchesNewLine => {
                            continue;
                        }
                        Flag::SwapGreed => "the swap-greed flag on a `(?U:...)` group",
                        Flag::Unicode => "the Unicode flag on a `(?u:...)` group",
                        Flag::CRLF => "the CRLF flag on a `(?R:...)` group",
                        Flag::IgnoreWhitespace => {
                            "the ignore-whitespace flag on a `(?x:...)` group"
                        }
                    };
                    self.refuse(written, MODIFIER_GROUP_READ_AS);
                }
            }
        }
        self.ast(&group.ast);
    }

    fn literal(&mut self, literal: &Literal, in_class: bool) {
        if in_class && literal.c == ']' && matches!(literal.kind, LiteralKind::Verbatim) {
            self.refuse(
                "an unescaped `]` opening a character class",
                "the empty class `[]`, which matches nothing, followed by the rest of the class as \
                 ordinary text; the member both grammars read is `\\]`",
            );
        }
        let (written, read_as) = match literal.kind {
            LiteralKind::Verbatim
            | LiteralKind::Meta
            | LiteralKind::Superfluous
            | LiteralKind::HexFixed(HexLiteralKind::X | HexLiteralKind::UnicodeShort)
            | LiteralKind::Special(
                SpecialLiteralKind::FormFeed
                | SpecialLiteralKind::Tab
                | SpecialLiteralKind::LineFeed
                | SpecialLiteralKind::CarriageReturn
                | SpecialLiteralKind::VerticalTab
                | SpecialLiteralKind::Space,
            ) => return,
            LiteralKind::Octal => (
                "an octal escape",
                "a legacy escape a JavaScript regex reads by its own rules and refuses outright \
                 under a Unicode flag",
            ),
            LiteralKind::HexBrace(_) => (
                "a braced code point escape -- `\\x{...}`, `\\u{...}` or `\\U{...}`",
                "an escaped `x`, `u` or `U` followed by a literal `{...}`, since the one braced \
                 form JavaScript has needs the `u` flag and a spliced literal carries no flags",
            ),
            LiteralKind::HexFixed(HexLiteralKind::UnicodeLong) => (
                "the eight-digit `\\U...` code point escape",
                "an escaped `U` followed by the digits themselves",
            ),
            LiteralKind::Special(SpecialLiteralKind::Bell) => (
                "the `\\a` bell escape",
                "an escaped `a`, matching that letter",
            ),
        };
        self.refuse(written, read_as);
    }

    /// Records `written` as the pattern's refusal, keeping the first construct the walk reached.
    fn refuse(&mut self, written: &'static str, read_as: &'static str) {
        self.refusal.get_or_insert(Unportable { read_as, written });
    }

    /// `pattern` with every `(?P<name>` it opens rewritten to `(?<name>`.
    fn rewritten(mut self, pattern: &str) -> String {
        if self.named_group_markers.is_empty() {
            return pattern.to_owned();
        }
        self.named_group_markers.sort_unstable();
        let mut result = String::with_capacity(pattern.len());
        let mut cut = 0;
        for marker in self.named_group_markers {
            result.push_str(&pattern[cut..marker]);
            cut = marker + 1;
        }
        result.push_str(&pattern[cut..]);
        result
    }
}

thread_local! {
    static ALIAS_INFO: RefCell<HashMap<String, AliasInfo>> = RefCell::new(HashMap::new());
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
pub fn register_alias_info(
    rust_ident: &str,
    export_name: &str,
    module_name: &str,
    kind: AliasKind,
) {
    #[cfg(not(feature = "jsonschema"))]
    let _: &_ = &module_name;
    ALIAS_INFO.with(|map| {
        map.borrow_mut().insert(
            rust_ident.to_owned(),
            AliasInfo {
                export_name: export_name.to_owned(),
                kind,
                #[cfg(feature = "jsonschema")]
                module_name: module_name.to_owned(),
            },
        );
    });
}

pub fn lookup_alias_info(rust_ident: &str) -> Option<AliasInfo> {
    ALIAS_INFO.with(|map| map.borrow().get(rust_ident).cloned())
}

pub fn safe_type_name(key: &str) -> String {
    if key.ends_with("Json") {
        key.strip_suffix("Json").map(str::to_owned).unwrap()
    } else {
        key.to_owned()
    }
}

/// The schema module a `#[model_schema()]` item publishes — an alias, a struct, an enum, a branded
/// newtype alike — which is also the module a reference assumes for a name the registry does not
/// hold.
///
/// A reference written *before* the item expands has nothing but the Rust ident to name a module
/// from — the registry is empty of the item, and the exported name is not recoverable from the
/// ident once a `name = "…"` override is in play. So the module is named from the ident on both
/// sides, and the two spellings agree in either declaration order. A rename moves what the item is
/// exported as; it does not move where the item's schema lives.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
pub fn ident_schema_module_name(rust_ident: &str) -> String {
    format!("{}_schema", to_snake_case(&safe_type_name(rust_ident)))
}

/// The export name is what `register_alias_info` stores and what the alias's TypeScript, zod, and
/// JSON-schema surfaces are written under, so every feature that references an alias needs it —
/// not just `typescript`.
///
/// An override is taken verbatim: the parser has already refused a value no surface can carry, so
/// what arrives here is the name the author wrote.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
pub fn compute_alias_export_name(rust_ident: &str, override_name: Option<&str>) -> String {
    override_name.map_or_else(
        || format!("{}Type", safe_type_name(rust_ident)),
        ToOwned::to_owned,
    )
}

/// [`compute_alias_export_name`] for a declared item — a struct, an enum, a tuple struct, a branded
/// newtype. Without an override the item keeps the name it is declared under, which is the one
/// difference from an alias: an alias has no surface name of its own and is given the `Type` suffix.
///
/// This is the single seam every item path takes its exported name from, and the registry is keyed
/// by the Rust ident and answers with it, so a sibling naming the type in Rust resolves to whatever
/// it is exported as.
pub fn compute_item_export_name(rust_ident: &str, override_name: Option<&str>) -> String {
    override_name.map_or_else(|| safe_type_name(rust_ident), ToOwned::to_owned)
}

/// Builds the `JSDoc` comment body an alias's `export type` is emitted under, which is what
/// `build_item_jsdoc` builds for a declared item. Between them they are every `JSDoc` body an
/// exported type carries, so the rule below holds wherever one is written rather than at the call
/// sites that reach for one.
///
/// An alias's ` ```rust example ` block is dropped before its lines reach the body, the way every
/// item shape drops it: the block is Rust source, and nothing reads it as such once it is sitting in
/// a `TypeScript` comment. An alias documented with nothing but an example is left naming itself,
/// as an undocumented one is.
#[cfg(feature = "typescript")]
pub fn format_docs_for_ts(docs: &[String], fallback_name: &str) -> String {
    let described = strip_examples_from_docs(docs);
    if described.is_empty() {
        format!(" * {fallback_name}\n * ")
    } else {
        described
            .iter()
            .map(|line| format!(" * {line}"))
            .chain(iter::once(" * ".to_owned()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(any(feature = "typescript", feature = "zod"))]
/// Extracts and concatenates documentation comments from a `syn::ItemStruct`.
///
/// # Arguments
///
/// * `item_struct` - A reference to the `syn::ItemStruct` to process.
///
/// # Returns
///
/// An `Option<String>` containing the concatenated documentation,
/// or `None` if no doc comments are found. Returns an empty string
/// if doc comments exist but are empty.
pub fn get_struct_docs(item_struct: &ItemStruct) -> Option<Vec<String>> {
    collect_doc_lines(&item_struct.attrs)
}

#[cfg(any(feature = "typescript", feature = "zod"))]
pub fn get_enum_docs(item_enum: &ItemEnum) -> Option<Vec<String>> {
    collect_doc_lines(&item_enum.attrs)
}

pub fn get_variant_docs(variant: &Variant) -> Option<Vec<String>> {
    collect_doc_lines(&variant.attrs)
}

pub fn get_field_docs(field: &Field) -> Option<Vec<String>> {
    collect_doc_lines(&field.attrs)
}

#[cfg(feature = "typescript")]
pub fn get_item_docs(attrs: &[Attribute]) -> Option<Vec<String>> {
    collect_doc_lines(attrs)
}

fn collect_doc_lines(attrs: &[Attribute]) -> Option<Vec<String>> {
    let mut doc_lines = Vec::new();

    for attr in attrs {
        if attr.path().is_ident("doc")
            && let Meta::NameValue(meta_name_value) = &attr.meta
            && let Expr::Lit(syn::ExprLit {
                lit: Lit::Str(lit_str),
                ..
            }) = &meta_name_value.value
        {
            let value = lit_str.value();
            // Split on newlines to handle block comments (/** */)
            // which may come as a single string with embedded \n
            for line in value.lines() {
                doc_lines.push(line.trim().to_owned());
            }
        }
    }

    if doc_lines.is_empty() {
        None
    } else {
        Some(doc_lines)
    }
}

#[cfg(any(
    feature = "typescript",
    feature = "zod",
    feature = "jsonschema",
    feature = "serde"
))]
pub fn to_snake_case(name: &str) -> String {
    let mut result = String::new();
    let mut prev_lower = false;
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            if prev_lower {
                result.push('_');
            }
            result.push(ch.to_ascii_lowercase());
            prev_lower = true;
        } else {
            result.push(ch);
            prev_lower = ch.is_ascii_lowercase();
        }
    }
    result
}

/// Extracts the first Rust code example from documentation comments.
///
/// Looks for a code fence with the format ` ```rust example` and extracts
/// all code until the closing ` ``` `. If multiple examples are found,
/// only the first one is returned.
///
/// # Arguments
///
/// * `docs` - A slice of documentation comment lines.
///
/// # Returns
///
/// An `Option<String>` containing the example code if found, or `None` if
/// no example fence is present.
#[cfg(feature = "zod")]
pub fn extract_example_from_docs(docs: &[String]) -> Option<String> {
    let mut in_example_block = false;
    let mut example_lines = Vec::new();

    for line in docs {
        let trimmed = line.trim();
        // Strip leading asterisk from block-style comments
        let cleaned = trimmed.strip_prefix('*').unwrap_or(trimmed).trim();

        // Check for opening fence
        if cleaned == "```rust example" {
            if !example_lines.is_empty() {
                // Already found one example, return it
                break;
            }
            in_example_block = true;
            continue;
        }

        // Check for closing fence
        if in_example_block && cleaned == "```" {
            // Found complete example
            break;
        }

        // Collect example lines
        if in_example_block {
            example_lines.push(line.clone());
        }
    }

    if example_lines.is_empty() {
        None
    } else {
        // Apply regex transformations to make doctest-compatible code work for schema_example
        let code = example_lines.join("\n");
        Some(transform_example_code(&code))
    }
}

/// Transforms doctest-compatible example code to be suitable for `schema_example()`.
///
/// Applies regex transformations to convert code that returns () (for doctest)
/// into code that returns the actual value (for schema serialization).
///
/// Current transformations:
/// - Strips `use` statements (type is already in scope in impl block)
/// - `println!("...", value);` → `value`
/// - `let _: Type = value;` → `value`
#[cfg(feature = "zod")]
fn transform_example_code(code: &str) -> String {
    let mut result = code.to_owned();

    // Pattern 0: Strip use statements
    // Remove lines starting with "use " (they're not needed in the impl block context)
    let re_use = regex::Regex::new(r"(?m)^\s*use\s+[^;]+;\s*\n?").unwrap();
    result = re_use.replace_all(&result, "").to_string();

    // Pattern 1: println!("...", variable); → variable
    // Matches: println!("anything", value); or println!("format {}", value);
    let re = regex::Regex::new(r"println!\s*\([^,)]+,\s*([^)]+)\)\s*;?\s*$").unwrap();
    if let Some(captures) = re.captures(&result)
        && let Some(variable) = captures.get(1)
    {
        result = re.replace(&result, variable.as_str()).to_string();
    }

    // Pattern 2: let _: Type = value; → value
    // Matches: let _: SomeType = value; or let _ = value;
    let re2 = regex::Regex::new(r"let\s+_(?:\s*:\s*[^=]+)?\s*=\s*([^;]+)\s*;?\s*$").unwrap();
    if let Some(captures) = re2.captures(&result)
        && let Some(variable) = captures.get(1)
    {
        result = re2.replace(&result, variable.as_str()).to_string();
    }

    result.trim().to_owned()
}

/// Strips example code blocks from documentation lines.
///
/// This is used for descriptions to avoid including example code in the description field.
#[cfg(any(feature = "typescript", feature = "zod"))]
pub fn strip_examples_from_docs(docs: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut in_example_block = false;

    for line in docs {
        let trimmed = line.trim();
        // Strip leading asterisk from block-style comments
        let cleaned = trimmed.strip_prefix('*').unwrap_or(trimmed).trim();

        // Check for opening fence
        if cleaned == "```rust example" {
            in_example_block = true;
            continue;
        }

        // Check for closing fence
        if in_example_block && cleaned == "```" {
            in_example_block = false;
            continue;
        }

        // Skip lines inside example blocks
        if in_example_block {
            continue;
        }

        result.push(line.clone());
    }

    result
}

/// The escape body a JavaScript regex literal spells a line terminator with, i.e. what follows
/// the backslash. The literal grammar excludes a raw line terminator outright, both on its own
/// and as the character a backslash escapes, so these are the only spellings available.
#[cfg(feature = "zod")]
const fn js_line_terminator_escape(ch: char) -> Option<&'static str> {
    match ch {
        '\n' => Some("n"),
        '\r' => Some("r"),
        '\u{2028}' => Some("u2028"),
        '\u{2029}' => Some("u2029"),
        _ => None,
    }
}

/// A `pattern` attribute value in the spelling every surface it is spliced into reads the same
/// way, or the rejection that keeps it off them, spanned on the literal the author wrote.
///
/// One string reaches three surfaces: the Rust validator's `regex::Regex::new(...).unwrap()`, the
/// Zod schema's JavaScript regex literal, and the JSON Schema `pattern` keyword, which ECMA-262
/// defines as a JavaScript regex. A pattern the `regex` crate cannot parse is a panic at the first
/// validated value. A pattern only the `regex` crate can parse is quieter and worse: the derive
/// expands clean and the generated JavaScript either throws where it loads or, where the two
/// grammars disagree rather than collide, matches a different set of strings than the Rust
/// validator it was written beside. Both verdicts are reached here, at expansion.
///
/// Where the grammars merely spell one construct differently the spelling is rewritten instead of
/// refused: `(?P<name>...)` becomes `(?<name>...)`. That rewrite is the only one, and it is a
/// spelling the `regex` crate reads too, so the one string that goes to all three surfaces still
/// means to the validator exactly what it meant before. A `]` opening a character class looks like
/// the same kind of fix and is not — `[]-a]` is three members and `[\]-a]` is a range — so it is
/// refused rather than escaped.
pub fn portable_pattern(lit: &LitStr) -> Result<String, syn::Error> {
    let pattern = lit.value();
    if let Err(err) = regex::Regex::new(&pattern) {
        return Err(syn::Error::new_spanned(
            lit,
            format!(
                "`pattern` is not a regex the `regex` crate can parse. The generated validator \
                 builds it with `regex::Regex::new(...).unwrap()`, so accepting it here would turn \
                 the first validated value into a panic. {err}"
            ),
        ));
    }
    js_spelling(&pattern).map_err(|message| syn::Error::new_spanned(lit, message))
}

/// The pattern rewritten to the spelling a JavaScript regex literal reads the same way, or the
/// first construct in it that a JavaScript regex literal has no reading for.
fn js_spelling(pattern: &str) -> Result<String, String> {
    let ast = PatternParser::new().parse(pattern).map_err(|err| {
        format!(
            "`pattern` parses for `regex::Regex::new` but not for the grammar this guard reads it \
             back with, so what the Zod and JSON Schema surfaces would be handed cannot be \
             decided. {err}"
        )
    })?;
    let mut walk = JsSpelling::default();
    walk.ast(&ast);
    if let Some(refusal) = walk.refusal.as_ref() {
        return Err(format!(
            "`pattern` uses {}, which the `regex` crate reads and a JavaScript regex literal does \
             not: there the same bytes are {}. The Zod schema splices this string between `/` \
             delimiters and the JSON Schema `pattern` keyword is an ECMA-262 regex, so the \
             constraint would say one thing in the Rust validator and another -- or nothing at all \
             -- on the surfaces generated beside it.",
            refusal.written, refusal.read_as
        ));
    }
    Ok(walk.rewritten(pattern))
}

/// Escapes a regex pattern for splicing between the `/` delimiters of a JavaScript regex literal.
///
/// The pattern is already a regex, so what needs work is what the literal syntax alone gives a
/// meaning to: the `/` delimiter, which becomes `\/`, and a raw line terminator, which the literal
/// cannot carry at all and so becomes its escape. A backslash escape is consumed whole, which keeps
/// an authored `\/` from gaining a second backslash and keeps a literal `\\` from being read as the
/// escape for the `/` that follows it. A backslash before a raw line terminator is an identity
/// escape, and the escape form denotes that same character.
#[cfg(feature = "zod")]
pub fn escape_js_regex_literal(pattern: &str) -> String {
    let mut result = String::with_capacity(pattern.len());
    let mut chars = pattern.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                result.push('\\');
                if let Some(escaped) = chars.next() {
                    match js_line_terminator_escape(escaped) {
                        Some(escape) => result.push_str(escape),
                        None => result.push(escaped),
                    }
                }
            }
            '/' => result.push_str("\\/"),
            _ => match js_line_terminator_escape(ch) {
                Some(escape) => {
                    result.push('\\');
                    result.push_str(escape);
                }
                None => result.push(ch),
            },
        }
    }
    result
}

#[cfg(test)]
#[cfg(any(feature = "typescript", feature = "zod"))]
mod tests;
