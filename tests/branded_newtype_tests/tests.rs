// Tests requiring both zod and typescript features
#[cfg(all(feature = "zod", feature = "typescript"))]
mod zod_ts_tests {
    use super::*;

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct CorrelationId(pub String);

    // Branded newtype with doc comment description
    /// A unique document identifier.
    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct DocId(pub String);

    // Generic branded newtype with doc comment and example
    /// Generic document identifier.
    ///
    /// - `DocumentId<String>` for API/HTTP layer.
    /// - `DocumentId<ObjectId>` for `MongoDB` layer.
    ///
    /// ```rust example
    /// DocumentId("64de3d95ff45b119e5b53a7e".to_string())
    /// ```
    #[model_schema(default_types(IdType = String))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct DocumentId<IdType>(pub IdType);

    #[model_schema(default_types(IdType = String))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct RoleId<IdType>(pub IdType);

    // Integer inner type branded newtype
    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct SequenceNum(pub u64);

    #[test]
    fn test_branded_newtype_ts_definition() {
        let ts = RoleId::<String>::ts_definition();
        assert!(
            ts.contains("export type RoleId<IdType> = IdType & $brand<\"RoleId\">"),
            "Got: {ts}"
        );
    }

    /// The parameter is what the brand wraps, and a value surface reaches it through the argument
    /// the enclosing factory binds for it — so the brand lands on the schema the caller supplied
    /// rather than on a value that admits anything whatever it was filled with. The TypeScript
    /// type beside it keeps `IdType`, its own declaration binding it for real.
    #[test]
    fn test_branded_newtype_zod_schema() {
        let zod = RoleId::<String>::zod_schema();
        assert!(
            zod.contains("export const RoleId$SchemaFactory = <IdType extends ZodType>("),
            "Got: {zod}"
        );
        assert!(zod.contains("idType.meta({"), "Got: {zod}");
        assert!(zod.contains("}).brand<\"RoleId\">();"), "Got: {zod}");
        assert!(!zod.contains("z.unknown()"), "Got: {zod}");
        assert!(!zod.contains("$ZodBranded"), "Got: {zod}");
    }

    #[test]
    fn test_branded_newtype_preserves_generic_param_name() {
        let ts = RoleId::<String>::ts_definition();
        // Should contain IdType not T or any other name
        assert!(
            ts.contains("IdType"),
            "Should preserve generic param name. Got: {ts}"
        );
    }

    #[test]
    fn test_branded_newtype_non_generic() {
        let ts = CorrelationId::ts_definition();
        assert!(
            ts.contains("export type CorrelationId = string & $brand<\"CorrelationId\">"),
            "Got: {ts}"
        );
    }

    #[test]
    fn test_branded_newtype_non_generic_zod() {
        let zod = CorrelationId::zod_schema();
        assert!(zod.contains("z.string().brand"), "Got: {zod}");
        assert!(
            zod.contains("$ZodBranded<ZodString, \"CorrelationId\">"),
            "Got: {zod}"
        );
    }

    #[test]
    fn test_branded_newtype_non_generic_zod_schema_content() {
        let zod = CorrelationId::zod_schema();
        // Should contain the raw schema definition
        assert!(
            zod.contains("const CorrelationId$RawSchema = z.string().brand"),
            "Should contain raw schema. Got: {zod}"
        );
        // Should contain the exported typed schema
        assert!(
            zod.contains("export const CorrelationId$Schema: $ZodBranded<ZodString, \"CorrelationId\"> = CorrelationId$RawSchema"),
            "Should contain exported schema referencing raw schema. Got: {zod}"
        );
    }

    #[test]
    fn test_branded_newtype_integer_inner() {
        let ts = SequenceNum::ts_definition();
        // Non-generic u64 maps to "number" in TypeScript
        assert!(
            ts.contains("export type SequenceNum = number & $brand<\"SequenceNum\">"),
            "Should have number branded type. Got: {ts}"
        );

        let zod = SequenceNum::zod_schema();
        // Zod should use z.number().int() for u64
        assert!(
            zod.contains("z.number().int()"),
            "Should use z.number().int() for u64. Got: {zod}"
        );
        assert!(
            zod.contains("SequenceNum$RawSchema"),
            "Should contain raw schema. Got: {zod}"
        );
    }

    #[test]
    fn test_branded_newtype_zod_has_description() {
        let zod = DocId::zod_schema();
        assert!(
            zod.contains("description:"),
            "Zod schema should contain description. Got:\n{zod}"
        );
        assert!(
            zod.contains("A unique document identifier"),
            "Zod schema should contain doc comment text. Got:\n{zod}"
        );
        // Must still have $Schema line after .meta()
        assert!(
            zod.contains("export const DocId$Schema:"),
            "Zod schema should have $Schema after .meta(). Got:\n{zod}"
        );
    }

    #[test]
    fn test_branded_newtype_generic_with_example() {
        let zod = DocumentId::<String>::zod_schema();
        assert!(
            zod.contains("description:"),
            "Should contain description. Got:\n{zod}"
        );
        assert!(
            zod.contains("Generic document identifier"),
            "Should contain doc comment text. Got:\n{zod}"
        );
        assert!(
            zod.contains("example:"),
            "Should contain example from doc comment. Got:\n{zod}"
        );
        // The example lands inside the one `.meta({` the brand writes, and the binding the item
        // publishes still stands after it.
        assert!(
            zod.contains("  example: \"64de3d95ff45b119e5b53a7e\",\n}).brand<\"DocumentId\">();"),
            "Should have the example inside the described block. Got:\n{zod}"
        );
        assert!(
            zod.contains("export const DocumentId$SchemaFactory = "),
            "Should have the factory after example injection. Got:\n{zod}"
        );
    }

    #[test]
    fn test_branded_newtype_generic_schema_example_method() {
        let example = DocumentId::<String>::schema_example();
        assert_eq!(
            example.as_str().unwrap(),
            "64de3d95ff45b119e5b53a7e",
            "schema_example() should return the inner value. Got: {example}"
        );
    }
}

// Tests for branded newtypes with string constraints
#[cfg(all(feature = "zod", feature = "typescript", feature = "serde"))]
mod constrained_branded_tests {
    use super::*;

    // Pattern-only constraint
    #[model_schema(pattern = "^[0-9a-fA-F]{24}$")]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct ObjectIdStr(pub String);

    #[model_schema(pattern = "^[a-z0-9_]+$", minLength = 3, maxLength = 50)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct SlugId(pub String);

    /// A pattern simple enough that a regex engine is avoidable work — the brand-side spelling of
    /// what a consumer denying `clippy::nursery` cannot compile if the validator builds a regex
    /// for it anyway.
    #[model_schema(pattern = "^/")]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct MountPath(pub String);

    #[test]
    fn test_constrained_branded_zod_has_constraints() {
        let zod = SlugId::zod_schema();
        assert!(
            zod.contains(".min(3)"),
            "Should contain minLength constraint. Got:\n{zod}"
        );
        assert!(
            zod.contains(".max(50)"),
            "Should contain maxLength constraint. Got:\n{zod}"
        );
        assert!(
            zod.contains(".check(z.regex(/^[a-z0-9_]+$/))"),
            "Should contain pattern constraint. Got:\n{zod}"
        );
        assert!(
            zod.contains(".brand"),
            "Should still have brand. Got:\n{zod}"
        );
        assert!(
            zod.contains("export const SlugId$Schema:"),
            "Should have $Schema. Got:\n{zod}"
        );
    }

    #[test]
    fn test_constrained_branded_validate_pass() {
        let valid = SlugId("hello_world".to_owned());
        valid.validate().unwrap();
    }

    #[test]
    fn test_constrained_branded_validate_too_short() {
        let short = SlugId("ab".to_owned());
        let result = short.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err()[0].contains("too short"));
    }

    #[test]
    fn test_constrained_branded_validate_too_long() {
        let long = SlugId("a".repeat(51));
        let result = long.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err()[0].contains("too long"));
    }

    #[test]
    fn test_constrained_branded_validate_bad_pattern() {
        let bad = SlugId("UPPERCASE".to_owned());
        let result = bad.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err()[0].contains("pattern"));
    }

    #[test]
    fn test_constrained_branded_serde_rejects_invalid() {
        // Serde should reject values that don't match constraints
        let too_short: Result<SlugId, _> = serde_json::from_str("\"ab\"");
        assert!(
            too_short.is_err(),
            "Should reject too-short value via serde"
        );

        let bad_pattern: Result<SlugId, _> = serde_json::from_str("\"UPPERCASE\"");
        assert!(bad_pattern.is_err(), "Should reject bad pattern via serde");
    }

    #[test]
    fn test_constrained_branded_serde_accepts_valid() {
        let result: Result<SlugId, _> = serde_json::from_str("\"hello_world\"");
        assert!(result.is_ok(), "Should accept valid value via serde");
        assert_eq!(result.unwrap(), SlugId("hello_world".to_owned()));
    }

    /// The brand's simple pattern turns away the same values the regex would have, with the same
    /// words, and still reaches every surface as written.
    #[test]
    fn test_anchored_single_character_prefix_branded() {
        let zod = MountPath::zod_schema();
        assert!(
            zod.contains(".check(z.regex(/^\\//))"),
            "Should contain pattern. Got:\n{zod}"
        );

        MountPath("/var/log".to_owned()).validate().unwrap();

        let result = MountPath("var/log".to_owned()).validate();
        assert_eq!(
            result.unwrap_err()[0],
            "value does not match pattern '^/'",
            "Rejection should read exactly as the regex path words it"
        );

        assert!(
            serde_json::from_str::<MountPath>("\"/etc\"").is_ok(),
            "Should accept a rooted path via serde"
        );
        assert!(
            serde_json::from_str::<MountPath>("\"etc\"").is_err(),
            "Should reject an unrooted path via serde"
        );
    }

    #[test]
    fn test_pattern_only_branded() {
        let zod = ObjectIdStr::zod_schema();
        assert!(
            zod.contains(".check(z.regex(/^[0-9a-fA-F]{24}$/))"),
            "Should contain pattern. Got:\n{zod}"
        );
        // Should not contain min/max
        assert!(!zod.contains(".min("), "Should not have min. Got:\n{zod}");
        assert!(!zod.contains(".max("), "Should not have max. Got:\n{zod}");

        let valid = ObjectIdStr("507f1f77bcf86cd799439011".to_owned());
        valid.validate().unwrap();

        let invalid = ObjectIdStr("not-a-hex-id".to_owned());
        assert!(invalid.validate().is_err());
    }
}

#[cfg(all(feature = "object_id", feature = "serde", feature = "zod"))]
mod constrained_objectid_branded_tests {
    use super::*;
    use mongodb::bson::oid::ObjectId;

    #[model_schema(pattern = "^[0-9a-fA-F]{24}$")]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct StrictObjectId(pub ObjectId);

    #[test]
    fn test_objectid_branded_validate_pass() {
        let oid = ObjectId::parse_str("507f1f77bcf86cd799439011").unwrap();
        let valid = StrictObjectId(oid);
        valid.validate().unwrap();
    }

    #[test]
    fn test_objectid_branded_serde_accepts_valid() {
        let json = r#"{"$oid": "507f1f77bcf86cd799439011"}"#;
        let result: Result<StrictObjectId, _> = serde_json::from_str(json);
        assert!(result.is_ok(), "Should accept valid ObjectId via serde");
    }

    /// The wire an `ObjectId` brand is described against: serde writes the extended-JSON `$oid`
    /// object, never the bare hex, for the brand exactly as for the `ObjectId` it wraps.
    #[test]
    fn an_objectid_brand_writes_the_oid_object_its_inner_writes() {
        let oid = ObjectId::parse_str("507f1f77bcf86cd799439011").unwrap();
        assert_eq!(
            serde_json::to_string(&StrictObjectId(oid)).unwrap(),
            r#"{"$oid":"507f1f77bcf86cd799439011"}"#
        );
        assert_eq!(
            serde_json::to_string(&oid).unwrap(),
            serde_json::to_string(&StrictObjectId(oid)).unwrap()
        );
    }
}

/// The three surfaces of an `ObjectId`-inner brand, pinned against the `$oid` object serde writes
/// for it. The `ObjectId` wire form is an object, so no surface may describe the brand as a string.
#[cfg(all(
    feature = "object_id",
    feature = "serde",
    feature = "zod",
    feature = "typescript",
    feature = "jsonschema"
))]
mod objectid_branded_surface_tests {
    use super::*;
    use mongodb::bson::oid::ObjectId;

    const OID_ZOD_BASE: &str =
        r#"z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" })"#;

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct PlainObjectId(pub ObjectId);

    #[model_schema(pattern = "^[0-9a-fA-F]{24}$")]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct HexObjectId(pub ObjectId);

    /// A brand whose pattern is wider than the hex the type holds — the shape that tells layering
    /// from replacement, since a surface holding only the brand's admits strings no `ObjectId` can
    /// ever be. Spelled as a class rather than with `.`, which the pattern guard refuses for its
    /// cross-engine divergence; `[a-z0-9]` still admits every lowercase letter, so the `zzz…` probe
    /// passes the brand's pattern and only the hex turns it away.
    #[model_schema(pattern = "^[a-z0-9]{24}$")]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct WideObjectId(pub ObjectId);

    #[model_schema(no_display)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct ObjectIdList(pub Vec<ObjectId>);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct HoldsObjectId {
        pub id: ObjectId,
    }

    #[test]
    fn an_unconstrained_objectid_brand_describes_the_oid_object_on_every_surface() {
        assert_eq!(
            PlainObjectId::ts_definition(),
            r#"export type PlainObjectId = ObjectId & $brand<"PlainObjectId">;"#
        );
        let zod = PlainObjectId::zod_schema();
        assert!(
            zod.contains(&format!(
                "const PlainObjectId$RawSchema = {OID_ZOD_BASE} }}).brand<"
            )),
            "Got:\n{zod}"
        );
        assert!(
            zod.contains(r#"PlainObjectId$Schema: $ZodBranded<ZodObject, "PlainObjectId">"#),
            "Got:\n{zod}"
        );
        assert_eq!(
            PlainObjectId::json_schema(),
            serde_json::json!({
                "type": "object",
                "properties": { "$oid": { "type": "string", "pattern": "^[a-f0-9]{24}$" } },
                "required": ["$oid"],
                "additionalProperties": false
            })
        );
    }

    /// A brand's string constraints measure the inner's `Display` — the bare hex — which on the
    /// wire is the `$oid` member, so both schema surfaces carry them there. Zod has no string
    /// check to apply to the object itself.
    ///
    /// The brand's own `pattern` is layered rather than written over the hex the type always
    /// holds: one JSON Schema string carries one `pattern`, and writing the brand's into that slot
    /// dropped the type's — which is how a brand wider than the hex came to admit, on this surface
    /// alone, strings an `ObjectId` can never hold.
    #[test]
    fn a_constrained_objectid_brand_carries_its_constraints_on_the_oid_member() {
        let zod = HexObjectId::zod_schema();
        assert!(
            zod.contains(&format!(
                "const HexObjectId$RawSchema = {OID_ZOD_BASE}.check(z.regex(/^[0-9a-fA-F]{{24}}$/)) }}).brand<"
            )),
            "Got:\n{zod}"
        );
        assert!(
            !zod.contains(") }).check("),
            "a string check must never sit on the $oid object. Got:\n{zod}"
        );
        assert!(
            zod.contains(r#"HexObjectId$Schema: $ZodBranded<ZodObject, "HexObjectId">"#),
            "Got:\n{zod}"
        );
        assert_eq!(
            HexObjectId::json_schema(),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "$oid": {
                        "type": "string",
                        "pattern": "^[a-f0-9]{24}$",
                        "allOf": [{ "pattern": "^[0-9a-fA-F]{24}$" }]
                    }
                },
                "required": ["$oid"],
                "additionalProperties": false
            })
        );
    }

    /// A transparent brand is nothing on the wire, so it describes exactly what its inner
    /// describes where that inner is named directly.
    #[test]
    fn an_unconstrained_objectid_brand_describes_what_the_field_position_describes() {
        assert_eq!(
            PlainObjectId::json_schema(),
            HoldsObjectId::json_schema()["properties"]["id"]
        );
    }

    /// Every `pattern` the `$oid` member states, applied: the member's own and the one each `allOf`
    /// branch adds. A payload has to match all of them, which is what a single `pattern` keyword
    /// cannot say and so what tells a layered brand pattern from one written over the type's.
    fn admits_oid(schema: &serde_json::Value, hex: &str) -> bool {
        let member = &schema["properties"]["$oid"];
        member["pattern"]
            .as_str()
            .into_iter()
            .chain(
                member["allOf"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|branch| branch["pattern"].as_str()),
            )
            .all(|pattern| regex::Regex::new(pattern).unwrap().is_match(hex))
    }

    /// The two surfaces read the same probe payloads the same way. A brand pattern wider than the
    /// hex is where they came apart: Zod ran the type's own regex before the brand's check, while
    /// the JSON schema had only the brand's left and admitted a `$oid` of 24 arbitrary characters.
    #[test]
    fn a_brand_pattern_wider_than_the_hex_still_narrows_to_the_hex_on_both_surfaces() {
        let zod = WideObjectId::zod_schema();
        assert!(zod.contains(OID_ZOD_BASE), "Got:\n{zod}");
        assert!(
            zod.contains(".check(z.regex(/^[a-z0-9]{24}$/))"),
            "Got:\n{zod}"
        );

        let schema = WideObjectId::json_schema();
        for (hex, admitted) in [
            ("507f1f77bcf86cd799439011", true),
            ("zzzzzzzzzzzzzzzzzzzzzzzz", false),
            ("507f1f77bcf86cd79943901", false),
        ] {
            assert_eq!(
                admits_oid(&schema, hex),
                admitted,
                "for {hex}, schema: {schema}"
            );
        }
    }

    /// An arrayed `ObjectId` writes the array around the `$oid` object, which is a container and
    /// so is described by the slot dispatch — the same rendering the unbranded tuple struct over
    /// the same `Vec` publishes. Every position reads one builder, so the item here is the object
    /// field position carries too.
    #[test]
    fn an_arrayed_objectid_brand_describes_the_array_of_oid_objects() {
        let zod = ObjectIdList::zod_schema();
        assert!(
            zod.contains(&format!(
                "const ObjectIdList$RawSchema = z.array({OID_ZOD_BASE} }})).brand<"
            )),
            "Got:\n{zod}"
        );
        assert!(
            zod.contains(r#"ObjectIdList$Schema: $ZodBranded<ZodArray, "ObjectIdList">"#),
            "Got:\n{zod}"
        );
        assert_eq!(
            ObjectIdList::json_schema(),
            serde_json::json!({
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": { "$oid": { "type": "string", "pattern": "^[a-f0-9]{24}$" } },
                    "required": ["$oid"],
                    "additionalProperties": false
                }
            })
        );
        assert_eq!(
            ObjectIdList::json_schema()["items"],
            PlainObjectId::json_schema()
        );
    }
}

// Branded newtype referenced from a struct (all features enabled)
#[cfg(all(feature = "zod", feature = "typescript", feature = "jsonschema"))]
mod branded_in_struct_all_features_tests {
    use super::*;

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct TaskId(pub String);

    #[model_schema(default_types(T = String))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct TaskTypeId<T>(pub T);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Task {
        pub id: TaskId,
        pub name: String,
        pub type_id: TaskTypeId<String>,
    }

    #[test]
    fn test_branded_in_struct_ts_definition() {
        let ts = Task::ts_definition();
        assert!(
            ts.contains("TaskId"),
            "Struct TS should reference TaskId. Got: {ts}"
        );
        assert!(
            ts.contains("TaskTypeId<string>"),
            "Struct TS should reference TaskTypeId<string>. Got: {ts}"
        );
    }

    #[test]
    fn test_branded_in_struct_zod_schema() {
        let zod = Task::zod_schema();
        assert!(
            zod.contains("TaskId$Schema"),
            "Struct Zod should reference TaskId$Schema. Got: {zod}"
        );
        assert!(
            zod.contains("TaskTypeId$Schema"),
            "Struct Zod should reference TaskTypeId$Schema. Got: {zod}"
        );
    }

    #[test]
    fn test_branded_in_struct_json_schema() {
        let schema = Task::json_schema();
        let properties = schema["properties"].as_object().unwrap();
        assert!(
            properties.contains_key("id"),
            "JSON schema should have 'id' property. Got: {schema}"
        );
        assert!(
            properties.contains_key("type_id"),
            "JSON schema should have 'type_id' property. Got: {schema}"
        );
        assert!(
            properties.contains_key("name"),
            "JSON schema should have 'name' property. Got: {schema}"
        );
    }

    #[test]
    fn test_branded_newtype_own_json_schema() {
        let schema = TaskId::json_schema();
        assert_eq!(
            schema["type"], "string",
            "Non-generic branded type should have type 'string'. Got: {schema}"
        );
    }

    /// A brand whose inner is a bare parameter names no type of its own, so its document is
    /// written at the type it declared for that parameter — as an uninstantiated parameter is
    /// wherever else it is written.
    #[test]
    fn test_generic_branded_newtype_own_json_schema() {
        assert_eq!(
            TaskTypeId::<String>::json_schema(),
            serde_json::json!({ "type": "string" })
        );
    }
}

// Branded newtype referenced from a struct (zod+typescript, no jsonschema)
#[cfg(all(feature = "zod", feature = "typescript", not(feature = "jsonschema")))]
mod branded_in_struct_no_jsonschema_tests {
    use super::*;

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct TaskIdNJ(pub String);

    #[model_schema(default_types(T = String))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct TaskTypeIdNJ<T>(pub T);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct TaskNJ {
        pub id: TaskIdNJ,
        pub name: String,
        pub type_id: TaskTypeIdNJ<String>,
    }

    #[test]
    fn test_branded_newtype_used_in_struct() {
        let ts = TaskNJ::ts_definition();
        assert!(
            ts.contains("TaskIdNJ"),
            "Struct TS should reference TaskIdNJ. Got: {ts}"
        );
        assert!(
            ts.contains("TaskTypeIdNJ<string>"),
            "Struct TS should reference TaskTypeIdNJ<string>. Got: {ts}"
        );

        let zod = TaskNJ::zod_schema();
        assert!(
            zod.contains("TaskIdNJ$Schema"),
            "Struct Zod should reference TaskIdNJ$Schema. Got: {zod}"
        );
        assert!(
            zod.contains("TaskTypeIdNJ$Schema"),
            "Struct Zod should reference TaskTypeIdNJ$Schema. Got: {zod}"
        );
    }
}

// Branded newtype referenced from a struct (jsonschema only, no zod/typescript)
#[cfg(all(
    feature = "jsonschema",
    not(feature = "zod"),
    not(feature = "typescript")
))]
mod branded_in_struct_jsonschema_only_tests {
    use super::*;

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct TaskIdJO(pub String);

    #[model_schema(default_types(T = String))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct TaskTypeIdJO<T>(pub T);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct TaskJO {
        pub id: TaskIdJO,
        pub name: String,
        pub type_id: TaskTypeIdJO<String>,
    }

    #[test]
    fn test_branded_in_struct_json_schema_only() {
        let schema = TaskJO::json_schema();
        let properties = schema["properties"].as_object().unwrap();
        assert!(
            properties.contains_key("id"),
            "JSON schema should have 'id' property. Got: {schema}"
        );
        assert!(
            properties.contains_key("type_id"),
            "JSON schema should have 'type_id' property. Got: {schema}"
        );
    }

    #[test]
    fn test_branded_own_json_schema_only() {
        let schema = TaskIdJO::json_schema();
        assert_eq!(schema["type"], "string", "Got: {schema}");
    }

    #[test]
    fn test_generic_branded_own_json_schema_only() {
        assert_eq!(
            TaskTypeIdJO::<String>::json_schema(),
            serde_json::json!({ "type": "string" })
        );
    }
}

// Branded newtype referenced from a struct (typescript only, no zod/jsonschema)
#[cfg(all(
    feature = "typescript",
    not(feature = "zod"),
    not(feature = "jsonschema")
))]
mod branded_in_struct_typescript_only_tests {
    use super::*;

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct TaskIdTO(pub String);

    #[model_schema(default_types(T = String))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct TaskTypeIdTO<T>(pub T);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct TaskTO {
        pub id: TaskIdTO,
        pub name: String,
        pub type_id: TaskTypeIdTO<String>,
    }

    #[test]
    fn test_branded_in_struct_ts_only() {
        let ts = TaskTO::ts_definition();
        assert!(
            ts.contains("TaskIdTO"),
            "Struct TS should reference TaskIdTO. Got: {ts}"
        );
        assert!(
            ts.contains("TaskTypeIdTO<string>"),
            "Struct TS should reference TaskTypeIdTO<string>. Got: {ts}"
        );
    }
}

// Branded newtype with constraints — json_schema should include them
#[cfg(feature = "jsonschema")]
mod branded_constrained_json_schema_tests {
    use super::*;

    #[model_schema(minLength = 24, maxLength = 24, pattern = "^[a-f\\d]{24}$")]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct ConstrainedId(pub String);

    #[model_schema(minLength = 3, maxLength = 50)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct ShortId(pub String);

    /// A generic brand's bare-parameter inner has no type of its own to describe, so its document
    /// is the declared default's — `{"type": "string"}` for `default_types(IdType = String)` —
    /// narrowed by the brand's own bounds through the same `allOf` a named inner is narrowed
    /// through. Not the inert `{}` a parameter with no default at all would describe as.
    #[model_schema(
        minLength = 24,
        maxLength = 24,
        pattern = "^[a-f\\d]{24}$",
        default_types(IdType = String)
    )]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct GenericConstrainedId<IdType>(pub IdType);

    #[test]
    fn test_constrained_generic_branded_json_schema_carries_the_defaults_document() {
        assert_eq!(
            GenericConstrainedId::<String>::json_schema(),
            serde_json::json!({
                "allOf": [
                    { "type": "string" },
                    {
                        "minLength": 24_u32,
                        "maxLength": 24_u32,
                        "pattern": "^[a-f0-9]{24}$"
                    }
                ]
            })
        );
    }

    #[test]
    fn test_constrained_branded_json_schema() {
        // The `\d` the brand is declared with reaches the schema as the members it stands for: a
        // JSON Schema `pattern` is an ECMA-262 regex, which reads `\d` as ASCII, while the Rust
        // validator beside it reads the Unicode class. Written out, both read the one set.
        assert_eq!(
            ConstrainedId::json_schema(),
            serde_json::json!({
                "type": "string",
                "minLength": 24_u32,
                "maxLength": 24_u32,
                "pattern": "^[a-f0-9]{24}$"
            })
        );
    }

    #[test]
    fn test_constrained_non_generic_json_schema() {
        let schema = ShortId::json_schema();
        assert_eq!(schema["type"], "string", "Got: {schema}");
        assert_eq!(schema["minLength"], 3_i64, "Got: {schema}");
        assert_eq!(schema["maxLength"], 50_i64, "Got: {schema}");
        // No pattern constraint — should not be present
        assert!(schema.get("pattern").is_none(), "Got: {schema}");
    }
}

/// A generic branded newtype whose inner is a bare type parameter, carrying string constraints.
/// The constraints have nowhere to land on the parameter itself, so they land on the parameter's
/// *declared default* instead: `StrictDocumentId$SchemaDefault` enforces the 24-hex bounds,
/// `StrictDocumentId$SchemaFactory` stays unconstrained for every other filling.
#[cfg(all(feature = "zod", feature = "typescript", feature = "serde"))]
mod constrained_generic_branded_tests {
    use super::*;

    #[model_schema(
        minLength = 24,
        maxLength = 24,
        pattern = "^[a-f\\d]{24}$",
        default_types(IdType = String)
    )]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct StrictDocumentId<IdType>(pub IdType);

    /// A declared default naming `StrictDocumentId` at exactly its own declared default — the
    /// `ProDoctivity` `EcmDocument`/`DocumentId` shape the fold feature was motivated by. Before
    /// the fix, the comparison read `StrictDocumentId$SchemaDefault`'s emitted text — the
    /// `.min`/`.max`/`.check` chain and all — against the plain rendering this struct's own field
    /// computes, so the two could never agree and the fold silently composed an unconstrained
    /// `StrictDocumentId$SchemaFactory(z.string())` in its place.
    #[model_schema(default_types(HolderType = StrictDocumentId<String>))]
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct StrictDocumentIdHolder<HolderType> {
        pub id: HolderType,
    }

    /// A SECOND constrained generic brand — itself carrying string constraints, unlike
    /// `StrictDocumentIdHolder` above, which carries none — whose declared default names
    /// `StrictDocumentId` at exactly its own declared default. The fold (txsch-qobf) resolves
    /// `default_zod_rendering` to the deferred `z.lazy(() => StrictDocumentId$SchemaDefault)`
    /// spelling, and `OuterId`'s own checks have to compose *inside* that thunk rather than after
    /// it — the folded-generic-sibling half of txsch-euxz's bug, the non-generic half being
    /// `constrained_default_names_a_sibling_tests::OuterBrand` below.
    #[model_schema(minLength = 10, default_types(WrapType = StrictDocumentId<String>))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct OuterId<WrapType>(pub WrapType);

    /// The builder every call to the factory runs through carries no string check at all: a caller
    /// filling `IdType` with something other than the declared default — an `ObjectId` schema, say
    /// — must not inherit bounds meant for the default.
    #[test]
    fn the_factorys_own_parameter_carries_no_check() {
        let zod = StrictDocumentId::<String>::zod_schema();
        let builder_end = zod.find("StrictDocumentId$SchemaFactoryCache").unwrap();
        let builder = &zod[..builder_end];
        for check in [".min(", ".max(", ".check("] {
            assert!(
                !builder.contains(check),
                "found {check} in builder:\n{builder}"
            );
        }
        assert!(builder.contains("idType.meta({"), "Got:\n{builder}");
    }

    /// `$SchemaDefault` is the factory called at the declared default argument, with the checks
    /// composed onto that argument — exactly the shape the design settled on.
    #[test]
    fn the_default_composes_the_factory_with_the_constrained_argument() {
        let zod = StrictDocumentId::<String>::zod_schema();
        assert!(
            zod.contains(
                "export const StrictDocumentId$SchemaDefault: ZodType<StrictDocumentId<string>> = \
                 StrictDocumentId$SchemaFactory(z.string().min(24).max(24).check(z.regex(/^[a-f0-9]{24}$/)));"
            ),
            "Got:\n{zod}"
        );
    }

    /// A value at the declared default validates against the 24-hex bounds through Rust's own
    /// `validate()`/serde path, which reads the same constraints the Zod surface enforces.
    #[test]
    fn the_default_instantiation_enforces_the_bounds_through_serde() {
        let valid: StrictDocumentId<String> =
            serde_json::from_str("\"64de3d95ff45b119e5b53a7e\"").unwrap();
        valid.validate().unwrap();

        let too_short: Result<StrictDocumentId<String>, _> = serde_json::from_str("\"abc\"");
        assert!(too_short.is_err(), "Should reject a too-short id via serde");
    }

    /// `$SchemaDefault` folds onto `StrictDocumentId$SchemaDefault` by reference, carrying the
    /// 24-hex bounds in rather than reconstructing an unconstrained instantiation the memo would
    /// not share with it.
    #[test]
    fn a_default_naming_the_constrained_brands_own_default_folds_onto_its_binding() {
        let zod = StrictDocumentIdHolder::<String>::zod_schema();
        assert!(
            zod.contains(
                "= StrictDocumentIdHolder$SchemaFactory(z.lazy(() => \
                 StrictDocumentId$SchemaDefault));"
            ),
            "Got:\n{zod}"
        );
    }

    /// `OuterId` is itself constrained, and its declared default folds onto `StrictDocumentId`'s
    /// own `$SchemaDefault` — the deferred, checks-carrying binding, not a plain
    /// `StrictDocumentId$SchemaFactory(z.string())` reconstruction. `OuterId`'s own `minLength`
    /// check composes *inside* the `z.lazy(...)` thunk, over `.check(...)`'s base spelling rather
    /// than `.min(...)`'s chain spelling — the deferred target's annotation is not guaranteed to
    /// carry `ZodString`'s own chain methods, only the `.check(...)` every schema exposes. Before
    /// the fix, this same check was appended after the thunk closed, landing on `ZodLazy`, which
    /// has neither.
    #[test]
    fn a_constrained_brands_default_naming_another_constrained_brands_default_composes_the_checks_inside_the_thunk()
     {
        let zod = OuterId::<String>::zod_schema();
        assert!(
            zod.contains(
                "export const OuterId$SchemaDefault: ZodType<OuterId<StrictDocumentId<string>>> = \
                 OuterId$SchemaFactory(z.lazy(() => \
                 StrictDocumentId$SchemaDefault.check(z.minLength(10))));"
            ),
            "Got:\n{zod}"
        );
    }

    /// The composed default enforces both bounds: `StrictDocumentId`'s own 24-hex, inherited
    /// through the fold, and `OuterId`'s own `minLength`.
    #[test]
    fn the_folded_default_instantiation_enforces_both_brands_bounds_through_serde() {
        let valid: OuterId<StrictDocumentId<String>> =
            serde_json::from_str("\"64de3d95ff45b119e5b53a7e\"").unwrap();
        valid.validate().unwrap();

        let too_short: Result<OuterId<StrictDocumentId<String>>, _> =
            serde_json::from_str("\"abc\"");
        assert!(
            too_short.is_err(),
            "Should reject a value StrictDocumentId's own 24-hex bound rejects"
        );
    }
}

/// The non-generic-sibling half of txsch-euxz's bug: a constrained generic brand whose declared
/// default names an ordinary (non-generic) `#[model_schema]` sibling. Held apart from
/// `constrained_generic_branded_tests` above (whose fixtures all name a *generic* sibling) so each
/// module pins one of the two spellings `default_zod_rendering` can defer to — a bare `$Schema`
/// here, a folded `$SchemaDefault` there.
#[cfg(all(feature = "zod", feature = "typescript", feature = "serde"))]
mod constrained_default_names_a_sibling_tests {
    use super::*;

    /// An unconstrained branded newtype — the sibling a generic brand's own declared default can
    /// name. Carries no checks of its own, so any composed checks downstream are unambiguously
    /// `OuterBrand`'s.
    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct InnerString(pub String);

    /// A constrained generic brand whose declared default names `InnerString` — a non-generic
    /// sibling, so `default_zod_rendering` defers to its bare `$Schema` binding (never a
    /// `$SchemaDefault`, which only a generic sibling publishes) rather than reconstructing it
    /// eagerly. This is the exact repro from txsch-euxz's own bug report.
    #[model_schema(minLength = 3, default_types(T = InnerString))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct OuterBrand<T>(pub T);

    /// Before the fix: `OuterBrand$SchemaFactory(z.lazy(() => InnerString$Schema).min(3))` —
    /// `.min` landing on `ZodLazy`, which does not have it. After: the check composes inside the
    /// thunk, over `InnerString$Schema`'s own base `.check(...)` surface.
    #[test]
    fn a_constrained_brands_default_naming_a_non_generic_sibling_composes_the_check_inside_the_thunk()
     {
        let zod = OuterBrand::<String>::zod_schema();
        assert!(
            zod.contains(
                "export const OuterBrand$SchemaDefault: ZodType<OuterBrand<InnerString>> = \
                 OuterBrand$SchemaFactory(z.lazy(() => InnerString$Schema.check(z.minLength(3))));"
            ),
            "Got:\n{zod}"
        );
    }

    /// The factory's own bare parameter still carries no check — exactly the invariant
    /// `constrained_generic_branded_tests::the_factorys_own_parameter_carries_no_check` pins for
    /// the primitive-default case, unaffected by where the default happens to point.
    #[test]
    fn the_factorys_own_parameter_carries_no_check() {
        let zod = OuterBrand::<String>::zod_schema();
        let builder_end = zod.find("OuterBrand$SchemaFactoryCache").unwrap();
        let builder = &zod[..builder_end];
        for check in [".min(", ".max(", ".check("] {
            assert!(
                !builder.contains(check),
                "found {check} in builder:\n{builder}"
            );
        }
    }

    #[test]
    fn the_default_instantiation_enforces_the_bound_through_serde() {
        let valid: OuterBrand<InnerString> = serde_json::from_str("\"abc\"").unwrap();
        valid.validate().unwrap();

        let too_short: Result<OuterBrand<InnerString>, _> = serde_json::from_str("\"ab\"");
        assert!(
            too_short.is_err(),
            "Should reject a too-short value via serde"
        );
    }
}

// Display impl is generated for branded newtypes when any schema feature is enabled
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
mod branded_display_tests {
    use super::*;

    #[model_schema(default_types(T = String))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct DisplayId<T>(pub T);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct SimpleDisplayId(pub String);

    #[test]
    fn test_generic_branded_display() {
        let id = DisplayId("abc123".to_owned());
        assert_eq!(format!("{id}"), "abc123");
    }

    #[test]
    fn test_non_generic_branded_display() {
        let id = SimpleDisplayId("xyz789".to_owned());
        assert_eq!(format!("{id}"), "xyz789");
    }

    #[test]
    fn test_branded_display_to_string() {
        let id = DisplayId("hello".to_owned());
        assert_eq!(id.to_string(), "hello");
    }
}

// A container inner type implements no Display, so `no_display` opts the brand out of the Display
// impl and of the assertion that guards it. Compiling this module is the assertion: emitting
// either one over a `Vec` inner is a hard error.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
mod branded_no_display_tests {
    use super::*;

    #[model_schema(no_display)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct Tags(pub Vec<String>);

    #[model_schema(no_display, default_types(T = String))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct TagList<T>(pub Vec<T>);

    #[test]
    fn test_no_display_brand_wraps_a_container() {
        let tags = Tags(vec!["a".to_owned(), "b".to_owned()]);
        assert_eq!(tags.0, vec!["a".to_owned(), "b".to_owned()]);

        let list = TagList(vec![1_u8, 2_u8]);
        assert_eq!(list.0, vec![1_u8, 2_u8]);
    }

    #[cfg(feature = "typescript")]
    #[test]
    fn test_no_display_brand_still_generates_typescript() {
        let ts = Tags::ts_definition();
        assert!(ts.contains("export type Tags"), "Got: {ts}");
    }

    #[cfg(feature = "zod")]
    #[test]
    fn test_no_display_brand_still_generates_zod() {
        let zod = Tags::zod_schema();
        assert!(zod.contains(&brand_marker("Tags")), "Got: {zod}");
    }
}

/// The four surfaces of a brand whose inner writes a container, pinned against what serde writes
/// for it.
///
/// `#[serde(transparent)]` puts the inner on the wire by itself, so an array inner writes an
/// array, a map inner an object, and a tuple inner the fixed-arity array — never a string. The
/// TypeScript type and the Zod value already described those; the Zod type annotation and the JSON
/// schema are pinned here beside them, so a consumer type-checking the exported binding and one
/// validating a payload read the same shape.
#[cfg(all(
    feature = "jsonschema",
    feature = "serde",
    feature = "typescript",
    feature = "zod"
))]
mod branded_composite_inner_tests {
    use super::*;
    use alloc::collections::BTreeSet;
    use std::collections::HashMap;

    #[model_schema(no_display)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct ByteQuad(pub [u8; 4]);

    #[model_schema(no_display)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct Grid(pub Vec<Vec<i32>>);

    #[model_schema(no_display)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct LabelList(pub Vec<String>);

    #[model_schema(no_display)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct LabelPair(pub (String, u32));

    #[model_schema(no_display)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct LabelSet(pub BTreeSet<String>);

    #[model_schema(no_display)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct Payload(pub serde_json::Value);

    #[model_schema(no_display)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct SparseLabels(pub Vec<Option<String>>);

    #[model_schema(no_display)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct WeightMap(pub HashMap<String, u32>);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Label {
        pub key: String,
    }

    #[model_schema(no_display)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct LabelRow(pub Vec<Label>);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct HoldsComposites {
        pub labels: Vec<String>,
        pub pair: (String, u32),
        pub weights: HashMap<String, u32>,
    }

    #[test]
    fn a_container_brand_writes_the_container_its_inner_writes() {
        for (rendered, expected) in [
            (
                serde_json::to_string(&LabelList(vec!["a".to_owned()])).unwrap(),
                r#"["a"]"#,
            ),
            (
                serde_json::to_string(&WeightMap(HashMap::from([("a".to_owned(), 1_u32)])))
                    .unwrap(),
                r#"{"a":1}"#,
            ),
            (
                serde_json::to_string(&LabelPair(("a".to_owned(), 1_u32))).unwrap(),
                r#"["a",1]"#,
            ),
            (
                serde_json::to_string(&LabelSet(BTreeSet::from(["a".to_owned()]))).unwrap(),
                r#"["a"]"#,
            ),
            (
                serde_json::to_string(&ByteQuad([1, 2, 3, 4])).unwrap(),
                "[1,2,3,4]",
            ),
            (
                serde_json::to_string(&Payload(serde_json::json!({ "x": 1_u32 }))).unwrap(),
                r#"{"x":1}"#,
            ),
            (
                serde_json::to_string(&SparseLabels(vec![Some("a".to_owned()), None])).unwrap(),
                r#"["a",null]"#,
            ),
            (
                serde_json::to_string(&Grid(vec![vec![1_i32, 2_i32]])).unwrap(),
                "[[1,2]]",
            ),
        ] {
            assert_eq!(rendered, expected);
        }
    }

    #[test]
    fn a_container_brand_reads_back_what_it_wrote() {
        let labels = LabelList(vec!["a".to_owned()]);
        assert_eq!(
            serde_json::from_str::<LabelList>(&serde_json::to_string(&labels).unwrap()).unwrap(),
            labels
        );
        let weights = WeightMap(HashMap::from([("a".to_owned(), 1_u32)]));
        assert_eq!(
            serde_json::from_str::<WeightMap>(&serde_json::to_string(&weights).unwrap()).unwrap(),
            weights
        );
        let pair = LabelPair(("a".to_owned(), 1_u32));
        assert_eq!(
            serde_json::from_str::<LabelPair>(&serde_json::to_string(&pair).unwrap()).unwrap(),
            pair
        );
    }

    #[test]
    fn an_array_brand_describes_the_array_on_every_surface() {
        assert_eq!(
            LabelList::ts_definition(),
            r#"export type LabelList = Array<string> & $brand<"LabelList">;"#
        );
        let zod = LabelList::zod_schema();
        assert!(
            zod.contains(r#"const LabelList$RawSchema = z.array(z.string()).brand<"LabelList">()"#),
            "Got:\n{zod}"
        );
        assert!(
            zod.contains(r#"LabelList$Schema: $ZodBranded<ZodArray, "LabelList">"#),
            "Got:\n{zod}"
        );
        assert_eq!(
            LabelList::json_schema(),
            serde_json::json!({ "type": "array", "items": { "type": "string" } })
        );
    }

    #[test]
    fn a_map_brand_describes_the_object_on_every_surface() {
        assert_eq!(
            WeightMap::ts_definition(),
            r#"export type WeightMap = Partial<Record<string, number>> & $brand<"WeightMap">;"#
        );
        let zod = WeightMap::zod_schema();
        assert!(
            zod.contains(
                r#"const WeightMap$RawSchema = z.record(z.string(), z.number().int()).brand<"WeightMap">()"#
            ),
            "Got:\n{zod}"
        );
        assert!(
            zod.contains(r#"WeightMap$Schema: $ZodBranded<ZodRecord, "WeightMap">"#),
            "Got:\n{zod}"
        );
        assert_eq!(
            WeightMap::json_schema(),
            serde_json::json!({ "type": "object", "additionalProperties": { "type": "integer" } })
        );
    }

    #[test]
    fn a_tuple_brand_describes_the_fixed_arity_array_on_every_surface() {
        assert_eq!(
            LabelPair::ts_definition(),
            r#"export type LabelPair = [string, number] & $brand<"LabelPair">;"#
        );
        let zod = LabelPair::zod_schema();
        assert!(
            zod.contains(
                r#"const LabelPair$RawSchema = z.tuple([z.string(), z.number().int()]).brand<"LabelPair">()"#
            ),
            "Got:\n{zod}"
        );
        assert!(
            zod.contains(r#"LabelPair$Schema: $ZodBranded<ZodTuple, "LabelPair">"#),
            "Got:\n{zod}"
        );
        assert_eq!(
            LabelPair::json_schema(),
            serde_json::json!({
                "type": "array",
                "prefixItems": [{ "type": "string" }, { "type": "integer" }],
                "items": false,
                "minItems": 2_u32,
                "maxItems": 2_u32
            })
        );
    }

    /// A sequence wrapper describes as the `Vec` of the same element does, and a fixed-size
    /// `[T; N]` carries the arity serde reads back — both through the brand, as they do everywhere
    /// else.
    #[test]
    fn a_set_and_a_fixed_array_brand_describe_their_arrays() {
        assert_eq!(
            LabelSet::json_schema(),
            serde_json::json!({ "type": "array", "items": { "type": "string" } })
        );
        assert!(
            LabelSet::zod_schema().contains(r#"$ZodBranded<ZodArray, "LabelSet">"#),
            "Got:\n{}",
            LabelSet::zod_schema()
        );
        assert_eq!(
            ByteQuad::json_schema(),
            serde_json::json!({
                "type": "array",
                "items": { "type": "integer" },
                "minItems": 4_u32,
                "maxItems": 4_u32
            })
        );
        assert!(
            ByteQuad::zod_schema().contains(r#"$ZodBranded<ZodArray, "ByteQuad">"#),
            "Got:\n{}",
            ByteQuad::zod_schema()
        );
    }

    /// An opaque inner carries no type name to narrow with, so the brand admits any value — the
    /// permissive empty schema, matching the `unknown` type and the `z.unknown()` value.
    #[test]
    fn an_opaque_brand_admits_any_value() {
        assert_eq!(
            Payload::ts_definition(),
            r#"export type Payload = unknown & $brand<"Payload">;"#
        );
        let zod = Payload::zod_schema();
        assert!(
            zod.contains(r#"const Payload$RawSchema = z.unknown().brand<"Payload">()"#),
            "Got:\n{zod}"
        );
        assert!(
            zod.contains(r#"Payload$Schema: $ZodBranded<ZodUnknown, "Payload">"#),
            "Got:\n{zod}"
        );
        assert_eq!(Payload::json_schema(), serde_json::json!({}));
    }

    /// A `None` inside the array is an item rather than an omission, and the levels nest — the
    /// brand carries both, because the array levels are the inner's own.
    #[test]
    fn a_nested_and_a_nullable_element_brand_keep_their_levels() {
        assert_eq!(
            Grid::json_schema(),
            serde_json::json!({
                "type": "array",
                "items": { "type": "array", "items": { "type": "integer" } }
            })
        );
        assert_eq!(
            SparseLabels::json_schema(),
            serde_json::json!({
                "type": "array",
                "items": { "anyOf": [{ "type": "string" }, { "type": "null" }] }
            })
        );
    }

    /// An element that names another type is carried by that type's own schema, as it is in every
    /// other position — so the brand defers rather than describing an open object.
    #[test]
    fn an_arrayed_sibling_brand_carries_the_siblings_own_schema() {
        assert_eq!(
            LabelRow::json_schema(),
            serde_json::json!({
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": { "key": { "type": "string" } },
                    "required": ["key"]
                }
            })
        );
        assert!(
            LabelRow::zod_schema().contains(r#"$ZodBranded<ZodArray, "LabelRow">"#),
            "Got:\n{}",
            LabelRow::zod_schema()
        );
    }

    /// A transparent brand is nothing on the wire, so it describes exactly what its inner
    /// describes where that inner is named directly.
    #[test]
    fn a_container_brand_describes_what_the_field_position_describes() {
        let holder = HoldsComposites::json_schema();
        assert_eq!(LabelList::json_schema(), holder["properties"]["labels"]);
        assert_eq!(LabelPair::json_schema(), holder["properties"]["pair"]);
        assert_eq!(WeightMap::json_schema(), holder["properties"]["weights"]);
    }
}

/// The four surfaces of a brand whose inner is written out of the brand's own type parameters,
/// pinned against what serde writes for it.
///
/// A parameter changes nothing about what `#[serde(transparent)]` puts on the wire: a `Vec` inner
/// writes its array whether the elements are `String` or a parameter. So the shape around the
/// parameter is described here exactly as the same shape is described where it is written without
/// a brand — the generic alias beside each brand is that shape, and every surface is asserted to
/// carry the same rendering it does.
///
/// The parameter itself carries what an uninstantiated parameter carries in every other position:
/// its own name in TypeScript, where the declaration binds it for real; the argument the enclosing
/// factory binds in Zod, where a schema is a value the caller has to fill before anything can
/// validate; and the permissive empty schema in JSON, where the one document written covers every
/// filling and so can describe none of them.
#[cfg(all(
    feature = "jsonschema",
    feature = "serde",
    feature = "typescript",
    feature = "zod"
))]
mod branded_generic_inner_tests {
    use super::branded_no_display_tests::TagList;
    use super::*;
    use std::collections::HashMap;

    #[model_schema(default_types(T = String))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct BareTag<T>(pub T);

    #[model_schema(no_display, default_types(T = u32))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct WeightIndex<T>(pub HashMap<String, T>);

    #[model_schema(default_types(T = String))]
    pub type BareSeq<T> = T;

    #[model_schema(default_types(T = String))]
    pub type TagSeq<T> = Vec<T>;

    #[model_schema(default_types(T = u32))]
    pub type WeightSeq<T> = HashMap<String, T>;

    #[test]
    fn a_generic_container_brand_writes_the_container_its_inner_writes() {
        let tags = TagList(vec!["a".to_owned()]);
        assert_eq!(serde_json::to_string(&tags).unwrap(), r#"["a"]"#);
        assert_eq!(
            serde_json::from_str::<TagList<String>>(&serde_json::to_string(&tags).unwrap())
                .unwrap(),
            tags
        );

        let weights = WeightIndex(HashMap::from([("a".to_owned(), 1_u32)]));
        assert_eq!(serde_json::to_string(&weights).unwrap(), r#"{"a":1}"#);
        assert_eq!(
            serde_json::from_str::<WeightIndex<u32>>(&serde_json::to_string(&weights).unwrap())
                .unwrap(),
            weights
        );

        let bare = BareTag("a".to_owned());
        assert_eq!(serde_json::to_string(&bare).unwrap(), r#""a""#);
        assert_eq!(
            serde_json::from_str::<BareTag<String>>(&serde_json::to_string(&bare).unwrap())
                .unwrap(),
            bare
        );

        // The alias beside each brand is the same shape with nothing branding it, and the wire is
        // the same — which is what makes the surfaces asserted equal below comparable at all.
        let tag_seq: TagSeq<String> = vec!["a".to_owned()];
        assert_eq!(serde_json::to_string(&tag_seq).unwrap(), r#"["a"]"#);
        let weight_seq: WeightSeq<u32> = HashMap::from([("a".to_owned(), 1_u32)]);
        assert_eq!(serde_json::to_string(&weight_seq).unwrap(), r#"{"a":1}"#);
        let bare_seq: BareSeq<String> = "a".to_owned();
        assert_eq!(serde_json::to_string(&bare_seq).unwrap(), r#""a""#);
    }

    #[test]
    fn an_arrayed_parameter_brand_describes_the_array_on_every_surface() {
        assert_eq!(
            TagList::<String>::ts_definition(),
            r#"export type TagList<T> = Array<T> & $brand<"TagList">;"#
        );
        let zod = TagList::<String>::zod_schema();
        assert!(zod.contains("  z.array(t).meta({"), "Got:\n{zod}");
        assert!(zod.contains(r#"}).brand<"TagList">();"#), "Got:\n{zod}");
        assert!(
            zod.contains("export const TagList$SchemaFactory = <T extends ZodType>("),
            "Got:\n{zod}"
        );
        assert_eq!(
            TagList::<String>::json_schema(),
            serde_json::json!({ "type": "array", "items": { "type": "string" } })
        );
    }

    #[test]
    fn a_mapped_parameter_brand_describes_the_object_on_every_surface() {
        assert_eq!(
            WeightIndex::<u32>::ts_definition(),
            r#"export type WeightIndex<T> = Partial<Record<string, T>> & $brand<"WeightIndex">;"#
        );
        let zod = WeightIndex::<u32>::zod_schema();
        assert!(
            zod.contains("  z.record(z.string(), t).meta({"),
            "Got:\n{zod}"
        );
        assert!(zod.contains(r#"}).brand<"WeightIndex">();"#), "Got:\n{zod}");
        assert!(
            zod.contains("export const WeightIndex$SchemaFactory = <T extends ZodType>("),
            "Got:\n{zod}"
        );
        assert_eq!(
            WeightIndex::<u32>::json_schema(),
            serde_json::json!({
                "type": "object",
                "additionalProperties": { "type": "integer" }
            })
        );
    }

    /// A parameter on its own carries no shape of its own, so each surface writes for it what it
    /// writes for a bare generic field — which is the alias asserted beside it. On the JSON surface
    /// that is the type the brand declared for the parameter, a document being written at one
    /// filling and the declaration naming it.
    #[test]
    fn a_bare_parameter_brand_matches_the_generic_field_convention() {
        assert_eq!(
            BareTag::<String>::ts_definition(),
            r#"export type BareTag<T> = T & $brand<"BareTag">;"#
        );
        let zod = BareTag::<String>::zod_schema();
        assert!(zod.contains("  t.meta({"), "Got:\n{zod}");
        assert!(zod.contains(r#"}).brand<"BareTag">();"#), "Got:\n{zod}");
        assert!(
            zod.contains("export const BareTag$SchemaFactory = <T extends ZodType>("),
            "Got:\n{zod}"
        );
        assert_eq!(
            BareTag::<String>::json_schema(),
            serde_json::json!({ "type": "string" })
        );
    }

    /// The rendering each surface gives the brand is the one it gives the same shape written
    /// without a brand: the JSON schemas are equal outright, and the TypeScript type and the Zod
    /// value each carry the fragment the alias carries, under the brand's own wrapping.
    #[test]
    fn a_generic_brand_renders_what_the_same_generic_alias_renders() {
        assert_eq!(
            TagList::<String>::json_schema(),
            tag_seq_schema::Schema::json_schema()
        );
        assert_eq!(
            WeightIndex::<u32>::json_schema(),
            weight_seq_schema::Schema::json_schema()
        );
        assert_eq!(
            BareTag::<String>::json_schema(),
            bare_seq_schema::Schema::json_schema()
        );

        for (brand, alias, shared) in [
            (
                TagList::<String>::ts_definition(),
                tag_seq_schema::Schema::ts_definition(),
                "Array<T>",
            ),
            (
                TagList::<String>::zod_schema(),
                tag_seq_schema::Schema::zod_schema(),
                "z.array(t)",
            ),
            (
                WeightIndex::<u32>::ts_definition(),
                weight_seq_schema::Schema::ts_definition(),
                "Partial<Record<string, T>>",
            ),
            (
                WeightIndex::<u32>::zod_schema(),
                weight_seq_schema::Schema::zod_schema(),
                "z.record(z.string(), t)",
            ),
            (
                BareTag::<String>::ts_definition(),
                bare_seq_schema::Schema::ts_definition(),
                "<T> = T",
            ),
            (
                BareTag::<String>::zod_schema(),
                bare_seq_schema::Schema::zod_schema(),
                "\n  t",
            ),
        ] {
            assert!(brand.contains(shared), "brand got:\n{brand}");
            assert!(alias.contains(shared), "alias got:\n{alias}");
        }
    }

    /// Zod publishes values, and a value-level surface has no generic a `const` can be declared
    /// over: a parameter rendered as the `Name$Schema` binding every unresolved type is named
    /// after would reference a binding no emitted module ever declares, and the consumer that
    /// pastes the output gets a `ReferenceError` before any payload is read.
    ///
    /// Asserted over the brand and the alias together, because both compose their value from the
    /// same rendering and so would drift apart one landing at a time.
    #[test]
    fn no_emitted_zod_value_references_a_binding_named_after_a_parameter() {
        for zod in [
            TagList::<String>::zod_schema(),
            WeightIndex::<u32>::zod_schema(),
            BareTag::<String>::zod_schema(),
            tag_seq_schema::Schema::zod_schema(),
            weight_seq_schema::Schema::zod_schema(),
            bare_seq_schema::Schema::zod_schema(),
        ] {
            assert!(!zod.contains("T$Schema"), "Got:\n{zod}");
        }
    }

    /// A generic alias publishes a factory exactly as the brand beside it does, so neither has an
    /// annotation left to erase an argument out of — the `const` each used to publish claimed
    /// `ZodType<Name<unknown>>` while the declaration beside it kept the argument, which is a type
    /// error at any field naming either.
    #[test]
    fn a_generic_alias_publishes_the_factory_the_brand_beside_it_publishes() {
        for (zod, factory) in [
            (
                tag_seq_schema::Schema::zod_schema(),
                "export const TagSeqType$SchemaFactory = <T extends ZodType>(",
            ),
            (
                weight_seq_schema::Schema::zod_schema(),
                "export const WeightSeqType$SchemaFactory = <T extends ZodType>(",
            ),
            (
                bare_seq_schema::Schema::zod_schema(),
                "export const BareSeqType$SchemaFactory = <T extends ZodType>(",
            ),
        ] {
            assert!(zod.contains(factory), "Got:\n{zod}");
            assert!(!zod.contains("<unknown>"), "Got:\n{zod}");
        }
    }
}

/// A brand's doc example is Rust the expansion has to compile, so it is instantiated at as many
/// concrete types as the brand declares parameters — one per parameter, not one overall.
#[cfg(feature = "zod")]
mod branded_example_arity_tests {
    use super::*;

    /// One parameter per side of the pair.
    ///
    /// ```rust example
    /// PairId(("a".to_string(), "b".to_string()))
    /// ```
    #[model_schema(no_display, default_types(A = String, B = String))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct PairId<A, B>(pub (A, B));

    /// The one parameter the example is written against.
    ///
    /// ```rust example
    /// SoloId("a".to_string())
    /// ```
    #[model_schema(default_types(T = String))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct SoloId<T>(pub T);

    #[test]
    fn a_two_parameter_brand_renders_the_example_its_inner_writes() {
        assert_eq!(
            PairId::<String, String>::schema_example(),
            serde_json::json!(["a", "b"])
        );
    }

    /// A single parameter is instantiated exactly as it was before arity was counted.
    #[test]
    fn a_one_parameter_brand_renders_the_example_it_always_did() {
        assert_eq!(SoloId::<String>::schema_example(), serde_json::json!("a"));
    }
}

/// What a build emitting no TypeScript writes for a brand.
///
/// The name a brand carries is a *type* argument to `.brand`, and the binding's annotation is a
/// type — neither of which a JavaScript parser reads, and a module carrying either stops at load
/// rather than at a payload. Zod's runtime brand takes no argument at all, so the bare call is the
/// same value under a spelling JavaScript does read.
#[cfg(all(feature = "zod", not(feature = "typescript")))]
mod branded_javascript_flavour_tests {
    use super::*;

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct PlainMark(pub String);

    #[model_schema(default_types(T = String))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct HeldMark<T>(pub T);

    #[test]
    fn a_brand_writes_the_marker_with_no_type_argument() {
        for zod in [PlainMark::zod_schema(), HeldMark::<String>::zod_schema()] {
            assert!(zod.contains(".brand()"), "Got:\n{zod}");
            assert!(!zod.contains(".brand<"), "Got:\n{zod}");
        }
    }

    #[test]
    fn a_brand_binding_carries_no_annotation() {
        for zod in [PlainMark::zod_schema(), HeldMark::<String>::zod_schema()] {
            assert!(!zod.contains("ZodType"), "Got:\n{zod}");
            assert!(!zod.contains("$ZodBranded"), "Got:\n{zod}");
            assert!(!zod.contains("$Schema:"), "Got:\n{zod}");
        }
    }
}

/// The four surfaces of a brand whose inner names another type, pinned against what serde writes
/// for it.
///
/// `#[serde(transparent)]` puts the named type on the wire by itself, so an object inner writes
/// that object — never a string. The TypeScript type and the Zod value already deferred to the
/// name; the JSON schema and the Zod type annotation are pinned here beside them, so a consumer
/// type-checking the exported binding and one validating a payload read the same shape.
#[cfg(all(
    feature = "jsonschema",
    feature = "serde",
    feature = "typescript",
    feature = "zod"
))]
mod branded_sibling_inner_tests {
    use super::*;

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Part {
        pub a: String,
        pub b: u32,
    }

    #[model_schema(no_display)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct WrappedPart(pub Part);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct HoldsPart {
        pub part: Part,
    }

    // Written before the type it names, which is what a reference has to survive.
    #[model_schema(no_display)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct WrappedTail(pub Tail);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Tail {
        pub z: String,
    }

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Chain {
        pub name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub next: Option<Box<ChainRef>>,
    }

    #[model_schema(no_display)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct ChainRef(pub Chain);

    // A string-shaped sibling: the one a constrained brand can be written over, the constrained
    // path asserting `Display` on the inner.
    #[model_schema(maxLength = 10)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct Slug(pub String);

    #[model_schema(minLength = 3)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct ShortSlug(pub Slug);

    // A constrained brand written *before* the string-shaped sibling it names, so nothing is
    // recorded for that name when the brand asks what it publishes.
    #[model_schema(minLength = 3)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct EarlySlug(pub LateSlug);

    #[model_schema(maxLength = 10)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct LateSlug(pub String);

    // The same forward declaration with the inner's argument fixed where it is written, which is
    // what the refusal for a parameterised inner does not reach. Written above `LateTag`, with
    // `TrailingFixedTag` below it, so the one declaration stands here in both the orders it can be
    // written in — the registry silent for the first and answering for the second.
    #[model_schema(minLength = 3)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct FixedTag(pub LateTag<String>);

    // The same inner reached through a parameter instead, unconstrained — which is what a generic
    // brand publishes a factory for.
    #[model_schema(default_types(TagType = String))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct OpenTag<TagType>(pub LateTag<TagType>);

    #[model_schema(default_types(TagType = String))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct LateTag<TagType>(pub TagType);

    // `FixedTag`'s mirror: the same two declarations, this one written where the registry has
    // already classified `LateTag`.
    #[model_schema(minLength = 3)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct TrailingFixedTag(pub LateTag<String>);

    #[test]
    fn a_sibling_brand_writes_the_object_its_inner_writes() {
        let part = Part {
            a: "a".to_owned(),
            b: 1,
        };
        assert_eq!(
            serde_json::to_string(&WrappedPart(part.clone())).unwrap(),
            r#"{"a":"a","b":1}"#
        );
        assert_eq!(
            serde_json::to_string(&part).unwrap(),
            serde_json::to_string(&WrappedPart(part)).unwrap()
        );
    }

    #[test]
    fn a_sibling_brand_describes_the_named_type_on_every_surface() {
        assert_eq!(
            WrappedPart::ts_definition(),
            r#"export type WrappedPart = Part & $brand<"WrappedPart">;"#
        );
        let zod = WrappedPart::zod_schema();
        assert!(
            zod.contains(r#"const WrappedPart$RawSchema = Part$Schema.brand<"WrappedPart">()"#),
            "Got:\n{zod}"
        );
        assert!(
            zod.contains(r#"WrappedPart$Schema: $ZodBranded<typeof Part$Schema, "WrappedPart">"#),
            "Got:\n{zod}"
        );
        assert_eq!(WrappedPart::json_schema(), Part::json_schema());
    }

    /// A transparent brand is nothing on the wire, so it describes exactly what its inner
    /// describes where that inner is named directly.
    #[test]
    fn a_sibling_brand_describes_what_the_field_position_describes() {
        assert_eq!(
            WrappedPart::json_schema(),
            HoldsPart::json_schema()["properties"]["part"]
        );
    }

    /// A reference resolves in either declaration order, so a brand standing before the type it
    /// names carries that type's own schema all the same.
    #[test]
    fn a_forward_declared_sibling_brand_carries_the_named_types_schema() {
        assert_eq!(WrappedTail::json_schema(), Tail::json_schema());
        assert!(
            WrappedTail::zod_schema()
                .contains(r#"WrappedTail$Schema: $ZodBranded<typeof Tail$Schema, "WrappedTail">"#),
            "Got:\n{}",
            WrappedTail::zod_schema()
        );
    }

    /// A cycle closed through a brand defers exactly as one closed through a field does: the name
    /// re-entered while still being written becomes a reference, and its body is hoisted to the
    /// root that reference resolves against.
    #[test]
    fn a_recursive_sibling_brand_defers_rather_than_inlining() {
        assert_eq!(
            ChainRef::json_schema(),
            serde_json::json!({
                "$defs": {
                    "ChainRef": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "name": { "type": "string" },
                            "next": { "$ref": "#/$defs/ChainRef" }
                        },
                        "required": ["name"]
                    }
                },
                "$ref": "#/$defs/ChainRef"
            })
        );
    }

    /// A brand over a string-shaped sibling keeps its own constraints, layered around the named
    /// type's schema rather than written in place of it — which is how the Zod value already
    /// composes them, the named schema first and the brand's checks after it.
    #[test]
    fn a_constrained_sibling_brand_layers_its_constraints_over_the_named_schema() {
        assert_eq!(
            ShortSlug::json_schema(),
            serde_json::json!({
                "allOf": [{ "type": "string", "maxLength": 10_u32 }, { "minLength": 3_u32 }]
            })
        );
        let zod = ShortSlug::zod_schema();
        assert!(
            zod.contains(r#"const ShortSlug$RawSchema = Slug$Schema.min(3).brand<"ShortSlug">()"#),
            "Got:\n{zod}"
        );
        assert!(
            zod.contains(r#"ShortSlug$Schema: $ZodBranded<typeof Slug$Schema, "ShortSlug">"#),
            "Got:\n{zod}"
        );
    }

    /// A brand reaching a name before the named item has expanded is asking a registry that has
    /// nothing recorded for it, and the constrained brand keeps the emission it has always had
    /// rather than being refused for where its inner happens to be written.
    ///
    /// That absence is the same one an unresolved user type leaves — a type this crate never
    /// expands, whose schema the author supplies — so refusing on it would refuse the second for
    /// the sake of the first. The consult is kept for the registration that follows instead, and
    /// here that registration proves a string and settles it silently: what the author reads is the
    /// same verdict wherever the two items are written. The `Display` assertion still bounds the
    /// Rust surface either way.
    #[test]
    fn a_constrained_brand_over_a_forward_declared_sibling_keeps_its_emission() {
        assert_eq!(
            EarlySlug::json_schema(),
            serde_json::json!({
                "allOf": [{ "type": "string", "maxLength": 10_u32 }, { "minLength": 3_u32 }]
            })
        );
        let zod = EarlySlug::zod_schema();
        assert!(
            zod.contains(
                r#"const EarlySlug$RawSchema = LateSlug$Schema.min(3).brand<"EarlySlug">()"#
            ),
            "Got:\n{zod}"
        );
    }

    /// A fixed argument is not a parameter, so the refusal beside it leaves this declaration where
    /// it was: the checks are appended to the factory call, and every surface holds the string the
    /// declaration fixed. This is the admission the guard makes for a name the registry has no
    /// answer for, reached by a name that carries an argument.
    #[test]
    fn a_constrained_brand_over_a_fixed_instantiation_carries_its_checks() {
        assert_eq!(
            FixedTag::json_schema(),
            serde_json::json!({
                "allOf": [{ "type": "string" }, { "minLength": 3_u32 }]
            })
        );
        let zod = FixedTag::zod_schema();
        assert!(
            zod.contains(
                r#"const FixedTag$RawSchema = LateTag$SchemaFactory(z.string()).min(3).brand<"FixedTag">()"#
            ),
            "Got:\n{zod}"
        );
        FixedTag(LateTag("abcd".to_owned())).validate().unwrap();
        assert_eq!(
            FixedTag(LateTag("ab".to_owned())).validate().unwrap_err(),
            vec!["value is too short: minimum length is 3, got 2".to_owned()]
        );
    }

    /// The same declaration written after the item it names carries the same checks to the same
    /// three surfaces, byte for byte.
    ///
    /// What `LateTag` records is a position rather than a word, so the registry answers with the
    /// argument this brand wrote — a string, which is what the emission composes the checks onto.
    /// One word could not have said that: it would have to stand for every filling, and the only
    /// word that does is the opaque one, which refuses the very declaration the other order
    /// admits. What the author reads is the same verdict wherever the two items are written.
    #[test]
    fn a_constrained_brand_over_a_fixed_instantiation_reads_the_same_in_either_order() {
        assert_eq!(TrailingFixedTag::json_schema(), FixedTag::json_schema());
        let zod = TrailingFixedTag::zod_schema();
        assert!(
            zod.contains(
                r#"const TrailingFixedTag$RawSchema = LateTag$SchemaFactory(z.string()).min(3).brand<"TrailingFixedTag">()"#
            ),
            "Got:\n{zod}"
        );
        assert_eq!(
            TrailingFixedTag::ts_definition(),
            r#"export type TrailingFixedTag = LateTag<string> & $brand<"TrailingFixedTag">;"#
        );
        TrailingFixedTag(LateTag("abcd".to_owned()))
            .validate()
            .unwrap();
        assert_eq!(
            TrailingFixedTag(LateTag("ab".to_owned()))
                .validate()
                .unwrap_err(),
            vec!["value is too short: minimum length is 3, got 2".to_owned()]
        );
    }

    /// Nothing the refusal added reaches a brand carrying no checks: it still publishes the
    /// factory, still composes the inner's own factory inside it, and still binds the parameter it
    /// was declared with.
    #[test]
    fn an_unconstrained_generic_brand_over_a_parameterised_inner_is_unchanged() {
        assert_eq!(
            OpenTag::<String>::ts_definition(),
            r#"export type OpenTag<TagType> = LateTag<TagType> & $brand<"OpenTag">;"#
        );
        let zod = OpenTag::<String>::zod_schema();
        assert!(
            zod.contains("const buildOpenTag$Schema = <TagType extends ZodType>("),
            "Got:\n{zod}"
        );
        assert!(
            zod.contains("  LateTag$SchemaFactory(tagType).meta({"),
            "Got:\n{zod}"
        );
        assert!(
            zod.contains("export const OpenTag$SchemaFactory = <TagType extends ZodType>("),
            "Got:\n{zod}"
        );
    }
}

/// The surfaces of a brand whose inner is a chrono type, pinned against what serde writes for it.
///
/// `#[serde(transparent)]` puts the chrono value on the wire by itself, so the brand writes the
/// same string a field of that type writes — and carries the same `"format"` keyword saying which
/// instant the string spells. Reaching the string through the TypeScript name every chrono type
/// shares with `String` is what dropped the keyword.
#[cfg(all(feature = "chrono", feature = "jsonschema", feature = "serde"))]
mod branded_chrono_inner_tests {
    use super::*;
    use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct Stamp(pub NaiveDate);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct Clock(pub NaiveTime);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct Moment(pub NaiveDateTime);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct Instant(pub DateTime<Utc>);

    #[model_schema(minLength = 10, maxLength = 10, pattern = r"^\d{4}-\d{2}-\d{2}$")]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct IsoStamp(pub NaiveDate);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct HoldsChrono {
        pub clock: NaiveTime,
        pub instant: DateTime<Utc>,
        pub moment: NaiveDateTime,
        pub stamp: NaiveDate,
    }

    #[test]
    fn a_chrono_brand_writes_the_string_its_inner_writes() {
        let date = NaiveDate::from_ymd_opt(2020, 1, 2).unwrap();
        assert_eq!(
            serde_json::to_string(&Stamp(date)).unwrap(),
            r#""2020-01-02""#
        );
        assert_eq!(
            serde_json::to_string(&date).unwrap(),
            serde_json::to_string(&Stamp(date)).unwrap()
        );
    }

    #[test]
    fn a_chrono_brand_carries_the_format_keyword() {
        assert_eq!(
            Stamp::json_schema(),
            serde_json::json!({ "type": "string", "format": "date" })
        );
        assert_eq!(
            Clock::json_schema(),
            serde_json::json!({ "type": "string", "format": "time" })
        );
        assert_eq!(
            Moment::json_schema(),
            serde_json::json!({ "type": "string", "format": "date-time" })
        );
        assert_eq!(
            Instant::json_schema(),
            serde_json::json!({ "type": "string", "format": "date-time" })
        );
    }

    /// A transparent brand is nothing on the wire, so it describes exactly what its inner
    /// describes where that inner is named directly.
    #[test]
    fn a_chrono_brand_describes_what_the_field_position_describes() {
        let holder = HoldsChrono::json_schema();
        assert_eq!(Stamp::json_schema(), holder["properties"]["stamp"]);
        assert_eq!(Clock::json_schema(), holder["properties"]["clock"]);
        assert_eq!(Moment::json_schema(), holder["properties"]["moment"]);
        assert_eq!(Instant::json_schema(), holder["properties"]["instant"]);
    }

    /// The wire is a string, so the brand's own constraints stay legal — and sit beside `type` and
    /// `format` the way they sit beside `type` alone. The `\d` the brand was declared with reaches
    /// the schema as the members it stands for, per the pattern guard's cross-engine translation.
    #[test]
    fn a_constrained_chrono_brand_carries_its_constraints_beside_the_format() {
        assert_eq!(
            IsoStamp::json_schema(),
            serde_json::json!({
                "type": "string",
                "format": "date",
                "minLength": 10_u32,
                "maxLength": 10_u32,
                "pattern": "^[0-9]{4}-[0-9]{2}-[0-9]{2}$"
            })
        );
    }

    /// The Zod value already emitted the same `z.iso.*` schema field position emits, and the
    /// annotation is the class zod gives it; only the JSON schema moved.
    #[cfg(all(feature = "typescript", feature = "zod"))]
    #[test]
    fn the_zod_surfaces_of_a_chrono_brand_are_unchanged() {
        let zod = Stamp::zod_schema();
        assert!(
            zod.contains(r#"const Stamp$RawSchema = z.iso.date().brand<"Stamp">()"#),
            "Got:\n{zod}"
        );
        assert!(
            zod.contains(r#"Stamp$Schema: $ZodBranded<ZodString, "Stamp">"#),
            "Got:\n{zod}"
        );
        assert_eq!(
            Stamp::ts_definition(),
            r#"export type Stamp = string & $brand<"Stamp">;"#
        );
    }
}

// A brand's constrained value is its inner field itself, so a path inner is measured the way a
// path field is: by the string serde writes for it. `no_display` is what the brand's own `Display`
// impl needs — a path has none to delegate to — and the constraints reach the value without one.
// Compiling this module is the assertion that a path brand is emitted at all.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
mod constrained_path_brand_tests {
    use super::*;
    use std::path::PathBuf;

    #[model_schema(no_display, minLength = 3, pattern = "^/[a-z]+$")]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct AssetPath(pub PathBuf);

    #[test]
    fn test_a_constrained_path_brand_is_held_to_its_bound_on_the_wire() {
        let too_short = serde_json::from_str::<AssetPath>("\"/a\"").unwrap_err();
        assert!(
            too_short
                .to_string()
                .contains("value is too short: minimum length is 3, got 2"),
            "Unexpected error: {too_short}"
        );

        let unmatched = serde_json::from_str::<AssetPath>("\"etc\"").unwrap_err();
        assert!(
            unmatched
                .to_string()
                .contains("value does not match pattern '^/[a-z]+$'"),
            "Unexpected error: {unmatched}"
        );

        let accepted = serde_json::from_str::<AssetPath>("\"/etc\"").unwrap();
        assert_eq!(accepted, AssetPath(PathBuf::from("/etc")));
        assert!(
            accepted.validate().is_ok(),
            "A payload the wire admits must be one validate() admits: {:?}",
            accepted.validate().err()
        );
    }

    #[test]
    fn test_a_constrained_path_brand_is_held_to_its_bound_by_validate() {
        AssetPath(PathBuf::from("/etc")).validate().unwrap();
        assert_eq!(
            AssetPath(PathBuf::from("/a")).validate().unwrap_err(),
            vec!["value is too short: minimum length is 3, got 2"]
        );
        assert_eq!(
            AssetPath(PathBuf::from("etcetera")).validate().unwrap_err(),
            vec!["value does not match pattern '^/[a-z]+$'"]
        );
    }

    // The bound the brand renders and the bound it enforces are one bound; a schema that
    // constrains what nothing checks is the disagreement this covers.
    #[cfg(feature = "zod")]
    #[test]
    fn test_the_zod_bound_on_a_path_brand_is_the_bound_the_wire_enforces() {
        let zod = AssetPath::zod_schema();
        assert!(zod.contains("z.string().min(3)"), "Got:\n{zod}");
        assert!(zod.contains(&brand_marker("AssetPath")), "Got:\n{zod}");
        assert!(
            serde_json::from_str::<AssetPath>("\"/a\"").is_err(),
            "the rendered minimum admits no shorter value on the wire"
        );
    }
}

// A transparent wrapper writes nothing of its own and derefs to what it holds, so a path branded
// under one is the same path on every surface a bare one is — and the constrained checks reach it
// through that deref, not through a `Display` none of these spellings has. Compiling this module is
// the assertion that each spelling is emitted at all.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
mod wrapped_path_brand_tests {
    use super::*;
    use alloc::borrow::Cow;
    use alloc::rc::Rc;
    use alloc::sync::Arc;
    use core::fmt::Debug;
    use serde::de::DeserializeOwned;
    use std::path::{Path, PathBuf};

    #[model_schema(no_display, minLength = 3, pattern = "^/[a-z]+$")]
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct ArcedPath(pub Arc<Path>);

    #[model_schema(no_display, minLength = 3, pattern = "^/[a-z]+$")]
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct BoxedPath(pub Box<Path>);

    #[model_schema(no_display, minLength = 3, pattern = "^/[a-z]+$")]
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct CowedPath(pub Cow<'static, Path>);

    #[model_schema(no_display, minLength = 3, pattern = "^/[a-z]+$")]
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct RcedPath(pub Rc<Path>);

    /// The wire criterion each spelling is held to: the bound rejects, the accepted payload is the
    /// path it spelled, and what the wire admits `validate()` admits too.
    fn assert_wire_bound<Brand>(
        name: &str,
        inner: fn(&Brand) -> &Path,
        validate: fn(&Brand) -> Result<(), Vec<String>>,
    ) where
        Brand: Debug + DeserializeOwned,
    {
        let too_short = serde_json::from_str::<Brand>("\"/a\"").unwrap_err();
        assert!(
            too_short
                .to_string()
                .contains("value is too short: minimum length is 3, got 2"),
            "for {name}, unexpected error: {too_short}"
        );

        let unmatched = serde_json::from_str::<Brand>("\"etc\"").unwrap_err();
        assert!(
            unmatched
                .to_string()
                .contains("value does not match pattern '^/[a-z]+$'"),
            "for {name}, unexpected error: {unmatched}"
        );

        let accepted = serde_json::from_str::<Brand>("\"/etc\"").unwrap();
        assert_eq!(inner(&accepted), Path::new("/etc"), "for {name}");
        assert!(
            validate(&accepted).is_ok(),
            "for {name}, a payload the wire admits must be one validate() admits: {:?}",
            validate(&accepted).err()
        );
    }

    /// The same bound reached from Rust rather than the wire, on a value built directly.
    fn assert_validate_bound<Brand>(
        name: &str,
        wrap: fn(&str) -> Brand,
        validate: fn(&Brand) -> Result<(), Vec<String>>,
    ) {
        validate(&wrap("/etc")).unwrap();
        assert_eq!(
            validate(&wrap("/a")).unwrap_err(),
            vec!["value is too short: minimum length is 3, got 2"],
            "for {name}"
        );
        assert_eq!(
            validate(&wrap("etcetera")).unwrap_err(),
            vec!["value does not match pattern '^/[a-z]+$'"],
            "for {name}"
        );
    }

    #[test]
    fn test_every_wrapped_path_brand_is_held_to_its_bound_on_the_wire() {
        assert_wire_bound::<ArcedPath>("ArcedPath", |brand| &brand.0, ArcedPath::validate);
        assert_wire_bound::<BoxedPath>("BoxedPath", |brand| &brand.0, BoxedPath::validate);
        assert_wire_bound::<CowedPath>("CowedPath", |brand| &brand.0, CowedPath::validate);
        assert_wire_bound::<RcedPath>("RcedPath", |brand| &brand.0, RcedPath::validate);
    }

    #[test]
    fn test_every_wrapped_path_brand_is_held_to_its_bound_by_validate() {
        assert_validate_bound(
            "ArcedPath",
            |raw| ArcedPath(Arc::from(Path::new(raw))),
            ArcedPath::validate,
        );
        assert_validate_bound(
            "BoxedPath",
            |raw| BoxedPath(Box::from(Path::new(raw))),
            BoxedPath::validate,
        );
        assert_validate_bound(
            "CowedPath",
            |raw| CowedPath(Cow::Owned(PathBuf::from(raw))),
            CowedPath::validate,
        );
        assert_validate_bound(
            "RcedPath",
            |raw| RcedPath(Rc::from(Path::new(raw))),
            RcedPath::validate,
        );
    }

    // The wrapper is nothing on the wire, so what a wrapped path brand renders is what the bare one
    // renders; a bound that moved with the spelling would be one the three surfaces no longer share.
    #[cfg(feature = "zod")]
    #[test]
    fn test_a_wrapped_path_brand_renders_the_bare_brands_zod_schema() {
        let bare = super::constrained_path_brand_tests::AssetPath::zod_schema();
        for (rendered, name) in [
            (ArcedPath::zod_schema(), "ArcedPath"),
            (BoxedPath::zod_schema(), "BoxedPath"),
            (CowedPath::zod_schema(), "CowedPath"),
            (RcedPath::zod_schema(), "RcedPath"),
        ] {
            assert_eq!(
                rendered,
                bare.replace("AssetPath", name),
                "for {name}, got:\n{rendered}"
            );
        }
    }

    #[cfg(feature = "jsonschema")]
    #[test]
    fn test_a_wrapped_path_brand_renders_the_bare_brands_json_schema() {
        let bare = super::constrained_path_brand_tests::AssetPath::json_schema();
        for (rendered, name) in [
            (ArcedPath::json_schema(), "ArcedPath"),
            (BoxedPath::json_schema(), "BoxedPath"),
            (CowedPath::json_schema(), "CowedPath"),
            (RcedPath::json_schema(), "RcedPath"),
        ] {
            assert_eq!(rendered, bare, "for {name}, got:\n{rendered}");
        }
    }
}

// `no_display` drops the `Display` impl, not the `Display` requirement: a brand whose checks read
// the inner's `to_string()` still needs it to render. Compiling this module is the assertion that
// the combination is accepted and still wired to the constraints.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
mod constrained_no_display_tests {
    use super::*;

    #[model_schema(no_display, pattern = "^[a-z0-9_]+$", minLength = 3)]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct QuietSlug(pub String);

    #[test]
    fn test_constrained_no_display_brand_validates() {
        QuietSlug("hello_world".to_owned()).validate().unwrap();
        QuietSlug("NO".to_owned()).validate().unwrap_err();
    }

    #[test]
    fn test_constrained_no_display_brand_enforces_constraints_through_serde() {
        serde_json::from_str::<QuietSlug>("\"hello_world\"").unwrap();
        serde_json::from_str::<QuietSlug>("\"NO\"").unwrap_err();
    }
}

// zod=OFF, typescript=ON tests
#[cfg(all(feature = "typescript", not(feature = "zod")))]
mod no_zod_tests {
    use super::*;

    #[model_schema(default_types(IdType = String))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct RoleIdNoZod<IdType>(pub IdType);

    // Non-generic branded newtype without Zod
    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct SessionToken(pub String);

    #[test]
    fn test_branded_newtype_no_zod_ts_definition() {
        let ts = RoleIdNoZod::<String>::ts_definition();
        assert!(
            ts.contains("declare const __brand_RoleIdNoZod: unique symbol"),
            "Got: {ts}"
        );
        assert!(
            ts.contains(
                "export type RoleIdNoZod<IdType> = IdType & { readonly [__brand_RoleIdNoZod]: true }"
            ),
            "Got: {ts}"
        );
    }

    #[test]
    fn test_branded_newtype_non_generic_no_zod() {
        let ts = SessionToken::ts_definition();
        // Should have unique symbol declaration
        assert!(
            ts.contains("declare const __brand_SessionToken: unique symbol"),
            "Should have unique symbol. Got: {ts}"
        );
        // Should have non-generic type definition
        assert!(
            ts.contains(
                "export type SessionToken = string & { readonly [__brand_SessionToken]: true }"
            ),
            "Should have branded type without generics. Got: {ts}"
        );
    }
}

// Serde transparent test (always runs when serde is available)
#[cfg(feature = "serde")]
mod serde_tests {
    use super::*;

    #[model_schema(default_types(IdType = String))]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct GenericId<IdType>(pub IdType);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct WrappedString(pub String);

    #[test]
    fn test_branded_newtype_serde_transparent() {
        // WrappedString("abc") should serialize as "abc"
        let val = WrappedString("abc".to_owned());
        let json = serde_json::to_string(&val).unwrap();
        assert_eq!(json, "\"abc\"");

        // "abc" should deserialize as WrappedString("abc")
        let deserialized: WrappedString = serde_json::from_str("\"abc\"").unwrap();
        assert_eq!(deserialized, WrappedString("abc".to_owned()));
    }

    #[test]
    fn test_branded_newtype_generic_serde_roundtrip() {
        // Serialize a generic branded newtype with String inner type
        let original = GenericId::<String>("abc-123".to_owned());
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(json, "\"abc-123\"", "Should serialize transparently");

        // Deserialize back
        let deserialized: GenericId<String> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, original, "Roundtrip should preserve equality");

        // Also test with a numeric inner type
        let num_original = GenericId::<u64>(42);
        let num_json = serde_json::to_string(&num_original).unwrap();
        assert_eq!(num_json, "42", "Should serialize u64 transparently");

        let num_deserialized: GenericId<u64> = serde_json::from_str(&num_json).unwrap();
        assert_eq!(
            num_deserialized, num_original,
            "Numeric roundtrip should preserve equality"
        );
    }
}

/// One value is one wire, and every spelling of that value publishes the one JSON type keyword the
/// wire describes as.
///
/// `#[serde(transparent)]` puts the inner on the wire by itself, so a brand over a `u32` writes
/// exactly what a field, a one-slot tuple struct and an alias of the same `u32` write. The four are
/// pinned against each other because a keyword taken from the rendered TypeScript name instead of
/// from the type spelled `integer` three times and `number` once — the same wire named twice, which
/// the merge repeating the keyword cannot pick a side in.
#[cfg(all(feature = "jsonschema", feature = "serde"))]
mod branded_scalar_keyword_tests {
    use super::*;

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct TickBrand(pub u32);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct TickSlot(pub u32);

    #[model_schema()]
    pub type TickAlias = u32;

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct HoldsTick {
        pub ticks: u32,
    }

    #[model_schema(no_display)]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct RatioBrand(pub f64);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct SwitchBrand(pub bool);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct SlugBrand(pub String);

    #[test]
    fn every_spelling_of_one_integer_publishes_one_keyword() {
        let integer = serde_json::json!({ "type": "integer" });
        assert_eq!(TickBrand::json_schema(), integer);
        assert_eq!(TickSlot::json_schema(), integer);
        assert_eq!(tick_alias_schema::Schema::json_schema(), integer);
        assert_eq!(HoldsTick::json_schema()["properties"]["ticks"], integer);

        let aliased: TickAlias = 7;
        assert_eq!(
            serde_json::to_value(aliased).unwrap(),
            serde_json::json!(7_u32)
        );
    }

    /// And the brands whose inner is not an integer describe exactly what they described before:
    /// a float is the `number` it always was, and a `bool` and a `String` are untouched.
    #[test]
    fn the_brands_over_every_other_scalar_are_byte_identical() {
        assert_eq!(
            RatioBrand::json_schema(),
            serde_json::json!({ "type": "number" })
        );
        assert_eq!(
            SwitchBrand::json_schema(),
            serde_json::json!({ "type": "boolean" })
        );
        assert_eq!(
            SlugBrand::json_schema(),
            serde_json::json!({ "type": "string" })
        );
    }

    /// The value the schema describes is the value serde writes: an integer keyword over a payload
    /// serde renders with no fractional part.
    #[test]
    fn the_keyword_names_what_serde_writes() {
        assert_eq!(serde_json::to_string(&TickBrand(7)).unwrap(), "7");
        assert_eq!(serde_json::to_string(&RatioBrand(0.5)).unwrap(), "0.5");
    }
}

#[cfg(any(
    feature = "serde",
    feature = "zod",
    feature = "typescript",
    feature = "jsonschema"
))]
use serde::{Deserialize, Serialize};
#[cfg(any(
    feature = "serde",
    feature = "zod",
    feature = "typescript",
    feature = "jsonschema"
))]
use tixschema::model_schema;

/// The marker a brand carries, as this build spells it: the name is a *type* argument, which only
/// a build emitting TypeScript writes at all.
#[cfg(feature = "zod")]
fn brand_marker(item_name: &str) -> String {
    #[cfg(feature = "typescript")]
    {
        format!(".brand<\"{item_name}\">()")
    }
    #[cfg(not(feature = "typescript"))]
    {
        let _: &str = item_name;
        ".brand()".to_owned()
    }
}
