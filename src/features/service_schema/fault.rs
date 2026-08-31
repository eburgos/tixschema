//! The seal on the published fault: the two declarations that turn a structural object type into
//! one only the generated code can write, and the form every generated constructor mints through.
//!
//! # What the seal is for
//!
//! A fault reports a failure the operation never declared, so an implementation that could mint one
//! could report a defect it did not have, and a caller reading a fault could not tell that one from
//! a real one. Rust refuses both routes — building the struct is `E0451`, the fields being private,
//! and calling a constructor is `E0624` — because Rust has a read-without-construct split and the
//! generated module is the scope it is drawn in.
//!
//! TypeScript has no such scope, and a plain structural object type is writable by anyone who can
//! name it. So the fault's own fields publish under a name of their own — the Rust struct is
//! declared as `<Service>FaultFields`, and a type publishes under the ident it was declared with —
//! and the name a caller reads is declared here as those fields intersected with one property keyed
//! on a `unique symbol` the bundle declares and exports nowhere. A module that cannot name the
//! symbol cannot write the property, and a value without the property is not a fault.
//!
//! # What it costs a caller, which is nothing
//!
//! The brand is a type-level property with no runtime value behind it — the symbol is
//! `declare const`, so nothing is emitted for it and no fault carries an extra key on the wire.
//! Every read is untouched: a caller still narrows on `isServiceFault`, still reads `fault.kind`,
//! `fault.detail`, `fault.field` and `fault.operation`, and still switches exhaustively over the
//! kind. Only writing one stops compiling.
//!
//! # How far it goes
//!
//! It stops the two routes a service implementation would take: an object literal under a
//! `<Service>Fault` annotation, and a structurally-equal value assigned into a `<Service>Fault`
//! position. Neither carries the branded property, and neither can be given one.
//!
//! It does not stop a deliberate type assertion. `built as UsageServiceFault` compiles, as does
//! anything laundered through `any` or `unknown`, and no TypeScript construct refuses those — the
//! language has no `E0451`. The generated constructors mint through exactly that assertion, in
//! [`minted`], which is the one direction an assertion is unambiguously sound in: the sealed type
//! is assignable to the fields type it is asserted from. What the seal buys is that fabricating a
//! fault is a deliberate, greppable act rather than something a plain annotated literal does
//! silently.

use crate::rename_rule::RenameRule;
use crate::service_schema::parse::ServiceDef;
use crate::service_schema::support::fault_fields_typescript_name;

pub fn emit(service: &ServiceDef) -> Vec<String> {
    let named = service.ident.to_string();
    vec![seal(&named), sealed_fault(&named)]
}

/// The body of a generated fault constructor: build the fields the Rust declaration published, then
/// seal them.
///
/// All three constructors — the dispatcher's two and the client's one — end this way, so the
/// assertion the seal costs is written once, here, rather than at each site. `members` is the
/// object's own lines, indented as they appear.
#[cfg(feature = "zod")]
pub fn minted(service: &str, members: &str) -> String {
    let fields = fault_fields_typescript_name(service);
    format!(
        "  const built: {fields} = {{\n\
         {members}\n  \
         }};\n  \
         return built as {service}Fault;"
    )
}

/// The symbol the brand is keyed on. `declare const` rather than a value, so nothing is emitted for
/// it and no fault gains a key on the wire; `unique symbol` so the property it keys is one no other
/// declaration can spell.
fn seal(service: &str) -> String {
    let sealed = seal_name(service);
    format!(
        "/**\n \
         * The brand on `{service}Fault`, declared here and exported from nowhere.\n \
         *\n \
         * A module that cannot name this symbol cannot write the property it keys, and a value \
         without\n \
         * that property is not a `{service}Fault`. It is `declare const` rather than a value: \
         nothing\n \
         * is emitted for it, and a fault carries no extra key on the wire.\n \
         */\n\
         declare const {sealed}: unique symbol;"
    )
}

/// What a caller names, and what only the generated client and dispatcher can build: the fields the
/// Rust declaration published, plus the brand.
fn sealed_fault(service: &str) -> String {
    let fields = fault_fields_typescript_name(service);
    let sealed = seal_name(service);
    format!(
        "/**\n \
         * A failure `{service}` never declared, as a caller reads it.\n \
         *\n \
         * The fields come from the same Rust declaration the dispatcher and the client build \
         faults\n \
         * from. The brand is what an implementation cannot write: it is keyed on a symbol this\n \
         * bundle declares and exports nowhere, so an object literal under this annotation does \
         not\n \
         * compile, and neither does a structurally-equal value assigned into this position.\n \
         *\n \
         * Reading one is unaffected. Narrow on `isServiceFault`, then read `kind`, `detail`, \
         `field`\n \
         * and `operation`, or switch over `kind` exhaustively — the brand has no runtime value \
         and\n \
         * never reaches the wire.\n \
         */\n\
         export type {service}Fault = {fields} & {{\n  \
         readonly [{sealed}]: true;\n\
         }};"
    )
}

/// The symbol's own name, carrying the service for the reason every published name does: a bundle
/// is one flat file, and ten services would otherwise declare one symbol ten times over.
fn seal_name(service: &str) -> String {
    format!(
        "{}FaultSeal",
        RenameRule::CamelCase.apply_to_variant(service)
    )
}
