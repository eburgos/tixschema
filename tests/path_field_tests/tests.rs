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
