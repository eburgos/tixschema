# CLAUDE.md

**Note**: This project uses [bd (beads)](https://github.com/steveyegge/beads) for issue tracking. Use `bd` commands instead of markdown TODOs. See [AGENTS.md](AGENTS.md) for workflow details.

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**tixschema** is a Rust procedural macro library that generates TypeScript type definitions and Zod v4 validation schemas from Rust structs and enums. This ensures type safety and consistency between Rust backends and TypeScript frontends in Tixena applications.

## Task Management System

**MANDATORY: Use `bd` (beads) for ALL task tracking.**

- ✅ **DO**: Use `bd` commands for issue/task tracking (`bd create`, `bd ready`, `bd update`, `bd close`)
- ❌ **DO NOT**: Use TodoWrite tool, markdown TODO lists, or any other task tracking system
- 📖 **Reference**: See [AGENTS.md](AGENTS.md) for complete bd workflow and commands

**Quick bd commands:**
```bash
bd ready --json           # Check for available work
bd create "Task" -t task -p 2 --json
bd update <id> --status in_progress --json
bd close <id> --reason "Completed" --json
```

## Essential Commands

### Testing
```bash
# Quick test with default features (recommended for rapid iteration)
just quick
# or
cargo test

# Comprehensive test - all 32 feature combinations (run before commits)
just test

# Test specific feature combinations
just test-minimal          # No features
just test-default          # Default features only
just test-named-features   # Key combinations

# Run a specific test by name
just test-name <TEST_NAME>
# or
cargo test <TEST_NAME>
```

### Code Quality
```bash
# Check code (runs cargo check + clippy with warnings as errors)
just check

# Format code
just fmt

# Check all feature combinations without running tests
just check-all
```

### Build & Documentation
```bash
# Build the project
cargo build

# Build and open documentation
just docs-open

# Build docs only
just docs
```

### Full CI Pipeline (what CI runs)
```bash
just ci
# Runs: clean, check-all, test, fmt
```

## Code Architecture

### Macro Processing Flow

1. **Entry Point** ([lib.rs](src/lib.rs))
   - Exports two proc macros: `#[model_schema]` and `#[model_schema_prop]`
   - `model_schema` is the main macro for structs/enums/type aliases
   - `model_schema_prop` is for field-level customization

2. **Macro Execution** ([model_schema.rs](src/model_schema.rs))
   - `exec_model_schema()` routes to `process_struct()`, `process_enum()`, `process_type_alias()`, or `process_branded_newtype()`
   - Parses Serde attributes when `serde` feature enabled
   - Extracts example code from doc comments (` ```rust example` fences)
   - Generates methods: `ts_definition()`, optionally `json_schema()`, optionally `schema_example()`, and optionally `validate()`
   - `process_branded_newtype()`: handles `#[serde(transparent)]` single-field tuple structs, generating Zod brand schemas or `unique symbol` branded TypeScript types

3. **Type Analysis** ([field_type.rs](src/field_type.rs))
   - `FieldDef`: Core data structure representing a field's type, optionality, docs, etc.
   - `FieldDefType`: Enum categorizing Rust types (primitives, collections, custom types, ObjectId, etc.)
   - `get_field_def()`: Recursively analyzes Rust types and builds FieldDef trees
   - Handles: Option<T>, Vec<T>, HashMap<String, T>, nested types, generics

4. **Feature Modules** ([features/](src/features/))
   - `serde.rs`: Parses Serde attributes (`rename`, `rename_all`, `tag`, etc.)
   - `zod.rs`: Generates Zod v4 schema strings (`z.string()`, `z.union()`, etc.), embeds examples in `.meta()`
   - `jsonschema.rs`: Generates JSON schema objects
   - `object_id.rs`: MongoDB ObjectId type detection and schema generation
   - `model_schema_prop.rs`: Parses field-level customization attributes (`pattern`, `minLength`, `maxLength`, `minimum`, `maximum`, `literal`, `as`, `preprocess`)

5. **Code Generation** ([generation/](src/generation/))
   - `typescript.rs`: Generates TypeScript type definitions and Zod schemas
   - Combines FieldDef trees into complete type/schema strings
   - Handles discriminated unions, generics, nested types
   - Calls `schema_example()` method to embed examples in Zod `.meta()` when available

6. **Example Extraction** ([utils.rs](src/utils.rs))
   - `extract_example_from_docs()`: Parses ` ```rust example` code fences from doc comments
   - Returns first example found (if multiple exist)
   - Example code is inserted into generated `schema_example()` method
   - Compiler validates example code at compile time

### Key Data Structures

**FieldDef** (central type representation):
```rust
pub struct FieldDef {
    pub name: String,                    // Field name (respects Serde rename)
    pub docs: String,                    // Rust doc comments → JSDoc
    pub field_type: FieldDefType,        // The actual type category
    pub array_depth: u8,                 // Vec<T> → Array<T>, one level per Vec/slice/set written
    pub nullable_levels: Vec<u8>,        // Which array levels were written as Option
    pub array_lengths: Vec<(u8, usize)>, // Length of each level written as [T; N]
    pub model_schema_prop_meta: ...,     // Field-level overrides
}
```

Levels count from the innermost value outward, so `Vec<Option<T>>` records level 0 — a `null`
among the array's items, rendered `Array<T | null>` — while `Option<Vec<T>>` records level 1, a
`null` in place of the whole array, rendered `Array<T> | undefined` in a field and
`Array<T> | null` in a slot. `is_optional()` asks the question of the outermost level, the only
one whose rendering depends on where the field sits.

`array_lengths` numbers levels the same way: `[[T; 2]; 3]` records `(0, 2)` and `(1, 3)`. Only the
validating surfaces spend it — `minItems`/`maxItems` in the JSON schema, `.length(N)` in Zod —
while TypeScript stays `Array<T>`. A length the expansion cannot read (const generic, `const`
item, computed expression) records nothing and describes as an unbounded array.

**FieldDefType** (type categories):
- Primitives: `Boolean`, `String`, `U8-U64`, `I8-I64`, `F32`, `F64`, `Usize`, `Isize`
- Complex: `SiblingType` (custom types), `Map` (HashMap), `Tuple`
- Special: `ObjectId` (MongoDB, feature-gated), `StringLiteral`, `TypeParam`, `Unknown`

`TypeParam` is one of the enclosing item's own type parameters, which `erase_type_parameters`
rewrites a written name into. The three surfaces answer for it differently: TypeScript renders the
name it was written with, JSON Schema describes it as `{}`, and Zod composes the argument the
enclosing factory binds for it (`idType` for `IdType`). A surface with no factory to bind an
argument — an alias, a branded newtype — calls `with_opaque_type_parameters` first and writes
`z.unknown()`.

### Feature Flag System

The crate uses 5 optional features for minimal dependencies:

- `serde`: Enables Serde attribute parsing and field renaming
- `zod`: Enables Zod schema generation (v4 syntax)
- `jsonschema`: Enables `json_schema()` method generation
- `object_id`: Enables MongoDB ObjectId type support
- `typescript`: Enables TypeScript type generation

**Total combinations tested**: 2^5 = 32 (via `cargo-hack` in CI)

**Default configuration**: All features enabled

## Critical Development Rules

### 1. The Name a Type Publishes Under

A type publishes under the Rust ident it is declared with, spelled exactly as written. Nothing is read off that spelling — no suffix is taken off it and no part of it is rewritten.

`#[model_schema(name = "...")]` is the only publishing override, and it moves the name on every surface at once: the `export type` line, the Zod consts, the `$defs` key a self-referential document hoists under, and every reference another type writes.

```rust
#[model_schema()]
pub struct User { ... }                             // → TypeScript: User

#[model_schema()]
pub struct UserData { ... }                         // → TypeScript: UserData

#[model_schema(name = "User")]
pub struct UserData { ... }                         // → TypeScript: User
```

A renamed item also publishes its Rust ident as an alias of the moved name — `export type UserData = User;`, `export const UserData$Schema = User$Schema;` — which is what lets a type declared *above* it, with only the ident to name it by, still resolve. `ident_schema_module_name` names the generated Rust module from the ident for the same reason: an override never moves it.

`compute_item_export_name` is the one seam a declared item's published name is read from, and `compute_alias_export_name` the alias counterpart — an alias has no surface name of its own and takes the `Type` suffix without an override.

Two declarations cannot publish under one name: the emitted types, schemas and definitions are one flat namespace, so `published_name_collision_errors` refuses the second at `exec_model_schema` — the ungated seam, so the verdict is the same in every feature combination — naming both declarations. `claim_published_name` holds the claims, keyed the opposite way to the `ALIAS_INFO` registry so an ident reclaiming its own name is never a collision.

### 2. Required Derives and Imports

```rust
use tixschema::model_schema;
use serde::{Deserialize, Serialize};

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MyType { ... }
```

### 3. HashMap Key Restriction

A map key must be one serde can write as a JSON object key. `String` is the open case: any string
is a key. `bool`, the numeric types, `char`, and the chrono date/time types are accepted too —
serde stringifies each one (`7` → `"7"`, `true` → `"true"`) — so the map describes as an open
object. A plain `#[model_schema()]` enum key narrows the object to its members. A key serde does
not stringify — a struct, tuple, `Vec`/array, `Option`, nested map, or `ObjectId` — is refused at
expansion:

```rust
// ✅ Supported — string, and any key serde stringifies (bool/numeric/char/chrono)
pub struct Config {
    pub settings: HashMap<String, String>,
    pub counts: HashMap<u32, String>,
}

// ❌ NOT supported - refused at expansion
#[model_schema()]
pub struct KeyStruct {
    pub value: String,
}

#[model_schema()]
pub struct BadMapKey {
    pub m: HashMap<KeyStruct, u32>,
}
// error: field `m`: a map key must be a plain `#[model_schema()]` enum, whose members
// become the object's keys — `KeyStruct` resolves to a type with no `enum_members()`
```

On the two nominal surfaces a key renders in the form serde **writes** it wherever the key's own
value form is not a TypeScript property key, and under the name it was written with wherever it is.
`typescript_map_key_typename` and `zod_map_record_call` are the one pair of seams that decide, both
reading `FieldDef::map_key_wire`, so the surfaces cannot drift:

| key reached | TypeScript | Zod |
|-------------|------------|-----|
| `String`, `char`, `PathBuf`, type parameter | own type (`string`, …) | own schema |
| plain enum | own type (`MetricSlot`) | `z.partialRecord(MetricSlot$Schema, V)` |
| the integer types | `number` | `z.record(z.number().int(), V)` |
| `NaiveDate` / `NaiveTime` / `NaiveDateTime` | `string` | own schema |
| `bool` | `"true" \| "false"` | `z.partialRecord(z.enum(["true", "false"]), V)` |
| `DateTime<Tz>` | `string` | `z.record(z.iso.datetime({ offset: true }), V)` |

The last two rows are the ones whose value form spells no property key: `boolean` and `Date` are
rejected by `Record`'s own constraint, `z.boolean()` rejects the string serde wrote, and
`z.coerce.date()` accepts it only to rewrite every key into a locale-dependent rendering. A brand or
alias over one of those loses its name at a key position for the same reason — the only binding it
publishes is that value-shaped schema — while a brand or alias over any other row keeps its name
exactly as before. The constructor moves with the key because `z.record` over an enumerated key
demands every member: a key whose published Zod binding enumerates — a plain enum, and any brand or
alias over one, `.brand()` being type-level only — is written with `z.partialRecord`, which admits
the subset a `HashMap` holds while still refusing a key outside the enumeration. JSON Schema is
unaffected at every position: a stringified key describes as `{"type": "object",
"additionalProperties": true}` whatever its wire form.

### 4. Type Mappings (Rust → TypeScript)

- `String` → `string`
- `bool` → `boolean`
- Numeric types → `number`
- `Option<T>` → `T | undefined` (Zod: `z.union([z.null().transform(() => undefined), T, z.undefined()]).prefault(undefined)`, accepting an explicit `null` and coercing it to `undefined`; JSON Schema `anyOf: [T, {"type": "null"}]`, key left out of `required`); with `#[model_schema_prop(ts_optional)]` the TypeScript key becomes optional instead: `field?: T`; with `#[model_schema_prop(nullable)]` all three surfaces render `T | null` with the key **required** instead: Zod `z.union([T, z.null()])`, JSON Schema `anyOf: [T, {"type": "null"}]` with the key in `required`
- `Vec<T>` → `Array<T>`
- `HashMap<String, T>` → `Partial<Record<string, T>>`
- Custom types → Reference by the name the referenced type publishes (its ident, or its `name` override)
- `ObjectId` → `ObjectId` (with $oid validation)
- `DateTime<Tz>` → `Date` (Zod `z.coerce.date()`); with `#[model_schema_prop(as_number)]` → `number` (inline epoch-ms coercer)
- `NaiveTime` → `string` (Zod `z.iso.time()` wrapped in an inline preprocessor that also accepts millis-since-start-of-day)
- `#[serde(flatten)]` field → TypeScript intersection (`A & B`), Zod `.and(...)`. A source writing more than one key set — a registered choice, or one reached through an `Option` — multiplies instead: `zod_merged_joins` builds one `.and(...)` chain per key set, `zod_merged_statements` binds the object to `{Type}$OwnSchema` and joins the copies where the object is a struct's own published binding, and `zod_merged_object` writes the object out once per copy where it is an enum variant's fragment of a larger literal. Either way the item contributes exactly one member to whatever union encloses it
- `#[serde(untagged)]` enum → TypeScript union (`A | B`), Zod `z.union([...])`, JSON Schema `anyOf`
- Type parameter → TypeScript parameter (`X<IdType>`), Zod `X$SchemaFactory(idType)` (memoized, one cache level per parameter) plus `X$SchemaDefault` — that same factory called at the item's own declared `default_types`, checks included — JSON Schema the document of whatever fills it — `default_types(...)` where the type stands alone, the reference site's arguments where a field embeds it. A lifetime is dropped on every surface, a borrowed value writing what its owned form writes; a const renders as an array length only, and is refused where it is handed to a written type as an argument

### 5. Zod v4 Requirements

**⚠️ IMPORTANT: This crate requires Zod v4 for full functionality.**

Frontend dependencies:
```bash
npm install zod@^4.0.0
```

Generated schemas use Zod v4's modern syntax:

```typescript
// ✅ Generated (Zod v4 compatible)
export const User$Schema = z.strictObject({
  id: z.string(),
  name: z.string(),
  email: z.union([z.null().transform(() => undefined), z.string(), z.undefined()]).prefault(undefined),      // Modern v4 syntax, null coerced to undefined
  age: z.union([z.null().transform(() => undefined), z.number().int(), z.undefined()]).prefault(undefined),  // Works with JSON schema generation
});

// ❌ OLD FORMAT (no longer generated)
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

**Benefits of Zod v4**:
- **JSON Schema Generation**: Can generate JSON schemas from Zod schemas
- **Cleaner Code**: No transform functions needed
- **Better Performance**: No runtime transform overhead

### 6. Serde Attribute Support

The macro respects Serde attributes when the `serde` feature is enabled:

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub user_id: String,  // → userId in TypeScript
    #[serde(rename = "emailAddress")]
    pub email: String,    // → emailAddress in TypeScript
}
```

### 7. Field Validation Attributes (`#[model_schema_prop(...)]`)

All validation constraints generate checks in **Zod (frontend), JSON Schema, and Rust (Serde deserialization)**:

| Attribute | Field Type | Zod | JSON Schema | Rust serde |
|-----------|------------|-----|-------------|------------|
| `pattern = "regex"` | `String` | `.check(z.regex(/regex/))` | `"pattern"` | validator + deserializer |
| `minLength = N` | `String` | `.min(N)` | `"minLength"` | validator + deserializer |
| `maxLength = N` | `String` | `.max(N)` | `"maxLength"` | validator + deserializer |
| `minimum = N` | numeric | `.min(N)` | `"minimum"` | validator + deserializer |
| `maximum = N` | numeric | `.max(N)` | `"maximum"` | validator + deserializer |
| `literal = "val"` | `String` | `z.literal("val")` | `{"type": "string", "const": "val"}` | — |
| `literal = true` | `bool` | `z.literal(true)` | `{"type": "boolean", "const": true}` | — |
| `literal = 214` | numeric | `z.literal(214)` | `{"type": "number", "const": 214}` | — |
| `preprocess = ["fn"]` | any | `z.preprocess(fn, ...)` | — | — (Zod-only) |
| `ts_optional` | `Option<T>` | — | — | — (TypeScript-only) |
| `as_number` | `DateTime<Tz>` | inline `z.preprocess(..., z.number())` | — | — (TS+Zod) |
| `nullable` | `Option<T>` | `z.union([T, z.null()])`, key required | `anyOf: [T, {"type":"null"}]`, key required | guard: refuses a key-dropping serde attr |

Multiple constraints on one field are combined. Multiple `preprocess` functions nest:
`z.preprocess(fn1, z.preprocess(fn2, innerSchema))`.

`literal` takes a string, boolean, integer or float literal. The kind written must match what the
field's Rust type can carry — a boolean literal only on a `bool` field, a numeric literal only on a
numeric field (`u8`–`f64`), a string literal only on a `String` field — or the attribute is a
compile error naming both the literal's kind and the field's declared type. A numeric literal is
stored as `f64` and rendered without a trailing `.0` on TypeScript and Zod; the JSON Schema `const`
is always written under `"type": "number"`, never `"integer"`.

`ts_optional` is a bare flag (no value): it renders an `Option<T>` field as the optional TypeScript key `field?: T` instead of the default `field: T | undefined`. Zod and JSON Schema output are unchanged. It is only valid on `Option<T>` fields (non-`Option` is a compile error) and composes with `as = Type`. The member must also have a key for the flag to make optional, and `validate_ts_optional_flag` refuses both positions where it has none, in every build: a positional slot writes no key at all, and one a serde attribute takes off the wire in both directions (`skip`, or `skip_serializing` and `skip_deserializing` together) is described on no surface. An attribute that drops the key one way only (`skip_serializing_if`, `skip_serializing`) leaves the member standing, and the flag still decides its spelling.

`as_number` is a bare flag (no value): it renders a `DateTime<Tz>` field as a `number` (epoch milliseconds) with an inline self-contained Zod coercer, instead of the default native `Date` (`z.coerce.date()`). It is only valid on `DateTime<Tz>` fields (anything else is a compile error) and is honored on a tuple-variant `DateTime<Tz>` enum payload.

`nullable` is a bare flag (no value): on an `Option<T>` field it renders `T | null` with the key **required** on TypeScript, Zod and JSON Schema, instead of the default coercing `T | undefined` with the key left optional. It is only valid on `Option<T>` fields (non-`Option` is a compile error), is refused together with `ts_optional` (the two disagree about the key), and composes with `preprocess` — the preprocess wrap goes around the whole nullable union. With the `serde` feature on, it is also refused together with a key-dropping serde attribute (`skip_serializing_if`, `skip_serializing`, `skip`) — the flag declares the key always written, so dropping it would let serde write a payload the generated schema does not admit. The mirror guard, `check_optional_field_serialization`, requires that same attribute on a bare `Option<T>` field carrying no `nullable`.

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct User {
    #[model_schema_prop(minLength = 3, maxLength = 50, pattern = "^[a-z0-9_]+$")]
    pub username: String,

    #[model_schema_prop(minimum = 0, maximum = 120)]
    pub age: u32,

    #[model_schema_prop(preprocess = ["epoch_to_date"])]
    pub date_value: NaiveDate,
}
```

#### Generated `validate()` method

When any field has constraints, the macro generates a `validate(&self) -> Result<(), Vec<String>>` method that aggregates all per-field errors. This is useful when constructing instances in code rather than through serde (serde validates automatically):

```rust
let result = my_instance.validate();
match result {
    Ok(()) => println!("Valid"),
    Err(errors) => println!("Errors: {:?}", errors),
}
```

The macro also generates into the type's schema module:
- `validate_{field}_value(&FieldType) -> Result<(), String>` — pure static validator per field
- `deserialize_{field}(D) -> Result<FieldType, E>` — serde hook that calls the static validator

For a generic type (one with type parameters), `validate()` is emitted only at the declared default
instantiation — `impl DocumentId<String> { pub fn validate(&self) -> Result<(), Vec<String>> { … } }`,
never a blanket `impl<IdType> DocumentId<IdType>` — because the constraints it checks (`minLength`,
`pattern`, …) belong to that one concrete filling, the same one `X$SchemaDefault` enforces. Rust
inherent impls do not specialize, so a blanket impl would make a downstream `impl
DocumentId<ObjectId> { pub fn validate(&self) -> … }` a duplicate-definition error; pinning to the
default leaves that door open for an author who wants their own validation on another instantiation.
The schema delegates (`ts_definition()`, `zod_schema()`, `json_schema()`, …) are unaffected and stay
on the type's own generic `impl<IdType> DocumentId<IdType>`, since they do not depend on the
constraints.

### 8. Branded Newtypes

Single-field tuple structs with `#[serde(transparent)]` are treated as branded/opaque types. The brand publishes under its Rust ident unless `#[model_schema(name = "...")]` names another, and that name reaches the surface twice: as the exported type and as the brand tag the values carry.

A non-generic brand publishes a `$RawSchema`/`$Schema` const pair:

```rust
#[model_schema()]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CorrelationId(pub String);
```

With `zod` + `typescript` features:
```typescript
export type CorrelationId = string & $brand<"CorrelationId">;
const CorrelationId$RawSchema = z.string().brand<"CorrelationId">().meta({
  description: "CorrelationId",
});

export const CorrelationId$Schema: typeof CorrelationId$RawSchema = CorrelationId$RawSchema;
```

A brand with a type parameter publishes `X$SchemaFactory` plus `X$SchemaDefault` instead, exactly
like any other generic item — never a plain `const`:

```rust
#[model_schema(default_types(IdType = String))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UserId<IdType>(pub IdType);
```

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
```

With `typescript` only (no `zod`):
```typescript
declare const __brand_UserId: unique symbol;
export type UserId<IdType> = IdType & { readonly [__brand_UserId]: true };
```

Rules:
- Generic parameter names are preserved exactly (`IdType` stays `IdType` in TypeScript)
- A brand with no type parameter publishes the `$RawSchema`/`$Schema` const pair; a brand with a
  type parameter publishes `$SchemaFactory`/`$SchemaDefault` instead, the default binding its
  factory call to `{Name}$RawSchemaDefault` first — the parameter's inner composes the factory's
  bound argument (`idType`), never the opaque `z.unknown()`
- Either binding is annotated `typeof` the raw `const` beneath it, never a Zod class: `.brand()`
  narrows at the value position, and the class an inner renders to is zod's to decide
  (`z.coerce.date()` is a `ZodCoercedDate`, `z.iso.date()` a `ZodISODate`, a brand over a brand a
  `$ZodBranded` of a `$ZodBranded`). Reading the annotation off the value is the one spelling that
  stays true of all of them
- Serde transparent serialization works normally — the newtype is invisible in JSON
- The slot takes no `#[model_schema_prop(...)]`: a brand publishes its inner's own schema with a
  `.brand()` written onto it, so no key written there reaches any surface, and one written there is
  refused at `exec_model_schema` — the ungated seam, so the verdict is the same in every feature
  combination — with the item re-emitted stripped of it. The three checks a brand does carry are
  written on the type: `#[model_schema(pattern = "...", minLength = N, maxLength = N)]`. A
  `#[serde(transparent)]` struct with a *named* field is no brand and is untouched
- An item whose published expression *is* another item's binding — an alias of a brand, a one-slot
  tuple struct over one, and the `$SchemaDefault` of either where it declares a parameter — is
  annotated `typeof {Name}$RawSchema` rather than `ZodType<{Name}>`, so the brand's narrowing
  survives the republish. An item that builds an expression of its own keeps `ZodType<{Name}>`

### Generic Types and Zod Factories

A Zod schema is a runtime value and TypeScript generics do not exist at runtime, so a generic
struct, tuple struct or enum has no one schema to publish. It exports `X$SchemaFactory` instead —
a function taking one required schema argument per parameter — while a type that binds no type
parameter keeps exporting `X$Schema` exactly as before. `zod_binding_suffix` is the one seam that
decides which, and `zod_published_binding` the one seam that writes either.

Beside the factory, a generic type also exports `X$SchemaDefault` — the factory called at the
type's own declared `default_types`, so a consumer who wants the ordinary filling shares the memo
rather than building a second schema for it. `zod_default_block` builds the const, through
`default_zod_rendering`, the same `get_field_def` path every field renders through; where a
declared default names another generic item at exactly the arguments that item calls its own,
the rendered argument folds onto that item's `$SchemaDefault` instead of reconstructing a factory
call the memo would not share with it — see `record_zod_default_arguments`. `zod_binding_reexport`
covers both bindings wherever a renamed item re-exports its factory.

`X$SchemaDefault` is a module-scope `const`, and any argument `default_zod_rendering` renders as a
reference to another item — the fold above, or an ordinary (non-folding) call to that item's own
`X$SchemaFactory` — names one more module-scope `const`. One macro invocation sees one type, so
nothing here can know whether that `const` is written above this one in the generated module or
below it; `default_zod_rendering` defers every such reference through `deferred_zod_operand`, the
same `z.lazy(() => …)` wrap `.and(...)` already relies on for a flattened base. A self-contained
expression like `z.string()` names no sibling `const` and is left eager. The fold's own gate
(`array_depth == 0 && !is_optional()`, a *direct* sibling) only scopes the fold — a default
wrapped in a `Vec`/`Option`/`Map`/`Tuple` has no bare binding to fold onto — but deferral is not
bounded by it: `FieldDef::names_a_sibling_binding` walks the whole rendered tree, so
`Tagged$SchemaFactory` inside `z.array(Tagged$SchemaFactory(z.string()))` defers exactly as a bare
`Tagged$SchemaFactory(z.string())` does, the `z.lazy` wrapping the whole expression rather than the
factory call alone. The deferral is also what ends a cycle between two declared defaults —
`CycleLeader`'s naming `CycleFollower` and `CycleFollower`'s naming `CycleLeader` back: neither can
have registered the other's arguments yet when it expands, so neither side folds, and both calls
read the other's factory behind `z.lazy` rather than at the top of a `const` initializer — the
module loads regardless of which of the two names the consuming project's entity list writes
first.

Each factory memoizes on the identity of its arguments, one `WeakMap` level per parameter
(`zod_cache_type` writes the nesting, `zod_factory_body` the walk), so two calls with the same
argument objects return the identical schema and no two argument lists collide.

A `WeakMap` fixes both of its own parameters at construction, so its value type cannot depend on
its key type and the store is written at the widened `ZodType` spelling throughout. What recovers
the precise types is an overload, which `zod_factory_declaration` emits: the signature a caller
sees names real type parameters, the implementation beneath it takes the widened ones the store is
keyed at, and the read is therefore already the implementation's return type. Nothing emitted is
`as`, `any` or `unknown`; nothing a caller passes is written to; and there is no preamble — a
generated module is the types in it and nothing shared between them.

Every parameter is a real TypeScript type parameter (`<IdType extends ZodType>`), never a bare
`ZodType` annotation — `ZodType` defaults its own parameters, so an argument annotated with it
infers every field as `unknown`.

A generic type may reach itself, directly or through a second type reaching back. JSON Schema
hoists it into `$defs` once and points a `$ref` at that definition; TypeScript writes the name
inside itself with its arguments; Zod calls the factory again with the argument the outer call was
handed, the memo cache being what ends the recursion — the schema is cached before the recursive
call is made. Which of the item's two bindings a self-reference names is read off the store
`record_zod_factory` writes, and that is written at `exec_model_schema` ahead of every shape rather
than where the binding is finally spelled: the fields are rendered before then, so an answer stored
on the item's own registry entry would be read before the item had put one there.

`write_field_type_and_schema` defers a member whose type reaches the item being defined, and one
that reaches a type declared *below* it — `reaches_a_type_declared_later`. The rule applies whether
or not the referenced type declares a parameter of its own: a zero-argument sibling not yet
registered is exactly as much a forward reference as a generic one, since its `X$Schema` is read
eagerly at the top of the referencing item's own module-scope const initializer. The second clause
is what ends a cycle spanning two types: every cycle contains at least one reference pointing
forward, since declaration positions cannot strictly decrease all the way round one, and what is
left once those are deferred cannot cycle. The deferral is a getter rather than `z.lazy`, which at
an operand
position collapses the factory's inferred return type to `any`.

### Declaring a Default Type per Parameter

JSON Schema has no type parameters, so a generic item's document is built from one concrete
filling, and `default_types(IdType = String, DateType = f64)` is where an author names it — one
`Parameter = Type` pair per type parameter, in any order, beside every other item-level argument.

`default_types_guard_errors` reads the declaration against the item's own parameters at
`exec_model_schema`, the one seam every expanded shape is dispatched from, so a struct, a tuple
struct, a branded newtype, an enum and an alias are all answered the same way and before any of
them is split off. It refuses in both directions:

- an entry naming nothing the item declares — refused in **every** feature configuration, spanned
  on the entry, since a misspelled parameter fills nothing while the one it was meant for keeps no
  default;
- a type parameter with no entry — refused only under `#[cfg(feature = "jsonschema")]`, spanned on
  the parameter, because nothing else reads the default;
- `default_types` on an item with no type parameter, and an empty `default_types()`, both being a
  declaration with nothing to declare;
- an entry filled at a type the field-definition dispatch has no arm for — refused under
  `#[cfg(any(feature = "zod", feature = "jsonschema"))]`, the two surfaces that read a declared
  filling, spanned on the filling. That dispatch takes a name it does not recognise for another
  `#[model_schema]` item, which is right for a sibling declared below and gibberish for `i128`: the
  emission names an `i128_schema` module nothing publishes. Tokens cannot separate the two, so
  `is_undescribable_primitive` refuses only what provably cannot become a sibling — the primitive
  names the language reserves that the dispatch answers for nowhere
  (`i128`, `u128`, `f16`, `f128`). `char` is not among them: it has an arm of its own, and a
  filling at it describes as the one-character string serde writes.

A const parameter earns its own refusal, from `const_parameter_argument_errors` at the same seam
and gated on `typescript`/`jsonschema`: a const handed to a written type as an *argument* stands
where a type is read, so the JSON side would call into a module named after it and the TypeScript
side would write a name its declaration binds nothing for. A const written as an array length is
untouched — that is the one position it renders in, as an unbounded array.

There is no fallback filling. A guessed one produces a document that silently rejects valid
payloads, which is what the declaration exists to prevent.

### String Constraints on a Generic Brand's Declared Default

A branded newtype's own `pattern`/`minLength`/`maxLength` normally have no inner to measure when
the inner is one of the brand's own bare type parameters — a parameter is the opaque value on
every surface, and `z.unknown()` carries no `.min`/`.max`. `branded_constraint_inner_error` reads
the declared default for that parameter instead of refusing outright: `declared_default_field`
resolves the same `default_types` entry `zod_default_block` reads, and `non_string_inner_shape`
asks of it the same question asked of a concrete argument — string-shaped, the checks compose onto
the default; not string-shaped, the refusal names the default rather than the parameter, through
`declared_default_constraint_message`. An entry the declaration left out falls back to `String`,
the same fallback `schema_example_value_type` uses for the identical gap, so the guard alone
requires nothing; only `jsonschema`'s own separate requirement (above) forces every parameter to
declare one.

The checks never reach the factory's own parameter — `branded_zod_inner` renders a bare-parameter
inner with no check appended regardless of genericness, since a caller filling the parameter with
something other than the default (an `ObjectId` schema, say) must not inherit bounds meant for it.
They land once, on `$SchemaDefault`'s argument for that one parameter: `zod_default_block` and
`zod_factory_block` both take a `ZodDefaultInputs` bundling `default_types` with the optional
`(parameter, checks)` pair a branded newtype's own expansion supplies — `None` for every ordinary
generic struct, tuple struct, enum and alias, which carry no type-level string constraint of their
own. The JSON surface needs no separate change: a bare parameter's document is already the runtime
argument bound to the parameter's declared filling (`json_argument_value`, defaulting through
`declared_filling_json_schema_value`), which `branded_layered_over` narrows with the brand's own
bounds through the same `allOf` a named inner is narrowed through — never the inert `{}` a
parameter with no default at all would describe as.

### Adding Examples to Types

To add examples to your types for inclusion in Zod schemas:

```rust
/// User profile description
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

**Key Points:**
- Use the exact syntax: ` ```rust example` (note the space and `example` keyword)
- The example code must type-check at compile time
- The last expression in the block should evaluate to the type
- Examples are **optional** - if not provided, no `schema_example()` method is generated
- If multiple examples exist, only the first one is used
- Examples respect Serde attributes (field names are serialized correctly)

**Generated Code:**
- A `schema_example()` method is generated that returns `serde_json::Value`
- The Zod schema includes `.meta({ example: <serialized_json> })`
- The example is serialized using Serde, so it matches your API's JSON format

## Common Development Tasks

### Adding a New Type Mapping

1. Add variant to `FieldDefType` in [src/field_type.rs](src/field_type.rs)
2. Update `get_field_def()` to detect the new type
3. Add TypeScript generation logic in [src/generation/typescript.rs](src/generation/typescript.rs)
4. Add Zod generation logic in [src/features/zod.rs](src/features/zod.rs) (if `zod` feature)
5. Add tests in appropriate test file (e.g., `tests/primitive_types_tests.rs`)

### Adding a New Feature

1. Add feature to `Cargo.toml` `[features]` section
2. Create feature module in [src/features/](src/features/)
3. Add `#[cfg(feature = "...")]` guards in affected code
4. Update `Features` struct in [src/features/mod.rs](src/features/mod.rs)
5. Add tests that verify behavior with/without the feature

### Adding Tests

Tests are organized by category in [tests/](tests/):

- `basic_tests.rs`: Simple structs, basic types
- `collection_tests.rs`: Vec, HashMap, nested collections
- `enum_tests.rs`: Plain enums, discriminated unions
- `serde_tests.rs`: Serde attribute handling
- `mongodb_tests.rs` / `mongodb_real_tests.rs`: ObjectId support
- `edge_cases_tests.rs`: Complex scenarios, deep nesting
- `semantic_types_tests.rs`: Type aliases

**Test pattern**:
```rust
#[test]
fn test_my_feature() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct Test { ... }

    let ts = Test::ts_definition();
    assert!(ts.contains("expected output"));
}
```

### Running Single Tests During Development

```bash
# Run a specific test
cargo test test_basic_struct

# Run all tests in a module with output
cargo test basic_tests -- --nocapture

# Run tests matching a pattern
cargo test objectid
```

## TypeScript Generation Pattern

Standard pattern for generating TypeScript files:

```rust
// Define entities enum
pub enum MyEntities {}

impl MyEntities {
    pub fn get_entities() -> (String, Vec<String>) {
        (
            "Generated Types".to_string(),
            vec![
                User::ts_definition(),
                Address::ts_definition(),
                // Add all types here
            ],
        )
    }
}

// Generation function
pub fn generate_ts_schemas(target_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut file_contents = String::from("import { z } from \"zod\";\n\n");
    let (header, type_definitions) = MyEntities::get_entities();

    file_contents.push_str(&format!("/*\n * {}\n */\n\n", header));
    file_contents.push_str(&type_definitions.join("\n\n"));
    file_contents.push('\n');

    fs::write(target_path, file_contents)?;
    Ok(())
}

// Test that runs generation
#[test]
fn test_generate_typescript() {
    generate_ts_schemas("../frontend/src/types/generated.ts").unwrap();
}
```

## Generated Output Understanding

The macro transforms Rust types to TypeScript following these rules:

1. **Type Name Transformation**: None. A type publishes under the Rust ident it is declared with, and `#[model_schema(name = "...")]` is the only thing that moves it.
2. **Field Names**: Respect serde rename attributes (`#[serde(rename = "...")]`, `#[serde(rename_all = "...")]`)
3. **Optional Fields**: `Option<T>` becomes `T | undefined` in TypeScript and `z.union([z.null().transform(() => undefined), type, z.undefined()]).prefault(undefined)` in Zod, accepting both an absent key and an explicit `null`
4. **Arrays**: `Vec<T>` becomes `Array<T>` in TypeScript
5. **Maps**: `HashMap<String, T>` becomes `Partial<Record<string, T>>` in TypeScript
6. **Nested Types**: Reference other types by the name they publish (their ident, or their `name` override)
7. **MongoDB ObjectId**: `ObjectId` becomes `ObjectId` in TypeScript with proper JSON schema validation
8. **ObjectId Serialization**: Uses MongoDB format `{ "$oid": "hex_string" }`
9. **ObjectId Validation**: Includes regex validation for 24-character hexadecimal strings

## Testing Best Practices

### 1. Always Test TypeScript Generation

Include generation tests in your test suite to catch issues early:

```rust
#[test]
fn test_generate_typescript() {
    generate_ts_schemas("../frontend/src/types/generated.ts").unwrap();
}
```

### 2. Validate JSON Schemas

When using the `jsonschema` feature, test that generated schemas are valid:

```rust
#[cfg(feature = "jsonschema")]
#[test]
fn test_json_schema_validity() {
    let schema = MyType::json_schema();
    assert!(schema.get("type").is_some());
}
```

### 3. Test Serialization Roundtrips

Ensure serde serialization/deserialization works correctly:

```rust
#[test]
fn test_serde_roundtrip() {
    let original = MyType { /* ... */ };
    let json = serde_json::to_string(&original).unwrap();
    let deserialized: MyType = serde_json::from_str(&json).unwrap();
    assert_eq!(original, deserialized);
}
```

### 4. Version Control Generated Files

Consider committing generated TypeScript files to version control for easier code review and frontend development without requiring Rust builds.

### 5. CI/CD Integration

Run generation tests in your CI pipeline to ensure frontend types stay in sync with backend changes (see [CI/CD Integration](#cicd-integration) section).

### 6. MongoDB ObjectId Testing

The crate includes comprehensive ObjectId tests with real MongoDB library integration (dev-only dependency). Test various ObjectId scenarios:

```rust
#[cfg(all(feature = "object_id", test))]
#[test]
fn test_objectid_types() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct Test {
        pub id: ObjectId,
        pub tags: Vec<ObjectId>,
        pub metadata: HashMap<String, ObjectId>,
    }

    let ts = Test::ts_definition();
    assert!(ts.contains("ObjectId"));
}
```

### 7. Complex Structure Testing

Test deeply nested structures and edge cases to ensure macro robustness:

```rust
#[test]
fn test_complex_nested() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct Complex {
        pub nested: Vec<HashMap<String, Vec<Other>>>,
    }

    let ts = Complex::ts_definition();
    // Verify correct nesting
}
```

### 8. Production Safety

- MongoDB dependency is dev-only, ensuring zero production overhead
- Feature flags allow minimal dependency footprint
- All type analysis happens at compile time (no runtime cost)

## CI/CD Integration

The CI pipeline ([.github/workflows/ci.yml](.github/workflows/ci.yml)) runs:

1. `cargo build --verbose`
2. `just check` (cargo check + clippy)
3. `cargo test --verbose` (basic tests)
4. `just test` (all 32 feature combinations via cargo-hack)
5. Discord notification with build status

**Before pushing**, run `just ci` locally to replicate the CI pipeline.

## MongoDB ObjectId Support

The `object_id` feature provides comprehensive MongoDB ObjectId type support with proper validation and serialization.

### Basic Usage

```rust
use mongodb::bson::oid::ObjectId;

#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct Document {
    pub id: ObjectId,                           // Scalar ObjectId
    pub author_id: ObjectId,
    pub tags: Vec<ObjectId>,                    // Array of ObjectIds
    pub metadata: HashMap<String, ObjectId>,    // HashMap with ObjectId values
    pub parent_id: Option<ObjectId>,            // Optional ObjectId
    pub related: HashMap<String, Vec<ObjectId>>, // Nested structures
}
```

### Generated TypeScript

```typescript
export type Document = {
  id: ObjectId;
  author_id: ObjectId;
  tags: Array<ObjectId>;
  metadata: Partial<Record<string, ObjectId>>;
  parent_id: ObjectId | undefined;
  related: Partial<Record<string, Array<ObjectId>>>;
};
```

### Generated Zod Schema

```typescript
export const Document$Schema = z.strictObject({
  id: z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) }),
  author_id: z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) }),
  tags: z.array(z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) })),
  metadata: z.record(z.string(), z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) })),
  parent_id: z.union([z.null().transform(() => undefined), z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) }), z.undefined()]).prefault(undefined),
  related: z.record(z.string(), z.array(z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) }))),
});
```

### JSON Serialization Format

ObjectIds serialize to MongoDB's standard JSON format:

```json
{
  "id": { "$oid": "507f1f77bcf86cd799439011" },
  "author_id": { "$oid": "507f1f77bcf86cd799439012" },
  "tags": [
    { "$oid": "507f1f77bcf86cd799439013" },
    { "$oid": "507f1f77bcf86cd799439014" }
  ],
  "metadata": {
    "template": { "$oid": "507f1f77bcf86cd799439015" }
  },
  "parent_id": { "$oid": "507f1f77bcf86cd799439016" },
  "related": {
    "references": [
      { "$oid": "507f1f77bcf86cd799439017" },
      { "$oid": "507f1f77bcf86cd799439018" }
    ]
  }
}
```

### Key Features

- **Validation**: Regex validation ensures 24-character hexadecimal strings
- **Type Safety**: Full TypeScript type checking for ObjectId fields
- **MongoDB Compatibility**: Matches MongoDB's native JSON format (`{ "$oid": "..." }`)
- **Zero Production Overhead**: MongoDB crate is a dev-dependency only
- **Comprehensive Testing**: Tested with real MongoDB library integration

## File Organization

```
tixschema/
├── src/
│   ├── lib.rs                    # Entry point, macro exports
│   ├── model_schema.rs           # Main macro logic
│   ├── field_type.rs             # Type system (FieldDef, FieldDefType)
│   ├── utils.rs                  # Helpers (naming, docs, serde parsing)
│   ├── features/
│   │   ├── mod.rs                # Feature detection
│   │   ├── serde.rs              # Serde attribute parsing
│   │   ├── zod.rs                # Zod schema generation
│   │   ├── jsonschema.rs         # JSON schema generation
│   │   ├── object_id.rs          # ObjectId support
│   │   └── model_schema_prop.rs  # Field attribute parsing
│   └── generation/
│       ├── mod.rs
│       └── typescript.rs         # TypeScript/Zod code generation
├── tests/
│   ├── basic_tests.rs
│   ├── collection_tests.rs
│   ├── enum_tests.rs
│   ├── serde_tests.rs
│   ├── mongodb_tests.rs
│   ├── mongodb_real_tests.rs
│   ├── edge_cases_tests.rs
│   ├── semantic_types_tests.rs
│   └── ...
├── justfile                      # Task runner (commands)
├── Cargo.toml                    # Dependencies, features
└── README.md                     # User documentation
```

## Common Pitfalls

1. **Type name ambiguity** → Ensure Rust type names and TypeScript output names don't collide with built-in types
2. **Missing required derives** → Compilation errors
3. **Non-string HashMap keys** → Compilation errors
4. **Forgetting to add types to entities enum** → Types not included in generated output
5. **Using Zod v3** → Generated schemas use v4 syntax and won't work
6. **Testing without feature combinations** → May break in different feature configurations

## Debugging Tips

```rust
// Print generated TypeScript during tests
let ts = MyType::ts_definition();
println!("Generated:\n{}", ts);

// Print JSON schema
#[cfg(feature = "jsonschema")]
println!("{}", serde_json::to_string_pretty(&MyType::json_schema())?);

// Check which features are enabled
#[cfg(test)]
use crate::features::Features;
println!("Enabled: {:?}", Features::enabled_features());
```

## Performance Considerations

- Macro expansion happens at compile time (zero runtime overhead)
- Generated methods return `String` (consider caching if called frequently)
- Feature flags reduce dependencies and compilation time when not needed
- MongoDB ObjectId validation regex is compiled once by Zod

## Related Documentation

- [README.md](README.md) - User guide, examples, troubleshooting
- [justfile](justfile) - All available commands with descriptions
