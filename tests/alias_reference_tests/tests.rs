use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tixschema::model_schema;

#[model_schema()]
pub type ReferencedDocumentId = String;

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocumentSlot {
    Primary,
    Secondary,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UsesAliasByValue {
    pub id: ReferencedDocumentId,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UsesAliasInCollections {
    pub ids: Vec<ReferencedDocumentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maybe_id: Option<ReferencedDocumentId>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UsesAliasAsMapValue {
    pub by_slot: HashMap<DocumentSlot, ReferencedDocumentId>,
}

#[model_schema()]
pub type DocumentSlotAlias = DocumentSlot;

#[model_schema()]
pub type DocumentSlotAliasChain = DocumentSlotAlias;

/// A map key written as an alias: the generated `enum_members()` call is a *type path*, so it
/// resolves through every link of the chain back to the enum that has the method.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UsesAliasAsMapKey {
    pub by_chained_slot: HashMap<DocumentSlotAliasChain, ReferencedDocumentId>,
    pub by_slot: HashMap<DocumentSlotAlias, ReferencedDocumentId>,
}

#[model_schema()]
pub type ReferencedDocumentIds = Vec<ReferencedDocumentId>;

#[model_schema()]
pub type DocumentIdsByName = HashMap<String, ReferencedDocumentId>;

#[model_schema()]
pub type DocumentIdsBySlot = HashMap<DocumentSlot, ReferencedDocumentId>;

/// Declaration order, taken both ways round. This struct names aliases that have not expanded yet,
/// so the module each reference resolves to is derived from the alias's Rust ident and nothing
/// else; [`NamesAliasesDeclaredEarlier`] names the same two once they are registered. Both have to
/// reach the same modules, or one of the two orders names a module that was never emitted.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NamesAliasesDeclaredLater {
    pub counts: Vec<LaterDeclaredCount>,
    pub ids: HashMap<String, LaterDeclaredId>,
}

#[model_schema()]
pub type LaterDeclaredId = String;

/// A rename moves what the alias is *exported* as. It does not move the module its schema is
/// published in — an override is not recoverable from the Rust ident, so a module named after the
/// override would be one no forward reference could ever name.
#[model_schema(name = "RenamedLaterDeclaredCount")]
pub type LaterDeclaredCount = u64;

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NamesAliasesDeclaredEarlier {
    pub counts: Vec<LaterDeclaredCount>,
    pub ids: HashMap<String, LaterDeclaredId>,
}

#[test]
fn struct_referencing_an_alias_expands_in_this_feature_combination() {
    let value = UsesAliasByValue {
        id: "doc-1".to_owned(),
    };
    assert_eq!(value.id, "doc-1");

    let collections = UsesAliasInCollections {
        ids: vec!["doc-2".to_owned()],
        maybe_id: None,
    };
    assert_eq!(collections.ids, vec!["doc-2".to_owned()]);
    assert!(collections.maybe_id.is_none());

    let mut by_slot = HashMap::new();
    by_slot.insert(DocumentSlot::Primary, "doc-3".to_owned());
    let map = UsesAliasAsMapValue { by_slot };
    assert_eq!(
        map.by_slot.get(&DocumentSlot::Primary),
        Some(&"doc-3".to_owned())
    );
}

#[test]
fn collection_aliases_expand_in_this_feature_combination() {
    let ids: ReferencedDocumentIds = vec!["doc-5".to_owned()];
    assert_eq!(ids, vec!["doc-5".to_owned()]);

    let by_name: DocumentIdsByName = HashMap::from([("n".to_owned(), "doc-6".to_owned())]);
    assert_eq!(by_name.get("n"), Some(&"doc-6".to_owned()));

    let by_slot: DocumentIdsBySlot = HashMap::from([(DocumentSlot::Primary, "doc-7".to_owned())]);
    assert_eq!(
        by_slot.get(&DocumentSlot::Primary),
        Some(&"doc-7".to_owned())
    );
}

#[test]
fn struct_keying_a_map_by_an_alias_of_an_enum_expands_in_this_feature_combination() {
    let mut by_slot = HashMap::new();
    by_slot.insert(DocumentSlot::Secondary, "doc-4".to_owned());
    let keyed = UsesAliasAsMapKey {
        by_slot,
        by_chained_slot: HashMap::new(),
    };
    assert_eq!(
        keyed.by_slot.get(&DocumentSlot::Secondary),
        Some(&"doc-4".to_owned())
    );
}

#[cfg(feature = "jsonschema")]
#[test]
fn alias_module_backs_the_reference_emitted_by_the_referencing_struct() {
    let schema = UsesAliasByValue::json_schema();
    let properties = schema.get("properties").unwrap();
    assert!(properties.get("id").is_some());
    assert_eq!(
        properties.get("id").unwrap(),
        &referenced_document_id_schema::Schema::json_schema()
    );
}

#[cfg(feature = "jsonschema")]
#[test]
fn alias_module_backs_the_map_value_reference() {
    let schema = UsesAliasAsMapValue::json_schema();
    let by_slot = schema.get("properties").unwrap().get("by_slot").unwrap();
    let alias_schema = referenced_document_id_schema::Schema::json_schema();
    for slot in DocumentSlot::enum_members() {
        assert_eq!(
            by_slot.get("properties").unwrap().get(&slot).unwrap(),
            &alias_schema,
            "slot {slot} in: {by_slot}"
        );
    }
}

/// The alias key must enumerate the same members the enum itself would, at both link depths —
/// nothing about the key going through an alias changes the object's properties.
#[cfg(feature = "jsonschema")]
#[test]
fn alias_keyed_map_enumerates_the_members_of_the_aliased_enum() {
    let schema = UsesAliasAsMapKey::json_schema();
    let properties = schema.get("properties").unwrap();
    let alias_schema = referenced_document_id_schema::Schema::json_schema();
    for field in ["by_slot", "by_chained_slot"] {
        let keyed = properties.get(field).unwrap().get("properties").unwrap();
        for slot in DocumentSlot::enum_members() {
            assert_eq!(keyed.get(&slot).unwrap(), &alias_schema, "{field}: {keyed}");
        }
        assert_eq!(
            keyed.as_object().unwrap().len(),
            DocumentSlot::enum_members().len(),
            "{field}: {keyed}"
        );
    }
}

/// An alias names a type, so the schema it publishes is that type's own — the same one the scalar
/// mapping gives a field written as the target directly.
#[cfg(feature = "jsonschema")]
#[test]
fn an_aliased_scalar_publishes_the_targets_schema() {
    assert_eq!(
        referenced_document_id_schema::Schema::json_schema(),
        serde_json::json!({ "type": "string" })
    );
}

/// A sibling target is carried by the one reference every position that names it carries, so the
/// alias and the type it names describe the same values.
#[cfg(feature = "jsonschema")]
#[test]
fn an_aliased_sibling_publishes_the_siblings_own_schema() {
    assert_eq!(
        document_slot_alias_schema::Schema::json_schema(),
        document_slot_schema::Schema::json_schema()
    );
}

/// The reference resolves through the registry, which answers with whatever the named alias
/// registered — so a chain of aliases lands on the enum at the end of it rather than on a link.
#[cfg(feature = "jsonschema")]
#[test]
fn an_alias_of_an_alias_resolves_to_the_type_at_the_end_of_the_chain() {
    assert_eq!(
        document_slot_alias_chain_schema::Schema::json_schema(),
        document_slot_schema::Schema::json_schema()
    );
}

#[cfg(feature = "jsonschema")]
#[test]
fn an_aliased_sequence_publishes_the_array_of_its_element() {
    assert_eq!(
        referenced_document_ids_schema::Schema::json_schema(),
        serde_json::json!({
            "type": "array",
            "items": referenced_document_id_schema::Schema::json_schema()
        })
    );
}

/// A map target describes as its own key and value do, at the alias exactly as in field position:
/// a `String` key leaves the members open under one value schema, an enum key enumerates them.
#[cfg(feature = "jsonschema")]
#[test]
fn an_aliased_map_publishes_the_object_its_key_and_value_describe() {
    let member = referenced_document_id_schema::Schema::json_schema();
    assert_eq!(
        document_ids_by_name_schema::Schema::json_schema(),
        serde_json::json!({ "type": "object", "additionalProperties": member })
    );

    let by_slot = document_ids_by_slot_schema::Schema::json_schema();
    let properties = by_slot.get("properties").unwrap();
    for slot in DocumentSlot::enum_members() {
        assert_eq!(properties.get(&slot).unwrap(), &member, "in: {by_slot}");
    }
    assert_eq!(
        properties.as_object().unwrap().len(),
        DocumentSlot::enum_members().len(),
        "in: {by_slot}"
    );
    assert_eq!(by_slot["additionalProperties"], false, "in: {by_slot}");
}

/// The stub this replaced answered every alias with an object carrying a lone `warning` key, which
/// under JSON Schema constrains nothing and so accepts every payload a slot could hold.
#[cfg(feature = "jsonschema")]
#[test]
fn no_alias_publishes_a_schema_that_validates_nothing() {
    for schema in [
        referenced_document_id_schema::Schema::json_schema(),
        referenced_document_ids_schema::Schema::json_schema(),
        document_slot_alias_schema::Schema::json_schema(),
        document_slot_alias_chain_schema::Schema::json_schema(),
        document_ids_by_name_schema::Schema::json_schema(),
        document_ids_by_slot_schema::Schema::json_schema(),
    ] {
        assert!(schema.get("warning").is_none(), "got: {schema}");
        assert!(schema.get("type").is_some(), "got: {schema}");
    }
}

#[cfg(feature = "typescript")]
#[test]
fn alias_reference_uses_the_registered_export_name() {
    let ts = UsesAliasByValue::ts_definition();
    assert!(ts.contains("ReferencedDocumentIdType"), "got: {ts}");
}

#[cfg(feature = "typescript")]
#[test]
fn alias_map_value_uses_the_registered_export_name() {
    let ts = UsesAliasAsMapValue::ts_definition();
    assert!(
        ts.contains("Record<DocumentSlot, ReferencedDocumentIdType>"),
        "got: {ts}"
    );
}

#[cfg(feature = "zod")]
#[test]
fn alias_map_value_zod_schema_name_matches_the_registered_export() {
    let zod = UsesAliasAsMapValue::zod_schema();
    assert!(
        zod.contains("ReferencedDocumentIdType$Schema"),
        "got: {zod}"
    );
}

#[cfg(feature = "zod")]
#[test]
fn alias_zod_schema_name_matches_the_reference_emitted_by_the_struct() {
    let alias_zod = referenced_document_id_schema::Schema::zod_schema();
    assert!(
        alias_zod.contains("ReferencedDocumentIdType$Schema"),
        "got: {alias_zod}"
    );
    let struct_zod = UsesAliasByValue::zod_schema();
    assert!(
        struct_zod.contains("ReferencedDocumentIdType$Schema"),
        "got: {struct_zod}"
    );
}

#[test]
fn aliases_declared_under_their_reference_expand_in_this_feature_combination() {
    let later = NamesAliasesDeclaredLater {
        counts: vec![9_u64],
        ids: HashMap::from([("n".to_owned(), "doc-8".to_owned())]),
    };
    assert_eq!(later.ids.get("n"), Some(&"doc-8".to_owned()));
    assert_eq!(later.counts, vec![9_u64]);

    let earlier = NamesAliasesDeclaredEarlier {
        counts: vec![10_u64],
        ids: HashMap::new(),
    };
    assert!(earlier.ids.is_empty());
    assert_eq!(earlier.counts, vec![10_u64]);
}

/// A struct naming an alias declared under it used to refuse the whole crate: the reference
/// assumed `later_declared_id_schema` while the alias published `later_declared_id_type_schema`,
/// and rustc reported an `E0433` for a module the author never wrote. One spelling now answers on
/// both sides, so which side of the alias the reference is written on changes nothing.
#[cfg(feature = "jsonschema")]
#[test]
fn an_alias_reference_describes_the_same_on_either_side_of_the_alias() {
    assert_eq!(
        NamesAliasesDeclaredLater::json_schema(),
        NamesAliasesDeclaredEarlier::json_schema()
    );
}

/// And what it resolves to is the alias's own module rather than something that merely exists:
/// each member carries exactly what the alias publishes, the renamed one included.
#[cfg(feature = "jsonschema")]
#[test]
fn a_forward_alias_reference_carries_what_the_alias_publishes() {
    let schema = NamesAliasesDeclaredLater::json_schema();
    let properties = schema.get("properties").unwrap();
    assert_eq!(
        properties
            .get("ids")
            .unwrap()
            .get("additionalProperties")
            .unwrap(),
        &later_declared_id_schema::Schema::json_schema(),
        "in: {schema}"
    );
    assert_eq!(
        properties.get("counts").unwrap().get("items").unwrap(),
        &later_declared_count_schema::Schema::json_schema(),
        "in: {schema}"
    );
}

/// The override and the module name come apart: the alias is exported under the name the author
/// wrote, from the module named after the Rust ident.
#[cfg(feature = "typescript")]
#[test]
fn a_renamed_alias_exports_under_the_override_from_the_module_named_for_its_ident() {
    let ts = later_declared_count_schema::Schema::ts_definition();
    assert!(ts.contains("RenamedLaterDeclaredCount"), "got: {ts}");
}

/// An alias is never exported under its Rust ident — it is given the `Type` suffix, or whatever an
/// override moves it to — so a forward reference to one always writes a name the alias's own
/// `export type` line does not. The alias answers at that name too, and every name a reference
/// writes is defined by the emission the author collects.
#[cfg(feature = "typescript")]
#[test]
fn every_name_a_forward_alias_reference_writes_is_defined_by_the_emission() {
    let forward = NamesAliasesDeclaredLater::ts_definition();
    let emission = [
        forward.clone(),
        NamesAliasesDeclaredEarlier::ts_definition(),
        later_declared_id_schema::Schema::ts_definition(),
        later_declared_count_schema::Schema::ts_definition(),
    ]
    .join("\n\n");
    for (written, referenced) in [
        ("counts: Array<LaterDeclaredCount>;", "LaterDeclaredCount"),
        (
            "ids: Partial<Record<string, LaterDeclaredId>>;",
            "LaterDeclaredId",
        ),
    ] {
        assert!(
            forward.contains(written),
            "{written} missing from: {forward}"
        );
        assert!(
            emission.contains(&format!("export type {referenced} ")),
            "{referenced} is referenced but never defined in: {emission}"
        );
    }
}

#[cfg(feature = "zod")]
#[test]
fn every_schema_a_forward_alias_reference_names_is_defined_by_the_emission() {
    let forward = NamesAliasesDeclaredLater::zod_schema();
    let emission = [
        forward.clone(),
        NamesAliasesDeclaredEarlier::zod_schema(),
        later_declared_id_schema::Schema::zod_schema(),
        later_declared_count_schema::Schema::zod_schema(),
    ]
    .join("\n\n");
    for (written, referenced) in [
        (
            "counts: z.array(LaterDeclaredCount$Schema)",
            "LaterDeclaredCount",
        ),
        (
            "ids: z.record(z.string(), LaterDeclaredId$Schema)",
            "LaterDeclaredId",
        ),
    ] {
        assert!(
            forward.contains(written),
            "{written} missing from: {forward}"
        );
        assert!(
            emission.contains(&format!("export const {referenced}$Schema")),
            "{referenced} is referenced but never defined in: {emission}"
        );
    }
}

/// A backward reference keeps the export name it always resolved to; the re-export is a second
/// name for the alias, never a rewrite of what names it.
#[cfg(feature = "typescript")]
#[test]
fn a_backward_alias_reference_still_writes_the_export_name() {
    let ts = NamesAliasesDeclaredEarlier::ts_definition();
    assert!(
        ts.contains("counts: Array<RenamedLaterDeclaredCount>;"),
        "got: {ts}"
    );
    assert!(
        ts.contains("ids: Partial<Record<string, LaterDeclaredIdType>>;"),
        "got: {ts}"
    );
}

#[cfg(feature = "zod")]
#[test]
fn a_renamed_alias_publishes_its_zod_schema_from_the_module_named_for_its_ident() {
    let zod = later_declared_count_schema::Schema::zod_schema();
    assert!(
        zod.contains("RenamedLaterDeclaredCount$Schema"),
        "got: {zod}"
    );
}

/// A build emitting no TypeScript writes an alias's binding as a bare `const`: the annotation
/// naming the type it validates is a type, and a JavaScript parser reading one stops at the `:`
/// with no initializer to read.
#[cfg(all(feature = "zod", not(feature = "typescript")))]
#[test]
fn a_javascript_build_writes_an_alias_binding_with_no_annotation() {
    for zod in [
        referenced_document_id_schema::Schema::zod_schema(),
        referenced_document_ids_schema::Schema::zod_schema(),
        document_ids_by_name_schema::Schema::zod_schema(),
    ] {
        assert!(zod.contains("export const "), "got: {zod}");
        assert!(!zod.contains("ZodType"), "got: {zod}");
        assert!(!zod.contains("$Schema:"), "got: {zod}");
    }
}
