# Contributing to TixSchema

## Getting Started

Clone the repository and ensure you have Rust installed.

## Task Management

This project uses [bd (beads)](https://github.com/steveyegge/beads) for issue tracking. See [AGENTS.md](AGENTS.md) for the complete workflow.

## Development Commands

### Testing

```bash
# Quick test with default features
just quick
# or
cargo test

# Comprehensive test - all feature combinations (run before commits)
just test

# Test specific feature combinations
just test-minimal
just test-default
just test-named-features

# Run a specific test
cargo test <TEST_NAME>
```

### Code Quality

```bash
# Check code (cargo check + clippy with warnings as errors)
just check

# Format code
just fmt

# Check all feature combinations
just check-all
```

### Type-Checking the Emitted TypeScript

The `service_schema` tests that compile an emitted bundle need a TypeScript compiler, which a
fresh clone does not have. They look for `tsc` on `PATH` (or wherever `TIXSCHEMA_TSC` names) and
stand down when they find none, saying so on stderr — so `just quick`, `just test` and `just all`
never require one, and a run that did not type-check anything says so rather than passing quietly.

```bash
# Refuses to stand down: fails if no compiler is reachable.
just typecheck-ts
```

### Measuring What a Consumer's Lint Levels Say About the Transport Macros

`#[service_schema(transports = [...])]` hands both halves of a service to the consumer as
`macro_rules!` bodies. What a `macro_rules!` body expands to is linted under the levels of the
crate that **invokes** it, so a consumer denying clippy's `restriction` and `nursery` sets is
denying them over code this crate wrote. Two things decide what they see, and neither can be
staged inside a `tests/` binary here:

- **whether the crate that declares the service is the crate that invokes the macro.** Same crate,
  the body is linted almost as if it were hand-written; different crates, rustc and most of clippy
  treat it as an external macro. `clippy::exhaustive_structs` and `clippy::exhaustive_enums` are
  the only two lints in the denied set that reach a cross-crate consumer.
- **whether the consumer publishes the module they placed it in.** `dead_code` reaches a private
  placement that drives nothing; `exhaustive_structs` and `missing_errors_doc` reach a public one.

Every `tests/` binary in this repository is the same-crate case, so the cross-crate half is
measured by hand rather than by `cargo test`. It is not wired into `just ci`: it needs a scratch
workspace outside the repository, and shelling out to `cargo clippy` from a test would run a
nested build in each of the feature combinations `just test` already walks.

Build a scratch workspace outside the repository with a `declaring` library that carries four
services — one with both outcomes, one with only one-way operations, one with only
request-and-reply operations, and one with no operation at all — and four consumers of it, each
carrying this repository's own deny set (`clippy::all`, `pedantic`, `restriction` and `nursery`
at deny, minus the allow list in `Cargo.toml`), each placing both halves of all four services with
one invocation per module file:

1. a cross-crate **binary** driving every emitted item from `main`;
2. a cross-crate **library** publishing every module;
3. a cross-crate **library** keeping every module private and exercising nothing;
4. a **same-crate** library that declares the services and publishes every module, reaching each
   macro by its bare name through `#[macro_use] mod contract;`.

Run `cargo clippy -p <consumer> -- -D warnings` over each. What the placements earn, measured on
rustc/clippy 1.98.0-beta.7:

| Consumer | Expected |
|---|---|
| cross-crate binary | no diagnostics |
| cross-crate library, modules published | no diagnostics |
| cross-crate library, modules private | no diagnostics |
| same-crate library, modules published | one `clippy::pub_use` on the consumer's own hoisting line, and `exhaustive_structs`/`exhaustive_enums` on the *proc macro's* own message and support types — never `dead_code`, never `missing_errors_doc`, and nothing originating in either `macro_rules!` body |

The last row's two remaining kinds are the proc macro's own emissions rather than a
`macro_rules!` body's, and clearing them means deciding whether a generated wire message may be
`#[non_exhaustive]` — a public-API question tracked separately.

An `#[allow]`, `#[expect]` or `#[doc(hidden)]` emitted into a consumer's expansion is **not** an
acceptable way to bring any of these to zero: it silences a check the consumer chose, in the
consumer's build, with no line of their own source to explain it. `just quick` asserts that
neither macro body carries one.

### Full CI Pipeline

```bash
just ci
# Runs: clean, check-all, test, fmt
```

## Test Organization

Tests are organized by category in `tests/`:

- `basic_tests.rs` — simple structs, basic types
- `primitive_types_tests.rs` — all numeric types, bool, String
- `collection_tests.rs` — Vec, HashMap, nested collections
- `enum_tests.rs` — plain enums, discriminated unions
- `tuple_variant_tests.rs` — single/multi-element tuples, mixed variants
- `recursive_type_tests.rs` — self-referential structs and enums
- `semantic_types_tests.rs` — type aliases
- `serde_tests.rs` — Serde attribute handling
- `model_schema_prop_tests.rs` — field-level customization
- `pattern_preprocess_tests.rs` — pattern/preprocess validation
- `validation_tests.rs` — validate() method generation
- `branded_newtype_tests.rs` — transparent tuple structs
- `example_tests.rs` — compiler-validated examples
- `chrono_tests.rs` — date/time types
- `mongodb_tests.rs`, `mongodb_real_tests.rs` — ObjectId support
- `jsonschema_tests.rs` — JSON Schema generation
- `zod_tests.rs` — Zod v4 schema generation
- `typescript_feature_tests.rs` — feature-gated TypeScript
- `advanced_tests.rs`, `edge_cases_tests.rs` — complex scenarios

### Test Pattern

```rust
#[test]
fn test_my_feature() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct MyType { ... }

    let ts = MyType::ts_definition();
    assert!(ts.contains("expected output"));
}
```

## Best Practices

- Run `just ci` before pushing to replicate the CI pipeline
- Test TypeScript generation in your test suite
- Consider committing generated TypeScript files for easier code review
- Pin Zod v4 in frontend dependencies
- MongoDB dependency is dev-only — zero production overhead

## Architecture

See [CLAUDE.md](CLAUDE.md) for detailed architecture documentation.
