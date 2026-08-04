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

#[cfg(feature = "typescript")]
mod typescript {
    use super::{
        Adjacent, External, Holder, Internal, LifetimeStruct, Pair, PlainConst, Positional,
        Untagged, Wrapper,
    };

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
        // The omission attribute is serde's, so only a serde build reads it and writes the key
        // optional; without serde the value stays the undefined-flavored member.
        let maybe = if cfg!(feature = "serde") {
            "  maybe?: ValueType;"
        } else {
            "  maybe: ValueType | undefined;"
        };
        assert!(ts.contains(maybe), "Got: {ts}");
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
    use super::{Pair, Wrapper};

    /// A `const` cannot be parameterised, so a parameter composes into the value as the opaque
    /// schema rather than as a `$Schema` binding no emitted module declares.
    #[test]
    fn a_parameter_composes_into_the_value_as_the_opaque_schema() {
        let zod = Wrapper::<String>::zod_schema();
        assert!(zod.contains("id: z.unknown()"), "Got: {zod}");
        assert!(zod.contains("children: z.array(z.unknown())"), "Got: {zod}");
        assert!(zod.contains("name: z.string()"), "Got: {zod}");
        assert!(!zod.contains("IdType"), "Got: {zod}");
    }

    #[test]
    fn a_parameter_is_opaque_at_every_depth_it_is_written_at() {
        let zod = Pair::<String, u32>::zod_schema();
        assert!(zod.contains("key: z.unknown()"), "Got: {zod}");
        assert!(
            zod.contains("by_key: z.record(z.string(), z.unknown())"),
            "Got: {zod}"
        );
        assert!(
            zod.contains("tuple: z.tuple([z.unknown(), z.unknown()])"),
            "Got: {zod}"
        );
        assert!(!zod.contains("KeyType"), "Got: {zod}");
        assert!(!zod.contains("ValueType"), "Got: {zod}");
    }

    /// The annotation states the type of the value beside it, and that value was composed with
    /// every parameter opaque — so the name it is written under is filled in the same way.
    #[cfg(feature = "typescript")]
    #[test]
    fn the_exported_binding_is_annotated_with_the_erased_arguments() {
        use super::{Holder, Positional};

        for (expected, zod) in [
            (
                "export const Wrapper$Schema: ZodType<Wrapper<unknown>> =",
                Wrapper::<String>::zod_schema(),
            ),
            (
                "export const Pair$Schema: ZodType<Pair<unknown, unknown>> =",
                Pair::<String, u32>::zod_schema(),
            ),
            (
                "export const Positional$Schema: ZodType<Positional<unknown>> =",
                Positional::<String>::zod_schema(),
            ),
            (
                "export const RenamedHolder$Schema: ZodType<RenamedHolder<unknown>> =",
                Holder::<String>::zod_schema(),
            ),
        ] {
            assert!(zod.contains(expected), "Got: {zod}");
        }
    }

    #[cfg(feature = "typescript")]
    #[test]
    fn every_enum_flavour_annotates_its_binding_the_same_way() {
        use super::{Adjacent, External, Internal, Untagged};

        for (flavour, zod) in [
            ("Adjacent", Adjacent::<String>::zod_schema()),
            ("Internal", Internal::<String>::zod_schema()),
            ("External", External::<String>::zod_schema()),
            ("Untagged", Untagged::<String>::zod_schema()),
        ] {
            assert!(
                zod.contains(&format!(
                    "export const {flavour}$Schema: ZodType<{flavour}<unknown>> ="
                )),
                "Got: {zod}"
            );
            assert!(zod.contains("z.unknown()"), "Got: {zod}");
            assert!(!zod.contains("IdType"), "Got: {zod}");
        }
    }
}

#[cfg(feature = "jsonschema")]
mod jsonschema {
    use super::{Pair, Wrapper};

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
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tixschema::model_schema;

#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wrapper<IdType> {
    pub children: Vec<IdType>,
    pub id: IdType,
    pub name: String,
}

#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pair<KeyType, ValueType> {
    pub by_key: HashMap<String, ValueType>,
    pub key: KeyType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maybe: Option<ValueType>,
    pub tuple: (KeyType, ValueType),
}

#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub enum Adjacent<IdType> {
    Named { id: IdType },
    Nothing,
    Single(IdType),
}

#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Internal<IdType> {
    Named { id: IdType },
    Nothing,
}

#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum External<IdType> {
    Named { id: IdType },
    Nothing,
    Single(IdType),
}

#[model_schema()]
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

#[model_schema(name = "RenamedHolder")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Holder<IdType> {
    pub id: IdType,
}

#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Positional<IdType>(pub IdType, pub String);

#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifetimeStruct<'label> {
    #[model_schema_prop(minLength = 1)]
    pub label: Cow<'label, str>,
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

    assert!(matches!(Adjacent::<String>::Nothing, Adjacent::Nothing));
    assert!(matches!(Internal::<String>::Nothing, Internal::Nothing));
    assert!(matches!(External::<String>::Nothing, External::Nothing));
    assert!(matches!(
        Untagged::<String>::Numbered { count: 1 },
        Untagged::Numbered { .. }
    ));
    assert!(matches!(PlainConst::<4>::Wide, PlainConst::Wide));
    assert_eq!(Holder { id: 1_u32 }.id, 1);
    assert_eq!(LifetimeStruct { label: "x".into() }.label, "x");
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
