mod features;
mod field_type;
mod generation;
mod model_schema;
mod rename_rule;
mod utils;

use model_schema::exec_model_schema;
use proc_macro::TokenStream;

/// # `model_schema`
///
/// A macro that generates TypeScript type definitions and Zod validation schemas for Rust structs and enums.
///
/// This macro adds a `ts_definition()` method to the annotated type that returns TypeScript type definitions
/// and Zod schemas as strings. It's particularly useful for maintaining consistent data structures
/// between your Rust backend and TypeScript/JavaScript frontend.
///
/// ## Features
///
/// - Generates TypeScript interfaces/types that mirror your Rust structs and enums
/// - Creates Zod validation schemas for runtime validation in JavaScript
/// - Respects Serde attributes like `rename` and `rename_all`
/// - Provides proper type mappings between Rust and TypeScript
/// - Handles nested types, generics, optional fields, and collections
/// - First-class `MongoDB` `ObjectId` support with proper serialization and validation
/// - Supports complex nested structures including deeply nested `HashMaps`
///
/// ## Usage
///
/// ```rust
/// use tixschema::model_schema;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize)]
/// #[serde(rename_all = "camelCase")]
/// #[model_schema()]
/// pub struct User {
///     pub id: String,
///     pub first_name: String,
///     pub last_name: String,
///     #[serde(skip_serializing_if = "Option::is_none")]
///     pub age: Option<u32>,
///     pub roles: Vec<String>,
/// }
///
/// // This will generate a ts_definition() method that returns:
/// //
/// // export type User = {
/// //   id: string,
/// //   firstName: string,
/// //   lastName: string,
/// //   age: number | undefined,
/// //   roles: Array<string>,
/// // };
/// //
/// // export const User$Schema: ZodType<User> = z.strictObject({
/// //   id: z.string(),
/// //   firstName: z.string(),
/// //   lastName: z.string(),
/// //   age: z.union([z.number(), z.undefined()]).prefault(undefined),
/// //   roles: z.array(z.string()),
/// // });
/// ```
///
/// ## Enum Support
///
/// ```rust
/// use tixschema::model_schema;
/// use serde;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize)]
/// #[serde(rename_all = "lowercase")]
/// #[model_schema()]
/// pub enum Status {
///     Active,
///     Pending,
///     Inactive,
/// }
///
/// // Generates:
/// // export type Status = "active" | "pending" | "inactive";
/// // export const Status$Schema: ZodType<Status> = z.enum(["active", "pending", "inactive"]);
/// ```
///
/// ## Tagged Unions (Discriminated Unions)
///
/// ```rust
/// use tixschema::model_schema;
/// use serde;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize)]
/// #[serde(tag = "type", rename_all = "camelCase")]
/// #[model_schema()]
/// pub enum Event {
///     UserCreated {
///         user_id: String,
///         timestamp: String,
///     },
///     UserDeleted {
///         user_id: String,
///         #[serde(skip_serializing_if = "Option::is_none")]
///         reason: Option<String>,
///     }
/// }
///
/// // Generates a discriminated union in TypeScript:
/// // export type Event = {
/// //   type: "userCreated";
/// //   userId: string;
/// //   timestamp: string;
/// // } | {
/// //   type: "userDeleted";
/// //   userId: string;
/// //   reason: string | undefined;
/// // };
/// ```
///
/// ## `MongoDB` `ObjectId` Support
///
/// When the `object_id` feature is enabled, the macro provides first-class support for `MongoDB` `ObjectId` types:
///
#[cfg_attr(
    feature = "object_id",
    doc = r#"
```rust
use tixschema::model_schema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Dummy ObjectId for doc test (in real usage, use mongodb::bson::oid::ObjectId)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ObjectId(String);

#[derive(Serialize, Deserialize)]
#[model_schema()]
pub struct Document {
    pub id: ObjectId,
    pub title: String,
    pub author_id: ObjectId,
    pub tags: Vec<ObjectId>,
    pub metadata: HashMap<String, ObjectId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<ObjectId>,
}

// Generates:
// export type Document = {
//   id: ObjectId;
//   title: string;
//   author_id: ObjectId;
//   tags: Array<ObjectId>;
//   metadata: Partial<Record<string, ObjectId>>;
//   parent_id: ObjectId | undefined;
// };
//
// export const Document$Schema = z.strictObject({
//   id: z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/i, { message: "Invalid ObjectId" }) }),
//   title: z.string(),
//   author_id: z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/i, { message: "Invalid ObjectId" }) }),
//   tags: z.array(z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/i, { message: "Invalid ObjectId" }) })),
//   metadata: z.record(z.string(), z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/i, { message: "Invalid ObjectId" }) })),
//   parent_id: z.union([z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/i, { message: "Invalid ObjectId" }) }), z.undefined()]).prefault(undefined),
// });
```
"#
)]
///
///
/// `ObjectId` fields are serialized using `MongoDB`'s standard format: `{ "$oid": "hex_string" }`
/// and include proper validation for 24-character hexadecimal `ObjectId` strings.
///
/// ## Where a Referenced Type Must Be Declared
///
/// A `#[model_schema()]` type publishes its schema in a module beside the type, and a type that
/// references it reaches that module through `use super::*`. **A type referenced by another must
/// therefore be declared at item scope** — in a module or at crate level, not inside a function
/// body, which no generated module is a child of. The referencing type itself is unconstrained: it
/// may be declared inside a function body as long as everything it names is not.
///
/// A reference to a function-local type fails to compile with an `E0433` naming the module the
/// macro built from that type's name, reported at the field's type:
///
/// ```text
/// error[E0433]: cannot find module or crate `inner_schema` in this scope
///   --> src/lib.rs:9:20
///    |
///  9 |     pub inner: Inner,
///    |                ^^^^^ use of unresolved module or unlinked crate `inner_schema`
/// ```
///
/// Moving the named type out of the function body — to the module the referencing type is written
/// in, or any module in scope there — is what resolves it.
///
/// ## Branded Newtypes and `no_display`
///
/// A `#[serde(transparent)]` single-field tuple struct becomes a branded type, and the macro gives
/// it a `Display` impl that delegates to the inner value. **The inner type must implement
/// `Display`.** An inner type that does not is reported at the inner field, naming the trait.
///
/// Pass `no_display` for brands whose inner type is a container (or anything else without
/// `Display`): the brand then gets no `Display` impl and no such requirement, while the generated
/// schema is unchanged. String constraints are a separate matter -- `pattern`, `minLength`, and
/// `maxLength` validate through `to_string()`, so a constrained brand needs a `Display` inner
/// whether or not it passes `no_display`.
///
/// ```rust
/// use tixschema::model_schema;
/// use serde::{Deserialize, Serialize};
///
/// #[model_schema()]
/// #[derive(Serialize, Deserialize)]
/// #[serde(transparent)]
/// pub struct UserId(pub String);
///
/// // `Vec<String>` has no `Display`, so this brand opts out of the impl.
/// #[model_schema(no_display)]
/// #[derive(Serialize, Deserialize)]
/// #[serde(transparent)]
/// pub struct Tags(pub Vec<String>);
/// ```
///
/// A generic brand carries the requirement as a `Display` bound on each type parameter, so a
/// non-`Display` type argument is rejected where the brand is used, not where it is declared.
///
#[proc_macro_attribute]
pub fn model_schema(args: TokenStream, input: TokenStream) -> TokenStream {
    exec_model_schema(args, input)
}

/// # `model_schema_prop`
///
/// Field-level attribute for customizing schema generation. Apply to fields inside
/// a struct or enum variant marked with `#[model_schema()]`.
///
/// ## Validation Constraints
///
/// These generate matching validation in Zod, JSON Schema, and Rust serde deserialization.
///
/// ### String Constraints
///
/// - `minLength = N` — minimum string length
/// - `maxLength = N` — maximum string length
/// - `pattern = "regex"` — regex the value must match
///
/// ```rust
/// use tixschema::{model_schema, model_schema_prop};
/// use serde::{Deserialize, Serialize};
///
/// #[model_schema()]
/// #[derive(Serialize, Deserialize)]
/// pub struct UserJson {
///     #[model_schema_prop(minLength = 3, maxLength = 50, pattern = "^[a-z0-9_]+$")]
///     pub username: String,
///
///     #[model_schema_prop(maxLength = 200)]
///     pub bio: String,
/// }
/// ```
///
/// Generated Zod: `z.string().min(3).max(50).check(z.regex(/^[a-z0-9_]+$/))`.
///
/// Generated JSON Schema: `{ "type": "string", "minLength": 3, "maxLength": 50, "pattern": "^[a-z0-9_]+$" }`.
///
/// One `pattern` string reaches three engines — the Rust validator's `regex::Regex`, the Zod
/// schema's JavaScript regex literal, and the JSON Schema `pattern` keyword, which ECMA-262
/// defines as a JavaScript regex — so it has to be a regex all three read the same way. A pattern
/// only the `regex` crate reads is refused at expansion, with the construct named: Unicode classes
/// (`\p{L}`), POSIX classes (`[[:alpha:]]`), class set operators (`&&`, `--`, `~~`), nested
/// classes, `\A` and `\z`, `\b{start}`, `\<` and `\>`, braced code point escapes (`\x{41}`), `\a`,
/// and inline flag directives (`(?i)`). `(?P<name>...)` is accepted and emitted as the
/// `(?<name>...)` both grammars read.
///
/// A `PathBuf` field carries these too, as does the `Path` borrow behind a wrapper: serde writes a
/// path as a JSON string, and the checks measure that string — the path's `to_string_lossy`
/// rendering, which is the exact wire value for every path serde can write.
///
/// ### Numeric Constraints
///
/// - `minimum = N` — minimum value (integers and floats)
/// - `maximum = N` — maximum value
///
/// ```rust
/// use tixschema::{model_schema, model_schema_prop};
/// use serde::{Deserialize, Serialize};
///
/// #[model_schema()]
/// #[derive(Serialize, Deserialize)]
/// pub struct ProductJson {
///     #[model_schema_prop(minimum = 0, maximum = 120)]
///     pub age_restriction: u32,
///
///     #[model_schema_prop(minimum = 0.0)]
///     pub price: f64,
/// }
/// ```
///
/// ### The `validate()` Method
///
/// When any field has constraints, the macro also generates a `validate(&self) -> Result<(), Vec<String>>`
/// method for validating instances constructed in code. Serde deserialization validates automatically.
///
/// ```rust
/// use tixschema::{model_schema, model_schema_prop};
/// use serde::{Deserialize, Serialize};
///
/// #[model_schema()]
/// #[derive(Serialize, Deserialize)]
/// pub struct RegistrationJson {
///     #[model_schema_prop(minLength = 3, maxLength = 30)]
///     pub username: String,
///
///     #[model_schema_prop(minimum = 0, maximum = 120)]
///     pub age: u32,
/// }
/// ```
///
/// When a schema output feature is active (`zod`, `typescript`, or `jsonschema`), the macro also
/// generates a `validate(&self) -> Result<(), Vec<String>>` method:
///
/// ```text
/// let reg = RegistrationJson { username: "ab".to_string(), age: 150 };
/// match reg.validate() {
///     Ok(()) => println!("valid"),
///     Err(errors) => {
///         for e in &errors {
///             println!("Error: {e}");
///         }
///         // "username: too short (minimum length 3, got 2)"
///         // "age: too large (maximum 120, got 150)"
///     }
/// }
/// ```
///
/// ## Type Overrides
///
/// - `as = Type` — name the type this field renders. The target must be the field's own type or
///   the value under its wrappers; naming any other type is a compile error, since every surface
///   is written from the declared type and the expansion has no second reading of the wire.
/// - `literal = "value"` — emit as a string literal type in TypeScript and Zod
///
/// ```rust
/// use tixschema::{model_schema, model_schema_prop};
/// use serde::{Deserialize, Serialize};
///
/// #[model_schema()]
/// #[derive(Serialize, Deserialize)]
/// pub struct ApiConfigJson {
///     #[model_schema_prop(as = String)]
///     pub metric: String,
///
///     pub enabled: bool,
/// }
/// ```
///
/// ## Zod Preprocessing
///
/// - `preprocess = ["fn1", "fn2"]` — wrap the Zod schema with `z.preprocess()` calls
///   (Zod-only, no effect on Rust types or serde deserialization)
///
/// Multiple preprocessors are applied as nested calls, innermost first:
///
/// ```rust
/// use tixschema::{model_schema, model_schema_prop};
/// use serde::{Deserialize, Serialize};
///
/// #[model_schema()]
/// #[derive(Serialize, Deserialize)]
/// pub struct EventJson {
///     // → z.preprocess(trim, z.preprocess(normalize, z.string()))
///     #[model_schema_prop(preprocess = ["trim", "normalize"])]
///     pub name: String,
/// }
/// ```
#[proc_macro_attribute]
pub fn model_schema_prop(_args: TokenStream, input: TokenStream) -> TokenStream {
    // For now, simply pass through the input
    input
}
