//! `#[model_schema()]` on an item that declares parameters.
//!
//! Three questions run through every fixture here. Does the emitted `impl` still name the type the
//! author declared — every parameter it binds, lifetimes and consts included. Does the TypeScript
//! declaration bind the parameters its fields are written under. And does a field typed with one
//! render as that parameter on the type surface while describing as the opaque value on the two
//! validating ones, which publish one schema for every instantiation and so can say nothing else
//! about it.
//!
//! A plain enum — all unit variants — cannot bind a *type* parameter at all: Rust refuses the
//! declaration for an unused parameter before the attribute is reached. What it can bind is a
//! const, which `PlainConst` carries, and that is the plain-enum shape of the same question: the
//! `impl` has to repeat the const while the declaration names nothing.
//!
//! Every fixture binding a type parameter declares a default type for it, which is what a build
//! generating a JSON document requires. Nothing here reads that declaration yet, so the surfaces
//! pinned below are the ones the undeclared fixtures pinned — which is the whole of the evidence
//! that declaring a default changes no emission.

#[cfg(feature = "typescript")]
mod typescript {
    use super::{
        Adjacent, External, FolderTree, Holder, Internal, KeyedByParameter, LifetimeStruct, Pair,
        PlainConst, Positional, Untagged, WireFolder, Wrapper,
    };

    /// TypeScript is a type surface and the declaration binds the parameter for real, so the key
    /// stays the name the author wrote where the two validating surfaces erase it.
    #[test]
    fn a_parameter_keyed_map_keeps_the_parameter_on_the_type_surface() {
        let ts = KeyedByParameter::<String, u32>::ts_definition();
        assert!(
            ts.contains("export type KeyedByParameter<KeyType, ValueType> = {"),
            "Got: {ts}"
        );
        assert!(
            ts.contains("  parameter_keyed: Partial<Record<KeyType, string>>;"),
            "Got: {ts}"
        );
        assert!(
            ts.contains("  both_parameters: Partial<Record<KeyType, ValueType>>;"),
            "Got: {ts}"
        );
        assert!(
            ts.contains("  concrete_string_key: Partial<Record<string, string>>;"),
            "Got: {ts}"
        );
        assert!(
            ts.contains("  stringified_number_key: Partial<Record<number, string>>;"),
            "Got: {ts}"
        );
    }

    /// The declaration binds what the fields under it are written with, so a field typed with a
    /// parameter is that parameter and not a reference to a generated type of the same name.
    #[test]
    fn a_struct_declaration_binds_its_parameters() {
        let ts = Wrapper::<String>::ts_definition();
        assert!(ts.contains("export type Wrapper<IdType> = {"), "Got: {ts}");
        assert!(ts.contains("  id: IdType;"), "Got: {ts}");
        assert!(ts.contains("  children: Array<IdType>;"), "Got: {ts}");
        assert!(ts.contains("  name: string;"), "Got: {ts}");
        assert!(!ts.contains("IdType$Schema"), "Got: {ts}");
    }

    /// Every parameter is bound, in declaration order, and each is reached through whatever shape
    /// the field was written around it.
    #[test]
    fn a_parameter_is_reached_through_the_shape_it_was_written_under() {
        let ts = Pair::<String, u32>::ts_definition();
        assert!(
            ts.contains("export type Pair<KeyType, ValueType> = {"),
            "Got: {ts}"
        );
        assert!(ts.contains("  key: KeyType;"), "Got: {ts}");
        assert!(
            ts.contains("  by_key: Partial<Record<string, ValueType>>;"),
            "Got: {ts}"
        );
        // serde drops the key for a `None`, which every build reads off the attribute on the
        // field.
        assert!(ts.contains("  maybe?: ValueType;"), "Got: {ts}");
        assert!(ts.contains("  tuple: [KeyType, ValueType];"), "Got: {ts}");
    }

    #[test]
    fn a_tuple_struct_declaration_binds_its_parameters() {
        let ts = Positional::<String>::ts_definition();
        assert!(
            ts.contains("export type Positional<IdType> = [IdType, string];"),
            "Got: {ts}"
        );
    }

    /// Which shape an enum's members are written in is what the tagging attributes decide, and
    /// only `serde` reads those; what the declaration binds is decided by the declaration, so it
    /// is asked of every flavour in every build.
    #[test]
    fn every_enum_flavour_declaration_binds_its_parameters() {
        for (flavour, ts) in [
            ("Adjacent", Adjacent::<String>::ts_definition()),
            ("Internal", Internal::<String>::ts_definition()),
            ("External", External::<String>::ts_definition()),
            ("Untagged", Untagged::<String>::ts_definition()),
        ] {
            assert!(
                ts.contains(&format!("export type {flavour}<IdType> = ")),
                "Got: {ts}"
            );
            assert!(ts.contains("id: IdType"), "Got: {ts}");
            assert!(!ts.contains("IdType$Schema"), "Got: {ts}");
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn each_tagged_shape_reaches_its_parameter_where_that_shape_puts_it() {
        let adjacent = Adjacent::<String>::ts_definition();
        assert!(adjacent.contains("  id: IdType;"), "Got: {adjacent}");
        assert!(adjacent.contains("  payload: IdType;"), "Got: {adjacent}");

        let internal = Internal::<String>::ts_definition();
        assert!(internal.contains("  id: IdType;"), "Got: {internal}");

        let external = External::<String>::ts_definition();
        assert!(external.contains("  id: IdType;"), "Got: {external}");
        assert!(external.contains("\"Single\": IdType;"), "Got: {external}");

        let untagged = Untagged::<String>::ts_definition();
        assert!(
            untagged.contains("export type Untagged<IdType> = { id: IdType } | { count: number };"),
            "Got: {untagged}"
        );
    }

    /// A const parameter names no type, so there is nothing for the declaration to bind — the
    /// `impl` still has to repeat it, which is what makes the fixture compile at all.
    #[test]
    fn a_const_parameter_reaches_no_declaration() {
        let ts = PlainConst::<4>::ts_definition();
        assert!(ts.contains("export type PlainConst =\n"), "Got: {ts}");
        assert!(!ts.contains("PlainConst<"), "Got: {ts}");
    }

    /// A lifetime names no type either, for the same reason and with the same outcome.
    #[test]
    fn a_lifetime_reaches_no_declaration() {
        let ts = LifetimeStruct::ts_definition();
        assert!(ts.contains("export type LifetimeStruct = {"), "Got: {ts}");
        assert!(ts.contains("  label: string;"), "Got: {ts}");
    }

    /// A reference is a type name on this surface whatever it carries, because a TypeScript
    /// generic is written and not called. Only the validating surface has a factory to reach.
    #[test]
    fn a_reference_carrying_arguments_is_still_written_as_a_type_name() {
        let concrete = WireFolder::ts_definition();
        assert!(
            concrete.contains("  doc: EcmDocument<string, number>;"),
            "Got: {concrete}"
        );
        assert!(concrete.contains("  plain: Envelope;"), "Got: {concrete}");

        let forwarded = FolderTree::<String>::ts_definition();
        for member in [
            "  labels: Array<string>;",
            "  many: Array<EcmDocument<IdType, number>>;",
            "  nested: Outer<Inner<string>>;",
            "  root: EcmDocument<IdType, number>;",
        ] {
            assert!(
                forwarded.contains(member),
                "Missing {member:?}: {forwarded}"
            );
        }
        assert!(!forwarded.contains("$SchemaFactory"), "Got: {forwarded}");
    }

    /// The line an item publishes under its own Rust ident is written after the declaration it
    /// refers to, and repeats the parameter list on both sides — a generic type named bare on
    /// either would be a TypeScript error of its own.
    #[test]
    fn the_reexport_lands_after_the_parameterised_declaration() {
        let ts = Holder::<String>::ts_definition();
        let declaration = ts.find("export type RenamedHolder<IdType> = {");
        let reexport = ts.find("export type Holder<IdType> = RenamedHolder<IdType>;");
        assert!(
            matches!((declaration, reexport), (Some(at), Some(after)) if at < after),
            "Got: {ts}"
        );
    }
}

#[cfg(feature = "zod")]
mod zod {
    #[cfg(all(feature = "chrono", feature = "object_id"))]
    use super::MixedArguments;
    use super::{
        Adjacent, Carried, Envelope, External, FolderTree, Holder, Internal, KeyedByParameter,
        LifetimeStruct, Pair, PlainConst, Positional, Quintet, Referrer, Tagged, Untagged,
        WireFolder, Wrapper,
    };

    /// A record key has to produce string keys, and serde says every instantiation this map has
    /// does: it writes a JSON object key as a string or refuses the map outright at serialization.
    /// So the key states that guarantee rather than declining to state anything, and the member
    /// comes out byte-identical to the concrete `String`-keyed one beside it.
    #[test]
    fn a_parameter_keyed_map_states_the_string_keys_serde_writes() {
        let zod = KeyedByParameter::<String, u32>::zod_schema();
        assert!(
            zod.contains("parameter_keyed: z.record(z.string(), z.string())"),
            "Got: {zod}"
        );
        // The value parameter is the factory's own argument since the factories landing; only
        // the KEY position is this rule's — it states the string keys serde writes.
        assert!(
            zod.contains("both_parameters: z.record(z.string(), valueType)"),
            "Got: {zod}"
        );
        assert!(!zod.contains("z.record(z.unknown()"), "Got: {zod}");
        // The factory legitimately names the parameter in its own signature; what must never
        // appear is the parameter standing where the KEY schema goes.
        assert!(!zod.contains("z.record(keyType"), "Got: {zod}");
        assert!(!zod.contains("KeyType$Schema"), "Got: {zod}");
    }

    /// A concrete key keeps its own answer: a bare string opens the object, and a key serde
    /// stringifies for the author keeps the narrowing it has always described as.
    #[test]
    fn a_concrete_key_beside_a_parameter_one_renders_as_it_always_did() {
        let zod = KeyedByParameter::<String, u32>::zod_schema();
        assert!(
            zod.contains("concrete_string_key: z.record(z.string(), z.string())"),
            "Got: {zod}"
        );
        assert!(
            zod.contains("stringified_number_key: z.record(z.number().int(), z.string())"),
            "Got: {zod}"
        );
    }

    /// Whether the output publishes `name` as a plain exported `const` — the binding a type that
    /// declares no parameter writes, in either build flavour. Asked this way because
    /// `{name}$SchemaFactory` carries `{name}$Schema` inside it, so the two names cannot be told
    /// apart by the prefix alone.
    fn publishes_a_schema_const(zod: &str, name: &str) -> bool {
        zod.contains(&format!("export const {name}$Schema:"))
            || zod.contains(&format!("export const {name}$Schema ="))
    }

    /// A Zod schema is a runtime value and a `const` cannot be parameterised, so a generic type has
    /// no one schema to publish — what it publishes is the function that builds one per filling.
    #[test]
    fn a_generic_type_publishes_a_factory_where_a_plain_type_publishes_a_schema() {
        let generic = Wrapper::<String>::zod_schema();
        assert!(
            generic.contains("export const Wrapper$SchemaFactory = "),
            "Got: {generic}"
        );
        assert!(
            !publishes_a_schema_const(&generic, "Wrapper"),
            "Got: {generic}"
        );

        let plain = Envelope::zod_schema();
        assert!(publishes_a_schema_const(&plain, "Envelope"), "Got: {plain}");
        assert!(!plain.contains("$SchemaFactory"), "Got: {plain}");
    }

    /// The argument the factory binds is what a field written with a parameter composes, so the
    /// caller's filling is what validates rather than a value that admits anything.
    #[test]
    fn a_parameter_composes_into_the_value_as_the_argument_bound_for_it() {
        let zod = Wrapper::<String>::zod_schema();
        assert!(zod.contains("id: idType,"), "Got: {zod}");
        assert!(zod.contains("children: z.array(idType),"), "Got: {zod}");
        assert!(zod.contains("name: z.string(),"), "Got: {zod}");
    }

    #[test]
    fn a_parameter_is_the_argument_at_every_depth_it_is_written_at() {
        let zod = Pair::<String, u32>::zod_schema();
        assert!(zod.contains("key: keyType,"), "Got: {zod}");
        assert!(
            zod.contains("by_key: z.record(z.string(), valueType),"),
            "Got: {zod}"
        );
        assert!(
            zod.contains("tuple: z.tuple([keyType, valueType]),"),
            "Got: {zod}"
        );
    }

    #[test]
    fn a_generic_tuple_struct_publishes_a_factory_too() {
        let zod = Positional::<String>::zod_schema();
        assert!(
            zod.contains("export const Positional$SchemaFactory = "),
            "Got: {zod}"
        );
        assert!(zod.contains("z.tuple([idType, z.string()])"), "Got: {zod}");
    }

    /// Which shape an enum's members are written in is what the tagging attributes decide, and only
    /// `serde` reads those; that the parameter is bound by a factory is decided by the declaration,
    /// so it is asked of every flavour in every build.
    #[test]
    fn every_enum_flavour_publishes_a_factory() {
        for (flavour, zod) in [
            ("Adjacent", Adjacent::<String>::zod_schema()),
            ("Internal", Internal::<String>::zod_schema()),
            ("External", External::<String>::zod_schema()),
            ("Untagged", Untagged::<String>::zod_schema()),
        ] {
            assert!(
                zod.contains(&format!("export const {flavour}$SchemaFactory = ")),
                "Got: {zod}"
            );
            assert!(zod.contains("idType"), "Got: {zod}");
            assert!(!publishes_a_schema_const(&zod, flavour), "Got: {zod}");
        }
    }

    /// A const parameter and a lifetime name no type, so neither reaches a schema and there is
    /// nothing for a factory to bind — the item publishes the one schema it has.
    #[test]
    fn an_item_binding_no_type_parameter_still_publishes_a_schema() {
        for (name, zod) in [
            ("PlainConst", PlainConst::<4>::zod_schema()),
            ("LifetimeStruct", LifetimeStruct::zod_schema()),
        ] {
            assert!(publishes_a_schema_const(&zod, name), "Got: {zod}");
            assert!(!zod.contains("$SchemaFactory"), "Got: {zod}");
        }
    }

    /// Both names an item is published under carry the one binding it has, so a generic type's
    /// re-export names the factory rather than a schema no module declares.
    #[test]
    fn the_ident_reexport_names_the_factory() {
        let zod = Holder::<String>::zod_schema();
        assert!(
            zod.contains("export const Holder$SchemaFactory = RenamedHolder$SchemaFactory;"),
            "Got: {zod}"
        );
        assert!(!publishes_a_schema_const(&zod, "Holder"), "Got: {zod}");
    }

    /// A base is read when the intersection is used rather than while the value holding it is
    /// built, which is what leaves declaration order irrelevant. The arguments are in scope for the
    /// whole of the builder, so the deferral composes inside the factory unchanged.
    #[cfg(feature = "serde")]
    #[test]
    fn a_generic_type_that_flattens_still_defers_its_base() {
        let zod = Carried::<String>::zod_schema();
        assert!(
            zod.contains("export const Carried$SchemaFactory = "),
            "Got: {zod}"
        );
        assert!(zod.contains("id: idType,"), "Got: {zod}");
        assert!(
            zod.contains(".and(z.lazy(() => Envelope$Schema))"),
            "Got: {zod}"
        );
    }

    /// Two calls with the same arguments reach the one schema: the miss path returns the very
    /// value it stored, and the hit path returns what was stored.
    #[test]
    fn a_factory_returns_what_it_stored_rather_than_building_again() {
        let zod = Wrapper::<String>::zod_schema();
        assert!(
            zod.contains(
                "  const hit = Wrapper$SchemaFactoryCache.get(idType);\n  if (hit) return \
                 hit;\n\n  const schema = buildWrapper$Schema(idType);\n  \
                 Wrapper$SchemaFactoryCache.set(idType, schema);\n  return schema;\n};"
            ),
            "Got: {zod}"
        );
    }

    /// Every argument keys a level of its own, so no two argument lists meet in one slot: a change
    /// in the first re-keys the outermost map, and a change in the last re-keys the one the schema
    /// is stored in.
    #[test]
    fn every_argument_keys_a_level_of_its_own() {
        let zod = Quintet::<u32, u32, u32, u32, u32>::zod_schema();
        for lookup in [
            "  let byBType = Quintet$SchemaFactoryCache.get(aType);",
            "  let byCType = byBType.get(bType);",
            "  let byDType = byCType.get(cType);",
            "  let byEType = byDType.get(dType);",
            "  const hit = byEType.get(eType);",
            "  const schema = buildQuintet$Schema(aType, bType, cType, dType, eType);",
            "  byEType.set(eType, schema);",
        ] {
            assert!(zod.contains(lookup), "Missing {lookup:?} in: {zod}");
        }
    }

    /// The builder holds the expression the arguments compose into and the factory's return type is
    /// read back off it, so the two cannot come to claim different shapes.
    #[cfg(feature = "typescript")]
    #[test]
    fn the_factory_return_type_is_read_back_off_the_builder() {
        let zod = Wrapper::<String>::zod_schema();
        assert!(
            zod.contains(
                "const buildWrapper$Schema = <IdType extends ZodType>(\n  idType: IdType,\n) =>"
            ),
            "Got: {zod}"
        );
        assert!(
            zod.contains(
                "type Wrapper$SchemaOf<IdType extends ZodType> = ReturnType<\n  typeof \
                 buildWrapper$Schema<IdType>\n>;"
            ),
            "Got: {zod}"
        );
        assert!(
            zod.contains(
                "export const Wrapper$SchemaFactory = <IdType extends ZodType>(\n  idType: \
                 IdType,\n): Wrapper$SchemaOf<IdType> => {"
            ),
            "Got: {zod}"
        );
    }

    /// Each parameter is a parameter of the function for real. A bare `ZodType` annotation compiles
    /// and infers nothing — `ZodType` defaults its own parameters — so a field validated through
    /// one would come back as the opaque value whatever the caller supplied.
    #[cfg(feature = "typescript")]
    #[test]
    fn every_parameter_is_a_type_parameter_rather_than_a_bare_annotation() {
        let zod = Pair::<String, u32>::zod_schema();
        assert!(
            zod.contains("<KeyType extends ZodType, ValueType extends ZodType>"),
            "Got: {zod}"
        );
        assert!(
            zod.contains("\n  keyType: KeyType,\n  valueType: ValueType,\n)"),
            "Got: {zod}"
        );
        assert!(!zod.contains("keyType: ZodType"), "Got: {zod}");
        assert!(!zod.contains("valueType: ZodType"), "Got: {zod}");
    }

    /// One parameter collapses to a single interface and a single lookup.
    #[cfg(feature = "typescript")]
    #[test]
    fn one_parameter_writes_one_cache_interface() {
        let zod = Wrapper::<String>::zod_schema();
        assert!(
            zod.contains(
                "interface Wrapper$SchemaFactoryCache {\n  get<IdType extends ZodType>(key: \
                 IdType): Wrapper$SchemaOf<IdType> | undefined;\n  set<IdType extends \
                 ZodType>(key: IdType, value: Wrapper$SchemaOf<IdType>): this;\n}"
            ),
            "Got: {zod}"
        );
        assert!(
            zod.contains(
                "const Wrapper$SchemaFactoryCache = createSchemaCache<Wrapper$SchemaFactoryCache>();"
            ),
            "Got: {zod}"
        );
        assert!(!zod.contains("Wrapper$SchemaFactoryCacheL1"), "Got: {zod}");
    }

    /// One level per parameter, each carrying the parameters resolved above it — which is what lets
    /// a lookup come back already typed and keeps the factory body free of assertions.
    #[cfg(feature = "typescript")]
    #[test]
    fn each_cache_level_carries_the_parameters_resolved_above_it() {
        let zod = Quintet::<u32, u32, u32, u32, u32>::zod_schema();
        for level in [
            "interface Quintet$SchemaFactoryCacheL1<AType extends ZodType> {\n  get<BType extends \
             ZodType>(key: BType): Quintet$SchemaFactoryCacheL2<AType, BType> | undefined;",
            "interface Quintet$SchemaFactoryCacheL4<AType extends ZodType, BType extends ZodType, \
             CType extends ZodType, DType extends ZodType> {\n  get<EType extends ZodType>(key: \
             EType): Quintet$SchemaOf<AType, BType, CType, DType, EType> | undefined;",
            "interface Quintet$SchemaFactoryCache {\n  get<AType extends ZodType>(key: AType): \
             Quintet$SchemaFactoryCacheL1<AType> | undefined;",
            "    byCType = createSchemaCache<Quintet$SchemaFactoryCacheL2<AType, BType>>();",
        ] {
            assert!(zod.contains(level), "Missing {level:?} in: {zod}");
        }
        assert!(!zod.contains("Quintet$SchemaFactoryCacheL5"), "Got: {zod}");
    }

    /// The one assertion the output carries lives in the shared preamble, so nothing a type
    /// publishes for itself needs `as`, `any`, or `unknown` to say what it validates.
    #[test]
    fn a_generic_type_publishes_no_assertion_and_no_opaque_value() {
        for zod in [
            Wrapper::<String>::zod_schema(),
            Pair::<String, u32>::zod_schema(),
            Positional::<String>::zod_schema(),
            Quintet::<u32, u32, u32, u32, u32>::zod_schema(),
            Carried::<String>::zod_schema(),
            Adjacent::<String>::zod_schema(),
        ] {
            assert!(!zod.contains(" as "), "Got: {zod}");
            assert!(!zod.contains("any"), "Got: {zod}");
            assert!(!zod.contains("unknown"), "Got: {zod}");
        }
    }

    /// A build with no `typescript` writes plain JavaScript: the same function and the same cache,
    /// with nothing to declare either to.
    #[cfg(not(feature = "typescript"))]
    #[test]
    fn a_javascript_build_writes_the_factory_untyped() {
        let zod = Wrapper::<String>::zod_schema();
        assert!(
            zod.contains("const buildWrapper$Schema = (\n  idType,\n) =>"),
            "Got: {zod}"
        );
        assert!(
            zod.contains("const Wrapper$SchemaFactoryCache = createSchemaCache();"),
            "Got: {zod}"
        );
        assert!(
            zod.contains("export const Wrapper$SchemaFactory = (\n  idType,\n) => {"),
            "Got: {zod}"
        );
        assert!(!zod.contains("interface "), "Got: {zod}");
        assert!(!zod.contains("ZodType"), "Got: {zod}");
    }

    /// The two surfaces that publish a `const` rather than a factory have no argument for a
    /// parameter inside them to name, so both still write the opaque value there.
    #[test]
    fn a_surface_publishing_no_factory_keeps_the_opaque_value() {
        let alias = super::boxed_schema::Schema::zod_schema();
        assert!(alias.contains("z.array(z.unknown())"), "Got: {alias}");
        assert!(!alias.contains("$SchemaFactory"), "Got: {alias}");

        let brand = Tagged::<String>::zod_schema();
        assert!(
            brand.contains("z.unknown().brand<\"Tagged\">()"),
            "Got: {brand}"
        );
        assert!(!brand.contains("$SchemaFactory"), "Got: {brand}");
    }

    /// A field naming a generic type has no schema to name — the type publishes a factory — so it
    /// calls that factory with what fills each parameter, and the plain sibling beside it still
    /// names the one schema its own type publishes.
    #[test]
    fn a_reference_carrying_arguments_calls_the_factory() {
        let zod = WireFolder::zod_schema();
        assert!(
            zod.contains("doc: EcmDocument$SchemaFactory(z.string(), z.number()),"),
            "Got: {zod}"
        );
        assert!(zod.contains("plain: Envelope$Schema,"), "Got: {zod}");
    }

    /// A forwarded parameter and a concrete type reach the call the same way: the parameter is the
    /// argument the enclosing factory binds, so there is no forwarding rule of its own.
    #[test]
    fn a_forwarded_parameter_is_an_argument_like_any_other() {
        let zod = FolderTree::<String>::zod_schema();
        assert!(
            zod.contains("root: EcmDocument$SchemaFactory(idType, z.number()),"),
            "Got: {zod}"
        );
    }

    /// An argument is rendered by the renderer that renders the reference, so an argument that is
    /// itself a reference composes at whatever depth it is written at.
    #[test]
    fn an_argument_that_is_itself_generic_nests() {
        let zod = FolderTree::<String>::zod_schema();
        assert!(
            zod.contains("nested: Outer$SchemaFactory(Inner$SchemaFactory(z.string())),"),
            "Got: {zod}"
        );
    }

    /// A set is a name carrying one argument, which is the shape a reference to a generic type is
    /// written in too — so the collection reading has to keep coming first, and the array it
    /// writes has to keep carrying whatever its element renders as.
    #[test]
    fn a_set_is_still_read_as_the_collection_it_is() {
        let zod = FolderTree::<String>::zod_schema();
        assert!(zod.contains("labels: z.array(z.string()),"), "Got: {zod}");
        assert!(
            zod.contains("many: z.array(EcmDocument$SchemaFactory(idType, z.number())),"),
            "Got: {zod}"
        );
        assert!(!zod.contains("HashSet$SchemaFactory"), "Got: {zod}");
    }

    /// Every argument reaches the call through whatever already answers for its type, so a date,
    /// a database identifier and a number are each written exactly as they are written anywhere
    /// else and none of the three is reached by a rule of its own.
    #[cfg(all(feature = "chrono", feature = "object_id"))]
    #[test]
    fn an_argument_renders_through_the_renderer_that_already_answers_for_it() {
        let zod = MixedArguments::zod_schema();
        for call in [
            "keyed: Inner$SchemaFactory(z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { \
             message: \"Invalid ObjectId\" }) })),",
            "sized: Inner$SchemaFactory(z.number().int()),",
            "stamped: Inner$SchemaFactory(z.coerce.date()),",
        ] {
            assert!(zod.contains(call), "Missing {call:?} in: {zod}");
        }
    }

    /// A reference names what the type it names publishes. An alias and a branded newtype publish
    /// a `const` whatever they were written with, so a field supplying either an argument still
    /// names that `const` rather than calling a factory neither declares.
    #[test]
    fn a_reference_to_a_type_publishing_a_const_still_names_it() {
        let zod = Referrer::zod_schema();
        assert!(zod.contains("boxed: Boxed$Schema,"), "Got: {zod}");
        assert!(zod.contains("tagged: Tagged$Schema,"), "Got: {zod}");
        assert!(!zod.contains("$SchemaFactory("), "Got: {zod}");
    }

    /// The one helper every factory builds its cache with, and the only assertion in the output.
    #[cfg(feature = "typescript")]
    #[test]
    fn the_preamble_carries_the_shared_cache_helper() {
        assert_eq!(
            tixschema::typescript_preamble!(),
            "const createSchemaCache = <Cache extends object>(): Cache => new WeakMap() as unknown as Cache;"
        );
    }

    #[cfg(not(feature = "typescript"))]
    #[test]
    fn the_preamble_carries_the_shared_cache_helper() {
        assert_eq!(
            tixschema::typescript_preamble!(),
            "const createSchemaCache = () => new WeakMap();"
        );
    }
}

#[cfg(feature = "jsonschema")]
mod jsonschema {
    use super::{KeyedByParameter, Pair, Wrapper};

    /// A key every instantiation writes as a string leaves the value side describable, so the
    /// object says what it holds instead of opening entirely — the answer the concrete
    /// `String`-keyed member beside it already gave.
    #[test]
    fn a_parameter_keyed_map_still_describes_its_values() {
        let schema = KeyedByParameter::<String, u32>::json_schema();
        let properties = &schema["properties"];
        assert_eq!(
            properties["parameter_keyed"],
            serde_json::json!({ "type": "object", "additionalProperties": { "type": "string" } })
        );
        assert_eq!(
            properties["both_parameters"],
            serde_json::json!({ "type": "object", "additionalProperties": {} })
        );
        assert_eq!(
            properties["parameter_keyed"],
            properties["concrete_string_key"]
        );
    }

    /// A key serde stringifies for the author is not the same question, and keeps the open object
    /// it has always described as.
    #[test]
    fn a_stringified_concrete_key_keeps_its_open_object() {
        let schema = KeyedByParameter::<String, u32>::json_schema();
        assert_eq!(
            schema["properties"]["stringified_number_key"],
            serde_json::json!({ "type": "object", "additionalProperties": true })
        );
    }

    /// One schema is written for every instantiation, so a parameter admits any value while the
    /// shape it sits in stays described.
    #[test]
    fn a_parameter_describes_as_the_open_schema() {
        let schema = Wrapper::<String>::json_schema();
        let properties = &schema["properties"];
        assert_eq!(properties["id"], serde_json::json!({}));
        assert_eq!(
            properties["children"],
            serde_json::json!({ "type": "array", "items": {} })
        );
        assert_eq!(properties["name"], serde_json::json!({ "type": "string" }));
    }

    #[test]
    fn the_shape_around_a_parameter_is_still_described() {
        let schema = Pair::<String, u32>::json_schema();
        let properties = &schema["properties"];
        assert_eq!(
            properties["by_key"],
            serde_json::json!({ "type": "object", "additionalProperties": {} })
        );
        assert_eq!(
            properties["tuple"],
            serde_json::json!({
                "type": "array",
                "prefixItems": [{}, {}],
                "items": false,
                "minItems": 2_u64,
                "maxItems": 2_u64
            })
        );
    }

    /// The `anyOf` an untagged enum describes as is what `#[serde(untagged)]` earns it, and only
    /// `serde` reads that attribute.
    #[cfg(feature = "serde")]
    #[test]
    fn an_untagged_member_describes_its_parameter_the_same_way() {
        let schema = super::Untagged::<String>::json_schema();
        assert_eq!(
            schema["anyOf"][0]["properties"]["id"],
            serde_json::json!({})
        );
    }
}

use alloc::borrow::Cow;
#[cfg(any(
    feature = "serde",
    feature = "typescript",
    feature = "zod",
    feature = "jsonschema"
))]
use core::hash::Hash;
use std::collections::{HashMap, HashSet};

#[cfg(all(feature = "chrono", feature = "object_id"))]
use chrono::{DateTime, Utc};
#[cfg(all(feature = "chrono", feature = "object_id"))]
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use tixschema::model_schema;

#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wrapper<IdType> {
    pub children: Vec<IdType>,
    pub id: IdType,
    pub name: String,
}

#[model_schema(default_types(KeyType = String, ValueType = u32))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pair<KeyType, ValueType> {
    pub by_key: HashMap<String, ValueType>,
    pub key: KeyType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maybe: Option<ValueType>,
    pub tuple: (KeyType, ValueType),
}

#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum Adjacent<IdType> {
    Named { id: IdType },
    Nothing,
    Single(IdType),
}

#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Internal<IdType> {
    Named { id: IdType },
    Nothing,
}

#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum External<IdType> {
    Named { id: IdType },
    Nothing,
    Single(IdType),
}

#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Untagged<IdType> {
    Named { id: IdType },
    Numbered { count: u32 },
}

/// The one enum shape a parameter can reach without a variant carrying it.
#[model_schema()]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PlainConst<const WIDTH: usize> {
    Narrow,
    Wide,
}

#[model_schema(name = "RenamedHolder", default_types(IdType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Holder<IdType> {
    pub id: IdType,
}

#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Positional<IdType>(pub IdType, pub String);

/// A map keyed by one of the item's own parameters, beside the concrete-keyed members the two
/// validating surfaces must keep answering for exactly as before. Consumed only by the surface
/// modules and the serde wire tests, so it exists only where one of them compiles.
#[cfg(any(
    feature = "serde",
    feature = "typescript",
    feature = "zod",
    feature = "jsonschema"
))]
#[model_schema(default_types(KeyType = String, ValueType = u32))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyedByParameter<KeyType: Eq + Hash, ValueType> {
    pub both_parameters: HashMap<KeyType, ValueType>,
    pub concrete_string_key: HashMap<String, String>,
    pub parameter_keyed: HashMap<KeyType, String>,
    pub stringified_number_key: HashMap<u32, String>,
}

#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifetimeStruct<'label> {
    #[model_schema_prop(minLength = 1)]
    pub label: Cow<'label, str>,
}

/// Enough parameters that every level of the lookup is a level with two neighbours: the first
/// argument keys the outermost map, the last keys the one the schema is stored in, and the three
/// between are only reached through the two.
#[model_schema(default_types(
    AType = u32,
    BType = String,
    CType = u32,
    DType = String,
    EType = u32
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quintet<AType, BType, CType, DType, EType> {
    pub alpha: AType,
    pub bravo: BType,
    pub charlie: CType,
    pub delta: DType,
    pub echo: EType,
}

#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub trace: String,
}

/// A generic alias, and a generic branded newtype: the two surfaces that publish a `const` rather
/// than a factory, and so the two a parameter inside still reaches as the opaque value.
#[model_schema(name = "Boxed", default_types(ValueType = String))]
pub type Boxed<ValueType> = Vec<ValueType>;

#[model_schema(default_types(ValueType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Tagged<ValueType>(pub ValueType);

/// A generic type that also flattens, which is the one shape where the parameters and the deferred
/// read of another type's binding have to hold at once.
#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Carried<IdType> {
    #[serde(flatten)]
    pub envelope: Envelope,
    pub id: IdType,
}

/// A parameter with no declared default type, which only a build generating a JSON document
/// refuses: nothing else reads the default, so there is nothing for the absence to break. The
/// fixture compiling under the feature sets that admit it is the whole of the assertion — under
/// the one that does not, it is not declared at all.
#[cfg(not(feature = "jsonschema"))]
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Undefaulted<IdType> {
    pub id: IdType,
}

#[cfg(not(feature = "jsonschema"))]
#[test]
fn a_parameter_with_no_default_still_expands_where_no_json_document_is_built() {
    assert_eq!(Undefaulted { id: 1_u32 }.id, 1);
}

/// The item the reference-site fixtures below point at. A generic type publishes a factory rather
/// than a schema, so a field naming it has nothing to name — it has to call that factory with what
/// fills each parameter.
#[model_schema(default_types(IdType = String, DateType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcmDocument<IdType, DateType> {
    pub created_at: DateType,
    pub document_id: IdType,
}

/// The inner half of a reference whose own argument is generic.
#[model_schema(default_types(ValueType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inner<ValueType> {
    pub value: ValueType,
}

#[model_schema(default_types(ValueType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outer<ValueType> {
    pub held: ValueType,
}

/// A referrer that declares no parameter of its own: every argument it supplies is a concrete
/// type, and the plain sibling beside them still names the one schema that one publishes.
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireFolder {
    pub doc: EcmDocument<String, f64>,
    pub plain: Envelope,
}

/// A referrer that forwards its own parameter into a reference beside a concrete argument, holds a
/// reference whose argument is itself generic, and names a set — the shape a sibling carrying one
/// argument could be mistaken for.
#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderTree<IdType> {
    pub labels: HashSet<String>,
    pub many: Vec<EcmDocument<IdType, f64>>,
    pub nested: Outer<Inner<String>>,
    pub root: EcmDocument<IdType, f64>,
}

/// A date, a database identifier and a number in argument position. Each is rendered by whatever
/// already answers for it everywhere else, so none of the three is reached by a rule of its own.
#[cfg(all(feature = "chrono", feature = "object_id"))]
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixedArguments {
    pub keyed: Inner<ObjectId>,
    pub sized: Inner<u32>,
    pub stamped: Inner<DateTime<Utc>>,
}

/// The two surfaces that publish a `const` whatever they were written with, named from a field
/// that supplies each of them an argument.
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Referrer {
    pub boxed: Boxed<u32>,
    pub tagged: Tagged<String>,
}

/// The pair the attribute used to produce on any generic item: `E0107` on the emitted `impl`,
/// which dropped the parameters the declaration binds, and `E0433` on a module named after a
/// parameter, which names no type and so publishes none. Both are compile errors, so the suite
/// compiling is the assertion; this names an instance of each declaration shape so no build can
/// drop one silently.
#[test]
fn a_generic_item_expands_to_rust_that_compiles() {
    let wrapper = Wrapper {
        children: vec!["a".to_owned()],
        id: "b".to_owned(),
        name: "c".to_owned(),
    };
    assert_eq!(wrapper.children.len(), 1);
    assert_eq!(wrapper.id, "b");
    assert_eq!(wrapper.name, "c");

    let pair = Pair {
        by_key: HashMap::from([("k".to_owned(), 1_u32)]),
        key: "k".to_owned(),
        maybe: None,
        tuple: ("k".to_owned(), 1_u32),
    };
    assert_eq!(pair.by_key.len(), 1);
    assert_eq!(pair.key, "k");
    assert_eq!(pair.maybe, None);
    assert_eq!(pair.tuple.1, 1);

    let positional = Positional("id".to_owned(), "name".to_owned());
    assert_eq!(positional.0, "id");
    assert_eq!(positional.1, "name");

    assert_eq!(Holder { id: 1_u32 }.id, 1);
    assert_eq!(LifetimeStruct { label: "x".into() }.label, "x");

    let carried = Carried {
        envelope: Envelope {
            trace: "t".to_owned(),
        },
        id: "i".to_owned(),
    };
    assert_eq!(carried.envelope.trace, "t");
    assert_eq!(carried.id, "i");

    let boxed: Boxed<u32> = vec![1_u32];
    assert_eq!(boxed.len(), 1);
    assert_eq!(Tagged("t".to_owned()).0, "t");

    let quintet = Quintet {
        alpha: 1_u32,
        bravo: "b".to_owned(),
        charlie: 3_u32,
        delta: "d".to_owned(),
        echo: 5_u32,
    };
    assert_eq!(quintet.alpha, 1);
    assert_eq!(quintet.bravo, "b");
    assert_eq!(quintet.charlie, 3);
    assert_eq!(quintet.delta, "d");
    assert_eq!(quintet.echo, 5);
}

/// The reference-site shapes, each named by a value: a concrete filling, a forwarded parameter, an
/// argument that is itself generic, and an argument supplied to a type that publishes a `const`.
#[test]
fn a_reference_carrying_arguments_expands_to_rust_that_compiles() {
    let document = |id: &str| EcmDocument {
        created_at: 1.0_f64,
        document_id: id.to_owned(),
    };

    let folder = WireFolder {
        doc: document("d"),
        plain: Envelope {
            trace: "t".to_owned(),
        },
    };
    assert!(folder.doc.created_at > 0.0_f64);
    assert_eq!(folder.doc.document_id, "d");
    assert_eq!(folder.plain.trace, "t");

    let tree = FolderTree {
        labels: HashSet::from(["l".to_owned()]),
        many: vec![document("m")],
        nested: Outer {
            held: Inner {
                value: "v".to_owned(),
            },
        },
        root: document("r"),
    };
    assert_eq!(tree.labels.len(), 1);
    assert_eq!(tree.many[0].document_id, "m");
    assert_eq!(tree.nested.held.value, "v");
    assert_eq!(tree.root.document_id, "r");

    let referrer = Referrer {
        boxed: vec![1_u32],
        tagged: Tagged("t".to_owned()),
    };
    assert_eq!(referrer.boxed.len(), 1);
    assert_eq!(referrer.tagged.0, "t");
}

/// The two argument kinds no build reaches without their own feature, named the same way.
#[cfg(all(feature = "chrono", feature = "object_id"))]
#[test]
fn a_dated_and_an_identified_argument_expand_to_rust_that_compiles() {
    let mixed = MixedArguments {
        keyed: Inner {
            value: ObjectId::new(),
        },
        sized: Inner { value: 1_u32 },
        stamped: Inner {
            value: DateTime::<Utc>::from_timestamp(0, 0).unwrap(),
        },
    };
    assert_eq!(mixed.sized.value, 1);
    assert_eq!(mixed.stamped.value.timestamp(), 0);
    assert!(!mixed.keyed.value.to_hex().is_empty());
}

/// The enum half of the same question, in every shape the tagging attributes reach — plus the one
/// a plain enum can bind, which is a const rather than a type.
#[test]
fn a_generic_enum_expands_to_rust_that_compiles() {
    assert!(matches!(Adjacent::<String>::Nothing, Adjacent::Nothing));
    assert!(matches!(Internal::<String>::Nothing, Internal::Nothing));
    assert!(matches!(External::<String>::Nothing, External::Nothing));
    assert!(matches!(
        Untagged::<String>::Numbered { count: 1 },
        Untagged::Numbered { .. }
    ));
    assert!(matches!(PlainConst::<4>::Wide, PlainConst::Wide));
}

/// The evidence the string-keyed rendering rests on: serde writes a JSON object key as a string
/// for every instantiation it accepts at all, and refuses the whole map at serialization for the
/// ones it does not — there is no instantiation whose keys reach the wire as anything else.
#[cfg(feature = "serde")]
#[test]
fn every_instantiation_the_wire_accepts_writes_string_keys() {
    let string_instantiation = KeyedByParameter::<String, u32> {
        parameter_keyed: HashMap::from([("a".to_owned(), "one".to_owned())]),
        both_parameters: HashMap::from([("a".to_owned(), 1_u32)]),
        concrete_string_key: HashMap::new(),
        stringified_number_key: HashMap::new(),
    };
    let written = serde_json::to_value(&string_instantiation).unwrap();
    assert_eq!(
        written["parameter_keyed"],
        serde_json::json!({ "a": "one" })
    );

    let by_number = KeyedByParameter::<u32, u32> {
        parameter_keyed: HashMap::from([(7_u32, "seven".to_owned())]),
        both_parameters: HashMap::new(),
        concrete_string_key: HashMap::new(),
        stringified_number_key: HashMap::new(),
    };
    assert_eq!(
        serde_json::to_value(&by_number).unwrap()["parameter_keyed"],
        serde_json::json!({ "7": "seven" }),
        "a key serde stringifies still reaches the wire as a string"
    );

    let by_bool = KeyedByParameter::<bool, u32> {
        parameter_keyed: HashMap::from([(true, "yes".to_owned())]),
        both_parameters: HashMap::new(),
        concrete_string_key: HashMap::new(),
        stringified_number_key: HashMap::new(),
    };
    assert_eq!(
        serde_json::to_value(&by_bool).unwrap()["parameter_keyed"],
        serde_json::json!({ "true": "yes" })
    );

    let by_sequence = KeyedByParameter::<Vec<u32>, u32> {
        parameter_keyed: HashMap::from([(vec![1_u32], "no".to_owned())]),
        both_parameters: HashMap::new(),
        concrete_string_key: HashMap::new(),
        stringified_number_key: HashMap::new(),
    };
    let refused = serde_json::to_value(&by_sequence).unwrap_err();
    assert!(
        refused.to_string().contains("key must be a string"),
        "an instantiation whose key is no string fails the whole map: {refused}"
    );

    let read_back = serde_json::from_value::<KeyedByParameter<u32, u32>>(serde_json::json!({
        "parameter_keyed": { "7": "seven" },
        "both_parameters": {},
        "concrete_string_key": {},
        "stringified_number_key": {},
    }))
    .unwrap();
    assert_eq!(read_back.parameter_keyed[&7], "seven");
}

/// A lifetime is the half of the `impl` fix no schema surface can show: nothing renders it, and
/// the only evidence it was carried through is that the constrained field still reaches both gates
/// its bound is held at.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn a_lifetime_struct_still_holds_its_field_to_its_bound() {
    let rejected = serde_json::from_str::<LifetimeStruct<'_>>(r#"{"label":""}"#).unwrap_err();
    assert!(
        rejected
            .to_string()
            .contains("'label' is too short: minimum length is 1, got 0"),
        "Unexpected error: {rejected}"
    );

    let accepted = serde_json::from_str::<LifetimeStruct<'_>>(r#"{"label":"x"}"#).unwrap();
    assert!(
        accepted.validate().is_ok(),
        "A payload the wire admits must be one validate() admits: {:?}",
        accepted.validate().err()
    );

    let empty = LifetimeStruct {
        label: Cow::Borrowed(""),
    };
    assert_eq!(
        empty.validate().unwrap_err(),
        vec!["'label' is too short: minimum length is 1, got 0"]
    );
}
