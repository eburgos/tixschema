//! Tests for the Dart type and JSON-codec backend (`dart` feature).
//!
//! Each `#[model_schema]` item earns `dart_definition()` inside a `{snake_case}_dart` module
//! beside it (never a direct inherent `impl` — see `features::dart::dart_module_tokens` for why).

use std::collections::HashMap;

#[cfg(feature = "object_id")]
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

use tixschema::model_schema;

// ---------------------------------------------------------------------------------------------
// Structs: primitives, Option in its three flavors, Vec, HashMap<String, T>, nested references.
// ---------------------------------------------------------------------------------------------

#[model_schema()]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StockItem {
    #[model_schema_prop(ts_optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aisle: Option<String>,
    pub attributes: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin_location: Option<String>,
    pub discontinued: bool,
    pub name: String,
    #[model_schema_prop(nullable)]
    pub note: Option<String>,
    pub on_hand: u32,
    pub sku: String,
    pub tags: Vec<String>,
    pub unit_price: f64,
}

#[model_schema()]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Address {
    pub city: String,
}

#[model_schema()]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warehouse {
    pub address: Address,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_address: Option<Address>,
}

// ---------------------------------------------------------------------------------------------
// A struct carrying no field at all, and a struct whose key serde drops for an empty value although
// its type is not an `Option` — the two shapes a Dart constructor gets wrong when it reads the
// wire's own questions as if they were the language's.
// ---------------------------------------------------------------------------------------------

#[model_schema()]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockAudit;

#[model_schema()]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StockLabel {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub batch_codes: Vec<String>,
    pub printed_by: String,
}

// ---------------------------------------------------------------------------------------------
// Plain enum: an enhanced enum carrying serde's own wire string.
// ---------------------------------------------------------------------------------------------

#[model_schema()]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StockStatus {
    Backordered,
    Discontinued,
    InStock,
}

// ---------------------------------------------------------------------------------------------
// Internally tagged enum (`tag = "..."`, no `content`).
// ---------------------------------------------------------------------------------------------

#[model_schema()]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StockEvent {
    Received { quantity: u32, supplier: String },
    Reset,
    Shipped { quantity: u32 },
}

// ---------------------------------------------------------------------------------------------
// Adjacently tagged enum (`tag = "...", content = "..."`).
// ---------------------------------------------------------------------------------------------

#[model_schema()]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum DynamicValue {
    Cleared {
        #[serde(skip)]
        reason: String,
    },
    Flag(bool),
    Nothing,
    Number(i64),
}

// ---------------------------------------------------------------------------------------------
// Externally tagged enum (serde's default once a variant carries data).
// ---------------------------------------------------------------------------------------------

#[model_schema()]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentMethod {
    Card { number: String },
    Cash,
    Voucher(String),
}

// ---------------------------------------------------------------------------------------------
// Untagged enum.
// ---------------------------------------------------------------------------------------------

#[model_schema()]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ReceivedAt {
    Epoch(i64),
    Iso(String),
}

// ---------------------------------------------------------------------------------------------
// Branded newtype.
// ---------------------------------------------------------------------------------------------

#[model_schema()]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Sku(pub String);

// ---------------------------------------------------------------------------------------------
// Generics: real Dart generics on the class, converter functions on fromJson/toJson.
// ---------------------------------------------------------------------------------------------

#[model_schema(default_types(ItemType = String))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page<ItemType> {
    pub items: Vec<ItemType>,
    pub total: u32,
}

#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Either<IdType> {
    Left { value: IdType },
    Right,
}

// ---------------------------------------------------------------------------------------------
// ObjectId (feature-gated).
// ---------------------------------------------------------------------------------------------

#[cfg(feature = "object_id")]
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: ObjectId,
}

// ---------------------------------------------------------------------------------------------
// Chrono (feature-gated).
// ---------------------------------------------------------------------------------------------

#[cfg(feature = "chrono")]
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub day: chrono::NaiveDate,
    #[model_schema_prop(as_number)]
    pub logged_at: chrono::DateTime<chrono::Utc>,
}

// ---------------------------------------------------------------------------------------------
// Publishing rules: the declared ident, a `name` override, and the alias it leaves behind.
// ---------------------------------------------------------------------------------------------

#[model_schema(name = "Customer")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomerData {
    pub id: String,
}

#[model_schema()]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderWithCustomer {
    pub customer: CustomerData,
}

// ---------------------------------------------------------------------------------------------
// Type alias: a wrapper class (not a Dart typedef, which cannot carry fromJson/toJson).
// ---------------------------------------------------------------------------------------------

#[model_schema()]
pub type Barcode = String;

#[model_schema()]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductLookup {
    pub by_slot: HashMap<StockStatus, String>,
}

// ---------------------------------------------------------------------------------------------
// Every declared type is constructible — keeps the compiler from calling any of the above dead
// code, and doubles as a check that the ordinary Rust side of each declaration still behaves.
// ---------------------------------------------------------------------------------------------

#[test]
fn test_every_declared_type_is_constructible() {
    let stock_item = StockItem {
        aisle: None,
        attributes: HashMap::new(),
        bin_location: None,
        discontinued: false,
        name: "Widget".to_owned(),
        note: None,
        on_hand: 0,
        sku: String::new(),
        tags: Vec::new(),
        unit_price: 0.0,
    };
    assert_eq!(stock_item.sku, "");

    let warehouse = Warehouse {
        address: Address {
            city: "Springfield".to_owned(),
        },
        backup_address: None,
    };
    assert_eq!(warehouse.address.city, "Springfield");

    let statuses = [
        StockStatus::InStock,
        StockStatus::Backordered,
        StockStatus::Discontinued,
    ];
    assert_eq!(statuses.len(), 3);

    let events = [
        StockEvent::Received {
            quantity: 1,
            supplier: "Acme".to_owned(),
        },
        StockEvent::Shipped { quantity: 1 },
        StockEvent::Reset,
    ];
    assert_eq!(events.len(), 3);

    let audit = StockAudit;
    assert_eq!(audit, StockAudit);

    let label = StockLabel {
        batch_codes: Vec::new(),
        printed_by: "Acme".to_owned(),
    };
    assert_eq!(label.printed_by, "Acme");

    let dynamic_values = [
        DynamicValue::Cleared {
            reason: String::new(),
        },
        DynamicValue::Number(1),
        DynamicValue::Flag(true),
        DynamicValue::Nothing,
    ];
    assert_eq!(dynamic_values.len(), 4);

    let payment_methods = [
        PaymentMethod::Cash,
        PaymentMethod::Card {
            number: "4242".to_owned(),
        },
        PaymentMethod::Voucher("ABC".to_owned()),
    ];
    assert_eq!(payment_methods.len(), 3);

    let received_ats = [ReceivedAt::Epoch(0), ReceivedAt::Iso(String::new())];
    assert_eq!(received_ats.len(), 2);

    let sku = Sku(String::new());
    assert_eq!(sku.0, "");

    let page = Page {
        items: vec!["a".to_owned()],
        total: 1,
    };
    assert_eq!(page.total, 1);

    let eithers = [
        Either::Left {
            value: "x".to_owned(),
        },
        Either::Right,
    ];
    assert_eq!(eithers.len(), 2);

    let customer_data = CustomerData { id: String::new() };
    let order = OrderWithCustomer {
        customer: customer_data.clone(),
    };
    assert_eq!(order.customer, customer_data);

    let _barcode: Barcode = "0000".to_owned();

    let lookup = ProductLookup {
        by_slot: HashMap::from([(StockStatus::InStock, "A1".to_owned())]),
    };
    assert_eq!(lookup.by_slot.len(), 1);
}

#[test]
#[cfg(feature = "object_id")]
fn test_document_is_constructible() {
    let document = Document {
        id: ObjectId::new(),
    };
    assert_ne!(document.id.to_hex(), String::new());
}

#[test]
#[cfg(feature = "chrono")]
fn test_sample_is_constructible() {
    let sample = Sample {
        created_at: chrono::Utc::now(),
        day: chrono::Utc::now().date_naive(),
        logged_at: chrono::Utc::now(),
    };
    assert!(sample.created_at <= chrono::Utc::now());
}

// ---------------------------------------------------------------------------------------------
// Structs: primitives, Option in its three flavors, Vec, HashMap<String, T>, nested references.
// ---------------------------------------------------------------------------------------------

#[test]
fn test_primitive_and_string_fields() {
    let dart = stock_item_dart::dart_definition();
    assert!(
        dart.contains("final String sku;"),
        "String should map to String. Got: {dart}"
    );
    assert!(
        dart.contains("final int on_hand;"),
        "u32 should map to int, under the Rust field name (not the wire's camelCase). Got: {dart}"
    );
    assert!(
        dart.contains("final double unit_price;"),
        "f64 should map to double. Got: {dart}"
    );
    assert!(
        dart.contains("final bool discontinued;"),
        "bool should map to bool. Got: {dart}"
    );
}

#[test]
#[cfg(feature = "serde")]
fn test_option_bare_is_nullable_and_optional_and_conditionally_omitted() {
    let dart = stock_item_dart::dart_definition();
    assert!(
        dart.contains("final String? bin_location;"),
        "bare Option<T> should be T?. Got: {dart}"
    );
    assert!(
        dart.contains("this.bin_location,") && !dart.contains("required this.bin_location,"),
        "the key can be dropped, so the constructor param should not be required. Got: {dart}"
    );
    assert!(
        dart.contains(
            "if (bin_location != null) 'binLocation': (bin_location == null ? null : bin_location),"
        ),
        "bare Option<T> toJson should conditionally omit the key. Got: {dart}"
    );
    assert!(
        dart.contains(
            "bin_location: (json['binLocation'] == null ? null : json['binLocation'] as String),"
        ),
        "fromJson should read the wire key back, null or a value alike. Got: {dart}"
    );
}

#[test]
fn test_ts_optional_makes_the_constructor_param_optional() {
    let dart = stock_item_dart::dart_definition();
    assert!(
        dart.contains("final String? aisle;"),
        "ts_optional stays T? like every other Option. Got: {dart}"
    );
    assert!(
        dart.contains("this.aisle,") && !dart.contains("required this.aisle,"),
        "ts_optional's constructor param should not be required. Got: {dart}"
    );
}

#[test]
fn test_nullable_flag_is_required_and_always_written() {
    let dart = stock_item_dart::dart_definition();
    assert!(
        dart.contains("final String? note;"),
        "nullable stays T? too. Got: {dart}"
    );
    assert!(
        dart.contains("required this.note,"),
        "nullable's key is always written, so the constructor param is required. Got: {dart}"
    );
    assert!(
        dart.contains("'note': (note == null ? null : note),"),
        "nullable's toJson should always write the key, null included. Got: {dart}"
    );
}

#[test]
fn test_vec_maps_to_list() {
    let dart = stock_item_dart::dart_definition();
    assert!(
        dart.contains("final List<String> tags;"),
        "Vec<String> should map to List<String>. Got: {dart}"
    );
}

#[test]
fn test_hashmap_string_key_maps_to_map() {
    let dart = stock_item_dart::dart_definition();
    assert!(
        dart.contains("final Map<String, String> attributes;"),
        "HashMap<String, String> should map to Map<String, String>. Got: {dart}"
    );
}

#[test]
fn test_struct_fromjson_and_tojson_round_trip_shape() {
    let dart = stock_item_dart::dart_definition();
    assert!(dart.contains("factory StockItem.fromJson(Map<String, dynamic> json)"));
    assert!(dart.contains("Map<String, dynamic> toJson()"));
    assert!(dart.contains("'sku': sku,"));
    assert!(dart.contains("sku: json['sku'] as String,"));
}

#[test]
fn test_nested_type_reference_calls_its_own_fromjson_and_tojson() {
    let dart = warehouse_dart::dart_definition();
    assert!(
        dart.contains("address: Address.fromJson(json['address']),"),
        "a nested type reference should decode via its own fromJson. Got: {dart}"
    );
    assert!(
        dart.contains("'address': (address).toJson(),"),
        "a nested type reference should encode via its own toJson. Got: {dart}"
    );
    assert!(
        dart.contains("final Address? backup_address;"),
        "Option<Address> should still be Address?. Got: {dart}"
    );
}

// ---------------------------------------------------------------------------------------------
// Plain enum.
// ---------------------------------------------------------------------------------------------

#[test]
#[cfg(feature = "serde")]
fn test_plain_enum_is_an_enhanced_enum_with_wire_values() {
    let dart = stock_status_dart::dart_definition();
    assert!(dart.contains("enum StockStatus {"));
    assert!(dart.contains("inStock('instock')"));
    assert!(dart.contains("backordered('backordered')"));
    assert!(dart.contains("discontinued('discontinued')"));
    assert!(dart.contains("static StockStatus fromJson(String json)"));
    assert!(dart.contains("String toJson() => wireValue;"));
}

#[test]
#[cfg(feature = "serde")]
fn test_plain_enum_wire_values_match_what_serde_writes() {
    let value = serde_json::to_value(StockStatus::Backordered).unwrap();
    assert_eq!(value, serde_json::json!("backordered"));
    let dart = stock_status_dart::dart_definition();
    assert!(dart.contains("'backordered'"));
}

// ---------------------------------------------------------------------------------------------
// Internally tagged enum.
// ---------------------------------------------------------------------------------------------

#[test]
fn test_internally_tagged_enum_is_a_sealed_hierarchy() {
    let dart = stock_event_dart::dart_definition();
    assert!(dart.contains("sealed class StockEvent {"));
    assert!(dart.contains("class StockEventReceived extends StockEvent {"));
    assert!(dart.contains("class StockEventShipped extends StockEvent {"));
    assert!(dart.contains("class StockEventReset extends StockEvent {"));
}

#[test]
#[cfg(feature = "serde")]
fn test_internally_tagged_named_variant_merges_the_tag_into_the_same_object() {
    let dart = stock_event_dart::dart_definition();
    assert!(
        dart.contains("'kind': 'received',"),
        "the tag entry should be written beside the variant's own fields. Got: {dart}"
    );
    assert!(dart.contains("final int quantity;"));
    assert!(dart.contains("final String supplier;"));
    assert!(
        dart.contains("json['kind'] as String"),
        "dispatch should read the tag key. Got: {dart}"
    );
}

#[test]
#[cfg(feature = "serde")]
fn test_internally_tagged_unit_variant_writes_only_the_tag() {
    let dart = stock_event_dart::dart_definition();
    assert!(
        dart.contains("{ 'kind': 'reset' }"),
        "a Unit variant under internal tagging writes just the tag. Got: {dart}"
    );
}

#[test]
#[cfg(feature = "serde")]
fn test_internally_tagged_wire_shape_matches_serde() {
    let received = StockEvent::Received {
        quantity: 5,
        supplier: "Acme".to_owned(),
    };
    let value = serde_json::to_value(&received).unwrap();
    assert_eq!(
        value,
        serde_json::json!({ "kind": "received", "quantity": 5_i32, "supplier": "Acme" })
    );
    let reset_value = serde_json::to_value(StockEvent::Reset).unwrap();
    assert_eq!(reset_value, serde_json::json!({ "kind": "reset" }));
}

// ---------------------------------------------------------------------------------------------
// Adjacently tagged enum.
// ---------------------------------------------------------------------------------------------

#[test]
#[cfg(feature = "serde")]
fn test_adjacently_tagged_tuple_single_writes_tag_and_content() {
    let dart = dynamic_value_dart::dart_definition();
    assert!(dart.contains("sealed class DynamicValue {"));
    assert!(
        dart.contains("{ 'type': 'Number', 'value': value }"),
        "adjacent tagging writes tag and content side by side. Got: {dart}"
    );
    assert!(
        dart.contains("json['value']"),
        "decode should read the content key. Got: {dart}"
    );
}

#[test]
#[cfg(feature = "serde")]
fn test_adjacently_tagged_unit_variant_has_no_content_key() {
    let dart = dynamic_value_dart::dart_definition();
    assert!(
        dart.contains("{ 'type': 'Nothing' }"),
        "a Unit variant under adjacent tagging writes no content key at all. Got: {dart}"
    );
}

#[test]
#[cfg(feature = "serde")]
fn test_adjacently_tagged_wire_shape_matches_serde() {
    let value = serde_json::to_value(DynamicValue::Number(7)).unwrap();
    assert_eq!(
        value,
        serde_json::json!({ "type": "Number", "value": 7_i32 })
    );
    let nothing = serde_json::to_value(DynamicValue::Nothing).unwrap();
    assert_eq!(nothing, serde_json::json!({ "type": "Nothing" }));
}

// ---------------------------------------------------------------------------------------------
// Externally tagged enum.
// ---------------------------------------------------------------------------------------------

#[test]
fn test_externally_tagged_unit_variant_is_a_bare_string() {
    let dart = payment_method_dart::dart_definition();
    assert!(
        dart.contains("if (json is String) { switch (json) { case 'Cash':"),
        "a Unit variant under external tagging dispatches off a bare string. Got: {dart}"
    );
    assert!(
        dart.contains("@override dynamic toJson() => 'Cash';"),
        "and encodes back to that bare string. Got: {dart}"
    );
}

#[test]
fn test_externally_tagged_named_and_tuple_variants_wrap_by_key() {
    let dart = payment_method_dart::dart_definition();
    assert!(
        dart.contains("json.containsKey('Card')") || dart.contains("case 'Card':"),
        "a Named variant is wrapped under its own variant-name key. Got: {dart}"
    );
    assert!(
        dart.contains("'Card': {"),
        "toJson should nest the fields under the variant-name key. Got: {dart}"
    );
    assert!(
        dart.contains("'Voucher':"),
        "a TupleSingle variant is also wrapped under its own key. Got: {dart}"
    );
}

#[test]
#[cfg(feature = "serde")]
fn test_externally_tagged_wire_shape_matches_serde() {
    assert_eq!(
        serde_json::to_value(PaymentMethod::Cash).unwrap(),
        serde_json::json!("Cash")
    );
    assert_eq!(
        serde_json::to_value(PaymentMethod::Card {
            number: "4242".to_owned()
        })
        .unwrap(),
        serde_json::json!({ "Card": { "number": "4242" } })
    );
    assert_eq!(
        serde_json::to_value(PaymentMethod::Voucher("ABC".to_owned())).unwrap(),
        serde_json::json!({ "Voucher": "ABC" })
    );
}

// ---------------------------------------------------------------------------------------------
// Untagged enum.
// ---------------------------------------------------------------------------------------------

#[test]
#[cfg(feature = "serde")]
fn test_untagged_enum_tries_each_variant_in_turn() {
    let dart = received_at_dart::dart_definition();
    assert!(dart.contains("sealed class ReceivedAt {"));
    assert!(dart.contains("try { return ReceivedAtEpoch("));
    assert!(dart.contains("try { return ReceivedAtIso("));
    assert!(dart.contains("throw ArgumentError('No variant of ReceivedAt matched');"));
}

#[test]
#[cfg(feature = "serde")]
fn test_untagged_wire_shape_matches_serde() {
    assert_eq!(
        serde_json::to_value(ReceivedAt::Epoch(1_700_000_000)).unwrap(),
        serde_json::json!(1_700_000_000_i64)
    );
    assert_eq!(
        serde_json::to_value(ReceivedAt::Iso("2025-01-01".to_owned())).unwrap(),
        serde_json::json!("2025-01-01")
    );
}

// ---------------------------------------------------------------------------------------------
// Branded newtype.
// ---------------------------------------------------------------------------------------------

#[test]
fn test_branded_newtype_wraps_the_inner_value() {
    let dart = sku_dart::dart_definition();
    assert!(dart.contains("class Sku {"));
    assert!(dart.contains("const Sku(this.value);"));
    assert!(dart.contains("final String value;"));
    assert!(dart.contains("factory Sku.fromJson(dynamic json) => Sku(json as String);"));
    assert!(dart.contains("dynamic toJson() => value;"));
}

#[test]
#[cfg(feature = "serde")]
fn test_branded_newtype_wire_shape_is_the_bare_inner_value() {
    let value = serde_json::to_value(Sku("ABC-123".to_owned())).unwrap();
    assert_eq!(value, serde_json::json!("ABC-123"));
}

// ---------------------------------------------------------------------------------------------
// Generics.
// ---------------------------------------------------------------------------------------------

#[test]
fn test_generic_struct_stays_generic_in_dart() {
    let dart = page_dart::dart_definition();
    assert!(
        dart.contains("class Page<ItemType> {"),
        "the Dart class should stay literally generic. Got: {dart}"
    );
    assert!(
        dart.contains("final List<ItemType> items;"),
        "a field using the type parameter should render it bare. Got: {dart}"
    );
}

#[test]
fn test_generic_struct_codec_threads_converter_functions() {
    let dart = page_dart::dart_definition();
    assert!(
        dart.contains(
            "factory Page.fromJson(Map<String, dynamic> json, ItemType Function(dynamic) itemTypeFromJson)"
        ),
        "fromJson should take one converter per type parameter. Got: {dart}"
    );
    assert!(
        dart.contains("Map<String, dynamic> toJson(dynamic Function(ItemType) itemTypeToJson)"),
        "toJson should take the matching encode converter. Got: {dart}"
    );
    assert!(
        dart.contains("itemTypeFromJson(e)"),
        "decoding a List<ItemType> should call the converter per element. Got: {dart}"
    );
    assert!(
        dart.contains("itemTypeToJson(e)"),
        "encoding a List<ItemType> should call the converter per element. Got: {dart}"
    );
}

#[test]
#[cfg(feature = "serde")]
fn test_generic_tagged_enum_threads_converters_through_the_whole_hierarchy() {
    let dart = either_dart::dart_definition();
    assert!(dart.contains("sealed class Either<IdType> {"));
    assert!(dart.contains("class EitherLeft<IdType> extends Either<IdType> {"));
    assert!(dart.contains("class EitherRight<IdType> extends Either<IdType> {"));
    assert!(dart.contains(
        "factory Either.fromJson(Map<String, dynamic> json, IdType Function(dynamic) idTypeFromJson)"
    ));
    assert!(dart.contains("idTypeFromJson(json['value'])"));
}

// ---------------------------------------------------------------------------------------------
// ObjectId (feature-gated).
// ---------------------------------------------------------------------------------------------

#[test]
#[cfg(feature = "object_id")]
fn test_object_id_maps_to_a_bare_objectid_reference() {
    let dart = document_dart::dart_definition();
    assert!(
        dart.contains("final ObjectId id;"),
        "ObjectId should be a bare reference, like TypeScript's. Got: {dart}"
    );
    assert!(dart.contains("id: ObjectId.fromJson(json['id']),"));
    assert!(dart.contains("'id': (id).toJson(),"));
}

// ---------------------------------------------------------------------------------------------
// Chrono (feature-gated).
// ---------------------------------------------------------------------------------------------

#[test]
#[cfg(feature = "chrono")]
fn test_datetime_maps_to_dart_datetime_by_default() {
    let dart = sample_dart::dart_definition();
    assert!(
        dart.contains("final DateTime created_at;"),
        "DateTime<Tz> should map to Dart's own DateTime. Got: {dart}"
    );
    assert!(dart.contains("created_at: DateTime.parse(json['created_at'] as String),"));
    assert!(dart.contains("'created_at': (created_at).toIso8601String(),"));
}

#[test]
#[cfg(feature = "chrono")]
fn test_datetime_as_number_maps_to_int() {
    let dart = sample_dart::dart_definition();
    assert!(
        dart.contains("final int logged_at;"),
        "as_number should render as epoch-millis int, like TypeScript's number. Got: {dart}"
    );
}

#[test]
#[cfg(feature = "chrono")]
fn test_naive_date_maps_to_string() {
    let dart = sample_dart::dart_definition();
    assert!(
        dart.contains("final String day;"),
        "NaiveDate should map to a plain ISO string. Got: {dart}"
    );
}

// ---------------------------------------------------------------------------------------------
// Publishing rules.
// ---------------------------------------------------------------------------------------------

#[test]
fn test_name_override_publishes_under_the_override_and_a_typedef_under_the_ident() {
    let dart = customer_data_dart::dart_definition();
    assert!(
        dart.contains("class Customer {"),
        "the class should be named after the override, not the Rust ident. Got: {dart}"
    );
    assert!(
        !dart.contains("class CustomerData {"),
        "the Rust ident should not itself become a class. Got: {dart}"
    );
    assert!(
        dart.contains("typedef CustomerData = Customer;"),
        "the Rust ident should still resolve, as a typedef. Got: {dart}"
    );
}

#[test]
fn test_a_reference_to_a_renamed_type_uses_its_published_name() {
    let dart = order_with_customer_dart::dart_definition();
    assert!(
        dart.contains("final Customer customer;"),
        "a field referencing a renamed type should use the published name. Got: {dart}"
    );
    assert!(dart.contains("customer: Customer.fromJson(json['customer']),"));
}

// ---------------------------------------------------------------------------------------------
// Type alias and enum-keyed maps.
// ---------------------------------------------------------------------------------------------

#[test]
fn test_alias_publishes_a_wrapper_class_with_its_own_codec() {
    let dart = barcode_dart::dart_definition();
    assert!(
        dart.contains("class BarcodeType {"),
        "an alias with no name override takes the Type suffix, like TypeScript's. Got: {dart}"
    );
    assert!(dart.contains("final String value;"));
    assert!(
        dart.contains("factory BarcodeType.fromJson(dynamic json) => BarcodeType(json as String);")
    );
    assert!(
        dart.contains("typedef Barcode = BarcodeType;"),
        "the Rust ident should resolve through a typedef. Got: {dart}"
    );
}

#[test]
fn test_enum_keyed_map_decodes_and_encodes_through_the_enum_itself() {
    let dart = product_lookup_dart::dart_definition();
    assert!(
        dart.contains("Map<StockStatus, String>"),
        "an enum-keyed map should keep the enum as the Dart key type, not its wire string. Got: {dart}"
    );
    assert!(
        dart.contains("StockStatus.fromJson(e.key)"),
        "decoding an enum key should go through the enum's own fromJson. Got: {dart}"
    );
    assert!(
        dart.contains("(e.key).toJson()"),
        "encoding an enum key should go through the enum's own toJson. Got: {dart}"
    );
}

// ---------------------------------------------------------------------------------------------
// A constructor's parameter list: the empty one, and the one whose requiredness the wire's own
// key-dropping must not decide.
// ---------------------------------------------------------------------------------------------

#[test]
fn test_a_struct_with_no_fields_writes_no_parameter_group_at_all() {
    let dart = stock_audit_dart::dart_definition();
    assert!(
        dart.contains("const StockAudit();"),
        "Dart's named-parameter group has to name at least one parameter: `const StockAudit({{}});` \
         is not an empty parameter list but a parse error, `missing_identifier` (\"Expected an \
         identifier\"). Got: {dart}"
    );
    assert!(
        !dart.contains("({})"),
        "no empty brace pair survives anywhere in the class. Got: {dart}"
    );
}

#[test]
#[cfg(feature = "serde")]
fn test_a_struct_shaped_variant_with_no_fields_writes_no_parameter_group_either() {
    let dart = dynamic_value_dart::dart_definition();
    assert!(
        dart.contains("const DynamicValueCleared();"),
        "a struct-shaped variant whose every field serde keeps off the wire carries no \
         parameter either, and builds its subclass the way a fieldless struct builds its class. \
         Got: {dart}"
    );
    assert!(
        !dart.contains("({})"),
        "no empty brace pair survives anywhere in the enum's subclasses. Got: {dart}"
    );
}

#[test]
#[cfg(feature = "serde")]
fn test_a_dropped_key_on_a_field_that_is_not_an_option_stays_required() {
    let dart = stock_label_dart::dart_definition();
    assert!(
        dart.contains("final List<String> batch_codes;"),
        "a Vec is not an Option, whatever serde does with its key. Got: {dart}"
    );
    assert!(
        dart.contains("required this.batch_codes,"),
        "a parameter that is not `required` falls back to `null`, which a non-nullable type \
         refuses: `missing_default_value_for_parameter`. Got: {dart}"
    );
    assert!(
        dart.contains("'batchCodes': batch_codes.map((e) => e).toList(),")
            && !dart.contains("if (batch_codes != null)"),
        "a field with no `null` in its Dart type has no absence to test, and testing one anyway \
         is `unnecessary_null_comparison`; its key is written every time, which the `default` \
         that drops it reads back on the other side. Got: {dart}"
    );
}

#[test]
#[cfg(feature = "serde")]
fn test_an_optional_reference_spells_the_null_away_before_calling_its_own_codec() {
    let dart = warehouse_dart::dart_definition();
    assert!(
        dart.contains("(backup_address == null ? null : (backup_address!).toJson())"),
        "Dart narrows a *private* final field inside an `== null` guard and a published one \
         never, so the guard alone does not let `toJson` be called on the field it guards: \
         `unchecked_use_of_nullable_value`. Got: {dart}"
    );
}

#[test]
#[cfg(feature = "serde")]
fn test_an_optional_scalar_calls_nothing_and_so_spells_no_null_away() {
    let dart = stock_item_dart::dart_definition();
    assert!(
        dart.contains("'note': (note == null ? null : note),"),
        "a value that encodes as itself calls nothing on the field, and a `!` there would be \
         `unnecessary_non_null_assertion` noise of the opposite kind. Got: {dart}"
    );
}
