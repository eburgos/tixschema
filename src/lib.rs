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
 *       "type": "integer"
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
  age?: number;
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
  age: z.union([z.number().int(), z.undefined()]).prefault(undefined),
  firstName: z.string(),
  id: z.string(),
  lastName: z.string(),
  roles: z.array(z.string()),
});

export const User$Schema: z.ZodType<User> = User$RawSchema;
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

export const Status$Schema: z.ZodType<Status> = Status$RawSchema;
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
 *         "userId": {
 *           "type": "string"
 *         }
 *       },
 *       "required": [
 *         "type",
 *         "timestamp",
 *         "userId"
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
 *           "type": "string"
 *         },
 *         "userId": {
 *           "type": "string"
 *         }
 *       },
 *       "required": [
 *         "type",
 *         "userId"
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
   * userId
   * 
   */
  userId: string;
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
  reason?: string;
  /**
   * userId
   * 
   */
  userId: string;
};
```"#]
///
/// `Event::zod_schema()`:
///
#[doc = r#"```typescript
const Event$RawSchema = z.discriminatedUnion("type", [z.strictObject({
  type: z.literal("userCreated"),
  timestamp: z.string(),
  userId: z.string(),
}), z.strictObject({
  type: z.literal("userDeleted"),
  reason: z.union([z.string(), z.undefined()]).prefault(undefined),
  userId: z.string(),
})]);

export const Event$Schema: z.ZodType<Event> = Event$RawSchema;
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
  parent_id?: ObjectId;
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
  parent_id: z.union([z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) }), z.undefined()]).prefault(undefined),
  tags: z.array(z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) })),
  title: z.string(),
});

export const Document$Schema: z.ZodType<Document> = Document$RawSchema;
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
/// pub struct RegistrationJson {
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
/// let reg = RegistrationJson { username: "ab".to_string(), age: 150 };
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

/// # `typescript_preamble`
///
/// The `TypeScript` a generated module carries once, above the per-type definitions it is
/// assembled from. Expands to a string literal and takes no arguments.
///
/// ```rust
/// const PREAMBLE: &str = tixschema::typescript_preamble!();
/// ```
///
/// A generic type publishes `X$SchemaFactory` rather than `X$Schema`, because a Zod schema is a
/// runtime value and one value cannot stand for every filling of a parameter. It also publishes
/// `X$SchemaDefault`, the factory called at the type's own declared `default_types` — the ordinary
/// filling, memoized like any other call so a consumer of the common case never has to construct
/// the argument list by hand. Each factory memoizes on the identity of the arguments it was
/// handed, and `createSchemaCache` is the helper they all build those caches with:
///
#[cfg_attr(
    feature = "typescript",
    doc = "```typescript
const createSchemaCache = <Cache extends object>(): Cache => new WeakMap() as unknown as Cache;
```"
)]
#[cfg_attr(
    not(feature = "typescript"),
    doc = "```javascript
const createSchemaCache = () => new WeakMap();
```"
)]
///
/// A cache maps an argument to the schema built from *that* argument, so its value type depends on
/// its key type. `TypeScript` can declare that dependency — each factory writes it out, one
/// interface per parameter — but cannot construct a map that satisfies it, since a `WeakMap` fixes
/// both of its own parameters at construction. So the dependency is declared where it does the
/// work and asserted exactly once, here: this line is the only assertion anywhere in the generated
/// output.
///
/// Emit it once per module, ahead of every `zod_schema()` the module carries. A module with no
/// generic type in it needs no preamble, and one that has any needs it exactly once.
#[cfg(feature = "zod")]
#[proc_macro]
pub fn typescript_preamble(input: TokenStream) -> TokenStream {
    model_schema::typescript_preamble_tokens(input.into()).into()
}
