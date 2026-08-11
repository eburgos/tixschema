mod features;
mod field_type;
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
///     #[serde(skip_serializing_if = "Option::is_none")]
///     pub age: Option<u32>,
///     pub first_name: String,
///     pub id: String,
///     pub last_name: String,
///     pub roles: Vec<String>,
/// }
/// ```
///
/// Each output block below is a run of the declaration above it under the default features, pasted
/// verbatim. The two methods answer separately -- `ts_definition()` returns the TypeScript alone,
/// `zod_schema()` the Zod alone -- and the JSON Schema carried by each leading `JSDoc` block is
/// written only while the `jsonschema` feature is on.
///
/// `User::ts_definition()`:
///
#[doc = r#"```typescript
/**
 * User
 * 
 * JSON Schema:
 * {
 *   "type": "object",
 *   "additionalProperties": false,
 *   "properties": {
 *     "age": {
 *       "anyOf": [
 *         {
 *           "type": "integer"
 *         },
 *         {
 *           "type": "null"
 *         }
 *       ]
 *     },
 *     "firstName": {
 *       "type": "string"
 *     },
 *     "id": {
 *       "type": "string"
 *     },
 *     "lastName": {
 *       "type": "string"
 *     },
 *     "roles": {
 *       "type": "array",
 *       "items": {
 *         "type": "string"
 *       }
 *     }
 *   },
 *   "required": [
 *     "firstName",
 *     "id",
 *     "lastName",
 *     "roles"
 *   ]
 * }
 */
export type User = {
  /**
   * age
   * 
   */
  age: number | undefined;
  /**
   * firstName
   * 
   */
  firstName: string;
  /**
   * id
   * 
   */
  id: string;
  /**
   * lastName
   * 
   */
  lastName: string;
  /**
   * roles
   * 
   */
  roles: Array<string>;
};
```"#]
///
/// `User::zod_schema()`:
///
#[doc = "```typescript
const User$RawSchema = z.strictObject({
  age: z.union([z.null().transform(() => undefined), z.number().int(), z.undefined()]).prefault(undefined),
  firstName: z.string(),
  id: z.string(),
  lastName: z.string(),
  roles: z.array(z.string()),
});

export const User$Schema: ZodType<User> = User$RawSchema;
```"]
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
///     Inactive,
///     Pending,
/// }
/// ```
///
/// `Status::ts_definition()`:
///
#[doc = r#"```typescript
/**
 * Status
 * 
 * JSON Schema:
 * {
 *   "type": "string",
 *   "enum": [
 *     "active",
 *     "inactive",
 *     "pending"
 *   ]
 * }
 */
export type Status =
  | "active"
  | "inactive"
  | "pending";
```"#]
///
/// `Status::zod_schema()`:
///
#[doc = r#"```typescript
const Status$RawSchema = z.enum(["active", "inactive", "pending"]).meta({
  description: "Status",
});

export const Status$Schema: ZodType<Status> = Status$RawSchema;
```"#]
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
///         timestamp: String,
///         user_id: String,
///     },
///     UserDeleted {
///         #[serde(skip_serializing_if = "Option::is_none")]
///         reason: Option<String>,
///         user_id: String,
///     },
/// }
/// ```
///
/// `Event::ts_definition()`, a discriminated union:
///
#[doc = r#"```typescript
/**
 * Event
 * 
 * JSON Schema:
 * {
 *   "type": "object",
 *   "oneOf": [
 *     {
 *       "additionalProperties": false,
 *       "properties": {
 *         "type": {
 *           "type": "string",
 *           "const": "userCreated"
 *         },
 *         "timestamp": {
 *           "type": "string"
 *         },
 *         "user_id": {
 *           "type": "string"
 *         }
 *       },
 *       "required": [
 *         "type",
 *         "timestamp",
 *         "user_id"
 *       ]
 *     },
 *     {
 *       "additionalProperties": false,
 *       "properties": {
 *         "type": {
 *           "type": "string",
 *           "const": "userDeleted"
 *         },
 *         "reason": {
 *           "anyOf": [
 *             {
 *               "type": "string"
 *             },
 *             {
 *               "type": "null"
 *             }
 *           ]
 *         },
 *         "user_id": {
 *           "type": "string"
 *         }
 *       },
 *       "required": [
 *         "type",
 *         "user_id"
 *       ]
 *     }
 *   ]
 * }
 */
export type Event = {
  /**
   * userCreated
   * 
   */
  type: "userCreated";
  /**
   * timestamp
   * 
   */
  timestamp: string;
  /**
   * user_id
   * 
   */
  user_id: string;
} | {
  /**
   * userDeleted
   * 
   */
  type: "userDeleted";
  /**
   * reason
   * 
   */
  reason: string | undefined;
  /**
   * user_id
   * 
   */
  user_id: string;
};
```"#]
///
/// `Event::zod_schema()`:
///
#[doc = r#"```typescript
const Event$RawSchema = z.discriminatedUnion("type", [z.strictObject({
  type: z.literal("userCreated"),
  timestamp: z.string(),
  user_id: z.string(),
}), z.strictObject({
  type: z.literal("userDeleted"),
  reason: z.union([z.null().transform(() => undefined), z.string(), z.undefined()]).prefault(undefined),
  user_id: z.string(),
})]);

export const Event$Schema: ZodType<Event> = Event$RawSchema;
```"#]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectId(pub String);

#[derive(Serialize, Deserialize)]
#[model_schema()]
pub struct Document {
    pub author_id: ObjectId,
    pub id: ObjectId,
    pub metadata: HashMap<String, ObjectId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<ObjectId>,
    pub tags: Vec<ObjectId>,
    pub title: String,
}
```

`Document::ts_definition()`:

```typescript
/**
 * Document
 * 
 * JSON Schema:
 * {
 *   "type": "object",
 *   "additionalProperties": false,
 *   "properties": {
 *     "author_id": {
 *       "type": "object",
 *       "properties": {
 *         "$oid": {
 *           "type": "string",
 *           "pattern": "^[a-f0-9]{24}$"
 *         }
 *       },
 *       "required": [
 *         "$oid"
 *       ],
 *       "additionalProperties": false
 *     },
 *     "id": {
 *       "type": "object",
 *       "properties": {
 *         "$oid": {
 *           "type": "string",
 *           "pattern": "^[a-f0-9]{24}$"
 *         }
 *       },
 *       "required": [
 *         "$oid"
 *       ],
 *       "additionalProperties": false
 *     },
 *     "metadata": {
 *       "type": "object",
 *       "additionalProperties": {
 *         "type": "object",
 *         "properties": {
 *           "$oid": {
 *             "type": "string",
 *             "pattern": "^[a-f0-9]{24}$"
 *           }
 *         },
 *         "required": [
 *           "$oid"
 *         ],
 *         "additionalProperties": false
 *       }
 *     },
 *     "parent_id": {
 *       "anyOf": [
 *         {
 *           "type": "object",
 *           "properties": {
 *             "$oid": {
 *               "type": "string",
 *               "pattern": "^[a-f0-9]{24}$"
 *             }
 *           },
 *           "required": [
 *             "$oid"
 *           ],
 *           "additionalProperties": false
 *         },
 *         {
 *           "type": "null"
 *         }
 *       ]
 *     },
 *     "tags": {
 *       "type": "array",
 *       "items": {
 *         "type": "object",
 *         "properties": {
 *           "$oid": {
 *             "type": "string",
 *             "pattern": "^[a-f0-9]{24}$"
 *           }
 *         },
 *         "required": [
 *           "$oid"
 *         ],
 *         "additionalProperties": false
 *       }
 *     },
 *     "title": {
 *       "type": "string"
 *     }
 *   },
 *   "required": [
 *     "author_id",
 *     "id",
 *     "metadata",
 *     "tags",
 *     "title"
 *   ]
 * }
 */
export type Document = {
  /**
   * author_id
   * 
   */
  author_id: ObjectId;
  /**
   * id
   * 
   */
  id: ObjectId;
  /**
   * metadata
   * 
   */
  metadata: Partial<Record<string, ObjectId>>;
  /**
   * parent_id
   * 
   */
  parent_id: ObjectId | undefined;
  /**
   * tags
   * 
   */
  tags: Array<ObjectId>;
  /**
   * title
   * 
   */
  title: string;
};
```

`Document::zod_schema()`:

```typescript
const Document$RawSchema = z.strictObject({
  author_id: z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) }),
  id: z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) }),
  metadata: z.record(z.string(), z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) })),
  parent_id: z.union([z.null().transform(() => undefined), z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) }), z.undefined()]).prefault(undefined),
  tags: z.array(z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) })),
  title: z.string(),
});

export const Document$Schema: ZodType<Document> = Document$RawSchema;
```
"#
)]
///
///
/// `ObjectId` fields are serialized using `MongoDB`'s standard format: `{ "$oid": "hex_string" }`
/// and include proper validation for 24-character hexadecimal `ObjectId` strings.
///
/// ## The Name a Type Publishes Under
///
/// A type publishes under the Rust ident it is declared with, spelled exactly as written. Nothing
/// is read off that spelling: no suffix is taken off it and no part of it is rewritten, so
/// `UserData` publishes as `UserData` on all three surfaces.
///
/// `#[model_schema(name = "...")]` is the one way to publish under something else. It moves the
/// name everywhere at once — the `export type` line, the Zod consts, the `$defs` key a
/// self-referential document hoists under, and every reference another type writes:
///
/// ```rust
/// # fn main() {}
/// use tixschema::model_schema;
/// use serde::{Deserialize, Serialize};
///
/// #[model_schema(name = "ContextValue")]
/// #[derive(Serialize, Deserialize)]
/// pub struct ContextValueData {
///     pub label: String,
/// }
///
/// #[model_schema()]
/// #[derive(Serialize, Deserialize)]
/// pub struct Holder {
///     pub value: ContextValueData,
/// }
/// ```
///
/// The declaration is published as `export type ContextValue` and `ContextValue$Schema`, and
/// `Holder`'s member is written `value: ContextValue;` and `value: ContextValue$Schema,`.
///
/// A type alias is the one shape with no surface name of its own, and publishes under a `Type`
/// suffix — `pub type Slug = String;` as `SlugType` — unless `name` says otherwise.
///
/// Two declarations cannot publish under one name. The emitted types, schemas and definitions are
/// one flat namespace, so a second declaration reaching a name already taken is refused at
/// expansion, naming both declarations:
///
/// ```text
/// error: model_schema: type `Beta` publishes as `Shared`, which type `Alpha` already publishes
///        as -- one name cannot carry two declarations, whose types, schemas and definitions would
///        overwrite each other. Give one of them a `#[model_schema(name = "...")]` of its own
/// ```
///
/// A renamed type also publishes its Rust ident as an alias of the moved name (`export type
/// ContextValueData = ContextValue;`), which is what lets a type declared *above* it — with only
/// the ident to name it by — still resolve. The Rust module the schema is published in is named
/// from the ident for the same reason, and an override never moves it.
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
/// ## Type Parameters and `default_types`
///
/// JSON Schema has no type parameters, so a generic item's document has to be built from one
/// concrete filling. `default_types` is where that filling is named — one `Parameter = Type` pair
/// per type parameter, in any order, beside every other item-level argument:
///
/// ```rust
/// use tixschema::model_schema;
/// use serde::{Deserialize, Serialize};
///
/// #[model_schema(default_types(IdType = String, DateType = f64))]
/// #[derive(Serialize, Deserialize)]
/// pub struct EcmDocument<IdType, DateType> {
///     pub document_id: IdType,
///     pub created_at: DateType,
/// }
/// ```
///
/// The declaration is read in both directions, and each refusal is spanned on what earned it. An
/// entry naming something the item does not declare is refused in **every** feature configuration:
/// a misspelled parameter fills nothing, and the parameter it was meant for would keep no default
/// at all. A type parameter with no entry is refused only where the `jsonschema` feature is on,
/// nothing else reading the default — without that feature the same item compiles untouched.
/// `default_types` on an item that declares no type parameter is refused, there being nothing for a
/// filling to fill; a lifetime and a const parameter name no type, so neither takes an entry.
///
/// A filling is also held against the bounds its parameter declares. Whether one satisfies them is
/// a question about trait impls, which a macro cannot answer, so the filling is handed to a
/// generated function carrying those bounds and the compiler answers — spanned on the entry as
/// written, in every feature configuration. So `default_types(CountType = String)` on
/// `struct Counted<CountType: Copy>` is refused: no value of the item can be held at that filling,
/// and a document built from it would describe nothing. A parameter with no bounds admits every
/// filling; a bound naming another parameter of the item holds only where that one is filled too,
/// so it is left to the item's own use sites.
///
/// There is no fallback filling. A guessed one produces a document that silently rejects valid
/// payloads, which is the failure the declaration exists to prevent.
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
/// String constraints stop at the brand's own type parameters. TypeScript binds a parameter for
/// real and Zod reaches it through the argument the brand's own factory binds, but the one JSON
/// document a brand publishes covers every instantiation and so describes a parameter as `{}`,
/// where `minLength` goes inert while `validate()` still measures `Display` — three surfaces, three
/// answers. So `#[model_schema(minLength = 3, default_types(T = String))] struct Slug<T>(pub T);`
/// is refused at the inner field. Constrain a string-typed inner instead.
///
/// A named inner is judged by what that name publishes, since that is the schema the checks are
/// appended to: `#[model_schema(minLength = 3)] struct Outer(pub Blob);` over
/// `#[serde(transparent)] struct Blob(pub serde_json::Value);` is refused for the same reason the
/// opaque inner spelled directly is. That answer comes from the named type's own expansion, so a
/// brand written above the type it names is not refused where it stands — the type it names has not
/// been read yet, and at that point a name declared below and a type this crate never expands are
/// the same silence. The consult is kept instead, and the named type's own expansion answers it:
/// the refusal arrives there, spanned on that declaration and naming the brand, so the verdict is
/// the same in either order. A brand over a type this crate never expands keeps the emission it has
/// always had, its consult never answered.
///
/// The brand's slot takes no `#[model_schema_prop(...)]` at all. A brand publishes its inner's own
/// schema with a `.brand()` written onto it, so no key written on the slot reaches any surface, and
/// one written there is refused. The three checks a brand does carry are the type-level ones above.
/// A `#[serde(transparent)]` struct with a *named* field is no brand, and its field attributes are
/// read as any other named field's are.
///
#[proc_macro_attribute]
pub fn model_schema(args: TokenStream, input: TokenStream) -> TokenStream {
    exec_model_schema(args.into(), input.into()).into()
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
/// pub struct UserData {
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
/// It also has to turn some value away. A pattern every string satisfies — `""`, `^`, `$`, `|`,
/// `a*` — constrains nothing, and is refused at expansion rather than published as a check that
/// checks nothing. `^$` is not one of them: it pins both ends of the value to one position, which
/// only the empty string has.
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
/// pub struct ProductData {
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
/// With `serde` and a schema output feature (`zod`, `typescript`, or `jsonschema`) both active, a
/// type carrying at least one constrained field also gets a
/// `validate(&self) -> Result<(), Vec<String>>` method for validating instances constructed in
/// code. Serde deserialization validates automatically.
///
/// ```rust
/// use tixschema::{model_schema, model_schema_prop};
/// use serde::{Deserialize, Serialize};
///
/// #[model_schema()]
/// #[derive(Serialize, Deserialize)]
/// pub struct RegistrationData {
///     #[model_schema_prop(minLength = 3, maxLength = 30)]
///     pub username: String,
///
///     #[model_schema_prop(minimum = 0, maximum = 120)]
///     pub age: u32,
/// }
/// ```
///
/// Every constrained field that fails contributes its own message, each naming the field it came
/// from:
///
/// ```text
/// let reg = RegistrationData { username: "ab".to_string(), age: 150 };
/// match reg.validate() {
///     Ok(()) => println!("valid"),
///     Err(errors) => {
///         for e in &errors {
///             println!("Error: {e}");
///         }
///         // "'username' is too short: minimum length is 3, got 2"
///         // "'age' is too large: maximum is 120, got 150"
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
/// pub struct ApiConfigData {
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
/// pub struct EventData {
///     // → z.preprocess(trim, z.preprocess(normalize, z.string()))
///     #[model_schema_prop(preprocess = ["trim", "normalize"])]
///     pub name: String,
/// }
/// ```
///
/// ## Flags
///
/// These take no value, and each is only valid on the field shape it names — applying one
/// elsewhere is a compile error.
///
/// - `ts_optional` — on an `Option<T>` field, writes the optional TypeScript key `field?: T`
///   instead of the default `field: T | undefined`. TypeScript-only; Zod and JSON Schema are
///   unchanged. The field must have a key for it to make optional: a positional slot writes none,
///   and a member a serde attribute takes off the wire in both directions is described nowhere, so
///   the flag is refused on either.
/// - `as_number` — on a `DateTime<Tz>` field, renders an epoch-millisecond `number` with an
///   inline Zod coercer instead of the default `Date`.
/// - `nullable` — on an `Option<T>` field, renders `T | null` with the key **required** on
///   TypeScript, Zod and JSON Schema, instead of the default `T | undefined` with the key
///   dropped for a `None`. Refused together with `ts_optional` — the two disagree about the key.
///   With the `serde` feature on, the field must never carry a key-dropping serde attribute
///   (`skip_serializing_if`, `skip_serializing`, `skip`): `nullable` declares the key always
///   written, so serde has to write `null` for a `None` rather than omit the key, and the guard
///   naming that pairing is the mirror of the one that requires such an attribute on a bare
///   `Option<T>` field with no `nullable`.
#[proc_macro_attribute]
pub fn model_schema_prop(_args: TokenStream, input: TokenStream) -> TokenStream {
    // For now, simply pass through the input
    input
}
