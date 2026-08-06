use core::cell::RefCell;
#[cfg(feature = "zod")]
use core::mem;
use core::ops::Range;
#[cfg(feature = "serde")]
use core::slice::from_ref;
use regex_syntax::ast::parse::Parser as PatternParser;
use regex_syntax::ast::{
    Assertion, AssertionKind, Ast, ClassBracketed, ClassPerl, ClassPerlKind, ClassSet,
    ClassSetBinaryOpKind, ClassSetItem, Flag, FlagsItemKind, Group, GroupKind, HexLiteralKind,
    Literal, LiteralKind, SpecialLiteralKind,
};
use regex_syntax::hir;
use std::collections::HashMap;
#[cfg(feature = "zod")]
use std::collections::HashSet;
use syn::{Attribute, Expr, Field, GenericParam, Generics, Lit, LitStr, Meta, Type, Variant};

#[cfg(any(
    feature = "typescript",
    feature = "zod",
    all(feature = "serde", feature = "jsonschema")
))]
use syn::ItemEnum;
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use syn::ItemStruct;

/// The JavaScript engine generation the emitted Zod regex literals and JSON Schema `pattern`
/// keywords are written for, and therefore the line the guard admits and refuses along.
const JS_ENGINE_BASELINE: &str = "ES2018";

/// What a JavaScript regex literal makes of the three flags the ECMA-262 regular expression
/// modifiers proposal did add.
const MODIFIER_GROUP_ABOVE_BASELINE_READ_AS: &str = "a group opening the ECMA-262 regular \
                                                     expression modifiers proposal added, which \
                                                     an engine predating it rejects as it parses \
                                                     the literal";

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

/// Why a construct both grammars parse still cannot go to the JavaScript surfaces: a flagless
/// literal tests one UTF-16 code unit where the `regex` crate tests one character, so a lone
/// character outside the Basic Multilingual Plane fills a one-character pattern there and never
/// here. Writing the class out settles which characters are named; it cannot settle how many code
/// units one of them is, and a spliced literal carries no `u` flag to settle it with.
const ASTRAL_DIVERGENCE: &str = "a character outside the Basic Multilingual Plane, which the \
                                 `regex` crate counts as one character and a flagless literal as \
                                 the two code units it is written from -- so the set is the same \
                                 and the count is not, and no spelling of the class closes that";

/// Why a construct cannot reach the JavaScript surfaces as the author wrote it. The three are
/// different failures and the rejection says which one it is.
#[derive(Clone, Copy)]
enum Divergence {
    /// A JavaScript regex literal reads the bytes only on an engine newer than the baseline the
    /// emitted schemas target.
    AboveBaseline,
    /// A JavaScript regex literal has no reading for the bytes at all.
    Unreadable,
    /// Both grammars read the bytes and pick out different characters by them.
    ValueSet,
}

/// What a registered Rust ident, *written as a type path*, resolves to — the one question a map key
/// asks of a name: what does serde write for a key spelled this way. A plain unit enum answers with
/// its members, the enumeration the JSON-schema map-key expansion calls `enum_members()` for; every
/// other answer is about the key's own wire form, a JSON object key being a string.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AliasKind {
    /// A plain unit enum, or an alias chain ending in one.
    EnumMembers,
    /// serde writes it as neither a string nor anything it will stringify, so it keys no map at
    /// all: a struct, a brand over one or over a container, a non-plain enum, or an alias whose
    /// target is any of those.
    NoEnumMembers,
    /// No `enum_members()`, but serde writes it as a bare string: `String` and `PathBuf`, a
    /// `#[serde(transparent)]` brand over one of those or over a plain enum, whose variant name is
    /// itself a bare string, and an alias chain ending in any of them. Such a type keys a map
    /// exactly as `String` does, under its own name.
    StringWire,
    /// No `enum_members()` and no bare string either, but serde stringifies it into a key all the
    /// same — a number, a `bool`, a chrono rendering, or a brand over one of those. The map is an
    /// object with nothing said about its members, which is what the bare inner already describes
    /// as; the brand's own name still stands on the nominal surfaces.
    Stringified,
    /// Undecidable at this expansion — an alias naming a type that was not registered before it.
    Unknown,
}

/// The `str` method call a `pattern` says the same thing as, for the patterns a regex engine is
/// avoidable work for.
///
/// Each variant carries the needle in the spelling the check takes it, which is the literal the
/// pattern's own escapes already resolved to and not the pattern text: `^foo\.bar` starts with
/// `foo.bar`, five bytes shorter than what was written.
#[cfg(feature = "serde")]
#[derive(Debug, PartialEq, Eq)]
pub enum TrivialPattern {
    /// `abc` -- the value has this string somewhere in it.
    Contains(String),
    /// `abc$` -- the value ends with this string.
    EndsWith(String),
    /// `^abc$` -- the value is this string.
    Equals(String),
    /// `^$` -- the value is the empty string.
    IsEmpty,
    /// `^abc` -- the value begins with this string.
    StartsWith(String),
}

/// One member of an untagged union as the Zod surface writes it, beside the two things a merge
/// that flattens the union has to know about it and cannot recover from the spelling.
#[cfg(feature = "zod")]
#[derive(Clone)]
pub struct ZodUnionMember {
    #[cfg(feature = "serde")]
    pub branch: Vec<usize>,
    #[cfg(feature = "serde")]
    pub non_object: Option<&'static str>,
    pub spelling: String,
}

#[cfg(all(feature = "serde", feature = "zod"))]
impl ZodUnionMember {
    /// The member's position, spelled the way the JSON-schema merge spells the same one.
    pub fn branch_path(&self) -> String {
        self.branch
            .iter()
            .map(usize::to_string)
            .collect::<Vec<String>>()
            .join(".")
    }
}

/// One leaf of the value surface a `#[model_schema()]` item published, in the vocabulary the
/// flatten-member refusal names a wire by.
#[cfg(all(feature = "serde", any(feature = "zod", feature = "typescript")))]
#[derive(Clone)]
pub struct WireLeaf {
    pub branch: Vec<usize>,
    pub non_object: Option<&'static str>,
}

#[cfg(all(feature = "serde", any(feature = "zod", feature = "typescript")))]
impl WireLeaf {
    /// The leaf's position, spelled the way the JSON-schema merge spells the same one.
    #[cfg(feature = "zod")]
    pub fn branch_path(&self) -> String {
        self.branch
            .iter()
            .map(usize::to_string)
            .collect::<Vec<String>>()
            .join(".")
    }

    /// Whether this leaf is the absence the name itself offers: the `null` of a choice the
    /// registration publishes at its own top level, one position in from the name and no deeper.
    pub fn is_published_absence(&self) -> bool {
        self.branch.len() == 1 && self.non_object == Some("null")
    }
}

/// What an object flattening an externally tagged enum joins for one of that enum's variants, on
/// each surface that writes a merge of its own.
///
/// The two spellings are recorded together, from one reading of the rendered variant, because they
/// answer one question — what serde writes for this variant into the object being merged — and a
/// surface that answered it on its own would be free to drift from the other.
#[cfg(all(feature = "serde", any(feature = "typescript", feature = "zod")))]
#[derive(Clone)]
pub struct FlattenVariant {
    #[cfg(feature = "typescript")]
    pub typescript: String,
    #[cfg(feature = "zod")]
    pub zod: String,
}

/// What a `#[model_schema()]` item publishes as a value, as the constrained-brand guard reads
/// shapes.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishedShape {
    /// The one shape written under this name, whatever fills it — `None` where that shape is one a
    /// string check lands on, and the answer an unrecorded name leaves.
    Flat(Option<&'static str>),
    /// The name publishes its own type parameter at this position in the list it declares them,
    /// so what it publishes is whatever the argument written there resolves to.
    Parameter(usize),
}

/// A constrained brand's consult the registry had no record to answer with, kept until the named
/// item registers and can answer it.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[derive(Clone)]
pub struct ShapeQuestion {
    /// What each argument the reference wrote resolves to, in the order it wrote them — the filling
    /// a recorded parameter position takes, and `None` where that argument is one a string check
    /// lands on.
    pub argument_shapes: Vec<Option<&'static str>>,
    /// The brand that wrote the checks, named so the refusal says which declaration to fix.
    pub brand: String,
    /// The name it asked about, which is the entry an answer arrives on.
    pub inner: String,
}

#[derive(Clone)]
pub struct AliasInfo {
    pub export_name: String,
    /// What an externally tagged enum's variants are spelled as where an object flattens the enum
    /// itself, one per variant in the order the union writes them, and empty for every other item.
    /// Filled by [`record_flatten_variants`] once that enum's own expansion has rendered them.
    #[cfg(all(feature = "serde", any(feature = "typescript", feature = "zod")))]
    pub flatten_variants: Vec<FlattenVariant>,
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    pub kind: AliasKind,
    #[cfg(feature = "jsonschema")]
    pub module_name: String,
    /// What an untagged enum's members are spelled as where an object flattens the enum itself, one
    /// per member in the order the union writes them — and empty both for every other item and
    /// wherever spelling the members says nothing the enum's own name does not already say. Filled
    /// by [`record_ts_union_members`] once that enum's own expansion has rendered them.
    #[cfg(all(feature = "serde", feature = "typescript"))]
    pub ts_union_members: Vec<String>,
    /// What the value surface written under this name is, in the vocabulary a constrained brand's
    /// refusal names shapes by — and [`PublishedShape::Flat(None)`] both when that surface is one
    /// string checks land on and when nothing has been recorded at all. Filled by
    /// [`record_value_shape`] as each item registers.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    pub value_shape: PublishedShape,
    /// What the value surface written under this name puts on the wire, one entry per leaf of it,
    /// and empty when nothing has been recorded at all. Filled by [`record_wire_leaves`] as each
    /// item registers.
    #[cfg(all(feature = "serde", any(feature = "zod", feature = "typescript")))]
    pub wire: Vec<WireLeaf>,
    /// What an untagged enum's members are spelled as on the Zod surface, and empty for every other
    /// item. Filled by [`record_zod_union_members`] once the enum's own expansion has rendered
    /// them.
    #[cfg(feature = "zod")]
    pub zod_union_members: Vec<ZodUnionMember>,
}

/// The walk over a parsed `pattern` that collects the rewrites its JavaScript spelling needs and
/// the first construct that has no JavaScript spelling at all.
#[derive(Default)]
struct JsSpelling {
    /// The byte span of each construct that is rewritten, beside what it is rewritten to.
    edits: Vec<(Range<usize>, &'static str)>,
    refusal: Option<Unportable>,
}

/// A doc-comment line paired with the span of the `#[doc = "..."]` literal it came from. Each
/// `///` line lowers to its own doc attribute with a real source span; a block comment's
/// embedded lines share the span of the one literal that carries all of them.
#[cfg(feature = "zod")]
#[derive(Clone)]
struct DocLine {
    span: proc_macro2::Span,
    text: String,
}

/// A construct the `regex` crate reads that a JavaScript regex literal cannot be handed as written.
struct Unportable {
    /// Which of the three ways it fails to carry over.
    divergence: Divergence,
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
            Ast::Empty(_) => {}
            Ast::Dot(_) => self.refuse_value_set(
                "the `.` any-character class",
                "one UTF-16 code unit other than a line terminator, where the `regex` crate reads \
                 one character other than a line feed -- so the two already part ways over a \
                 carriage return",
            ),
            Ast::ClassPerl(perl) => self.perl_class(perl, false),
            Ast::Flags(_) => self.refuse(
                "an inline flag directive `(?...)`",
                "a group opening no JavaScript regex parses",
            ),
            Ast::Literal(literal) => self.literal(literal, false),
            Ast::Assertion(assertion) => self.assertion(assertion),
            Ast::ClassUnicode(_) => self.refuse(UNICODE_CLASS_WRITTEN, UNICODE_CLASS_READ_AS),
            Ast::ClassBracketed(class) => self.bracketed_class(class),
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

    /// Walks a `[...]` class, refusing it first if it is negated.
    fn bracketed_class(&mut self, class: &ClassBracketed) {
        if class.negated {
            self.refuse_value_set(
                "a negated character class `[^...]`",
                "the complement of the same members taken one UTF-16 code unit at a time, where \
                 the `regex` crate takes it one character at a time -- so a lone character outside \
                 the Basic Multilingual Plane fills the class here and reaches that literal as the \
                 two code units no one-character class holds",
            );
        }
        self.class_set(&class.kind);
    }

    fn class_item(&mut self, item: &ClassSetItem) {
        match item {
            ClassSetItem::Empty(_) => {}
            ClassSetItem::Perl(perl) => self.perl_class(perl, true),
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
                    let marker = group.span.start.offset + 2;
                    self.edits.push((marker..marker + 1, ""));
                }
            }
            GroupKind::NonCapturing(flags) => {
                for item in &flags.items {
                    let FlagsItemKind::Flag(flag) = &item.kind else {
                        continue;
                    };
                    // `i`, `m` and `s` are the three the modifiers proposal added, so they are
                    // refused for post-dating the baseline; the rest were never in ECMA-262 and
                    // are refused outright. Both refusals name the group the flag was written on.
                    let (written, divergence, read_as) = match flag {
                        Flag::CaseInsensitive => (
                            "the case-insensitive flag on a `(?i:...)` group",
                            Divergence::AboveBaseline,
                            MODIFIER_GROUP_ABOVE_BASELINE_READ_AS,
                        ),
                        Flag::MultiLine => (
                            "the multi-line flag on a `(?m:...)` group",
                            Divergence::AboveBaseline,
                            MODIFIER_GROUP_ABOVE_BASELINE_READ_AS,
                        ),
                        Flag::DotMatchesNewLine => (
                            "the dot-matches-newline flag on a `(?s:...)` group",
                            Divergence::AboveBaseline,
                            MODIFIER_GROUP_ABOVE_BASELINE_READ_AS,
                        ),
                        Flag::SwapGreed => (
                            "the swap-greed flag on a `(?U:...)` group",
                            Divergence::Unreadable,
                            MODIFIER_GROUP_READ_AS,
                        ),
                        Flag::Unicode => (
                            "the Unicode flag on a `(?u:...)` group",
                            Divergence::Unreadable,
                            MODIFIER_GROUP_READ_AS,
                        ),
                        Flag::CRLF => (
                            "the CRLF flag on a `(?R:...)` group",
                            Divergence::Unreadable,
                            MODIFIER_GROUP_READ_AS,
                        ),
                        Flag::IgnoreWhitespace => (
                            "the ignore-whitespace flag on a `(?x:...)` group",
                            Divergence::Unreadable,
                            MODIFIER_GROUP_READ_AS,
                        ),
                    };
                    self.record(divergence, written, read_as);
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

    /// Equalises a `\d`, `\w` or `\s` — or refuses its negation.
    fn perl_class(&mut self, perl: &ClassPerl, in_class: bool) {
        if perl.negated {
            self.refuse_value_set(
                negated_perl_class_written(&perl.kind),
                "the complement of the ASCII class, taken one UTF-16 code unit at a time, where \
                 the `regex` crate takes the complement of the Unicode class one character at a \
                 time -- writing the ASCII members out settles the first difference and leaves \
                 the second",
            );
            return;
        }
        let (bare, bracketed) = perl_class_equalised(&perl.kind);
        let members = if in_class { bare } else { bracketed };
        self.edits
            .push((perl.span.start.offset..perl.span.end.offset, members));
    }

    /// Records `written` as the pattern's refusal, keeping the first construct the walk reached.
    fn record(&mut self, divergence: Divergence, written: &'static str, read_as: &'static str) {
        self.refusal.get_or_insert(Unportable {
            divergence,
            read_as,
            written,
        });
    }

    /// Refuses a construct a JavaScript regex literal has no reading for at all.
    fn refuse(&mut self, written: &'static str, read_as: &'static str) {
        self.record(Divergence::Unreadable, written, read_as);
    }

    /// Refuses a construct both grammars read and pick out different characters by.
    fn refuse_value_set(&mut self, written: &'static str, read_as: &'static str) {
        self.record(Divergence::ValueSet, written, read_as);
    }

    /// `pattern` with every construct the walk collected an equalising spelling for replaced by it.
    fn rewritten(mut self, pattern: &str) -> String {
        if self.edits.is_empty() {
            return pattern.to_owned();
        }
        self.edits.sort_unstable_by_key(|(span, _)| span.start);
        let mut result = String::with_capacity(pattern.len());
        let mut cut = 0;
        for (span, replacement) in self.edits {
            result.push_str(&pattern[cut..span.start]);
            result.push_str(replacement);
            cut = span.end;
        }
        result.push_str(&pattern[cut..]);
        result
    }
}

#[cfg(feature = "zod")]
thread_local! {
    /// The names whose items publish a Zod factory. Kept out of [`ALIAS_INFO`] so the answer
    /// survives the item's own registration — see [`record_zod_factory`].
    static ZOD_FACTORY_PUBLISHERS: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
    /// Each generic item's own `$SchemaDefault` fold-comparison keys, one plain rendering per
    /// parameter in declaration order — see [`record_zod_default_arguments`].
    static ZOD_DEFAULT_ARGUMENTS: RefCell<HashMap<String, Vec<String>>> = RefCell::new(HashMap::new());
}

thread_local! {
    static ALIAS_INFO: RefCell<HashMap<String, AliasInfo>> = RefCell::new(HashMap::new());
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
thread_local! {
    static SHAPE_QUESTIONS: RefCell<Vec<ShapeQuestion>> = const { RefCell::new(Vec::new()) };
}

/// Keeps a constrained brand's unanswered consult for whichever expansion registers the name it
/// asked about.
///
/// Recorded once the brand has passed its own guards, so a brand that publishes nothing leaves no
/// question behind for a later item to refuse it over.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
pub fn record_shape_question(question: ShapeQuestion) {
    SHAPE_QUESTIONS.with(|questions| questions.borrow_mut().push(question));
}

/// Every question asked about a name, in the order the brands asking them expanded.
///
/// The questions are left in place rather than taken: a name is registered by one expansion, and
/// leaving them makes reading them an observation rather than a move — nothing downstream has to
/// know whether something else read first.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
pub fn shape_questions_for(rust_ident: &str) -> Vec<ShapeQuestion> {
    SHAPE_QUESTIONS.with(|questions| {
        questions
            .borrow()
            .iter()
            .filter(|question| question.inner == rust_ident)
            .cloned()
            .collect()
    })
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
                #[cfg(all(feature = "serde", any(feature = "typescript", feature = "zod")))]
                flatten_variants: Vec::new(),
                kind,
                #[cfg(feature = "jsonschema")]
                module_name: module_name.to_owned(),
                #[cfg(all(feature = "serde", feature = "typescript"))]
                ts_union_members: Vec::new(),
                value_shape: PublishedShape::Flat(None),
                #[cfg(all(feature = "serde", any(feature = "zod", feature = "typescript")))]
                wire: Vec::new(),
                #[cfg(feature = "zod")]
                zod_union_members: Vec::new(),
            },
        );
    });
}

/// Records what the value surface written under a name is, on the entry that name has just
/// registered.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
pub fn record_value_shape(rust_ident: &str, shape: PublishedShape) {
    ALIAS_INFO.with(|map| {
        if let Some(info) = map.borrow_mut().get_mut(rust_ident) {
            info.value_shape = shape;
        }
    });
}

/// Records what the value surface written under a name puts on the wire, on the entry that name has
/// just registered.
#[cfg(all(feature = "serde", any(feature = "zod", feature = "typescript")))]
pub fn record_wire_leaves(rust_ident: &str, leaves: &[WireLeaf]) {
    ALIAS_INFO.with(|map| {
        if let Some(info) = map.borrow_mut().get_mut(rust_ident) {
            info.wire = leaves.to_vec();
        }
    });
}

/// Records what an untagged enum's members are spelled as on the Zod surface, on the entry that
/// enum has already registered.
#[cfg(feature = "zod")]
pub fn record_zod_union_members(rust_ident: &str, members: &[ZodUnionMember]) {
    ALIAS_INFO.with(|map| {
        if let Some(info) = map.borrow_mut().get_mut(rust_ident) {
            info.zod_union_members = members.to_vec();
        }
    });
}

/// Records what an untagged enum's members are spelled as where an object flattens the enum itself,
/// on the entry that enum has already registered.
#[cfg(all(feature = "serde", feature = "typescript"))]
pub fn record_ts_union_members(rust_ident: &str, members: &[String]) {
    ALIAS_INFO.with(|map| {
        if let Some(info) = map.borrow_mut().get_mut(rust_ident) {
            info.ts_union_members = members.to_vec();
        }
    });
}

/// Records what an externally tagged enum's variants are spelled as where an object flattens the
/// enum itself, on the entry that enum has already registered.
#[cfg(all(feature = "serde", any(feature = "typescript", feature = "zod")))]
pub fn record_flatten_variants(rust_ident: &str, variants: &[FlattenVariant]) {
    ALIAS_INFO.with(|map| {
        if let Some(info) = map.borrow_mut().get_mut(rust_ident) {
            info.flatten_variants = variants.to_vec();
        }
    });
}

/// Records which of the two Zod bindings a name publishes.
#[cfg(feature = "zod")]
pub fn record_zod_factory(rust_ident: &str, publishes: bool) {
    ZOD_FACTORY_PUBLISHERS.with(|names| {
        if publishes {
            names.borrow_mut().insert(rust_ident.to_owned());
        } else {
            names.borrow_mut().remove(rust_ident);
        }
    });
}

/// Whether the Zod binding published under `rust_ident` is a factory rather than a `const` — what a
/// reference to the name has to know to write itself. See [`record_zod_factory`].
#[cfg(feature = "zod")]
pub fn publishes_zod_factory(rust_ident: &str) -> bool {
    ZOD_FACTORY_PUBLISHERS.with(|names| names.borrow().contains(rust_ident))
}

/// Records `rust_ident`'s own `$SchemaDefault` fold-comparison keys — the plain [`FieldDef::zod_type`]
/// rendering of each declared-default field, one per parameter in declaration order, computed
/// before deferral and before a constrained brand's `.min`/`.max`/`.check` chain is appended.
#[cfg(feature = "zod")]
pub fn record_zod_default_arguments(rust_ident: &str, arguments: Vec<String>) {
    ZOD_DEFAULT_ARGUMENTS.with(|map| {
        map.borrow_mut().insert(rust_ident.to_owned(), arguments);
    });
}

/// The argument list [`record_zod_default_arguments`] recorded for `rust_ident`, or `None` where
/// nothing was — the item declares no parameter, has not registered yet, or this build never
/// reads defaults at all.
#[cfg(feature = "zod")]
pub fn zod_default_arguments(rust_ident: &str) -> Option<Vec<String>> {
    ZOD_DEFAULT_ARGUMENTS.with(|map| map.borrow().get(rust_ident).cloned())
}

/// The characters a `\d`, `\w` or `\s` covers in *both* engines, written out as a class body.
const fn perl_class_equalised(kind: &ClassPerlKind) -> (&'static str, &'static str) {
    match *kind {
        ClassPerlKind::Digit => ("0-9", "[0-9]"),
        ClassPerlKind::Word => ("0-9A-Za-z_", "[0-9A-Za-z_]"),
        ClassPerlKind::Space => (r"\t\n\v\f\r ", r"[\t\n\v\f\r ]"),
    }
}

/// How a rejection names the negated form of a perl class.
const fn negated_perl_class_written(kind: &ClassPerlKind) -> &'static str {
    match *kind {
        ClassPerlKind::Digit => r"the `\D` negated digit class",
        ClassPerlKind::Word => r"the `\W` negated word class",
        ClassPerlKind::Space => r"the `\S` negated whitespace class",
    }
}

pub fn lookup_alias_info(rust_ident: &str) -> Option<AliasInfo> {
    ALIAS_INFO.with(|map| map.borrow().get(rust_ident).cloned())
}

/// The type a spelling names, read through the invisible grouping a `macro_rules!` substitution
/// arrives inside.
pub fn written_type(ty: &Type) -> &Type {
    let mut current = ty;
    while let Type::Group(group) = current {
        current = &group.elem;
    }
    current
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

/// The spelling every reference to a `#[model_schema()]` item falls back to when the registry
/// cannot answer for it, which is what a reference standing *before* the item expanded has and
/// nothing else — the ident with the `Json` suffix taken off, that being what the field walk
/// records for a sibling and what [`ident_schema_module_name`] names the module from.
#[cfg(any(feature = "typescript", feature = "zod"))]
fn ident_reexport_name(rust_ident: &str, export_name: &str) -> Option<String> {
    let referenced = safe_type_name(rust_ident);
    (referenced != export_name).then_some(referenced)
}

/// The names an item's own declaration binds as type parameters.
pub fn type_parameters_in_scope(generics: &Generics) -> Vec<String> {
    generics
        .params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Type(type_param) => Some(type_param.ident.to_string()),
            GenericParam::Const(_) | GenericParam::Lifetime(_) => None,
        })
        .collect()
}

/// The parameter list a generic item's `TypeScript` declaration is written under — `<IdType>`,
/// `<IdType, DateType>` — or the empty string for an item that binds none.
#[cfg(feature = "typescript")]
pub fn ts_generic_params(generics: &Generics) -> String {
    let parameters = type_parameters_in_scope(generics);
    if parameters.is_empty() {
        String::new()
    } else {
        format!("<{}>", parameters.join(", "))
    }
}

/// The `TypeScript` line an item publishes under its own Rust ident, or nothing when it is already
/// exported under it. The parameter list is repeated on both sides so a generic item stays generic
/// through the re-export.
#[cfg(feature = "typescript")]
pub fn ident_reexport_ts(rust_ident: &str, export_name: &str, ts_generics: &str) -> String {
    ident_reexport_name(rust_ident, export_name).map_or_else(String::new, |referenced| {
        format!("\n\nexport type {referenced}{ts_generics} = {export_name}{ts_generics};")
    })
}

/// The zod counterpart of [`ident_reexport_ts`] — a binding, not a second schema, so the two names
/// carry the one schema the item published. It is written unannotated because a zod-only build has
/// no `ZodType` to annotate it with, and the binding's own type is the exported schema's.
#[cfg(feature = "zod")]
pub fn ident_reexport_zod(rust_ident: &str, export_name: &str, binding_suffix: &str) -> String {
    ident_reexport_name(rust_ident, export_name).map_or_else(String::new, |referenced| {
        format!("\n\nexport const {referenced}{binding_suffix} = {export_name}{binding_suffix};")
    })
}

/// The identifier a factory binds one type parameter's schema argument to — `idType` for `IdType`.
///
/// A Zod schema is a value and a `const` cannot be parameterised, so a generic type publishes a
/// function of one argument per parameter rather than a schema, and a field written with a
/// parameter composes that argument. Lower-camel of the declared name, so the argument reads as
/// the parameter it fills while staying a name of its own beside it.
#[cfg(feature = "zod")]
pub fn zod_factory_argument(parameter: &str) -> String {
    let mut characters = parameter.chars();
    characters.next().map_or_else(String::new, |first| {
        format!("{}{}", first.to_lowercase(), characters.as_str())
    })
}

/// The local a JSON document binds one type parameter's argument document to — `_arg_id_type` for
/// `IdType`.
#[cfg(feature = "jsonschema")]
pub fn json_argument_binding(parameter: &str) -> String {
    format!("_arg_{}", to_snake_case(parameter))
}

/// [`compute_alias_export_name`] for a declared item — a struct, an enum, a tuple struct, a branded
/// newtype. Without an override the item keeps the name it is declared under, which is the one
/// difference from an alias: an alias has no surface name of its own and is given the `Type` suffix.
pub fn compute_item_export_name(rust_ident: &str, override_name: Option<&str>) -> String {
    override_name.map_or_else(|| safe_type_name(rust_ident), ToOwned::to_owned)
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
/// Extracts and concatenates documentation comments from a `syn::ItemStruct`.
pub fn get_struct_docs(item_struct: &ItemStruct) -> Option<Vec<String>> {
    collect_doc_lines(&item_struct.attrs)
}

/// An enum's doc lines, read for the prose two surfaces publish (a plain enum's `JSDoc`, every
/// shape's item description) — not for its ` ```rust example ` block, which
/// [`extract_example_tokens`] reads off the attributes directly so the tokens it builds keep their
/// spans.
#[cfg(any(
    feature = "typescript",
    feature = "zod",
    all(feature = "serde", feature = "jsonschema")
))]
pub fn get_enum_docs(item_enum: &ItemEnum) -> Option<Vec<String>> {
    collect_doc_lines(&item_enum.attrs)
}

pub fn get_variant_docs(variant: &Variant) -> Option<Vec<String>> {
    collect_doc_lines(&variant.attrs)
}

pub fn get_field_docs(field: &Field) -> Option<Vec<String>> {
    collect_doc_lines(&field.attrs)
}

/// The doc lines of anything carrying attributes, read by the `TypeScript` surface for the prose
/// an alias publishes.
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

/// [`DocLine`]s for every doc attribute on `attrs`, one per physical line.
#[cfg(feature = "zod")]
fn doc_lines_with_spans(attrs: &[Attribute]) -> Vec<DocLine> {
    let mut lines = Vec::new();
    for attr in attrs {
        if attr.path().is_ident("doc")
            && let Meta::NameValue(meta_name_value) = &attr.meta
            && let Expr::Lit(syn::ExprLit {
                lit: Lit::Str(lit_str),
                ..
            }) = &meta_name_value.value
        {
            // `resolved_at` keeps the doc line's location (what a diagnostic underlines) while
            // giving the token the macro's own hygiene (what marks it as generated rather than
            // user-written) — respanning bare would also make an ordinary lint pass (clippy's
            // style lints, not just rustc's own type errors) treat the example as code the author
            // typed at that doc line, rather than the illustrative snippet it is.
            let span = lit_str.span().resolved_at(proc_macro2::Span::call_site());
            for line in lit_str.value().lines() {
                lines.push(DocLine {
                    text: line.trim().to_owned(),
                    span,
                });
            }
        }
    }
    lines
}

/// The [`DocLine`]s inside the first ` ```rust example ` fence, or `None` where there is no such
/// fence. Later fences are ignored.
#[cfg(feature = "zod")]
fn example_doc_lines(lines: &[DocLine]) -> Option<Vec<DocLine>> {
    let mut in_example_block = false;
    let mut example_lines: Vec<DocLine> = Vec::new();

    for line in lines {
        let trimmed = line.text.trim();
        let cleaned = trimmed.strip_prefix('*').unwrap_or(trimmed).trim();

        if cleaned == "```rust example" {
            if !example_lines.is_empty() {
                break;
            }
            in_example_block = true;
            continue;
        }

        if in_example_block && cleaned == "```" {
            break;
        }

        if in_example_block {
            example_lines.push(line.clone());
        }
    }

    if example_lines.is_empty() {
        None
    } else {
        Some(example_lines)
    }
}

/// Whether `line` (already trimmed) is a bare `use` statement — the one pattern
/// `transform_example_code` strips regardless of where it sits, so it is dropped line-by-line up
/// front rather than through the whole-example regex.
#[cfg(feature = "zod")]
fn is_use_statement(line: &str) -> bool {
    regex::Regex::new(r"^use\s+[^;]+;$").unwrap().is_match(line)
}

/// Recursively respans every token, and every group's own delimiter span, onto `span`.
#[cfg(feature = "zod")]
fn respan(tokens: proc_macro2::TokenStream, span: proc_macro2::Span) -> proc_macro2::TokenStream {
    tokens
        .into_iter()
        .map(|tree| match tree {
            proc_macro2::TokenTree::Group(group) => {
                let mut respanned =
                    proc_macro2::Group::new(group.delimiter(), respan(group.stream(), span));
                respanned.set_span(span);
                proc_macro2::TokenTree::Group(respanned)
            }
            mut other @ (proc_macro2::TokenTree::Ident(_)
            | proc_macro2::TokenTree::Punct(_)
            | proc_macro2::TokenTree::Literal(_)) => {
                other.set_span(span);
                other
            }
        })
        .collect()
}

/// Groups `lines` into the smallest runs of consecutive lines that parse as balanced tokens on
/// their own, pairing each run's source text with the span of the line it starts on. A single
/// statement occupies its own run in the common case; a value split across lines (a multi-line
/// struct literal, say) is the smallest run whose delimiters close.
#[cfg(feature = "zod")]
fn raw_statement_groups(lines: &[DocLine]) -> Vec<(String, proc_macro2::Span)> {
    let mut groups = Vec::new();
    let mut buffer = String::new();
    let mut buffer_span: Option<proc_macro2::Span> = None;

    for line in lines {
        if buffer.is_empty() {
            buffer_span = Some(line.span);
        } else {
            buffer.push('\n');
        }
        buffer.push_str(&line.text);

        if buffer.parse::<proc_macro2::TokenStream>().is_ok() {
            let span = buffer_span.take().unwrap();
            groups.push((mem::take(&mut buffer), span));
        }
    }
    if !buffer.is_empty() {
        groups.push((buffer, buffer_span.unwrap()));
    }
    groups
}

/// The example's tokens, each respanned onto the `///` line — or, for a value split across
/// lines, the first line of the run — it was written on.
///
/// `transform_example_code`'s `println!`/`let _` unwrapping only ever matches a whole example's
/// trailing statement (its regexes anchor on the end of the input), so it is applied to the last
/// group alone; every earlier group keeps its own raw tokens, respanned but otherwise untouched.
#[cfg(feature = "zod")]
fn respan_example_tokens(lines: &[DocLine]) -> proc_macro2::TokenStream {
    let content: Vec<DocLine> = lines
        .iter()
        .filter(|line| {
            let trimmed = line.text.trim();
            !trimmed.is_empty() && !is_use_statement(trimmed)
        })
        .cloned()
        .collect();

    let groups = raw_statement_groups(&content);
    let last_index = groups.len().saturating_sub(1);

    let mut out = proc_macro2::TokenStream::new();
    for (index, (source, span)) in groups.iter().enumerate() {
        let candidate = if index == last_index {
            transform_example_code(source)
        } else {
            source.clone()
        };
        let tokens: proc_macro2::TokenStream = candidate.parse().unwrap();
        out.extend(respan(tokens, *span));
    }
    out
}

/// The example's tokens, respanned line-by-line onto the `///` lines that wrote them, or `None`
/// where there is no ` ```rust example ` fence.
///
/// `str::parse::<TokenStream>()` alone stamps every token with `Span::call_site()` — for an
/// attribute macro, the whole `#[model_schema(...)]` invocation — so a typo or a type mismatch
/// inside the example used to be reported there instead of on the line the author wrote. Each
/// `///` line lowers to its own `#[doc = "..."]` attribute with a real span, which is enough to
/// point the diagnostic at that line (or, where a value spans several lines, the first of them).
#[cfg(feature = "zod")]
pub fn extract_example_tokens(attrs: &[Attribute]) -> Option<proc_macro2::TokenStream> {
    let lines = doc_lines_with_spans(attrs);
    let example_lines = example_doc_lines(&lines)?;
    Some(respan_example_tokens(&example_lines))
}

/// Transforms doctest-compatible example code to be suitable for `schema_example()`.
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
/// Every doc body the crate writes — an item's, an alias's, a field's, an enum variant's, and the
/// descriptions spelled from the same lines — passes through here, so the block is dropped once
/// rather than at each surface.
pub fn strip_examples_from_docs(docs: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut in_example_block = false;

    for line in docs {
        let trimmed = line.trim();
        // Strip leading asterisk from block-style comments
        let cleaned = trimmed.strip_prefix('*').unwrap_or(trimmed).trim();

        if cleaned == "```rust example" {
            in_example_block = true;
            continue;
        }

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
        return Err(match refusal.divergence {
            Divergence::Unreadable => format!(
                "`pattern` uses {}, which the `regex` crate reads and a JavaScript regex literal \
                 does not: there the same bytes are {}. The Zod schema splices this string \
                 between `/` delimiters and the JSON Schema `pattern` keyword is an ECMA-262 \
                 regex, so the constraint would say one thing in the Rust validator and another \
                 -- or nothing at all -- on the surfaces generated beside it.",
                refusal.written, refusal.read_as
            ),
            Divergence::ValueSet => format!(
                "`pattern` uses {}, which the `regex` crate and a JavaScript regex literal both \
                 read and cover different characters by: there the same bytes are {}. What is \
                 left over either way is {}. The Zod schema splices this string between `/` \
                 delimiters and the JSON Schema `pattern` keyword is an ECMA-262 regex, so the \
                 constraint would accept one set of strings in the Rust validator and a different \
                 set on the surfaces generated beside it. Write the characters you mean out as a \
                 class instead.",
                refusal.written, refusal.read_as, ASTRAL_DIVERGENCE
            ),
            Divergence::AboveBaseline => format!(
                "`pattern` uses {}, which a JavaScript regex literal carries only on an engine \
                 newer than {}, the baseline the schemas this crate generates are written for: \
                 there the same bytes are {}. The Zod schema splices this string between `/` \
                 delimiters and the JSON Schema `pattern` keyword is an ECMA-262 regex, so an \
                 engine at that baseline throws where the schema loads instead of validating \
                 anything.",
                refusal.written, JS_ENGINE_BASELINE, refusal.read_as
            ),
        });
    }
    Ok(walk.rewritten(pattern))
}

/// The `pattern` handed back when it turns some value away, or the refusal it earns for admitting
/// every value -- spanned on the literal the author wrote.
pub fn constraining_pattern(lit: &LitStr, pattern: String) -> Result<String, syn::Error> {
    if admits_every_value(&pattern) {
        return Err(syn::Error::new_spanned(
            lit,
            "`pattern` admits every value, so it constrains nothing: every string has a position \
             this matches at, which leaves the generated validator turning no value away and the \
             Zod and JSON schemas publishing a check every payload passes. Taking it would leave a \
             contract that says the value is checked when nothing checks it -- the same silent \
             claim a bound written where no surface reads one is refused for. Write the pattern \
             the value has to match, or drop it.",
        ));
    }
    Ok(pattern)
}

/// Whether a search for `pattern` succeeds in every haystack, read off the HIR rather than off the
/// pattern text: `^` and `(^)` and `^|a` are one verdict written three ways, and `^$` is written
/// out of the same two anchors as `^` and `$` yet admits only the empty string.
fn admits_every_value(pattern: &str) -> bool {
    regex_syntax::ParserBuilder::new()
        .unicode(true)
        .utf8(true)
        .build()
        .parse(pattern)
        .is_ok_and(|parsed| matches_every_haystack(&parsed))
}

/// Whether a search for this sub-expression succeeds in every haystack.
fn matches_every_haystack(hir: &hir::Hir) -> bool {
    match hir.kind() {
        hir::HirKind::Look(_) => is_text_anchor(hir),
        hir::HirKind::Capture(capture) => matches_every_haystack(&capture.sub),
        hir::HirKind::Alternation(branches) => branches.iter().any(matches_every_haystack),
        hir::HirKind::Concat(parts) => concat_matches_every_haystack(parts),
        hir::HirKind::Empty
        | hir::HirKind::Literal(_)
        | hir::HirKind::Class(_)
        | hir::HirKind::Repetition(_) => matches_at_every_position(hir),
    }
}

/// Whether a concatenation matches at a position every haystack has.
///
/// Every part that asks the haystack for nothing can be tried anywhere, so a run of them matches
/// anywhere; one whole-text anchor among them fixes that anywhere to a position every haystack
/// still has. A second anchor is what breaks it -- `^$` requires the start and the end to be the
/// same position -- and so is any part that consumes a character.
fn concat_matches_every_haystack(parts: &[hir::Hir]) -> bool {
    parts.iter().filter(|part| is_text_anchor(part)).count() <= 1
        && parts
            .iter()
            .filter(|part| !is_text_anchor(part))
            .all(matches_at_every_position)
}

/// Whether this sub-expression matches the empty string at every position of every haystack, and
/// so asks the haystack for nothing at all.
fn matches_at_every_position(hir: &hir::Hir) -> bool {
    match hir.kind() {
        hir::HirKind::Empty => true,
        hir::HirKind::Repetition(repetition) => repetition.min == 0,
        hir::HirKind::Capture(capture) => matches_at_every_position(&capture.sub),
        hir::HirKind::Concat(parts) => parts.iter().all(matches_at_every_position),
        hir::HirKind::Alternation(branches) => branches.iter().any(matches_at_every_position),
        hir::HirKind::Literal(_) | hir::HirKind::Class(_) | hir::HirKind::Look(_) => false,
    }
}

/// Whether a sub-expression is one of the two whole-text anchors, the only assertions a search
/// finds in every haystack. The line anchors `(?m)` turns these into never reach this crate --
/// [`portable_pattern`] refuses an inline flag first -- and every word-boundary flavour asks the
/// haystack for something the empty string does not have.
fn is_text_anchor(hir: &hir::Hir) -> bool {
    matches!(
        *hir.kind(),
        hir::HirKind::Look(hir::Look::Start | hir::Look::End)
    )
}

/// What a `pattern` accepts stated without a regex, for exactly the patterns
/// `clippy::trivial_regex` proves one is unnecessary for -- and `None` for every other pattern,
/// which keeps its `regex::Regex` and is the only thing that reads a pattern of any real shape.
#[cfg(feature = "serde")]
pub fn trivial_pattern(pattern: &str) -> Option<TrivialPattern> {
    let parsed = regex_syntax::ParserBuilder::new()
        .unicode(true)
        .utf8(true)
        .build()
        .parse(pattern)
        .ok()?;
    match parsed.kind() {
        hir::HirKind::Literal(_) => literal_needle(from_ref(&parsed)).map(TrivialPattern::Contains),
        hir::HirKind::Concat(parts) => trivial_concat(parts),
        hir::HirKind::Empty
        | hir::HirKind::Class(_)
        | hir::HirKind::Look(_)
        | hir::HirKind::Repetition(_)
        | hir::HirKind::Capture(_)
        | hir::HirKind::Alternation(_) => None,
    }
}

/// A concatenation's equivalent check, decided by what sits at its two ends.
///
/// The arms are in the lint's order, and the fall-through it relies on is not reachable from any
/// of them: a concatenation that starts or ends with an anchor holds something that is not a
/// literal, so the all-literals reading is already ruled out by the time an anchored arm's needle
/// comes back missing.
#[cfg(feature = "serde")]
fn trivial_concat(parts: &[hir::Hir]) -> Option<TrivialPattern> {
    let opens_at_text_start = is_anchor(parts.first()?, hir::Look::Start);
    let closes_at_text_end = is_anchor(parts.last()?, hir::Look::End);
    let inner = parts.get(1..parts.len() - 1).unwrap_or_default();

    if opens_at_text_start && closes_at_text_end {
        if inner.is_empty() {
            return Some(TrivialPattern::IsEmpty);
        }
        return literal_needle(inner).map(TrivialPattern::Equals);
    }
    if opens_at_text_start && matches!(*parts.last()?.kind(), hir::HirKind::Literal(_)) {
        return literal_needle(&parts[1..]).map(TrivialPattern::StartsWith);
    }
    if closes_at_text_end && matches!(*parts.first()?.kind(), hir::HirKind::Literal(_)) {
        return literal_needle(&parts[..parts.len() - 1]).map(TrivialPattern::EndsWith);
    }
    literal_needle(parts).map(TrivialPattern::Contains)
}

/// Whether one part of a concatenation is the given whole-haystack anchor.
#[cfg(feature = "serde")]
fn is_anchor(part: &hir::Hir, anchor: hir::Look) -> bool {
    matches!(*part.kind(), hir::HirKind::Look(look) if look == anchor)
}

/// The non-empty `str` a run of parts spells, or `None` where any of them is not a literal.
///
/// An empty needle would name a check that always passes, which is a different statement than any
/// of these variants makes; bytes that are not UTF-8 name no `str` at all. Neither is reachable
/// through a `pattern` parsed in UTF-8 mode, and both keep the regex rather than guess.
#[cfg(feature = "serde")]
fn literal_needle(parts: &[hir::Hir]) -> Option<String> {
    let mut bytes: Vec<u8> = Vec::new();
    for part in parts {
        let hir::HirKind::Literal(hir::Literal(run)) = part.kind() else {
            return None;
        };
        bytes.extend_from_slice(run);
    }
    if bytes.is_empty() {
        return None;
    }
    String::from_utf8(bytes).ok()
}

/// Escapes a regex pattern for splicing between the `/` delimiters of a JavaScript regex literal.
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
