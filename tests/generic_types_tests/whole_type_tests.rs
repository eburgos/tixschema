//! Whole generic types, read on every surface at once — TypeScript declaration, Zod factory, and
//! JSON document together, plus the wire where serde compiles, since a suite that asks one
//! question at a time can pass while the answers disagree with each other.
//!
//! The recursive pair at the end reaches itself while still being described: JSON writes one
//! definition and points a `$ref` back at it, and Zod defers the read so the factory reaches its
//! own memo first — pinned against the real output under `tsc` and `zod`.

/// serde's `rename_all` is what turns the declared members into the keys every surface here spells,
/// so these read the emitted declaration only where the attribute is parsed at all.
#[cfg(all(feature = "typescript", feature = "serde"))]
mod typescript {
    use super::{
        ArchiveBranch, ArchiveEntry, ArchiveEvent, ArchiveNode, ArchiveTrunk, ArchiveWire,
    };
    // The brand's intersection is read only where the value surface that writes its marker
    // compiles — see the test below.
    #[cfg(feature = "zod")]
    use super::{ArchiveId, ArchiveStamp};

    /// Every parameter the item declares is bound by the declaration, in declaration order, and
    /// each member is written at whatever the parameter was written under: bare, wrapped in a
    /// collection, made optional by serde, or handed to a sibling.
    #[test]
    fn a_whole_type_binds_every_parameter_its_members_are_written_under() {
        let ts = ArchiveEntry::<String, f64, String, u32, String>::ts_definition();
        assert!(
            ts.contains(
                "export type ArchiveEntry<IdType, DateType, TagType, SizeType, OwnerType> = {"
            ),
            "Got: {ts}"
        );
        assert!(ts.contains("  stamp: ArchiveStamp<IdType>;"), "Got: {ts}");
        assert!(ts.contains("  createdAt: DateType;"), "Got: {ts}");
        assert!(ts.contains("  tags: Array<TagType>;"), "Got: {ts}");
        assert!(
            ts.contains("  byteSize: SizeType | undefined;"),
            "Got: {ts}"
        );
        assert!(
            ts.contains("  ownersByRole: Partial<Record<string, OwnerType>>;"),
            "Got: {ts}"
        );
    }

    /// A brand is a type parameter's own spelling intersected with the marker, so a generic brand
    /// binds its parameter and spends it in the intersection. Which marker is written
    /// (`$brand<"Name">`) is Zod's business, so the intersection is read only where that surface
    /// compiles.
    #[cfg(feature = "zod")]
    #[test]
    fn a_generic_brand_is_named_with_the_argument_forwarded_to_it() {
        let brand = ArchiveId::<String>::ts_definition();
        assert!(
            brand.contains("export type ArchiveId<IdType> = IdType & $brand<\"ArchiveId\">;"),
            "Got: {brand}"
        );
        let holder = ArchiveStamp::<String>::ts_definition();
        assert!(
            holder.contains("export type ArchiveStamp<IdType> = {"),
            "Got: {holder}"
        );
        assert!(
            holder.contains("  archiveId: ArchiveId<IdType>;"),
            "Got: {holder}"
        );
    }

    /// A discriminated enum's members are its variants, each written as the object the tag joins,
    /// and the parameter is bound by the union's own declaration rather than by any one member.
    #[test]
    fn a_discriminated_generic_enum_binds_its_parameter_across_every_variant() {
        let ts = ArchiveEvent::<String>::ts_definition();
        assert!(
            ts.contains("export type ArchiveEvent<IdType> ="),
            "Got: {ts}"
        );
        assert!(ts.contains("kind: \"created\""), "Got: {ts}");
        assert!(ts.contains("id: ArchiveId<IdType>"), "Got: {ts}");
        assert!(ts.contains("kind: \"purged\""), "Got: {ts}");
        assert!(ts.contains("reason: string"), "Got: {ts}");
    }

    /// An untagged enum is the bare union of what its variants write, and the parameter reaches it
    /// through the one variant that names it.
    #[test]
    fn an_untagged_generic_enum_is_a_union_of_what_its_variants_write() {
        let ts = ArchiveWire::<String>::ts_definition();
        assert!(
            ts.contains("export type ArchiveWire<IdType> ="),
            "Got: {ts}"
        );
        assert!(ts.contains("id: IdType"), "Got: {ts}");
        assert!(ts.contains("count: number"), "Got: {ts}");
    }

    /// TypeScript names a recursive type by writing its own name inside itself, arguments and all —
    /// the declaration binds the parameter and the member spends it again at the same filling.
    #[test]
    fn a_recursive_generic_names_itself_with_the_arguments_it_was_declared_under() {
        let node = ArchiveNode::<String>::ts_definition();
        assert!(
            node.contains("export type ArchiveNode<IdType> = {"),
            "Got: {node}"
        );
        assert!(
            node.contains("  children: Array<ArchiveNode<IdType>>;"),
            "Got: {node}"
        );

        let branch = ArchiveBranch::<String>::ts_definition();
        assert!(
            branch.contains("  entries: Array<ArchiveTrunk<IdType>>;"),
            "Got: {branch}"
        );
        let trunk = ArchiveTrunk::<String>::ts_definition();
        assert!(
            trunk.contains("  branches: Array<ArchiveBranch<IdType>>;"),
            "Got: {trunk}"
        );
    }
}

/// The factory's parameter list, its cache interfaces and its argument annotations are TypeScript,
/// so a build without that surface writes the same schemas with none of them — and the member keys
/// are serde's. Read where all three compile.
#[cfg(all(feature = "zod", feature = "typescript", feature = "serde"))]
mod zod {
    use super::{
        ArchiveBranch, ArchiveEntry, ArchiveEvent, ArchiveId, ArchiveNode, ArchiveStamp,
        ArchiveTrunk, ArchiveWire,
    };

    /// A generic type publishes a factory, and every member is written from whatever fills its own
    /// parameter: the argument the factory bound, a collection of one, the optional wrap serde's
    /// attribute earns, and a sibling factory called with the argument forwarded to it.
    #[test]
    fn a_whole_type_writes_each_member_from_the_argument_that_fills_it() {
        let zod = ArchiveEntry::<String, f64, String, u32, String>::zod_schema();
        assert!(
            zod.contains("  stamp: ArchiveStamp$SchemaFactory(idType),"),
            "Got: {zod}"
        );
        assert!(zod.contains("  createdAt: dateType,"), "Got: {zod}");
        assert!(zod.contains("  tags: z.array(tagType),"), "Got: {zod}");
        assert!(
            zod.contains(
                "  byteSize: z.union([z.null().transform(() => undefined), sizeType, \
                 z.undefined()]).prefault(undefined),"
            ),
            "Got: {zod}"
        );
        assert!(
            zod.contains("  ownersByRole: z.record(z.string(), ownerType),"),
            "Got: {zod}"
        );
        assert!(
            zod.contains("export function ArchiveEntry$SchemaFactory"),
            "Got: {zod}"
        );
        assert!(!zod.contains("ArchiveEntry$Schema:"), "Got: {zod}");
    }

    /// Five parameters means five levels of memo, each keyed on one argument and holding the map
    /// the next is looked up in, so the schema is reached only through all five in order.
    #[test]
    fn a_five_parameter_factory_memoizes_one_level_per_argument() {
        let zod = ArchiveEntry::<String, f64, String, u32, String>::zod_schema();
        assert!(
            zod.contains(
                "const ArchiveEntry$SchemaFactoryCache = new WeakMap<ZodType, WeakMap<ZodType, \
                 WeakMap<ZodType, WeakMap<ZodType, WeakMap<ZodType, ArchiveEntry$SchemaOf<ZodType, \
                 ZodType, ZodType, ZodType, ZodType>>>>>>();"
            ),
            "Got: {zod}"
        );
        for below in ["byDateType", "byTagType", "bySizeType", "byOwnerType"] {
            assert!(
                zod.contains(&format!("    {below} = new WeakMap();")),
                "level {below} missing: {zod}"
            );
        }
    }

    /// A generic brand is a factory too: the brand is appended to whatever schema the argument
    /// carries, and the type holding one calls that factory rather than naming a schema.
    #[test]
    fn a_generic_brand_is_a_factory_the_holder_calls() {
        let brand = ArchiveId::<String>::zod_schema();
        assert!(brand.contains(".brand<\"ArchiveId\">()"), "Got: {brand}");
        assert!(
            brand.contains("export function ArchiveId$SchemaFactory"),
            "Got: {brand}"
        );
        let holder = ArchiveStamp::<String>::zod_schema();
        assert!(
            holder.contains("  archiveId: ArchiveId$SchemaFactory(idType),"),
            "Got: {holder}"
        );
    }

    /// Both enum shapes are built inside the factory, so a variant naming a parameter reads the
    /// argument the factory bound: a tagged one through `discriminatedUnion`, an untagged one
    /// through the bare union its variants write.
    #[test]
    fn both_generic_enum_shapes_are_built_from_the_bound_argument() {
        let tagged = ArchiveEvent::<String>::zod_schema();
        assert!(
            tagged.contains("z.discriminatedUnion(\"kind\", ["),
            "Got: {tagged}"
        );
        assert!(
            tagged.contains("  id: ArchiveId$SchemaFactory(idType),"),
            "Got: {tagged}"
        );
        assert!(
            tagged.contains("export function ArchiveEvent$SchemaFactory"),
            "Got: {tagged}"
        );

        let untagged = ArchiveWire::<String>::zod_schema();
        assert!(untagged.contains("z.union(["), "Got: {untagged}");
        assert!(untagged.contains("id: idType,"), "Got: {untagged}");
        assert!(
            untagged.contains("export function ArchiveWire$SchemaFactory"),
            "Got: {untagged}"
        );
    }

    /// A value is a value: one Zod schema cannot stand for every filling, so a recursive generic
    /// reaches itself by calling its own factory with the argument it was handed, never a
    /// `$Schema` binding a generic type does not publish. Written as a getter so the call happens
    /// after the factory reaches its own memo — what ends the recursion.
    #[test]
    fn a_self_recursive_generic_calls_its_own_factory_behind_a_deferred_read() {
        let zod = ArchiveNode::<String>::zod_schema();
        assert!(
            zod.contains(
                "  get children() { return z.array(ArchiveNode$SchemaFactory(idType)); },"
            ),
            "Got: {zod}"
        );
        assert!(!zod.contains("ArchiveNode$Schema)"), "Got: {zod}");
    }

    /// A cycle between two generic types is ended at the reference written *forward* — naming a
    /// type declared below, which a cycle cannot be built without. That one is deferred; the
    /// backward reference is read as it stands, since what's left after deferring forward
    /// references cannot cycle.
    #[test]
    fn a_generic_cycle_is_deferred_at_the_reference_written_forward() {
        let branch = ArchiveBranch::<String>::zod_schema();
        assert!(
            branch.contains(
                "  get entries() { return z.array(ArchiveTrunk$SchemaFactory(idType)); },"
            ),
            "Got: {branch}"
        );

        let trunk = ArchiveTrunk::<String>::zod_schema();
        assert!(
            trunk.contains("  branches: z.array(ArchiveBranch$SchemaFactory(idType)),"),
            "Got: {trunk}"
        );
        assert!(!trunk.contains("get branches()"), "Got: {trunk}");
    }
}

/// The documents are pinned byte for byte, and every key in them is the one serde writes.
#[cfg(all(feature = "jsonschema", feature = "serde"))]
mod json_schema {
    use super::{
        ArchiveBranch, ArchiveEntry, ArchiveEvent, ArchiveId, ArchiveNode, ArchiveStamp,
        ArchiveTrunk, ArchiveWire, Looped, TwoLoopedFillings,
    };

    /// JSON Schema has no type parameters, so the document exists at one filling: the declared one.
    /// Every member is described as whatever that filling describes as, through the same dispatch a
    /// field written at that type would take, and serde's key rules reach the document unchanged.
    #[test]
    fn a_whole_type_describes_every_member_at_its_declared_filling() {
        assert_eq!(
            serde_json::to_string(&ArchiveEntry::<String, f64, String, u32, String>::json_schema())
                .unwrap(),
            "{\"type\":\"object\",\"additionalProperties\":false,\"properties\":{\"byteSize\":{\"an\
             yOf\":[{\"type\":\"integer\"},{\"type\":\"null\"}]},\"createdAt\":{\"type\":\"number\"\
             },\"ownersByRole\":{\"type\":\"obj\
             ect\",\"additionalProperties\":{\"type\":\"string\"}},\"stamp\":{\"type\":\"object\",\
             \"additionalProperties\":false,\"properties\":{\"archiveId\":{\"type\":\"string\"}},\"\
             required\":[\"archiveId\"]},\"tags\":{\"type\":\"array\",\"items\":{\"type\":\"string\
             \"}}},\"required\":[\"createdAt\",\"ownersByRole\",\"stamp\",\"tags\"]}"
        );
    }

    /// A brand describes as what it wraps — the document has no marker to carry — so a generic
    /// brand describes as its declared filling, and the type holding it embeds that.
    #[test]
    fn a_generic_brand_describes_as_the_filling_it_wraps() {
        assert_eq!(
            serde_json::to_string(&ArchiveId::<String>::json_schema()).unwrap(),
            "{\"type\":\"string\"}"
        );

        assert_eq!(
            serde_json::to_string(&ArchiveStamp::<String>::json_schema()).unwrap(),
            "{\"type\":\"object\",\"additionalProperties\":false,\"properties\":{\"archiveId\":{\"t\
             ype\":\"string\"}},\"required\":[\"archiveId\"]}"
        );
    }

    /// A tagged enum is the `oneOf` of the objects its tag joins; an untagged one is the `anyOf` of
    /// what its variants write. The parameter is described at its declared filling in each.
    #[test]
    fn both_generic_enum_shapes_describe_at_the_declared_filling() {
        assert_eq!(
            serde_json::to_string(&ArchiveEvent::<String>::json_schema()).unwrap(),
            "{\"type\":\"object\",\"oneOf\":[{\"additionalProperties\":false,\"properties\":{\"kind\
             \":{\"type\":\"string\",\"const\":\"created\"},\"id\":{\"type\":\"string\"}},\"require\
             d\":[\"kind\",\"id\"]},{\"additionalProperties\":false,\"properties\":{\"kind\":{\"typ\
             e\":\"string\",\"const\":\"purged\"},\"reason\":{\"type\":\"string\"}},\"required\":[\
             \"kind\",\"reason\"]}]}"
        );

        assert_eq!(
            serde_json::to_string(&ArchiveWire::<String>::json_schema()).unwrap(),
            "{\"anyOf\":[{\"type\":\"object\",\"properties\":{\"count\":{\"type\":\"integer\"}},\"r\
             equired\":[\"count\"],\"additionalProperties\":false},{\"type\":\"object\",\"propertie\
             s\":{\"id\":{\"type\":\"string\"}},\"required\":[\"id\"],\"additionalProperties\":fals\
             e}]}"
        );
    }

    /// A document cannot be written to the bottom of a recursion, so the type is hoisted into
    /// `$defs` once and the recursive member points a `$ref` back at it. One definition per name
    /// *and filling* — the key carries a readable label off the filling's own `"type"` keyword
    /// plus a digest, since a bare name would let two fillings of the same generic collide.
    #[test]
    fn a_recursive_generic_is_hoisted_once_and_pointed_back_at() {
        assert_eq!(
            serde_json::to_string(&ArchiveNode::<String>::json_schema()).unwrap(),
            "{\"$defs\":{\"ArchiveNode.string-1579594ac99678fa\":{\"type\":\"object\",\"additional\
             Properties\":false,\"properties\":{\"children\":{\"type\":\"array\",\"items\":{\"$ref\
             \":\"#/$defs/ArchiveNode.string-1579594ac99678fa\"}},\"id\":{\"type\":\"string\"}},\"r\
             equired\":[\"children\",\"id\"]}},\"$ref\":\"#/$defs/ArchiveNode.string-1579594ac9967\
             8fa\"}"
        );
    }

    /// A cycle spanning two types is hoisted at whichever of them the document is built from, the
    /// other written out inline underneath it — so each of the pair is a document in its own right
    /// and the `$ref` closes back on the one that was asked for.
    #[test]
    fn a_generic_cycle_is_hoisted_at_whichever_type_the_document_is_built_from() {
        assert_eq!(
            serde_json::to_string(&ArchiveBranch::<String>::json_schema()).unwrap(),
            "{\"$defs\":{\"ArchiveBranch.string-1579594ac99678fa\":{\"type\":\"object\",\"addition\
             alProperties\":false,\"properties\":{\"entries\":{\"type\":\"array\",\"items\":{\"type\
             \":\"object\",\"additionalProperties\":false,\"properties\":{\"branches\":{\"type\":\"\
             array\",\"items\":{\"$ref\":\"#/$defs/ArchiveBranch.string-1579594ac99678fa\"}},\"id\"\
             :{\"type\":\"string\"}},\"required\":[\"branches\",\"id\"]}},\"label\":{\"type\":\"str\
             ing\"}},\"required\":[\"entries\",\"label\"]}},\"$ref\":\"#/$defs/ArchiveBranch.strin\
             g-1579594ac99678fa\"}"
        );

        assert_eq!(
            serde_json::to_string(&ArchiveTrunk::<String>::json_schema()).unwrap(),
            "{\"$defs\":{\"ArchiveTrunk.string-1579594ac99678fa\":{\"type\":\"object\",\"additiona\
             lProperties\":false,\"properties\":{\"branches\":{\"type\":\"array\",\"items\":{\"type\
             \":\"object\",\"additionalProperties\":false,\"properties\":{\"entries\":{\"type\":\"a\
             rray\",\"items\":{\"$ref\":\"#/$defs/ArchiveTrunk.string-1579594ac99678fa\"}},\"label\
             \":{\"type\":\"string\"}},\"required\":[\"entries\",\"label\"]}},\"id\":{\"type\":\"st\
             ring\"}},\"required\":[\"branches\",\"id\"]}},\"$ref\":\"#/$defs/ArchiveTrunk.string\
             -1579594ac99678fa\"}"
        );
    }

    /// Two sibling fields naming the same recursive generic at different fillings are the shape
    /// neither the in-flight cycle check nor a bare `$defs` key can tell apart — the frames are
    /// sequential, not nested. Each filling gets its own definition and `$ref`; a third field at
    /// the first filling shares that definition; each recursive member resolves back to its own
    /// filling.
    #[test]
    fn siblings_at_different_fillings_of_one_recursive_generic_each_get_their_own_definition() {
        let document = TwoLoopedFillings::json_schema();
        let defs = document["$defs"].as_object().unwrap();
        assert_eq!(
            defs.len(),
            2,
            "one definition per distinct filling: {defs:?}"
        );

        let strings_ref = &document["properties"]["strings"];
        let also_strings_ref = &document["properties"]["also_strings"];
        let numbers_ref = &document["properties"]["numbers"];
        assert_eq!(
            strings_ref, also_strings_ref,
            "two references at the same filling still share one definition: {document}"
        );
        assert_ne!(
            strings_ref, numbers_ref,
            "two references at different fillings must not share a definition: {document}"
        );

        let key_of = |reference: &serde_json::Value| {
            reference["$ref"]
                .as_str()
                .and_then(|r| r.strip_prefix("#/$defs/"))
                .unwrap()
                .to_owned()
        };
        let strings_key = key_of(strings_ref);
        let numbers_key = key_of(numbers_ref);
        assert_ne!(strings_key, numbers_key);

        // A `$ref` is a URI-reference; RFC 3986 forbids all of these in one, whatever filling built
        // the key it points at.
        for key in [&strings_key, &numbers_key] {
            for forbidden in ['"', '{', '}', '[', ']', ',', ':', ' '] {
                assert!(
                    !key.contains(forbidden),
                    "$defs key carries a character a URI-reference forbids: {forbidden:?} in {key}"
                );
            }
        }

        let strings_body = &defs[&strings_key];
        let numbers_body = &defs[&numbers_key];
        assert_eq!(
            strings_body["properties"]["value"],
            serde_json::json!({ "type": "string" })
        );
        assert_eq!(
            numbers_body["properties"]["value"],
            serde_json::json!({ "type": "integer" })
        );

        // Each definition's own recursive member closes back on itself, not on the other filling.
        assert_eq!(
            strings_body["properties"]["children"]["items"],
            serde_json::json!({ "$ref": format!("#/$defs/{strings_key}") })
        );
        assert_eq!(
            numbers_body["properties"]["children"]["items"],
            serde_json::json!({ "$ref": format!("#/$defs/{numbers_key}") })
        );

        // What each definition actually admits is what serde actually writes for that filling.
        let string_node = Looped::<String> {
            children: Vec::new(),
            value: "leaf".to_owned(),
        };
        assert_eq!(
            serde_json::to_value(&string_node).unwrap()["value"],
            serde_json::json!("leaf")
        );
        let number_node = Looped::<u32> {
            children: Vec::new(),
            value: 7_u32,
        };
        assert_eq!(
            serde_json::to_value(&number_node).unwrap()["value"],
            serde_json::json!(7_u32)
        );
    }
}

/// The README's recursive-generic example, held to the two things a pasted example has to be: the
/// README still declares it exactly as declared here, and the emission shown beside it is what the
/// generator answers with. Drift on either side fails here rather than in someone's editor.
#[cfg(all(feature = "typescript", feature = "zod"))]
mod readme {
    use super::Node;

    fn readme() -> &'static str {
        include_str!("../../README.md")
    }

    /// The README declares this, and the generator writes that. Each line is matched whole on both
    /// sides, so a spelling that is merely a prefix of the emitted one cannot pass for it.
    fn assert_readme_declares_and_shows(declaration: &str, emission: &str, lines: &[&str]) {
        assert!(
            readme().contains(declaration),
            "the README no longer declares this verbatim:\n{declaration}"
        );
        for line in lines {
            assert!(
                emission.lines().any(|written| written == *line),
                "the generator no longer writes this line: {line}\nGot: {emission}"
            );
            assert!(
                readme().lines().any(|shown| shown == *line),
                "the README no longer shows this line verbatim: {line}"
            );
        }
    }

    #[test]
    fn the_readme_recursive_example_is_declared_and_shown_as_the_generator_writes_it() {
        assert_readme_declares_and_shows(
            "pub struct Node<IdType> {\n    pub children: Vec<Self>,\n    pub id: IdType,\n}",
            &Node::<String>::ts_definition(),
            &[
                "export type Node<IdType> = {",
                "  children: Array<Node<IdType>>;",
                "  id: IdType;",
            ],
        );
        assert_readme_declares_and_shows(
            "pub struct Node<IdType> {",
            &Node::<String>::zod_schema(),
            &[
                "const buildNode$Schema = <IdType extends ZodType>(",
                "  idType: IdType,",
                ") =>",
                "  z.strictObject({",
                "  get children() { return z.array(Node$SchemaFactory(idType)); },",
                "  id: idType,",
                "});",
            ],
        );
    }
}

/// The wire the surfaces above describe, read off the value itself. A schema that agrees with the
/// other two and disagrees with serde describes nothing anyone sends.
#[cfg(feature = "serde")]
mod wire {
    use super::{ArchiveEntry, ArchiveEvent, ArchiveId, ArchiveNode, ArchiveStamp, ArchiveWire};
    use std::collections::HashMap;

    #[test]
    fn the_whole_type_writes_the_keys_its_schemas_describe() {
        let entry = ArchiveEntry::<String, f64, String, u32, String> {
            stamp: ArchiveStamp {
                archive_id: ArchiveId("doc-1".to_owned()),
            },
            created_at: 1.5_f64,
            tags: vec!["a".to_owned()],
            byte_size: None,
            owners_by_role: HashMap::from([("editor".to_owned(), "ana".to_owned())]),
        };
        let written = serde_json::to_value(&entry).unwrap();
        assert_eq!(
            written,
            serde_json::json!({
                "stamp": { "archiveId": "doc-1" },
                "createdAt": 1.5_f64,
                "tags": ["a"],
                "ownersByRole": { "editor": "ana" },
            })
        );
        // The absent key is the one the schema left out of `required` and TypeScript marked `?`.
        assert!(written.get("byteSize").is_none());
    }

    #[test]
    fn the_generic_enums_write_the_shapes_their_schemas_describe() {
        assert_eq!(
            serde_json::to_value(ArchiveEvent::Created {
                id: ArchiveId::<String>("doc-1".to_owned()),
            })
            .unwrap(),
            serde_json::json!({ "kind": "created", "id": "doc-1" })
        );
        assert_eq!(
            serde_json::to_value(ArchiveWire::<String>::Counted { count: 2_u32 }).unwrap(),
            serde_json::json!({ "count": 2_u32 })
        );
    }

    #[test]
    fn a_recursive_generic_writes_itself_all_the_way_down() {
        let node = ArchiveNode::<String> {
            id: "root".to_owned(),
            children: vec![ArchiveNode {
                id: "leaf".to_owned(),
                children: Vec::new(),
            }],
        };
        assert_eq!(
            serde_json::to_value(&node).unwrap(),
            serde_json::json!({
                "id": "root",
                "children": [{ "id": "leaf", "children": [] }],
            })
        );
    }
}

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tixschema::model_schema;

/// A brand over a parameter: the one item shape whose parameter is spent on a single written target
/// rather than on a list of members, held here so the type below can forward an argument into it.
#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArchiveId<IdType>(pub IdType);

/// The level between the brand and the whole type, so the argument the entry supplies is forwarded
/// twice before it reaches the brand that spends it.
#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveStamp<IdType> {
    pub archive_id: ArchiveId<IdType>,
}

/// Five parameters, each written under a different shape: made optional by serde, held bare, put in
/// a map's value position, handed to a sibling, and wrapped in a collection.
#[model_schema(default_types(
    IdType = String,
    DateType = f64,
    TagType = String,
    SizeType = u32,
    OwnerType = String
))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveEntry<IdType, DateType, TagType, SizeType, OwnerType> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_size: Option<SizeType>,
    pub created_at: DateType,
    pub owners_by_role: HashMap<String, OwnerType>,
    pub stamp: ArchiveStamp<IdType>,
    pub tags: Vec<TagType>,
}

/// A tagged generic enum, whose parameter reaches only one of its variants.
#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ArchiveEvent<IdType> {
    Created { id: ArchiveId<IdType> },
    Purged { reason: String },
}

/// The same question for the shape with no tag to join the variants: a bare union, whose members
/// are told apart by their keys alone.
#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArchiveWire<IdType> {
    Counted { count: u32 },
    Identified { id: IdType },
}

/// A generic type that reaches itself. Every reference around the cycle carries the same filling,
/// which is what lets one definition stand for the whole of it.
#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveNode<IdType> {
    pub children: Vec<Self>,
    pub id: IdType,
}

/// The README's own recursive example, declared here exactly as the README declares it so the two
/// cannot drift.
#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node<IdType> {
    pub children: Vec<Self>,
    pub id: IdType,
}

/// The same, spanning two types rather than one — and declared *above* the type it names, so the
/// reference that closes the cycle is written forward.
#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveBranch<IdType> {
    pub entries: Vec<ArchiveTrunk<IdType>>,
    pub label: String,
}

#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveTrunk<IdType> {
    pub branches: Vec<ArchiveBranch<IdType>>,
    pub id: IdType,
}

/// A recursive generic reached by two sibling fields at two different fillings — sequential rather
/// than nested, so neither reference is in flight while the other runs. Read by the JSON surface
/// alone, the one that hoists a recursive type into a `$defs` entry a bare name could collide
/// under.
#[cfg(all(feature = "jsonschema", feature = "serde"))]
#[model_schema(default_types(ValueType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Looped<ValueType> {
    pub children: Vec<Self>,
    pub value: ValueType,
}

#[cfg(all(feature = "jsonschema", feature = "serde"))]
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwoLoopedFillings {
    pub also_strings: Looped<String>,
    pub numbers: Looped<u32>,
    pub strings: Looped<String>,
}

/// The declaration itself is the assertion in a build that reads no surface: an item that does not
/// expand is a compile error, whatever is switched on. Every fixture is built here, so each one is
/// held to expanding under every combination rather than only under the ones that read it.
#[test]
fn every_fixture_expands_in_this_build() {
    let stamp = ArchiveStamp {
        archive_id: ArchiveId::<String>("doc-1".to_owned()),
    };
    let entry = ArchiveEntry::<String, f64, String, u32, String> {
        byte_size: None,
        created_at: 1.5_f64,
        owners_by_role: HashMap::new(),
        stamp,
        tags: Vec::new(),
    };
    assert!(entry.tags.is_empty());

    let event = ArchiveEvent::Purged {
        reason: "expired".to_owned(),
    };
    assert!(matches!(event, ArchiveEvent::<String>::Purged { .. }));
    let wire = ArchiveWire::<String>::Counted { count: 2_u32 };
    assert!(matches!(wire, ArchiveWire::Counted { count: 2_u32 }));

    let node = ArchiveNode::<String> {
        children: Vec::new(),
        id: "root".to_owned(),
    };
    assert!(node.children.is_empty());
    let readme_node = Node::<String> {
        children: Vec::new(),
        id: "root".to_owned(),
    };
    assert!(readme_node.children.is_empty());
    let trunk = ArchiveTrunk::<String> {
        branches: Vec::new(),
        id: "root".to_owned(),
    };
    let branch = ArchiveBranch::<String> {
        entries: vec![trunk],
        label: "root".to_owned(),
    };
    assert_eq!(branch.label, "root");
}
