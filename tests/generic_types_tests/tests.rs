//! `#[model_schema()]` on an item that declares parameters — the emitted `impl`'s own parameter
//! list, what each surface fills a parameter with, and the default type every JSON-visible fixture
//! declares, since JSON Schema has no type parameters of its own.

#[cfg(feature = "typescript")]
mod typescript {
    use super::{
        Adjacent, External, FolderTree, Holder, Internal, KeyedByParameter, LifetimeStruct, Pair,
        PlainConst, Positional, Untagged, WireFolder, Wrapper, concrete_alias_schema,
        keyed_alias_schema,
    };

    /// `Record<K, V>` requires `K extends keyof any`, so the key states the string keys serde
    /// actually writes rather than naming the parameter — the value beside it keeps that.
    #[test]
    fn a_parameter_keyed_map_states_the_string_keys_serde_writes_on_the_type_surface() {
        let ts = KeyedByParameter::<String, u32>::ts_definition();
        assert!(
            ts.contains("export type KeyedByParameter<KeyType, ValueType> = {"),
            "Got: {ts}"
        );
        assert!(
            ts.contains("  parameter_keyed: Partial<Record<string, string>>;"),
            "Got: {ts}"
        );
        assert!(
            ts.contains("  both_parameters: Partial<Record<string, ValueType>>;"),
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
        assert!(!ts.contains("Record<KeyType"), "Got: {ts}");
    }

    /// An alias reaches the same rule, its own target being classified the way a struct's fields
    /// are. Both members of the declaration are held: the target and the re-export written from it,
    /// the second binding the parameters the first spends.
    #[test]
    fn a_parameter_keyed_map_alias_states_the_string_key_and_keeps_the_value_parameter() {
        let ts = keyed_alias_schema::Schema::ts_definition();
        assert!(
            ts.contains(
                "export type KeyedAliasType<KeyType, ValueType> = \
                 Partial<Record<string, ValueType>>;"
            ),
            "Got: {ts}"
        );
        assert!(
            ts.contains(
                "export type KeyedAlias<KeyType, ValueType> = KeyedAliasType<KeyType, ValueType>;"
            ),
            "Got: {ts}"
        );
        assert!(!ts.contains("Record<KeyType"), "Got: {ts}");
    }

    /// Nothing parameterised, nothing to classify: the alias renders what it always rendered.
    #[test]
    fn an_alias_declaring_no_parameter_renders_its_target_as_written() {
        let ts = concrete_alias_schema::Schema::ts_definition();
        assert!(
            ts.contains("export type ConcreteAliasType = Partial<Record<string, number>>;"),
            "Got: {ts}"
        );
        assert!(
            ts.contains("export type ConcreteAlias = ConcreteAliasType;"),
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
        assert!(ts.contains("  maybe: ValueType | undefined;"), "Got: {ts}");
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
        Adjacent, BatchedDefault, Carried, CycleFollower, CycleLeader, EchoedDefault, EcmDocument,
        Envelope, External, FolderTree, Holder, Internal, KeyedByParameter, LifetimeStruct,
        ListedDefault, OverriddenDefault, Pair, PlainConst, Positional, Quintet, Referrer,
        SlottedDefault, Summarised, Tagged, Untagged, WireFolder, Wrapper, keyed_alias_schema,
    };
    #[cfg(all(feature = "typescript", feature = "serde"))]
    use super::{ConstrainedEchoedDefault, ConstrainedId, DeepEchoedDefault};

    /// Two generic items whose declared defaults name each other. Neither has registered the
    /// other's arguments when it expands, so neither folds; both calls are deferred, and which of
    /// the two names is written first in the generated module is never asked.
    #[test]
    fn a_cycle_between_two_defaults_is_deferred_on_both_sides() {
        let leader = CycleLeader::<u32>::zod_schema();
        assert!(
            leader.contains(
                "= CycleLeader$SchemaFactory(z.lazy(() => \
                 CycleFollower$SchemaFactory(z.number().int())));"
            ),
            "Got: {leader}"
        );

        let follower = CycleFollower::<u32>::zod_schema();
        assert!(
            follower.contains(
                "= CycleFollower$SchemaFactory(z.lazy(() => \
                 CycleLeader$SchemaFactory(z.number().int())));"
            ),
            "Got: {follower}"
        );
    }

    /// serde writes every map key as a string or refuses the map at serialization, so the key
    /// states that guarantee — coming out byte-identical to the concrete `String`-keyed member.
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

    /// The alias composes both answers in its factory: the VALUE parameter is the bound argument,
    /// while the KEY parameter states the string guarantee and leaves its own argument unspent.
    #[test]
    fn a_parameter_keyed_map_alias_composes_the_factory_argument_with_the_string_key() {
        let zod = keyed_alias_schema::Schema::zod_schema();
        assert!(
            zod.contains("const buildKeyedAliasType$Schema = "),
            "Got: {zod}"
        );
        assert!(
            zod.contains("  z.record(z.string(), valueType);"),
            "Got: {zod}"
        );
        // The key's argument is still bound where the declaration wrote it, spent or not; only a
        // build that declares types spells the binding out.
        #[cfg(feature = "typescript")]
        assert!(
            zod.contains(
                "const buildKeyedAliasType$Schema = \
                 <KeyType extends ZodType, ValueType extends ZodType>(\n  keyType: KeyType,"
            ),
            "Got: {zod}"
        );
        assert!(!zod.contains("z.record(z.unknown()"), "Got: {zod}");
        assert!(!zod.contains("z.record(keyType"), "Got: {zod}");
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

    /// Whether `name` publishes a plain exported `const`. Checked this way because
    /// `{name}$SchemaFactory` contains `{name}$Schema` as a substring, so prefix matching alone
    /// cannot tell them apart.
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
            generic.contains("export function Wrapper$SchemaFactory"),
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
            zod.contains("export function Positional$SchemaFactory"),
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
                zod.contains(&format!("export function {flavour}$SchemaFactory")),
                "Got: {zod}"
            );
            assert!(zod.contains("idType"), "Got: {zod}");
            assert!(!publishes_a_schema_const(&zod, flavour), "Got: {zod}");
        }
    }

    /// A const parameter and a lifetime name no type, so neither reaches a schema and there is
    /// nothing for a factory to bind — the item publishes the one schema it has, and no default
    /// beside it: a `$SchemaDefault` calls a factory, and there is none to call.
    #[test]
    fn an_item_binding_no_type_parameter_still_publishes_a_schema() {
        for (name, zod) in [
            ("PlainConst", PlainConst::<4>::zod_schema()),
            ("LifetimeStruct", LifetimeStruct::zod_schema()),
        ] {
            assert!(publishes_a_schema_const(&zod, name), "Got: {zod}");
            assert!(!zod.contains("$SchemaFactory"), "Got: {zod}");
            assert!(!zod.contains("$SchemaDefault"), "Got: {zod}");
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

    /// The re-export covers both bindings a generic item publishes, not only the factory it always
    /// had — a renamed item's alias answers to `$SchemaDefault` exactly as it answers to
    /// `$SchemaFactory`.
    #[test]
    fn the_default_is_reexported_alongside_the_factory() {
        let zod = Holder::<String>::zod_schema();
        assert!(
            zod.contains("export const Holder$SchemaDefault = RenamedHolder$SchemaDefault;"),
            "Got: {zod}"
        );
    }

    /// A generic item's `$SchemaDefault` is the factory called with each parameter's declared
    /// default — the ordinary case a consumer no longer has to construct by hand.
    #[test]
    fn a_generic_type_publishes_a_default_through_its_own_factory() {
        let zod = Wrapper::<String>::zod_schema();
        assert!(
            zod.contains("export const Wrapper$SchemaDefault"),
            "Got: {zod}"
        );
        assert!(
            zod.contains("= Wrapper$SchemaFactory(z.string());"),
            "Got: {zod}"
        );
    }

    /// Under `typescript`, the default carries an annotation naming the instantiation it validates
    /// — the one place a `TypeScript` type is spelled for a binding a factory otherwise reads its
    /// return type back off.
    #[cfg(feature = "typescript")]
    #[test]
    fn the_default_is_annotated_with_the_instantiation_it_validates() {
        let zod = Wrapper::<String>::zod_schema();
        assert!(
            zod.contains(
                "export const Wrapper$SchemaDefault: ZodType<Wrapper<string>> = \
                 Wrapper$SchemaFactory(z.string());"
            ),
            "Got: {zod}"
        );
    }

    /// Two parameters fill two arguments, each read off its own `default_types` entry and written
    /// in declaration order.
    #[test]
    fn a_two_parameter_default_fills_each_argument_in_declaration_order() {
        let zod = EcmDocument::<String, f64>::zod_schema();
        assert!(
            zod.contains("export const EcmDocument$SchemaDefault"),
            "Got: {zod}"
        );
        assert!(
            zod.contains("= EcmDocument$SchemaFactory(z.string(), z.number());"),
            "Got: {zod}"
        );
    }

    /// A default naming another item at exactly that item's own default folds onto its
    /// `$SchemaDefault`, carrying the checks and brand by reference instead of rebuilding them
    /// under a `z.string()` the memo would not share.
    #[test]
    fn a_default_naming_a_siblings_own_default_folds_onto_its_binding() {
        let zod = EchoedDefault::<String>::zod_schema();
        assert!(
            zod.contains("= EchoedDefault$SchemaFactory(z.lazy(() => Tagged$SchemaDefault));"),
            "Got: {zod}"
        );
    }

    /// At an argument other than the sibling's own default, the fold does not fire — the call is
    /// deferred exactly as an ordinary field reference is, since neither `const` can know whether
    /// the other is declared above or below it in the generated module.
    #[test]
    fn a_default_naming_a_sibling_at_another_filling_still_calls_its_factory() {
        let zod = OverriddenDefault::<String>::zod_schema();
        assert!(
            zod.contains(
                "= OverriddenDefault$SchemaFactory(z.lazy(() => \
                 Tagged$SchemaFactory(z.number().int())));"
            ),
            "Got: {zod}"
        );
    }

    /// A sibling reference wrapped in `Vec` defers exactly like a bare one — `z.lazy` wraps the
    /// whole expression, not merely the factory call inside `z.array(...)`.
    #[test]
    fn a_default_wrapping_a_sibling_in_vec_defers_the_whole_expression() {
        let zod = BatchedDefault::<Vec<Tagged<String>>>::zod_schema();
        assert!(
            zod.contains(
                "= BatchedDefault$SchemaFactory(z.lazy(() => \
                 z.array(Tagged$SchemaFactory(z.string()))));"
            ),
            "Got: {zod}"
        );
    }

    /// The same hazard one level deeper through `Option` rather than `Vec`: the fold gate requires
    /// `!is_optional()`, so this also falls through to the ordinary rendering, and that rendering
    /// is deferred whole for the same reason the `Vec`-wrapped case above is.
    #[test]
    fn a_default_wrapping_a_sibling_in_option_defers_the_whole_expression() {
        let zod = SlottedDefault::<Option<Tagged<String>>>::zod_schema();
        assert!(
            zod.contains(
                "= SlottedDefault$SchemaFactory(z.lazy(() => \
                 z.union([z.null().transform(() => undefined), Tagged$SchemaFactory(z.string()), \
                 z.undefined()]).prefault(undefined)));"
            ),
            "Got: {zod}"
        );
    }

    /// A `Vec`-wrapped default with nothing but a primitive inside names no sibling `const` at any
    /// depth, so it stays eager exactly as a bare primitive default does — the deferral is keyed
    /// on what the tree names, not on whether it is wrapped.
    #[test]
    fn a_default_wrapping_only_a_primitive_in_vec_stays_eager() {
        let zod = ListedDefault::<Vec<String>>::zod_schema();
        assert!(
            zod.contains("= ListedDefault$SchemaFactory(z.array(z.string()));"),
            "Got: {zod}"
        );
        assert!(!zod.contains("z.lazy"), "Got: {zod}");
    }

    /// Pins the bounds-through-serde behavior for `ConstrainedId` directly, mirroring what
    /// `constrained_generic_branded_tests::StrictDocumentId` covers in `branded_newtype_tests`.
    #[cfg(all(feature = "typescript", feature = "serde"))]
    #[test]
    fn a_constrained_ids_declared_default_enforces_the_bounds_through_serde() {
        let valid: ConstrainedId<String> =
            serde_json::from_str("\"64de3d95ff45b119e5b53a7e\"").unwrap();
        valid.validate().unwrap();

        let too_short: Result<ConstrainedId<String>, _> = serde_json::from_str("\"abc\"");
        assert!(too_short.is_err(), "Should reject a too-short id via serde");
    }

    /// `validate()` itself, not just the `deserialize_with` hook, at the declared default —
    /// running the same `minLength`/`maxLength`/`pattern` checks `$SchemaDefault` enforces.
    #[cfg(all(feature = "typescript", feature = "serde"))]
    #[test]
    fn the_declared_defaults_validate_method_runs_the_same_checks_the_default_schema_enforces() {
        let valid = ConstrainedId("64de3d95ff45b119e5b53a7e".to_owned());
        valid.validate().unwrap();

        let short = ConstrainedId("short".to_owned());
        let errors = short.validate().unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("too short") && e.contains("minimum length is 24")),
            "Got: {errors:?}"
        );
    }

    /// The point of pinning `validate()` to the declared default: a hand-written impl for a
    /// *different* instantiation compiles alongside the generated one with no duplicate-definition
    /// error, and it is the hand-written body that runs.
    #[cfg(all(feature = "typescript", feature = "serde"))]
    #[test]
    fn a_hand_written_impl_for_another_instantiation_compiles_and_runs_beside_the_generated_one() {
        ConstrainedId(1_u32).validate().unwrap();
        assert_eq!(
            ConstrainedId(0_u32).validate().unwrap_err(),
            vec!["id must not be zero".to_owned()]
        );
    }

    /// The fold fires for a constrained brand exactly as for `Tagged`: the comparison key is the
    /// plain rendering, not the `.min`/`.max`/`.check` chain `$SchemaDefault` emits — so the bounds
    /// carry in by reference instead of being silently dropped.
    #[cfg(all(feature = "typescript", feature = "serde"))]
    #[test]
    fn a_default_naming_a_constrained_brands_own_default_folds_onto_its_binding() {
        let zod = ConstrainedEchoedDefault::<String>::zod_schema();
        assert!(
            zod.contains(
                "= ConstrainedEchoedDefault$SchemaFactory(z.lazy(() => \
                 ConstrainedId$SchemaDefault));"
            ),
            "Got: {zod}"
        );
    }

    /// The fold chains two levels deep here, reading `ConstrainedEchoedDefault`'s comparison key
    /// back rather than the deferred, checks-carrying text its own `$SchemaDefault` emits.
    #[cfg(all(feature = "typescript", feature = "serde"))]
    #[test]
    fn a_default_naming_a_siblings_default_that_itself_folds_chains_two_levels_deep() {
        let zod = DeepEchoedDefault::<String>::zod_schema();
        assert!(
            zod.contains(
                "= DeepEchoedDefault$SchemaFactory(z.lazy(() => \
                 ConstrainedEchoedDefault$SchemaDefault));"
            ),
            "Got: {zod}"
        );
    }

    /// A base is read when the intersection is used rather than while the value holding it is
    /// built, which is what leaves declaration order irrelevant. The arguments are in scope for the
    /// whole of the builder, so the deferral composes inside the factory unchanged.
    #[cfg(feature = "serde")]
    #[test]
    fn a_generic_type_that_flattens_still_defers_its_base() {
        let zod = Carried::<String>::zod_schema();
        assert!(
            zod.contains("export function Carried$SchemaFactory"),
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
                 Wrapper$SchemaFactoryCache.set(idType, schema);\n  return schema;\n}"
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
                "export function Wrapper$SchemaFactory<IdType extends ZodType>(\n  idType: \
                 IdType,\n): Wrapper$SchemaOf<IdType>;"
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
        // The widened spelling appears exactly once each, in the implementation signature the
        // overload above covers — never in what a caller is offered.
        assert_eq!(zod.matches("keyType: ZodType,").count(), 1, "Got: {zod}");
        assert_eq!(zod.matches("valueType: ZodType,").count(), 1, "Got: {zod}");
    }

    /// The precise signature is declared as an overload and the store is keyed at the widened one
    /// the implementation takes, so the read is already the implementation's return type.
    #[cfg(feature = "typescript")]
    #[test]
    fn one_parameter_writes_one_overload_over_one_weak_map() {
        let zod = Wrapper::<String>::zod_schema();
        assert!(
            zod.contains(
                "const Wrapper$SchemaFactoryCache = new WeakMap<ZodType, \
                 Wrapper$SchemaOf<ZodType>>();"
            ),
            "Got: {zod}"
        );
        assert!(
            zod.contains(
                "export function Wrapper$SchemaFactory<IdType extends ZodType>(\n  idType: \
                 IdType,\n): Wrapper$SchemaOf<IdType>;\nexport function \
                 Wrapper$SchemaFactory(\n  idType: ZodType,\n): Wrapper$SchemaOf<ZodType> {"
            ),
            "Got: {zod}"
        );
        assert!(!zod.contains("interface "), "Got: {zod}");
    }

    /// One `WeakMap` level per parameter, nested to the exact depth the type declares, and each
    /// level below the first built where it is first needed.
    #[cfg(feature = "typescript")]
    #[test]
    fn each_parameter_keys_a_weak_map_level_of_its_own() {
        let zod = Quintet::<u32, u32, u32, u32, u32>::zod_schema();
        assert!(
            zod.contains(
                "const Quintet$SchemaFactoryCache = new WeakMap<ZodType, WeakMap<ZodType, \
                 WeakMap<ZodType, WeakMap<ZodType, WeakMap<ZodType, Quintet$SchemaOf<ZodType, \
                 ZodType, ZodType, ZodType, ZodType>>>>>>();"
            ),
            "Got: {zod}"
        );
        assert!(zod.contains("    byCType = new WeakMap();"), "Got: {zod}");
        assert!(!zod.contains("interface "), "Got: {zod}");
    }

    /// Nothing a generic type publishes is asserted: every memo is reached through a parameter the
    /// factory's own signature binds, so there is no read to narrow and nothing to widen through.
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
            zod.contains("const Wrapper$SchemaFactoryCache = new WeakMap();"),
            "Got: {zod}"
        );
        assert!(
            zod.contains("export function Wrapper$SchemaFactory(\n  idType,\n) {"),
            "Got: {zod}"
        );
        assert!(!zod.contains("interface "), "Got: {zod}");
        assert!(!zod.contains("ZodType"), "Got: {zod}");
    }

    /// The alias and the brand write the same untyped factory, and the brand's marker drops the
    /// type argument only TypeScript reads — the two spellings that made this build's output stop
    /// at load rather than at a payload.
    #[cfg(not(feature = "typescript"))]
    #[test]
    fn a_javascript_build_writes_the_alias_and_the_brand_untyped() {
        let alias = super::boxed_schema::Schema::zod_schema();
        assert!(
            alias.contains("const buildBoxed$Schema = (\n  valueType,\n) =>"),
            "Got: {alias}"
        );
        assert!(!alias.contains("ZodType"), "Got: {alias}");
        assert!(!alias.contains("$Schema:"), "Got: {alias}");

        let brand = Tagged::<String>::zod_schema();
        assert!(brand.contains("}).brand();"), "Got: {brand}");
        assert!(!brand.contains(".brand<"), "Got: {brand}");
        assert!(!brand.contains("ZodType"), "Got: {brand}");
    }

    /// An alias and a branded newtype are generic publishers like any other, so each binds an
    /// argument per parameter and composes it — the alias into the shape it names, the brand into
    /// the value it marks.
    #[test]
    fn every_generic_publisher_binds_its_parameter_as_a_factory_argument() {
        let alias = super::boxed_schema::Schema::zod_schema();
        assert!(
            alias.contains("export function Boxed$SchemaFactory"),
            "Got: {alias}"
        );
        assert!(alias.contains("z.array(valueType)"), "Got: {alias}");
        assert!(!alias.contains("z.unknown()"), "Got: {alias}");

        let brand = Tagged::<String>::zod_schema();
        assert!(
            brand.contains("export function Tagged$SchemaFactory"),
            "Got: {brand}"
        );
        assert!(brand.contains("  valueType.meta({"), "Got: {brand}");
        assert!(!brand.contains("z.unknown()"), "Got: {brand}");
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

    /// A reference names what the type it names publishes, and the seam saying which of the two it
    /// was is the item's own registry entry — so an alias and a branded newtype joining the
    /// factories moved every field naming either with them, no reference-site rule of its own.
    #[test]
    fn a_reference_to_an_alias_or_a_brand_calls_the_factory_it_publishes() {
        let zod = Referrer::zod_schema();
        assert!(
            zod.contains("boxed: Boxed$SchemaFactory(z.number().int()),"),
            "Got: {zod}"
        );
        assert!(
            zod.contains("tagged: Tagged$SchemaFactory(z.string()),"),
            "Got: {zod}"
        );
    }

    /// The argument a containing factory was handed is what reaches the alias and the brand it
    /// holds, so a caller filling the container fills what validates inside it — where before the
    /// container took the filling and threw it away.
    #[test]
    fn a_forwarded_parameter_reaches_an_alias_and_a_brand_alike() {
        let zod = Summarised::<String>::zod_schema();
        assert!(
            zod.contains("boxed: Boxed$SchemaFactory(valueType),"),
            "Got: {zod}"
        );
        assert!(
            zod.contains("tagged: Tagged$SchemaFactory(valueType),"),
            "Got: {zod}"
        );
    }

    /// Nothing a generated module carries is shared between the types in it, so a consumer's
    /// generator has no preamble to emit ahead of them.
    #[test]
    fn a_generic_type_carries_its_whole_cache_itself() {
        let zod = Wrapper::<String>::zod_schema();
        assert!(
            zod.contains("$SchemaFactoryCache = new WeakMap"),
            "Got: {zod}"
        );
        assert!(!zod.contains("createSchemaCache"), "Got: {zod}");
    }
}

#[cfg(feature = "jsonschema")]
mod jsonschema {
    /// `#[serde(flatten)]` is read only where `serde` is, so the merge these pin is only written
    /// there.
    #[cfg(feature = "serde")]
    mod flattened_parameter {
        use super::super::{Carried, FlatCarrier, FlatReferrer};

        /// A flattened parameter is a flattened value like any other: the object its filling
        /// describes contributes its members to the one being written. The merge never sees which
        /// end filled the parameter.
        #[test]
        fn a_flattened_parameter_merges_the_members_the_reference_site_filled_it_with() {
            assert_eq!(
                FlatReferrer::json_schema()["properties"]["held"],
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "tag": { "type": "string" },
                        "value": { "type": "integer" }
                    },
                    "required": ["tag", "value"],
                    "additionalProperties": false
                })
            );
        }

        /// A non-object filling has no members to merge. The declaration cannot refuse this itself
        /// — only the reference site names the filling — so the refusal fires where it's finally
        /// known.
        #[test]
        #[should_panic(
            expected = "`FlatCarrier`: `#[serde(flatten)]` of `held` is not written as \
                        an object — its schema describes a `string`, which has no \
                        members to merge"
        )]
        fn a_flattened_parameter_filled_by_a_scalar_is_refused_where_the_filling_is_known() {
            let _: serde_json::Value = FlatCarrier::<String>::json_schema();
        }

        /// A declared filling reaches the merge exactly as a reference-site one does — built at
        /// the default the declaration names, with no reference site involved.
        #[test]
        fn a_flattened_parameter_merges_the_members_its_declared_default_names() {
            assert_eq!(
                serde_json::to_string(
                    &super::super::FlatDefaulted::<super::super::Envelope>::json_schema()
                )
                .unwrap(),
                "{\"type\":\"object\",\"properties\":{\"tag\":{\"type\":\"string\"},\
                 \"trace\":{\"type\":\"string\"}},\"required\":[\"tag\",\"trace\"],\
                 \"additionalProperties\":false}"
            );
        }

        /// A flatten source naming a type rather than a parameter is untouched by any of this.
        #[test]
        fn a_named_flatten_source_beside_a_parameter_describes_as_it_always_has() {
            assert_eq!(
                serde_json::to_string(&Carried::<String>::json_schema()).unwrap(),
                "{\"type\":\"object\",\"properties\":{\"id\":{\"type\":\"string\"},\
                 \"trace\":{\"type\":\"string\"}},\"required\":[\"id\",\"trace\"],\
                 \"additionalProperties\":false}"
            );
        }
    }

    #[cfg(all(feature = "chrono", feature = "object_id"))]
    use super::StoredFolder;
    use super::{
        Branch, CountedFolder, EcmDocument, Envelope, Grove, KeyedByParameter, Pair, Perch, Roost,
        Sealed, Tagged, WireFolder, Wrapper, ecm_document_schema,
    };

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
            serde_json::json!({
                "type": "object",
                "additionalProperties": { "type": "integer" }
            })
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

    /// JSON Schema has no type parameters, so a document standing on its own is written at the one
    /// filling the item stated for itself.
    #[test]
    fn a_parameter_describes_as_the_type_declared_for_it() {
        let schema = Wrapper::<String>::json_schema();
        let properties = &schema["properties"];
        assert_eq!(properties["id"], serde_json::json!({ "type": "string" }));
        assert_eq!(
            properties["children"],
            serde_json::json!({ "type": "array", "items": { "type": "string" } })
        );
        assert_eq!(properties["name"], serde_json::json!({ "type": "string" }));
    }

    /// A `char` default renders the one-character string serde writes for it — the document
    /// `is_undescribable_primitive` refused before `char` gained a `FieldDefType` arm, naming a
    /// `char_schema` module nothing publishes instead.
    #[test]
    fn a_char_default_describes_the_one_character_string() {
        use super::Initialed;

        let schema = Initialed::<char>::json_schema();
        assert_eq!(
            schema["properties"]["initial"],
            serde_json::json!({ "type": "string", "minLength": 1_i32, "maxLength": 1_i32 })
        );
    }

    #[test]
    fn the_shape_around_a_parameter_is_still_described() {
        let schema = Pair::<String, u32>::json_schema();
        let properties = &schema["properties"];
        assert_eq!(
            properties["by_key"],
            serde_json::json!({
                "type": "object",
                "additionalProperties": { "type": "integer" }
            })
        );
        assert_eq!(
            properties["tuple"],
            serde_json::json!({
                "type": "array",
                "prefixItems": [{ "type": "string" }, { "type": "integer" }],
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
            serde_json::json!({ "type": "string" })
        );
    }

    /// The whole of what the declared filling is for: the document a generic type publishes on its
    /// own is the one its own declaration named, member by member.
    #[test]
    fn a_standalone_document_is_written_at_the_declared_filling() {
        let properties = EcmDocument::<String, f64>::json_schema()["properties"].clone();
        assert_eq!(
            properties["document_id"],
            serde_json::json!({ "type": "string" })
        );
        assert_eq!(
            properties["created_at"],
            serde_json::json!({ "type": "number" })
        );
    }

    /// A field embeds the document its own arguments name, not the one the named type declared for
    /// itself — a field carrying one filling described by another rejects every payload it holds.
    #[test]
    fn a_reference_embeds_the_document_its_own_arguments_name() {
        let wire = WireFolder::json_schema()["properties"]["doc"]["properties"].clone();
        assert_eq!(wire["document_id"], serde_json::json!({ "type": "string" }));
        assert_eq!(wire["created_at"], serde_json::json!({ "type": "number" }));

        let counted = CountedFolder::json_schema()["properties"]["doc"]["properties"].clone();
        assert_eq!(
            counted["document_id"],
            serde_json::json!({ "type": "integer" })
        );
        assert_eq!(
            counted["created_at"],
            serde_json::json!({ "type": "boolean" })
        );
    }

    /// An alias and a branded newtype are generic publishers like any other, so each is written at
    /// its own declared filling standing alone and at the reference site's where a field names it.
    #[test]
    fn a_generic_alias_and_brand_are_written_at_the_filling_that_reached_them() {
        assert_eq!(
            Tagged::<String>::json_schema(),
            serde_json::json!({ "type": "string" })
        );
        assert_eq!(
            super::boxed_schema::Schema::json_schema(),
            serde_json::json!({ "type": "array", "items": { "type": "string" } })
        );

        let properties = CountedFolder::json_schema()["properties"].clone();
        assert_eq!(
            properties["tagged"],
            serde_json::json!({ "type": "integer" })
        );
        assert_eq!(
            properties["boxed"],
            serde_json::json!({ "type": "array", "items": { "type": "boolean" } })
        );
    }

    /// The arguments are positional, in the order the item declares its parameters — which is not
    /// the order its fields are written in, so a document read off the wrong end would pass every
    /// assertion that only counted them.
    #[test]
    fn the_arguments_fill_the_parameters_in_declaration_order() {
        let properties = ecm_document_schema::Schema::json_schema_with(&[
            serde_json::json!({ "type": "boolean" }),
            serde_json::json!({ "type": "integer" }),
        ])["properties"]
            .clone();
        assert_eq!(
            properties["document_id"],
            serde_json::json!({ "type": "boolean" })
        );
        assert_eq!(
            properties["created_at"],
            serde_json::json!({ "type": "integer" })
        );
    }

    /// Every argument reaches the document through whatever already describes its type, so a date
    /// and a database identifier are each written exactly as they are written anywhere else.
    #[cfg(all(feature = "chrono", feature = "object_id"))]
    #[test]
    fn an_argument_is_described_by_whatever_already_answers_for_it() {
        let stored = StoredFolder::json_schema()["properties"]["doc"]["properties"].clone();
        assert_eq!(
            stored["document_id"],
            serde_json::json!({
                "type": "object",
                "properties": { "$oid": { "type": "string", "pattern": "^[a-f0-9]{24}$" } },
                "required": ["$oid"],
                "additionalProperties": false
            })
        );
        assert_eq!(
            stored["created_at"],
            serde_json::json!({ "type": "string", "format": "date-time" })
        );
    }

    /// A parameter forwarded into a reference is filled by whatever filled the item forwarding it,
    /// so a document reached two levels down still names the type at the top of the chain.
    #[test]
    fn a_forwarded_parameter_carries_the_filling_it_was_handed() {
        let properties = super::FolderTree::<String>::json_schema()["properties"].clone();
        assert_eq!(
            properties["root"]["properties"]["document_id"],
            serde_json::json!({ "type": "string" })
        );
        assert_eq!(
            properties["many"]["items"]["properties"]["document_id"],
            serde_json::json!({ "type": "string" })
        );
        assert_eq!(
            properties["nested"]["properties"]["held"]["properties"]["value"],
            serde_json::json!({ "type": "string" })
        );
    }

    /// A filling naming another item is a request made of that item, not a literal document — the
    /// standalone document holds at the parameter position exactly what the named item publishes.
    #[test]
    fn a_filling_naming_another_item_embeds_the_document_that_item_publishes() {
        let schema = Sealed::<Envelope>::json_schema();
        assert_eq!(schema["properties"]["body"], Envelope::json_schema());
        assert_eq!(
            schema["properties"]["seal"],
            serde_json::json!({ "type": "string" })
        );
    }

    /// A name is one name however it was reached. Reached once as a declared filling and once as a
    /// plain field, it is still the single definition the root carries, and both arrivals are
    /// pointers into it rather than two copies of a body.
    #[test]
    fn a_name_reached_as_a_filling_and_as_a_field_is_defined_once() {
        let schema = Grove::<Branch>::json_schema();
        let pointer = serde_json::json!({ "$ref": "#/$defs/Branch" });
        assert_eq!(schema["properties"]["filled"], pointer);
        assert_eq!(schema["properties"]["named"], pointer);

        let defs = schema["$defs"].as_object().unwrap().clone();
        assert_eq!(defs.len(), 1, "Got: {defs:?}");
        assert_eq!(
            defs["Branch"]["properties"]["children"],
            serde_json::json!({ "type": "array", "items": pointer })
        );
    }

    /// A cycle closing through a declared filling defers the same way every other edge does: the
    /// name is recognized mid-description, so the filling describes as a pointer.
    #[test]
    fn a_cycle_closing_through_a_declared_filling_defers_the_same_way() {
        let schema = Roost::<Perch>::json_schema();
        assert_eq!(
            schema["properties"]["host"],
            serde_json::json!({ "$ref": "#/$defs/Perch" })
        );
        assert_eq!(
            schema["$defs"]["Perch"]["properties"]["roosts"]["items"]["properties"]["host"],
            serde_json::json!({ "$ref": "#/$defs/Perch" })
        );
        assert_eq!(
            schema["$defs"]["Perch"]["properties"]["tag"],
            serde_json::json!({ "type": "string" })
        );
    }

    /// The key now carries the filling — a readable label off the argument's own `"type"`
    /// keyword, plus a digest keeping alike labels from colliding — so the pinned string below
    /// gained that text, with no character a URI-reference forbids.
    #[test]
    fn a_cycle_that_keeps_its_filling_defers_through_the_one_definition() {
        let document = super::Recurring::<String>::json_schema();
        assert_eq!(
            serde_json::to_string(&document).unwrap(),
            "{\"$defs\":{\"Recurring.string-1579594ac99678fa\":{\"type\":\"object\",\"additionalPr\
             operties\":false,\"properties\":{\"next\":{\"anyOf\":[{\"$ref\":\"#/$defs/Recurring.s\
             tring-1579594ac99678fa\"},{\"type\":\"null\"}]},\"value\":{\"type\":\"string\"}},\"req\
             uired\":[\"value\"]}},\"$ref\":\"#/$defs/Recurring.string-1579594ac99678fa\"}"
        );
        let reference = document["$ref"].as_str().unwrap();
        for forbidden in ['"', '{', '}', '[', ']', ',', ':', ' '] {
            assert!(
                !reference.contains(forbidden),
                "$ref carries a character a URI-reference forbids: {forbidden:?} in {reference}"
            );
        }
    }

    /// A reference at a filling the document is not being written at has nowhere to put its own
    /// definition. Both fillings are named in the refusal — which one is wrong is the author's call.
    #[test]
    #[should_panic(
        expected = "`Refilled`: a reference closes a cycle at a filling the document is \
                    not being written at — in flight at [{\"type\":\"string\"}], and \
                    this reference names [{\"type\":\"boolean\"}]"
    )]
    fn a_cycle_that_changes_filling_is_refused_naming_both_fillings() {
        let _: serde_json::Value = super::Refilled::<String>::json_schema();
    }

    /// The refusal states what stands in the way and what would move it, so an author meeting it
    /// reads why one definition cannot hold two fillings and what would let it.
    #[test]
    #[should_panic(
        expected = "a document holds one definition per name, so a cycle cannot change \
                    filling partway through it; write the reference at the filling \
                    already in flight, or key the definitions by name and filling so \
                    each filling gets a definition of its own"
    )]
    fn the_refused_cycle_states_the_limitation_and_the_way_past_it() {
        let _: serde_json::Value = super::Refilled::<String>::json_schema();
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

/// A `char` filling for a parameter's declared default — refused before `char` gained a
/// `FieldDefType` arm of its own. The fixture compiling is part of the assertion.
#[cfg(feature = "jsonschema")]
#[model_schema(default_types(InitialType = char))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Initialed<InitialType> {
    pub initial: InitialType,
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

/// A constrained field beside an unconstrained type parameter, combined with a lifetime — the
/// combination `default_instantiation`'s lifetime-passthrough branch exists for. The lifetime
/// carries through unchanged, letting the hand-written impl below coexist with the generated one.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[model_schema(default_types(LabelType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotatedConstrained<'label, LabelType> {
    pub label: LabelType,
    pub source: Cow<'label, str>,
    #[model_schema_prop(minLength = 1)]
    pub tag: String,
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
impl AnnotatedConstrained<'_, u32> {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        if self.label == 0 {
            return Err(vec!["label must not be zero".to_owned()]);
        }
        Ok(())
    }
}

/// A parameter bounded in the `where` clause rather than beside itself, filled at a type that
/// satisfies the bound. Every declared filling is checked against the bounds its parameter carries,
/// so the fixture compiling is the whole of the assertion that a satisfying one is let through.
#[model_schema(default_types(LabelType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Labelled<LabelType>
where
    LabelType: Clone + Eq,
{
    pub label: LabelType,
}

/// A parameter whose bound names the item's other parameter — checked jointly at both declared
/// fillings (`String` implements `From<String>` via the reflexive impl).
#[model_schema(default_types(WideType = String, NarrowType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Widened<WideType: From<NarrowType>, NarrowType: Clone> {
    pub narrow: NarrowType,
    pub wide: WideType,
}

/// A parameter bounded both ways at once: `Clone` is checked at the filling alone,
/// `Into<Cow<'label, str>>` at the whole parameter list — neither checked twice.
#[model_schema(default_types(LabelType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotated<'label, LabelType: Clone + Into<Cow<'label, str>>> {
    pub label: LabelType,
    pub source: Cow<'label, str>,
}

/// A const beside a bound that reads only type parameters. A const takes no filling, so it is
/// declared nowhere in the joint check and left out of its call; the fixture compiling asserts its
/// presence leaves that check standing.
#[model_schema(default_types(WideType = String, NarrowType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Padded<WideType: From<NarrowType>, NarrowType: Clone, const WIDTH: usize> {
    pub narrow: NarrowType,
    pub wide: WideType,
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

/// A generic alias, and a generic branded newtype: the two item shapes holding a single written
/// target rather than a list of fields, and so the two whose parameters are classified at that
/// target rather than one field at a time.
#[model_schema(name = "Boxed", default_types(ValueType = String))]
pub type Boxed<ValueType> = Vec<ValueType>;

#[model_schema(default_types(ValueType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Tagged<ValueType>(pub ValueType);

/// One parameter in a map's key position, one in its value position — what
/// [`KeyedByParameter`] holds for a struct, held here for an alias. Consumed only by the surface
/// modules, so it exists only where one of them compiles.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[model_schema(default_types(KeyType = String, ValueType = u32))]
pub type KeyedAlias<KeyType, ValueType> = HashMap<KeyType, ValueType>;

/// The same target with nothing parameterised, held beside the one above so the classification
/// cannot move a non-generic alias.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[model_schema()]
pub type ConcreteAlias = HashMap<String, u32>;

/// A generic type that also flattens, which is the one shape where the parameters and the deferred
/// read of another type's binding have to hold at once.
#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Carried<IdType> {
    #[serde(flatten)]
    pub envelope: Envelope,
    pub id: IdType,
}

/// A parameter with no declared default type — only a build generating a JSON document refuses
/// this, so it is declared only under the feature sets that admit it.
#[cfg(not(feature = "jsonschema"))]
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Undefaulted<IdType> {
    pub id: IdType,
}

/// A filling holding a map serde can key. The zod surface reads a declared filling as deeply as the
/// JSON one, refusing a key serde cannot write with no document in the build at all, so the filling
/// that renders here is the one written at a key it can.
#[cfg(all(not(feature = "jsonschema"), feature = "zod"))]
#[model_schema(default_types(HeldType = HashMap<String, u32>))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringKeyedHolder<HeldType> {
    pub held: HeldType,
}

/// The item the reference-site fixtures below point at. A generic type publishes a factory rather
/// than a schema, so a field naming it has nothing to name — it has to call that factory with what
/// fills each parameter.
#[model_schema(default_types(IdType = String, DateType = f64))]
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

/// A second concrete filling of the three generic publishers, at arguments that are neither of the
/// ones any of them declared for itself — the whole of the evidence that what a field embeds is the
/// reference site's filling rather than the declaration's, in every build.
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountedFolder {
    pub boxed: Boxed<bool>,
    pub doc: EcmDocument<u32, bool>,
    pub tagged: Tagged<u32>,
}

/// The stored shape of the same document: a database identifier and a date where the wire shape
/// carries a string and a number. Each argument reaches the document through whatever already
/// describes its type, so neither is reached by a rule of its own.
#[cfg(all(feature = "chrono", feature = "object_id"))]
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredFolder {
    pub doc: EcmDocument<ObjectId, DateTime<Utc>>,
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

/// An alias and a branded newtype, named from a field that supplies each of them a concrete
/// argument.
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Referrer {
    pub boxed: Boxed<u32>,
    pub tagged: Tagged<String>,
}

/// A declared filling that names another item rather than a primitive — written by asking that
/// item for its own document, reading the names in flight and definitions being hoisted. Read
/// by the JSON surface alone, the only one that fills a parameter with a document.
#[cfg(feature = "jsonschema")]
#[model_schema(default_types(BodyType = Envelope))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sealed<BodyType> {
    pub body: BodyType,
    pub seal: String,
}

/// A self-referential item: what puts a name in `$defs` at all.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub children: Vec<Self>,
    pub name: String,
}

/// The same name reached twice over, once as a declared filling and once as a plain field — the two
/// arrivals a single hoisted definition has to answer for both of.
#[cfg(feature = "jsonschema")]
#[model_schema(default_types(BranchType = Branch))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grove<BranchType> {
    pub filled: BranchType,
    pub named: Branch,
}

/// A cycle that closes through a declared filling — reachable without arguments only because
/// the parameter also carries a Rust-level default. A cycle through two declared fillings would
/// be a cycle in the parameter defaults themselves, which rustc refuses before the attribute runs.
#[cfg(feature = "jsonschema")]
#[model_schema(default_types(HostType = Perch))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Roost<HostType = Perch> {
    pub host: HostType,
}

#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Perch {
    pub roosts: Vec<Roost>,
    pub tag: String,
}

/// The same two, named from a field that forwards the referrer's own parameter into each. Read by
/// the Zod surface alone, where an argument is a value that has to be passed on, so it exists only
/// where that surface compiles.
#[cfg(feature = "zod")]
#[model_schema(default_types(ValueType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summarised<ValueType> {
    pub boxed: Boxed<ValueType>,
    pub tagged: Tagged<ValueType>,
}

/// A `default_types` entry naming another generic item at exactly its own default — the fold
/// that lets the argument read `Tagged$SchemaDefault` rather than reconstruct
/// `Tagged$SchemaFactory(z.string())`, a call the memo would not share with it.
#[cfg(feature = "zod")]
#[model_schema(default_types(HolderType = Tagged<String>))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EchoedDefault<HolderType> {
    pub held: HolderType,
}

/// The same shape at a filling that names `Tagged` at an argument other than its own declared
/// default — the whole of the evidence that the fold is conditioned on the arguments matching
/// rather than firing for any reference to a generic sibling.
#[cfg(feature = "zod")]
#[model_schema(default_types(HolderType = Tagged<u32>))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverriddenDefault<HolderType> {
    pub held: HolderType,
}

/// A declared default wrapping a sibling reference in a `Vec` — the direct-sibling fold gate
/// requires `array_depth == 0`, so this falls through to the ordinary rendering, naming
/// `Tagged$SchemaFactory` from inside `z.array(...)` rather than at the argument's top level.
#[cfg(feature = "zod")]
#[model_schema(default_types(Items = Vec<Tagged<String>>))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchedDefault<Items> {
    pub items: Items,
}

/// The same hazard one level deeper through `Option` rather than `Vec`.
#[cfg(feature = "zod")]
#[model_schema(default_types(Slot = Option<Tagged<String>>))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlottedDefault<Slot> {
    pub slot: Slot,
}

/// A `Vec`-wrapped default with nothing but a primitive inside — nothing here names a sibling
/// `const` at any depth, so the wrapped expression stays eager exactly as a bare primitive
/// default does.
#[cfg(feature = "zod")]
#[model_schema(default_types(Items = Vec<String>))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListedDefault<Items> {
    pub items: Items,
}

/// A generic branded newtype whose inner is a bare type parameter, carrying string constraints
/// — held here rather than reused from `branded_newtype_tests` because each integration test
/// file compiles as its own crate.
#[cfg(all(feature = "zod", feature = "typescript", feature = "serde"))]
#[model_schema(
    minLength = 24,
    maxLength = 24,
    pattern = "^[a-f\\d]{24}$",
    default_types(IdType = String)
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConstrainedId<IdType>(pub IdType);

/// The declared default (`ConstrainedId<String>`) gets the generated `validate()`, so this
/// second inherent impl at a different instantiation is not a duplicate-definition error — it
/// would be if the generated one were a blanket `impl<IdType> ConstrainedId<IdType>`.
#[cfg(all(feature = "zod", feature = "typescript", feature = "serde"))]
impl ConstrainedId<u32> {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        if self.0 == 0 {
            return Err(vec!["id must not be zero".to_owned()]);
        }
        Ok(())
    }
}

/// A declared default naming a *constrained* generic brand at exactly its own default. The fold
/// comparison has to key on the plain rendering — not the `.min`/`.max`/`.check` chain
/// `$SchemaDefault` emits — or a constrained brand could never fold.
#[cfg(all(feature = "zod", feature = "typescript", feature = "serde"))]
#[model_schema(default_types(HolderType = ConstrainedId<String>))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstrainedEchoedDefault<HolderType> {
    pub held: HolderType,
}

/// A declared default naming a sibling whose own recorded fold key itself names a further
/// sibling at matching arguments — the fold chains two levels deep.
#[cfg(all(feature = "zod", feature = "typescript", feature = "serde"))]
#[model_schema(default_types(HolderType = ConstrainedEchoedDefault<ConstrainedId<String>>))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepEchoedDefault<HolderType> {
    pub held: HolderType,
}

#[cfg(feature = "zod")]
#[model_schema(default_types(ValueType = CycleFollower<u32>))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleLeader<ValueType> {
    pub value: ValueType,
}

#[cfg(feature = "zod")]
#[model_schema(default_types(ValueType = CycleLeader<u32>))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleFollower<ValueType> {
    pub value: ValueType,
}

/// A generic type that names itself, at the filling it was entered at — the cycle a document
/// holding one definition per name describes as a definition and a reference into it. Read only
/// where a JSON document is built.
#[cfg(feature = "jsonschema")]
#[model_schema(default_types(ValueType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recurring<ValueType> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<Box<Self>>,
    pub value: ValueType,
}

/// The same cycle, closed at a filling the outer frame was not written at: the position holds a
/// `bool` where the document is being written at a `String`, and one definition cannot describe
/// both.
#[cfg(feature = "jsonschema")]
#[model_schema(default_types(ValueType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Refilled<ValueType> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<Box<Refilled<bool>>>,
    pub value: ValueType,
}

/// A struct whose flattened source is one of its own type parameters, gated on both `serde`
/// (which reads `#[serde(flatten)]`) and `jsonschema` (which builds the document). The declared
/// filling is a scalar; the object filling reaches it from the reference site below.
#[cfg(all(feature = "serde", feature = "jsonschema"))]
#[model_schema(default_types(HeldType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatCarrier<HeldType> {
    #[serde(flatten)]
    pub held: HeldType,
    pub tag: String,
}

/// The same struct reached from a field that fills its parameter with an object, which is the
/// other end a filling can arrive from.
#[cfg(all(feature = "serde", feature = "jsonschema"))]
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatReferrer {
    pub held: FlatCarrier<Inner<u32>>,
}

/// The same seam with the object filling written in the declaration instead: the default names a
/// sibling item, so the standalone document is the one place the merge and a declared filling meet.
#[cfg(all(feature = "serde", feature = "jsonschema"))]
#[model_schema(default_types(HeldType = Envelope))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatDefaulted<HeldType> {
    #[serde(flatten)]
    pub held: HeldType,
    pub tag: String,
}

#[cfg(all(not(feature = "jsonschema"), feature = "zod"))]
#[test]
fn a_writably_keyed_map_filling_still_renders_its_default() {
    let zod = StringKeyedHolder::<HashMap<String, u32>>::zod_schema();
    assert!(
        zod.contains("StringKeyedHolder$SchemaDefault"),
        "Got:\n{zod}"
    );
}

#[cfg(not(feature = "jsonschema"))]
#[test]
fn a_parameter_with_no_default_still_expands_where_no_json_document_is_built() {
    assert_eq!(Undefaulted { id: 1_u32 }.id, 1);
}

/// Every shape a bound can reach — satisfied at the filling alone, jointly with a neighbour,
/// both at once, and beside a const that takes no filling — still expands and holds what the
/// author wrote.
#[test]
fn a_bounded_parameter_filled_at_a_type_its_bound_admits_still_expands() {
    assert_eq!(
        Labelled {
            label: "one".to_owned()
        }
        .label,
        "one"
    );
    let widened = Widened {
        narrow: 'x',
        wide: String::from('x'),
    };
    assert_eq!(widened.narrow, 'x');
    assert_eq!(widened.wide, "x");

    let annotated = Annotated {
        label: "two".to_owned(),
        source: Cow::Borrowed("three"),
    };
    assert_eq!(annotated.label, "two");
    assert_eq!(annotated.source, "three");

    let padded: Padded<String, char, 4> = Padded {
        narrow: 'y',
        wide: String::from('y'),
    };
    assert_eq!(padded.narrow, 'y');
    assert_eq!(padded.wide, "y");
}

/// Each alias named by a value, the way every other declaration shape here is: an alias that binds
/// parameters is still the type it names, and a filling of it is a map serde writes as an object.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[test]
fn a_generic_alias_expands_to_rust_that_compiles() {
    let keyed: KeyedAlias<String, u32> = HashMap::from([("k".to_owned(), 1_u32)]);
    assert_eq!(keyed["k"], 1);

    let concrete: ConcreteAlias = HashMap::from([("k".to_owned(), 2_u32)]);
    assert_eq!(concrete["k"], 2);
}

/// The pair of compile errors the attribute used to produce on any generic item: `E0107` on the
/// `impl` (dropped parameters) and `E0433` on a module named after a parameter (names no type).
/// The suite compiling is the assertion.
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

    let counted = CountedFolder {
        boxed: vec![true],
        doc: EcmDocument {
            created_at: false,
            document_id: 1_u32,
        },
        tagged: Tagged(2_u32),
    };
    assert_eq!(counted.boxed.len(), 1);
    assert!(!counted.doc.created_at);
    assert_eq!(counted.doc.document_id, 1);
    assert_eq!(counted.tagged.0, 2);
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

    let stored = StoredFolder {
        doc: EcmDocument {
            created_at: DateTime::<Utc>::from_timestamp(0, 0).unwrap(),
            document_id: ObjectId::new(),
        },
    };
    assert_eq!(stored.doc.created_at.timestamp(), 0);
    assert!(!stored.doc.document_id.to_hex().is_empty());
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
/// the only evidence it was carried through is that the constrained field is still read into the
/// borrowed form and still held to its bound by the validator.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn a_lifetime_struct_still_holds_its_field_to_its_bound() {
    assert_eq!(
        serde_json::from_str::<LifetimeStruct<'_>>(r#"{"label":""}"#)
            .unwrap()
            .validate()
            .unwrap_err(),
        vec!["'label': too short: minimum length is 1, got 0"],
        "the borrowed field was read and then held to its bound"
    );

    let accepted = serde_json::from_str::<LifetimeStruct<'_>>(r#"{"label":"x"}"#).unwrap();
    assert!(
        accepted.validate().is_ok(),
        "the value the bound admits has to pass: {:?}",
        accepted.validate().err()
    );

    let empty = LifetimeStruct {
        label: Cow::Borrowed(""),
    };
    assert_eq!(
        empty.validate().unwrap_err(),
        vec!["'label': too short: minimum length is 1, got 0"]
    );
}

/// The declared-default `validate()` for a type combining a lifetime with a type parameter still
/// enforces the constrained field's bound, and the hand-written impl at a different
/// instantiation compiles and runs without colliding with the generated one.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn a_lifetime_beside_a_type_parameter_still_pins_validate_to_the_declared_default() {
    let valid = AnnotatedConstrained {
        label: "anything".to_owned(),
        source: Cow::Borrowed("s"),
        tag: "t".to_owned(),
    };
    valid.validate().unwrap();

    let untagged = AnnotatedConstrained {
        label: "anything".to_owned(),
        source: Cow::Borrowed("s"),
        tag: String::new(),
    };
    assert_eq!(
        untagged.validate().unwrap_err(),
        vec!["'tag': too short: minimum length is 1, got 0"]
    );

    let other_instantiation = AnnotatedConstrained {
        label: 7_u32,
        source: Cow::Borrowed("s"),
        tag: String::new(),
    };
    other_instantiation.validate().unwrap();
}
