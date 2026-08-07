use serde::{Deserialize, Serialize};
#[cfg(any(feature = "jsonschema", feature = "zod"))]
use std::collections::HashMap;
use tixschema::model_schema;

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct BasePart {
    owner: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct DataElementSampleValueEntry {
    data_element_id: String,
    #[serde(flatten)]
    variant: DataElementSampleValueVariant,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "dataType")]
enum DataElementSampleValueVariant {
    Alphanumeric {
        #[serde(rename = "sampleValues")]
        sample_values: Vec<String>,
    },
    Logical {
        #[serde(rename = "sampleValues")]
        sample_values: Vec<bool>,
    },
    Numeric {
        #[serde(rename = "sampleValues")]
        sample_values: Vec<i64>,
    },
}

/// Two bases that flatten each other: neither body exists when the other's merge asks for it, and
/// no finite value inhabits either type.
#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct CycleFirst {
    first_own: String,
    #[serde(flatten)]
    second: CycleSecond,
}

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct CycleSecond {
    #[serde(flatten)]
    first: Box<CycleFirst>,
    second_own: String,
}

/// The same cycle with a union in the middle: the branch that closes it is the deferred name, and
/// a reference carries none of the members it stands for.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct CycleUnionNode {
    #[serde(flatten)]
    either: CycleUnionEither,
    own: String,
}

#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum CycleUnionEither {
    Only(Box<CycleUnionNode>),
}

/// A cycle spanning both places an intersection operand is written: a struct's `#[serde(flatten)]`
/// base on one side, an internally tagged newtype variant's content on the other. Each names the
/// other's `const`, so neither declaration order puts both above the other.
#[cfg(feature = "zod")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct CycleVariantContent {
    #[serde(flatten)]
    back: CycleVariantHost,
    own: String,
}

#[cfg(feature = "zod")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type")]
enum CycleVariantHost {
    Wrapped(Box<CycleVariantContent>),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct ExtraPart {
    priority: i64,
}

/// A base that names itself, written only where something asks what it describes as: what a
/// flattened branch that is a reference rather than an object contributes to its container.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatHolder {
    #[serde(flatten)]
    base: FlatNode,
    extra: String,
}

#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatNode {
    children: Vec<Self>,
    val: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlattenOnly {
    #[serde(flatten)]
    variant: DataElementSampleValueVariant,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct MultiFlatten {
    #[serde(flatten)]
    base: BasePart,
    #[serde(flatten)]
    extra: ExtraPart,
    id: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NoFlatten {
    id: String,
    name: String,
}

/// A plain enum flattened into a struct. Declared without `#[model_schema()]`: a plain enum writes
/// its own variant name rather than an object, so the crate refuses the declaration and the wire
/// form is only readable from a plain serde type.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum FlatHue {
    Red,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverEnum {
    own: String,
    #[serde(flatten)]
    tone: FlatHue,
}

/// A newtype over a `String` flattened into a struct: serde writes it as a string, not an object,
/// so the Zod merge has nothing to join keys to and refuses the declaration where it was written.
#[cfg(not(feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatSlug(String);

#[cfg(not(feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverBrand {
    own: String,
    #[serde(flatten)]
    slug: FlatSlug,
}

/// The same newtype declared *below* the object that flattens it — the recording holds only what
/// has already expanded, so the guard keeps the declaration-order fallback while the JSON-schema
/// merge (reading the document at run time) refuses it regardless of order.
#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverLaterBrand {
    own: String,
    #[serde(flatten)]
    slug: LaterFlatSlug,
}

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct LaterFlatSlug(String);

/// An untagged enum every member of which serde writes as an object, and the struct that flattens
/// it. serde writes whichever member matched into the object the struct is writing, so what the
/// struct writes is one key set per member.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatFirst {
    a: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatSecond {
    b: bool,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum FlatEither {
    First(FlatFirst),
    Second(FlatSecond),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverUntagged {
    #[serde(flatten)]
    either: FlatEither,
    own: String,
}

/// The same union reached through an `Option`, so the object writes one of the members' key sets
/// beside its own or writes its own alone: two choices, multiplied.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverOptionalUntagged {
    #[serde(flatten, skip_serializing_if = "Option::is_none", default)]
    either: Option<FlatEither>,
    own: String,
}

/// The same union under an alias. An alias *is* the type it names, so the object that flattens it
/// writes exactly what the object flattening the enum writes.
#[cfg(feature = "zod")]
#[model_schema()]
type FlatEitherAlias = FlatEither;

#[cfg(feature = "zod")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverAliasedUntagged {
    #[serde(flatten)]
    either: FlatEitherAlias,
    own: String,
}

/// An object that flattens a union declared *below* it. Nothing at the merge tells that source
/// apart from a struct declared below — both are names the expansion has not seen — so the merge
/// takes the same fallback a forward reference already takes and names the source as one operand.
#[cfg(feature = "zod")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverLaterUntagged {
    #[serde(flatten)]
    either: LaterEither,
    own: String,
}

#[cfg(feature = "zod")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum LaterEither {
    First(FlatFirst),
    Second(FlatSecond),
}

/// The same shape with one member serde writes as a string rather than an object. Standing on its
/// own it is an ordinary union — an untagged enum may hold a scalar — and every surface describes
/// it.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum FlatScalarEither {
    Obj(FlatFirst),
    Text(String),
}

/// Flattening it is what no value satisfies, so the Zod surface refuses the declaration once the
/// members it multiplies over reveal a scalar. The struct exists only where nothing records those
/// members — where the JSON-schema merge answers instead.
#[cfg(not(feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverScalarUntagged {
    #[serde(flatten)]
    either: FlatScalarEither,
    own: String,
}

/// The same union declared *below* the object that flattens it — carrying no recorded members,
/// the Zod merge falls back to naming it as a plain operand, while the JSON-schema merge
/// (reading members at run time) refuses it regardless of order.
#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverLaterScalarUntagged {
    #[serde(flatten)]
    either: LaterScalarEither,
    own: String,
}

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum LaterScalarEither {
    Obj(FlatFirst),
    Text(String),
}

/// The same shape with one member reached through an `Option`. Standing on its own it is an
/// ordinary union — a member may be written as its value or as a bare `null`, and every surface
/// describes the choice.
#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum FlatNullableEither {
    Base(FlatFirst),
    Maybe(Option<FlatSecond>),
}

/// Flattening it is what neither direction of serde agrees with the other on, so the Zod surface
/// refuses the declaration and the struct exists only where nothing records the members — which is
/// where the JSON-schema merge answers for the same declaration at run time instead.
#[cfg(all(feature = "jsonschema", not(feature = "zod")))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverNullableUntagged {
    #[serde(flatten)]
    either: FlatNullableEither,
    own: String,
}

/// The same union declared *below* the object that flattens it, which every table holds: the
/// recording carries nothing for it, so the Zod merge names it as the one operand it is while the
/// JSON-schema merge refuses it wherever it was declared.
#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverLaterNullableUntagged {
    #[serde(flatten)]
    either: LaterNullableEither,
    own: String,
}

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum LaterNullableEither {
    Base(FlatFirst),
    Maybe(Option<FlatSecond>),
}

/// Three members that name another item rather than spelling a type out: a brand over a string, a
/// brand over a `bool`, and a plain unit enum. What serde writes each as lives on the named item,
/// so the merge asks the registry rather than the spelling.
#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(transparent)]
struct MemberSlug(String);

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(transparent)]
struct MemberSwitch(bool);

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum MemberHue {
    Blue,
    Red,
}

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum MemberSlugEither {
    Base(FlatFirst),
    Slug(MemberSlug),
}

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum MemberSwitchEither {
    Base(FlatFirst),
    Switch(MemberSwitch),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum MemberHueEither {
    Base(FlatFirst),
    Hue(MemberHue),
}

#[cfg(all(feature = "jsonschema", not(feature = "zod")))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverMemberSlugUntagged {
    #[serde(flatten)]
    either: MemberSlugEither,
    own: String,
}

#[cfg(all(feature = "jsonschema", not(feature = "zod")))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverMemberSwitchUntagged {
    #[serde(flatten)]
    either: MemberSwitchEither,
    own: String,
}

#[cfg(not(feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverMemberHueUntagged {
    #[serde(flatten)]
    either: MemberHueEither,
    own: String,
}

/// And the same brand member in a union declared below the object, which keeps the fallback: a
/// name the recording holds nothing for is named as the one operand it is, whatever the registry
/// could have said about the members.
#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverLaterMemberSlugUntagged {
    #[serde(flatten)]
    either: LaterMemberSlugEither,
    own: String,
}

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum LaterMemberSlugEither {
    Base(FlatFirst),
    Slug(MemberSlug),
}

/// Three members naming another item rather than spelling a type out, each a different wire
/// shape: one is a JSON array, one is the object a map writes, one has a nullable published
/// surface. serde flattens only the map and refuses the other two.
#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct MemberBag(Vec<String>);

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct MemberBucket(HashMap<String, String>);

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct MemberMaybeSecond(Option<FlatSecond>);

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum MemberBagEither {
    Base(FlatFirst),
    Items(MemberBag),
}

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum MemberBucketEither {
    Base(FlatFirst),
    Bucket(MemberBucket),
}

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum MemberMaybeEither {
    Base(FlatFirst),
    Maybe(MemberMaybeSecond),
}

#[cfg(all(feature = "jsonschema", not(feature = "zod")))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverMemberBagUntagged {
    #[serde(flatten)]
    either: MemberBagEither,
    own: String,
}

#[cfg(all(feature = "jsonschema", not(feature = "zod")))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverMemberMaybeUntagged {
    #[serde(flatten)]
    either: MemberMaybeEither,
    own: String,
}

/// The map-shaped member is the one of the three that stays where it was: serde writes a map's keys
/// straight into the object being written, which is what flattening is, so the multiplication keeps
/// its branch and the merge keeps its document.
#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverMemberBucketUntagged {
    #[serde(flatten)]
    either: MemberBucketEither,
    own: String,
}

/// The array-shaped and nullable members in unions declared below the object that flattens them,
/// which keeps the fallback: the recording holds nothing for either name, so the Zod merge writes
/// the one operand it is while the JSON-schema merge refuses it wherever it was declared.
#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverLaterMemberBagUntagged {
    #[serde(flatten)]
    either: LaterMemberBagEither,
    own: String,
}

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum LaterMemberBagEither {
    Base(FlatFirst),
    Items(MemberBag),
}

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverLaterMemberMaybeUntagged {
    #[serde(flatten)]
    either: LaterMemberMaybeEither,
    own: String,
}

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum LaterMemberMaybeEither {
    Base(FlatFirst),
    Maybe(MemberMaybeSecond),
}

/// A member naming a tagged enum: serde writes a data-carrying variant as a single-key object but
/// a unit variant as a bare string — a leaf no object can be merged with, one level in from the
/// member.
#[cfg(any(feature = "jsonschema", feature = "zod", feature = "typescript"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum MemberExtBare {
    Bare,
    Wrapped(FlatSecond),
}

#[cfg(all(feature = "jsonschema", not(feature = "zod")))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum MemberExtBareEither {
    Base(FlatFirst),
    Ext(MemberExtBare),
}

#[cfg(all(feature = "jsonschema", not(feature = "zod")))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverMemberExtBareUntagged {
    #[serde(flatten)]
    either: MemberExtBareEither,
    own: String,
}

/// The same enum in a union declared below the object that flattens it, which keeps the fallback:
/// the recording holds nothing for the union's name, so the Zod merge writes the one operand it is
/// while the JSON-schema merge refuses it wherever it was declared.
#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverLaterMemberExtBareUntagged {
    #[serde(flatten)]
    either: LaterMemberExtBareEither,
    own: String,
}

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum LaterMemberExtBareEither {
    Base(FlatFirst),
    Ext(MemberExtBare),
}

/// The same tagged enum flattened directly, with no union between: flattened, serde writes a unit
/// variant as its name holding `null` rather than the bare string it is standing alone, and reads
/// that back as the variant — so the merges owe it one branch per variant.
#[cfg(any(feature = "jsonschema", feature = "zod", feature = "typescript"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct ExtBareDirectHolder {
    #[serde(flatten)]
    ext: MemberExtBare,
    own: String,
}

/// And a tagged enum whose every variant carries data, which serde writes as the object its name
/// tags in every case — admitted where it always was, on every surface.
#[cfg(any(feature = "jsonschema", feature = "zod", feature = "typescript"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum MemberExtObjects {
    One(FlatFirst),
    Two(FlatSecond),
}

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum MemberExtObjEither {
    Base(FlatFirst),
    Ext(MemberExtObjects),
}

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverMemberExtObjUntagged {
    #[serde(flatten)]
    either: MemberExtObjEither,
    own: String,
}

/// That enum flattened directly, where Zod and TypeScript multiply for different reasons: a Zod
/// intersection recognizes only the keys its operands name and a `z.union` names none, so joining
/// admits nothing; TypeScript distributes over every branch instead.
#[cfg(any(feature = "jsonschema", feature = "zod", feature = "typescript"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct ExtObjDirectHolder {
    #[serde(flatten)]
    ext: MemberExtObjects,
    own: String,
}

/// The same enum through an `Option`: `keyof` a union is the keys its branches share, which for
/// two differently-tagged variants is none — an empty mapped type every object passes through —
/// unless each variant is closed against the other.
#[cfg(feature = "typescript")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct OptExtObjHolder {
    #[serde(flatten, skip_serializing_if = "Option::is_none", default)]
    ext: Option<MemberExtObjects>,
    own: String,
}

/// An untagged union written over the objects its members are, rather than over names — the only
/// case where the declaration itself spells each member's keys, since serde matches a member by
/// shape.
#[cfg(any(feature = "jsonschema", feature = "zod", feature = "typescript"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum InlineUntagEither {
    Left { left: String },
    Right { right: bool },
}

#[cfg(any(feature = "jsonschema", feature = "zod", feature = "typescript"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct InlineUntagDirectHolder {
    #[serde(flatten)]
    either: InlineUntagEither,
    own: String,
}

/// A union member that names itself, and the struct that flattens the union. The member describes
/// as a reference into the definitions, and the body it points at is written by the time the merge
/// asks for it.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatSelfNode {
    kids: Vec<Self>,
    leaf: String,
}

#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum FlatSelfEither {
    Node(FlatSelfNode),
}

#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverSelfUntagged {
    #[serde(flatten)]
    either: FlatSelfEither,
    own: String,
}

/// An untagged enum whose members overlap: one member's key set is a subset of the other's, and
/// the difference is a key that member omits when it is absent. serde writes the first member that
/// matches, so the payload it writes for the narrower member is one the wider member admits too.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct OverlapNarrow {
    a: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct OverlapWide {
    a: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    b: Option<String>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum OverlapEither {
    Narrow(OverlapNarrow),
    Wide(OverlapWide),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverOverlap {
    #[serde(flatten)]
    either: OverlapEither,
    own: String,
}

/// One object that merges both spellings of a union: a discriminated enum, whose members are
/// exclusive, and the overlapping untagged one, whose members are not.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind")]
enum MixedTagged {
    Left { left: String },
    Right { right: bool },
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverMixed {
    #[serde(flatten)]
    either: OverlapEither,
    own: String,
    #[serde(flatten)]
    tagged: MixedTagged,
}

/// A union one member of which is itself a union, and the struct that flattens the outer one. serde
/// writes whichever leaf member matched into the object the struct is writing, so the nesting
/// contributes no key of its own and what the struct writes is one key set per leaf.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NestPlain {
    plain: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind")]
enum NestTagged {
    Left { left: String },
    Right { right: bool },
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum NestEither {
    Plain(NestPlain),
    Tagged(NestTagged),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NestHolder {
    #[serde(flatten)]
    either: NestEither,
    own: String,
}

/// The same nesting with one member at every level: a choice of one is no choice, so no wrapper is
/// written and the holder describes as the one key set it writes.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NestOnly {
    a: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum NestOnlyInner {
    A(NestOnly),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum NestOnlyOuter {
    Inner(NestOnlyInner),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NestOnlyHolder {
    #[serde(flatten)]
    either: NestOnlyOuter,
    own: String,
}

/// Two unions that name each other. The outer one is deferred by the frame that reads it and its
/// body is written by the time the merge asks for it, so the cycle carries no missing body — it is
/// visible only as a name the expansion reaches twice on one path.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NestCycleHolder {
    #[serde(flatten)]
    either: NestCycleOuter,
    own: String,
}

#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum NestCycleOuter {
    Inner(Box<NestCycleInner>),
}

#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum NestCycleInner {
    Back(Box<NestCycleOuter>),
}

/// A base reached through an `Option`: serde writes the base's members beside the object's own
/// when `Some`, and the object alone when `None` — the guard forces `skip_serializing_if`, so
/// absence is the only `None` the crate admits. Two required members, so a payload carrying only
/// one is a base serde never writes.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct OptBase {
    left: String,
    right: bool,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct OptHolder {
    #[serde(flatten, skip_serializing_if = "Option::is_none", default)]
    maybe: Option<OptBase>,
    own: String,
}

/// The other spelling of the same absence: a nullable-surfaced registration flattened directly, no
/// union between. serde reads and writes both forms exactly as the `Option`-typed field does, so
/// the merge owes both spellings one answer.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct MaybeOptBase(Option<OptBase>);

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NamedMaybeHolder {
    #[serde(flatten)]
    maybe: MaybeOptBase,
    own: String,
}

/// The same two spellings over a scalar rather than an object: serde writes nothing for `None`
/// (reading the object's own keys back as it) but refuses `Some` outright — one branch
/// round-trips, the other satisfies no payload, so the guard refuses the declaration where it was
/// written.
#[cfg(not(feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct MaybeCount(Option<i64>);

#[cfg(not(feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct MaybeCountHolder {
    #[serde(flatten)]
    count: MaybeCount,
    own: String,
}

/// The same registration declared *below* the object that flattens it — the recording holds only
/// what has already expanded, so the guard keeps the declaration-order fallback while the
/// JSON-schema merge (reading the document at run time) refuses it regardless of order.
#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct LaterMaybeCountHolder {
    #[serde(flatten)]
    count: LaterMaybeCount,
    own: String,
}

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct LaterMaybeCount(Option<i64>);

/// And a name whose published surface is not nullable, which offers no absence and stays the one
/// operand it always was.
#[cfg(any(feature = "jsonschema", feature = "zod", feature = "typescript"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct PlainOptBase(OptBase);

#[cfg(any(feature = "jsonschema", feature = "zod", feature = "typescript"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NamedPlainHolder {
    own: String,
    #[serde(flatten)]
    plain: PlainOptBase,
}

/// The same absence over a source that is itself a union: serde writes the matched member's keys or
/// no keys at all, so the choice the `Option` adds sits outside the choice the enum already offered.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct OptUnionHolder {
    #[serde(flatten, skip_serializing_if = "Option::is_none", default)]
    maybe: Option<NestTagged>,
    own: String,
}

/// The source an enum's own struct variants flatten, and one form of the enum per tagging: serde
/// writes the source's members into the variant's own content object, wherever that object sits.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct VariantExtra {
    note: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct VariantRank {
    rank: i64,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum ExternalFlatVariant {
    Named {
        #[serde(flatten)]
        extra: VariantExtra,
        x: String,
    },
    Plain {
        y: String,
    },
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind")]
enum InternalFlatVariant {
    Named {
        #[serde(flatten)]
        extra: VariantExtra,
        x: String,
    },
    Plain {
        y: String,
    },
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind", content = "data")]
enum AdjacentFlatVariant {
    Named {
        #[serde(flatten)]
        extra: VariantExtra,
        x: String,
    },
    Plain {
        y: String,
    },
}

/// Two sources flattened into one variant: both sets of members join the same content object.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum TwiceFlatVariant {
    Named {
        #[serde(flatten)]
        extra: VariantExtra,
        #[serde(flatten)]
        rank: VariantRank,
        x: String,
    },
}

/// The same source at an untagged enum's own member position: no discriminator stands over it, so
/// serde writes the source's members beside the variant's own and nothing else.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum UntaggedFlatVariant {
    Named {
        #[serde(flatten)]
        extra: VariantExtra,
        x: String,
    },
    Plain {
        y: String,
    },
}

/// Two sources flattened into one untagged member.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum TwiceUntaggedFlatVariant {
    Named {
        #[serde(flatten)]
        extra: VariantExtra,
        #[serde(flatten)]
        rank: VariantRank,
        x: String,
    },
}

/// An object that flattens such an enum. A member that flattens does not prove its own key list, so
/// no member of this union is closed against keys the expansion cannot enumerate.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct UntaggedFlatVariantHolder {
    #[serde(flatten)]
    either: UntaggedFlatVariant,
    own: String,
}

/// The branch of an untagged document that names `key`.
#[cfg(feature = "jsonschema")]
fn untagged_branch(document: &serde_json::Value, key: &str) -> serde_json::Value {
    document["anyOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|branch| branch["properties"].as_object().unwrap().contains_key(key))
        .unwrap()
        .clone()
}

/// The content object of `variant` in `document`, whichever of the three taggings put it there.
#[cfg(feature = "jsonschema")]
fn variant_content(document: &serde_json::Value, variant: &str) -> serde_json::Value {
    let branches = document["oneOf"].as_array().unwrap();
    let member = branches
        .iter()
        .find(|branch| {
            let properties = branch["properties"].as_object().unwrap();
            properties.contains_key(variant)
                || properties
                    .get("kind")
                    .is_some_and(|tag| tag["const"] == variant)
        })
        .unwrap();
    let properties = member["properties"].as_object().unwrap();
    for key in [variant, "data"] {
        if let Some(content) = properties.get(key) {
            return content.clone();
        }
    }
    member.clone()
}

/// The keys a content object names, and the keys it requires.
#[cfg(feature = "jsonschema")]
fn described_keys(content: &serde_json::Value) -> (Vec<String>, Vec<String>) {
    let named = content["properties"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    let required = content["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|key| key.as_str().unwrap().to_owned())
        .collect();
    (named, required)
}

/// Whether `payload` is accepted by a document every leaf of which is an object closed by
/// `additionalProperties: false`: a leaf accepts when it names every key the payload carries and
/// requires no key it does not.
#[cfg(feature = "jsonschema")]
fn closed_document_accepts(schema: &serde_json::Value, payload: &serde_json::Value) -> bool {
    if let Some(branches) = schema.get("oneOf") {
        return accepting_branches(branches, payload) == 1;
    }
    if let Some(branches) = schema.get("anyOf") {
        return accepting_branches(branches, payload) >= 1;
    }
    let written = payload.as_object().unwrap();
    let named = schema["properties"].as_object().unwrap();
    let required = schema["required"].as_array().unwrap();
    written.keys().all(|key| named.contains_key(key))
        && required
            .iter()
            .all(|key| written.contains_key(key.as_str().unwrap()))
}

/// How many of a union's branches accept `payload` — what the two spellings disagree about.
#[cfg(feature = "jsonschema")]
fn accepting_branches(branches: &serde_json::Value, payload: &serde_json::Value) -> usize {
    branches
        .as_array()
        .unwrap()
        .iter()
        .filter(|branch| closed_document_accepts(branch, payload))
        .count()
}

#[test]
fn test_flatten_structs_constructible() {
    let base = BasePart {
        owner: String::new(),
    };
    assert!(base.owner.is_empty());
    let extra = ExtraPart { priority: 0 };
    assert_eq!(extra.priority, 0_i64);
    let flatten_only = FlattenOnly {
        variant: DataElementSampleValueVariant::Alphanumeric {
            sample_values: Vec::new(),
        },
    };
    assert!(matches!(
        flatten_only.variant,
        DataElementSampleValueVariant::Alphanumeric { .. }
    ));
    let multi = MultiFlatten {
        base: BasePart {
            owner: String::new(),
        },
        extra: ExtraPart { priority: 0 },
        id: String::new(),
    };
    assert!(multi.id.is_empty());
    let no_flatten = NoFlatten {
        id: String::new(),
        name: String::new(),
    };
    assert!(no_flatten.id.is_empty());
}

#[test]
#[cfg(feature = "typescript")]
fn test_flatten_typescript_intersection() {
    let ts = DataElementSampleValueEntry::ts_definition();
    assert!(ts.contains("export type DataElementSampleValueEntry = {"));
    assert!(ts.contains("dataElementId: string;"));
    assert!(ts.contains("} & DataElementSampleValueVariant;"));
    assert!(!ts.contains("variant:"));
}

#[test]
#[cfg(feature = "typescript")]
fn test_flatten_typescript_multiple() {
    let ts = MultiFlatten::ts_definition();
    assert!(ts.contains("id: string;"));
    assert!(ts.contains("} & BasePart & ExtraPart;"));
    assert!(!ts.contains("base:"));
    assert!(!ts.contains("extra:"));
}

#[test]
#[cfg(feature = "typescript")]
fn test_flatten_only_typescript_is_alias() {
    let ts = FlattenOnly::ts_definition();
    assert!(ts.contains("export type FlattenOnly = DataElementSampleValueVariant;"));
    assert!(!ts.contains("Record<string, never>"));
    assert!(!ts.contains("variant:"));
}

#[test]
#[cfg(feature = "typescript")]
fn test_no_flatten_typescript_unchanged() {
    let ts = NoFlatten::ts_definition();
    assert!(ts.contains("export type NoFlatten = {"));
    assert!(ts.contains("id: string;"));
    assert!(ts.contains("name: string;"));
    assert!(!ts.contains(" & "));
}

#[test]
#[cfg(feature = "typescript")]
fn test_flatten_variant_keeps_pascal_discriminator_and_camel_fields() {
    let ts = DataElementSampleValueVariant::ts_definition();
    assert!(ts.contains("dataType: \"Numeric\""));
    assert!(ts.contains("sampleValues:"));
    assert!(!ts.contains("sample_values"));
}

#[test]
#[cfg(feature = "zod")]
fn test_flatten_zod_intersection() {
    let zod = DataElementSampleValueEntry::zod_schema();
    assert!(zod.contains("z.strictObject({"));
    assert!(zod.contains("dataElementId:"));
    assert!(zod.contains("}).and(z.lazy(() => DataElementSampleValueVariant$Schema));"));
    assert!(!zod.contains("variant:"));
}

#[test]
#[cfg(feature = "zod")]
fn test_flatten_zod_multiple_chained() {
    let zod = MultiFlatten::zod_schema();
    assert!(
        zod.contains("}).and(z.lazy(() => BasePart$Schema)).and(z.lazy(() => ExtraPart$Schema));")
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_no_flatten_zod_unchanged() {
    let zod = NoFlatten::zod_schema();
    assert!(zod.contains("z.strictObject({"));
    assert!(!zod.contains(".and("));
}

/// A base's schema is a `const` the emitted module reads, and nothing orders one type's module
/// against another's — naming it straight into the intersection would fail for a base declared
/// below, so the operand is deferred until something validates.
#[test]
#[cfg(feature = "zod")]
fn test_a_flattened_base_is_never_read_while_the_const_initializes() {
    for zod in [
        DataElementSampleValueEntry::zod_schema(),
        MultiFlatten::zod_schema(),
        FlattenOnly::zod_schema(),
    ] {
        for name in ["DataElementSampleValueVariant", "BasePart", "ExtraPart"] {
            assert!(
                !zod.contains(&format!(".and({name}$Schema)")),
                "`{name}$Schema` is read eagerly in: {zod}"
            );
        }
    }
}

/// Two bases that flatten each other name each other's `const`, and no declaration order puts both
/// above the other. Deferring each read is what makes the pair's modules load at all; the cycle is
/// then reached only by asking the schema to validate, never by importing it.
#[test]
#[cfg(feature = "zod")]
fn test_a_flatten_cycle_defers_both_sides_of_the_pair() {
    let first = CycleFirst::zod_schema();
    let second = CycleSecond::zod_schema();

    assert!(
        first.contains("}).and(z.lazy(() => CycleSecond$Schema));"),
        "expected a deferred base, got: {first}"
    );
    assert!(
        second.contains("}).and(z.lazy(() => CycleFirst$Schema));"),
        "expected a deferred base, got: {second}"
    );
    assert!(
        !first.contains(".and(CycleSecond$Schema)"),
        "`CycleSecond$Schema` is read eagerly in: {first}"
    );
    assert!(
        !second.contains(".and(CycleFirst$Schema)"),
        "`CycleFirst$Schema` is read eagerly in: {second}"
    );
}

/// An intersection operand is written in two places — a flattened base and an internally tagged
/// variant's content — and a cycle can run through both, so both sides carry the same deferral
/// regardless of module assembly order.
#[test]
#[cfg(feature = "zod")]
fn test_a_flatten_cycle_through_a_variants_content_defers_both_sides() {
    let content = CycleVariantContent::zod_schema();
    let host = CycleVariantHost::zod_schema();

    assert!(
        content.contains("}).and(z.lazy(() => CycleVariantHost$Schema));"),
        "expected a deferred base, got: {content}"
    );
    assert!(
        host.contains("}).and(z.lazy(() => CycleVariantContent$Schema))"),
        "expected a deferred content, got: {host}"
    );
    assert!(
        !content.contains(".and(CycleVariantHost$Schema)"),
        "`CycleVariantHost$Schema` is read eagerly in: {content}"
    );
    assert!(
        !host.contains(".and(CycleVariantContent$Schema)"),
        "`CycleVariantContent$Schema` is read eagerly in: {host}"
    );
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_flatten_json_schema_distributes_base_into_variants() {
    let schema = DataElementSampleValueEntry::json_schema();
    let one_of = schema["oneOf"].as_array().unwrap();
    assert_eq!(one_of.len(), 3);
    for branch in one_of {
        assert_eq!(
            branch["additionalProperties"],
            serde_json::Value::Bool(false)
        );
        let props = branch["properties"].as_object().unwrap();
        assert!(props.contains_key("dataElementId"));
        assert!(props.contains_key("dataType"));
        assert!(props.contains_key("sampleValues"));
        let req = branch["required"].as_array().unwrap();
        assert!(req.iter().any(|v| v.as_str() == Some("dataElementId")));
        assert!(req.iter().any(|v| v.as_str() == Some("dataType")));
        assert!(req.iter().any(|v| v.as_str() == Some("sampleValues")));
    }
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_multi_flatten_json_schema_merges_plain_structs() {
    let schema = MultiFlatten::json_schema();
    assert!(schema.get("oneOf").is_none());
    assert_eq!(
        schema["additionalProperties"],
        serde_json::Value::Bool(false)
    );
    let props = schema["properties"].as_object().unwrap();
    assert!(props.contains_key("id"));
    assert!(props.contains_key("owner"));
    assert!(props.contains_key("priority"));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_flatten_only_json_schema_is_union() {
    let schema = FlattenOnly::json_schema();
    let one_of = schema["oneOf"].as_array().unwrap();
    assert_eq!(one_of.len(), 3);
    for branch in one_of {
        let props = branch["properties"].as_object().unwrap();
        assert!(props.contains_key("dataType"));
        assert!(props.contains_key("sampleValues"));
        assert!(!props.contains_key("dataElementId"));
    }
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_no_flatten_json_schema_closes_additional_properties() {
    let schema = NoFlatten::json_schema();
    assert_eq!(
        schema["additionalProperties"],
        serde_json::Value::Bool(false)
    );
    assert!(schema.get("oneOf").is_none());
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_flatten_recursive_base_contributes_its_fields() {
    let schema = FlatHolder::json_schema();
    assert!(schema.get("oneOf").is_none());
    assert_eq!(
        schema["additionalProperties"],
        serde_json::Value::Bool(false)
    );
    let props = schema["properties"].as_object().unwrap();
    assert!(props.contains_key("children"));
    assert!(props.contains_key("val"));
    assert!(props.contains_key("extra"));
    let req = schema["required"].as_array().unwrap();
    for name in ["children", "val", "extra"] {
        assert!(
            req.iter().any(|v| v.as_str() == Some(name)),
            "{name} missing from {req:?}"
        );
    }
}

/// The base's own self-reference is written from the container's root, so it has to resolve there.
#[test]
#[cfg(feature = "jsonschema")]
fn test_flatten_recursive_base_self_reference_resolves_from_the_container() {
    let schema = FlatHolder::json_schema();
    let reference = schema["properties"]["children"]["items"]["$ref"]
        .as_str()
        .unwrap();
    let pointer = reference.strip_prefix('#').unwrap();
    let resolved = schema.pointer(pointer).unwrap();
    let resolved_props = resolved["properties"].as_object().unwrap();
    assert!(resolved_props.contains_key("children"));
    assert!(resolved_props.contains_key("val"));
}

/// A cycle closed through flatten edges has no body to merge at either end, so it is named rather
/// than described as the closed object over whatever fields happened to be written first.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`CycleSecond`: `#[serde(flatten)]` of `CycleFirst` closes a flatten cycle"
)]
fn test_flatten_cycle_is_rejected_rather_than_described() {
    assert!(CycleFirst::json_schema().is_object());
}

/// The rejection is the cycle's, not the entry point's: asking either end names the edge that
/// closes it.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`CycleFirst`: `#[serde(flatten)]` of `CycleSecond` closes a flatten cycle"
)]
fn test_flatten_cycle_is_rejected_from_either_end() {
    assert!(CycleSecond::json_schema().is_object());
}

/// A cycle that closes through one member of a union is the same cycle — there is no body to merge
/// at the branch either — so the branch is named rather than merged as the reference it is, which
/// contributes nothing and closes the document around the base alone.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`CycleUnionNode`: `#[serde(flatten)]` of `CycleUnionEither` closes a flatten cycle through a union member — its branch 1 is `CycleUnionNode`"
)]
fn test_a_flatten_cycle_through_a_union_member_is_rejected() {
    assert!(CycleUnionNode::json_schema().is_object());
}

/// The remedy is the one a cycle closed through the value itself names.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "write the field as a named member so the cycle defers through a reference"
)]
fn test_the_union_member_cycle_refusal_names_the_remedy() {
    assert!(CycleUnionNode::json_schema().is_object());
}

/// Flattening a base that does not name itself writes the document it wrote before, byte for byte.
#[test]
#[cfg(feature = "jsonschema")]
fn test_non_recursive_flatten_documents_are_byte_identical() {
    assert_eq!(
        serde_json::to_string(&DataElementSampleValueEntry::json_schema()).unwrap(),
        r#"{"type":"object","oneOf":[{"type":"object","properties":{"dataElementId":{"type":"string"},"dataType":{"type":"string","const":"Alphanumeric"},"sampleValues":{"type":"array","items":{"type":"string"}}},"required":["dataElementId","dataType","sampleValues"],"additionalProperties":false},{"type":"object","properties":{"dataElementId":{"type":"string"},"dataType":{"type":"string","const":"Logical"},"sampleValues":{"type":"array","items":{"type":"boolean"}}},"required":["dataElementId","dataType","sampleValues"],"additionalProperties":false},{"type":"object","properties":{"dataElementId":{"type":"string"},"dataType":{"type":"string","const":"Numeric"},"sampleValues":{"type":"array","items":{"type":"integer"}}},"required":["dataElementId","dataType","sampleValues"],"additionalProperties":false}]}"#
    );
    assert_eq!(
        serde_json::to_string(&MultiFlatten::json_schema()).unwrap(),
        r#"{"type":"object","properties":{"id":{"type":"string"},"owner":{"type":"string"},"priority":{"type":"integer"}},"required":["id","owner","priority"],"additionalProperties":false}"#
    );
    assert_eq!(
        serde_json::to_string(&FlattenOnly::json_schema()).unwrap(),
        r#"{"type":"object","oneOf":[{"type":"object","properties":{"dataType":{"type":"string","const":"Alphanumeric"},"sampleValues":{"type":"array","items":{"type":"string"}}},"required":["dataType","sampleValues"],"additionalProperties":false},{"type":"object","properties":{"dataType":{"type":"string","const":"Logical"},"sampleValues":{"type":"array","items":{"type":"boolean"}}},"required":["dataType","sampleValues"],"additionalProperties":false},{"type":"object","properties":{"dataType":{"type":"string","const":"Numeric"},"sampleValues":{"type":"array","items":{"type":"integer"}}},"required":["dataType","sampleValues"],"additionalProperties":false}]}"#
    );
    assert_eq!(
        serde_json::to_string(&NoFlatten::json_schema()).unwrap(),
        r#"{"type":"object","additionalProperties":false,"properties":{"id":{"type":"string"},"name":{"type":"string"}},"required":["id","name"]}"#
    );
}

/// And flattening a base that names itself with no union in the middle writes the document it wrote
/// before, byte for byte: the whole-body path reads the deferred name as it always did.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_deferred_flatten_document_is_byte_identical() {
    assert_eq!(
        serde_json::to_string(&FlatHolder::json_schema()).unwrap(),
        r##"{"$defs":{"FlatNode":{"type":"object","additionalProperties":false,"properties":{"children":{"type":"array","items":{"$ref":"#/$defs/FlatNode"}},"val":{"type":"string"}},"required":["children","val"]}},"type":"object","properties":{"extra":{"type":"string"},"children":{"type":"array","items":{"$ref":"#/$defs/FlatNode"}},"val":{"type":"string"}},"required":["extra","children","val"],"additionalProperties":false}"##
    );
}

#[test]
fn test_flatten_serialization_is_flat() {
    let entry = DataElementSampleValueEntry {
        data_element_id: "abc".to_owned(),
        variant: DataElementSampleValueVariant::Numeric {
            sample_values: vec![1_i64, 2_i64, 3_i64],
        },
    };
    let json = serde_json::to_value(&entry).unwrap();
    assert_eq!(json["dataElementId"], "abc");
    assert_eq!(json["dataType"], "Numeric");
    assert_eq!(
        json["sampleValues"],
        serde_json::json!([1_i64, 2_i64, 3_i64])
    );
    assert!(json.get("variant").is_none());

    let back: DataElementSampleValueEntry = serde_json::from_value(json).unwrap();
    assert_eq!(back, entry);
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_flatten_recursive_base_serializes_flat() {
    let holder = FlatHolder {
        base: FlatNode {
            children: vec![FlatNode {
                children: Vec::new(),
                val: "leaf".to_owned(),
            }],
            val: "root".to_owned(),
        },
        extra: "x".to_owned(),
    };
    let json = serde_json::to_value(&holder).unwrap();
    assert_eq!(json["val"], "root");
    assert_eq!(json["extra"], "x");
    assert_eq!(json["children"][0]["val"], "leaf");
    assert!(json.get("base").is_none());

    let back: FlatHolder = serde_json::from_value(json).unwrap();
    assert_eq!(back, holder);
}

/// What serde writes for a flattened plain enum: the enum's own variant name, as a key holding
/// null. No schema closed around the struct's remaining fields names that key, which is why the
/// declaration is refused rather than described.
#[test]
fn test_flattening_a_plain_enum_writes_a_key_the_struct_does_not_name() {
    assert_eq!(
        serde_json::to_value(FlatOverEnum {
            own: "o".to_owned(),
            tone: FlatHue::Red,
        })
        .unwrap(),
        serde_json::json!({ "own": "o", "Red": null })
    );
}

/// And what serde writes for a flattened newtype that reaches the wire as a string — nothing. The
/// value never reaches the wire at all, wherever the name was declared.
#[test]
#[cfg(not(feature = "zod"))]
fn test_flattening_a_string_newtype_is_unserializable() {
    let refusal = serde_json::to_value(FlatOverBrand {
        own: "o".to_owned(),
        slug: FlatSlug("s".to_owned()),
    })
    .unwrap_err()
    .to_string();
    assert!(
        refusal.contains("can only flatten structs and maps"),
        "Got: {refusal}"
    );
}

#[test]
#[cfg(any(feature = "jsonschema", feature = "zod"))]
fn test_flattening_a_later_string_newtype_is_unserializable() {
    let refusal = serde_json::to_value(FlatOverLaterBrand {
        own: "o".to_owned(),
        slug: LaterFlatSlug("s".to_owned()),
    })
    .unwrap_err()
    .to_string();
    assert!(
        refusal.contains("can only flatten structs and maps"),
        "Got: {refusal}"
    );
}

/// So the merge refuses it too, rather than closing the object around the fields that are left.
#[test]
#[cfg(all(feature = "jsonschema", not(feature = "zod")))]
#[should_panic(
    expected = "`FlatOverBrand`: `#[serde(flatten)]` of `FlatSlug` is not written as an object"
)]
fn test_flattening_a_string_newtype_is_refused_by_the_merge() {
    assert!(FlatOverBrand::json_schema().is_object());
}

/// And in those same words wherever the newtype was declared, which is the one reading of the
/// declaration that survives the Zod surface refusing it at expansion.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`FlatOverLaterBrand`: `#[serde(flatten)]` of `LaterFlatSlug` is not written as an object"
)]
fn test_flattening_a_later_string_newtype_is_refused_by_the_merge() {
    assert!(FlatOverLaterBrand::json_schema().is_object());
}

/// The remedy the refusal names is one the author can act on.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(expected = "write the field as a named member so the value gets a key of its own")]
fn test_the_flatten_merge_refusal_names_the_remedy() {
    assert!(FlatOverLaterBrand::json_schema().is_object());
}

/// And a source the registry could answer for is refused where it was written instead, in those
/// same words: the guard names the wire the newtype recorded rather than waiting for a document
/// nothing on the Zod surface would ever build.
#[test]
#[cfg(feature = "zod")]
fn test_the_later_string_newtype_keeps_the_declaration_order_fallback() {
    let zod = FlatOverLaterBrand::zod_schema();
    assert!(
        zod.contains("z.lazy(() => LaterFlatSlug$Schema)"),
        "expected the name as the one operand it is, got: {zod}"
    );
}

/// What serde writes for a flattened untagged enum: the struct's own fields, and beside them the
/// members of whichever union member matched. One key set per member, and no key naming the field.
#[test]
fn test_flattening_an_untagged_enum_writes_the_matched_members_keys() {
    assert_eq!(
        serde_json::to_value(FlatOverUntagged {
            own: "o".to_owned(),
            either: FlatEither::First(FlatFirst { a: "x".to_owned() }),
        })
        .unwrap(),
        serde_json::json!({ "own": "o", "a": "x" })
    );
    assert_eq!(
        serde_json::to_value(FlatOverUntagged {
            own: "o".to_owned(),
            either: FlatEither::Second(FlatSecond { b: true }),
        })
        .unwrap(),
        serde_json::json!({ "own": "o", "b": true })
    );
}

/// So the merged schema is the union of the merges: the base multiplied over every member of the
/// union, each branch closed around exactly the keys that member writes, under the spelling the
/// untagged source used.
#[test]
#[cfg(feature = "jsonschema")]
fn test_flattening_an_untagged_enum_multiplies_the_base_over_its_members() {
    assert_eq!(
        serde_json::to_string(&FlatOverUntagged::json_schema()).unwrap(),
        r#"{"type":"object","anyOf":[{"type":"object","properties":{"own":{"type":"string"},"a":{"type":"string"}},"required":["own","a"],"additionalProperties":false},{"type":"object","properties":{"own":{"type":"string"},"b":{"type":"boolean"}},"required":["own","b"],"additionalProperties":false}]}"#
    );
}

/// And every payload serde writes is accepted by it. Before the base multiplied out, the document
/// closed around `own` alone and rejected both.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_untagged_flatten_schema_accepts_every_payload_serde_writes() {
    let schema = FlatOverUntagged::json_schema();
    for payload in [
        serde_json::json!({ "own": "o", "a": "x" }),
        serde_json::json!({ "own": "o", "b": true }),
    ] {
        assert!(
            closed_document_accepts(&schema, &payload),
            "{payload} is rejected by {schema}"
        );
    }
    assert!(
        !closed_document_accepts(
            &serde_json::json!({
                "type": "object",
                "properties": { "own": { "type": "string" } },
                "required": ["own"],
                "additionalProperties": false
            }),
            &serde_json::json!({ "own": "o", "a": "x" })
        ),
        "the document closed around the base alone accepts a key it does not name"
    );
}

/// A union member serde writes as a string is a member serde cannot flatten at all.
#[test]
#[cfg(not(feature = "zod"))]
fn test_flattening_an_untagged_enum_over_a_string_member_is_unserializable() {
    let refusal = serde_json::to_value(FlatOverScalarUntagged {
        own: "o".to_owned(),
        either: FlatScalarEither::Text("t".to_owned()),
    })
    .unwrap_err()
    .to_string();
    assert!(
        refusal.contains("can only flatten structs and maps"),
        "Got: {refusal}"
    );
}

/// The same value written for the union declared below the object, which every table holds: what
/// serde refuses is the declaration, not the spelling any one surface gave it.
#[test]
#[cfg(any(feature = "jsonschema", feature = "zod"))]
fn test_flattening_a_later_untagged_enum_over_a_string_member_is_unserializable() {
    let refusal = serde_json::to_value(FlatOverLaterScalarUntagged {
        own: "o".to_owned(),
        either: LaterScalarEither::Text("t".to_owned()),
    })
    .unwrap_err()
    .to_string();
    assert!(
        refusal.contains("can only flatten structs and maps"),
        "Got: {refusal}"
    );
}

/// So the merge refuses the whole union, naming the branch that cannot join the object.
#[test]
#[cfg(all(feature = "jsonschema", not(feature = "zod")))]
#[should_panic(
    expected = "`FlatOverScalarUntagged`: `#[serde(flatten)]` of `FlatScalarEither` writes a union member that is not an object — its branch 2 describes a `string`"
)]
fn test_a_string_member_of_a_flattened_untagged_enum_is_refused_by_the_merge() {
    assert!(FlatOverScalarUntagged::json_schema().is_object());
}

/// And it refuses it in those same words wherever the union was declared, which is the one reading
/// of the declaration that survives the Zod surface refusing it at expansion.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`FlatOverLaterScalarUntagged`: `#[serde(flatten)]` of `LaterScalarEither` writes a union member that is not an object — its branch 2 describes a `string`"
)]
fn test_a_string_member_of_a_later_flattened_untagged_enum_is_refused_by_the_merge() {
    assert!(FlatOverLaterScalarUntagged::json_schema().is_object());
}

/// An `Option` member is the one shape where serde's two directions describe different payload
/// sets: it writes flattened `None` as the object's own keys alone with no error, then refuses to
/// read those same keys back — no branch a multiplication could write covers both directions.
#[test]
#[cfg(any(feature = "jsonschema", feature = "zod"))]
fn test_an_optional_flattened_union_member_is_written_and_then_not_read_back() {
    let written = serde_json::to_value(FlatOverLaterNullableUntagged {
        own: "o".to_owned(),
        either: LaterNullableEither::Maybe(None),
    })
    .unwrap();
    assert_eq!(written, serde_json::json!({ "own": "o" }));
    let refusal = serde_json::from_value::<FlatOverLaterNullableUntagged>(written)
        .unwrap_err()
        .to_string();
    assert!(
        refusal.contains("did not match any variant"),
        "Got: {refusal}"
    );

    let present = serde_json::to_value(FlatOverLaterNullableUntagged {
        own: "o".to_owned(),
        either: LaterNullableEither::Maybe(Some(FlatSecond { b: true })),
    })
    .unwrap();
    assert_eq!(present, serde_json::json!({ "own": "o", "b": true }));
    serde_json::from_value::<FlatOverLaterNullableUntagged>(present).unwrap();
}

/// So the merge refuses the whole union, naming the null the absence is described as and the leaf
/// it sits at: an `Option` is a choice of its own below the member, so the absence is `2.2` rather
/// than `2`.
#[test]
#[cfg(all(feature = "jsonschema", not(feature = "zod")))]
#[should_panic(
    expected = "`FlatOverNullableUntagged`: `#[serde(flatten)]` of `FlatNullableEither` writes a union member that is not an object — its branch 2.2 describes a `null`"
)]
fn test_an_optional_member_of_a_flattened_untagged_enum_is_refused_by_the_merge() {
    assert!(FlatOverNullableUntagged::json_schema().is_object());
}

/// And in those same words wherever the union was declared, which is the one reading of the
/// declaration that survives the Zod surface refusing it at expansion.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`FlatOverLaterNullableUntagged`: `#[serde(flatten)]` of `LaterNullableEither` writes a union member that is not an object — its branch 2.2 describes a `null`"
)]
fn test_an_optional_member_of_a_later_flattened_untagged_enum_is_refused_by_the_merge() {
    assert!(FlatOverLaterNullableUntagged::json_schema().is_object());
}

/// A member that names a brand over a string is one serde refuses to flatten for the reason it
/// refuses a directly flattened brand: what it writes is the bare string the brand's inner writes.
#[test]
#[cfg(any(feature = "jsonschema", feature = "zod"))]
fn test_flattening_an_untagged_enum_over_a_named_string_member_is_unserializable() {
    let refusal = serde_json::to_value(FlatOverLaterMemberSlugUntagged {
        own: "o".to_owned(),
        either: LaterMemberSlugEither::Slug(MemberSlug("s".to_owned())),
    })
    .unwrap_err()
    .to_string();
    assert!(
        refusal.contains("can only flatten structs and maps"),
        "Got: {refusal}"
    );
}

/// So the merge refuses it, naming the branch and the string the brand publishes as — the answer
/// the registry holds for the name, which the spelling of the member does not carry.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`FlatOverLaterMemberSlugUntagged`: `#[serde(flatten)]` of `LaterMemberSlugEither` writes a union member that is not an object — its branch 2 describes a `string`"
)]
fn test_a_named_string_member_of_a_flattened_untagged_enum_is_refused_by_the_merge() {
    assert!(FlatOverLaterMemberSlugUntagged::json_schema().is_object());
}

/// In the same words wherever the union was declared, which is the reading that survives the Zod
/// surface refusing the declared-above one at expansion.
#[test]
#[cfg(all(feature = "jsonschema", not(feature = "zod")))]
#[should_panic(
    expected = "`FlatOverMemberSlugUntagged`: `#[serde(flatten)]` of `MemberSlugEither` writes a union member that is not an object — its branch 2 describes a `string`"
)]
fn test_a_named_string_member_of_an_earlier_untagged_enum_is_refused_by_the_merge() {
    assert!(FlatOverMemberSlugUntagged::json_schema().is_object());
}

/// The same for a brand serde stringifies and for a plain unit enum, each named by the keyword its
/// own published document carries.
#[test]
#[cfg(all(feature = "jsonschema", not(feature = "zod")))]
#[should_panic(
    expected = "`FlatOverMemberSwitchUntagged`: `#[serde(flatten)]` of `MemberSwitchEither` writes a union member that is not an object — its branch 2 describes a `boolean`"
)]
fn test_a_named_boolean_member_of_a_flattened_untagged_enum_is_refused_by_the_merge() {
    assert!(FlatOverMemberSwitchUntagged::json_schema().is_object());
}

#[test]
#[cfg(all(feature = "jsonschema", not(feature = "zod")))]
#[should_panic(
    expected = "`FlatOverMemberHueUntagged`: `#[serde(flatten)]` of `MemberHueEither` writes a union member that is not an object — its branch 2 describes a `string`"
)]
fn test_a_named_enumerated_member_of_a_flattened_untagged_enum_is_refused_by_the_merge() {
    assert!(FlatOverMemberHueUntagged::json_schema().is_object());
}

/// A plain enum is the one of the three serde does not refuse outright — the flatten serializer
/// writes the variant name as a key of its own and reads it back. What refuses it is that both
/// schema surfaces describe it as the string its member name is, and a string joins no object.
#[test]
#[cfg(not(feature = "zod"))]
fn test_a_flattened_plain_enum_member_is_written_as_a_key_no_schema_describes() {
    let holder = FlatOverMemberHueUntagged {
        own: "o".to_owned(),
        either: MemberHueEither::Hue(MemberHue::Red),
    };
    let written = serde_json::to_value(&holder).unwrap();
    assert_eq!(written, serde_json::json!({ "own": "o", "Red": null }));
    assert_eq!(
        serde_json::from_value::<FlatOverMemberHueUntagged>(written).unwrap(),
        holder
    );
    #[cfg(feature = "jsonschema")]
    assert_eq!(
        MemberHue::json_schema(),
        serde_json::json!({ "type": "string", "enum": ["Blue", "Red"] })
    );
}

/// The remedy that refusal names is the same one every flattened non-object gets.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(expected = "write the field as a named member so the value gets a key of its own")]
fn test_the_untagged_branch_refusal_names_the_remedy() {
    assert!(FlatOverLaterScalarUntagged::json_schema().is_object());
}

/// A union member that names itself writes its own keys beside the struct's, the same as any other
/// member: what it describes as says nothing about what it writes.
#[test]
#[cfg(feature = "jsonschema")]
fn test_flattening_a_self_naming_union_member_writes_its_keys() {
    assert_eq!(
        serde_json::to_value(FlatOverSelfUntagged {
            own: "o".to_owned(),
            either: FlatSelfEither::Node(FlatSelfNode {
                kids: Vec::new(),
                leaf: "l".to_owned(),
            }),
        })
        .unwrap(),
        serde_json::json!({ "own": "o", "kids": [], "leaf": "l" })
    );
}

/// So the member merges as the body it names, reference and all: before it was read back it carried
/// no members, and the document closed around the base alone.
#[test]
#[cfg(feature = "jsonschema")]
fn test_a_deferred_union_member_merges_as_the_body_it_names() {
    assert_eq!(
        serde_json::to_string(&FlatOverSelfUntagged::json_schema()).unwrap(),
        r##"{"$defs":{"FlatSelfNode":{"type":"object","additionalProperties":false,"properties":{"kids":{"type":"array","items":{"$ref":"#/$defs/FlatSelfNode"}},"leaf":{"type":"string"}},"required":["kids","leaf"]}},"type":"object","properties":{"own":{"type":"string"},"kids":{"type":"array","items":{"$ref":"#/$defs/FlatSelfNode"}},"leaf":{"type":"string"}},"required":["own","kids","leaf"],"additionalProperties":false}"##
    );
}

/// And the document accepts what serde writes, self-reference resolving from the container's root.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_deferred_union_member_schema_accepts_the_payload_serde_writes() {
    let schema = FlatOverSelfUntagged::json_schema();
    let payload = serde_json::json!({ "own": "o", "kids": [], "leaf": "l" });
    assert!(
        closed_document_accepts(&schema, &payload),
        "{payload} is rejected by {schema}"
    );
    let reference = schema["properties"]["kids"]["items"]["$ref"]
        .as_str()
        .unwrap();
    let resolved = schema
        .pointer(reference.strip_prefix('#').unwrap())
        .unwrap();
    assert!(
        resolved["properties"]
            .as_object()
            .unwrap()
            .contains_key("leaf")
    );
}

/// What serde writes for a flattened untagged enum whose members overlap: the narrower member's
/// keys, which are a subset of the wider member's. serde takes the first member that matches, so
/// this is the payload it both writes and reads back.
#[test]
fn test_flattening_an_overlapping_untagged_enum_writes_the_narrower_members_keys() {
    let narrow = FlatOverOverlap {
        own: "o".to_owned(),
        either: OverlapEither::Narrow(OverlapNarrow { a: "x".to_owned() }),
    };
    let written = serde_json::to_value(&narrow).unwrap();
    assert_eq!(written, serde_json::json!({ "own": "o", "a": "x" }));
    let back: FlatOverOverlap = serde_json::from_value(written).unwrap();
    assert_eq!(back, narrow);
}

/// And two branches of the merged document admit that payload: the narrow member's branch names
/// exactly its keys, and the wide member's branch names one more that it does not require.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_overlapping_payload_is_admitted_by_two_branches() {
    let schema = FlatOverOverlap::json_schema();
    assert_eq!(
        accepting_branches(
            &schema["anyOf"],
            &serde_json::json!({ "own": "o", "a": "x" })
        ),
        2,
        "Got: {schema}"
    );
}

/// So the merge keeps the spelling its source used. An untagged enum is first-match-wins, and more
/// than one branch admitting a payload is its normal state, which is what `anyOf` says and `oneOf`
/// denies.
#[test]
#[cfg(feature = "jsonschema")]
fn test_an_overlapping_untagged_flatten_keeps_the_any_of_spelling() {
    assert_eq!(
        serde_json::to_string(&FlatOverOverlap::json_schema()).unwrap(),
        r#"{"type":"object","anyOf":[{"type":"object","properties":{"own":{"type":"string"},"a":{"type":"string"}},"required":["own","a"],"additionalProperties":false},{"type":"object","properties":{"own":{"type":"string"},"a":{"type":"string"},"b":{"anyOf":[{"type":"string"},{"type":"null"}]}},"required":["own","a"],"additionalProperties":false}]}"#
    );
}

/// And the document accepts every payload serde writes for it. Wrapped in `oneOf`, the payload the
/// narrower member writes matched two branches and was rejected.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_overlapping_untagged_flatten_schema_accepts_every_payload_serde_writes() {
    let schema = FlatOverOverlap::json_schema();
    for payload in [
        serde_json::json!({ "own": "o", "a": "x" }),
        serde_json::json!({ "own": "o", "a": "x", "b": "y" }),
    ] {
        assert!(
            closed_document_accepts(&schema, &payload),
            "{payload} is rejected by {schema}"
        );
    }
}

/// What serde writes for an object that flattens both spellings of a union: the discriminated
/// enum's tag and members, the untagged enum's matched member, and the object's own keys.
#[test]
fn test_flattening_both_spellings_of_a_union_writes_every_members_keys() {
    let mixed = FlatOverMixed {
        own: "o".to_owned(),
        tagged: MixedTagged::Left {
            left: "l".to_owned(),
        },
        either: OverlapEither::Narrow(OverlapNarrow { a: "x".to_owned() }),
    };
    let written = serde_json::to_value(&mixed).unwrap();
    assert_eq!(
        written,
        serde_json::json!({ "kind": "Left", "left": "l", "a": "x", "own": "o" })
    );
    let back: FlatOverMixed = serde_json::from_value(written).unwrap();
    assert_eq!(back, mixed);
}

/// So each source keeps its own wrapper around its own branches, nested in the order the sources
/// were merged: the untagged enum's overlapping members under `anyOf`, and inside each of them the
/// discriminated enum's exclusive members under `oneOf`.
#[test]
#[cfg(feature = "jsonschema")]
fn test_a_mixed_merge_keeps_each_sources_wrapper() {
    let schema = FlatOverMixed::json_schema();
    let untagged = schema["anyOf"].as_array().unwrap();
    assert_eq!(untagged.len(), 2, "Got: {schema}");
    for branch in untagged {
        assert_eq!(
            branch["oneOf"].as_array().unwrap().len(),
            2,
            "Got: {schema}"
        );
    }
}

/// And the document accepts every payload serde writes for it, whichever member of either union
/// matched.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_mixed_merge_schema_accepts_every_payload_serde_writes() {
    let schema = FlatOverMixed::json_schema();
    for payload in [
        serde_json::json!({ "kind": "Left", "left": "l", "a": "x", "own": "o" }),
        serde_json::json!({ "kind": "Left", "left": "l", "a": "x", "b": "y", "own": "o" }),
        serde_json::json!({ "kind": "Right", "right": true, "a": "x", "own": "o" }),
        serde_json::json!({ "kind": "Right", "right": true, "a": "x", "b": "y", "own": "o" }),
    ] {
        assert!(
            closed_document_accepts(&schema, &payload),
            "{payload} is rejected by {schema}"
        );
    }
}

/// What serde writes for a struct that flattens a union one member of which is itself a union: the
/// struct's own keys, and beside them the keys of whichever leaf member matched. The inner union is
/// a choice, not a value, so it writes no key of its own.
#[test]
fn test_flattening_a_nested_union_writes_the_leaf_members_keys() {
    let nested: [(NestEither, serde_json::Value); 3] = [
        (
            NestEither::Plain(NestPlain {
                plain: "p".to_owned(),
            }),
            serde_json::json!({ "own": "o", "plain": "p" }),
        ),
        (
            NestEither::Tagged(NestTagged::Left {
                left: "l".to_owned(),
            }),
            serde_json::json!({ "own": "o", "kind": "Left", "left": "l" }),
        ),
        (
            NestEither::Tagged(NestTagged::Right { right: true }),
            serde_json::json!({ "own": "o", "kind": "Right", "right": true }),
        ),
    ];
    for (either, expected) in nested {
        let holder = NestHolder {
            own: "o".to_owned(),
            either,
        };
        let written = serde_json::to_value(&holder).unwrap();
        assert_eq!(written, expected);
        let back: NestHolder = serde_json::from_value(written).unwrap();
        assert_eq!(back, holder);
    }
}

/// So the merged schema multiplies the base out over the leaves rather than over the inner union,
/// which carries no members to merge: before the branches expanded, the document named none of the
/// leaves' keys and closed around `own` alone.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_nested_union_schema_accepts_every_payload_serde_writes() {
    let schema = NestHolder::json_schema();
    for payload in [
        serde_json::json!({ "own": "o", "plain": "p" }),
        serde_json::json!({ "own": "o", "kind": "Left", "left": "l" }),
        serde_json::json!({ "own": "o", "kind": "Right", "right": true }),
    ] {
        assert!(
            closed_document_accepts(&schema, &payload),
            "{payload} is rejected by {schema}"
        );
    }
}

/// And each union keeps the spelling its own source used, however deep it sits: the untagged outer
/// one is first-match-wins under `anyOf`, and the discriminated inner one exclusive under `oneOf`.
#[test]
#[cfg(feature = "jsonschema")]
fn test_a_nested_union_branch_expands_under_its_own_spelling() {
    assert_eq!(
        serde_json::to_string(&NestHolder::json_schema()).unwrap(),
        r#"{"type":"object","anyOf":[{"type":"object","properties":{"own":{"type":"string"},"plain":{"type":"string"}},"required":["own","plain"],"additionalProperties":false},{"type":"object","oneOf":[{"type":"object","properties":{"own":{"type":"string"},"kind":{"type":"string","const":"Left"},"left":{"type":"string"}},"required":["own","kind","left"],"additionalProperties":false},{"type":"object","properties":{"own":{"type":"string"},"kind":{"type":"string","const":"Right"},"right":{"type":"boolean"}},"required":["own","kind","right"],"additionalProperties":false}]}]}"#
    );
}

/// What serde writes when every level of the nesting holds one member: the same one key set the
/// single leaf writes.
#[test]
fn test_flattening_a_single_member_nested_union_writes_the_leafs_keys() {
    let holder = NestOnlyHolder {
        own: "o".to_owned(),
        either: NestOnlyOuter::Inner(NestOnlyInner::A(NestOnly { a: "x".to_owned() })),
    };
    let written = serde_json::to_value(&holder).unwrap();
    assert_eq!(written, serde_json::json!({ "own": "o", "a": "x" }));
    let back: NestOnlyHolder = serde_json::from_value(written).unwrap();
    assert_eq!(back, holder);
}

/// So the document is that one object, wrapped in nothing: a choice of one is no choice at any
/// depth. Before the branches expanded, it closed around `own` and rejected the only payload the
/// type has.
#[test]
#[cfg(feature = "jsonschema")]
fn test_a_single_member_nested_union_collapses_to_the_leafs_object() {
    assert_eq!(
        serde_json::to_string(&NestOnlyHolder::json_schema()).unwrap(),
        r#"{"type":"object","properties":{"own":{"type":"string"},"a":{"type":"string"}},"required":["own","a"],"additionalProperties":false}"#
    );
    assert!(closed_document_accepts(
        &NestOnlyHolder::json_schema(),
        &serde_json::json!({ "own": "o", "a": "x" })
    ));
}

/// A cycle closed through two unions that name each other has a body at every step — the deferred
/// one is filled in before the merge reads it — so it is the expansion path that names it, and the
/// merge names that path rather than descending it forever.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`NestCycleHolder`: `#[serde(flatten)]` of `NestCycleOuter` closes a flatten cycle through nested unions — its branch 1.1 names `NestCycleOuter`, already expanding on the path `NestCycleOuter`"
)]
fn test_a_cycle_through_nested_unions_names_the_path_it_closes() {
    assert!(NestCycleHolder::json_schema().is_object());
}

/// The remedy is the one every flatten cycle names.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "write the field as a named member so the cycle defers through a reference"
)]
fn test_the_nested_union_cycle_refusal_names_the_remedy() {
    assert!(NestCycleHolder::json_schema().is_object());
}

/// What serde writes for a base reached through an `Option`: the base's members beside the object's
/// own when the field is `Some`, and the object's own alone when it is `None`. Both read back as the
/// value that wrote them, so both are payloads of the type rather than one form and one accident.
#[test]
fn test_an_optional_flattened_base_writes_its_members_or_nothing() {
    let forms: [(Option<OptBase>, serde_json::Value); 2] = [
        (
            Some(OptBase {
                left: "l".to_owned(),
                right: true,
            }),
            serde_json::json!({ "left": "l", "right": true, "own": "o" }),
        ),
        (None, serde_json::json!({ "own": "o" })),
    ];
    for (maybe, expected) in forms {
        let holder = OptHolder {
            maybe,
            own: "o".to_owned(),
        };
        let written = serde_json::to_value(&holder).unwrap();
        assert_eq!(written, expected);
        let back: OptHolder = serde_json::from_value(written).unwrap();
        assert_eq!(back, holder);
    }
}

/// And when the optional base is a union, the same two forms with the matched member's keys in the
/// present one: the `Option` and the enum are two choices, and only the innermost writes keys.
#[test]
fn test_an_optional_flattened_union_writes_the_matched_members_keys_or_nothing() {
    let forms: [(Option<NestTagged>, serde_json::Value); 3] = [
        (
            Some(NestTagged::Left {
                left: "l".to_owned(),
            }),
            serde_json::json!({ "kind": "Left", "left": "l", "own": "o" }),
        ),
        (
            Some(NestTagged::Right { right: true }),
            serde_json::json!({ "kind": "Right", "right": true, "own": "o" }),
        ),
        (None, serde_json::json!({ "own": "o" })),
    ];
    for (maybe, expected) in forms {
        let holder = OptUnionHolder {
            maybe,
            own: "o".to_owned(),
        };
        let written = serde_json::to_value(&holder).unwrap();
        assert_eq!(written, expected);
        let back: OptUnionHolder = serde_json::from_value(written).unwrap();
        assert_eq!(back, holder);
    }
}

/// So the merged document accepts both: the base's members are what an object writes beside its own
/// or does not write at all, and folding them into one key set required the object to write keys the
/// `None` payload never carries.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_optional_flatten_schema_accepts_every_payload_serde_writes() {
    let schema = OptHolder::json_schema();
    for payload in [
        serde_json::json!({ "left": "l", "right": true, "own": "o" }),
        serde_json::json!({ "own": "o" }),
    ] {
        assert!(
            closed_document_accepts(&schema, &payload),
            "{payload} is rejected by {schema}"
        );
    }
}

/// And rejects a base written in part. serde writes the base whole or not at all, so a payload
/// carrying some of its required members is one no value of the type produces — which is what the
/// two branches say and dropping the members from `required` would not.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_optional_flatten_schema_rejects_a_partial_base() {
    let schema = OptHolder::json_schema();
    for payload in [
        serde_json::json!({ "left": "l", "own": "o" }),
        serde_json::json!({ "right": true, "own": "o" }),
    ] {
        assert!(
            !closed_document_accepts(&schema, &payload),
            "{payload} is accepted by {schema}"
        );
    }
}

/// The document says both: base members joined to the object's under one `anyOf` branch, the
/// object's own alone under another — the branches overlap on the payload the absent branch
/// stands for.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_optional_flatten_document_offers_the_base_and_its_absence() {
    assert_eq!(
        serde_json::to_string(&OptHolder::json_schema()).unwrap(),
        r#"{"type":"object","anyOf":[{"type":"object","properties":{"own":{"type":"string"},"left":{"type":"string"},"right":{"type":"boolean"}},"required":["own","left","right"],"additionalProperties":false},{"type":"object","properties":{"own":{"type":"string"}},"required":["own"],"additionalProperties":false}]}"#
    );
}

/// And when the optional source is a union, the absence joins its branches from outside: each member
/// keeps the spelling its own enum was written under, and the absent branch is a choice about the
/// whole union rather than a member of it.
#[test]
#[cfg(feature = "jsonschema")]
fn test_an_optional_flattened_union_offers_every_member_and_their_absence() {
    let schema = OptUnionHolder::json_schema();
    for payload in [
        serde_json::json!({ "kind": "Left", "left": "l", "own": "o" }),
        serde_json::json!({ "kind": "Right", "right": true, "own": "o" }),
        serde_json::json!({ "own": "o" }),
    ] {
        assert!(
            closed_document_accepts(&schema, &payload),
            "{payload} is rejected by {schema}"
        );
    }
    assert_eq!(
        serde_json::to_string(&schema).unwrap(),
        r#"{"type":"object","anyOf":[{"type":"object","oneOf":[{"type":"object","properties":{"own":{"type":"string"},"kind":{"type":"string","const":"Left"},"left":{"type":"string"}},"required":["own","kind","left"],"additionalProperties":false},{"type":"object","properties":{"own":{"type":"string"},"kind":{"type":"string","const":"Right"},"right":{"type":"boolean"}},"required":["own","kind","right"],"additionalProperties":false}]},{"type":"object","properties":{"own":{"type":"string"}},"required":["own"],"additionalProperties":false}]}"#
    );
}

/// The same two key sets on TypeScript: `| undefined` said something else — that the whole value
/// may be missing — and `&` binds tighter than `|`, so it admitted neither payload for an absent
/// base.
#[test]
#[cfg(feature = "typescript")]
fn test_the_optional_flatten_type_offers_the_base_and_its_absence() {
    let ts = OptHolder::ts_definition();
    assert!(
        ts.contains("} & (OptBase | { [K in keyof OptBase]?: never });"),
        "expected the base beside its absence, got: {ts}"
    );
    assert!(
        !ts.contains("| undefined"),
        "the whole value is offered as absent in: {ts}"
    );
}

/// And when the optional source is a union, the absence joins it from outside: the members keep the
/// spelling their own enum was written under, and the branch that carries none of them is written
/// over the keys every member shares.
#[test]
#[cfg(feature = "typescript")]
fn test_an_optional_flattened_union_type_offers_its_members_and_their_absence() {
    let ts = OptUnionHolder::ts_definition();
    assert!(
        ts.contains("} & (NestTagged | { [K in keyof NestTagged]?: never });"),
        "expected the union beside its absence, got: {ts}"
    );
    assert!(
        !ts.contains("| undefined"),
        "the whole value is offered as absent in: {ts}"
    );
}

/// Zod writes the choice outside the intersection because that's the only place it can read it —
/// an intersection recognizes only the keys its operands name, so a choice operand would leave
/// each branch missing the other's keys.
#[test]
#[cfg(feature = "zod")]
fn test_the_optional_flatten_schema_offers_the_base_and_its_absence() {
    let zod = OptHolder::zod_schema();
    assert!(
        zod.contains(
            "z.union([\n  OptHolder$OwnSchema.and(z.lazy(() => OptBase$Schema)),\n  OptHolder$OwnSchema,\n])"
        ),
        "expected the base beside its absence, got: {zod}"
    );
    assert!(
        !zod.contains(".and(z.lazy(() => z.union("),
        "the choice is written inside the intersection in: {zod}"
    );
}

/// And no branch of it admits a base written in part. The base joins a branch whole, under the name
/// its own schema is bound to, or the branch is the object's own keys alone — so a payload carrying
/// some of the base's members belongs to neither, and neither does a bare `undefined`.
#[test]
#[cfg(feature = "zod")]
fn test_the_optional_flatten_schema_admits_no_partial_base() {
    let zod = OptHolder::zod_schema();
    assert!(
        !zod.contains("z.undefined()") && !zod.contains(".prefault("),
        "the whole value is offered as absent in: {zod}"
    );
    assert!(
        !zod.contains(".optional()"),
        "the base's members are offered one at a time in: {zod}"
    );
    assert_eq!(
        zod.matches("OptBase$Schema").count(),
        1,
        "the base is named more than once in: {zod}"
    );
}

/// What the object's own keys are bound to, so each branch names them rather than repeating them:
/// one strict object, read by both branches of the choice.
#[test]
#[cfg(feature = "zod")]
fn test_the_optional_flatten_schema_binds_the_objects_own_keys_once() {
    let zod = OptHolder::zod_schema();
    assert!(
        zod.contains("const OptHolder$OwnSchema = z.strictObject({\n  own: z.string(),\n});"),
        "expected the object's own keys bound once, got: {zod}"
    );
    assert_eq!(
        zod.matches("z.strictObject(").count(),
        1,
        "the object's own keys are written more than once in: {zod}"
    );
}

/// The same two payloads reached through a name rather than an `Option`: serde reads and writes
/// both forms, so the merge owes it two key sets rather than a refusal.
#[test]
fn test_a_named_nullable_flattened_base_writes_its_members_or_nothing() {
    let forms: [(Option<OptBase>, serde_json::Value); 2] = [
        (
            Some(OptBase {
                left: "l".to_owned(),
                right: true,
            }),
            serde_json::json!({ "left": "l", "right": true, "own": "o" }),
        ),
        (None, serde_json::json!({ "own": "o" })),
    ];
    for (maybe, expected) in forms {
        let holder = NamedMaybeHolder {
            maybe: MaybeOptBase(maybe),
            own: "o".to_owned(),
        };
        let written = serde_json::to_value(&holder).unwrap();
        assert_eq!(written, expected);
        let back: NamedMaybeHolder = serde_json::from_value(written).unwrap();
        assert_eq!(back, holder);
    }
}

/// So the merged document is the one the `Option`-typed field's own writes, key for key: the base's
/// members joined to the object's under one branch and the object's own alone under another. The two
/// spellings reach the same absence, so they describe as one document.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_named_nullable_flatten_document_is_the_optional_flattens_own() {
    assert_eq!(
        serde_json::to_string(&NamedMaybeHolder::json_schema()).unwrap(),
        serde_json::to_string(&OptHolder::json_schema()).unwrap(),
    );
    assert_eq!(
        serde_json::to_string(&NamedMaybeHolder::json_schema()).unwrap(),
        r#"{"type":"object","anyOf":[{"type":"object","properties":{"own":{"type":"string"},"left":{"type":"string"},"right":{"type":"boolean"}},"required":["own","left","right"],"additionalProperties":false},{"type":"object","properties":{"own":{"type":"string"}},"required":["own"],"additionalProperties":false}]}"#
    );
}

/// And it accepts every payload serde writes and no base written in part, which is what the two
/// branches say and folding them into one key set could not.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_named_nullable_flatten_document_admits_the_captured_payloads() {
    let schema = NamedMaybeHolder::json_schema();
    for payload in [
        serde_json::json!({ "left": "l", "right": true, "own": "o" }),
        serde_json::json!({ "own": "o" }),
    ] {
        assert!(
            closed_document_accepts(&schema, &payload),
            "{payload} is rejected by {schema}"
        );
    }
    for payload in [
        serde_json::json!({ "left": "l", "own": "o" }),
        serde_json::json!({ "right": true, "own": "o" }),
    ] {
        assert!(
            !closed_document_accepts(&schema, &payload),
            "{payload} is accepted by {schema}"
        );
    }
}

/// Zod writes the same choice around the intersection, leaving the name spelled as the nullable
/// binding it is — its null side carries no key an intersection could take.
#[test]
#[cfg(feature = "zod")]
fn test_the_named_nullable_flatten_schema_offers_the_base_and_its_absence() {
    let zod = NamedMaybeHolder::zod_schema();
    assert!(
        zod.contains(
            "z.union([\n  NamedMaybeHolder$OwnSchema.and(z.lazy(() => MaybeOptBase$Schema)),\n  NamedMaybeHolder$OwnSchema,\n])"
        ),
        "expected the base beside its absence, got: {zod}"
    );
    assert_eq!(
        zod.matches("MaybeOptBase$Schema").count(),
        1,
        "the base is named more than once in: {zod}"
    );
}

/// TypeScript writes the same choice: the name carries the value branch since the intersection
/// distributes over it and `null` takes no key, while the other branch reads the base's members
/// off the value side by name.
#[test]
#[cfg(feature = "typescript")]
fn test_the_named_nullable_flatten_type_offers_the_base_and_its_absence() {
    let ts = NamedMaybeHolder::ts_definition();
    assert!(
        ts.contains("} & (MaybeOptBase | { [K in keyof NonNullable<MaybeOptBase>]?: never });"),
        "expected the base beside its absence, got: {ts}"
    );
    assert!(
        !ts.contains("| undefined"),
        "the whole value is offered as absent in: {ts}"
    );
}

/// The absence is a question about what the name published — a direct flatten naming a
/// non-nullable item is spelled exactly as before there was a second branch: one intersection on
/// Zod, one closed object on the document, the bare name on TypeScript.
#[test]
#[cfg(feature = "typescript")]
fn test_a_named_non_nullable_flatten_type_is_byte_identical() {
    let ts = NamedPlainHolder::ts_definition();
    let declared = &ts[ts.find("export type").unwrap()..];
    assert_eq!(
        declared,
        "export type NamedPlainHolder = {\n  /**\n   * own\n   * \n   */\n  own: string;\n} & PlainOptBase;"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_a_named_non_nullable_flatten_schema_is_byte_identical() {
    #[cfg(feature = "typescript")]
    const EXPECTED: &str = "const NamedPlainHolder$RawSchema = z.strictObject({\n  own: z.string(),\n}).and(z.lazy(() => PlainOptBase$Schema));\n\nexport const NamedPlainHolder$Schema: ZodType<NamedPlainHolder> = NamedPlainHolder$RawSchema;";
    #[cfg(not(feature = "typescript"))]
    const EXPECTED: &str = "export const NamedPlainHolder$Schema = z.strictObject({\n  own: z.string(),\n}).and(z.lazy(() => PlainOptBase$Schema));";

    assert_eq!(NamedPlainHolder::zod_schema(), EXPECTED);
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_a_named_non_nullable_flatten_document_is_byte_identical() {
    assert_eq!(
        serde_json::to_string(&NamedPlainHolder::json_schema()).unwrap(),
        r#"{"type":"object","properties":{"own":{"type":"string"},"left":{"type":"string"},"right":{"type":"boolean"}},"required":["own","left","right"],"additionalProperties":false}"#
    );
}

/// A nullable scalar writes one of its two values and refuses the other, which is what makes the
/// declaration one no spelling of the merge can describe: serde writes the object's own keys alone
/// for the `None` and reads them back as it, and refuses the `Some` where it stands.
#[test]
#[cfg(not(feature = "zod"))]
fn test_flattening_a_nullable_scalar_writes_the_absence_and_refuses_the_value() {
    assert_eq!(
        serde_json::to_value(MaybeCountHolder {
            count: MaybeCount(None),
            own: "o".to_owned(),
        })
        .unwrap(),
        serde_json::json!({ "own": "o" })
    );
    let refusal = serde_json::to_value(MaybeCountHolder {
        count: MaybeCount(Some(3)),
        own: "o".to_owned(),
    })
    .unwrap_err()
    .to_string();
    assert!(
        refusal.contains("can only flatten structs and maps"),
        "Got: {refusal}"
    );
}

/// So the JSON-schema merge refuses the whole declaration at the branch the value sits at, rather
/// than writing the absence branch alone.
#[test]
#[cfg(all(feature = "jsonschema", not(feature = "zod")))]
#[should_panic(
    expected = "`MaybeCountHolder`: `#[serde(flatten)]` of `MaybeCount` writes a union member that is not an object — its branch 1 describes a `integer`"
)]
fn test_flattening_a_nullable_scalar_is_refused_by_the_merge() {
    assert!(MaybeCountHolder::json_schema().is_object());
}

/// And in those same words wherever the registration was declared, which is the one reading of the
/// declaration that survives the Zod surface refusing it at expansion.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`LaterMaybeCountHolder`: `#[serde(flatten)]` of `LaterMaybeCount` writes a union member that is not an object — its branch 1 describes a `integer`"
)]
fn test_flattening_a_later_nullable_scalar_is_refused_by_the_merge() {
    assert!(LaterMaybeCountHolder::json_schema().is_object());
}

/// A registration written below the object has recorded nothing when the object expands, so it
/// proves neither the absence it publishes nor the value that is no object, and the merge writes the
/// one operand the name is — the same fallback a name this crate never expands takes.
#[test]
#[cfg(feature = "zod")]
fn test_the_later_nullable_scalar_keeps_the_declaration_order_fallback() {
    let zod = LaterMaybeCountHolder::zod_schema();
    assert!(
        zod.contains(".and(z.lazy(() => LaterMaybeCount$Schema))"),
        "expected the name as the one operand it is, got: {zod}"
    );
    assert!(
        !zod.contains("z.union(["),
        "the name is spelled as a choice in: {zod}"
    );
}

/// The absence is a question about the `Option` and nothing else, so a base written without one is
/// spelled exactly as it was before there was a second branch to spell — byte for byte, on both
/// surfaces.
#[test]
#[cfg(feature = "typescript")]
fn test_a_non_optional_flatten_type_is_byte_identical() {
    let ts = MultiFlatten::ts_definition();
    let declared = &ts[ts.find("export type").unwrap()..];
    assert_eq!(
        declared,
        "export type MultiFlatten = {\n  /**\n   * id\n   * \n   */\n  id: string;\n} & BasePart & ExtraPart;"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_a_non_optional_flatten_schema_is_byte_identical() {
    #[cfg(feature = "typescript")]
    const EXPECTED: &str = "const MultiFlatten$RawSchema = z.strictObject({\n  id: z.string(),\n}).and(z.lazy(() => BasePart$Schema)).and(z.lazy(() => ExtraPart$Schema));\n\nexport const MultiFlatten$Schema: ZodType<MultiFlatten> = MultiFlatten$RawSchema;";
    #[cfg(not(feature = "typescript"))]
    const EXPECTED: &str = "export const MultiFlatten$Schema = z.strictObject({\n  id: z.string(),\n}).and(z.lazy(() => BasePart$Schema)).and(z.lazy(() => ExtraPart$Schema));";

    assert_eq!(MultiFlatten::zod_schema(), EXPECTED);
}

/// What serde writes for a flattened untagged enum is one key set per member, and what Zod says
/// about it is the object multiplied over those members: a union of intersections, not an
/// intersection with a union.
#[test]
#[cfg(feature = "zod")]
fn test_the_untagged_flatten_schema_multiplies_the_object_over_the_unions_members() {
    #[cfg(feature = "typescript")]
    const EXPECTED: &str = "const FlatOverUntagged$OwnSchema = z.strictObject({\n  own: z.string(),\n});\n\nconst FlatOverUntagged$RawSchema = z.union([\n  FlatOverUntagged$OwnSchema.and(z.lazy(() => FlatFirst$Schema)),\n  FlatOverUntagged$OwnSchema.and(z.lazy(() => FlatSecond$Schema)),\n]);\n\nexport const FlatOverUntagged$Schema: ZodType<FlatOverUntagged> = FlatOverUntagged$RawSchema;";
    #[cfg(not(feature = "typescript"))]
    const EXPECTED: &str = "const FlatOverUntagged$OwnSchema = z.strictObject({\n  own: z.string(),\n});\n\nexport const FlatOverUntagged$Schema = z.union([\n  FlatOverUntagged$OwnSchema.and(z.lazy(() => FlatFirst$Schema)),\n  FlatOverUntagged$OwnSchema.and(z.lazy(() => FlatSecond$Schema)),\n]);";

    assert_eq!(FlatOverUntagged::zod_schema(), EXPECTED);
}

/// And the union itself is never an operand. Its name carries no key set, so an intersection built
/// on it is the shape that rejected everything; the members reach the merge through the registry
/// instead, each deferred exactly as a single base already was.
#[test]
#[cfg(feature = "zod")]
fn test_the_untagged_flatten_schema_names_no_union_as_an_operand() {
    let zod = FlatOverUntagged::zod_schema();
    assert!(
        !zod.contains("FlatEither"),
        "the union is named as an operand in: {zod}"
    );
    assert!(
        !zod.contains(".and(FlatFirst$Schema)") && !zod.contains(".and(FlatSecond$Schema)"),
        "a member is read eagerly in: {zod}"
    );
    assert_eq!(
        zod.matches("z.strictObject(").count(),
        1,
        "the object's own keys are written more than once in: {zod}"
    );
}

/// The two choices multiply. An `Option` around the union offers the members or none of them, and
/// the union offers one member or another, so the object writes one branch per member and one more
/// for the absence — the same multiplication the JSON-schema document is written from.
#[test]
#[cfg(feature = "zod")]
fn test_an_optional_untagged_flatten_multiplies_the_members_and_the_absence() {
    let zod = FlatOverOptionalUntagged::zod_schema();
    assert!(
        zod.contains(
            "z.union([\n  FlatOverOptionalUntagged$OwnSchema.and(z.lazy(() => FlatFirst$Schema)),\n  FlatOverOptionalUntagged$OwnSchema.and(z.lazy(() => FlatSecond$Schema)),\n  FlatOverOptionalUntagged$OwnSchema,\n])"
        ),
        "expected every member beside the absence, got: {zod}"
    );
    assert!(
        !zod.contains("z.undefined()") && !zod.contains(".optional()"),
        "the absence is offered as a value rather than as a branch in: {zod}"
    );
}

/// What serde writes for it: the matched member's keys beside the object's own, or the object's own
/// alone. Both read back as the value that wrote them, so both are payloads of the type.
#[test]
fn test_an_optional_flattened_untagged_enum_writes_a_members_keys_or_nothing() {
    let forms: [(Option<FlatEither>, serde_json::Value); 3] = [
        (
            Some(FlatEither::First(FlatFirst { a: "x".to_owned() })),
            serde_json::json!({ "a": "x", "own": "o" }),
        ),
        (
            Some(FlatEither::Second(FlatSecond { b: true })),
            serde_json::json!({ "b": true, "own": "o" }),
        ),
        (None, serde_json::json!({ "own": "o" })),
    ];
    for (either, expected) in forms {
        let holder = FlatOverOptionalUntagged {
            either,
            own: "o".to_owned(),
        };
        let written = serde_json::to_value(&holder).unwrap();
        assert_eq!(written, expected);
        let back: FlatOverOptionalUntagged = serde_json::from_value(written).unwrap();
        assert_eq!(back, holder);
    }
}

/// An alias is the type it names on every surface, so an object flattening the alias multiplies
/// over exactly the members the enum's own name would have given it.
#[test]
#[cfg(feature = "zod")]
fn test_an_aliased_untagged_flatten_multiplies_over_the_same_members() {
    let zod = FlatOverAliasedUntagged::zod_schema();
    assert!(
        zod.contains(
            "z.union([\n  FlatOverAliasedUntagged$OwnSchema.and(z.lazy(() => FlatFirst$Schema)),\n  FlatOverAliasedUntagged$OwnSchema.and(z.lazy(() => FlatSecond$Schema)),\n])"
        ),
        "expected the aliased union's members, got: {zod}"
    );
}

/// A union nested inside a union contributes its leaves, not its name — the nesting writes no key
/// of its own, so the object's own keys stay where they were and the leaf joins them directly.
#[test]
#[cfg(feature = "zod")]
fn test_a_nested_untagged_flatten_multiplies_over_the_leaf_members() {
    let nested = NestHolder::zod_schema();
    assert!(
        nested.contains(
            "z.union([\n  NestHolder$OwnSchema.and(z.lazy(() => NestPlain$Schema)),\n  NestHolder$OwnSchema.and(z.lazy(() => NestTagged$Schema)),\n])"
        ),
        "expected the leaves of the nesting, got: {nested}"
    );
    let only = NestOnlyHolder::zod_schema();
    assert!(
        only.contains("}).and(z.lazy(() => NestOnly$Schema));"),
        "expected the one leaf joined directly, got: {only}"
    );
    assert!(
        !only.contains("$OwnSchema") && !only.contains("z.union(["),
        "a choice of one is written as a choice in: {only}"
    );
}

/// A union declared below the object that flattens it is named as one operand — the spelling that
/// rejects every payload the object writes, kept deliberately.
#[test]
#[cfg(feature = "zod")]
fn test_a_union_declared_below_the_object_is_named_as_one_operand() {
    let zod = FlatOverLaterUntagged::zod_schema();
    assert!(
        zod.contains("}).and(z.lazy(() => LaterEither$Schema));"),
        "expected the union named as one operand, got: {zod}"
    );
    assert!(
        !zod.contains("$OwnSchema"),
        "a source with no members to multiply wrote a choice in: {zod}"
    );
}

/// A base Zod does read a key set off is untouched. `z.discriminatedUnion` propagates its members'
/// keys to the intersection, so an internally tagged base was never the shape that failed and is
/// spelled byte for byte as it was.
#[test]
#[cfg(feature = "zod")]
fn test_an_internally_tagged_flatten_schema_is_byte_identical() {
    #[cfg(feature = "typescript")]
    const EXPECTED: &str = "const DataElementSampleValueEntry$RawSchema = z.strictObject({\n  dataElementId: z.string(),\n}).and(z.lazy(() => DataElementSampleValueVariant$Schema));\n\nexport const DataElementSampleValueEntry$Schema: ZodType<DataElementSampleValueEntry> = DataElementSampleValueEntry$RawSchema;";
    #[cfg(not(feature = "typescript"))]
    const EXPECTED: &str = "export const DataElementSampleValueEntry$Schema = z.strictObject({\n  dataElementId: z.string(),\n}).and(z.lazy(() => DataElementSampleValueVariant$Schema));";

    assert_eq!(DataElementSampleValueEntry::zod_schema(), EXPECTED);
}

/// A union holding a member serde writes as a scalar is a union like any other while nothing
/// flattens it: the member is a branch of the choice, where a scalar is exactly what a payload may
/// be, and the schema is spelled byte for byte as it was before the merge could tell the two apart.
#[test]
#[cfg(feature = "zod")]
fn test_a_standalone_union_with_a_scalar_member_is_byte_identical() {
    #[cfg(feature = "typescript")]
    const EXPECTED: &str = "const FlatScalarEither$RawSchema = z.union([FlatFirst$Schema, z.string()]);\n\nexport const FlatScalarEither$Schema: ZodType<FlatScalarEither> = FlatScalarEither$RawSchema;";
    #[cfg(not(feature = "typescript"))]
    const EXPECTED: &str =
        "export const FlatScalarEither$Schema = z.union([FlatFirst$Schema, z.string()]);";

    assert_eq!(FlatScalarEither::zod_schema(), EXPECTED);
}

/// A union holding an `Option`-reached member, a brand, or a plain enum is a union like any other
/// while nothing flattens it — each spelled byte for byte as before the merge could tell the
/// shapes apart.
#[test]
#[cfg(feature = "zod")]
fn test_standalone_unions_the_merge_now_refuses_are_byte_identical() {
    #[cfg(feature = "typescript")]
    const EXPECTED: [&str; 4] = [
        "const FlatNullableEither$RawSchema = z.union([FlatFirst$Schema, z.nullable(FlatSecond$Schema)]);\n\nexport const FlatNullableEither$Schema: ZodType<FlatNullableEither> = FlatNullableEither$RawSchema;",
        "const MemberSlugEither$RawSchema = z.union([FlatFirst$Schema, MemberSlug$Schema]);\n\nexport const MemberSlugEither$Schema: ZodType<MemberSlugEither> = MemberSlugEither$RawSchema;",
        "const MemberSwitchEither$RawSchema = z.union([FlatFirst$Schema, MemberSwitch$Schema]);\n\nexport const MemberSwitchEither$Schema: ZodType<MemberSwitchEither> = MemberSwitchEither$RawSchema;",
        "const MemberHueEither$RawSchema = z.union([FlatFirst$Schema, MemberHue$Schema]);\n\nexport const MemberHueEither$Schema: ZodType<MemberHueEither> = MemberHueEither$RawSchema;",
    ];
    #[cfg(not(feature = "typescript"))]
    const EXPECTED: [&str; 4] = [
        "export const FlatNullableEither$Schema = z.union([FlatFirst$Schema, z.nullable(FlatSecond$Schema)]);",
        "export const MemberSlugEither$Schema = z.union([FlatFirst$Schema, MemberSlug$Schema]);",
        "export const MemberSwitchEither$Schema = z.union([FlatFirst$Schema, MemberSwitch$Schema]);",
        "export const MemberHueEither$Schema = z.union([FlatFirst$Schema, MemberHue$Schema]);",
    ];

    assert_eq!(
        [
            FlatNullableEither::zod_schema(),
            MemberSlugEither::zod_schema(),
            MemberSwitchEither::zod_schema(),
            MemberHueEither::zod_schema(),
        ],
        EXPECTED.map(str::to_owned)
    );
}

/// And the reach of both refusals is the recording's, exactly as the scalar one's is: a union
/// declared below the object records nothing for the merge to read, so it is still named as the one
/// operand it is and the fallback is unchanged.
#[test]
#[cfg(feature = "zod")]
fn test_the_unions_declared_below_the_object_are_still_named_as_one_operand() {
    let nullable = FlatOverLaterNullableUntagged::zod_schema();
    assert!(
        nullable.contains("}).and(z.lazy(() => LaterNullableEither$Schema));"),
        "expected the union named as one operand, got: {nullable}"
    );
    assert!(
        !nullable.contains("z.nullable("),
        "the absence reached the intersection in: {nullable}"
    );
    let slug = FlatOverLaterMemberSlugUntagged::zod_schema();
    assert!(
        slug.contains("}).and(z.lazy(() => LaterMemberSlugEither$Schema));"),
        "expected the union named as one operand, got: {slug}"
    );
    assert!(
        !slug.contains(".and(z.lazy(() => MemberSlug$Schema))"),
        "the brand member reached the intersection in: {slug}"
    );
}

/// Flattening one is refused at expansion rather than emitted — the branch that used to be written
/// for the scalar member intersected with it, which no payload satisfies and nothing reported.
#[test]
#[cfg(feature = "zod")]
fn test_a_union_with_a_scalar_member_declared_below_is_still_named_as_one_operand() {
    let zod = FlatOverLaterScalarUntagged::zod_schema();
    assert!(
        zod.contains("}).and(z.lazy(() => LaterScalarEither$Schema));"),
        "expected the union named as one operand, got: {zod}"
    );
    assert!(
        !zod.contains(".and(z.lazy(() => z.string()))"),
        "the scalar member reached the intersection in: {zod}"
    );
}

/// A member naming a registration whose wire is a JSON array is one serde refuses to flatten for
/// the reason it refuses a directly written array: what it writes is the array its slot writes, and
/// an array carries no keys to put into the object.
#[test]
#[cfg(any(feature = "jsonschema", feature = "zod"))]
fn test_flattening_an_untagged_enum_over_a_named_array_member_is_unserializable() {
    let refusal = serde_json::to_value(FlatOverLaterMemberBagUntagged {
        own: "o".to_owned(),
        either: LaterMemberBagEither::Items(MemberBag(vec!["x".to_owned()])),
    })
    .unwrap_err()
    .to_string();
    assert!(
        refusal.contains("can only flatten structs and maps"),
        "Got: {refusal}"
    );
}

/// So the merge refuses the whole union, naming the array the member describes as — the keyword the
/// registry now carries for the name, where before the one recorded word covered the array and the
/// map alike and could rule neither out.
#[test]
#[cfg(all(feature = "jsonschema", not(feature = "zod")))]
#[should_panic(
    expected = "`FlatOverMemberBagUntagged`: `#[serde(flatten)]` of `MemberBagEither` writes a union member that is not an object — its branch 2 describes a `array`"
)]
fn test_a_named_array_member_of_a_flattened_untagged_enum_is_refused_by_the_merge() {
    assert!(FlatOverMemberBagUntagged::json_schema().is_object());
}

/// And in those same words wherever the union was declared, which is the one reading of the
/// declaration that survives the Zod surface refusing it at expansion.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`FlatOverLaterMemberBagUntagged`: `#[serde(flatten)]` of `LaterMemberBagEither` writes a union member that is not an object — its branch 2 describes a `array`"
)]
fn test_a_named_array_member_of_a_later_flattened_untagged_enum_is_refused_by_the_merge() {
    assert!(FlatOverLaterMemberBagUntagged::json_schema().is_object());
}

/// A member naming a nullable-surfaced registration carries the same null an `Option<T>` member
/// carries, one name away — serde's two directions describe different payload sets exactly as
/// there.
#[test]
#[cfg(any(feature = "jsonschema", feature = "zod"))]
fn test_a_named_nullable_flattened_union_member_is_written_and_then_not_read_back() {
    let written = serde_json::to_value(FlatOverLaterMemberMaybeUntagged {
        own: "o".to_owned(),
        either: LaterMemberMaybeEither::Maybe(MemberMaybeSecond(None)),
    })
    .unwrap();
    assert_eq!(written, serde_json::json!({ "own": "o" }));
    let refusal = serde_json::from_value::<FlatOverLaterMemberMaybeUntagged>(written)
        .unwrap_err()
        .to_string();
    assert!(
        refusal.contains("did not match any variant"),
        "Got: {refusal}"
    );

    let present = serde_json::to_value(FlatOverLaterMemberMaybeUntagged {
        own: "o".to_owned(),
        either: LaterMemberMaybeEither::Maybe(MemberMaybeSecond(Some(FlatSecond { b: true }))),
    })
    .unwrap();
    assert_eq!(present, serde_json::json!({ "own": "o", "b": true }));
    serde_json::from_value::<FlatOverLaterMemberMaybeUntagged>(present).unwrap();
}

/// So the merge refuses it at the leaf the null sits at — `2.2`, a position below the member, which
/// is where the name's own choice puts it and not where the member stands.
#[test]
#[cfg(all(feature = "jsonschema", not(feature = "zod")))]
#[should_panic(
    expected = "`FlatOverMemberMaybeUntagged`: `#[serde(flatten)]` of `MemberMaybeEither` writes a union member that is not an object — its branch 2.2 describes a `null`"
)]
fn test_a_named_nullable_member_of_a_flattened_untagged_enum_is_refused_by_the_merge() {
    assert!(FlatOverMemberMaybeUntagged::json_schema().is_object());
}

#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`FlatOverLaterMemberMaybeUntagged`: `#[serde(flatten)]` of `LaterMemberMaybeEither` writes a union member that is not an object — its branch 2.2 describes a `null`"
)]
fn test_a_named_nullable_member_of_a_later_flattened_untagged_enum_is_refused_by_the_merge() {
    assert!(FlatOverLaterMemberMaybeUntagged::json_schema().is_object());
}

/// The map-shaped member is admitted on every surface, and serde agrees in both directions: it
/// writes the map's keys into the object and reads those same keys back. That is what the array
/// and the map being one recorded word could not tell apart.
#[test]
#[cfg(any(feature = "jsonschema", feature = "zod"))]
fn test_a_named_map_member_of_a_flattened_untagged_enum_round_trips() {
    let written = serde_json::to_value(FlatOverMemberBucketUntagged {
        own: "o".to_owned(),
        either: MemberBucketEither::Bucket(MemberBucket(HashMap::from([(
            "k".to_owned(),
            "v".to_owned(),
        )]))),
    })
    .unwrap();
    assert_eq!(written, serde_json::json!({ "own": "o", "k": "v" }));
    serde_json::from_value::<FlatOverMemberBucketUntagged>(written).unwrap();
}

/// And its Zod schema keeps the multiplication it always had, byte for byte.
#[test]
#[cfg(feature = "zod")]
fn test_a_named_map_member_keeps_its_multiplication() {
    let zod = FlatOverMemberBucketUntagged::zod_schema();
    assert!(
        zod.contains(
            "z.union([\n  FlatOverMemberBucketUntagged$OwnSchema.and(z.lazy(() => FlatFirst$Schema)),\n  FlatOverMemberBucketUntagged$OwnSchema.and(z.lazy(() => MemberBucket$Schema)),\n])"
        ),
        "expected the map member's branch, got: {zod}"
    );
}

/// And the JSON-schema merge writes its document rather than refusing it.
#[test]
#[cfg(feature = "jsonschema")]
fn test_a_named_map_member_keeps_its_merged_document() {
    assert!(FlatOverMemberBucketUntagged::json_schema().is_object());
}

/// Standing on their own the three unions are unions like any other, spelled byte for byte as they
/// were before the merge could tell the shapes apart.
#[test]
#[cfg(feature = "zod")]
fn test_the_named_wire_unions_are_byte_identical_standing_alone() {
    #[cfg(feature = "typescript")]
    const EXPECTED: [&str; 3] = [
        "const MemberBagEither$RawSchema = z.union([FlatFirst$Schema, MemberBag$Schema]);\n\nexport const MemberBagEither$Schema: ZodType<MemberBagEither> = MemberBagEither$RawSchema;",
        "const MemberBucketEither$RawSchema = z.union([FlatFirst$Schema, MemberBucket$Schema]);\n\nexport const MemberBucketEither$Schema: ZodType<MemberBucketEither> = MemberBucketEither$RawSchema;",
        "const MemberMaybeEither$RawSchema = z.union([FlatFirst$Schema, MemberMaybeSecond$Schema]);\n\nexport const MemberMaybeEither$Schema: ZodType<MemberMaybeEither> = MemberMaybeEither$RawSchema;",
    ];
    #[cfg(not(feature = "typescript"))]
    const EXPECTED: [&str; 3] = [
        "export const MemberBagEither$Schema = z.union([FlatFirst$Schema, MemberBag$Schema]);",
        "export const MemberBucketEither$Schema = z.union([FlatFirst$Schema, MemberBucket$Schema]);",
        "export const MemberMaybeEither$Schema = z.union([FlatFirst$Schema, MemberMaybeSecond$Schema]);",
    ];

    assert_eq!(
        [
            MemberBagEither::zod_schema(),
            MemberBucketEither::zod_schema(),
            MemberMaybeEither::zod_schema(),
        ],
        EXPECTED.map(str::to_owned)
    );
}

/// What serde writes for a member naming a tagged enum: the single-key object the variant's name
/// tags for a data-carrying variant, and the variant's name as a key holding null for a unit one —
/// the flattened form of the bare string that variant is written as.
#[test]
#[cfg(any(feature = "jsonschema", feature = "zod"))]
fn test_a_tagged_enum_member_writes_the_unit_variant_as_a_bare_name() {
    let forms = [
        (
            MemberExtBare::Bare,
            serde_json::json!({ "own": "o", "Bare": null }),
        ),
        (
            MemberExtBare::Wrapped(FlatSecond { b: true }),
            serde_json::json!({ "own": "o", "Wrapped": { "b": true } }),
        ),
    ];
    for (variant, expected) in forms {
        let written = serde_json::to_value(FlatOverLaterMemberExtBareUntagged {
            own: "o".to_owned(),
            either: LaterMemberExtBareEither::Ext(variant),
        })
        .unwrap();
        assert_eq!(written, expected);
    }
}

/// And the merge refuses the declaration at the leaf the bare string sits at — `2.1`, a position
/// below the member, which is where the enum's own choice puts it and not where the member stands.
#[test]
#[cfg(all(feature = "jsonschema", not(feature = "zod")))]
#[should_panic(
    expected = "`FlatOverMemberExtBareUntagged`: `#[serde(flatten)]` of `MemberExtBareEither` writes a union member that is not an object — its branch 2.1 describes a `string`"
)]
fn test_a_tagged_enum_member_of_a_flattened_untagged_enum_is_refused_by_the_merge() {
    assert!(FlatOverMemberExtBareUntagged::json_schema().is_object());
}

/// And in those same words wherever the union was declared, which is the one reading of the
/// declaration that survives the Zod surface refusing it at expansion.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`FlatOverLaterMemberExtBareUntagged`: `#[serde(flatten)]` of `LaterMemberExtBareEither` writes a union member that is not an object — its branch 2.1 describes a `string`"
)]
fn test_a_tagged_enum_member_of_a_later_flattened_untagged_enum_is_refused_by_the_merge() {
    assert!(FlatOverLaterMemberExtBareUntagged::json_schema().is_object());
}

/// A tagged enum whose every variant carries data writes an object for each of them, so serde joins
/// it to the object being written and reads it back — and every surface admits it exactly where it
/// always did.
#[test]
#[cfg(any(feature = "jsonschema", feature = "zod"))]
fn test_an_all_object_tagged_enum_member_round_trips() {
    let written = serde_json::to_value(FlatOverMemberExtObjUntagged {
        own: "o".to_owned(),
        either: MemberExtObjEither::Ext(MemberExtObjects::One(FlatFirst { a: "x".to_owned() })),
    })
    .unwrap();
    assert_eq!(
        written,
        serde_json::json!({ "own": "o", "One": { "a": "x" } })
    );
    serde_json::from_value::<FlatOverMemberExtObjUntagged>(written).unwrap();
}

/// And its Zod schema keeps the multiplication it always had, byte for byte: one branch per member
/// of the union, with the tagged enum named as the one operand it is rather than written out once
/// per variant.
#[test]
#[cfg(feature = "zod")]
fn test_an_all_object_tagged_enum_member_keeps_its_multiplication() {
    let zod = FlatOverMemberExtObjUntagged::zod_schema();
    assert!(
        zod.contains(
            "z.union([\n  FlatOverMemberExtObjUntagged$OwnSchema.and(z.lazy(() => FlatFirst$Schema)),\n  FlatOverMemberExtObjUntagged$OwnSchema.and(z.lazy(() => MemberExtObjects$Schema)),\n])"
        ),
        "expected the tagged enum's branch, got: {zod}"
    );
}

/// And the JSON-schema merge writes its document rather than refusing it.
#[test]
#[cfg(feature = "jsonschema")]
fn test_an_all_object_tagged_enum_member_keeps_its_merged_document() {
    assert!(FlatOverMemberExtObjUntagged::json_schema().is_object());
}

/// Standing on their own the two tagged enums are written exactly as they were: the leaves are what
/// a merge reads about them and say nothing about what either publishes.
#[test]
#[cfg(feature = "zod")]
fn test_the_tagged_enums_are_byte_identical_standing_alone() {
    #[cfg(feature = "typescript")]
    const EXPECTED: [&str; 2] = [
        "const MemberExtBare$RawSchema = z.union([z.literal(\"Bare\"), z.strictObject({\n  \"Wrapped\": FlatSecond$Schema,\n})]);\n\nexport const MemberExtBare$Schema: ZodType<MemberExtBare> = MemberExtBare$RawSchema;",
        "const MemberExtObjects$RawSchema = z.union([z.strictObject({\n  \"One\": FlatFirst$Schema,\n}), z.strictObject({\n  \"Two\": FlatSecond$Schema,\n})]);\n\nexport const MemberExtObjects$Schema: ZodType<MemberExtObjects> = MemberExtObjects$RawSchema;",
    ];
    #[cfg(not(feature = "typescript"))]
    const EXPECTED: [&str; 2] = [
        "export const MemberExtBare$Schema = z.union([z.literal(\"Bare\"), z.strictObject({\n  \"Wrapped\": FlatSecond$Schema,\n})]);",
        "export const MemberExtObjects$Schema = z.union([z.strictObject({\n  \"One\": FlatFirst$Schema,\n}), z.strictObject({\n  \"Two\": FlatSecond$Schema,\n})]);",
    ];

    assert_eq!(
        [MemberExtBare::zod_schema(), MemberExtObjects::zod_schema(),],
        EXPECTED.map(str::to_owned)
    );
}

/// The direct position asks the same round-trip question and gets the other answer: serde writes
/// the unit variant as its name holding `null`, not the bare string it is standing alone, and
/// reads that back as the variant.
#[test]
#[cfg(any(feature = "jsonschema", feature = "zod"))]
fn test_a_directly_flattened_tagged_enum_round_trips_every_variant() {
    let forms = [
        (
            MemberExtBare::Bare,
            serde_json::json!({ "Bare": null, "own": "o" }),
        ),
        (
            MemberExtBare::Wrapped(FlatSecond { b: true }),
            serde_json::json!({ "Wrapped": { "b": true }, "own": "o" }),
        ),
    ];
    for (ext, expected) in forms {
        let holder = ExtBareDirectHolder {
            ext,
            own: "o".to_owned(),
        };
        let written = serde_json::to_value(&holder).unwrap();
        assert_eq!(written, expected);
        let back: ExtBareDirectHolder = serde_json::from_value(written).unwrap();
        assert_eq!(back, holder);
    }
}

/// So the document distributes the object over the variants rather than refusing at the branch the
/// bare string sits at: one branch per variant, the unit one carrying the single key serde writes
/// for it.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_direct_tagged_flatten_document_writes_one_branch_per_variant() {
    assert_eq!(
        serde_json::to_string(&ExtBareDirectHolder::json_schema()).unwrap(),
        r#"{"type":"object","oneOf":[{"type":"object","properties":{"own":{"type":"string"},"Bare":{"type":"null"}},"required":["own","Bare"],"additionalProperties":false},{"type":"object","properties":{"own":{"type":"string"},"Wrapped":{"type":"object","additionalProperties":false,"properties":{"b":{"type":"boolean"}},"required":["b"]}},"required":["own","Wrapped"],"additionalProperties":false}]}"#
    );
}

/// And it admits exactly the payloads serde writes, and no payload carrying two variants at once or
/// none.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_direct_tagged_flatten_document_admits_the_captured_payloads() {
    let schema = ExtBareDirectHolder::json_schema();
    for payload in [
        serde_json::json!({ "Bare": null, "own": "o" }),
        serde_json::json!({ "Wrapped": { "b": true }, "own": "o" }),
    ] {
        assert!(
            closed_document_accepts(&schema, &payload),
            "{payload} is rejected by {schema}"
        );
    }
    for payload in [
        serde_json::json!({ "own": "o" }),
        serde_json::json!({ "Bare": null, "Wrapped": { "b": true }, "own": "o" }),
        serde_json::json!({ "Bare": null }),
    ] {
        assert!(
            !closed_document_accepts(&schema, &payload),
            "{payload} is accepted by {schema}"
        );
    }
}

/// And Zod multiplies the object over the same variants, each written in the spelling a merge joins
/// rather than the one the union publishes: the unit variant is the key serde writes for it, not the
/// literal the enum's own schema names.
#[test]
#[cfg(feature = "zod")]
fn test_the_direct_tagged_flatten_schema_multiplies_the_object_over_the_variants() {
    let zod = ExtBareDirectHolder::zod_schema();
    assert!(
        zod.contains(
            "z.union([\n  ExtBareDirectHolder$OwnSchema.and(z.lazy(() => z.strictObject({\n  \"Bare\": z.null(),\n}))),\n  ExtBareDirectHolder$OwnSchema.and(z.lazy(() => z.strictObject({\n  \"Wrapped\": FlatSecond$Schema,\n}))),\n])"
        ),
        "expected one operand per variant, got: {zod}"
    );
    assert!(
        !zod.contains("MemberExtBare$Schema"),
        "the union is named as an operand in: {zod}"
    );
}

/// And TypeScript spells the same two key sets, because it cannot reach them by distributing: the
/// bare string a unit variant publishes standing alone intersects the object to `never`, and the
/// payload serde writes for that variant belongs to no branch of the result.
#[test]
#[cfg(feature = "typescript")]
fn test_the_direct_tagged_flatten_type_spells_the_key_set_serde_writes() {
    let ts = ExtBareDirectHolder::ts_definition();
    assert!(
        ts.contains(
            "} & ({\n  /**\n   * Bare\n   * \n   */\n  \"Bare\": null;\n  \"Wrapped\"?: never;\n} | {\n  /**\n   * Wrapped\n   * \n   */\n  \"Wrapped\": FlatSecond;\n  \"Bare\"?: never;\n});"
        ),
        "expected one key set per variant, got: {ts}"
    );
    assert!(
        !ts.contains("& MemberExtBare"),
        "the union is named as an operand in: {ts}"
    );
}

/// The same enum every variant of which carries data. serde writes each as the object its name tags
/// and reads both back, so the declaration is one all four surfaces admit.
#[test]
#[cfg(any(feature = "jsonschema", feature = "zod", feature = "typescript"))]
fn test_a_directly_flattened_all_object_tagged_enum_round_trips_every_variant() {
    let forms = [
        (
            MemberExtObjects::One(FlatFirst { a: "x".to_owned() }),
            serde_json::json!({ "One": { "a": "x" }, "own": "o" }),
        ),
        (
            MemberExtObjects::Two(FlatSecond { b: true }),
            serde_json::json!({ "Two": { "b": true }, "own": "o" }),
        ),
    ];
    for (ext, expected) in forms {
        let holder = ExtObjDirectHolder {
            ext,
            own: "o".to_owned(),
        };
        let written = serde_json::to_value(&holder).unwrap();
        assert_eq!(written, expected);
        let back: ExtObjDirectHolder = serde_json::from_value(written).unwrap();
        assert_eq!(back, holder);
    }
}

/// So Zod multiplies the object over those variants too. Nothing about them has to be proved: an
/// intersection recognizes exactly the keys its operands name and a `z.union` names none, so the
/// object joined to the union as one operand describes a payload set no value inhabits.
#[test]
#[cfg(feature = "zod")]
fn test_the_all_object_tagged_direct_flatten_schema_multiplies_the_object_over_the_variants() {
    let zod = ExtObjDirectHolder::zod_schema();
    assert!(
        zod.contains(
            "z.union([\n  ExtObjDirectHolder$OwnSchema.and(z.lazy(() => z.strictObject({\n  \"One\": FlatFirst$Schema,\n}))),\n  ExtObjDirectHolder$OwnSchema.and(z.lazy(() => z.strictObject({\n  \"Two\": FlatSecond$Schema,\n}))),\n])"
        ),
        "expected one operand per variant, got: {zod}"
    );
    assert!(
        !zod.contains("MemberExtObjects$Schema"),
        "the union is named as an operand in: {zod}"
    );
}

/// TypeScript distributes the intersection over the union on its own, but the excess-property
/// check reads the union of both key sets — a payload carrying both tags would satisfy either
/// branch structurally, which no value produces. Spelling the variants is what rules that out.
#[test]
#[cfg(feature = "typescript")]
fn test_an_all_object_tagged_direct_flatten_type_closes_each_variant_against_the_other() {
    let ts = ExtObjDirectHolder::ts_definition();
    let declared = &ts[ts.find("export type").unwrap()..];
    assert_eq!(
        declared,
        "export type ExtObjDirectHolder = {\n  /**\n   * own\n   * \n   */\n  own: string;\n} & ({\n  /**\n   * One\n   * \n   */\n  \"One\": FlatFirst;\n  \"Two\"?: never;\n} | {\n  /**\n   * Two\n   * \n   */\n  \"Two\": FlatSecond;\n  \"One\"?: never;\n});"
    );
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_an_all_object_tagged_direct_flatten_document_is_byte_identical() {
    assert_eq!(
        serde_json::to_string(&ExtObjDirectHolder::json_schema()).unwrap(),
        r#"{"type":"object","oneOf":[{"type":"object","properties":{"own":{"type":"string"},"One":{"type":"object","additionalProperties":false,"properties":{"a":{"type":"string"}},"required":["a"]}},"required":["own","One"],"additionalProperties":false},{"type":"object","properties":{"own":{"type":"string"},"Two":{"type":"object","additionalProperties":false,"properties":{"b":{"type":"boolean"}},"required":["b"]}},"required":["own","Two"],"additionalProperties":false}]}"#
    );
}

/// The absence branch beside the same enum reads the closed variants' keys, where it had none to
/// read: `keyof` a union is the keys its branches share, which before the exclusions was an empty
/// mapped type `{}` that every object passes through.
#[test]
#[cfg(feature = "typescript")]
fn test_the_optional_all_object_tagged_flatten_type_names_the_keys_its_absence_leaves_out() {
    let ts = OptExtObjHolder::ts_definition();
    let declared = &ts[ts.find("export type").unwrap()..];
    assert_eq!(
        declared,
        "export type OptExtObjHolder = {\n  /**\n   * own\n   * \n   */\n  own: string;\n} & (({\n  /**\n   * One\n   * \n   */\n  \"One\": FlatFirst;\n  \"Two\"?: never;\n} | {\n  /**\n   * Two\n   * \n   */\n  \"Two\": FlatSecond;\n  \"One\"?: never;\n}) | { [K in keyof ({\n  /**\n   * One\n   * \n   */\n  \"One\": FlatFirst;\n  \"Two\"?: never;\n} | {\n  /**\n   * Two\n   * \n   */\n  \"Two\": FlatSecond;\n  \"One\"?: never;\n})]?: never });"
    );
}

/// Standing on their own the two tagged enums are written exactly as they were on TypeScript too.
/// The exclusions answer what an intersection with an open object does to the choice, and a name
/// publishing the choice alone is joined to nothing: it describes one variant at a time already.
#[test]
#[cfg(feature = "typescript")]
fn test_the_tagged_enums_are_byte_identical_standing_alone_on_typescript() {
    let bare = MemberExtBare::ts_definition();
    let objects = MemberExtObjects::ts_definition();
    assert_eq!(
        [
            &bare[bare.find("export type").unwrap()..],
            &objects[objects.find("export type").unwrap()..],
        ],
        [
            "export type MemberExtBare =   /**\n   * Bare\n   * \n   */\n  \"Bare\" | {\n  /**\n   * Wrapped\n   * \n   */\n  \"Wrapped\": FlatSecond;\n};",
            "export type MemberExtObjects = {\n  /**\n   * One\n   * \n   */\n  \"One\": FlatFirst;\n} | {\n  /**\n   * Two\n   * \n   */\n  \"Two\": FlatSecond;\n};",
        ]
    );
}

/// The untagged footing answers the same way, on the members whose keys the declaration spells. What
/// serde writes is one member's keys at a time — it matches a member on its shape — and the merged
/// type said otherwise for the reason the tagged one did.
#[test]
#[cfg(any(feature = "jsonschema", feature = "zod", feature = "typescript"))]
fn test_a_directly_flattened_inline_untagged_union_round_trips_every_member() {
    let forms = [
        (
            InlineUntagEither::Left {
                left: "l".to_owned(),
            },
            serde_json::json!({ "left": "l", "own": "o" }),
        ),
        (
            InlineUntagEither::Right { right: true },
            serde_json::json!({ "right": true, "own": "o" }),
        ),
    ];
    for (either, expected) in forms {
        let holder = InlineUntagDirectHolder {
            either,
            own: "o".to_owned(),
        };
        let written = serde_json::to_value(&holder).unwrap();
        assert_eq!(written, expected);
        let back: InlineUntagDirectHolder = serde_json::from_value(written).unwrap();
        assert_eq!(back, holder);
    }
}

/// So the merged type spells the members in the union's name's place, each closed against the keys
/// the other names.
#[test]
#[cfg(feature = "typescript")]
fn test_an_inline_untagged_direct_flatten_type_closes_each_member_against_the_other() {
    let ts = InlineUntagDirectHolder::ts_definition();
    let declared = &ts[ts.find("export type").unwrap()..];
    assert_eq!(
        declared,
        "export type InlineUntagDirectHolder = {\n  /**\n   * own\n   * \n   */\n  own: string;\n} & ({ left: string; right?: never } | { right: boolean; left?: never });"
    );
}

/// And the union standing alone is written exactly as it was, on every surface: the exclusions are
/// what an intersection with an open object needs, and nothing joins the name here.
#[test]
#[cfg(feature = "typescript")]
fn test_the_inline_untagged_union_is_byte_identical_standing_alone() {
    let ts = InlineUntagEither::ts_definition();
    let declared = &ts[ts.find("export type").unwrap()..];
    assert_eq!(
        declared,
        "export type InlineUntagEither = { left: string } | { right: boolean };"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_the_inline_untagged_direct_flatten_schema_is_unchanged() {
    let zod = InlineUntagDirectHolder::zod_schema();
    assert!(
        zod.contains(
            "z.union([\n  InlineUntagDirectHolder$OwnSchema.and(z.lazy(() => z.strictObject({ left: z.string(), }))),\n  InlineUntagDirectHolder$OwnSchema.and(z.lazy(() => z.strictObject({ right: z.boolean(), }))),\n])"
        ),
        "expected one operand per member, got: {zod}"
    );
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_the_inline_untagged_direct_flatten_document_is_unchanged() {
    assert_eq!(
        serde_json::to_string(&InlineUntagDirectHolder::json_schema()).unwrap(),
        r#"{"type":"object","anyOf":[{"type":"object","properties":{"own":{"type":"string"},"left":{"type":"string"}},"required":["own","left"],"additionalProperties":false},{"type":"object","properties":{"own":{"type":"string"},"right":{"type":"boolean"}},"required":["own","right"],"additionalProperties":false}]}"#
    );
}

/// What serde writes for a `#[serde(flatten)]` field of an enum's own struct variant: the source's
/// members sit in the variant's content object, under no key of their own — the same merge a
/// struct's own flattened field gets, one level deeper.
#[test]
fn test_a_flattened_variant_field_writes_its_members_into_the_variants_content() {
    assert_eq!(
        serde_json::to_value(ExternalFlatVariant::Named {
            extra: VariantExtra {
                note: "n".to_owned()
            },
            x: "y".to_owned(),
        })
        .unwrap(),
        serde_json::json!({ "Named": { "note": "n", "x": "y" } })
    );
    assert_eq!(
        serde_json::to_value(InternalFlatVariant::Named {
            extra: VariantExtra {
                note: "n".to_owned()
            },
            x: "y".to_owned(),
        })
        .unwrap(),
        serde_json::json!({ "kind": "Named", "note": "n", "x": "y" })
    );
    assert_eq!(
        serde_json::to_value(AdjacentFlatVariant::Named {
            extra: VariantExtra {
                note: "n".to_owned()
            },
            x: "y".to_owned(),
        })
        .unwrap(),
        serde_json::json!({ "kind": "Named", "data": { "note": "n", "x": "y" } })
    );
    assert_eq!(
        serde_json::to_value(TwiceFlatVariant::Named {
            extra: VariantExtra {
                note: "n".to_owned()
            },
            rank: VariantRank { rank: 3 },
            x: "y".to_owned(),
        })
        .unwrap(),
        serde_json::json!({ "Named": { "note": "n", "rank": 3_i64, "x": "y" } })
    );
}

/// And reads them back the same way, so the merged shape is what a payload must carry in both
/// directions.
#[test]
fn test_a_flattened_variant_field_reads_back_from_the_merged_shape() {
    let external: ExternalFlatVariant =
        serde_json::from_value(serde_json::json!({ "Named": { "note": "n", "x": "y" } })).unwrap();
    assert_eq!(
        external,
        ExternalFlatVariant::Named {
            extra: VariantExtra {
                note: "n".to_owned()
            },
            x: "y".to_owned(),
        }
    );
    let internal: InternalFlatVariant =
        serde_json::from_value(serde_json::json!({ "kind": "Named", "note": "n", "x": "y" }))
            .unwrap();
    assert_eq!(
        internal,
        InternalFlatVariant::Named {
            extra: VariantExtra {
                note: "n".to_owned()
            },
            x: "y".to_owned(),
        }
    );
}

/// So the TypeScript the variant describes as is an intersection at the content's own position,
/// never a key holding the source.
#[test]
#[cfg(feature = "typescript")]
fn test_a_flattened_variant_field_is_a_typescript_intersection_inside_the_variant() {
    let external = ExternalFlatVariant::ts_definition();
    assert!(external.contains("} & VariantExtra;"), "Got: {external}");
    assert!(!external.contains("extra:"), "Got: {external}");
    assert!(external.contains("y: string;"), "Got: {external}");

    let internal = InternalFlatVariant::ts_definition();
    assert!(internal.contains("} & VariantExtra"), "Got: {internal}");
    assert!(!internal.contains("extra:"), "Got: {internal}");

    let adjacent = AdjacentFlatVariant::ts_definition();
    assert!(adjacent.contains("} & VariantExtra;"), "Got: {adjacent}");
    assert!(!adjacent.contains("extra:"), "Got: {adjacent}");
}

#[test]
#[cfg(feature = "typescript")]
fn test_two_flattened_variant_fields_join_the_same_content_object() {
    let ts = TwiceFlatVariant::ts_definition();
    assert!(ts.contains("} & VariantExtra & VariantRank;"), "Got: {ts}");
    assert!(!ts.contains("rank:"), "Got: {ts}");
}

/// And the Zod schema joins the source through the same deferred operand a struct's own flattened
/// base joins through, so a source declared below the enum is still read when something validates.
#[test]
#[cfg(feature = "zod")]
fn test_a_flattened_variant_field_is_a_zod_intersection_inside_the_variant() {
    for zod in [
        ExternalFlatVariant::zod_schema(),
        InternalFlatVariant::zod_schema(),
        AdjacentFlatVariant::zod_schema(),
    ] {
        assert!(
            zod.contains("}).and(z.lazy(() => VariantExtra$Schema))"),
            "Got: {zod}"
        );
        assert!(!zod.contains("extra:"), "Got: {zod}");
        assert!(
            !zod.contains(".and(VariantExtra$Schema)"),
            "the source is read while the const initializes: {zod}"
        );
    }
}

#[test]
#[cfg(feature = "zod")]
fn test_two_flattened_variant_fields_chain_their_zod_operands() {
    let zod = TwiceFlatVariant::zod_schema();
    assert!(
        zod.contains(
            "}).and(z.lazy(() => VariantExtra$Schema)).and(z.lazy(() => VariantRank$Schema))"
        ),
        "Got: {zod}"
    );
}

/// An internally tagged member that flattens is an intersection rather than an object, and Zod
/// discriminates only between objects — so the union carrying it is a plain one.
#[test]
#[cfg(feature = "zod")]
fn test_an_internally_tagged_flattened_variant_forces_a_plain_zod_union() {
    let zod = InternalFlatVariant::zod_schema();
    assert!(zod.contains("z.union(["), "Got: {zod}");
    assert!(!zod.contains("z.discriminatedUnion("), "Got: {zod}");
}

/// The JSON document names the source's members where the variant's own members are named, and
/// requires them beside its own — no key stands for the flattened field itself.
#[test]
#[cfg(feature = "jsonschema")]
fn test_a_flattened_variant_field_merges_into_the_variants_json_content() {
    for document in [
        ExternalFlatVariant::json_schema(),
        InternalFlatVariant::json_schema(),
        AdjacentFlatVariant::json_schema(),
    ] {
        let content = variant_content(&document, "Named");
        let (named, required) = described_keys(&content);
        assert!(named.contains(&"note".to_owned()), "Got: {content}");
        assert!(named.contains(&"x".to_owned()), "Got: {content}");
        assert!(!named.contains(&"extra".to_owned()), "Got: {content}");
        assert!(required.contains(&"note".to_owned()), "Got: {content}");
        assert!(required.contains(&"x".to_owned()), "Got: {content}");
        assert_eq!(content["additionalProperties"], serde_json::json!(false));
    }
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_two_flattened_variant_fields_merge_into_one_json_content() {
    let content = variant_content(&TwiceFlatVariant::json_schema(), "Named");
    let (named, required) = described_keys(&content);
    for key in ["note", "rank", "x"] {
        assert!(named.contains(&key.to_owned()), "Got: {content}");
        assert!(required.contains(&key.to_owned()), "Got: {content}");
    }
}

/// And the document accepts exactly the payload serde writes.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_flattened_variant_document_accepts_what_serde_writes() {
    let cases = [
        (
            ExternalFlatVariant::json_schema(),
            serde_json::to_value(ExternalFlatVariant::Named {
                extra: VariantExtra {
                    note: "n".to_owned(),
                },
                x: "y".to_owned(),
            })
            .unwrap(),
        ),
        (
            InternalFlatVariant::json_schema(),
            serde_json::to_value(InternalFlatVariant::Named {
                extra: VariantExtra {
                    note: "n".to_owned(),
                },
                x: "y".to_owned(),
            })
            .unwrap(),
        ),
    ];
    for (document, payload) in cases {
        assert!(
            closed_document_accepts(&document, &payload),
            "{document} rejected {payload}"
        );
    }
}

/// A variant carrying no flattened field is untouched: its content stays the object it always was,
/// with a key per declared field and no intersection anywhere.
#[test]
#[cfg(feature = "typescript")]
fn test_a_variant_without_a_flattened_field_keeps_its_typescript_object() {
    let ts = ExternalFlatVariant::ts_definition();
    let declared = &ts[ts.find("export type").unwrap()..];
    let plain = &declared[declared.find("\"Plain\"").unwrap()..];
    assert!(!plain.contains(" & "), "Got: {plain}");
}

#[test]
#[cfg(feature = "zod")]
fn test_a_variant_without_a_flattened_field_keeps_its_zod_object() {
    let zod = AdjacentFlatVariant::zod_schema();
    let plain = &zod[zod.find("z.literal(\"Plain\")").unwrap()..];
    assert!(!plain.contains(".and("), "Got: {plain}");
}

/// What serde writes for a `#[serde(flatten)]` field of an *untagged* enum's own struct variant:
/// the source's members sit beside the variant's own, under no key of their own and with no
/// discriminator over them.
#[test]
fn test_a_flattened_untagged_variant_field_writes_its_members_beside_the_variants_own() {
    assert_eq!(
        serde_json::to_value(UntaggedFlatVariant::Named {
            extra: VariantExtra {
                note: "n".to_owned()
            },
            x: "y".to_owned(),
        })
        .unwrap(),
        serde_json::json!({ "note": "n", "x": "y" })
    );
    assert_eq!(
        serde_json::to_value(TwiceUntaggedFlatVariant::Named {
            extra: VariantExtra {
                note: "n".to_owned()
            },
            rank: VariantRank { rank: 3 },
            x: "y".to_owned(),
        })
        .unwrap(),
        serde_json::json!({ "note": "n", "rank": 3_i64, "x": "y" })
    );
}

/// And reads them back the same way, for the flattening member and for the sibling beside it.
#[test]
fn test_a_flattened_untagged_variant_field_round_trips_both_members() {
    for value in [
        UntaggedFlatVariant::Named {
            extra: VariantExtra {
                note: "n".to_owned(),
            },
            x: "y".to_owned(),
        },
        UntaggedFlatVariant::Plain { y: "p".to_owned() },
    ] {
        let written = serde_json::to_value(&value).unwrap();
        let back: UntaggedFlatVariant = serde_json::from_value(written).unwrap();
        assert_eq!(back, value);
    }
}

/// So the TypeScript the member describes as is an intersection at the member's own position, never
/// a key holding the source.
#[test]
#[cfg(feature = "typescript")]
fn test_a_flattened_untagged_variant_field_is_a_typescript_intersection_inside_the_member() {
    let ts = UntaggedFlatVariant::ts_definition();
    let declared = &ts[ts.find("export type").unwrap()..];
    assert_eq!(
        declared,
        "export type UntaggedFlatVariant = { x: string } & VariantExtra | { y: string };"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_two_flattened_untagged_variant_fields_join_the_same_member_object() {
    let ts = TwiceUntaggedFlatVariant::ts_definition();
    assert!(
        ts.contains("{ x: string } & VariantExtra & VariantRank;"),
        "Got: {ts}"
    );
    assert!(!ts.contains("rank:"), "Got: {ts}");
}

/// And the Zod schema joins the source through the same deferred operand the tagged forms join
/// through, so a source declared below the enum is still read when something validates.
#[test]
#[cfg(feature = "zod")]
fn test_a_flattened_untagged_variant_field_is_a_zod_intersection_inside_the_member() {
    let zod = UntaggedFlatVariant::zod_schema();
    assert!(
        zod.contains("}).and(z.lazy(() => VariantExtra$Schema))"),
        "Got: {zod}"
    );
    assert!(!zod.contains("extra:"), "Got: {zod}");
    assert!(
        !zod.contains(".and(VariantExtra$Schema)"),
        "the source is read while the const initializes: {zod}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_two_flattened_untagged_variant_fields_chain_their_zod_operands() {
    let zod = TwiceUntaggedFlatVariant::zod_schema();
    assert!(
        zod.contains(
            "}).and(z.lazy(() => VariantExtra$Schema)).and(z.lazy(() => VariantRank$Schema))"
        ),
        "Got: {zod}"
    );
}

/// The keys `payload` carries, sorted, beside the keys `branch` names and the keys it requires.
#[cfg(feature = "jsonschema")]
fn written_against_described(
    branch: &serde_json::Value,
    payload: &serde_json::Value,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut written: Vec<String> = payload.as_object().unwrap().keys().cloned().collect();
    written.sort();
    let (mut named, mut required) = described_keys(branch);
    named.sort();
    required.sort();
    (written, named, required)
}

/// The JSON document names exactly the keys serde writes for the member, and requires every one of
/// them: the source's members sit where the variant's own are named, and no key stands for the
/// flattened field itself.
#[test]
#[cfg(feature = "jsonschema")]
fn test_a_flattened_untagged_variant_field_merges_into_the_members_json_object() {
    let payload = serde_json::to_value(UntaggedFlatVariant::Named {
        extra: VariantExtra {
            note: "n".to_owned(),
        },
        x: "y".to_owned(),
    })
    .unwrap();
    let branch = untagged_branch(&UntaggedFlatVariant::json_schema(), "x");
    let (written, named, required) = written_against_described(&branch, &payload);
    assert_eq!(named, written, "Got: {branch}");
    assert_eq!(required, written, "Got: {branch}");
    assert_eq!(branch["additionalProperties"], serde_json::json!(false));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_two_flattened_untagged_variant_fields_merge_into_one_json_object() {
    let payload = serde_json::to_value(TwiceUntaggedFlatVariant::Named {
        extra: VariantExtra {
            note: "n".to_owned(),
        },
        rank: VariantRank { rank: 3 },
        x: "y".to_owned(),
    })
    .unwrap();
    let branch = untagged_branch(&TwiceUntaggedFlatVariant::json_schema(), "x");
    let (written, named, required) = written_against_described(&branch, &payload);
    assert_eq!(named, written, "Got: {branch}");
    assert_eq!(required, written, "Got: {branch}");
}

/// And exactly one branch of the union accepts each member's real payload: merging a source into
/// one member leaves the others describing what they always described.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_flattened_untagged_variant_document_accepts_what_serde_writes() {
    let document = UntaggedFlatVariant::json_schema();
    for value in [
        UntaggedFlatVariant::Named {
            extra: VariantExtra {
                note: "n".to_owned(),
            },
            x: "y".to_owned(),
        },
        UntaggedFlatVariant::Plain { y: "p".to_owned() },
    ] {
        let payload = serde_json::to_value(&value).unwrap();
        assert_eq!(
            accepting_branches(&document["anyOf"], &payload),
            1,
            "{document} on {payload}"
        );
    }
}

/// The nested shape the description named before the merge is one serde never writes, and the
/// document turns it away.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_flattened_untagged_variant_document_rejects_the_stale_nested_shape() {
    let document = UntaggedFlatVariant::json_schema();
    assert!(
        !closed_document_accepts(
            &document,
            &serde_json::json!({ "extra": { "note": "n" }, "x": "y" })
        ),
        "Got: {document}"
    );
}

/// An object that flattens the enum writes the matched member's keys beside its own, in both
/// directions.
#[test]
fn test_flattening_an_enum_whose_member_flattens_round_trips_every_member() {
    let forms = [
        (
            UntaggedFlatVariant::Named {
                extra: VariantExtra {
                    note: "n".to_owned(),
                },
                x: "y".to_owned(),
            },
            serde_json::json!({ "note": "n", "x": "y", "own": "o" }),
        ),
        (
            UntaggedFlatVariant::Plain { y: "p".to_owned() },
            serde_json::json!({ "y": "p", "own": "o" }),
        ),
    ];
    for (either, expected) in forms {
        let holder = UntaggedFlatVariantHolder {
            either,
            own: "o".to_owned(),
        };
        let written = serde_json::to_value(&holder).unwrap();
        assert_eq!(written, expected);
        let back: UntaggedFlatVariantHolder = serde_json::from_value(written).unwrap();
        assert_eq!(back, holder);
    }
}

/// And no member of that union is closed against a key it cannot enumerate: the flattening member's
/// own key list is not provable from one expansion, so its sibling is told to deny nothing and the
/// merge falls back to the one operand the enum's name is.
#[test]
#[cfg(feature = "typescript")]
fn test_a_flattening_untagged_member_closes_no_sibling_against_unprovable_keys() {
    let ts = UntaggedFlatVariantHolder::ts_definition();
    assert!(!ts.contains("?: never"), "Got: {ts}");
    assert!(ts.contains("} & UntaggedFlatVariant;"), "Got: {ts}");
    assert!(
        UntaggedFlatVariant::ts_definition().contains("| { y: string };"),
        "Got: {}",
        UntaggedFlatVariant::ts_definition()
    );
}
