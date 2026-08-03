use alloc::borrow::Cow;
use alloc::rc::Rc;
use alloc::sync::Arc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tixschema::model_schema;

/// `Path` is `PathBuf`'s borrowed form, reachable only behind a wrapper or a reference. Every
/// spelling below writes the same JSON string the owned form writes.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct PathFields {
    arced: Arc<Path>,
    boxed: Box<Path>,
    cowed: Cow<'static, Path>,
    owned: PathBuf,
    rced: Rc<Path>,
}

/// The spelling every field above is held against: the owned form written bare.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct PlainPathFields {
    arced: PathBuf,
    boxed: PathBuf,
    cowed: PathBuf,
    owned: PathBuf,
    rced: PathBuf,
}

fn path_fields() -> PathFields {
    PathFields {
        arced: Arc::from(Path::new("/etc/hosts")),
        boxed: PathBuf::from("/etc/hosts").into_boxed_path(),
        cowed: Cow::Owned(PathBuf::from("/etc/hosts")),
        owned: PathBuf::from("/etc/hosts"),
        rced: Rc::from(Path::new("/etc/hosts")),
    }
}

fn plain_path_fields() -> PlainPathFields {
    PlainPathFields {
        arced: PathBuf::from("/etc/hosts"),
        boxed: PathBuf::from("/etc/hosts"),
        cowed: PathBuf::from("/etc/hosts"),
        owned: PathBuf::from("/etc/hosts"),
        rced: PathBuf::from("/etc/hosts"),
    }
}

/// The criterion the mapping rests on: serde writes a borrowed path exactly as it writes the owned
/// one, so the two spellings owe the same schema.
#[test]
fn test_every_path_spelling_writes_the_owned_form_wire_value() {
    assert_eq!(
        serde_json::to_value(path_fields()).unwrap(),
        serde_json::to_value(plain_path_fields()).unwrap()
    );
}

#[test]
fn test_a_borrowed_path_field_round_trips_through_the_owned_wire_form() {
    let payload = serde_json::to_string(&path_fields()).unwrap();
    assert_eq!(
        serde_json::from_str::<PathFields>(&payload).unwrap(),
        path_fields()
    );
}

/// The field declarations of a generated `TypeScript` definition, without the `JSDoc` around them
/// — the struct's own doc comment is the one thing the twins are not meant to share.
#[cfg(feature = "typescript")]
fn ts_field_declarations(definition: &str) -> Vec<String> {
    definition
        .lines()
        .filter(|line| line.starts_with("  ") && line.ends_with(';'))
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(feature = "typescript")]
#[test]
fn test_path_fields_render_as_strings_in_typescript() {
    let declarations = ts_field_declarations(&PathFields::ts_definition());
    assert_eq!(
        declarations,
        ts_field_declarations(&PlainPathFields::ts_definition())
    );
    for field in ["arced", "boxed", "cowed", "owned", "rced"] {
        assert!(
            declarations.contains(&format!("  {field}: string;")),
            "for {field}, got: {declarations:?}"
        );
    }
}

#[cfg(feature = "zod")]
#[test]
fn test_path_fields_render_as_strings_in_zod() {
    let schema = PathFields::zod_schema();
    assert_eq!(
        schema.replace("PathFields", "PlainPathFields"),
        PlainPathFields::zod_schema()
    );
    for field in ["arced", "boxed", "cowed", "owned", "rced"] {
        assert!(
            schema.contains(&format!("{field}: z.string()")),
            "for {field}, got: {schema}"
        );
    }
}

#[cfg(feature = "jsonschema")]
#[test]
fn test_path_fields_render_as_strings_in_json_schema() {
    let schema = PathFields::json_schema();
    let properties = schema
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .unwrap();
    for field in ["arced", "boxed", "cowed", "owned", "rced"] {
        assert_eq!(
            properties.get(field).unwrap().get("type").unwrap(),
            &serde_json::json!("string"),
            "for {field}, got: {schema}"
        );
    }
    assert_eq!(
        serde_json::to_string(&schema)
            .unwrap()
            .replace("PathFields", "PlainPathFields"),
        serde_json::to_string(&PlainPathFields::json_schema()).unwrap()
    );
}

/// A constrained path is measured by the string serde writes for it, so the bound the three
/// surfaces render is the bound both the wire and `validate()` hold the field to.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_a_constrained_path_is_rejected_on_the_wire_and_by_validate() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    struct ConstrainedPath {
        #[model_schema_prop(minLength = 3, pattern = "^/[a-z]+$")]
        owned: PathBuf,
    }

    let too_short = serde_json::from_str::<ConstrainedPath>(r#"{"owned":"/a"}"#).unwrap_err();
    assert!(
        too_short
            .to_string()
            .contains("'owned' is too short: minimum length is 3, got 2"),
        "Unexpected error: {too_short}"
    );

    let unmatched = serde_json::from_str::<ConstrainedPath>(r#"{"owned":"etc"}"#).unwrap_err();
    assert!(
        unmatched
            .to_string()
            .contains("'owned' does not match pattern '^/[a-z]+$'"),
        "Unexpected error: {unmatched}"
    );

    let accepted = serde_json::from_str::<ConstrainedPath>(r#"{"owned":"/etc"}"#).unwrap();
    assert_eq!(accepted.owned, PathBuf::from("/etc"));
    assert!(
        accepted.validate().is_ok(),
        "A payload the wire admits must be one validate() admits: {:?}",
        accepted.validate().err()
    );

    let short = ConstrainedPath {
        owned: PathBuf::from("/a"),
    };
    assert_eq!(
        short.validate().unwrap_err(),
        vec!["'owned' is too short: minimum length is 3, got 2"]
    );
}

/// Every spelling of a path field writes the same wire string, so each carries its constraint to
/// the same place — the borrowed forms through the wrapper they are reachable behind.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_every_constrained_path_spelling_is_held_to_its_bound() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    struct ConstrainedPaths {
        #[model_schema_prop(minLength = 3)]
        arced: Arc<Path>,
        #[model_schema_prop(minLength = 3)]
        boxed: Box<Path>,
        #[model_schema_prop(minLength = 3)]
        cowed: Cow<'static, Path>,
        #[model_schema_prop(minLength = 3)]
        listed: Vec<PathBuf>,
        #[model_schema_prop(minLength = 3)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        maybe: Option<PathBuf>,
        #[model_schema_prop(minLength = 3)]
        owned: PathBuf,
        #[model_schema_prop(minLength = 3)]
        rced: Rc<Path>,
    }

    const GOOD: &str = r#"{"arced":"/aaa","boxed":"/bbb","cowed":"/ccc","listed":["/ddd"],"maybe":"/eee","owned":"/fff","rced":"/ggg"}"#;
    const SPELLINGS: [(&str, &str); 7] = [
        ("arced", r#""a""#),
        ("boxed", r#""b""#),
        ("cowed", r#""c""#),
        ("listed", r#"["d"]"#),
        ("maybe", r#""e""#),
        ("owned", r#""f""#),
        ("rced", r#""g""#),
    ];

    let accepted = serde_json::from_str::<ConstrainedPaths>(GOOD).unwrap();
    assert!(
        accepted.validate().is_ok(),
        "A payload the wire admits must be one validate() admits: {:?}",
        accepted.validate().err()
    );

    for (field, short) in SPELLINGS {
        let mut payload: serde_json::Value = serde_json::from_str(GOOD).unwrap();
        payload[field] = serde_json::from_str(short).unwrap();
        let error = serde_json::from_str::<ConstrainedPaths>(&payload.to_string()).unwrap_err();
        assert!(
            error.to_string().contains(&format!(
                "'{field}' is too short: minimum length is 3, got 1"
            )),
            "spelling {field} was admitted by the wire: {error}"
        );
    }

    let short = ConstrainedPaths {
        arced: Arc::from(Path::new("a")),
        boxed: PathBuf::from("b").into_boxed_path(),
        cowed: Cow::Owned(PathBuf::from("c")),
        listed: vec![PathBuf::from("d")],
        maybe: Some(PathBuf::from("e")),
        owned: PathBuf::from("f"),
        rced: Rc::from(Path::new("g")),
    };
    assert_eq!(
        short.validate().unwrap_err(),
        SPELLINGS
            .iter()
            .map(|(field, _)| format!("'{field}' is too short: minimum length is 3, got 1"))
            .collect::<Vec<_>>(),
        "every spelling must report for itself"
    );
}

/// The bound rendered for a path field and the bound enforced for it are one bound — the
/// disagreement this covers is a schema that constrains what nothing checks.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn test_the_zod_bound_on_a_path_is_the_bound_the_wire_enforces() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    struct RenderedBound {
        #[model_schema_prop(minLength = 3)]
        owned: PathBuf,
    }

    let schema = RenderedBound::zod_schema();
    assert!(schema.contains("owned: z.string().min(3)"), "got: {schema}");
    assert!(
        serde_json::from_str::<RenderedBound>(r#"{"owned":"/a"}"#).is_err(),
        "the rendered minimum admits no shorter value on the wire"
    );
}

/// The rendering the checks measure, where it and the raw path part ways: serde refuses to write a
/// path that is not UTF-8 at all, so the lossy form is the wire value wherever there is one, and
/// the paths it renders differently are the ones no payload carries.
#[cfg(all(
    unix,
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_a_path_that_is_not_utf8_is_measured_by_its_lossy_rendering() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt as _;

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    struct LossilyMeasured {
        #[model_schema_prop(minLength = 3)]
        owned: PathBuf,
    }

    let instance = LossilyMeasured {
        owned: PathBuf::from(OsStr::from_bytes(&[0xff])),
    };
    assert!(
        instance.validate().is_ok(),
        "one invalid byte renders as the three-byte replacement character: {:?}",
        instance.validate().err()
    );
    assert!(
        serde_json::to_string(&instance).is_err(),
        "serde writes no wire value for a path that is not UTF-8"
    );
}
