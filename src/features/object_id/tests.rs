use super::*;

/// The `$oid` strings the two surfaces were told apart by, with the verdict a flagless ECMA-262
/// regex gives each: `new RegExp(OBJECT_ID_HEX_PATTERN).test(oid)` under node v26.2.0. The upper-
/// case hex is the one a flag turns, and the lower-case one is what `ObjectId::to_hex()` writes.
#[cfg(feature = "object_id")]
const OBJECT_ID_HEX_STRINGS: [(&str, bool); 2] = [
    ("507f1f77bcf86cd799439011", true),
    ("507F1F77BCF86CD799439011", false),
];

#[test]
fn test_object_id_detection() {
    assert!(is_object_id_type("ObjectId"));
    assert!(!is_object_id_type("String"));
    assert!(!is_object_id_type("UserId"));
}

#[test]
fn test_object_id_typescript_type() {
    assert_eq!(get_object_id_typescript_type(), "ObjectId");
}

#[cfg(feature = "object_id")]
#[test]
fn test_object_id_zod_schema() {
    let schema = get_object_id_zod_schema();
    assert!(schema.contains("$oid"));
    assert!(schema.contains("regex"));
    assert!(schema.contains("24"));
}

/// The regex literal the Zod surface writes, split into its source and the flags it carries.
#[cfg(feature = "object_id")]
fn emitted_zod_literal(zod_schema: &str) -> (&str, &str) {
    let literal = zod_schema.split_once(".regex(/").unwrap().1;
    let (source, after) = literal.split_once('/').unwrap();
    let (flags, _) = after.split_once(',').unwrap();
    (source, flags)
}

/// A JavaScript regex literal as the `regex` crate reads it, on the same recorded-JavaScript
/// discipline the emitted-pattern tests use. The flags decide a verdict as much as the source does,
/// so they are translated rather than dropped; a flag with no reading recorded here stops the test
/// instead of being silently ignored.
#[cfg(feature = "object_id")]
fn javascript_literal(source: &str, flags: &str) -> regex::Regex {
    assert!(
        flags.chars().all(|flag| flag == 'i'),
        "no reading is recorded for the flag set /{flags}"
    );
    let case_folding = if flags.contains('i') { "(?i)" } else { "" };
    regex::Regex::new(&format!("{case_folding}{source}")).unwrap()
}

/// Both surfaces constrain `$oid` by one hex under one case rule.
///
/// A JSON Schema `pattern` is a flagless ECMA-262 regex with nowhere to hold a flag, so agreement
/// can only come from the source spelling — which leaves the Zod literal's flag set as the one
/// place the two can part ways, and an empty one as the only way they cannot. The contract both
/// describe is what serde writes, and `ObjectId::to_hex()` writes lower-case.
#[cfg(feature = "object_id")]
#[test]
fn test_both_object_id_surfaces_read_one_hex_under_one_case_rule() {
    let zod_schema = get_object_id_zod_schema();
    let (source, flags) = emitted_zod_literal(&zod_schema);
    assert_eq!(
        source, OBJECT_ID_HEX_PATTERN,
        "the Zod literal spells the `$oid` hex its own way"
    );
    let zod = javascript_literal(source, flags);
    // The `pattern` keyword takes the constant with no flag, because it has nowhere to put one.
    let json_schema = javascript_literal(OBJECT_ID_HEX_PATTERN, "");

    for (oid, javascript) in OBJECT_ID_HEX_STRINGS {
        assert_eq!(
            json_schema.is_match(oid),
            javascript,
            "the JSON Schema `pattern` parts ways with JavaScript over {oid:?}"
        );
        assert_eq!(
            zod.is_match(oid),
            json_schema.is_match(oid),
            "the two `$oid` surfaces part ways over {oid:?}: Zod reads /{source}/{flags}, \
             the JSON Schema `pattern` reads {source} with no flag"
        );
    }
}
