//! JSON Schema generation feature module.
//!
//! This module handles JSON schema generation when the "jsonschema" feature is enabled.

/// What every pointer the crate writes opens with; what follows it is the name being deferred.
const DEFS_PREFIX: &str = "#/$defs/";

/// How a merge that cannot proceed names itself in the diagnostic it raises.
///
/// `subject` is the frame that reads the merge, `edge` what the merged schema was reached through,
/// and each remedy the way out in the spelling that applies where that edge was written.
pub struct MergeDiagnostic<'msg> {
    /// The way out of a cycle: what makes the edge defer rather than merge.
    pub cycle_remedy: &'msg str,
    pub edge: &'msg str,
    /// The way out of a merged value that is not an object: what gives that value a place of its
    /// own.
    pub non_object_remedy: &'msg str,
    pub subject: &'msg str,
}

/// One schema merged into a base, together with how the author named it.
///
/// A merged schema is a `serde_json` expression by the time it reaches the merge and no longer
/// carries the name it came from, so the label travels beside it — it is what a diagnostic points
/// the author at.
pub struct MergedSource {
    pub label: String,
    /// Whether the value was reached through an `Option`. serde writes the members of a `Some` into
    /// the object being written and writes nothing at all for a `None`, so an optional source is two
    /// key sets rather than one — a choice the merge multiplies the base out over, the same way a
    /// union is.
    pub optional: bool,
    pub value: proc_macro2::TokenStream,
}

/// Check if we should generate JSON schema methods.
#[cfg(test)]
pub const fn should_generate_json_schema() -> bool {
    true // Always true when this module is compiled (feature is enabled)
}

/// Where a document holds the definition of `def_name`.
///
/// The crate writes draft 2020-12 (`prefixItems` with `"items": false` is that draft's fixed-arity
/// array, and the draft before it spells the same array with `items`/`additionalItems`), whose
/// deferred schema is a `$ref` into the document's own `$defs`.
fn defs_pointer(def_name: &str) -> String {
    format!("{DEFS_PREFIX}{def_name}")
}

/// The pair of JSON-schema methods every schema module publishes.
///
/// `json_schema` is the document a caller asks for. `json_schema_within` is that same description
/// written into a document already being built, and carries the two things that have to travel
/// with it: the names whose descriptions are still being written, and the definitions the
/// document's root must hold.
///
/// A cycle is only knowable there. A type names another by inlining it, and no expansion can see
/// the cycle it is part of — the other type may not have been expanded yet, and one that has been
/// cannot be revisited. So the name is recognized while the description runs: a name re-entered
/// while still in flight describes as a `$ref` into `$defs`, and the frame that put it in flight
/// hoists its body to the root that pointer resolves against.
pub fn json_schema_methods(
    def_name: &str,
    body: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let pointer = defs_pointer(def_name);
    quote::quote! {
        pub fn json_schema() -> serde_json::Value {
            let mut in_flight: Vec<&'static str> = Vec::new();
            let mut hoisted_defs = serde_json::Map::new();
            let described = Self::json_schema_within(&mut in_flight, &mut hoisted_defs);
            if hoisted_defs.is_empty() {
                return described;
            }
            // The pointers into them are from the root, so the definitions join it — ahead of the
            // description, which is the rest of the document. Every description the crate writes
            // is an object, which is what can take them as a member.
            let mut rooted = serde_json::Map::new();
            rooted.insert("$defs".to_string(), serde_json::Value::Object(hoisted_defs));
            if let serde_json::Value::Object(members) = described {
                rooted.extend(members);
            }
            serde_json::Value::Object(rooted)
        }

        pub fn json_schema_within(
            in_flight: &mut Vec<&'static str>,
            hoisted_defs: &mut serde_json::Map<String, serde_json::Value>,
        ) -> serde_json::Value {
            if in_flight.contains(&#def_name) {
                // Reserved rather than written: the frame that put this name in flight is still
                // writing the body, and fills the entry in once it has one.
                hoisted_defs.entry(#def_name).or_insert(serde_json::Value::Null);
                return serde_json::json!({ "$ref": #pointer });
            }
            in_flight.push(#def_name);
            let described = #body;
            in_flight.pop();
            if hoisted_defs.contains_key(#def_name) {
                hoisted_defs.insert(#def_name.to_string(), described);
                return serde_json::json!({ "$ref": #pointer });
            }
            described
        }
    }
}

/// Generates the JSON schema method implementation for structs.
///
/// When the struct has `#[serde(flatten)]` fields, the base properties are
/// distributed into each branch of the flattened types' schemas (cross-product
/// over any `oneOf`), producing a strict closed schema that validates the base
/// fields and the flattened union together.
pub fn generate_struct_json_schema_method(
    json_schema_fields: &[proc_macro2::TokenStream],
    flatten_json_schemas: &[MergedSource],
    def_name: &str,
) -> proc_macro2::TokenStream {
    let body = if flatten_json_schemas.is_empty() {
        closed_object_body(json_schema_fields)
    } else {
        flattened_object_body(json_schema_fields, flatten_json_schemas, def_name)
    };
    json_schema_methods(def_name, &body)
}

/// The struct's own fields as one closed object.
fn closed_object_body(json_schema_fields: &[proc_macro2::TokenStream]) -> proc_macro2::TokenStream {
    quote::quote! {
        {
            let mut schema_obj = serde_json::Map::new();
            schema_obj.insert("type".to_string(), serde_json::Value::String("object".to_string()));
            schema_obj.insert("additionalProperties".to_string(), serde_json::Value::Bool(false));
            let mut properties = serde_json::Map::new();
            let mut required = Vec::new();

            #(#json_schema_fields)*

            schema_obj.insert(
                "properties".to_string(),
                serde_json::Value::Object(properties),
            );

            schema_obj.insert("required".to_string(), serde_json::Value::Array(required));

            serde_json::Value::Object(schema_obj)
        }
    }
}

/// The four things the merge asks of a schema it is handed, as the tokens the merging block opens
/// with: how two objects join, whether a schema is a deferred name, what type it commits its value
/// to, and which branches it offers under which spelling.
fn merge_readers() -> proc_macro2::TokenStream {
    let defs_prefix = DEFS_PREFIX;
    quote::quote! {
        fn merge_object_schemas(
            a: &serde_json::Map<String, serde_json::Value>,
            b: &serde_json::Map<String, serde_json::Value>,
        ) -> serde_json::Map<String, serde_json::Value> {
            let mut out = serde_json::Map::new();
            out.insert("type".to_string(), serde_json::Value::String("object".to_string()));
            let mut properties = serde_json::Map::new();
            for src in [a, b] {
                if let Some(p) = src.get("properties").and_then(serde_json::Value::as_object) {
                    for (k, v) in p {
                        properties.insert(k.clone(), v.clone());
                    }
                }
            }
            out.insert("properties".to_string(), serde_json::Value::Object(properties));
            let mut required: Vec<serde_json::Value> = Vec::new();
            for src in [a, b] {
                if let Some(r) = src.get("required").and_then(serde_json::Value::as_array) {
                    for item in r {
                        if !required.contains(item) {
                            required.push(item.clone());
                        }
                    }
                }
            }
            out.insert("required".to_string(), serde_json::Value::Array(required));
            out.insert("additionalProperties".to_string(), serde_json::Value::Bool(false));
            out
        }

        // A merged schema that names itself describes as a reference into the definitions being
        // hoisted, and a reference is the one thing with no properties to merge. The body it
        // points at is written by then — the frame that deferred the name fills the entry in
        // before it returns — so the merge reads it back. A union member defers the same way the
        // whole merged body does, so both ends read it through here.
        fn deferred_name(schema: &serde_json::Value) -> Option<&str> {
            schema.get("$ref")?.as_str()?.strip_prefix(#defs_prefix)
        }

        // What a description commits its value to on the wire, when it commits to anything. A
        // union of branches and a bare reference name no type of their own, and neither is
        // provably not an object, so both are left to the merge.
        fn described_type(schema: &serde_json::Value) -> Option<&str> {
            schema.get("type")?.as_str()
        }

        // What serde picked one of, and how the source spelled the choice. A discriminated enum
        // writes `oneOf` and an untagged one `anyOf`; the merge owes both the same branches — the
        // value that reached it wrote whichever branch matched, so the branch is what the base
        // joins — and owes each its own spelling, which is what says whether a payload two branches
        // admit is an error or the ordinary case.
        //
        // A schema that offers no choice answers with none, which is what tells the expansion it has
        // reached something the base can merge rather than descend into.
        fn union_branches(schema: &serde_json::Value) -> Option<(&'static str, &[serde_json::Value])> {
            for keyword in ["oneOf", "anyOf"] {
                if let Some(union) = schema.get(keyword).and_then(serde_json::Value::as_array) {
                    return Some((keyword, union.as_slice()));
                }
            }
            None
        }
    }
}

/// What the merge holds while it multiplies, as the tokens that declare it.
///
/// The spelling travels with the source that used it, because the two spellings say different
/// things about a payload more than one branch admits. A discriminated enum's members are
/// exclusive, so `oneOf` costs it nothing; an untagged enum is first-match-wins, and members whose
/// key sets overlap admit each other's payloads as a matter of course — under `oneOf` the document
/// would reject exactly what serde writes for the narrower member. So the branches a source
/// multiplies out are held under that source's own wrapper, and a second source multiplies each of
/// them again from there: the wrappers nest rather than flatten into one, and each branch set keeps
/// the rule its own union was written under.
///
/// A source is a tree rather than a list for the same reason: a branch that is itself a union was
/// written under a spelling of its own, and grafting its leaves onto the source's wrapper would
/// answer for them under a rule they were not written under.
fn merged_tree() -> proc_macro2::TokenStream {
    quote::quote! {
        enum Branches<'defs> {
            // What a source contributes when it is not there — reached through an `Option`, or
            // naming an item whose own published surface offers a `null` beside its value: no
            // members, so the branch names exactly the keys the object writes on its own.
            Absent,
            Object(&'defs serde_json::Map<String, serde_json::Value>),
            Union(&'static str, Vec<Branches<'defs>>),
        }

        enum Merged {
            Object(serde_json::Map<String, serde_json::Value>),
            Union(&'static str, Vec<Merged>),
        }

        impl Branches<'_> {
            // What one base becomes once this source's choices are written into it: every leaf of
            // the source contributes its members to a copy of the base, under the wrapper the level
            // that offered it was written with.
            fn merged_into(&self, base: &serde_json::Map<String, serde_json::Value>) -> Merged {
                match *self {
                    // Merged rather than copied: an absent source contributes no members, and the
                    // branch is still a branch of a document whose others were written by the
                    // merge — the same keys in the same order, holding what the base already held.
                    Self::Absent => {
                        Merged::Object(merge_object_schemas(base, &serde_json::Map::new()))
                    }
                    Self::Object(members) => Merged::Object(merge_object_schemas(base, members)),
                    Self::Union(spelling, ref branches) => {
                        let mut merged: Vec<Merged> = branches
                            .iter()
                            .map(|branch| branch.merged_into(base))
                            .collect();
                        // One key set is an object rather than a choice between objects, so a level
                        // offering a single branch writes no wrapper and its spelling goes unread.
                        if merged.len() == 1 {
                            merged.swap_remove(0)
                        } else {
                            Merged::Union(spelling, merged)
                        }
                    }
                }
            }

            // The same source with its own absence offered beside it — what an `Option` makes of
            // whatever it wraps. One object cannot say that a group of keys is written together or
            // not at all: required of every payload rejects the absent one, and required of none
            // admits a base written in part, which is a payload the source never writes. A choice
            // between two key sets is exactly what the two forms are, so it is written as one.
            //
            // `anyOf` is the rule that choice was written under. A source whose own members are all
            // optional writes nothing for some of its values, and that payload is the one its
            // absence writes too — two branches admitting it is the ordinary case rather than the
            // ambiguity `oneOf` would call it.
            fn or_absent(self) -> Self {
                Self::Union("anyOf", vec![self, Self::Absent])
            }
        }

        impl Merged {
            // Every leaf gains the members of one leaf of the source, so a source reaches the
            // branches an earlier source left behind rather than only the object it started from.
            fn multiplied(self, source: &Branches<'_>) -> Self {
                match self {
                    Self::Union(keyword, branches) => Self::Union(
                        keyword,
                        branches
                            .into_iter()
                            .map(|branch| branch.multiplied(source))
                            .collect(),
                    ),
                    Self::Object(base) => source.merged_into(&base),
                }
            }

            fn into_value(self) -> serde_json::Value {
                match self {
                    Self::Object(members) => serde_json::Value::Object(members),
                    Self::Union(keyword, branches) => {
                        let mut out = serde_json::Map::new();
                        out.insert(
                            "type".to_string(),
                            serde_json::Value::String("object".to_string()),
                        );
                        out.insert(
                            keyword.to_string(),
                            serde_json::Value::Array(
                                branches.into_iter().map(Self::into_value).collect(),
                            ),
                        );
                        serde_json::Value::Object(out)
                    }
                }
            }
        }
    }
}

/// How the expansion refuses a schema it cannot merge, as the tokens that declare the refusals.
///
/// The whole merged body and a branch any depth down are the same two failures, so one expansion
/// answers for both and each refusal carries both wordings: an empty position is the body itself,
/// and any other names the branch it was reached through. A branch's position is the trail of
/// one-based choices taken to reach it, so a member of a nested union is named `1.2` rather than
/// twice as `2`.
fn expansion_refusals(diagnostic: &MergeDiagnostic<'_>) -> proc_macro2::TokenStream {
    let MergeDiagnostic {
        cycle_remedy,
        edge,
        non_object_remedy,
        subject,
    } = *diagnostic;
    quote::quote! {
        fn branch_path(position: &[usize]) -> String {
            position
                .iter()
                .map(usize::to_string)
                .collect::<Vec<String>>()
                .join(".")
        }

        // An entry still only reserved is this merge coming back around to a name whose body is
        // still being written: there is nothing to merge, and the type it would describe has no
        // finite value to inhabit it.
        fn refuse_missing_body(label: &str, position: &[usize], name: &str) -> ! {
            if position.is_empty() {
                panic!(
                    "`{}`: {} `{}` closes a flatten cycle — the flattened body does not exist to merge, and no finite value inhabits the type; {}",
                    #subject, #edge, name, #cycle_remedy,
                );
            }
            panic!(
                "`{}`: {} `{}` closes a flatten cycle through a union member — its branch {} is `{}`, whose body does not exist to merge, and no finite value inhabits the type; {}",
                #subject, #edge, label, branch_path(position), name, #cycle_remedy,
            );
        }

        // A name whose body does exist but stands on the path the expansion is already descending:
        // the same cycle, closed through unions rather than through the value itself.
        fn refuse_repeated_name(
            label: &str,
            position: &[usize],
            name: &str,
            expanding: &[&str],
        ) -> ! {
            let path = expanding
                .iter()
                .map(|resolved| format!("`{resolved}`"))
                .collect::<Vec<String>>()
                .join(" → ");
            panic!(
                "`{}`: {} `{}` closes a flatten cycle through nested unions — its branch {} names `{}`, already expanding on the path {}, and no finite value inhabits the type; {}",
                #subject, #edge, label, branch_path(position), name, path, #cycle_remedy,
            );
        }

        fn refuse_non_object(label: &str, position: &[usize], named: &str) -> ! {
            if position.is_empty() {
                panic!(
                    "`{}`: {} `{}` is not written as an object — its schema describes a `{}`, which has no members to merge, and what serde writes for it does not join the object being written; {}",
                    #subject, #edge, label, named, #non_object_remedy,
                );
            }
            panic!(
                "`{}`: {} `{}` writes a union member that is not an object — its branch {} describes a `{}`, which has no members to merge, and what serde writes for that member does not join the object being written; {}",
                #subject, #edge, label, branch_path(position), named, #non_object_remedy,
            );
        }
    }
}

/// The branch tree one merged schema contributes, as the tokens that declare how it is read.
///
/// A union names no type of its own, so the branch is where the questions the whole merged body was
/// asked are asked again — and serde cannot write a branch that is not an object into the object
/// being written any more than it could write the whole value that way. A branch that is a deferred
/// name carries none of the members it stands for, so it is read back out of the definitions first,
/// the same way the whole merged body is. A branch that is itself a union names no members either,
/// so the questions are asked again below it, and again, until every leaf is an object the base can
/// merge or a refusal.
///
/// Two things bound the descent. Plain nesting reaches a strictly smaller part of a finite document,
/// so it ends on its own; a deferred name does not, because the body it resolves to may name it
/// back. So the names resolved on the way down are carried, and a name reached twice on one path is
/// a cycle by construction.
fn branch_expansion() -> proc_macro2::TokenStream {
    quote::quote! {
        fn expanded_branches<'defs>(
            schema: &'defs serde_json::Value,
            hoisted_defs: &'defs serde_json::Map<String, serde_json::Value>,
            expanding: &mut Vec<&'defs str>,
            position: &mut Vec<usize>,
            label: &str,
        ) -> Option<Branches<'defs>> {
            let mut resolved = None;
            let body = match deferred_name(schema) {
                None => schema,
                Some(name) => {
                    let Some(named_body) = hoisted_defs.get(name).filter(|body| body.is_object())
                    else {
                        refuse_missing_body(label, position, name);
                    };
                    // The path holds only names already descended through, so the first frame never
                    // finds itself on it and this refusal always has a branch to name.
                    if expanding.contains(&name) {
                        refuse_repeated_name(label, position, name, expanding);
                    }
                    resolved = Some(name);
                    named_body
                }
            };

            if let Some(named) = described_type(body) {
                // A `null` among the choices the flatten edge itself offers is the absence rather
                // than a refusal: the source is nullable, serde writes no member of it for that
                // value, and the payload carrying none of them is the one serde reads back as that
                // value — the same two key sets a source reached through an `Option` writes. A
                // `null` below that level is a member of a choice serde matched by shape, where the
                // absent form is one serde writes and then matches no member for, and the refusal
                // stands.
                if named == "null" && position.len() == 1 {
                    return Some(Branches::Absent);
                }
                if named != "object" {
                    refuse_non_object(label, position, named);
                }
            }

            let Some((spelling, branches)) = union_branches(body) else {
                return body.as_object().map(Branches::Object);
            };

            // The name guards what is below it and nothing else, so it joins the path only for the
            // descent and leaves it before the level that resolved it answers.
            if let Some(name) = resolved {
                expanding.push(name);
            }
            let mut expanded: Vec<Branches<'defs>> = Vec::new();
            for (index, branch) in branches.iter().enumerate() {
                position.push(index + 1);
                let below = expanded_branches(branch, hoisted_defs, expanding, position, label);
                position.pop();
                expanded.extend(below);
            }
            if resolved.is_some() {
                expanding.pop();
            }
            (!expanded.is_empty()).then_some(Branches::Union(spelling, expanded))
        }
    }
}

/// A base object's members with the members of every schema merged beside them.
///
/// serde writes a `#[serde(flatten)]` base and an internally tagged variant's newtype content the
/// same way — what is merged contributes its members to the object the base is writing — so both
/// describe through this one merge rather than each spelling its own. `base` is a
/// `serde_json::Map` expression and every entry of `merged` a `serde_json::Value` one; the result
/// is a `serde_json::Value`.
///
/// A merged schema that is itself a union multiplies out rather than collapsing, so each branch
/// stays a closed object naming exactly the members that branch writes. Both spellings of a union
/// are read: a discriminated enum's `oneOf` and an untagged one's `anyOf` alike name what serde
/// picked one of, and the merged schema is the union of the merges. A source reached through an
/// `Option` offers its own absence beside whatever it described as, the two key sets being what the
/// field actually writes.
///
/// Only a value serde writes as an object has members to contribute, and the expansion cannot
/// always tell which types those are — a name reaches the merge without saying what it writes. The
/// schema it produces does say, so the merge reads it there: a description naming any type but
/// `object` is refused rather than merged, which is the last point at which the wrong schema can
/// still be stopped. The one exception is the `null` of a choice the edge itself offers, which is
/// the source's own absence written out and is read as that. A flatten edge that closes a cycle is
/// refused on the same terms, named for the frame that read it.
///
/// The reading itself is the tokens [`merge_readers`] emits, and what the multiplication is carried
/// in is [`merged_tree`]; both are written into the block this returns, so the whole merge is one
/// expression the caller can place wherever a `serde_json::Value` is wanted.
pub fn merged_object_value(
    base: &proc_macro2::TokenStream,
    merged: &[MergedSource],
    diagnostic: &MergeDiagnostic<'_>,
) -> proc_macro2::TokenStream {
    let refusals = expansion_refusals(diagnostic);
    let expansion = branch_expansion();
    let labels = merged.iter().map(|source| source.label.as_str());
    let optionals = merged.iter().map(|source| source.optional);
    let values = merged.iter().map(|source| &source.value);
    let readers = merge_readers();
    let tree = merged_tree();
    quote::quote! {
        {
            #tree
            #readers
            #refusals
            #expansion

            let flattened: Vec<(&'static str, bool, serde_json::Value)> =
                vec![ #((#labels, #optionals, #values)),* ];

            let mut described = Merged::Object(#base);
            for (label, optional, fs) in &flattened {
                let mut expanding: Vec<&str> = Vec::new();
                let mut position: Vec<usize> = Vec::new();
                if let Some(source) =
                    expanded_branches(fs, hoisted_defs, &mut expanding, &mut position, label)
                {
                    // The absence is offered around whatever the source described as, so a union
                    // reached through an `Option` keeps its own spelling and gains the choice
                    // outside it rather than one more member inside it.
                    let offered = if *optional { source.or_absent() } else { source };
                    described = described.multiplied(&offered);
                }
            }

            described.into_value()
        }
    }
}

/// The struct's own fields distributed into each branch of the flattened types' schemas.
///
/// A flatten edge that closes a cycle is rejected where it is read rather than merged: `def_name`
/// is the frame that reads it, and names one end of the closing edge in the diagnostic.
fn flattened_object_body(
    json_schema_fields: &[proc_macro2::TokenStream],
    flatten_json_schemas: &[MergedSource],
    def_name: &str,
) -> proc_macro2::TokenStream {
    let base = quote::quote! {
        {
            let mut schema_obj = serde_json::Map::new();
            schema_obj.insert("type".to_string(), serde_json::Value::String("object".to_string()));
            schema_obj.insert("additionalProperties".to_string(), serde_json::Value::Bool(false));
            let mut properties = serde_json::Map::new();
            let mut required = Vec::new();

            #(#json_schema_fields)*

            schema_obj.insert("properties".to_string(), serde_json::Value::Object(properties));
            schema_obj.insert("required".to_string(), serde_json::Value::Array(required));
            schema_obj
        }
    };
    merged_object_value(
        &base,
        flatten_json_schemas,
        &MergeDiagnostic {
            cycle_remedy: "write the field as a named member so the cycle defers through a reference",
            edge: "`#[serde(flatten)]` of",
            non_object_remedy: "write the field as a named member so the value gets a key of its own",
            subject: def_name,
        },
    )
}

/// Generates the JSON schema method implementation for plain enums.
pub fn generate_plain_enum_json_schema_method(
    enumerated: &[proc_macro2::TokenStream],
    def_name: &str,
) -> proc_macro2::TokenStream {
    let body = quote::quote! {
        {
            let mut schema_obj = serde_json::Map::new();
            schema_obj.insert("type".to_string(), serde_json::Value::String("string".to_string()));
            schema_obj.insert("enum".to_string(), serde_json::Value::Array(
                [#(#enumerated),*].into_iter().map(|v: &str| serde_json::Value::String(v.to_string())).collect()
            ));

            serde_json::Value::Object(schema_obj)
        }
    };
    json_schema_methods(def_name, &body)
}

#[cfg(test)]
mod tests;
