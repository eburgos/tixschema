//! One declaration written twice: once through a `macro_rules!` metavariable, once by hand.
//!
//! The fixtures below are the same struct. The only thing that differs between them is which of
//! the two ways the field types were spelled, and that is not something any surface describes — so
//! every assertion here is the same assertion: the two describe identically, and neither describes
//! a field as the opaque value.

#[cfg(feature = "jsonschema")]
mod jsonschema {
    use super::{Substituted, Written};
    #[cfg(feature = "serde")]
    use super::{SubstitutedSlug, WrittenSlug};

    #[test]
    fn a_substituted_field_type_documents_as_the_written_one() {
        assert_eq!(Substituted::json_schema(), Written::json_schema());
    }

    /// The empty schema admits every payload, which is what a field the reader could not classify
    /// documents as. Every member here names a type, so none of them may.
    #[test]
    fn no_substituted_field_documents_as_the_empty_schema() {
        let schema = Substituted::json_schema();
        let properties = schema["properties"].as_object().unwrap().clone();
        assert_eq!(properties.len(), 6, "Got: {schema}");
        for (member, described) in &properties {
            assert!(
                described.as_object().is_some_and(|body| !body.is_empty()),
                "{member} describes nothing: {schema}"
            );
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn a_substituted_brand_inner_documents_as_the_written_one() {
        assert_eq!(SubstitutedSlug::json_schema(), WrittenSlug::json_schema());
    }
}

#[cfg(feature = "typescript")]
mod typescript {
    use super::{Substituted, Written, as_written};
    #[cfg(feature = "serde")]
    use super::{SubstitutedSlug, WrittenSlug};

    #[test]
    fn a_substituted_field_type_reads_as_the_written_one() {
        assert_eq!(
            as_written(&Substituted::ts_definition()),
            Written::ts_definition()
        );
    }

    /// The failure this pins is a silent one: a type the reader has no arm for lands on the opaque
    /// value, which is a declaration that describes nothing rather than one that refuses.
    #[test]
    fn no_substituted_field_lands_on_the_opaque_value() {
        let ts = Substituted::ts_definition();
        assert!(!ts.contains("unknown"), "Got: {ts}");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn a_substituted_brand_inner_reads_as_the_written_one() {
        assert_eq!(
            as_written(&SubstitutedSlug::ts_definition()),
            WrittenSlug::ts_definition()
        );
    }
}

/// The brand's own validator reads the inner type to decide how the checked value reaches it, so
/// the substituted brand has to accept and refuse exactly what the written one does.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
mod wire {
    use super::{SubstitutedSlug, WrittenSlug};

    #[test]
    fn a_substituted_brand_measures_the_value_the_written_one_measures() {
        for candidate in ["", "ok", "far too long to fit"] {
            let payload = serde_json::to_string(candidate).unwrap();
            let substituted = serde_json::from_str::<SubstitutedSlug>(&payload).is_ok();
            let written = serde_json::from_str::<WrittenSlug>(&payload).is_ok();
            assert_eq!(substituted, written, "for {candidate:?}");
        }
    }
}

#[cfg(feature = "zod")]
mod zod {
    use super::{Substituted, Written, as_written};
    #[cfg(feature = "serde")]
    use super::{SubstitutedSlug, WrittenSlug};

    #[test]
    fn a_substituted_field_type_validates_as_the_written_one() {
        assert_eq!(
            as_written(&Substituted::zod_schema()),
            Written::zod_schema()
        );
    }

    #[test]
    fn no_substituted_field_lands_on_the_opaque_value() {
        let zod = Substituted::zod_schema();
        assert!(!zod.contains("z.unknown()"), "Got: {zod}");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn a_substituted_brand_inner_validates_as_the_written_one() {
        assert_eq!(
            as_written(&SubstitutedSlug::zod_schema()),
            WrittenSlug::zod_schema()
        );
    }
}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tixschema::model_schema;

macro_rules! declare_holder {
    ($name:ident, $text:ty, $count:ty, $many:ty, $maybe:ty, $keyed:ty, $pair:ty) => {
        #[model_schema()]
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub struct $name {
            pub count: $count,
            pub keyed: $keyed,
            pub many: $many,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub maybe: $maybe,
            pub pair: $pair,
            pub text: $text,
        }
    };
}

/// A brand narrows the value its inner type describes, so a constrained brand has to read that
/// inner as the type it names — through the substitution as much as beside it.
#[cfg(feature = "serde")]
macro_rules! declare_slug {
    ($name:ident, $inner:ty) => {
        #[model_schema(minLength = 1, maxLength = 8)]
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub $inner);
    };
}

declare_holder!(
    Substituted,
    String,
    u32,
    Vec<String>,
    Option<u32>,
    HashMap<String, u32>,
    (String, u32)
);

#[cfg(feature = "serde")]
declare_slug!(SubstitutedSlug, String);

#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Written {
    pub count: u32,
    pub keyed: HashMap<String, u32>,
    pub many: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maybe: Option<u32>,
    pub pair: (String, u32),
    pub text: String,
}

#[cfg(feature = "serde")]
#[model_schema(minLength = 1, maxLength = 8)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WrittenSlug(pub String);

/// The substituted item's surface under the written twin's name, so the two can be compared as the
/// one description they are — the name is the only thing about them that differs.
///
/// The JSON document names nothing, so only the two rendered surfaces have a name to put back.
#[cfg(any(feature = "typescript", feature = "zod"))]
fn as_written(rendered: &str) -> String {
    rendered.replace("Substituted", "Written")
}
