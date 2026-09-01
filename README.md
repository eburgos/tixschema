# TixSchema

A Rust procedural macro library that generates TypeScript type definitions and Zod v4 validation schemas from Rust structs and enums, and whole [service definitions](#services) -- messages, clients and dispatchers, in both languages -- from Rust traits, ensuring type safety and consistency between Rust backends and TypeScript frontends.

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

Any Rust struct annotated with `#[model_schema()]` generates a corresponding TypeScript type and Zod schema. Primitive types map as follows: `String` to `string`, `bool` to `boolean`, all numeric types to `number` (integers get `.int()`), `Option<T>` to `T` under a key the omission attribute decides ([Optional Fields](#optional-fields)), `Vec<T>` to `Array<T>`, and `HashMap<String, T>` to `Partial<Record<string, T>>`.

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

### Published Names (`name`)

A type publishes under the Rust ident it is declared with, spelled exactly as written. Nothing is read off that spelling -- no suffix is taken off it and no part of it is rewritten -- so `UserData` publishes as `UserData` on all three surfaces.

`#[model_schema(name = "...")]` is the one way to publish under something else. It moves the name everywhere at once: the `export type` line, the Zod consts, the `$defs` key a self-referential document hoists under, and every reference another type writes.

```rust
#[model_schema(name = "ContextValue")]
#[derive(Serialize, Deserialize)]
pub struct ContextValueData {
    pub label: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct Holder {
    pub value: ContextValueData,
}
```

Generated TypeScript (`JSDoc` blocks elided):

```typescript
export type ContextValue = {
  label: string;
};

export type ContextValueData = ContextValue;

export type Holder = {
  value: ContextValue;
};
```

The `export type ContextValueData = ContextValue;` line is the one place the Rust ident is written on purpose. A type declared *above* the renamed one has only that ident to name it by -- an override is not recoverable from it -- so both spellings answer, and the alias carries the one type. The Zod surface publishes the same pair, `export const ContextValueData$Schema = ContextValue$Schema;`, and the Rust module the schema lives in is named from the ident for the same reason: an override never moves it.

A type alias is the one shape with no surface name of its own, and publishes under a `Type` suffix -- `pub type Slug = String;` as `SlugType` -- unless `name` says otherwise. See [Type Aliases](#type-aliases).

Two declarations cannot publish under one name. The emitted types, schemas and definitions are one flat namespace, so a second declaration reaching a name already taken is refused at expansion, in every feature configuration, naming both declarations:

```text
error: model_schema: type `Beta` publishes as `Shared`, which type `Alpha` already publishes as
       -- one name cannot carry two declarations, whose types, schemas and definitions would
       overwrite each other. Give one of them a `#[model_schema(name = "...")]` of its own
```

### Optional Fields

`Option<T>` fields validate as `z.union([z.null().transform(() => undefined), type, z.undefined()]).prefault(undefined)` in Zod v4 and are left out of the JSON Schema's `required` list, whose `anyOf: [type, { "type": "null" }]` describes the same two spellings serde reads into `None`. The `.prefault(undefined)` makes the field default to `undefined` when omitted from the input, and the leading `z.null()` arm coerces an explicit `null` to the same `undefined`.

TypeScript writes every `Option<T>` field as `field: T | undefined`. The one thing that writes the other spelling, `field?: T`, is [`#[model_schema_prop(ts_optional)]`](#ts_optional) on the author's word. A serde attribute that drops the key decides what reaches the wire — the JSON Schema's `required` list and the Zod schema both read it — and never which of the two TypeScript spellings is written. [`#[model_schema_prop(nullable)]`](#nullable-object-keys-nullable) writes a third spelling, `field: T | null`, for the field whose `None` reaches the wire as an explicit `null` rather than an absent key.

Each flavor is pinned to the one shape its declared type describes, and the `serde` feature enforces the pin. A bare `Option<T>` field is a compile error unless it carries a key-dropping attribute (`skip_serializing_if`, `skip_serializing`, or `skip`) — its declared shape is `T | undefined`, so a `None` has to leave the key out, not reach the wire as `null`. `nullable` is refused the opposite way: pairing it with a key-dropping attribute is a compile error too, since the flag declares the key always written. Either violation names the field and the attribute to add or remove. Deserialization does not enforce either pin: `{}` and `{"a":null}` both read into `None` under both flavors, so an optional key accepts an explicit `null` on input even on a field whose own serializer never writes one.

A bare `#[serde(skip)]` is not that. serde writes the key into no payload *and* throws it away out of every payload that supplies one, so there is nothing on the wire for any surface to describe: TypeScript writes no member, Zod no key, and the JSON Schema neither a `properties` entry nor a `required` one. `#[serde(skip_serializing, skip_deserializing)]` is the same wire spelled out and is answered the same way. Note what this costs on the way in — a `z.strictObject` and an `additionalProperties: false` both *reject* a payload carrying that key, while serde accepts such a payload and discards the value. The schemas describe the payload serde writes, and that key appears in none of them; a member under an optional key would instead claim the key is sometimes written, and would describe a value nothing ever reads.

`#[serde(skip_deserializing)]` on its own drops neither surface's key: serde writes it in every payload, so the member keeps a required key, and the value a payload supplies under it is discarded on the way in.

A positional slot answers by that same wire, read in the place a tuple writes rather than under a key. A slot carrying a bare `#[serde(skip)]` is absent from the array serde writes *and* from every array serde reads, and the slots behind it move up: `struct Pair(#[serde(skip)] Option<String>, String)` holding `(Some("s"), "x")` writes `["x"]`, reads that back, and refuses `["s","x"]`. So the described tuple is the slots that remain — `[string]` in TypeScript, `z.tuple([z.string()])` in Zod, and one `prefixItems` entry under `minItems`/`maxItems` of 1. Dropping every slot leaves `[]`, which is the array serde writes there.

A slot dropped from only one of the two directions has no such description and is refused where it is declared. `skip_serializing` alone writes `["x"]` and reads only `["s","x"]`; `skip_deserializing` alone writes `["s","x"]` and reads only `["x"]`. A named field in that position is still describable — the key is absent from one payload and present in the other, which an optional key covers — but a slot is written by its place, so the two payloads differ in their arity and no fixed-arity tuple describes both.

A struct declaring a single slot keeps it whatever these attributes say: serde writes and reads a newtype struct's only slot regardless, so nothing there is dropped.

A tuple *variant*'s slots read the same way, with one difference at the lone-slot arity. `enum E { One(#[serde(skip)] String, u32) }` holding `("s", 7)` writes `{"One":[7]}`, reads that back, and refuses `{"One":["s",7]}`, so the described tuple shrinks exactly as a struct's does — under the content key when one is named, and to `{"One":[]}` when every slot is dropped. Where a variant differs is that it has its own name to fall back on: a variant declaring one slot and dropping it is written as a unit variant, `"One"` externally, `{"type":"One"}` under a tag and `null` untagged, and each surface describes it as the unit it has become. Because serde does read a variant slot's attributes at every arity, the one-directional halves are refused there at every arity too — `One(#[serde(skip_serializing)] String)` writes `"One"` and reads only `{"One":"s"}`.

Which key a field writes is read off the attribute in every build, `serde` feature or not — one declaration describes one wire under every toggle. What the feature buys is the renaming, the tagging and the guards.

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct UserWithOptionals {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}
```

Generated TypeScript:

```typescript
export type UserWithOptionals = {
  id: string;
  name: string;
  email: string | undefined;
  phone: string | undefined;
  avatar_url: string | undefined;
};
```

### Collections and Maps

`Vec<T>` becomes `Array<T>`, and so does every other std wrapper serde writes as a JSON array of its element: `VecDeque<T>`, `BinaryHeap<T>`, `HashSet<T>`, and `BTreeSet<T>`. Each is that array on the wire, so each is typed and validated as that array — the element decides what the array holds. Nesting is kept at whatever depth it is written: a `Vec<Vec<T>>` (or `HashSet<Vec<T>>`, or any other mix of those wrappers) writes an array of arrays, so it becomes `Array<Array<T>>`, `z.array(z.array(...))`, and a JSON schema whose `items` is itself an array.

`LinkedList<T>` is the one std sequence left out. It writes that same array, so the covered spellings already describe every wire form it has, and a type written with one is refused at compile time with the rewrite named: use `Vec<T>`, or `VecDeque<T>` where values are pushed at both ends.

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<HashMap<String, String>>,
}
```

`HashMap<K, V>` and `BTreeMap<K, V>` are both read as maps, and the key decides what the map describes as. A `String` key is the open case: any string is a key, so the JSON schema puts the value's schema under `"additionalProperties"`, TypeScript writes `Partial<Record<string, V>>`, and Zod writes `z.record(z.string(), V)`.

A key written as a plain `#[model_schema()]` enum is the narrowed case: the enum's members become the object's properties and nothing else is allowed, so the JSON schema lists each member with the value's schema and closes the object with `"additionalProperties": false`, while TypeScript and Zod name the enum itself as the key type — `Partial<Record<Slot, V>>` and `z.partialRecord(Slot$Schema, V)`. Zod's constructor is `z.partialRecord` because `z.record` over a key whose schema enumerates its members demands every one of them, while a `HashMap` holds any subset and serde writes the subset; a key outside the enumeration is refused either way. The members are named as serde writes them, so a `#[serde(rename_all = "...")]` on the enum renames the properties too. This holds wherever the enum is declared relative to the type that writes the map, and through a `#[model_schema()]` alias of one.

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

A brand adds a name to its inner's wire and nothing else, so it keys a map however its inner keys one — at every link of a chain of brands. A brand over a **plain enum** is the open case on the structural surface: serde writes the variant name, which is a bare string, and the brand publishes no `enum_members()` of its own for a schema to close the object over, so it opens the object under its own name rather than closing it over members nothing can supply. Its Zod binding is a different matter — `.brand()` is type-level only, so what it publishes is the enum's own `z.enum`, which enumerates, and the map is written `z.partialRecord(SlotBrand$Schema, V)`. A brand over a value serde **stringifies** — a number, a chrono `Naive*` type — describes exactly as that bare inner describes, the open object with nothing said about its members, while TypeScript and Zod keep the brand's name as the key type. A brand over a `bool` or a `DateTime<Tz>` is the one place that name has nowhere to land: `boolean` and `Date` spell no TypeScript property key, and the only binding such a brand publishes is the value-shaped schema Zod refuses or rewrites in key position — so the key renders in the form serde writes it, exactly as the bare inner does, and `HashMap<Enabled, V>` is `Partial<Record<"true" | "false", V>>` and `z.partialRecord(z.enum(["true", "false"]), V)`. Only a brand over something serde writes as no key at all — a struct, a container, a tuple, a nested map, an `ObjectId` — is refused, and so is a brand over an inner this expansion has not classified yet: what serde writes for such an inner is exactly what cannot be told.

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

Every other key is neither open nor enumerable, and none of them is refused: serde stringifies each one into a key for you. The JSON schema describes such a map as an object and says nothing about its members — `{"type": "object", "additionalProperties": true}` — whatever the key. serde writes the object with the key's string form for its keys: `7` becomes `"7"`, `true` becomes `"true"`, a chrono `NaiveDate` its ISO rendering. Only the narrowing is missing, not the object. The rule the refusals apply is refuse-what-serde-refuses, never refuse-what-is-not-a-`String`.

On the two nominal surfaces such a key keeps its own type wherever that type spells a TypeScript property key, and renders in the form serde **writes** it wherever it does not:

| key | TypeScript | Zod |
|-----|------------|-----|
| `u8`–`i64`, `usize`, `isize` | `number` | `z.record(z.number().int(), V)` |
| `NaiveDate` / `NaiveTime` / `NaiveDateTime` | `string` | the type's own schema |
| `bool` | `"true" \| "false"` | `z.partialRecord(z.enum(["true", "false"]), V)` |
| `DateTime<Tz>` | `string` | `z.record(z.iso.datetime({ offset: true }), V)` |

So a `HashMap<u32, String>` is `Partial<Record<number, string>>` and `z.record(z.number().int(), z.string())`, and a `HashMap<bool, String>` is `Partial<Record<"true" | "false", string>>` and `z.partialRecord(z.enum(["true", "false"]), z.string())`. `Record<K, V>` is declared `K extends keyof any`, which `boolean` and `Date` do not satisfy, and in key position `z.boolean()` rejects the `"true"` serde wrote while `z.coerce.date()` accepts the timestamp only to rewrite every key into a locale-dependent `Date.prototype.toString()` rendering — neither surface can carry the value form, so both carry the wire form instead. The constructor moves with the key because `z.record` over an enumerated key demands every one of its members; `z.partialRecord` (zod ≥ 4.0.0) admits a map holding one of them, or none. A brand or alias over one of those keys renders exactly as the bare key does, its name having nothing to stand in.

A key written as one of the enclosing item's own type parameters is the open case, read off that same rule. The expansion cannot see which type the instantiation will supply, but serde has already settled what any of them may write: an instantiation whose key writes as a string keys the map, and one whose key does not fails the whole map at serialization with `key must be a string` — so string keys hold for every instantiation that serializes at all. All three surfaces say exactly that and nothing more: `{"type": "object", "additionalProperties": V}`, `z.record(z.string(), V)`, and `Partial<Record<string, V>>` — the trio a `String` key earns. The value side stays described, being no parameter's business, so `HashMap<KeyType, ValueType>` on `Holder<KeyType, ValueType>` is `Partial<Record<string, ValueType>>`.

The key position is the one place TypeScript stops rendering a parameter as itself — see [Type Parameters](#type-parameters) for what it does everywhere else — and the declaration is what forces it. `Record<K, T>` is declared `K extends keyof any`, so a declaration handing it a parameter it binds without bounding does not type-check at all, and the consumer pasting the emitted `.ts` gets the error before writing a value. Bounding the parameter would move the failure rather than fix it: the bound propagates to every name written over the item, including the erased fillings a binding annotated for the opaque value supplies. What the member gives up is naming the parameter in its key; what it gains is the guarantee serde already makes about every filling that reaches the wire.

### Pointers and Borrowed Values

`Box<T>`, `Rc<T>`, `Arc<T>` and `Cow<'_, T>` describe as `T`. serde writes each of them as the value it holds, with nothing of its own around it, so a field written under one is the field its inner type is — on every surface, and wherever the wrapper was written: `Box<Option<T>>` is the `Option<T>` field, optional; `Vec<Rc<T>>` is the `Vec<T>` field; `Box<str>`, `Rc<str>` and `Cow<'_, str>` are `String` fields, `str` being what a `String` writes.

This is what lets a type hold itself: `Option<Box<Self>>` is the self-reference, deferred in the generated schema the way any self-reference is. An `Rc` or `Arc` field needs serde's `rc` feature, which is where serde's impls for those two live.

`RefCell<T>`, `Cell<T>`, `Mutex<T>` and `RwLock<T>` describe as `T` on every surface the same way: serde writes each as the value it guards, returning a serialization error rather than panicking when the guard cannot be taken (an already-mutably-borrowed `RefCell`, a poisoned `Mutex`/`RwLock`) — the same fallible path a schema does not describe for any other type. What sets them apart from `Box`/`Rc`/`Arc`/`Cow` is `Deref`: none of the four implements it, so `#[model_schema_prop(pattern = ..., minLength = ..., maxLength = ..., minimum = ..., maximum = ...)]` cannot reach through one to the value it guards, and is refused at expansion, naming the wrapper, wherever it is combined with one.

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

An enum whose variants are all unit variants and which names no `tag` and no `untagged` generates a TypeScript string union and `z.enum()` in Zod, matching the bare variant name Serde writes for it. Either attribute changes what Serde writes, and the surfaces follow it rather than the string union. Naming a `tag` moves the variant name under the tag key, into an object, so the same all-unit variants are described as the tagged objects below. Naming `untagged` drops the name altogether -- Serde writes every unit variant as a bare `null` -- and the declaration is refused per variant, the way any unit variant in an untagged enum is refused under "Untagged Enums" below.

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

A variant carrying nothing is the tag alone, `{"type":"Bare"}`, and an enum whose variants are *all* unit variants is written that way throughout -- one object per variant, each holding only the tag. That is the shape of a service's error code, and it is described as the tagged object union it is written as, never as the bare string union the same variants would publish with no `tag` named.

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
- **Zod** cannot name a choice as an operand, so it multiplies instead. An intersection recognizes exactly the keys its operands name; `z.discriminatedUnion` propagates its members' keys and is an operand like any other, while a plain `z.union` names none -- each of its branches would be asked to validate the whole payload alone and reject the keys the container and the sibling branches carry, which is every payload the object writes. So a flattened untagged enum, and a flattened source reached through an `Option`, are written as a union *of* intersections: one branch per combination, with the object's own keys bound once to `{Type}$OwnSchema` and read by every branch. A union nested inside a union contributes its leaves, the nesting writing no key of its own. With one combination there is nothing to choose between, so the object's own keys stay where they were.
- The same multiplication runs where an enum's own struct variant flattens such a source, on every tagging. A variant's object is a fragment of a larger literal with no name to bind beside it, so the combinations are written where the object stands: one `z.strictObject({ ... }).and(...)` per key set, joined in a `z.union`. The variant still contributes exactly one member to the enum's union -- that member is a union itself. Externally tagged, it sits under the variant-name key; adjacently tagged, under the content key, leaving the enclosing `z.discriminatedUnion` intact because the tag stays in the member's own object; internally tagged, it *is* the member, and the enclosing union drops to a plain `z.union` for the reason any flattened member forces it to; untagged, it is the member as well. Two separately written `z.strictObject`s over the same keys validate identically, so writing the object once per combination costs generated text and nothing else. TypeScript needs no equivalent -- it distributes an intersection over a union on its own -- and JSON Schema already multiplies these branches at run time.
- That multiplication needs the union's members, which reach the merge through the registry -- so **an untagged enum must be declared above the object that flattens it**. Declared below, it has not expanded yet and the merge names it as one operand, the spelling that rejects every payload the object writes. Nothing at the merge tells such a source apart from a plain struct declared below, which is a spelling that works, so this is a declaration-order requirement rather than a diagnostic. It sits beside the reason the operands are deferred at all: a base is a `const` of its own, and nothing orders one type's schema against another's.
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

A member of an untagged variant carries `#[model_schema_prop(...)]` exactly as the same field written in a tagged variant does: the constraint reaches the Zod schema, the JSON Schema and the Rust side alike -- the same per-member validator and the same [`validate()` accessor](#the-validate-method) -- and every guard the attribute earns is reported at the member. One thing is the untagged member's alone: it is also hung with the `deserialize_with` hook, because an untagged read picks the variant by which one accepts the payload, so the constraint decides what the value *is* and not merely whether it is admissible. Everywhere else the read admits the value and `validate()` answers for the bound; see [where a constraint is checked](#where-a-constraint-is-checked).

What differs is the position, and it costs the read its diagnosis. Serde tries an untagged enum's variants in declaration order, and a member whose bound fails takes its variant out of the candidate set rather than ending the read -- which is exactly what the same declaration means under `anyOf` and under `z.union`. So a value the bound rejects lands on the next variant that accepts it, and when none does, serde's derived `Deserialize` has already discarded each candidate's own error and answers with one sentence of its own:

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum SoleConstrained {
    Slug {
        #[model_schema_prop(minLength = 2)]
        slug: String,
    },
}

let refused = serde_json::from_str::<SoleConstrained>(r#"{"slug":"A"}"#).unwrap_err();
assert_eq!(refused.to_string(), "data did not match any variant of untagged enum SoleConstrained");
```

The tagged twin of that declaration reads back `'slug': too short: minimum length is 2, got 1`, because its tag names the variant before its members are read. To learn which bound refused an untagged value, ask a surface that answers per member rather than per variant: validate the payload against the generated Zod schema or JSON Schema for a diagnosable verdict, or -- once the value is in hand -- call the enum's `validate()`, or the schema module's `validate_{variant}_{member}_value` directly.

A member holding a map or a tuple describes as the struct field written from the same type does, on every surface. A map is dispatched on the classification its key earns -- enumerated properties for a key whose members the expansion knows, `additionalProperties` for an open one -- and a key no surface can write is refused at the member, at whatever depth the map sits at. A tuple is the fixed-arity array Serde writes, `prefixItems` and the arity bounds included. A member the parser could not classify at all is the one shape that stays permissive: it carries no type name to narrow with, so it admits any value, exactly as the same field does.

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

### Type Parameters

A struct, a tuple struct, an enum, an alias and a branded newtype can each name their own type parameters, and every surface reads such a name under one rule.

**TypeScript binds the parameter for real.** It is a type surface, and the declaration it emits carries the parameter list: `export type Wrapper<T> = { id: T }` is a generic type, and a use site fills `T` in. The one position where it does not is a map's key, which states `string` on all three surfaces — see [Collections and Maps](#collections-and-maps).

**JSON Schema writes the document of whatever fills the parameter.** JSON Schema has no type parameters at all, so a document exists only at one filling: the type `default_types` declares where the document stands on its own, and the reference site's arguments where a field embeds it. The shape around the parameter -- an array, a map's keys, a tuple's arity -- is described either way.

**Zod publishes a factory rather than a schema -- and a default beside it.** A Zod schema is a runtime value and TypeScript generics do not exist at runtime, so there is no one value a generic type could publish: the caller has to say what fills each parameter before anything can validate. A generic type therefore exports `X$SchemaFactory`, a function taking one required schema argument per parameter, and a field written with a parameter composes the argument bound for it. It also exports `X$SchemaDefault`: the factory called at the type's own declared `default_types`, memoized like any other call, so a consumer who wants the ordinary filling never constructs the argument list by hand.

**With the `jsonschema` feature on, every type parameter needs a declared default type.** JSON Schema has no type parameters, so its document has to be built from one concrete filling, and `default_types` is where that filling is named — see [Declaring the default type](#declaring-the-default-type) below.

```rust
#[model_schema(default_types(IdType = String))]
pub struct Wrapper<IdType> {
    pub children: Vec<IdType>,
    pub id: IdType,
    pub name: String,
}
```

```typescript
export type Wrapper<IdType> = {
  children: Array<IdType>;
  id: IdType;
  name: string;
};

const buildWrapper$Schema = <IdType extends ZodType>(
  idType: IdType,
) =>
  z.strictObject({
  children: z.array(idType),
  id: idType,
  name: z.string(),
});

type Wrapper$SchemaOf<IdType extends ZodType> = ReturnType<
  typeof buildWrapper$Schema<IdType>
>;

const Wrapper$SchemaFactoryCache = new WeakMap<ZodType, Wrapper$SchemaOf<ZodType>>();

export function Wrapper$SchemaFactory<IdType extends ZodType>(
  idType: IdType,
): Wrapper$SchemaOf<IdType>;
export function Wrapper$SchemaFactory(
  idType: ZodType,
): Wrapper$SchemaOf<ZodType> {
  const hit = Wrapper$SchemaFactoryCache.get(idType);
  if (hit) return hit;

  const schema = buildWrapper$Schema(idType);
  Wrapper$SchemaFactoryCache.set(idType, schema);
  return schema;
}

export const Wrapper$SchemaDefault: ZodType<Wrapper<string>> = Wrapper$SchemaFactory(z.string());
```

Every parameter is a real TypeScript type parameter, never a bare `ZodType` annotation: `ZodType` defaults its own parameters, so an argument annotated with it would infer every field it validates as `unknown` and the caller would learn nothing from the schema handed back. Arguments are required -- a default would let a call site say nothing about a filling and still be handed a schema, which is exactly the silent mis-validation the factory exists to prevent.

Each factory memoizes on the *identity* of the arguments it was handed, one cache level per parameter. Two calls with the same argument objects return the identical schema, and no two argument lists collide -- a change in the first argument and a change in the last each key a different level:

```typescript
const wireDocument = EcmDocument$SchemaFactory(z.string(), z.number());
const storedDocument = EcmDocument$SchemaFactory(objectIdSchema, z.date());

EcmDocument$SchemaFactory(z.string(), z.number()) === wireDocument;  // false -- fresh arguments, new key
```

That is why `$SchemaDefault` is written as a call through the factory rather than composed inline: a declared default that itself names another generic item -- `default_types(IdType = DocumentId<String>)` -- reads back `DocumentId`'s own `$SchemaDefault` rather than reconstructing `DocumentId$SchemaFactory(z.string())` from scratch. The two calls would key different cache entries even though they mean the same filling, so reading the sibling's binding back is what keeps `EcmDocument$SchemaDefault` sharing the one `DocumentId` schema everything else that asks for the ordinary filling shares too.

That reference is deferred, exactly like a flattened base's is: `DocumentId$SchemaDefault` is a module-scope `const`, a generated module concatenates one type's output after another in whatever order the consuming project's entity list produces, and one macro invocation sees one type -- so nothing here can know whether `DocumentId`'s `const` is written above `EcmDocument`'s in the emitted module or below it.

```typescript
export const EcmDocument$SchemaDefault: ZodType<EcmDocument<DocumentId<string>, number>> =
  EcmDocument$SchemaFactory(z.lazy(() => DocumentId$SchemaDefault), z.number());
```

The same deferral covers an argument that does not fold -- a declared default naming a sibling at arguments other than that sibling's own default still calls its factory, behind the identical `z.lazy(() => …)` -- which is what keeps a cycle between two declared defaults from throwing at import: neither side can have registered the other's arguments yet when it is expanded, so neither folds, and both read the other's factory lazily rather than at the top of a `const` initializer.

A generic type that also flattens keeps the deferred read of its base, so declaration order stays irrelevant: `.and(z.lazy(() => Envelope$Schema))` composes inside the factory unchanged.

#### A generic type may reach itself

A generic type that holds itself -- directly, or through a second type that holds it back -- is described on every surface. JSON Schema hoists it into `$defs` once and points a `$ref` back at that one definition, which stands because every reference around the cycle carries the same filling. TypeScript writes the name inside itself, arguments and all. Zod calls the factory again with the argument the outer call was handed, and the memo cache is what makes that terminate: the schema is in the cache before the recursive call is made, so it comes back rather than being rebuilt.

```rust
#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node<IdType> {
    pub children: Vec<Self>,
    pub id: IdType,
}
```

```typescript
export type Node<IdType> = {
  children: Array<Node<IdType>>;
  id: IdType;
};

const buildNode$Schema = <IdType extends ZodType>(
  idType: IdType,
) =>
  z.strictObject({
  get children() { return z.array(Node$SchemaFactory(idType)); },
  id: idType,
});
```

The member is written as a getter so that the call is made after the factory has reached its cache rather than while its object is still being built. Where a cycle spans two types, the reference the getter is written on is the one pointing *forward* -- at a type declared below -- because that is the reference no cycle can be built without: if every reference in a cycle named something already declared, declaration positions would have to decrease all the way round. Deferring those leaves nothing that can cycle, and every reference pointing back at a type already declared is written as it stands.

#### Declaring the default type

JSON Schema has no type parameters. A generic type's document has to be built from one concrete filling, and nothing in the declaration says which — so `default_types` says it, one `Parameter = Type` pair per type parameter:

```rust
#[model_schema(default_types(IdType = String, DateType = f64))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EcmDocument<IdType, DateType> {
    pub document_id: DocumentId<IdType>,
    pub created_at: DateType,
}
```

The pairs may be written in any order, and the argument sits beside every other item-level argument -- `name`, `pattern`, `minLength`, `maxLength`, `no_display` -- changing none of them. A lifetime and a const parameter name no type, so neither takes an entry.

A lifetime is dropped from every emitted surface, neither TypeScript nor JSON Schema having one and a borrowed value writing exactly what its owned form writes. A const renders in one position only -- an array length, which [describes as an unbounded array](#collections-and-maps) -- so a const handed to a written type as an *argument* is refused, spanned on the argument: an argument list is read as a list of types, and the const standing in one would have the JSON document call into a module named after it and the TypeScript declaration write a name it binds nothing for. A const no written type carries is untouched.

The declared filling is what `json_schema()` writes the document at. Beside it, a generic type's schema module publishes `json_schema_with`, which takes one document per parameter, positionally, in the order the item declares them:

```rust
let wire = EcmDocument::<String, f64>::json_schema();
assert_eq!(wire["properties"]["createdAt"], serde_json::json!({ "type": "number" }));

let stored = ecm_document_schema::Schema::json_schema_with(&[
    serde_json::json!({ "type": "object", "properties": { "$oid": { "type": "string" } } }),
    serde_json::json!({ "type": "string", "format": "date-time" }),
]);
```

A field naming a generic type reaches that same entry point with the arguments written at the field, so what it embeds is the document its own filling describes -- a `StoredFolder` holding `EcmDocument<ObjectId, DateTime<Utc>>` embeds the `$oid` object and a date-time string, where a `WireFolder` holding `EcmDocument<String, f64>` embeds a string and a number. A parameter forwarded into such a reference carries whatever filled the item forwarding it.

Zod reads the same declaration for `EcmDocument$SchemaDefault` -- see [Type Parameters](#type-parameters) above -- so the JSON document a standalone type describes and the Zod schema its ordinary filling validates against are built from the one statement of what that filling is, never two.

The declaration is read in both directions, and each refusal points at what earned it:

| Written | Verdict |
|---------|---------|
| An entry naming something the item does not declare | Refused in **every** feature configuration, spanned on the entry. A misspelled parameter fills nothing, and the parameter it was meant for would be left with no default at all. |
| A type parameter with no entry | Refused only where `jsonschema` is on, spanned on the parameter. Nothing else reads the default, so the same item compiles untouched without that feature. |
| `default_types` on an item declaring no type parameter | Refused, there being nothing for a filling to fill. |
| An entry filled at a type the macro cannot describe -- `i128`, `u128`, `f16`, `f128` | Refused where `zod` or `jsonschema` is on, spanned on the filling -- those are the two surfaces that read a declared filling. A name the type dispatch has no arm for is read as another `#[model_schema]` item, which is right for a sibling declared below and gibberish for a primitive: the JSON emission would name an `i128_schema` module nothing publishes, and the zod `$SchemaDefault` an `i128$Schema` binding no generated module exports. Only the reserved primitive names the dispatch answers for nowhere are refused, so a forward-referenced sibling keeps compiling -- and so does a filling at `char`, which describes as the one-character string serde writes for it. |

There is deliberately no fallback. Guessing a filling produces a document that silently rejects valid payloads, which is the failure the declaration exists to prevent.

#### The module preamble

Each factory declares its own store, so a generated module is the types in it and nothing else -- there is no preamble to emit ahead of them, and nothing shared between one type and the next.

A `WeakMap` fixes both of its own parameters at construction, so its value type cannot depend on its key type; a store declared beside the factory is therefore keyed and valued at the widened `ZodType` spelling. What recovers the precise types is an **overload**: the signature a caller sees names real type parameters, and the implementation beneath it takes the widened ones the store is written at. The read is then already the implementation's return type, so there is nothing to cast.

```typescript
const Wrapper$SchemaFactoryCache = new WeakMap<ZodType, Wrapper$SchemaOf<ZodType>>();

export function Wrapper$SchemaFactory<IdType extends ZodType>(
  idType: IdType,
): Wrapper$SchemaOf<IdType>;
export function Wrapper$SchemaFactory(
  idType: ZodType,
): Wrapper$SchemaOf<ZodType> { ... }
```

A type declaring N parameters writes N nested levels, each keyed by one argument, and the levels below the first are built where they are first needed:

```typescript
const Quintet$SchemaFactoryCache = new WeakMap<ZodType, WeakMap<ZodType, WeakMap<ZodType, WeakMap<ZodType, WeakMap<ZodType, Quintet$SchemaOf<ZodType, ZodType, ZodType, ZodType, ZodType>>>>>>();
```

Nothing in the emitted output is `as`, `any`, or `unknown`, and nothing a caller passes is written to. The store holds the arguments weakly, so a schema nobody else holds is collected rather than pinned for the life of the module.

#### An alias is a factory too

Every generic item publishes a factory, an alias among them, so a parameter inside one is the argument that factory binds for it:

```rust
#[model_schema(default_types(T = String))]
pub type Wrapper<T> = Vec<T>;
```

```typescript
export type WrapperType<T> = Array<T>;

const buildWrapperType$Schema = <T extends ZodType>(
  t: T,
) =>
  z.array(t);
```

Only the item's *own* parameters are read this way. A name the expansion cannot resolve because the type lives elsewhere keeps its `Name$Schema` reference, since that type publishes the binding.

One consequence is worth stating outright: a parameter is a value only where a factory binds one for it, and a JSON document is written at one filling rather than for the type as declared — so no `model_schema_prop` bound may be spelled against a type parameter, and a *field* typed with one is refused for the same reason, at every depth the parameter is reached through. A branded newtype's own `pattern`/`minLength`/`maxLength` are the one exception: applied directly to a bare-parameter inner, they read the parameter's *declared default* rather than the parameter itself — see [Branded Newtype Validation Constraints](#branded-newtype-validation-constraints).

```rust
#[model_schema(default_types(IdType = String))]
pub struct Constrained<IdType> {
    #[model_schema_prop(minLength = 3)] // refused, and so is it on `Option<IdType>` or `Vec<IdType>`
    pub id: IdType,
}
```

The value's type is whatever the instantiation supplies, so nothing here holds it to anything: Zod hands the check to a schema it has no bound on, the JSON schema describes the value as the permissive empty one, the generated validator emits no check for it, and serde reads the payload back untouched. Constrain the argument instead — declare the type the instantiation supplies as a branded newtype carrying the bound — or drop the key. The JSDoc says nothing about a bound the refusal turns away either, the sentence being written only where something holds the value to it.

### Branded Newtypes

`#[serde(transparent)]` tuple structs with a single public field generate branded TypeScript types. The newtype is invisible in JSON serialization but carries a distinct type identity in TypeScript, preventing accidental mixing of different ID types.

```rust
use tixschema::model_schema;
use serde::{Deserialize, Serialize};

// Generic branded newtype (good for parameterized IDs)
#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UserId<IdType>(pub IdType);

// Non-generic branded newtype
#[model_schema()]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CorrelationId(pub String);
```

Generated TypeScript (with `zod` feature):

```typescript
export type UserId<IdType> = IdType & $brand<"UserId">;
const buildUserId$Schema = <IdType extends ZodType>(
  idType: IdType,
) =>
  idType.meta({
  description: "UserId",
}).brand<"UserId">();

type UserId$SchemaOf<IdType extends ZodType> = ReturnType<
  typeof buildUserId$Schema<IdType>
>;

const UserId$SchemaFactoryCache = new WeakMap<ZodType, UserId$SchemaOf<ZodType>>();

export function UserId$SchemaFactory<IdType extends ZodType>(
  idType: IdType,
): UserId$SchemaOf<IdType>;
export function UserId$SchemaFactory(
  idType: ZodType,
): UserId$SchemaOf<ZodType> {
  const hit = UserId$SchemaFactoryCache.get(idType);
  if (hit) return hit;

  const schema = buildUserId$Schema(idType);
  UserId$SchemaFactoryCache.set(idType, schema);
  return schema;
}

const UserId$RawSchemaDefault = UserId$SchemaFactory(z.string());

export const UserId$SchemaDefault: typeof UserId$RawSchemaDefault = UserId$RawSchemaDefault;

export type CorrelationId = string & $brand<"CorrelationId">;
const CorrelationId$RawSchema = z.string().brand<"CorrelationId">().meta({
  description: "CorrelationId",
});

export const CorrelationId$Schema: typeof CorrelationId$RawSchema = CorrelationId$RawSchema;
```

Generated TypeScript (without `zod` feature):

```typescript
declare const __brand_UserId: unique symbol;
export type UserId<IdType> = IdType & { readonly [__brand_UserId]: true };

declare const __brand_CorrelationId: unique symbol;
export type CorrelationId = string & { readonly [__brand_CorrelationId]: true };
```

Notes:

- The brand is published under the Rust ident, spelled exactly as written, unless `#[model_schema(name = "...")]` names another -- see [Published Names](#published-names-name). The name reaches the surface twice: as the exported type and as the brand tag the values carry.
- Generic parameter names (e.g., `IdType`) are preserved exactly in the TypeScript type.
- A generic brand publishes a factory, as every other generic item does, so the brand and its inner's shape land on the argument the caller supplied rather than on a value pinned at expansion. A parameter reaches it as the factory's own argument, under the rule in [Type Parameters](#type-parameters); a brand written over that parameter still describes what its inner writes, so `TagList<T>(pub Vec<T>)` is an array on every surface — `Array<T>`, `z.array(t)`, `{"type": "array", "items": {}}` — and not the bare parameter.
- The description is written before the brand in a factory and after it in a `const`. Inside a factory the receiver is the parameter the caller filled, and Zod's `.meta()` returns `this` — which TypeScript resolves back to that bare parameter, dropping the marker `.brand<"Name">()` had just added. Both orders build the same schema.
- Serde transparent serialization works normally -- the wrapper is invisible in JSON.
- An item whose published expression *is* another item's binding -- an alias of a brand, a one-slot tuple struct over one, and the `$SchemaDefault` of either where it declares a parameter -- is annotated `typeof {Name}$RawSchema` rather than `ZodType<{Name}>`, so the brand's narrowing survives the republish; `.brand()` narrows at the value position, which a restated `ZodType<{Name}>` discards. An item that builds an expression of its own keeps `ZodType<{Name}>`.
- The slot takes no `#[model_schema_prop(...)]`. A brand publishes its inner's own schema with a `.brand()` written onto it, so no key written on the slot -- `pattern`, `as`, `preprocess`, `literal`, `ts_optional`, `nullable`, any of them -- reaches any surface, and one written there is refused with a message naming the spelling that does work. The three checks a brand carries are written on the type itself: `#[model_schema(pattern = "...", minLength = N, maxLength = N)]`, under [Branded Newtype Validation Constraints](#branded-newtype-validation-constraints). This is about the *slot* of a single-field tuple struct; a `#[serde(transparent)]` struct with a **named** field is no brand, and its field attributes are read as any other named field's are.
- Use branded newtypes for opaque IDs and phantom types to prevent passing the wrong ID type across domain boundaries.

#### A Named Inner Must Carry `#[model_schema()]`

**Error:** `cannot find module or crate <type>_schema in this scope` (`E0433`), reported at the inner type.

A brand carries its inner's wire form and adds a name to it, so it describes as that inner describes — by reference, through the schema module the inner's own `#[model_schema()]` publishes. A named inner declared without the attribute publishes no such module, and the reference names one that was never emitted. This is the same requirement a field naming that type lives under, reported the same way and at the same place:

```rust
// Wrong: `Foreign` carries no `#[model_schema()]`, so no `foreign_schema` module exists
#[derive(Serialize, Deserialize)]
pub struct Foreign(pub String);

#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct HoldsForeign {
    pub id: Foreign, // E0433: cannot find module or crate `foreign_schema` in this scope
}

#[model_schema(no_display)]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct Wrapped(pub Foreign); // E0433: the same module, named in brand position
```

The remedy is either of two:

- **Annotate the inner.** Put `#[model_schema()]` on `Foreign`, and both spellings resolve — the brand describing as whatever `Foreign` describes as.
- **Brand the wire form instead.** Where the inner is a foreign scalar this crate cannot annotate -- `uuid::Uuid`, `rust_decimal::Decimal` -- brand the type it is written as: `pub struct Wrapped(pub String)`, with `pattern` on the brand where the format is worth pinning.

A type this crate describes must be one it can read. The alternative -- letting a name it cannot resolve fall back to `{"type": "string"}` -- describes a struct as a string whenever the inner turns out to be one, which is the failure the brand's schema path exists to rule out.

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

You can add `pattern`, `minLength`, and `maxLength` constraints directly on the `#[model_schema()]` attribute for branded newtypes. Constraints are enforced in three places: the generated Zod schema, serde deserialization, and a `validate()` method on the type. A brand is the one Rust position where the check still runs on the read, and it has to: a message holding a branded field publishes no `validate()` that reaches into it, so the read is the only thing there is.

**The inner type has to be one whose schema is a string** — `String`, `PathBuf`, `ObjectId`, a chrono date/time type, or a named type whose own schema is one of those. A numeric, boolean, container (`Vec`, array, `HashMap`, tuple), or opaque inner is rejected at expansion time, because the three constraints are string checks and each surface would read them differently: Zod's `.min`/`.max` become bounds on the value itself, JSON Schema ignores `minLength`/`maxLength`/`pattern` outside `"type": "string"`, and `validate()` measures the inner's `Display` rendering.

**A named inner is judged by what the named type publishes**, not by the fact that it is a name. The brand appends its checks to that type's own schema binding — `Inner$Schema.min(3, ...)` — so a name whose schema is an object, a union, a `z.enum`, a number, an array or `z.unknown()` is rejected exactly as the same shape spelled directly is, and the refusal names both the brand and the inner. A brand over `serde_json::Value` is opaque, and so is a brand over that brand.

That answer comes from the type's own expansion, so it is only available once that expansion has run: a brand written **above** the type it names, or over a type this crate never expands at all, is admitted with the emission it has always had. Declaration order is not a diagnostic — moving a declaration must not turn a compiling program into a rejected one — so keep a constrained brand below the type it constrains if you want the check.

An opaque inner is `serde_json::Value`, which both validating surfaces read as the opaque value (see [Type Parameters](#type-parameters)) and which the checks are refused against outright — there is nothing there for them to attach to.

**One of the brand's own type parameters is different: it reads its declared default instead of being refused.** A bare parameter is not itself a type the checks could measure, but `#[model_schema]` already requires (for `jsonschema`) or defaults (elsewhere) a concrete filling for every parameter — see [Declaring the default type](#declaring-the-default-type) — and that filling is exactly as concrete as a named type's own schema. So the guard asks the same question of the declared default it asks of a named inner: a string-shaped default carries the checks, a non-string one is refused in its place. The checks land once, on `$SchemaDefault`'s argument for that parameter; the factory itself stays unconstrained, so a caller filling the parameter with something else — an `ObjectId` schema, say — is never held to bounds meant for the default:

```rust
// Accepted: `IdType`'s declared default is `String`, so the checks land there.
#[model_schema(minLength = 24, maxLength = 24, pattern = "^[a-f0-9]{24}$", default_types(IdType = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DocumentId<IdType>(pub IdType);

// Rejected: `IdType`'s declared default is `u32`, which cannot carry them.
#[model_schema(minLength = 3, default_types(IdType = u32))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GenericCount<IdType>(pub IdType);
```

```typescript
const buildDocumentId$Schema = <IdType extends ZodType>(idType: IdType) =>
  idType.brand<"DocumentId">();

// The factory itself carries no check — a non-default filling is not held to it.
export const DocumentId$SchemaFactory = <IdType extends ZodType>(idType: IdType) => { /* ... */ };

// $SchemaDefault composes the factory with the constrained default argument.
export const DocumentId$SchemaDefault = DocumentId$SchemaFactory(
  z.string().min(24, { error: (issue) => `too short: minimum length is 24, got ${String(issue.input).length}` }).max(24, { error: (issue) => `too long: maximum length is 24, got ${String(issue.input).length}` }).check(z.regex(/^[a-f0-9]{24}$/, { error: "does not match pattern '^[a-f0-9]{24}$'" })),
);
```

`validate()` itself is emitted only at that same declared default — `impl DocumentId<String> { pub fn validate(&self) -> Result<(), Vec<String>> { … } }`, never a blanket `impl<IdType> DocumentId<IdType>` — because Rust inherent impls do not specialize: a blanket one would make a downstream `impl DocumentId<ObjectId> { pub fn validate(&self) -> … }` a duplicate-definition error. Pinning `validate()` to the default leaves that door open for an author who wants their own validation on another instantiation; the schema delegates (`ts_definition()`, `zod_schema()`, `json_schema()`, …) are unaffected and stay on the type's own generic `impl<IdType> DocumentId<IdType>`, since they do not depend on the constraints.

An entry `default_types` leaves out falls back to `String`, the same fallback a parameter with no declared filling gets everywhere else the declaration is read — so a constrained bare-parameter brand with no `default_types` at all still compiles. Only the `jsonschema` feature actually requires a declared entry for every parameter, through the unrelated requirement in the table above.

**A name written over one of those parameters is rejected too**, whether or not the named type has expanded yet. `Tagged<T>` is not a type the declaration fixed — the instantiation fixes it — so the checks land on whatever the caller supplies: Zod appends them to a schema the call site decides the shape of, the one JSON document written for every instantiation still holds the `{}` a parameter describes as, and `validate()` measures the inner's `Display` rendering, which rejects `TaggedSlug(Tagged(7))` for having one decimal digit rather than for its value. This is the one place the registry's silence is not read as consent, and it is what keeps declaration order out of the diagnostic: the same two declarations written in the other order are rejected through the registry, so admitting this order would decide one program two ways.

```rust
// Rejected: the checks would measure whatever the instantiation supplies for `T`.
#[model_schema(minLength = 3, default_types(T = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaggedSlug<T>(pub Tagged<T>);

#[model_schema(default_types(T = String))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Tagged<T>(pub T);
```

Brand a string-typed inner instead. The bound belongs on a declaration that writes a string — a `String` brand of its own, which the parameterised type can then be instantiated with (`Tagged<BoundedSlug>`), so the check sits where every surface can read it.

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
const SlugId$RawSchema = z.string().min(3, { error: (issue) => `too short: minimum length is 3, got ${String(issue.input).length}` }).max(50, { error: (issue) => `too long: maximum length is 50, got ${String(issue.input).length}` }).check(z.regex(/^[a-z0-9_]+$/, { error: "does not match pattern '^[a-z0-9_]+$'" })).brand<"SlugId">().meta({
  description: "SlugId",
});

export const SlugId$Schema: typeof SlugId$RawSchema = SlugId$RawSchema;
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
    Err(errors) => println!("{:?}", errors), // ["too short: minimum length is 3, got 2"]
}
```

A brand's report names no field where a struct field's is named and quoted (`'username': ...`): a newtype has one value and no field name to quote, so whatever holds it supplies the name.

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

An inner type the macro cannot resolve because it lives elsewhere — another brand, a user type — is
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
/// - `DocumentId<ObjectId>` for `MongoDB` layer
///
/// ```rust example
/// DocumentId("64de3d95ff45b119e5b53a7e".to_string())
/// ```
#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DocumentId<IdType>(pub IdType);
```

Generated Zod:

```typescript
const buildDocumentId$Schema = <IdType extends ZodType>(
  idType: IdType,
) =>
  idType.meta({
  description: "Generic document identifier.\n- `DocumentId<String>` for API/HTTP layer\n- `DocumentId<ObjectId>` for `MongoDB` layer",
  example: "64de3d95ff45b119e5b53a7e",
}).brand<"DocumentId">();

type DocumentId$SchemaOf<IdType extends ZodType> = ReturnType<
  typeof buildDocumentId$Schema<IdType>
>;

const DocumentId$SchemaFactoryCache = new WeakMap<ZodType, DocumentId$SchemaOf<ZodType>>();

export function DocumentId$SchemaFactory<IdType extends ZodType>(
  idType: IdType,
): DocumentId$SchemaOf<IdType>;
export function DocumentId$SchemaFactory(
  idType: ZodType,
): DocumentId$SchemaOf<ZodType> {
  const hit = DocumentId$SchemaFactoryCache.get(idType);
  if (hit) return hit;

  const schema = buildDocumentId$Schema(idType);
  DocumentId$SchemaFactoryCache.set(idType, schema);
  return schema;
}

const DocumentId$RawSchemaDefault = DocumentId$SchemaFactory(z.string());

export const DocumentId$SchemaDefault: typeof DocumentId$RawSchemaDefault = DocumentId$RawSchemaDefault;
```

### Services

`#[service_schema()]` declares a service as a trait: one trait, one context, and one message in and one message out per operation. Failing to implement an operation is a compile error, which is the whole reason the construct is a trait rather than a table of handlers.

**Services need the `serde` feature.** A service's messages derive `Serialize` and `Deserialize`, a dispatcher reads payloads through `serde_json`, and the camelCase and kebab-case keys it puts on the wire come from serde attributes the macro writes. A build without the feature is refused at the declaration rather than left to publish TypeScript that names Rust fields the wire never carries:

```text
service_schema: a service needs tixschema's `serde` feature, and this build does not have it
       without it the TypeScript a service publishes names Rust fields rather than the camelCase
       and kebab-case keys its own dispatcher writes, so the two halves disagree about the wire
       add `features = ["serde"]` to the tixschema dependency in Cargo.toml
```

**A service that publishes TypeScript needs the `zod` feature too.** A message validates when it is
constructed, in both directions: the generated client parses what it is about to send before it
reaches a transport, and the generated dispatcher parses what arrived before it enters an
implementation, so an implementation may assume its message is valid. On the TypeScript side both
checks are the same parse, against the `<Message>$Schema` const `#[model_schema()]` publishes — and
only a build with the `zod` feature publishes one.

So a build with `typescript` on and `zod` off publishes the service's **types** and neither seam
artifact: no `<Service>Schema::ts_client()` and no `<Service>Schema::ts_service()`. The types are
published as they always are, because they describe what the Rust dispatcher and the Rust client put
on the wire, and that half validates in either build. The two that are withheld are withheld rather
than emitted without their check: a client that forwards whatever it is handed and a dispatcher that
narrows an unread payload with `as` both compile and both read exactly like the checked ones, while
the Rust half of the same service goes on validating — two halves of one service disagreeing about
what they accept, with nothing to say so.

A bundle that names either one in such a build is refused where it names it, which is the one place
the choice of features can still be acted on:

```text
error[E0599]: no associated function or constant named `ts_client` found for struct `UsageServiceSchema` in the current scope
   |
23 | #[service_schema()]
   | ------------------- associated function or constant `ts_client` not found for this struct
...
34 |     let _ = UsageServiceSchema::ts_client();
   |                                 ^^^^^^^^^ associated function or constant not found in `UsageServiceSchema`
```

**A service is declared at module scope, never inside a function body.** The module the macro
generates opens with `use super::*;`, which is how the client and the per-operation validators
reach the trait and the message types declared beside it. A module written inside a function body
has the enclosing *module* as its parent rather than the function, so `super` from there reaches
past every name that function declared. `#[model_schema()]` carries the same requirement, for the
same reason.

The macro cannot refuse the placement: an attribute macro is handed the annotated item's own tokens
and nothing about the scope it was written in, and a trait written inside a function is the same
tokens as one written beside it. A function-scoped declaration earns the compiler's own resolution
errors instead — one for the trait, and one for each type an operation named:

```text
error[E0405]: cannot find trait `UsageService` in module `super`
error[E0425]: cannot find type `BalanceRequest` in this scope
error[E0425]: cannot find type `BalanceResponse` in this scope
error[E0425]: cannot find type `BalanceError` in this scope
```

One consequence is worth knowing before it costs anyone an afternoon: rustdoc wraps a doctest with
no explicit `fn main` in one, so a doctest that declares a service writes `fn main() {}` of its own.

#### The Shape of a Service

A service names a **context** type parameter, and every operation takes it as its first argument after `&self`. An operation receives one message and answers `Result<Success, Error>`.

```rust
use serde::{Deserialize, Serialize};
use tixschema::{model_schema, service_schema};

#[model_schema()]
#[derive(Deserialize, Serialize)]
pub struct AvailableBalanceRequest {
    pub organization_id: String,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
pub struct AvailableBalanceResponse {
    pub available_credits: u32,
    pub is_post_paid: bool,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum BalanceError {
    DbError,
    InsufficientBalance { shortfall: u32 },
}

#[service_schema()]
pub trait UsageService<Ctx> {
    /// No reply, so no return type and no error arm.
    #[service_schema_op(one_way)]
    async fn apply_bundle(&self, ctx: &Ctx, req: AvailableBalanceRequest);

    /// Carried on the wire as `usage-generation-request` rather than `can-generate`.
    #[service_schema_op(message = "usage-generation-request")]
    async fn can_generate(
        &self,
        ctx: &Ctx,
        req: AvailableBalanceRequest,
    ) -> Result<AvailableBalanceResponse, BalanceError>;

    /// Several arguments after the context: the macro declares the message.
    async fn expire_credit(
        &self,
        ctx: &Ctx,
        organization_id: String,
        credit_id: String,
    ) -> Result<AvailableBalanceResponse, BalanceError>;

    /// One argument after the context: that argument already is the message.
    async fn get_available_balance(
        &self,
        ctx: &Ctx,
        req: AvailableBalanceRequest,
    ) -> Result<AvailableBalanceResponse, BalanceError>;

    /// None at all, so the macro declares an empty message.
    async fn sweep(&self, ctx: &Ctx) -> Result<AvailableBalanceResponse, BalanceError>;
}
```

The context is whatever an implementation needs and no caller may see -- a logger, a correlation id. The implementation chooses the type, and it reaches no message, no schema and no wire:

```rust
pub struct UsageContext {
    pub logger_name: String,
}

pub struct UsageBackEnd;

impl UsageService<UsageContext> for UsageBackEnd {
    async fn apply_bundle(&self, ctx: &UsageContext, req: AvailableBalanceRequest) {
        // ...
    }

    // ... and the other four, or this does not compile.
}
```

The trait is emitted as declared, except that `async fn` is desugared to `-> impl Future<Output = ...> + Send`, the desugaring the compiler's own `async_fn_in_trait` warning recommends. A trait with `async fn` is not dyn compatible, so the generated dispatcher is generic over the implementing type rather than taking `&dyn UsageService`.

#### Service Messages

Every operation receives exactly one message. How many arguments it takes after the context decides only who declares that message:

| arguments after the context | the incoming message is | who declares it |
| --- | --- | --- |
| exactly one | that argument, as-is | you, with `#[model_schema()]`, like any other type |
| more than one | a message declared from the argument list | the macro, named `<Operation>Request` |
| none | an empty message | the macro, named `<Operation>Request`, with no fields |

**One argument is the normal case and the one to reach for.** The argument *is* the message: nothing is synthesized, nothing is renamed, and the type is one you wrote and can reuse, version and reference from elsewhere.

**The multi-argument form costs something, and the cost is on the wire.** Each argument's name becomes a field name on the declared message, so renaming a parameter -- an invisible refactor in Rust -- moves a key on the wire, and no compiler will flag it. Reach for it where the input is one identifier and a named type would be ceremony; reach for a declared type everywhere else. Every generated message repeats this in its own rustdoc and in the JSDoc it publishes.

`expire_credit` takes two arguments, so `ExpireCreditRequest` is declared from them, in declaration order and with camelCase keys:

```typescript
export type ExpireCreditRequest = {
  organizationId: string;
  creditId: string;
};
```

```typescript
const ExpireCreditRequest$RawSchema = z.strictObject({
  organizationId: z.string(),
  creditId: z.string(),
});

export const ExpireCreditRequest$Schema: ZodType<ExpireCreditRequest> = ExpireCreditRequest$RawSchema;
```

`sweep` takes none, so its message is empty rather than absent. An operation that later needs a field gains one here, instead of changing from carrying no payload to carrying one and breaking every caller:

```typescript
export type SweepRequest = Record<string, never>;

const SweepRequest$RawSchema = z.strictObject({
});

export const SweepRequest$Schema: ZodType<SweepRequest> = SweepRequest$RawSchema;
```

`get_available_balance` takes one argument, so nothing is declared for it at all -- `AvailableBalanceRequest` is the message, exactly as you wrote it.

#### Operation Names

An operation's name is written once, in Rust, and spelled per language by convention. Nothing is repeated and no attribute is involved:

| where | spelling | derived as |
| --- | --- | --- |
| Rust | `get_available_balance` | the trait method as declared |
| TypeScript | `getAvailableBalance` | camelCase, what a TypeScript caller expects to type |
| the wire | `get-available-balance` | kebab-case of the method name |

`#[service_schema_op(message = "...")]` moves the **wire name only**. Rust still calls the operation by its method name and TypeScript by the camelCased one:

```rust
#[service_schema_op(message = "usage-generation-request")]
async fn can_generate(
    &self,
    ctx: &Ctx,
    req: AvailableBalanceRequest,
) -> Result<AvailableBalanceResponse, BalanceError>;
```

It exists for one reason: services already ship wire names nobody would derive from the method. An operation a caller knows as `canGenerate` may dispatch today as `usage-generation-request`, the two sharing no substring, so without the override no such service could adopt this construct without changing bytes. A greenfield operation writes no attribute at all. The override replaces the derived name rather than adding a second one -- after it, nothing answers to `can-generate`.

#### Service Errors

An operation's `Result` names **one** type in its error position. That type is normally an enum, which is a union, so "an operation's errors" is one `#[model_schema()]` type with several variants rather than several types.

What is per-operation is *which* type is named, and two operations on one service may name entirely unrelated ones. A service that genuinely has one error surface pays nothing for this -- it writes the same type in every signature. What it buys is that a read operation is never handed a write-conflict variant to match on, and a caller's `switch` over an operation's errors is exhaustive over exactly what that operation can return.

A variant may carry data of its own, which the tag sits beside:

```typescript
export type BalanceError = {
  errorCode: "db-error";
} | {
  errorCode: "insufficient-balance";
  shortfall: number;
};
```

#### One-Way Operations

An operation marked `#[service_schema_op(one_way)]` expects no reply, and therefore declares no return type:

```rust
#[service_schema_op(one_way)]
async fn apply_bundle(&self, ctx: &Ctx, req: AvailableBalanceRequest);
```

The flag and the return type must agree, and the macro checks both directions, so a forgotten `Result` cannot become a silent fire-and-forget. A missing return type with no flag is refused naming both choices:

```text
service_schema: operation `apply_bundle` has no return type
       add `#[service_schema_op(one_way)]` if it expects no reply,
       or give it a `Result<Success, Error>` return
```

And the flag with a return type is refused too:

```text
service_schema: operation `apply_bundle` is marked `one_way` but returns a value
       a one-way operation produces no reply
```

There is no error arm on a one-way operation, because there is no reply to carry one. An operation that needs to report failure is a request-and-reply operation declared wrong. In TypeScript a one-way method answers `Promise<void>`; in Rust it answers nothing beyond the send.

The one failure it can still report is its own. A message that fails its own validation never reaches the transport, and the fault naming the key has nowhere to go but out of band: in Rust it is `Err(ServiceFault)`, the failure arm the method already has, and in TypeScript `Promise<void>` has no failure arm at all, so it is **thrown**. What is thrown is published rather than left to be discovered from the body -- `<Service>Refusal`, an `Error` carrying the fault -- and the method's own `JSDoc` names it:

```typescript
export type UsageServiceRefusal = Error & { fault: UsageServiceFault };
```

Silently dropping a refused message was the alternative and is worse: a fault is a defect meant to page a human, and a dropped one pages nobody.

#### Service Validation

A message is validated when it is constructed, in both directions, by the validator `#[model_schema()]` already generates for it -- `validate()` on the Rust side, the Zod schema on the TypeScript side. Nothing new is written for a service.

Inbound, the dispatcher deserializes the payload into the operation's message and validates it *before* the operation body is entered. **A service implementation may therefore assume its incoming message is valid, because an invalid one never reaches it.**

Outbound, the generated client validates the message it is about to send, so a malformed request fails at the caller with the field named rather than as a remote error a round trip later -- and the transport is never touched:

```typescript
    async expireCredit(req) {
      const validated = ExpireCreditRequest$Schema.safeParse(req);
      if (!validated.success) {
        return {
          ok: false,
          error: {
            isServiceFault: true,
            fault: usageServiceOutboundFault("expire-credit", validated.error.issues),
          },
        };
      }
      return transport.request<UsageServiceExpireCreditResult>("expire-credit", validated.data);
    },
```

A one-way method runs the same check and has nowhere to put the result of it, so it throws the refusal the section above names -- still before the transport is reached:

```typescript
    async applyBundle(req) {
      const validated = AvailableBalanceRequest$Schema.safeParse(req);
      if (!validated.success) {
        throw usageServiceRefused(usageServiceOutboundFault("apply-bundle", validated.error.issues));
      }
      await transport.notify("apply-bundle", validated.data);
    },
```

#### Service Faults

A payload that will not deserialize, a message that fails validation, an operation name nothing recognises, a handler that panicked and a call the transport could not carry are none of the things an operation declared it could fail at. Folding them into a service's domain errors would put transport concerns in every signature, so they come back as a **fault**: a defect rather than a condition, meant to be logged at error level and to page a human.

A result therefore keeps two arms, with the fault riding inside the failure arm behind a literal a caller can narrow on:

```typescript
export type UsageServiceGetAvailableBalanceResult =
  | { ok: true; value: AvailableBalanceResponse }
  | { ok: false; error: BalanceError | { isServiceFault: true; fault: UsageServiceFault } };
```

A caller that only handles the declared errors still compiles; one that wants to distinguish narrows on `isServiceFault` and then on whatever the error type discriminates on, with no `default` arm to write:

```typescript
if (result.ok) {
  render(result.value.availableCredits);
} else if ("isServiceFault" in result.error) {
  reportUnexpected(result.error.fault);
} else {
  switch (result.error.errorCode) {
    case "db-error":
      return retryLater();
    case "insufficient-balance":
      return showUpgradePrompt();
  }
}
```

**No service implementation can answer with a fault.** In Rust an operation's signature admits only its own error type, so there is nowhere to put one. In TypeScript the interface an implementation satisfies is typed against an *outcome*, which has no fault member in it:

```typescript
export type UsageServiceGetAvailableBalanceOutcome =
  | { ok: true; value: AvailableBalanceResponse }
  | { ok: false; error: BalanceError };
```

**And the fault type itself refuses to be written.** An outcome with no fault member stops a fault being *returned*; it does not stop one being *built*, and a plain structural object type is writable by anyone who can name it. So the fault's fields publish under a name of their own and the name a caller reads is those fields under a brand keyed on a symbol the bundle declares and exports nowhere:

```typescript
export type UsageServiceFaultFields = {
  detail: string;
  field: string | undefined;
  kind: UsageServiceFaultKind;
  operation: string;
};

declare const usageServiceFaultSeal: unique symbol;

export type UsageServiceFault = UsageServiceFaultFields & {
  readonly [usageServiceFaultSeal]: true;
};
```

The fields still come from the Rust declaration, so nothing about the wire moved and nothing a caller reads changed -- narrow on `isServiceFault`, then read `kind`, `detail`, `field` and `operation`, or switch over `kind` exhaustively. The brand is a type and never a value: `declare const` emits nothing, and a fault carries no extra key.

What stops compiling is writing one. An object literal under a `UsageServiceFault` annotation cannot carry the branded property, and neither can a structurally-equal value assigned into a `UsageServiceFault` position -- a module outside the bundle cannot name the symbol, and a module inside has no value to write. What it does **not** stop is a deliberate type assertion: `built as UsageServiceFault` compiles, as does anything laundered through `any` or `unknown`, and TypeScript has no construct that refuses those. The generated constructors mint through exactly that assertion, from the fields type, which is the one direction an assertion is unambiguously sound in. So the seal makes fabricating a fault a deliberate, greppable act rather than something an annotated literal does silently -- it is not the `E0451` Rust gives, because TypeScript has none to give.

Exactly two generated places build one: the dispatcher, when an incoming message cannot be turned into a valid request or names an operation nothing recognises, and the client, when the message *it* is about to send fails its own validation or the transport reports that the call never landed. In Rust a client call answers `Result<Success, CallError<Error>>`, `CallError` being `Operation(E)` for the error the operation declared and `Fault(ServiceFault)` for a defect that reached the caller.

Which kind a refused payload is reported under is `serde_json`'s own classification of the refusal, not the shape of the sentence it wrote. `undeserializable-payload` means the bytes are not a document at all -- not JSON, or a document that ends early -- which is a sender whose serialization is broken, and such a refusal names no field because nothing was read far enough for a key to be what went wrong. `failed-validation` means the bytes read as a document and did not match the message: a field carrying the wrong type of value, a key that is missing, a value a bound refuses. That is a value someone supplied. The byte offset serde appends is dropped, it being a position inside an encoding the caller never saw.

The TypeScript dispatcher raises only the second of the two, and that is the same line rather than a different one: it is handed a payload somebody already read out of the bytes, so by the time it runs the bytes *were* a document and the only thing left to be wrong is what the document said. Bytes that are no document at all fail before it, in whatever turns them into the value it takes. So one defect reaches a call site under one kind whichever language served it.

The field is named wherever the refusal named one: a validator's report names it in single quotes, and serde names it in backticks for a missing or unknown key. A bound declared on a field's *type* rather than on the field -- a constrained brand -- is checked as the value is read, and the brand's own report names nothing, the brand being the value rather than a member of anything; the field holding it writes its own wire key into the refusal, so `'organizationId': too short: minimum length is 3, got 2` is what a caller reads, one quoted run holding the whole name. A type mismatch is the one refusal left carrying no field: serde says what it expected and not where, and inventing a name for it would send a caller to the wrong key.

#### What a Service Generates

On the Rust side, beside the trait, in a module named for the service (`usage_service_schema`):

- `ServiceFault` and `ServiceFaultKind` -- readable by anyone, constructible by nothing outside the module. `ServiceFault` is the alias; the struct is declared as `<Service>FaultFields`, which is the name its *fields* publish under in TypeScript, `<Service>Fault` there being the sealed type written over them.
- `CallError<E>` -- `Operation(E)` or `Fault(ServiceFault)`.
- `<Operation>Message` and `validated_<operation>` -- the name each operation's message is republished under, and the validator that message runs. Which of the two answers a check gives depends on the message's concrete type, so both halves of a service ask this one function rather than answering for themselves, from wherever they were expanded.
- `Answered<T, E>` -- the envelope a request-and-reply operation answers in, written by a dispatcher and read back by a client.
- `ExpireCreditRequest` and `SweepRequest` -- the messages declared for the operations that named none, each an ordinary `#[model_schema()]` type.

Beside the trait, two `macro_rules!` per transport the service asked for -- see [Transports](#transports) -- and nothing else transport-shaped. A service that asks for no transport is emitted neither a dispatcher nor a client.

#### Transports

`#[service_schema(transports = ["amqp_rpc"])]` says which transports the service wants. Each one contributes two `#[macro_export]` macros, `{service_snake_case}_{transport}_dispatcher` and `{service_snake_case}_{transport}_client`, whose bodies hold that transport's two halves as *tokens*: nothing inside either is compiled where the service is declared.

Two macros rather than one, because the two halves of a service usually live in different crates -- a crate that calls the service can see the contract but has no business seeing the server's backend, and a server crate has no use for a client. Each is invoked and placed by the half that wants it, and neither drags in the other.

```rust,ignore
// In the crate that declares the service: nothing below is built here.
#[service_schema(transports = ["amqp_rpc"])]
pub trait UsageService<Ctx> { /* ... */ }
```

Each invocation goes in a module of its own **file**, and those `mod` declarations go above the crate's `use` items. A `macro_rules!` body is linted under the levels of the crate that *invokes* it, and this is the placement that measures clean: an inline `mod amqp_transport { ... }` earns `clippy::inline_modules`, a `mod` written below a `use` earns `clippy::arbitrary_source_item_ordering`, and reaching the author's own types through `use declaring_crate::*;` earns `clippy::wildcard_imports`.

```text
// In the crate that serves it -- src/lib.rs
mod amqp_transport;

// src/amqp_transport.rs
declaring_crate::usage_service_amqp_rpc_dispatcher!();
```

```text
// In a crate that calls it -- src/lib.rs: no backend, no bus library.
mod amqp_client;

// src/amqp_client.rs
use declaring_crate::{AvailableBalanceRequest, AvailableBalanceResponse};

declaring_crate::usage_service_amqp_rpc_client!();
```

Each macro takes no arguments and emits bare items rather than a module of its own, so the caller names the module and two transports in one crate cannot collide. The dispatcher emits `IncomingMessage` -- built with `IncomingMessage::new(operation, payload)` by whichever crate owns the bus and read through `operation()` and `payload()`, its two fields being private -- `Reply`, the handle a transport implements to answer one message, with `send` and `fault`, and `dispatch(svc, ctx, message, reply)`, generic over the implementing type. Beside them go the panic guard an arm calls its implementation behind and the reader that classifies a refused payload, both emitted only where the service declares an operation for them to serve. The client emits:

- `Transport` -- the seam a client is bound to, with `notify` and `request`. Both answer a `Result`, whose failure arm carries in words what stopped a call from travelling; the client turns it into a fault of kind `transport-failure`.
- `UsageServiceClient` -- one method per operation, over any `Transport`. Each takes that operation's arguments and no context: a context is what an implementation needs, and a caller has nothing to hand one to. Every method answers a `Result`, and every method's generated documentation says under an `# Errors` heading what its failure arm can hold.
- The mirror that reads a fault back off the wire and the reader that turns one envelope into three outcomes, emitted only where the service declares an operation that answers. A service declaring only one-way operations is emitted neither, there being no reply for either to read.

Paths inside a `macro_rules!` body resolve where the macro is *invoked*, so the two kinds are spelled apart. Everything tixschema generated is reached through `$crate::`, which resolves in the crate that declared the service; every runtime crate is reached through a leading `::` and resolves in the invoking crate, **which is therefore the one that names `serde`, `serde_json` and `tracing` in its own manifest**. That is the point of the shape: a crate that only declares services names serde alone. A crate that places only a client names `serde` and `serde_json` and not `tracing`, nothing in a client catching a panic; the `use` in the client module above is what resolves the message types the author declared, which the expansion spells exactly as they were written.

A crate that declares a service *and* places one of its halves invokes that macro by name rather than by path -- `usage_service_amqp_rpc_dispatcher!()` -- after the declaration and inside it or a module below it, `#[macro_use]` carrying it out of a submodule. Rust refuses an absolute path to a `macro_export` macro that a macro produced, from within the crate that produced it -- `crate::usage_service_amqp_rpc_dispatcher!()` and `use crate::usage_service_amqp_rpc_dispatcher;` both earn a deny-by-default future-incompatibility:

```text
error: macro-expanded `macro_export` macros from the current crate cannot be referred to by absolute paths
   = note: `#[deny(macro_expanded_macro_exports_accessed_by_absolute_paths)]` (part of `#[deny(future_incompatible)]`) on by default
```

Another crate reaches either macro by path as above.

`#[macro_export]` places each macro at the declaring crate's root whatever module it was written in, so each name has to be unique across that crate, and `$crate` reads from that same root. A service declared at the crate root needs nothing further; one declared in a submodule re-exports at the crate root its generated module, its trait and the messages the macro declared, which is all either half reaches by name -- every message a dispatcher deserializes is reached as `$crate::{service}_schema::<Operation>Message`.

Two of those three the declaring crate now checks for itself. A service that asked for at least one transport resolves its generated module and its trait against the crate root at the declaration, so leaving either unreachable fails **that** crate's own `cargo build` rather than every crate that goes on to invoke a macro. The caret sits on the trait, with the attribute macro named as the origin:

```text
error[E0433]: cannot find `usage_service_schema` in `crate`
  --> src/services/usage.rs:15:11
   |
14 | #[service_schema(transports = ["amqp_rpc"])]
   | -------------------------------------------- in this attribute macro expansion
15 | pub trait UsageService<Ctx> {
   |           ^^^^^^^^^^^^ unresolved import

error[E0405]: cannot find trait `UsageService` in the crate root
```

Each is earned on its own, so re-exporting one leaves the other standing rather than repeating both. Write them as **`pub use`**, not `use`: a private import at the root satisfies an invocation inside the declaring crate -- which is what this crate's own test harnesses rely on -- while a crate invoking the macro from outside earns ``error[E0603]: module `usage_service_schema` is private``. A service that asked for no transport publishes no macro, is checked against no root, and compiles below the crate root with nothing re-exported.

On the TypeScript side, three artifacts, reached through a unit struct named `<Service>Schema`:

```rust
pub fn get_entities() -> (String, Vec<String>) {
    (
        "service entities".to_string(),
        vec![
            UsageServiceSchema::ts_definition(),
            UsageServiceSchema::ts_client(),
            UsageServiceSchema::ts_service(),
        ],
    )
}
```

`ts_definition()` answers with every message the macro declared for the service, the fault type and the kind it reports, and one result type per operation that answers -- **so a generated message needs no registration line of its own.** The author never wrote those types and has no reason to know their names; a forgotten line would leave a Rust-only message and a client unable to call the operation.

`ts_client()` answers with the transport seam, the client type and the factory that binds one:

```typescript
export type UsageServiceClient = {
  /**
   * Sends `apply-bundle` on `UsageService`, which expects no reply.
   *
   * @throws {UsageServiceRefusal} when the message fails its own schema. The operation
   * answers `Promise<void>`, so there is no failure arm to put the fault in; the transport
   * is still never reached.
   */
  applyBundle(req: AvailableBalanceRequest): Promise<void>;
  /** Calls `usage-generation-request` on `UsageService` and waits for the answer. */
  canGenerate(req: AvailableBalanceRequest): Promise<UsageServiceCanGenerateResult>;
  /** Calls `expire-credit` on `UsageService` and waits for the answer. */
  expireCredit(req: ExpireCreditRequest): Promise<UsageServiceExpireCreditResult>;
  /** Calls `get-available-balance` on `UsageService` and waits for the answer. */
  getAvailableBalance(req: AvailableBalanceRequest): Promise<UsageServiceGetAvailableBalanceResult>;
  /** Calls `sweep` on `UsageService` and waits for the answer. */
  sweep(req: SweepRequest): Promise<UsageServiceSweepResult>;
};
```

`ts_service()` answers with the interface an implementation satisfies in full, the outcome types it answers with, and the dispatcher factory:

```typescript
export interface UsageServiceImpl<Ctx> {
  /** Handles `apply-bundle` on `UsageService`, which expects no reply. */
  applyBundle(ctx: Ctx, req: AvailableBalanceRequest): Promise<void>;
  /** Handles `usage-generation-request` on `UsageService` and answers it. */
  canGenerate(ctx: Ctx, req: AvailableBalanceRequest): Promise<UsageServiceCanGenerateOutcome>;
  /** Handles `expire-credit` on `UsageService` and answers it. */
  expireCredit(ctx: Ctx, req: ExpireCreditRequest): Promise<UsageServiceExpireCreditOutcome>;
  /** Handles `get-available-balance` on `UsageService` and answers it. */
  getAvailableBalance(ctx: Ctx, req: AvailableBalanceRequest): Promise<UsageServiceGetAvailableBalanceOutcome>;
  /** Handles `sweep` on `UsageService` and answers it. */
  sweep(ctx: Ctx, req: SweepRequest): Promise<UsageServiceSweepOutcome>;
}
```

```typescript
export function createUsageServiceDispatcher<Ctx>(
  impl: UsageServiceImpl<Ctx>,
): (ctx: Ctx, operation: string, payload: unknown) => Promise<unknown> {
```

Every member is required, so an implementation missing one is refused where it reaches `createUsageServiceDispatcher`. Every emitted name carries the service -- `UsageServiceFault`, `UsageServiceGetAvailableBalanceResult`, `UsageServiceClient`, and the fault's own brand symbol `usageServiceFaultSeal` -- because TypeScript has no per-service scope and a bundle is one flat file. Rust needs no such prefix, the generated module being the scope TypeScript lacks.

#### Transports

tixschema generates no transport. What it emits is the seam either side of one:

- **The operation name arrives from the transport**, beside the payload and never inside it, so no message type has to reserve a key for routing. `dispatch` is handed the name and matches on it; a gRPC method name or an HTTP path serves the same role on those transports.
- **The encoding lives behind `Reply`.** `Reply::send` is handed a value rather than bytes, because a transport merges its own fields -- a correlation id, an error flag -- into the object before serializing it, and neither is reachable behind an encoded buffer.
- **Queues, retries, timeouts, acknowledgement, connection handling and reconnection belong to the caller.** `dispatch` returns nothing, so the adapter that called it still holds the delivery when the arm finishes, and acknowledges there -- for every message, including a one-way one. That placement is what lets the handle carry replying and nothing else: a path that never replies names nothing about replying.
- **A call that never landed has a place to be reported.** `Transport::notify` and `Transport::request` both answer a `Result`, and `Err` carries whatever the transport wants to say about a message that did not go out or a reply that is not coming. Without it a transport that hit its own deadline could only panic or hang, and a caller would be left holding a call that never completes and no fault to report. Whether there is a deadline, and how long it is, stays the transport's decision -- the arm is where the answer is reported, not where it is decided.

A request-and-reply arm answers exactly once, through `send` or `fault`, whichever way it goes. A one-way arm that reached its implementation calls neither; one whose payload was refused before it got there answers with a fault, that being the only thing it can say about a message it never ran.

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

Generated Zod for `username` -- each check carrying the sentence the Rust validator reports for the same bound, so the two languages answer a bad value with the same words:

```text
z.string()
  .min(3, { error: (issue) => `too short: minimum length is 3, got ${String(issue.input).length}` })
  .max(30, { error: (issue) => `too long: maximum length is 30, got ${String(issue.input).length}` })
  .check(z.regex(/^[a-z0-9_]+$/, { error: "does not match pattern '^[a-z0-9_]+$'" }))
```

Generated JSON Schema for `username`: `{ "type": "string", "minLength": 3, "maxLength": 30, "pattern": "^[a-z0-9_]+$" }`

The TypeScript type is unchanged -- still just `string`.

A value breaking more than one of a field's bounds is reported once per bound on both sides, in the order the bounds were declared: `validate()` collects every violation rather than stopping at the first, and every check on the Zod schema runs. So `"A!"` in the field above reads `'username': too short: minimum length is 3, got 2; 'username': does not match pattern '^[a-z0-9_]+$'` from a Rust service and from a TypeScript one alike.

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

#### A `pattern` has to turn some value away

A pattern every string satisfies is refused too, for a different reason: it is a constraint that constrains nothing. `""`, `^`, `$`, `|`, `a*` and `^a*` all match at some position of every string, so the generated validator would reject no value and the Zod and JSON schemas would publish a check every payload passes -- an attribute that reads as a guarantee and is not one. The derive fails at expansion, naming the field or the brand.

The verdict is read off the parsed pattern rather than off its text, so the same shape written any way gets the same answer -- and the near misses keep theirs:

| Pattern | Verdict |
|---|---|
| `""`, `^`, `$`, `\|`, `()`, `(^)` | Refused -- matches at a position every string has |
| `a*`, `a?`, `a\|`, `^a*`, `a*$` | Refused -- the repeated part may run zero times, or the alternative may be skipped |
| `^$` | Accepted -- both ends at one position, which only the empty string has |
| `^a*$` | Accepted -- both ends pinned around a run of `a` |
| `\b`, `\B` | Constraining -- the empty string holds no word boundary -- and refused by the next rule |
| `\b\w+`, `\Ba` | Accepted -- a boundary with something beside it |
| `a+`, `^[a-z]+$` | Accepted -- a character has to be there |

#### A `pattern` cannot be one assertion and nothing else

`\b` and `\B` are refused for a third reason, and it is not about what they accept. The generated Rust validator builds the pattern with `regex::Regex::new`, and `clippy::trivial_regex` -- which a consumer denying `clippy::nursery` gets -- calls a lone assertion "unlikely to be useful as it is" and reports it against the `#[model_schema]` attribute, where the author has no edit to make. For the shapes that lint does name a replacement for -- `^abc`, `abc$`, `^abc$`, `^$`, `abc` -- this crate emits the `str` call instead of a regex, so they compile clean; for a lone boundary it names none, so there is nothing to put in the regex's place.

Write the boundary beside what has to sit next to it: `\b\w+` rather than `\b`, `\B\w` rather than `\B`. That keeps the regex, keeps the boundary check, and is no longer trivial to the lint. Only the lone assertion is refused -- `\ba`, `^\bfoo` and every other pattern with a boundary somewhere in it are untouched.

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

When any field carries a constraint, the macro generates a `validate(&self) -> Result<(), Vec<String>>` method. It is what checks a field's constraints -- on an instance built in Rust and on one just read off the wire alike, the read itself having admitted the value.

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
        // "'username': too short: minimum length is 3, got 2"
        // "'age': too large: maximum is 120, got 150"
    }
}
```

The macro also generates into the type's schema module:
- `validate_{field}_value(&FieldType) -> Result<(), Vec<String>>` -- pure static validator per field, answering every bound the value broke in the order they were declared
- `deserialize_{field}(D) -> Result<FieldType, E>` -- serde hook that calls the static validator

A constrained field of an enum variant is named for its variant too -- `validate_{variant}_{field}_value` and `deserialize_{variant}_{field}`, with `{variant}` in `snake_case`. One schema module holds every variant's helpers, and a field name is unique only within the variant that declares it, so two variants naming one field carry their own constraints.

An enum whose members carry constraints publishes the same `validate(&self) -> Result<(), Vec<String>>`, under every tagging serde offers (externally, internally and adjacently tagged, and `#[serde(untagged)]`). A value holds one variant at a time, so the check runs that variant's members and no other's, collecting every violation in the words a struct's field is answered in:

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Action {
    Delete {
        #[model_schema_prop(minLength = 5)]
        note: String,
    },
    Upload {
        #[model_schema_prop(minLength = 3)]
        note: String,
    },
}

let deleting = Action::Delete { note: "abc".to_string() };
assert_eq!(
    deleting.validate().unwrap_err(),
    vec!["'note': too short: minimum length is 5, got 3".to_string()],
);
// The same value read as an `Upload` is held to that variant's own minimum, and passes.
assert!(Action::Upload { note: "abc".to_string() }.validate().is_ok());
```

Parity with structs runs both ways: an enum no member of which carries a constraint publishes no `validate()`, exactly as a constraint-free struct publishes none.

#### Constraints under `Option`, wrappers, and sequences

A constraint describes the value the field puts on the wire, wherever it was written. `validate()` therefore reaches through everything the parser reads through -- an `Option`, a transparent wrapper (`Box`, `Rc`, `Arc`, `Cow`), and every sequence level (`Vec`, `VecDeque`, `HashSet`, `BTreeSet`, `BinaryHeap`, arrays and slices) -- and checks the innermost value, which is the same place the Zod, TypeScript and JSON Schema surfaces put it.

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

### Where a constraint is checked

By `validate()`, and not as the payload is read.

A constraint describes the value, not the shape. A payload carrying a value it rejects is still structurally the message it claims to be -- every key present, every value of its field's declared type -- so the read admits it and `validate()` is what refuses it, naming the field. Checking it on the read makes the two indistinguishable to whoever receives the failure, and "I could not parse this at all" and "I parsed it and the value broke a rule" send a caller looking in different places.

A field still generates both helpers into its schema module: `validate_{field}_value()`, which `validate()` calls, and `deserialize_{field}`, a serde hook an author may hang on a field of their own accord. Two positions carry a check on the read without being asked to, and both have to:

- **A member of an `#[serde(untagged)]` enum**, where whether the member is admissible is what chooses which variant the payload is. The check is part of reading the value rather than part of judging it -- exactly as it is under `anyOf` and `z.union` on the two schema surfaces the same type publishes -- and `validate()` cannot stand in for it, since by the time it runs the variant has already been chosen. A wrapped member's hook deserializes the member's own declared type and then runs the same walk `validate()` runs over it; the two differ in that `validate()` answers with every violation in the instance while a `Deserializer` answers with one line, so the read stops at the first value that broke a bound and hands serde that value's violations joined by `; `.
- **A constrained brand**, where the same reasoning applies wherever the brand is used as an untagged member. The hook is on the brand's type rather than on the field, so it is one hook covering every position the brand appears in.

A field whose key may be left out keeps that reading. Where a member is hung with the hook -- an untagged variant's, which is the one position that is -- an `Option` written outermost (under any number of transparent wrappers) is given `#[serde(default)]` alongside it, since a `deserialize_with` otherwise turns a missing key into an error; a field that writes its own `default` keeps the one it wrote. Off the hook there is nothing to put back and no `default` is written, so a required key that goes missing is still the error it always was.

#### A bound the field's own type declares

A field carrying no `model_schema_prop` of its own may still hold a type that declares one -- a constrained brand, or a nested `#[model_schema()]` type. The enclosing `validate()` runs whatever validator that type published and writes its report under the field that held it:

```rust
#[model_schema(minLength = 3)]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct Slug(pub String);

#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct Enrolment {
    pub slug: Slug,
}
```

```rust
Enrolment { slug: Slug("a".to_string()) }.validate();
// Err(["'slug': too short: minimum length is 3, got 1"])
```

The brand's own report says `too short: ...` and names nothing, the brand being the value rather than a field of anything -- so the field name is the message's to supply, written first and in single quotes, which is where a reader of these reports already looks for one: `'slug': too short: ...`. A nested type's report already names its own member, and the field name is written *into* that name rather than in front of it: `'holds.name': too short: ...`. One quoted run carries the whole path, so a reader taking the first one gets the member that was actually wrong rather than the hop above it -- and it is the string the TypeScript schema published from the same declaration reports for the same payload.

A `#[serde(flatten)]` hop writes no key, so it contributes no segment: a bound under a flattened `claims` inside `account` reads `'account.jti': too short: ...`, which is what the wire carries and what TypeScript names.

This reaches through the same wrappers a constraint is reached through -- `Option`, sequences, `Box`/`Cow`/`Rc`/`Arc`, fixed arrays -- and applies to a struct's field, to a named member of an enum variant under any tagging, and to the lone slot of an `#[serde(untagged)]` newtype variant. That last one contributes no segment either, for the same reason a flattened hop contributes none: what an untagged newtype member writes on the wire *is* the inner value, so a violation beneath it is already one of the enclosing object's own keys. So a union of newtype members publishes a `validate()` that dispatches to whichever variant the value holds, and a field holding such a union reports the path the payload spells -- `'account.jti': too short: ...` whether the account is a union or a plain struct.

It reaches nothing through a map or a tuple. It also reaches nothing through a *tagged* variant's positional slot or a tuple struct's slots, which have no name to report under and no arm named for them; those are the same limits a constraint's own walk has.

The brand's read-time hook stands unchanged beside this, so a brand's bound is now enforced in both places. They answer different questions: a payload arriving over the wire is refused on the read, and a message built in Rust -- where no read ever ran, which is every outbound message a generated client sends -- is refused by `validate()`. A nested type's bound has no hook at all, so its enclosing `validate()` is the only thing that enforces it.

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

`literal` also accepts a boolean or a numeric value, provided the field's own Rust type can carry it: `literal = true`/`literal = false` on a `bool` field, `literal = 214` on any numeric field. Every surface renders the literal in the field's own kind, so the kind written must match the field's declared type — `literal = true` on a `String` field, or `literal = "x"` on a `bool` field, is a compile error naming both sides.

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminUser {
    pub id: String,
    #[model_schema_prop(literal = true)]
    pub is_system_admin: bool,
    pub email: String,
}
```

Generated:

```typescript
export type AdminUser = {
  id: string;
  isSystemAdmin: true;
  email: string;
};

export const AdminUser$Schema = z.strictObject({
  id: z.string(),
  isSystemAdmin: z.literal(true),
  email: z.string(),
});
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

The bare `ts_optional` flag asks for the optional key (`field?: T` rather than `field: T | undefined`) on the author's word. It is the only thing that writes that spelling.

**Two positions have no key for it to make optional, and writing it on either is a compile error, in every build.** A positional slot writes no key at all, so the flag names a spelling the tuple line has no room for. A member a serde attribute takes off the wire in *both* directions (`skip`, or `skip_serializing` and `skip_deserializing` together) is described on no surface, so there is no line for the flag to write its key on. An attribute that drops the key one way only (`skip_serializing_if`, `skip_serializing`) leaves the member standing and the flag decides its spelling as usual.

With the `serde` feature on, an `Option<T>` field must carry a key-dropping attribute anyway: its declared shape is `T | undefined`, so its key has to be dropped for a `None`, not written as `null` the way serde writes it absent an attribute saying otherwise — the guard refuses the declaration and names the attribute to add (or, if the wire really does carry `null` here, points at [`nullable`](#nullable-object-keys-nullable) instead). The example below is a build with the feature **off**, where no attribute is read and no such guard runs:

```rust
#[model_schema()]
pub struct Profile {
    pub name: String,
    #[model_schema_prop(ts_optional)]
    pub nickname: Option<String>,
    pub nick_handle: Option<String>,
}
```

Generated TypeScript:

```typescript
export type Profile = {
  name: string;
  nickname?: string;
  nick_handle: string | undefined;
};
```

`nick_handle` is the same field without the flag, so the flag is the whole of the difference between those two lines.

This is a TypeScript-only knob -- the Zod schema and JSON Schema are unchanged (the field is already optional in both, flagged or not). The flag is only valid on `Option<T>` fields; applying it to a non-`Option` field is a compile error. It composes with `as = Type`, which names the type the field already renders.

### Nullable Object Keys (`nullable`)

The bare `nullable` flag is for the `Option<T>` field whose empty state genuinely *is* `null` on the wire — the key is always written and `null` is its value, not the absence of one. Where the default coerces an incoming `null` away to `undefined` and leaves the key optional, `nullable` renders the field as `T | null` with the key **required** on all three surfaces:

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub name: String,
    #[model_schema_prop(nullable)]
    pub template_id: Option<String>,
}
```

Generated TypeScript:

```typescript
export type DocumentMetadata = {
  name: string;
  templateId: string | null;
};
```

Generated Zod: `templateId: z.union([z.string(), z.null()])` -- no `.prefault`, no coercing arm. Parsing `{ name: "invoice.pdf", templateId: null }` succeeds; parsing `{ name: "invoice.pdf" }` fails, because the flag declares the key required.

Generated JSON Schema: `"templateId": { "anyOf": [{ "type": "string" }, { "type": "null" }] }`, with `templateId` in `required`.

`nullable` is only valid on `Option<T>` fields; applying it to a non-`Option` field is a compile error. It is refused together with `ts_optional` -- the two disagree about the key (`nullable` keeps it, `ts_optional` drops it), and writing both would spell `field?: T | null`, a third state neither flag models. It composes with `preprocess`: the preprocess wrap goes around the whole nullable union, coercing the raw value before it is checked against `T | null`.

With the `serde` feature on, `nullable` is also refused together with any key-dropping serde attribute -- `skip_serializing_if`, `skip_serializing`, or `skip` -- because the flag declares the key always written and one of those would drop it instead, leaving serde write a payload the generated schema does not admit. The guard names the field and the flag. Deserialization needs no such guard: a `nullable` field reads both `{"templateId":null}` and `{"templateId":"invoice.pdf"}` the same way it always did, `{}` included, since nothing about `nullable` changes what serde accepts on the way in.

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
- A generic struct or enum has its example built at one instantiation, with every type parameter filled at `String`. A lifetime elides there and needs no filling, but a **const parameter takes none** -- `String` names a type, a const is a value, and no value is the one every example would be written at. So a struct or an enum that declares a const parameter and writes an example is refused, naming the const; drop the example or the const parameter. Nothing is owed where no example is written, on a type alias, or with the `zod` feature off, `zod` being the only surface that reads one.
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
  id: z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) }),
  title: z.string(),
  author_id: z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) }),
  tags: z.array(z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) })),
  metadata: z.record(z.string(), z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) })),
  parent_id: z.union([z.null().transform(() => undefined), z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) }), z.undefined()]).prefault(undefined),
  related_docs: z.record(z.string(), z.array(z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) }))),
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
  updated_at: z.union([z.null().transform(() => undefined), z.coerce.date(), z.undefined()]).prefault(undefined),
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
- `#[serde(skip_serializing_if = "...")]` -- the key is left out of the payload when the predicate fires: `roles: z.array(z.string()).optional(),` in Zod and no `required` entry in the JSON Schema. On a field that is not an `Option` the TypeScript member takes the optional key too, `roles?: Array<string>;`, there being no second spelling for it; an `Option` field keeps `T | undefined` unless [`ts_optional`](#ts_optional) asks otherwise
- `#[serde(skip)]` -- the key is written into no payload and read out of none, so no surface describes the member at all: no TypeScript member, no Zod key, and neither a `properties` nor a `required` entry. On a tuple-struct or tuple-variant slot it takes the slot out of the described tuple, which shortens the arity -- and a variant declaring one slot becomes a unit variant, which is what serde writes for it
- `#[serde(skip_serializing)]` -- the write half of `skip`: the key is left out of every payload while a supplied one is still read, and every surface answers as it does for `skip_serializing_if`
- `#[serde(skip_deserializing)]` -- the read half: the key is written into every payload while a supplied one is discarded, so the member keeps a required key

The three `skip` spellings are three different wires, and [Optional Fields](#optional-fields) reads each one in both directions, positional slots included.

If the `serde` feature is disabled but serde attributes are present, you will see compile-time warnings and field names will not be transformed.

## Important Notes

1. **Naming Convention**: A type publishes under the Rust ident it is declared with, spelled exactly as written. `#[model_schema(name = "...")]` is the only way to publish under another name, and it moves that name on every surface at once. Two declarations reaching one published name are refused at expansion. See [Published Names](#published-names-name).

2. **Type References**: Nested types reference each other by the name the referenced type publishes -- its Rust ident, or its `name` override.

3. **Map Keys**: `HashMap` and `BTreeMap` are both supported, and the key decides what the map describes as: a `String` key gives an object open to any string key, and a plain `#[model_schema()]` enum key gives one closed to the enum's members. A brand and a `#[model_schema()]` alias answer by their inner and their target, keeping their own name on the TypeScript and Zod surfaces. A key path proved to be one serde writes as no key at all, and any sequence-wrapped key, is refused at expansion; every other key describes as an open object. See [Collections and Maps](#collections-and-maps).

4. **Array Types**: `Vec<T>` becomes `Array<T>` in TypeScript, one `Array<...>` per level written — `Vec<Vec<T>>` is `Array<Array<T>>`.

5. **Optional Fields**: `Option<T>` becomes `field: T | undefined` in TypeScript unless `#[model_schema_prop(ts_optional)]` asks for `field?: T`, and `z.union([z.null().transform(() => undefined), type, z.undefined()]).prefault(undefined)` in Zod v4 either way.

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
  email: z.union([z.null().transform(() => undefined), z.string(), z.undefined()]).prefault(undefined),
  age: z.union([z.null().transform(() => undefined), z.number().int(), z.undefined()]).prefault(undefined),
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
- **Type safety**: Validates exactly the payloads the TypeScript member admits, absent key included.
