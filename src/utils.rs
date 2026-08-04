use core::cell::RefCell;
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

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use syn::ItemEnum;
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use syn::ItemStruct;

/// The JavaScript engine generation the emitted Zod regex literals and JSON Schema `pattern`
/// keywords are written for, and therefore the line the guard admits and refuses along.
///
/// A pattern this crate emits is read by whatever engine loads the generated schema, not by the
/// one that happened to be installed where the derive ran, so the admission list has to be
/// decided against a version written down rather than measured. ES2018 is the floor because it is
/// what the guard's own translations already assume: `(?P<name>...)` is rewritten to
/// `(?<name>...)`, and named capture groups are ES2018. Nothing else the crate emits needs
/// anything newer, so nothing is bought by raising it.
const JS_ENGINE_BASELINE: &str = "ES2018";

/// What a JavaScript regex literal makes of the three flags the ECMA-262 regular expression
/// modifiers proposal did add.
///
/// An engine at [`JS_ENGINE_BASELINE`] predates the proposal and rejects the group opening; a
/// recent one parses it and matches what the `regex` crate matches. That is why these are refused
/// against the recorded baseline rather than against whichever runtime is at hand — the schema is
/// read wherever it is loaded, not where it was generated.
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
///
/// A type path sees straight through an alias and a `#[serde(transparent)]` brand writes what its
/// inner writes, so both answer for their target and their inner rather than for themselves — and
/// through the registry, so a chain of either carries its end's answer to every link.
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
///
/// `branch` is the trail of one-based choices the member sits at, so a member of a nested union is
/// named `1.2` rather than twice as `2` — the position the JSON-schema merge names the same member
/// by, the recording being already multiplied out where that merge descends.
///
/// `non_object` is what serde writes the member as when that is provably not an object, named by
/// the JSON type keyword the other surface writes for it. serde flattens structs and maps and
/// nothing else, so such a member is one no object can be merged with, on any surface.
///
/// Both travel under `serde` alone, which is the feature that reads `#[serde(untagged)]` and
/// `#[serde(flatten)]` at all: without it nothing records a member and nothing merges one, and the
/// spelling is all the Zod surface still writes.
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
///
/// `branch` is the trail of one-based choices the leaf sits at behind the name, empty for a surface
/// that offers no choice at all — so an item publishing `anyOf[value, null]` carries its null at
/// `2`, and a member naming that item carries it at the member's own position followed by that `2`.
/// `non_object` is the JSON type keyword the leaf describes as where the registration proves it is
/// no object, and `None` where nothing there proves it — a map, an object, and a name nothing has
/// classified alike.
///
/// Recorded and read wherever `serde` meets a surface that writes a merge of its own — `zod`, which
/// multiplies the object over the source's branches, and `typescript`, which spells the source's
/// absence beside it. Without `serde` no field reaches a merge at all, and without either surface
/// there is nothing that would ask.
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
    ///
    /// That depth is what the answer turns on. serde writes no member at all for this leaf and
    /// reads the payload carrying none of them back as the absent value, so a source reached this
    /// way writes two key sets and the merge owes it both. A `null` further down sits under a
    /// choice serde matched a member on, where the same payload is one serde writes and then
    /// matches no member for — the refusal a member position already carries.
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
///
/// One word cannot answer for a name that publishes a family. A generic item whose written target
/// *is* one of its own parameters writes whatever the instantiation hands it, so the only flat
/// answer available at its declaration is the opaque one every parameter describes as — and a
/// brand written over `Later<String>` would then be refused for a shape the emission never
/// composes, while the same declaration written above it is admitted for want of any record at
/// all. So such a name records the position instead, and the reader fills it with the argument the
/// reference carries.
///
/// A target with its own fixed shape — a `Vec<T>`, a map, a tuple — records that shape, which no
/// filling changes.
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
///
/// At the brand's own expansion a name declared below it and a name this crate never expands are
/// one absence, and the argument the reference wrote proves nothing on its own — a publisher may
/// write a string whatever its parameter is handed. So the admission stands at that moment, and
/// what is recorded here is the question rather than a verdict. The registry is a compile-local
/// recording the later expansion also writes to, so the item that finally registers under the name
/// reads the questions naming it and answers them in its own output.
///
/// The arguments are kept as the shapes they resolve to rather than as the types they were written
/// as, because the brand's expansion is the only one still holding those tokens.
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
    ///
    /// The shape above it answers the constrained-brand guard's question, which is what a check can
    /// be appended to; this answers the merge's, which is whether an object can be joined to it —
    /// two questions one word could not hold apart, an `integer` and a `number` being one shape and
    /// two documents, and an array and a map one shape and opposite answers.
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
///
/// The AST walked is the one `regex::Regex::new` is itself built on, so the guard's reading of a
/// pattern is the crate's own reading of it. A second grammar written here would answer for the
/// crate's while drifting from it, and it would have to tell `[--/]`, three ordinary class members,
/// from `[+--]`, a set difference — which the bytes alone do not say.
#[derive(Default)]
struct JsSpelling {
    /// The byte span of each construct that is rewritten, beside what it is rewritten to.
    ///
    /// Every rewrite is local — it replaces one construct's own span and leaves the bytes around
    /// it alone — which is what makes it safe to apply them all in one pass: `(?P<name>` loses its
    /// `P`, and a `\d`, `\w` or `\s` is replaced by the members it stands for. The `regex` crate
    /// reads the results back exactly as it read the originals, so the one string that goes to all
    /// three surfaces still means to the Rust validator what the guard decided it means.
    ///
    /// Escaping a class-opening `]` looks like the same kind of fix and is not — `[]-a]` is three
    /// members and `[\]-a]` is a range — which is why that one is refused rather than rewritten.
    edits: Vec<(Range<usize>, &'static str)>,
    refusal: Option<Unportable>,
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
    ///
    /// A negated class is the last construct both grammars read and fill differently, and the
    /// members cannot settle it: the complement is taken one code unit at a time there and one
    /// character at a time here, so `[^0-9]` parts ways over an astral character exactly as the
    /// `\D` above it does. Nothing is bought by admitting the bracketed spelling of a construct
    /// already refused under its perl one.
    ///
    /// The one class whose `regex` reading does leave every astral character out has to name them
    /// by astral bounds, and a flagless literal reads those as surrogate halves in descending
    /// order and will not parse the class at all — so there is no admissible spelling to fall back
    /// to, which is what makes refusal the whole verdict rather than a table of survivors.
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
    ///
    /// The plain forms name a set each engine reads its own way, and the members they share can be
    /// written out, so they are. The negated forms name a *complement*, and a complement taken one
    /// code unit at a time is not the complement taken one character at a time however the members
    /// are spelled: `[^0-9]` diverges over an astral character exactly as `\D` does. Nothing is
    /// gained by rewriting them, so they are refused with the divergence named.
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
///
/// The question is the constrained-brand guard's: a brand appends `.min`/`.max`/a regex check to
/// its inner's own schema binding, and a name is the one inner spelling whose binding this
/// expansion cannot read off the declaration — the schema lives in the module the *named* item
/// published. So each item answers for itself as it registers, and a brand over a name reads the
/// answer back rather than guessing at it.
///
/// [`PublishedShape::Flat(None)`] is what an unanswered name and a string-checked one both leave,
/// and the guard treats them alike: a name it cannot classify keeps the emission it has always
/// had. That is the same regime the map-key registry already runs — `AliasKind::Unknown` is a name
/// that was not registered before the type reading it — rather than a second one.
///
/// A name whose written target *is* one of its own parameters records that position instead of a
/// word, because one word would have to stand for every instantiation and only the opaque one
/// does — see [`PublishedShape`].
///
/// A chain resolves one link at a time and cannot cycle, because an entry is only ever built from
/// entries registered before it.
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
///
/// The question is the flatten merge's: what serde writes for a member, named by the JSON type
/// keyword the other surface writes for the same member, so the two refuse the same declaration in
/// the same words. A name is the one member spelling the expansion reading it cannot answer for —
/// the document lives in the module the *named* item published — so each item answers for itself as
/// it registers.
///
/// Recorded as leaves rather than as one word because a published surface may be a choice: an item
/// whose own surface is nullable publishes its value and a bare `null`, and the merge descending it
/// names that `null` a level below the member. One word at the member's own position could not say
/// where it sits, and naming it at the member's position would put the two merges into
/// disagreement — the very thing the branch trail exists to prevent.
///
/// A chain resolves one link at a time and cannot cycle, because an entry is only ever built from
/// entries registered before it.
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
///
/// A merge that flattens the enum is a different macro invocation from the enum's own, so the
/// members reach it through the registry or not at all — an intersection recognizes exactly the
/// keys its operands name, and a `z.union` names none, so the merge joins one member per branch
/// rather than the union as one operand.
///
/// What is recorded is already multiplied out: a member that is itself a recorded union contributes
/// that union's members instead of its own name. That leaves the merge nothing to walk, and the
/// walk is what could not be made to terminate — two unions naming each other is a shape a merge is
/// free to reach. The recording cannot hold such a cycle, because an entry is only ever built from
/// entries registered before it.
///
/// Each member carries where it sits and whether serde writes it as an object, because the merge is
/// the position that has to answer for both and the spelling tells it neither. The member itself is
/// left alone: an untagged enum may hold a scalar, and it is joining that scalar to an object that
/// no value satisfies.
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
///
/// The two spellings of one union part where an object is joined to it. Standing alone the union
/// describes one member at a time, and the enum's own name is the whole of what it publishes;
/// intersected with an open object each member is left satisfied by a payload carrying the other
/// members' keys as well, which serde writes for no value. The members recorded here carry the
/// exclusions that close them against one another, so the merge can spell them in the name's place.
///
/// Nothing is recorded where those exclusions come to nothing — a union of names proves no key an
/// object could be told to leave out — and the merge then names the union as it always did.
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
///
/// A variant standing alone and the same variant written into an object being merged are two
/// spellings of one wire. serde writes a data-carrying variant as the single-key object its name
/// tags, which is what the union already publishes; it writes a unit variant as that name alone
/// standing on its own, and as that name holding `null` where the enum is flattened — a key set,
/// where the union publishes a bare string no object joins. So the flatten-edge spelling is recorded
/// beside the union's rather than read back off it.
///
/// Recorded for every externally tagged enum, and read by each surface wherever the enum is
/// flattened directly. A Zod intersection recognizes exactly the keys its operands name, so it takes
/// one operand per variant; TypeScript distributes over a union of objects on its own and takes the
/// variants for what the union alone cannot say — the key set serde writes for a unit variant, and
/// each variant's silence about the keys its siblings tag.
#[cfg(all(feature = "serde", any(feature = "typescript", feature = "zod")))]
pub fn record_flatten_variants(rust_ident: &str, variants: &[FlattenVariant]) {
    ALIAS_INFO.with(|map| {
        if let Some(info) = map.borrow_mut().get_mut(rust_ident) {
            info.flatten_variants = variants.to_vec();
        }
    });
}

/// Records which of the two Zod bindings a name publishes.
///
/// A reference reads this back rather than deciding for itself. The decision belongs to the named
/// item — it turns on whether that item declares type parameters, nothing a reference can see — and
/// is taken in one place, before the item is dispatched to its shape, so the binding an item
/// publishes and every reference written to it move together whatever the reference was spelled
/// with.
///
/// Held apart from the item's registry entry rather than on it, because a *self*-reference is
/// written while the item's own expansion is still running: the entry is created as the item
/// registers and its fields are rendered from it, so an answer stored on the entry would be read
/// before the item that owns it had put one there — and the `false` a fresh entry carries reads as
/// "publishes a `const`", which for a generic item names a binding nothing declares. Recorded here
/// the answer stands from before the first field is rendered until after the last.
///
/// A name never recorded answers `false`, which is the right reading for every item that declares
/// no parameters and for every name this expansion has not seen.
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
///
/// This is a comparison key, not the text `$SchemaDefault` emits: the fold in
/// [`default_zod_rendering`] renders a downstream reference's own arguments the same plain way, so
/// the two must agree in form for the comparison to ever succeed. Recorded once as the item's
/// `$SchemaDefault` is built, so a later item whose own declared default names this one at the
/// identical arguments can read them back and fold onto this binding instead of composing a second
/// call the memo cache does not share with it — two separately written `z.string()` calls are two
/// different objects, and the cache keys on argument identity, not on what the argument says.
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
///
/// The `regex` crate reads the three as Unicode classes and a flagless JavaScript literal reads
/// the narrower ASCII ones, so the reading the two share is the ASCII one, spelled out. For `\s`
/// the two Unicode sets are not even nested — the `regex` crate spaces U+0085 and JavaScript does
/// not, JavaScript spaces U+FEFF and the `regex` crate does not — which leaves the ASCII run as
/// the only common ground rather than merely the safe one.
///
/// Returned as the bare body and the bracketed class, because where the construct sits decides
/// which is written: a class nested inside a class is a construct the guard refuses, so a `\d`
/// inside one contributes its members and not another `[...]`.
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
///
/// A type written through a `$t:ty` metavariable reaches the expansion wrapped in a
/// `None`-delimited group — rustc's way of keeping the substituted type one unit whatever the
/// expansion writes around it — which `syn` parses as [`Type::Group`]. The grouping is all it is:
/// the type inside is the one the author wrote, at the spans they wrote it at, and it renders the
/// same source text. What differs is the shape, and every reader below classifies by shape, so a
/// substituted `String` handed over ungrouped is a type none of their arms answer for — it lands
/// on the opaque value, silently, and the field comes to admit anything.
///
/// Substitutions nest — a metavariable passed on through a second `macro_rules!` arrives grouped
/// twice — so the grouping is read all the way through rather than one layer off. A parenthesised
/// type is left alone: `(String)` is a grouping the author wrote, and `syn` keeps it as
/// [`Type::Paren`] precisely so it can be told from this one.
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

/// The spelling every reference to a `#[model_schema()]` item falls back to when the registry
/// cannot answer for it, which is what a reference standing *before* the item expanded has and
/// nothing else — the ident with the `Json` suffix taken off, that being what the field walk
/// records for a sibling and what [`ident_schema_module_name`] names the module from.
///
/// An item exported under this spelling already answers at it. One exported under any other — an
/// alias, which is given the `Type` suffix, or anything carrying a `name = "…"` override — does
/// not, so it publishes the ident as a name of its own on each nominal surface: the two
/// declaration orders then spell the reference differently, and both spellings are defined by the
/// same emission. The module seam settled the same question for the JSON surface, which addresses
/// a Rust path rather than a name.
#[cfg(any(feature = "typescript", feature = "zod"))]
fn ident_reexport_name(rust_ident: &str, export_name: &str) -> Option<String> {
    let referenced = safe_type_name(rust_ident);
    (referenced != export_name).then_some(referenced)
}

/// The names an item's own declaration binds as type parameters.
///
/// This is the whole of what separates a name the expansion cannot resolve *because it is a
/// parameter* from one it cannot resolve because the type lives elsewhere: the first is in scope
/// at the declaration and names no type any emitted output can reference, the second names a type
/// that publishes its own schema module. Every surface that has to draw that line draws it here —
/// see [`crate::field_type::FieldDef::erase_type_parameters`] for what each of them then does with
/// it.
///
/// Lifetimes and const parameters name no type a field position can be written out of, so they are
/// left out. That is also why an emitted `impl` block is spelled from `split_for_impl` instead of
/// from this list: the block has to carry every parameter the declaration binds, lifetimes and
/// consts included, while only these can reach a schema.
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
///
/// Spelled once for the alias, the struct and every enum shape, so the three cannot come to write
/// the same declaration differently. The names are written as declared, which is what a field
/// typed with one already renders as: the declaration and the fields it binds have to spell a
/// parameter the same way or the type does not close.
///
/// A lifetime and a const parameter never reach here — they name no type `TypeScript` has a
/// declaration slot for — so `struct Label<'a>` publishes a plain `export type Label`.
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
///
/// `binding_suffix` is what the item's own binding is named with, and the two names have to agree
/// on it: a generic type publishes a factory rather than a schema, so both spellings end in
/// `$SchemaFactory` there and in `$Schema` everywhere else.
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
///
/// The counterpart of [`zod_factory_argument`] on the surface that writes Rust rather than
/// JavaScript, and named off the parameter for the same reason: the argument reads as the parameter
/// it fills while staying a name of its own beside it. Snake case because that is what a Rust local
/// is spelled in, and underscore-led because a declared parameter need not reach the document at
/// all — the item is not owed a warning for one the wire does not carry.
#[cfg(feature = "jsonschema")]
pub fn json_argument_binding(parameter: &str) -> String {
    format!("_arg_{}", to_snake_case(parameter))
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

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
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

/// An enum's doc lines. Reachable in every schema build, not just the two that publish prose: the
/// `schema_example()` an item's ` ```rust example ` block earns is read off these too.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
pub fn get_enum_docs(item_enum: &ItemEnum) -> Option<Vec<String>> {
    collect_doc_lines(&item_enum.attrs)
}

pub fn get_variant_docs(variant: &Variant) -> Option<Vec<String>> {
    collect_doc_lines(&variant.attrs)
}

pub fn get_field_docs(field: &Field) -> Option<Vec<String>> {
    collect_doc_lines(&field.attrs)
}

/// The doc lines of anything carrying attributes. Read by the `TypeScript` surface for the prose it
/// publishes, and by the guard that answers for a ` ```rust example ` block written on an item no
/// annotation can be spelled for.
#[cfg(any(feature = "typescript", feature = "zod"))]
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
/// Where a construct has a spelling both grammars read alike, it is rewritten instead of refused,
/// and the rewrite is what every surface receives — the Rust validator included, which is what
/// keeps the three from validating different sets. There are two: `(?P<name>...)` becomes
/// `(?<name>...)`, one group under two spellings; and `\d`, `\w` and `\s` become the members they
/// stand for, the `regex` crate reading the three as Unicode classes where a flagless literal
/// reads the narrower ASCII ones. The first changes nothing about what the pattern matches. The
/// second narrows the Rust side on purpose, to the set the JavaScript side was going to enforce
/// regardless.
///
/// What has no such spelling is refused with the construct named. That covers `.` and the negated
/// `\D`, `\W` and `\S`: a flagless literal matches one UTF-16 code unit where the `regex` crate
/// matches one character, so writing the members out settles which characters are named and never
/// how many code units one of them is. A `]` opening a character class looks like a fixable
/// spelling and is not — `[]-a]` is three members and `[\]-a]` is a range — so it too is refused
/// rather than escaped.
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
///
/// A `pattern` that matches at some position of every string is a constraint that constrains
/// nothing. Nothing downstream can make it say anything: the generated validator would reject no
/// value, and the Zod and JSON Schema surfaces would publish a check every payload passes. Taking
/// it silently leaves the author holding a contract that says the value is checked when nothing
/// checks it -- the same claim a bound written where no surface reads one is refused for, so it is
/// refused the same way, where it is written.
///
/// It also settles what such a pattern would have been emitted as. The `str` call
/// [`trivial_pattern`] names for a simple pattern does not exist here -- there is no call for "and
/// then check nothing" -- and dropping the check from a validator whose only constraint is that
/// pattern leaves `value` unread in the emitted `pub fn validate_..._value(value: &str)`, which is
/// a fresh `-D warnings` failure in the consumer crate the validator is written into. Refusing at
/// expansion means no such validator is ever emitted.
///
/// What stays on the regex path is every pattern that turns some value away, `\b` included. That
/// one is the residual: `clippy::trivial_regex` flags it and names no replacement -- a probe of
/// `#[model_schema_prop(pattern = r"\b")]` under `cargo clippy --all-targets -- -D warnings` in a
/// crate denying `clippy::nursery` reports `trivial regex ... the regex is unlikely to be useful
/// as it is` against the `#[model_schema]` attribute -- and it is left standing, because `\b` is a
/// real constraint: the empty string holds no word boundary, so a value is turned away by it, and
/// refusing it would drop a check the author is owed.
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
///
/// The rule is one sentence with one exception. A pattern admits every value when nothing in it
/// asks the haystack for anything -- when it matches the empty string wherever it is tried
/// ([`matches_at_every_position`]) -- and the exception is that a single whole-text anchor asks
/// for nothing either, since every haystack has a start and an end. Two of them do ask: `^$` pins
/// both to one position, which only the empty string has.
///
/// It errs toward `false` everywhere the reading is not certain, and everything it declines keeps
/// its `regex::Regex`, where being conservative costs nothing. `^^` and `(?:^)+` are both admit-
/// everything shapes it does not classify, and a pattern the parser cannot read is not classified
/// at all -- that one is [`portable_pattern`]'s refusal to make, ahead of this one.
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
///
/// A repetition that may run zero times is such a match wherever it is tried, whatever it repeats;
/// an alternation is one as soon as any single branch is. A literal or a class consumes a
/// character, and a look-around holds only where the haystack has the property it names -- `^` at
/// the start, `\b` at a word boundary — so neither asks for nothing.
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
///
/// The lint is right, and it is the consumer who pays for being wrong: the `Regex::new` it fires
/// on is written by this crate into the consumer's crate, so the diagnostic lands on their
/// `#[model_schema]` attribute with no edit available at that site. Answering it means emitting
/// the call it asks for, and the emitted call has to accept and reject the same values the regex
/// did, byte for byte, or a pattern would mean one thing in a consumer denying the lint and
/// another in one that does not.
///
/// So the classification is clippy's own, read off `clippy_lints/src/regex.rs::is_trivial_regex`
/// (rust-lang/rust-clippy) and mirrored arm for arm: a bare literal, and a concatenation whose
/// only non-literal parts are a leading `^`, a trailing `$`, or both. It is deliberately no wider
/// than that. A shape the lint does not name is left on the regex path, where it costs nothing;
/// a shape misread as trivial would silently change which values a constraint admits.
///
/// Two of the shapes the lint calls trivial and offers no replacement for -- a pattern that
/// matches everything (`""`, `^`, `$`), and one whose alternatives are all empty (`|`) -- never
/// reach here at all: [`constraining_pattern`] refuses them where they are written, since there is
/// no call to emit for them, only the absence of a check. The third, a bare `\b`, does reach here
/// and keeps its regex, because it turns a value away and so has a check worth keeping.
///
/// The HIR walked is the one the lint reads, parsed with the options it parses with, so what is
/// classified here is what it classifies. Anchors are the whole-haystack `Look::Start` and
/// `Look::End` alone -- the line anchors `(?m)` turns those into never reach this crate, since
/// [`portable_pattern`] refuses an inline flag before a pattern is ever emitted.
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
