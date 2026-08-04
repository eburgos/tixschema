# TixSchema

A Rust procedural macro library that generates TypeScript type definitions and Zod v4 validation schemas from Rust structs and enums, ensuring type safety and consistency between Rust backends and TypeScript frontends.

## Installation

### Rust Dependencies

Add the following to your `Cargo.toml`:

```toml
[dependencies]
tixschema = <path or crate_id or repo>  # eg: { git = "https://github.com/tixena/tixschema.git" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### Frontend Dependencies (Zod v4)

**Important:** This crate requires Zod v4 for full functionality, especially JSON schema generation. Zod v3 is not supported.

```bash
npm install zod@^4.0.0
# or
yarn add zod@^4.0.0
```

## Quick Start

```rust
use tixschema::model_schema;
use serde::{Deserialize, Serialize};

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub age: u32,
    pub is_active: bool,
}
```

This generates:
- `User::ts_definition()` -- Returns a TypeScript type definition and Zod schema as a string
- `User::json_schema()` -- Returns a JSON schema (requires `jsonschema` feature)

Generated TypeScript:

```typescript
export type User = {
  id: string;
  name: string;
  email: string;
  age: number;
  is_active: boolean;
};

export const User$Schema: ZodType<User> = z.strictObject({
  id: z.string(),
  name: z.string(),
  email: z.string(),
  age: z.number().int(),
  is_active: z.boolean(),
});
```

## Usage

### Structs

Any Rust struct annotated with `#[model_schema()]` generates a corresponding TypeScript type and Zod schema. Primitive types map as follows: `String` to `string`, `bool` to `boolean`, all numeric types to `number` (integers get `.int()`), `Option<T>` to `T | undefined`, `Vec<T>` to `Array<T>`, and `HashMap<String, T>` to `Partial<Record<string, T>>`.

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct UserProfile {
    pub user_id: String,
    pub name: String,
    pub age: u32,
    pub score: f64,
    pub is_verified: bool,
}
```

### Optional Fields

`Option<T>` fields become `T | undefined` in TypeScript and `z.union([type, z.undefined()]).prefault(undefined)` in Zod v4. The `.prefault(undefined)` makes the field default to `undefined` when omitted from the input.

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct UserWithOptionals {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}
```

### Collections and Maps

`Vec<T>` becomes `Array<T>`, and so does every other std wrapper serde writes as a JSON array of its element: `VecDeque<T>`, `LinkedList<T>`, `BinaryHeap<T>`, `HashSet<T>`, and `BTreeSet<T>`. Each is that array on the wire, so each is typed and validated as that array — the element decides what the array holds. Nesting is kept at whatever depth it is written: a `Vec<Vec<T>>` (or `HashSet<Vec<T>>`, or any other mix of those wrappers) writes an array of arrays, so it becomes `Array<Array<T>>`, `z.array(z.array(...))`, and a JSON schema whose `items` is itself an array.

A fixed-size array `[T; N]` is that array too, with its length described. serde writes exactly `N` items and reads one back only at that length, so the two validating surfaces say so: the JSON schema pins the level with `"minItems": N` and `"maxItems": N`, and Zod appends `.length(N)`. The bound belongs to the level it was written at, so `Vec<[T; 3]>` is an unbounded array of 3-element arrays and `[Vec<T>; 3]` is a 3-element array of unbounded ones. TypeScript stays `Array<T>`: the fixed-length form its type system has is the N-element tuple, which has to be written out element by element and stops being readable long before `N` stops being legal. A length the macro cannot read — a const generic parameter, a `const` item, any computed expression — describes as an unbounded array, the macro running before there is a value to ask for; a slice `[T]` has no length to describe at all.

```rust
use std::collections::HashMap;

#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct UserWithCollections {
    pub id: String,
    pub tags: Vec<String>,
    pub scores: Vec<u32>,
    pub metadata: HashMap<String, String>,
    pub settings: Option<HashMap<String, String>>,
}
```

`HashMap<K, V>` and `BTreeMap<K, V>` are both read as maps, and the key decides what the map describes as. A `String` key is the open case: any string is a key, so the JSON schema puts the value's schema under `"additionalProperties"`, TypeScript writes `Partial<Record<string, V>>`, and Zod writes `z.record(z.string(), V)`.

A key written as a plain `#[model_schema()]` enum is the narrowed case: the enum's members become the object's properties and nothing else is allowed, so the JSON schema lists each member with the value's schema and closes the object with `"additionalProperties": false`, while TypeScript and Zod name the enum itself as the key type — `Partial<Record<Slot, V>>` and `z.record(Slot$Schema, V)`. The members are named as serde writes them, so a `#[serde(rename_all = "...")]` on the enum renames the properties too. This holds wherever the enum is declared relative to the type that writes the map, and through a `#[model_schema()]` alias of one.

```rust
use std::collections::HashMap;

#[model_schema()]
#[derive(Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Slot {
    Primary,
    Secondary,
}

#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct Schedule {
    // {"type": "object", "properties": {"Primary": {...}, "Secondary": {...}},
    //  "additionalProperties": false}
    pub slots: HashMap<Slot, String>,
}
```

A `#[serde(transparent)]` branded newtype over a string — a brand over `String` or `PathBuf`, or over another such brand — is the open case wearing a name. serde writes the brand as the bare string its inner is, which is exactly what a JSON object key is, so the map is an object keyed by arbitrary strings. TypeScript and Zod keep the brand's own spelling as the key type the way they keep an enum's — `Partial<Record<CorrelationId, V>>` and `z.record(CorrelationId$Schema, V)` — while the JSON schema is the open object, having no brand to say.

```rust
use std::collections::HashMap;

#[model_schema()]
#[derive(Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct CorrelationId(String);

#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct Traces {
    // {"type": "object", "additionalProperties": {"type": "string"}}, serialized {"abc": "..."}
    pub spans: HashMap<CorrelationId, String>,
}
```

A brand adds a name to its inner's wire and nothing else, so it keys a map however its inner keys one — at every link of a chain of brands. A brand over a **plain enum** is the open case too: serde writes the variant name, which is a bare string, and the brand publishes no `enum_members()` of its own for a schema to close the object over, so it opens the object under its own name rather than closing it over members nothing can supply. A brand over a value serde **stringifies** — a number, a `bool`, a chrono type — describes exactly as that bare inner describes, the open object with nothing said about its members, while TypeScript and Zod keep the brand's name as the key type. Only a brand over something serde writes as no key at all — a struct, a container, a tuple, a nested map, an `ObjectId` — is refused, and so is a brand over an inner this expansion has not classified yet: what serde writes for such an inner is exactly what cannot be told.

A `#[model_schema()]` **alias** answers the same way, its target standing in for it because a type path resolves straight through an alias. An alias of `String` or `PathBuf`, of a string-shaped brand, or of another such alias is written as that bare string, so it keys a map exactly as a `String` does — under the alias's own exported name on the nominal surfaces and as the open object on the structural one. An alias of a plain enum enumerates its members, as it always has.

```rust
use std::collections::HashMap;

#[model_schema()]
type SlotKey = String;

#[model_schema()]
#[derive(Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct Tick(u32);

#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct Samples {
    // {"type": "object", "additionalProperties": {"type": "number"}}, serialized {"abc": 1.0}
    // TypeScript `Partial<Record<SlotKeyType, number>>`, Zod `z.record(SlotKeyType$Schema, ...)`
    pub by_slot: HashMap<SlotKey, f64>,
    // {"type": "object", "additionalProperties": true}, serialized {"7": 1.0}
    // TypeScript `Partial<Record<Tick, number>>`, Zod `z.record(Tick$Schema, ...)`
    pub by_tick: HashMap<Tick, f64>,
}
```

A key path is the one spelling that can name an enum, an alias or a brand, so a key path the expansion can prove serde writes as no key at all — a struct, a brand over one or over a container, a tagged or untagged enum, or a `#[model_schema()]` alias of any of them — is refused where it is written, at whatever depth the map sits at, naming the type as the author spelled it. A key path the expansion has not seen yet, one declared after the type that writes the map or one from another crate, cannot be ruled out and is emitted as an enumerating key; a key that turns out to have no members surfaces as an `E0599` at the key type instead.

The remaining refusals are all one rule: a JSON object key is a string, and `serde_json` raises `key must be a string` — refusing the whole map at serialization, with no fallback form — for every key whose own wire form is not one. Each of these is therefore refused rather than described, there being no object to describe:

- a **sequence-wrapped** key — a `Vec`, a `[T; N]`, one of the sets — which writes a JSON array;
- a **tuple** key, which writes a JSON array;
- a **nested map** key and an **`ObjectId`** key, which write JSON objects;
- an **`Option`** key, whose `Some` writes what its inner writes but whose `None` writes nothing a key can be — so a single `None` fails the whole map at runtime, and a schema for the `Some` half alone would describe a contract the type does not keep.

All of them are covered under [Compilation Errors](#compilation-errors), with the exact diagnostic each produces.

Every other key is neither open nor enumerable, and none of them is refused: serde stringifies each one into a key for you. The JSON schema describes such a map as an object and says nothing about its members — `{"type": "object", "additionalProperties": true}` — while TypeScript and Zod keep the key's own type, so a `HashMap<u32, String>` is `Partial<Record<number, string>>` and `z.record(z.number().int(), z.string())`. serde writes the object with the key's string form for its keys: `7` becomes `"7"`, `true` becomes `"true"`, a chrono `NaiveDate` its ISO rendering. A brand over one of those writes the same object and describes as the same one, under the brand's name. Only the narrowing is missing, not the object. The rule the refusals apply is refuse-what-serde-refuses, never refuse-what-is-not-a-`String`.

### Pointers and Borrowed Values

`Box<T>`, `Rc<T>`, `Arc<T>` and `Cow<'_, T>` describe as `T`. serde writes each of them as the value it holds, with nothing of its own around it, so a field written under one is the field its inner type is — on every surface, and wherever the wrapper was written: `Box<Option<T>>` is the `Option<T>` field, optional; `Vec<Rc<T>>` is the `Vec<T>` field; `Box<str>`, `Rc<str>` and `Cow<'_, str>` are `String` fields, `str` being what a `String` writes.

This is what lets a type hold itself: `Option<Box<Self>>` is the self-reference, deferred in the generated schema the way any self-reference is. An `Rc` or `Arc` field needs serde's `rc` feature, which is where serde's impls for those two live.

```rust
use std::borrow::Cow;
use std::sync::Arc;

#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct TreeNode {
    pub label: Cow<'static, str>,
    pub shared_tags: Arc<[String]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<Box<TreeNode>>,
}
```

### Enums

#### Plain Enums

Plain enums (all unit variants) generate a TypeScript string union and `z.enum()` in Zod.

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserStatus {
    Active,
    Inactive,
    Pending,
    Suspended,
}
```

#### Discriminated Unions (Tagged Enums)

Enums with `#[serde(tag = "...")]` generate TypeScript discriminated unions.

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PaymentMethod {
    CreditCard {
        card_number: String,
        expiry_date: String,
        cvv: String,
    },
    BankTransfer {
        account_number: String,
        routing_number: String,
    },
    PayPal {
        email: String,
    },
}
```

#### Tuple Variants

Discriminated unions also support tuple variants. Single-element tuples are flattened to a `value` field, and multi-element tuples generate a TypeScript tuple type.

```rust
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "value")]
pub enum FixedValue {
    // Unit variant (no data)
    Empty,
    // Single-element tuple: { type: "Text", value: string }
    Text(String),
    // Multi-element tuple: { type: "Pair", value: [string, number] }
    Pair(String, i64),
    // Named struct variant (existing behavior)
    Named { field_a: String, field_b: bool },
}
```

Generated TypeScript:

```typescript
export type FixedValue = {
  type: "Empty";
} | {
  type: "Text";
  value: string;
} | {
  type: "Pair";
  value: [string, number];
} | {
  type: "Named";
  field_a: string;
  field_b: boolean;
};
```

Generated Zod:

```typescript
export const FixedValue$Schema: ZodType<FixedValue> = z.discriminatedUnion("type", [
  z.strictObject({
    type: z.literal("Empty"),
  }),
  z.strictObject({
    type: z.literal("Text"),
    value: z.string(),
  }),
  z.strictObject({
    type: z.literal("Pair"),
    value: z.tuple([z.string(), z.number().int()]),
  }),
  z.strictObject({
    type: z.literal("Named"),
    field_a: z.string(),
    field_b: z.boolean(),
  }),
]);
```

You can customize the tag and content field names using Serde's `tag` and `content` attributes:

```rust
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "kind", content = "data")]
pub enum Event {
    Text(String),
    Number(i64),
}
```

This generates `kind` as the discriminator and `data` as the value field instead of the defaults (`type` and `value`).

#### Externally Tagged Enums (No Tagging Attributes)

An enum with data-carrying variants that names neither `tag` nor `content` is externally tagged, which is Serde's default: the variant name is the sole key of an object holding the content, and a unit variant is that name as a bare string. The generated surfaces describe that form -- a JSON Schema `oneOf`, a TypeScript union, and a Zod `z.union([...])`. There is no field every member shares, so the Zod schema is a plain union rather than a `z.discriminatedUnion`.

```rust
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum External {
    Bare,
    Fields { a: String, b: bool },
    Pair(u32, u32),
    Single(String),
}
```

Serialized by Serde:

```json
"Bare"
{ "Fields": { "a": "a", "b": true } }
{ "Pair": [1, 2] }
{ "Single": "a" }
```

Generated TypeScript:

```typescript
export type External = "Bare" | {
  "Fields": {
    a: string;
    b: boolean;
  };
} | {
  "Pair": [number, number];
} | {
  "Single": string;
};
```

Generated Zod:

```typescript
export const External$Schema: ZodType<External> = z.union([
  z.literal("Bare"),
  z.strictObject({
    "Fields": z.strictObject({ a: z.string(), b: z.boolean() }),
  }),
  z.strictObject({
    "Pair": z.tuple([z.number().int(), z.number().int()]),
  }),
  z.strictObject({
    "Single": z.string(),
  }),
]);
```

Add `#[serde(tag = "...", content = "...")]` to get the adjacently tagged `{ type, value }` form documented above instead.

#### Internally Tagged Enums (`tag` With No `content`)

An enum that names `tag` but no `content` is internally tagged: there is no key for a variant's data, so Serde writes it as members of the object the tag is written in. A struct variant's fields sit beside the tag, and so do the members of a newtype variant's inner type -- which the surfaces describe as an intersection, the same composition `#[serde(flatten)]` uses.

```rust
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TagPayload {
    pub a: String,
    pub b: bool,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum Internal {
    Bare,
    Fields { a: String, b: bool },
    Wrapped(TagPayload),
}
```

Serialized by Serde:

```json
{ "type": "Bare" }
{ "type": "Fields", "a": "a", "b": true }
{ "type": "Wrapped", "a": "a", "b": true }
```

Generated TypeScript:

```typescript
export type Internal = {
  type: "Bare";
} | {
  type: "Fields";
  a: string;
  b: boolean;
} | {
  type: "Wrapped";
} & TagPayload;
```

Generated Zod -- a flattened member is an intersection, which has no shape of its own to read the discriminator out of, so a union holding one is a plain `z.union` rather than a `z.discriminatedUnion`:

```typescript
export const Internal$Schema: ZodType<Internal> = z.union([
  z.strictObject({ type: z.literal("Bare") }),
  z.strictObject({ type: z.literal("Fields"), a: z.string(), b: z.boolean() }),
  z.strictObject({ type: z.literal("Wrapped") }).and(z.lazy(() => TagPayload$Schema)),
]);
```

The content joins the member under `z.lazy` for the reason a flattened base does: the inner type is a `const` of its own and nothing orders one type's schema against another's, so reading the name while the union's `const` initializes would fail for any inner type declared below -- and a cycle running through a variant's content and a `#[serde(flatten)]` base has no order that puts each above the other. Deferred, every module loads whatever order it is assembled in.

Only a value Serde writes as an object has members to put beside the tag. A newtype variant wrapping a string, a number, a boolean, a sequence, an `Option` or a tuple is one Serde refuses to serialize at run time (`cannot serialize tagged newtype variant ... containing a string`), and a multi-element tuple variant is one Serde's own derive refuses outright. `#[model_schema()]` rejects all of those at expansion rather than describing a value that cannot reach the wire; name a `content` key so the value gets an object of its own, or wrap it in a struct whose fields can sit beside the tag.

A name is not a promise of an object either. A newtype variant wrapping a plain `#[model_schema()]` enum is rejected at expansion the same way: a plain enum writes its own variant name, which Serde puts beside the tag as a key holding null and which a schema closed around the tag rejects. Every other named type -- a struct, a newtype over a scalar -- looks alike to the expansion, so one that turns out not to be written as an object is caught when `json_schema()` runs, with a diagnostic naming the variant, the inner type and the same remedy. The TypeScript and Zod intersections for that case are a known gap: they are written, and only the JSON Schema surface refuses them.

### Intersection Types (`#[serde(flatten)]`)

A struct field marked `#[serde(flatten)]` is lifted into the parent type as a TypeScript intersection (`A & B`) and a Zod `.and()` chain, instead of a nested object. This is the idiomatic way to compose a common set of fields with a discriminated union.

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataElementSampleValueEntry {
    pub data_element_id: String,
    #[serde(flatten)]
    pub variant: DataElementSampleValueVariant,
}

#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(tag = "dataType")]
pub enum DataElementSampleValueVariant {
    Alphanumeric {
        #[serde(rename = "sampleValues")]
        sample_values: Vec<String>,
    },
    Numeric {
        #[serde(rename = "sampleValues")]
        sample_values: Vec<i64>,
    },
}
```

Generated TypeScript:

```typescript
export type DataElementSampleValueEntry = {
  dataElementId: string;
} & DataElementSampleValueVariant;
```

Generated Zod:

```typescript
export const DataElementSampleValueEntry$Schema: ZodType<DataElementSampleValueEntry> =
  z.strictObject({
    dataElementId: z.string(),
  }).and(z.lazy(() => DataElementSampleValueVariant$Schema));
```

Notes:

- The flattened base joins the intersection under `z.lazy`. A base is a `const` of its own and nothing orders one type's schema against another's, so reading the name while the intersection's `const` initializes would fail for any base declared below -- and for two bases that flatten each other, no order puts each above the other. Deferred, every module loads whatever order it is assembled in, and a pair that does flatten each other fails when asked to validate rather than at import.
- Multiple `#[serde(flatten)]` fields chain: `{ ... } & BasePart & ExtraPart` in TypeScript and `z.strictObject({ ... }).and(z.lazy(() => BasePart$Schema)).and(z.lazy(() => ExtraPart$Schema))` in Zod.
- A struct whose only field is flattened becomes a plain alias (e.g. `export type FlattenOnly = DataElementSampleValueVariant;`).
- **JSON Schema** stays strict: rather than `allOf` (which cannot faithfully compose a tagged union under `additionalProperties: false`), the base properties are distributed into each branch of the flattened union, keeping every branch closed. Both spellings of a union are distributed into: a discriminated enum's `oneOf` and an untagged enum's `anyOf`. Plain-struct flattens merge into a single closed object; multiple flattened unions form a cross-product.
- Each flattened union keeps the spelling it was written under. A discriminated enum's members are exclusive, so its branches stay a `oneOf`; an untagged enum is first-match-wins and its members may overlap (one member's keys a subset of another's, the difference optional), so its branches are an `anyOf` -- under `oneOf` the document would reject exactly what Serde writes for the narrower member, which matches both branches. An object flattening one of each nests the wrappers, the untagged `anyOf` sitting inside each branch of the discriminated `oneOf`.
- A flattened source reached through an `Option` is a choice rather than a key set. Serde writes its members beside the object's own for a `Some` and writes the object alone for a `None`, so the **JSON Schema** offers both under an `anyOf`: one branch with the source's members merged in, one naming the object's own keys alone. Folding the members into a single object would require keys the `None` payload never carries, and dropping them from `required` would admit a base written in part -- neither is a payload Serde produces. A union reached through an `Option` keeps its own spelling inside the branch and gains the absence outside it.
- Only a value Serde writes as an object can be flattened -- Serde refuses the rest at run time (`can only flatten structs and maps`). Flattening a plain `#[model_schema()]` enum is rejected at expansion; any other type that turns out not to be written as an object is caught when `json_schema()` runs, naming the field's type and the remedy (write the field as a named member). A flattened union whose members are described one by one is checked the same way, member by member: a member Serde does not write as an object is refused with its branch named.

### Untagged Enums (`#[serde(untagged)]`)

An enum marked `#[serde(untagged)]` serializes as just its content, with no discriminator field. It generates a TypeScript union (`A | B`), a Zod `z.union([...])`, and a JSON Schema `anyOf`. Newtype (`S(T)`) and named-struct (`{ a: A }`) variants are supported.

```rust
/// ISO-8601 date string; branded newtype carrying the regex pattern.
#[model_schema(pattern = r"^\d{4}-\d{2}-\d{2}$")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct DateString(pub String);

/// A date sample value: an ISO date string OR an epoch number.
#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum DateValue {
    N(i64),
    S(DateString),
}
```

Generated TypeScript:

```typescript
export type DateValue = number | DateString;
```

Generated Zod:

```typescript
const DateValue$RawSchema = z.union([z.number().int(), DateString$Schema]);
export const DateValue$Schema: ZodType<DateValue> = DateValue$RawSchema;
```

Generated JSON Schema:

```json
{ "anyOf": [
  { "type": "integer" },
  { "type": "string", "pattern": "^[0-9]{4}-[0-9]{2}-[0-9]{2}$" }
] }
```

The `\d` the brand was declared with reaches the schema as `[0-9]`, the members it stands for, so the JSON Schema and the Rust validator accept the one set of strings -- see [What a `pattern` may contain](#what-a-pattern-may-contain).

Named-struct variants render each member as a closed object:

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum NamedUnion {
    A { x: String },
    B { y: i64 },
}
```

```typescript
// TypeScript
export type NamedUnion = { x: string } | { y: number };
// Zod
z.union([z.strictObject({ x: z.string(), }), z.strictObject({ y: z.number().int(), })])
// JSON Schema: { "anyOf": [ <object>, <object> ] }  — each branch has additionalProperties: false
```

Untagged enums compose with `#[serde(flatten)]`: a flattened variant carrying `Vec<DateValue>` renders `sampleValues: z.array(DateValue$Schema)` (TypeScript `Array<DateValue>`), and the JSON-schema `items` for that field is the `DateValue` `anyOf`.

A member of an untagged variant carries `#[model_schema_prop(...)]` exactly as the same field written in a tagged variant does: the constraint reaches the Zod schema and the JSON Schema, and every guard the attribute earns is reported at the member. The one difference is the Rust side -- an untagged enum generates no per-field validators, so a constrained member has no `validate()` contribution and no deserialization check.

**Unsupported variants:** unit variants and multi-field tuple variants in an untagged enum produce a compile-time error.

### Nested Types

Types annotated with `#[model_schema()]` can reference each other. The TypeScript output uses the type's name (without any suffix) as the reference.

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct Address {
    pub street: String,
    pub city: String,
    pub zip_code: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct UserWithAddress {
    pub id: String,
    pub name: String,
    pub address: Address,
    pub backup_addresses: Vec<Address>,
}
```

A referenced type must be declared at item scope — in a module or at crate level, not inside a function body. Each type publishes its schema in a module beside itself, and a type that references one reaches that module through `use super::*`, which a module nested in a function body is not part of. The referencing type is unconstrained: it may sit inside a function body as long as everything it names does not. See [Function-Local Types](#function-local-types) for the error a violation produces.

### Recursive Types

The library supports recursive and self-referential types. In the generated Zod schema, recursive fields use JavaScript getter syntax to defer the reference and avoid "use before declaration" errors.

The self-reference can be written either with the type's own name (`Vec<TreeNode>`) or with the `Self` keyword (`Vec<Self>`); both produce identical output.

```rust
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TreeNode {
    pub val: String,
    pub children: Vec<TreeNode>,
}
```

Generated TypeScript:

```typescript
export type TreeNode = {
  val: string;
  children: Array<TreeNode>;
};
```

Generated Zod:

```typescript
const TreeNode$RawSchema = z.strictObject({
  val: z.string(),
  get children() { return z.array(TreeNode$Schema); },
});

export const TreeNode$Schema: ZodType<TreeNode> = TreeNode$RawSchema;
```

Recursive enums are also supported. Only the variants that contain a self-reference use getter syntax; non-recursive variants use normal property syntax:

```rust
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "value")]
pub enum DynamicValue {
    #[serde(rename = "string")]
    String(String),
    #[serde(rename = "integer")]
    Integer(i64),
    #[serde(rename = "array")]
    Array(Vec<DynamicValue>),
    #[serde(rename = "object")]
    Object(HashMap<String, DynamicValue>),
}
```

Generated Zod:

```typescript
export const DynamicValue$Schema: ZodType<DynamicValue> = z.discriminatedUnion("type", [
  z.strictObject({
    type: z.literal("string"),
    value: z.string(),
  }),
  z.strictObject({
    type: z.literal("integer"),
    value: z.number().int(),
  }),
  z.strictObject({
    type: z.literal("array"),
    get value() { return z.array(DynamicValue$Schema); },
  }),
  z.strictObject({
    type: z.literal("object"),
    get value() { return z.record(z.string(), DynamicValue$Schema); },
  }),
]);
```

### Type Aliases

The `#[model_schema()]` macro supports `type` alias statements, creating semantic type aliases that appear in the generated TypeScript output. Use the `name` argument to control the generated TypeScript name.

```rust
use tixschema::model_schema;

#[model_schema(name = "DocumentId")]
pub type DocumentId = String;

#[model_schema(name = "Revision")]
pub type Revision = i64;

#[model_schema(name = "Tags")]
pub type Tags = Vec<String>;
```

Generated TypeScript:

```typescript
export type DocumentId = string;

export type Revision = number;

export type Tags = Array<string>;
```

Type aliases are referenced by name when used as struct fields:

```rust
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DocumentRecord {
    pub document_id: DocumentId,
    pub revision: Revision,
}
```

Generated TypeScript:

```typescript
export type DocumentRecord = {
  document_id: DocumentId;
  revision: Revision;
};
```

### Branded Newtypes

`#[serde(transparent)]` tuple structs with a single public field generate branded TypeScript types. The newtype is invisible in JSON serialization but carries a distinct type identity in TypeScript, preventing accidental mixing of different ID types.

```rust
use tixschema::model_schema;
use serde::{Deserialize, Serialize};

// Generic branded newtype (good for parameterized IDs)
#[model_schema()]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UserId<ID_TYPE>(pub ID_TYPE);

// Non-generic branded newtype
#[model_schema()]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CorrelationId(pub String);
```

Generated TypeScript (with `zod` feature):

```typescript
export type UserId<ID_TYPE> = ID_TYPE & z.$brand<"UserId">;
const UserId$RawSchema = ID_TYPE$Schema.brand<"UserId">();
export const UserId$Schema: $ZodBranded<typeof ID_TYPE$Schema, "UserId"> = UserId$RawSchema;

export type CorrelationId = string & z.$brand<"CorrelationId">;
const CorrelationId$RawSchema = z.string().brand<"CorrelationId">();
export const CorrelationId$Schema: ZodType<CorrelationId> = CorrelationId$RawSchema;
```

Generated TypeScript (without `zod` feature):

```typescript
declare const __brand_UserId: unique symbol;
export type UserId<ID_TYPE> = ID_TYPE & { readonly [__brand_UserId]: true };
export function assertUserId<ID_TYPE>(value: ID_TYPE): asserts value is UserId<ID_TYPE> {}

declare const __brand_CorrelationId: unique symbol;
export type CorrelationId = string & { readonly [__brand_CorrelationId]: true };
export function assertCorrelationId(value: string): asserts value is CorrelationId {}
```

Notes:

- If the Rust type name ends with `Json`, the suffix is stripped in the generated TypeScript (e.g., `UserIdJson` becomes `UserId`). Otherwise, the Rust name is used as-is.
- Generic parameter names (e.g., `ID_TYPE`) are preserved exactly.
- A brand describes what its inner writes whether or not the inner names the brand's type parameters, so `TagList<T>(pub Vec<T>)` is an array on every surface — `Array<T>`, `z.array(T$Schema)`, `$ZodBranded<ZodArray, "TagList">`, `{"type": "array", "items": {}}` — and not the bare parameter. The parameter itself carries what an uninstantiated parameter carries anywhere else: its own name in TypeScript, the `$Schema` binding named after it in Zod, and the permissive empty schema in JSON, since one schema is written for every instantiation.
- Serde transparent serialization works normally -- the wrapper is invisible in JSON.
- Use branded newtypes for opaque IDs and phantom types to prevent passing the wrong ID type across domain boundaries.

#### The `Display` Requirement and `no_display`

Every branded newtype gets a `Display` impl that delegates to its inner value, so **the inner type must implement `Display`**. An inner type that does not is reported at the inner field, naming the trait:

```text
error[E0277]: `Vec<String>` doesn't implement `std::fmt::Display`
  --> src/lib.rs:7:21
   |
 7 | pub struct Tags(pub Vec<String>);
   |                     ^^^^^^^^^^^ the trait `std::fmt::Display` is not implemented for `Vec<String>`
```

Pass `no_display` for brands over an inner type that has none — a container, or a `PathBuf`. The brand then gets no `Display` impl and carries no such requirement; its schema and its serde transparency are unchanged. String constraints are a separate matter: `pattern`, `minLength`, and `maxLength` validate through `to_string()` for every inner but a path, so a constrained brand over one of the others needs a `Display` inner whether or not it passes `no_display` — and a container inner cannot carry them at all (see [Branded Newtype Validation Constraints](#branded-newtype-validation-constraints)).

```rust
use tixschema::model_schema;
use serde::{Deserialize, Serialize};

#[model_schema(no_display)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Tags(pub Vec<String>);
```

A generic brand carries the requirement as a `Display` bound on each type parameter, so a non-`Display` type argument (e.g. `DocumentId<Vec<String>>`) is rejected where the brand is used rather than where it is declared.

#### Branded Newtype Validation Constraints

You can add `pattern`, `minLength`, and `maxLength` constraints directly on the `#[model_schema()]` attribute for branded newtypes. Constraints are enforced in three places: the generated Zod schema, serde deserialization, and a `validate()` method on the type.

**The inner type has to be one whose schema is a string** — `String`, `PathBuf`, `ObjectId`, a chrono date/time type, another brand, or a generic parameter. A numeric, boolean, container (`Vec`, array, `HashMap`, tuple), or opaque (`serde_json::Value`) inner is rejected at expansion time, because the three constraints are string checks and each surface would read them differently: Zod's `.min`/`.max` become bounds on the value itself, JSON Schema ignores `minLength`/`maxLength`/`pattern` outside `"type": "string"`, and `validate()` measures the inner's `Display` rendering.

**The inner type also has to implement `Display`,** since validation runs against `to_string()`. That holds whether or not the brand passes `no_display`: the flag drops the `Display` impl, not the requirement.

A path inner is the one exception, and needs no `Display`: its checks read the path's `to_string_lossy` rendering — the string serde writes for it — exactly as a constrained path field's do. Such a brand still passes `no_display`, since the impl a brand gets by default has no inner `Display` to delegate to:

```rust
#[model_schema(no_display, minLength = 3, pattern = "^/[a-z]+$")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssetPath(pub std::path::PathBuf);
```

That holds for every spelling of a path: a transparent wrapper writes nothing of its own on the wire and derefs to what it holds, so `Arc<Path>`, `Box<Path>`, `Cow<'static, Path>`, and `Rc<Path>` are branded and bounded exactly as `PathBuf` is.

```rust
#[model_schema(pattern = "^[a-z0-9_]+$", minLength = 3, maxLength = 50)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SlugId(pub String);
```

Generated Zod:

```typescript
const SlugId$RawSchema = z.string()
  .min(3)
  .max(50)
  .check(z.regex(/^[a-z0-9_]+$/))
  .brand<"SlugId">()
  .meta({
    description: "SlugId",
  });

export const SlugId$Schema: $ZodBranded<ZodString, "SlugId"> = SlugId$RawSchema;
```

Serde deserialization validates automatically:

```rust
// Rejects values that violate constraints
let result: Result<SlugId, _> = serde_json::from_str("\"ab\"");
assert!(result.is_err()); // too short (min 3)

let result: Result<SlugId, _> = serde_json::from_str("\"UPPERCASE\"");
assert!(result.is_err()); // pattern mismatch

// Accepts valid values
let result: Result<SlugId, _> = serde_json::from_str("\"hello_world\"");
assert!(result.is_ok());
```

The `validate()` method checks constraints on instances constructed directly in code:

```rust
let slug = SlugId("hello_world".to_string());
assert!(slug.validate().is_ok());

let bad = SlugId("ab".to_string());
match bad.validate() {
    Ok(()) => unreachable!(),
    Err(errors) => println!("{:?}", errors), // ["'value' is too short: minimum length is 3, got 2"]
}
```

You can use any combination of the three constraints:

```rust
// Pattern only -- e.g., ObjectId hex string
#[model_schema(pattern = "^[0-9a-fA-F]{24}$")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObjectIdStr(pub String);

// Length only -- no pattern
#[model_schema(minLength = 1, maxLength = 255)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NonEmptyString(pub String);
```

An inner type whose schema is not a string is rejected at the field:

```rust
// This will NOT compile:
#[model_schema(pattern = "^[0-9]+$")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BadNum(pub u64);
```

```text
error: model_schema: branded newtype `BadNum` applies string constraints (pattern, minLength,
       maxLength) to a numeric inner type, which cannot carry them: ...
 --> src/lib.rs:4:19
  |
4 | pub struct BadNum(pub u64);
  |                   ^^^
```

An inner type the macro cannot resolve — another brand, a user type, a generic parameter — is
admitted here and checked for `Display` instead, so a non-`Display` one is still reported at the
field:

```rust
#[model_schema(no_display)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Tags(pub Vec<String>);

// This will NOT compile either: `Tags` has no `Display` for `validate()` to render.
#[model_schema(pattern = "^[a-z]+$")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TagsBrand(pub Tags);
```

```text
error[E0277]: `Tags` doesn't implement `std::fmt::Display`
  --> src/lib.rs:12:26
   |
12 | pub struct TagsBrand(pub Tags);
   |                          ^^^^ unsatisfied trait bound
```

#### Doc Comments and Examples on Branded Newtypes

Branded newtypes support doc comments (for Zod `.meta({ description })`) and compiler-validated examples, just like structs and enums:

```rust
/// Generic document identifier.
///
/// - `DocumentId<String>` for API/HTTP layer
/// - `DocumentId<ObjectId>` for MongoDB layer
///
/// ```rust example
/// DocumentId("64de3d95ff45b119e5b53a7e".to_string())
/// ```
#[model_schema()]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DocumentId<ID_TYPE>(pub ID_TYPE);
```

Generated Zod:

```typescript
const DocumentId$RawSchema = z.string().brand<"DocumentId">().meta({
  description: "Generic document identifier.\n...",
  example: "64de3d95ff45b119e5b53a7e",
});

export const DocumentId$Schema: $ZodBranded<ZodString, "DocumentId"> = DocumentId$RawSchema;
```

## Field Validation (`model_schema_prop`)

Use `#[model_schema_prop(...)]` on individual fields to add validation constraints, override types, or apply Zod preprocessing. Constraints are enforced in both Zod (frontend) and a generated `validate()` method (Rust).

### String Constraints (minLength, maxLength, pattern)

```rust
use tixschema::{model_schema, model_schema_prop};
use serde::{Deserialize, Serialize};

#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct UserProfile {
    #[model_schema_prop(minLength = 3, maxLength = 30, pattern = "^[a-z0-9_]+$")]
    pub username: String,

    #[model_schema_prop(maxLength = 200)]
    pub bio: String,

    #[model_schema_prop(pattern = "^[0-9a-fA-F]{24}$")]
    pub external_id: String,
}
```

Generated Zod for `username`: `z.string().min(3).max(30).check(z.regex(/^[a-z0-9_]+$/))`

Generated JSON Schema for `username`: `{ "type": "string", "minLength": 3, "maxLength": 30, "pattern": "^[a-z0-9_]+$" }`

The TypeScript type is unchanged -- still just `string`.

A type whose schema this crate writes whole carries none of the five constraints, and writing one on
such a field is a compile error naming the keys and the type: `ObjectId` writes a `{"$oid": "..."}`
object rather than a string, and the chrono types (`NaiveDate`, `NaiveTime`, `NaiveDateTime`,
`DateTime<Tz>`) write their own ISO spellings, which no surface reads a length, a pattern or a range
beside. Carry the value in a `String` field if it needs one.

A `PathBuf` field carries the same three constraints, as does the `Path` borrow behind a wrapper (`Box<Path>`, `Cow<'_, Path>`, `Arc<Path>`, `Rc<Path>`): serde writes a path as a JSON string, which is what the three surfaces render a constrained string for. The checks measure that string -- the path's `to_string_lossy` rendering, which is the exact wire value for every path serde can write, a path that is not UTF-8 being one serde refuses to serialize at all.

#### What a `pattern` may contain

One `pattern` string reaches three readers: the generated Rust validator builds it with `regex::Regex::new`, the Zod schema splices it between `/` delimiters as a regex literal, and the JSON Schema `pattern` keyword is an ECMA-262 regex. A pattern is accepted only if all three read it, and read it the same way; otherwise the derive fails at expansion with the construct named.

**Engine baseline: ES2018.** The emitted Zod literals and JSON Schema patterns are written for an ECMA-262 engine at ES2018 or newer, and the guard's admissions are decided against that version rather than against whichever JavaScript runtime happens to be installed where the derive runs -- the schema is read wherever it is loaded, not where it was generated. ES2018 is what the crate's own output already assumes: it emits `(?<name>...)` named capture groups, which are ES2018. Nothing it emits needs anything newer.

Zod literals are spliced without flags, so the JavaScript side is always non-Unicode mode.

Some constructs are **translated** on the way out, to a spelling that means the same thing to all three readers. The translation is what every surface receives, the Rust validator included, so the three never validate different sets:

| You write | Every surface receives | Why |
|---|---|---|
| `(?P<name>...)` | `(?<name>...)` | One group under two spellings; the `regex` crate reads both, JavaScript reads only the second. |
| `\d` | `[0-9]` | The `regex` crate reads `\d` as the Unicode digit class, a flagless literal as ASCII -- so `^\d+$` would accept `١` in Rust and reject it in the browser. |
| `\w` | `[0-9A-Za-z_]` | Same divergence: the `regex` crate counts `é` and `α` as word characters, a flagless literal does not. |
| `\s` | `[\t\n\v\f\r ]` | The two whitespace sets are not even nested -- the `regex` crate spaces U+0085 and JavaScript does not, JavaScript spaces U+FEFF and the `regex` crate does not -- leaving the ASCII run as the only common ground. |

A class you already wrote out is left exactly as you wrote it, byte for byte.

Some constructs are **refused**, because no spelling makes the readers agree:

- `.`, and the negated classes `\D`, `\W`, `\S`. A flagless JavaScript literal matches one UTF-16 code unit where the `regex` crate matches one character, so a single character outside the Basic Multilingual Plane -- an emoji, say -- fills `^.$` in Rust and never in JavaScript. Writing the members out settles *which* characters are named and cannot settle how many code units one of them is. Name the characters you mean instead.
- `(?i:...)`, `(?m:...)` and `(?s:...)`, and their negated and combined forms. These are ECMA-262 regular expression modifiers, which post-date the ES2018 baseline; an engine at the baseline throws a `SyntaxError` where the schema loads. Recent runtimes do parse them, which is exactly why the baseline is a recorded decision rather than a measurement.
- `(?i)`, `(?x:...)`, `(?U:...)`, `(?u:...)`, `(?R:...)` -- inline flag directives and flags ECMA-262 never had.
- `\A`, `\z`, `\b{start}`, `\<`, `\>` -- anchors and boundaries JavaScript reads as escaped letters. Use `^` and `$`.
- `\p{...}`, `\pL`, `\P{...}` and POSIX `[:alpha:]` -- Unicode and POSIX classes a flagless literal reads as ordinary characters.
- `&&`, `--`, `~~` class operators and a class nested inside a class -- set operations JavaScript reads as class members.
- `\x{...}`, `\u{...}`, `\U...` and octal escapes -- code point escapes JavaScript reads differently or not at all. Write the character.
- An unescaped `]` opening a class, as in `[]-a]`. This one is refused rather than escaped because escaping it changes the meaning: `[]-a]` is the three members `]`, `-` and `a`, and `[\]-a]` is a range.

### Numeric Constraints (minimum, maximum)

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct Product {
    #[model_schema_prop(minimum = 0, maximum = 120)]
    pub age_restriction: u32,

    #[model_schema_prop(minimum = 0.0)]
    pub price: f64,
}
```

### The `validate()` Method

When any field carries a constraint, the macro generates a `validate(&self) -> Result<(), Vec<String>>` method. Use this to validate instances constructed directly in Rust code (serde deserialization validates automatically on the way in).

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct Registration {
    #[model_schema_prop(minLength = 3, maxLength = 30)]
    pub username: String,

    #[model_schema_prop(minimum = 0, maximum = 120)]
    pub age: u32,
}

let reg = Registration { username: "ab".to_string(), age: 150 };

match reg.validate() {
    Ok(()) => println!("valid"),
    Err(errors) => {
        for e in &errors {
            println!("Error: {e}");
        }
        // "username: too short (minimum length 3, got 2)"
        // "age: too large (maximum 120, got 150)"
    }
}
```

The macro also generates into the type's schema module:
- `validate_{field}_value(&FieldType) -> Result<(), String>` -- pure static validator per field
- `deserialize_{field}(D) -> Result<FieldType, E>` -- serde hook that calls the static validator

A constrained field of an enum variant is named for its variant too -- `validate_{variant}_{field}_value` and `deserialize_{variant}_{field}`, with `{variant}` in `snake_case`. One schema module holds every variant's helpers, and a field name is unique only within the variant that declares it, so two variants naming one field carry their own constraints.

#### Constraints under `Option`, wrappers, and sequences

A constraint describes the value the field puts on the wire, wherever it was written. `validate()` therefore reaches through everything the parser reads through -- an `Option`, a transparent wrapper (`Box`, `Rc`, `Arc`, `Cow`), and every sequence level (`Vec`, `VecDeque`, `HashSet`, `BTreeSet`, `LinkedList`, `BinaryHeap`, arrays and slices) -- and checks the innermost value, which is the same place the Zod, TypeScript and JSON Schema surfaces put it.

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct Article {
    #[model_schema_prop(minLength = 3)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,   // checked inside the Some; a None is nothing to check

    #[model_schema_prop(minLength = 3)]
    pub tags: Vec<String>,          // checked per element, once per failing tag
}
```

Deserialization applies the same reach. A bare field's hook answers for the constrained value itself; a wrapped field's hook deserializes the field's own declared type and then runs that same walk over it, so a payload carrying a value the constraint rejects is rejected as it is read, not only when `validate()` is called. The two differ in one thing: `validate()` answers with every violation in the instance, while a `Deserializer` answers with one error, so the read stops at the first.

A field whose key may be left out keeps that reading: an `Option` written outermost (under any number of transparent wrappers) is given `#[serde(default)]` alongside the hook, since a `deserialize_with` otherwise turns a missing key into an error. A field that writes its own `default` keeps the one it wrote.

### Literal Values

You can constrain a `String` field to a specific literal value using `model_schema_prop(literal = "value")`. This generates `z.literal("value")` in the Zod schema and a literal type in TypeScript, while the Rust field remains a `String`.

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Action {
    Generate {
        #[model_schema_prop(literal = "document")]
        value: String,
    },
}
```

Generated:

```typescript
export type Action = {
  type: "generate";
  value: "document";
};

export const Action$Schema = z.discriminatedUnion("type", [
  z.strictObject({
    type: z.literal("generate"),
    value: z.literal("document"),
  }),
]);
```

**Recommended: Single-value enums over literal strings.** While `model_schema_prop(literal = ...)` works, a single-value enum provides type safety in Rust -- the field can only hold the correct value at compile time, whereas a `String` with a literal annotation can hold any string in Rust (the constraint only applies in the generated TypeScript/Zod output).

```rust
// Correct: single-value enum
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub enum DocumentLiteralValue {
    #[serde(rename = "document")]
    Document,
}

#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Action {
    Generate {
        value: DocumentLiteralValue,  // Type-safe in Rust
    },
}

// Wrong: not recommended
#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ActionAlt {
    Generate {
        #[model_schema_prop(literal = "document")]
        value: String,  // Any string value accepted in Rust
    },
}
```

Both approaches produce identical TypeScript and Zod output. The single-value enum is preferred because:

- **Type safety in Rust** -- impossible to construct with a wrong value
- **Self-documenting** -- the enum name makes the intent clear
- **Pattern matching** -- match on `DocumentLiteralValue::Document` instead of checking string equality
- **Naming convention**: Use the `<Something>LiteralValue` naming pattern (e.g., `DocumentLiteralValue`)

### Type Names (`as`)

Use `as` to name the type a field renders. The target must be the type the field already renders --
either the field's own type, or the value under its wrappers, so `as = String` is written on a
`String`, an `Option<String>` and a `Vec<String>` alike:

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct ApiConfig {
    pub id: String,
    #[model_schema_prop(as = String)]
    pub metric_type: String,
    #[model_schema_prop(as = String, minLength = 1)]
    pub tags: Vec<String>,
    pub enabled: bool,
}
```

Naming any other type is a compile error. The key cannot override the emitted type: all three
surfaces are written from the field's declared type because that is the type serde reads and writes,
and a `serialize_with` names a function whose output the expansion cannot see -- so a target that
rendered differently would describe a payload serde never produces. `as` also cannot be written
beside `preprocess`; the two have no defined order.

### Zod Preprocessing

`preprocess = ["fn1", "fn2"]` wraps the Zod schema with `z.preprocess()` calls. This is Zod-only -- no effect on Rust types or serde deserialization. Multiple preprocessors are applied as nested calls.

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct Event {
    // generates: z.preprocess(epochToDate, z.string())
    #[model_schema_prop(preprocess = ["epochToDate"])]
    pub created_at: String,

    // generates: z.preprocess(trim, z.preprocess(normalize, z.string()))
    #[model_schema_prop(preprocess = ["trim", "normalize"])]
    pub name: String,
}
```

### Optional TypeScript Keys (`ts_optional`)

By default an `Option<T>` field renders as a required key carrying `| undefined` (`field: T | undefined`). Add the bare `ts_optional` flag to render it as an optional key instead (`field?: T`).

This is a TypeScript-only knob -- the Zod schema and JSON Schema are unchanged (the field is already optional in both). The flag is only valid on `Option<T>` fields; applying it to a non-`Option` field is a compile error. It composes with `as = Type`, which names the type the field already renders.

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[model_schema_prop(ts_optional)]
    pub nickname: Option<String>,
}
```

Generated TypeScript:

```typescript
export type Profile = {
  name: string;
  nickname?: string;
};
```

Without `ts_optional`, `nickname` would render as `nickname: string | undefined`.

## Compiler-Validated Examples

You can provide compiler-validated example values for your types that will be embedded in the generated Zod schemas' `.meta()` field. Examples are written in doc comment code blocks and fully type-checked at compile time.

```rust
/// User profile
/// ```rust example
/// User {
///     name: "John Doe".to_string(),
///     email: "john@example.com".to_string(),
///     age: 25,
/// }
/// ```
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub name: String,
    pub email: String,
    pub age: u32,
}
```

Generated Zod:

```typescript
export const User$Schema: ZodType<User> = z.strictObject({
  name: z.string(),
  email: z.string(),
  age: z.number().int(),
}).meta({
  example: { name: "John Doe", email: "john@example.com", age: 25 }
});
```

You can include arbitrary setup code before the example value:

```rust
/// User with tags
/// ```rust example
/// let user_id = "usr_123".to_string();
/// let tags = vec!["admin".to_string(), "active".to_string()];
/// User {
///     id: user_id,
///     tags,
///     age: 30,
/// }
/// ```
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub id: String,
    pub tags: Vec<String>,
    pub age: u32,
}
```

Enum examples work similarly:

```rust
/// Data type enumeration
/// ```rust example
/// DataType::Numeric
/// ```
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DataType {
    Alphanumeric,
    Numeric,
    Date,
}
```

Key points:

- Examples are **optional** -- if not provided, no example is generated.
- Use the exact syntax: ` ```rust example` (note the space and `example` keyword).
- If multiple examples are present, only the **first one** is used.
- Examples respect Serde attributes (field renaming, etc.).
- Being Rust source, the example block is dropped from the JSDoc comment above the generated `export type`, whatever the type is declared as. On a struct or an enum it reaches the Zod `example` field; on a type alias, which publishes no `example` field, it reaches no generated surface at all.
- The example code is executed at compile time and serialized to JSON.
- Wrong types produce compile errors, ensuring examples stay in sync with your types.

## MongoDB ObjectId Support

Enable the `object_id` feature for first-class MongoDB ObjectId support with proper serialization and validation.

```rust
use tixschema::model_schema;
use serde::{Deserialize, Serialize};
use mongodb::bson::oid::ObjectId;

#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct Document {
    pub id: ObjectId,
    pub title: String,
    pub author_id: ObjectId,
    pub tags: Vec<ObjectId>,
    pub metadata: HashMap<String, ObjectId>,
    pub parent_id: Option<ObjectId>,
    pub related_docs: HashMap<String, Vec<ObjectId>>,
}
```

Generated TypeScript:

```typescript
export type Document = {
  id: ObjectId;
  title: string;
  author_id: ObjectId;
  tags: Array<ObjectId>;
  metadata: Partial<Record<string, ObjectId>>;
  parent_id: ObjectId | undefined;
  related_docs: Partial<Record<string, Array<ObjectId>>>;
};

export const Document$Schema = z.strictObject({
  id: z.object({ $oid: z.string().regex(/^[a-f\d]{24}$/i, { message: "Invalid ObjectId" }) }),
  title: z.string(),
  author_id: z.object({ $oid: z.string().regex(/^[a-f\d]{24}$/i, { message: "Invalid ObjectId" }) }),
  tags: z.array(z.object({ $oid: z.string().regex(/^[a-f\d]{24}$/i, { message: "Invalid ObjectId" }) })),
  metadata: z.record(z.string(), z.object({ $oid: z.string().regex(/^[a-f\d]{24}$/i, { message: "Invalid ObjectId" }) })),
  parent_id: z.union([z.object({ $oid: z.string().regex(/^[a-f\d]{24}$/i, { message: "Invalid ObjectId" }) }), z.undefined()]).prefault(undefined),
  related_docs: z.record(z.string(), z.array(z.object({ $oid: z.string().regex(/^[a-f\d]{24}$/i, { message: "Invalid ObjectId" }) }))),
});
```

ObjectIds serialize to MongoDB's standard JSON format:

```json
{
  "id": { "$oid": "507f1f77bcf86cd799439011" },
  "title": "My Document",
  "author_id": { "$oid": "507f1f77bcf86cd799439012" },
  "tags": [
    { "$oid": "507f1f77bcf86cd799439013" },
    { "$oid": "507f1f77bcf86cd799439014" }
  ]
}
```

Key details:

- Uses `{ "$oid": "hex_string" }` format matching MongoDB's native serialization.
- Validates 24-character hexadecimal ObjectId strings via regex.
- Supports ObjectIds in arrays, HashMaps, optional fields, and deeply nested structures.
- The MongoDB crate is a dev-dependency only -- zero production overhead.

## Chrono Date/Time Types

Enable the `chrono` feature for chrono date/time type support. All chrono types map to `string` in TypeScript, with appropriate Zod validation for the specific date/time format.

```toml
tixschema = { features = ["chrono"] }
```

Supported types and mappings:

| Rust Type | TypeScript | Zod Schema | JSON Schema Format |
|-----------|------------|------------|--------------------|
| `NaiveDate` | `string` | `z.iso.date()` | `"date"` |
| `NaiveTime` | `string` | `z.preprocess(<millis arrow>, z.iso.time())` | `"time"` |
| `NaiveDateTime` | `string` | `z.iso.datetime({ local: true })` | `"date-time"` |
| `DateTime<Tz>` (default) | `Date` | `z.coerce.date()` | `"date-time"` |
| `DateTime<Tz>` + `#[model_schema_prop(as_number)]` | `number` | `z.preprocess(<epoch arrow>, z.number())` | `"date-time"` |

`DateTime<Tz>` renders as a native TypeScript `Date` (`z.coerce.date()`) by default, which is what MongoDB needs to expire a BSON `Date` via TTL. The bare `as_number` flag opts a single `DateTime<Tz>` field into an epoch-milliseconds `number` instead, validated by a self-contained inline coercer (no imported helper). `as_number` is only valid on a `DateTime<Tz>` field — using it elsewhere is a compile error.

`NaiveTime` stays a TypeScript `string`, but its Zod schema also accepts millis-since-start-of-day, converting them to an `HH:MM:SS` string before validation.

Example:

```rust
use tixschema::{model_schema, model_schema_prop};
use serde::{Deserialize, Serialize};
use chrono::{NaiveDate, NaiveTime, NaiveDateTime, DateTime, Utc};

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Event {
    pub name: String,
    pub date: NaiveDate,
    pub time: NaiveTime,
    pub local_datetime: NaiveDateTime,
    pub created_at: DateTime<Utc>,
    #[model_schema_prop(as_number)]
    pub epoch_ms: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}
```

Generated TypeScript:

```typescript
export type Event = {
  name: string;
  date: string;
  time: string;
  local_datetime: string;
  created_at: Date;
  epoch_ms: number;
  updated_at: Date | undefined;
};
```

Generated Zod:

```typescript
export const Event$Schema: ZodType<Event> = z.strictObject({
  name: z.string(),
  date: z.iso.date(),
  time: z.preprocess((arg) => { if (typeof arg === "number") { const s = Math.floor(arg / 1000); const hh = String(Math.floor(s / 3600)).padStart(2, "0"); const mm = String(Math.floor((s % 3600) / 60)).padStart(2, "0"); const ss = String(s % 60).padStart(2, "0"); return `${hh}:${mm}:${ss}`; } return arg; }, z.iso.time()),
  local_datetime: z.iso.datetime({ local: true }),
  created_at: z.coerce.date(),
  epoch_ms: z.preprocess((arg) => { if (arg instanceof Date) return arg.getTime(); if (typeof arg === "string") return Date.parse(arg); return arg; }, z.number()),
  updated_at: z.union([z.coerce.date(), z.undefined()]).prefault(undefined),
});
```

Chrono types also work in collections (`Vec<NaiveDate>` generates `z.array(z.iso.date())`) and in enums as tuple variant elements (`as_number` is honored on a tuple-variant `DateTime<Tz>` payload).

## Feature Flags

The crate uses optional features to control code generation and dependencies. All features can be independently enabled or disabled.

| Feature | Default | Description |
|---------|---------|-------------|
| `serde` | Yes | Serde attribute parsing (`rename`, `rename_all`, `tag`, etc.) |
| `zod` | Yes | Zod v4 schema generation alongside TypeScript types |
| `jsonschema` | Yes | JSON Schema generation via `json_schema()` method |
| `typescript` | Yes | TypeScript type generation via `ts_definition()` method |
| `object_id` | No | MongoDB ObjectId type support with validation |
| `chrono` | No | Chrono date/time type support (`NaiveDate`, `NaiveTime`, `NaiveDateTime`, `DateTime<Tz>`) |

Common configurations:

```toml
# Default (serde + zod + jsonschema + typescript)
tixschema = "0.1.0"

# All features including optional ones
tixschema = { features = ["serde", "zod", "jsonschema", "typescript", "object_id", "chrono"] }

# Minimal (TypeScript only, no Zod or JSON Schema)
tixschema = { default-features = false, features = ["typescript"] }

# TypeScript + Zod without JSON Schema
tixschema = { default-features = false, features = ["serde", "zod", "typescript"] }
```

All 2^6 = 64 feature combinations are tested in CI via `cargo-hack`.

## Generating TypeScript Files

Create a utility function to generate TypeScript files with all your types:

```rust
use std::fs;

pub enum MyEntities {}

impl MyEntities {
    pub fn get_entities() -> (String, Vec<String>) {
        (
            "Generated Types".to_string(),
            vec![
                User::ts_definition(),
                UserStatus::ts_definition(),
                PaymentMethod::ts_definition(),
                Address::ts_definition(),
            ],
        )
    }
}

pub fn generate_ts_schemas(target_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut file_contents = String::from("import { z } from \"zod\";\n\n");
    let (header, type_definitions) = MyEntities::get_entities();

    file_contents.push_str(&format!("/*\n * {}\n */\n\n", header));
    file_contents.push_str(&type_definitions.join("\n\n"));
    file_contents.push('\n');

    fs::write(target_path, file_contents)?;
    Ok(())
}

// Run generation as a test
#[test]
fn generate_typescript() {
    generate_ts_schemas("../frontend/src/types/generated.ts").unwrap();
}
```

### Alternatives

There are several ways to run the generation:

- **Test-based generation** (shown above): Run `cargo test generate_typescript` to produce the file. Simple and works well for most projects.
- **Binary crate**: Create a small binary that imports your types and calls `ts_definition()`. Run it with `cargo run --bin generate_types`.
- **`just` command**: Wrap `cargo test --test generation` in a justfile target for a convenient `just generate-ts` command.

**Note:** `build.rs` is not recommended for proc-macro libraries since the types are not available during the build script phase.

## Integration with Frontend

1. Run your TypeScript generation: `cargo test generate_typescript`
2. The generated file will include all your types and schemas
3. Import and use in your TypeScript/JavaScript code:

```typescript
import { z } from "zod";
import { User, User$Schema } from './types/generated';

// Runtime validation
const userData = User$Schema.parse(apiResponse);

// Type-safe usage
const user: User = {
  id: "123",
  name: "John Doe",
  email: "john@example.com",
  age: 30,
  is_active: true
};
```

You can also generate JSON schemas from the Zod schemas for API documentation or OpenAPI specs:

```typescript
import { generateSchema } from '@zod-schema/json-schema';
import { User$Schema } from './types/generated';

const jsonSchema = generateSchema(User$Schema);
```

## Serde Attribute Support

The macro respects Serde attributes when the `serde` feature is enabled:

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub user_id: String,       // becomes userId in TypeScript
    pub first_name: String,    // becomes firstName in TypeScript
    pub last_name: String,     // becomes lastName in TypeScript
    #[serde(rename = "emailAddress")]
    pub email: String,         // becomes emailAddress in TypeScript
    pub created_at: String,    // becomes createdAt in TypeScript
}
```

Supported Serde attributes:

- `#[serde(rename = "...")]` -- rename individual fields
- `#[serde(rename_all = "camelCase")]` -- rename all fields with a naming convention
- `#[serde(tag = "...")]` -- internally tagged enums: the variant's data is written beside the discriminator
- `#[serde(tag = "...", content = "...")]` -- adjacently tagged enums
- `#[serde(untagged)]` -- untagged enums generate a union (`A | B`) / Zod `z.union([...])` / JSON Schema `anyOf`
- `#[serde(flatten)]` -- flatten a field into the parent as an intersection type (`A & B`) / Zod `.and(...)`
- `#[serde(transparent)]` -- transparent wrappers (used for branded newtypes)
- `#[serde(skip_serializing_if = "...")]` -- respected but does not affect generated types

If the `serde` feature is disabled but serde attributes are present, you will see compile-time warnings and field names will not be transformed.

## Important Notes

1. **Naming Convention**: The `Json` suffix on Rust type names is optional. If present, it is automatically stripped in the generated TypeScript output (e.g., `UserJson` becomes `User`). If no `Json` suffix is present, the Rust type name is used as-is (e.g., `User` stays `User`).

2. **Type References**: Nested types reference each other by their TypeScript name (with the `Json` suffix stripped if present).

3. **Map Keys**: `HashMap` and `BTreeMap` are both supported, and the key decides what the map describes as: a `String` key gives an object open to any string key, and a plain `#[model_schema()]` enum key gives one closed to the enum's members. A brand and a `#[model_schema()]` alias answer by their inner and their target, keeping their own name on the TypeScript and Zod surfaces. A key path proved to be one serde writes as no key at all, and any sequence-wrapped key, is refused at expansion; every other key describes as an open object. See [Collections and Maps](#collections-and-maps).

4. **Array Types**: `Vec<T>` becomes `Array<T>` in TypeScript, one `Array<...>` per level written — `Vec<Vec<T>>` is `Array<Array<T>>`.

5. **Optional Fields**: `Option<T>` becomes `T | undefined` in TypeScript and `z.union([type, z.undefined()]).prefault(undefined)` in Zod v4.

6. **Complex Nesting**: The crate supports deeply nested structures including `HashMap<String, Vec<HashMap<String, T>>>` and similar patterns.

7. **Doc Comments**: Rust doc comments on types and fields are carried through to the generated TypeScript as JSDoc comments.

8. **Built-in Type Mappings**: Besides the primitives, `String` and `std::path::PathBuf` both map to `string` (`z.string()`), and `serde_json::Value` maps to `unknown` (`z.unknown()`). MongoDB `ObjectId` and the `chrono` date/time types are supported behind their respective feature flags.

## Error Handling & Troubleshooting

### Feature-Related Errors

#### ObjectId Errors

**Error:** `cannot find type 'ObjectId' in this scope`

**Cause:** Using `ObjectId` without the `object_id` feature enabled.

**Solutions:**
```toml
# Option 1: Enable the object_id feature
tixschema = { features = ["object_id"] }

# Option 2: Use full path in your code
# use mongodb::bson::oid::ObjectId;
```

#### JSON Schema Method Missing

**Error:** `no function or associated item named 'json_schema' found`

**Cause:** Calling `.json_schema()` without the `jsonschema` feature enabled.

**Solution:**
```toml
tixschema = { features = ["jsonschema"] }
```

#### Zod Schema Missing

**Symptom:** Generated TypeScript contains only types, no Zod schemas.

**Cause:** `zod` feature is disabled.

**Solution:**
```toml
tixschema = { features = ["zod"] }
```

#### Serde Attributes Ignored

**Symptom:** Field names not transformed (e.g., `user_id` instead of `userId`).

**Cause:** `serde` feature is disabled.

**Solution:**
```toml
tixschema = { features = ["serde"] }
```

### Compilation Errors

#### Map Keys Without Enumerable Members

**Error:** *a map key must be a plain `#[model_schema()]` enum, whose members become the object's keys — `<type>` resolves to a type with no `enum_members()`*, reported at the field; or `no associated function or constant named enum_members found for <type>` (`E0599`), reported at the map's key type.

A map key written as a type path must be something serde writes into a key: a plain `#[model_schema()]` enum, whose members become the object's keys; a `#[serde(transparent)]` brand or a `#[model_schema()]` alias whose wire form is one serde writes into a key; or a `String`. [Collections and Maps](#collections-and-maps) sets out what each of those describes as, and every key that is none of them is covered there too — this entry is about the two diagnostics a type path earns.

A key the expansion has already seen and knows serde writes as no key at all — a struct, a brand over one or over a container, a tagged or untagged enum, or a `#[model_schema()]` alias of one of those — is named directly, at the field that writes the map:

```rust
#[model_schema()]
pub struct Doc {
    pub id: String,
}

// Wrong: `Doc` is a struct, so it has no members for the object's keys
#[model_schema()]
pub struct BadCounts {
    pub counts: HashMap<Doc, u32>, // reported at the field, naming `Doc`
}
```

An alias resolves through to what it names, so an alias of a struct is refused under the alias's own name, and a `#[model_schema()]` alias whose target *is* such a map is refused at the alias.

Items expand in the order they are written, though, so a key declared after the type that writes the map, like one from another crate, has not been seen yet and is emitted as an enumerating key. When such a key carries no `enum_members()`, the same requirement surfaces as an `E0599` at the key type:

```rust
// Wrong: `LateKey` has not expanded yet, so it is emitted as an enumerating key — and it
// resolves to `String`, which has no members to enumerate
#[model_schema()]
pub struct BadConfig {
    pub slots: HashMap<LateKey, String>, // E0599, reported at `LateKey`
}

#[model_schema()]
pub type LateKey = String;

// Correct: the key is a plain enum, and its members become the object's keys
#[model_schema()]
pub enum Slot {
    Primary,
    Secondary,
}

// Correct: the alias expands first, so the registry knows serde writes it as a bare string
#[model_schema()]
pub type EarlyKey = String;

#[model_schema()]
pub struct Config {
    pub slots: HashMap<Slot, String>,
    pub names: HashMap<EarlyKey, String>,
}
```

A plain enum key works wherever it is declared: it carries `enum_members()`, so the emitted call resolves whichever order the two items expand in, and declaration order decides only which of the two diagnostics a key *without* members earns. Every other supported spelling — an alias of a string, a brand — is known only through the registry, so it has to expand before the type that keys a map by it; until then the expansion cannot tell it from a plain enum declared later, and emits the enumerating key it would need to be.

#### Sequence-Wrapped Map Keys

**Error:** *a map key must be a value serde writes as a string ... this key is a sequence of `<type>`*, reported at the field.

A JSON object key is a string. A sequence writes a JSON array, so `serde_json` refuses to serialize a map keyed by one at all — `key must be a string`, raised at serialization time, with no object and no array-of-pairs fallback. There is no wire form for a schema to describe, so the spelling is refused rather than described:

```rust
// Wrong: the key writes an array, which serde will not use as an object key
#[model_schema()]
pub struct BadCounts {
    pub counts: HashMap<Vec<Slot>, u32>,
}

// Correct: the key is the element itself, whose members become the object's keys
#[model_schema()]
pub struct Counts {
    pub counts: HashMap<Slot, u32>,
}
```

Every sequence spelling earns this — `Vec`, `[T; N]`, and the sets — the wrapper being what serde writes as an array. The message names the element rather than the wrapper, the parser having already collapsed those spellings onto their array levels. A sequence in the map's *value* is untouched: only the key has to be a string.

#### Map Keys serde Refuses to Write

**Error:** *a map key must be a value serde writes as a string ... serde writes `<type>` as a JSON array | a JSON object, and refuses to serialize a map keyed by one at all*, reported at the field.

The same rule as above, reached by the key's own type rather than by a wrapper around it. A tuple writes a JSON array; a nested map and an `ObjectId` write JSON objects. `serde_json` uses none of them as an object key — it raises `key must be a string` and refuses the whole map — so there is no wire form for a schema to describe:

```rust
// Wrong: each key writes an array or an object, which serde will not use as an object key
#[model_schema()]
pub struct BadCounts {
    pub by_pair: HashMap<(Slot, Slot), u32>,
    pub by_map: HashMap<BTreeMap<String, u32>, u32>,
    pub by_oid: HashMap<ObjectId, u32>,
}

// Correct: key by a value serde writes as a string
#[model_schema()]
pub struct Counts {
    pub by_slot: HashMap<Slot, u32>,
    pub by_id: HashMap<String, u32>,
}
```

The keys serde *does* stringify for you are untouched and stay open — numbers, `bool`, and the chrono types all describe as `{"type": "object", "additionalProperties": true}`, as [Collections and Maps](#collections-and-maps) sets out. The rule is refuse-what-serde-refuses, never refuse-what-is-not-a-`String`. A tuple or a map in the map's *value* is untouched too: only the key has to be a string.

#### `Option`-Wrapped Map Keys

**Error:** *a map key must be a value serde writes as a string ... this key is an `Option<T>`, whose `Some` serde writes as the bare `T` while a `None` has no string form at all and makes serde refuse the whole map; key it by `T`*, reported at the field.

An `Option` key is transparent for a `Some` — `HashMap<Option<Slot>, u32>` writes `{"Daily": 1}`, exactly what the bare-keyed map writes — and has no form at all for a `None`, which fails serialization with `key must be a string`. A schema describing only the `Some` half would validate documents the type can raise a runtime error on and say nothing about the half it cannot write, so the spelling is refused and the inner is named as the remedy:

```rust
// Wrong: a `None` key has no wire form, and one is enough to fail the whole map
#[model_schema()]
pub struct BadCounts {
    pub counts: HashMap<Option<Slot>, u32>,
}

// Correct: key by the inner, which is what every `Some` was already writing
#[model_schema()]
pub struct Counts {
    pub counts: HashMap<Slot, u32>,
}
```

An `Option` in the map's *value* is untouched: a map entry cannot be dropped the way an object key can, so a `None` value is written as `null` and described as such.

#### Function-Local Types

**Error:** `cannot find module or crate <type>_schema in this scope` (`E0433`), reported at the field's type.

A referenced type must be declared at item scope. A type declared inside a function body publishes its schema module inside that same body, where the `use super::*` of the referencing type's own module never reaches it:

```rust
// Wrong: `Inner` is declared inside the function body
#[test]
fn builds_the_schema() {
    #[model_schema()]
    pub struct Inner {
        pub id: String,
    }

    #[model_schema()]
    pub struct Outer {
        pub inner: Inner, // E0433: cannot find module or crate `inner_schema` in this scope
    }
}

// Correct: `Inner` is declared at item scope; `Outer` may stay in the function body
#[model_schema()]
pub struct Inner {
    pub id: String,
}

#[test]
fn builds_the_schema() {
    #[model_schema()]
    pub struct Outer {
        pub inner: Inner,
    }
}
```

#### Missing Derives

**Error:** Various compilation errors related to traits.

Always include required derives:
```rust
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MyType {
    // fields...
}
```

### Runtime Issues

#### Missing TypeScript Types

**Error:** `Cannot find name 'ObjectId'` in TypeScript.

**Solution:** Define the ObjectId type in your TypeScript project:
```typescript
// types/mongodb.ts
export interface ObjectId {
  $oid: string;
}
```

#### JSON Schema Validation Failures

**Problem:** Data doesn't validate against generated schemas.

Common causes:

1. **Field naming mismatch**: Check serde attributes and feature flags.
2. **Optional field handling**: Ensure consistent `Option<T>` usage.
3. **ObjectId format**: Must be `{ "$oid": "hex_string" }` format.

Debug by printing generated output:
```rust
println!("{}", MyType::ts_definition());

#[cfg(feature = "jsonschema")]
println!("{}", serde_json::to_string_pretty(&MyType::json_schema()).unwrap());
```

## Zod v4 Compatibility

This library generates Zod v4 compatible schemas exclusively. The key difference from Zod v3 is how optional fields are handled:

Zod v4 (generated by this library):
```typescript
export const User$Schema = z.strictObject({
  id: z.string(),
  name: z.string(),
  email: z.union([z.string(), z.undefined()]).prefault(undefined),
  age: z.union([z.number().int(), z.undefined()]).prefault(undefined),
});
```

Zod v3 style (not supported):
```typescript
// This format is NOT generated and will not work
export const User$Schema = z.strictObject({
  id: z.string(),
  name: z.string(),
  email: z.string().optional(),
  age: z.number().int().optional(),
}).transform(args => Object.assign(args, {
  email: args.email,
  age: args.age
}));
```

Benefits of the Zod v4 approach:

- **JSON Schema generation**: Zod v4 can generate JSON schemas directly from the validation schemas.
- **Cleaner code**: No complex transform functions needed.
- **Better performance**: Eliminates runtime transform overhead.
- **Type safety**: Maintains the same `T | undefined` TypeScript semantics.
