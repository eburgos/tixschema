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
        &referenced_document_id_type_schema::Schema::json_schema()
    );
}

#[cfg(feature = "jsonschema")]
#[test]
fn alias_module_backs_the_map_value_reference() {
    let schema = UsesAliasAsMapValue::json_schema();
    let by_slot = schema.get("properties").unwrap().get("by_slot").unwrap();
    let alias_schema = referenced_document_id_type_schema::Schema::json_schema();
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
    let alias_schema = referenced_document_id_type_schema::Schema::json_schema();
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
    let alias_zod = referenced_document_id_type_schema::Schema::zod_schema();
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
