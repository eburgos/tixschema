//! A service whose operations cover every input shape and every outcome, read off the TypeScript
//! it publishes and off a bundle written to a file the way a consuming codebase writes one.

#![cfg(feature = "serde")]

#[cfg(feature = "typescript")]
mod the_bundle_one_registration_line_produces {
    use super::{
        ApplyBundleReceipt, AuditServiceSchema, BalanceRequest, BalanceResponse, CreditWriteError,
        ProbeError, ProbeServiceSchema,
    };
    use std::env::temp_dir;
    use std::fs;
    use std::path::PathBuf;

    /// Every name the seam publishes. The pair below reads for all five: one half asserts a build
    /// with the Zod surface writes each of them, the other that a build without it writes none.
    const SEAM_DECLARATIONS: [&str; 5] = [
        "export type ProbeServiceTransport = {",
        "export type ProbeServiceClient = {",
        "export function createProbeServiceClient(",
        "export interface ProbeServiceImpl<Ctx> {",
        "export function createProbeServiceDispatcher<Ctx>(",
    ];

    /// The bundle a consuming codebase writes: its own types named by hand, one line each, and the
    /// service named once. Nothing here names a message the macro declared — that is the point.
    fn bundle() -> String {
        let mut written = vec![
            BalanceRequest::ts_definition(),
            BalanceResponse::ts_definition(),
            ApplyBundleReceipt::ts_definition(),
            ProbeError::ts_definition(),
            CreditWriteError::ts_definition(),
        ];
        written.extend(author_schemas());
        written.push(ProbeServiceSchema::ts_definition());
        written.extend(probe_seam());
        written.join("\n\n")
    }

    /// The client and the dispatcher a bundle carries, which only a build with the Zod surface
    /// publishes: both parse a message against the schema `#[model_schema()]` writes for it, and a
    /// build that writes none publishes neither rather than a pair that checks nothing.
    #[cfg(feature = "zod")]
    fn probe_seam() -> Vec<String> {
        vec![
            ProbeServiceSchema::ts_client(),
            ProbeServiceSchema::ts_service(),
        ]
    }

    #[cfg(not(feature = "zod"))]
    const fn probe_seam() -> Vec<String> {
        Vec::new()
    }

    /// The second service's half of the same pair, so the collision test below reads two full
    /// services in whichever build it runs in.
    #[cfg(feature = "zod")]
    fn audit_seam() -> Vec<String> {
        vec![
            AuditServiceSchema::ts_client(),
            AuditServiceSchema::ts_service(),
        ]
    }

    #[cfg(not(feature = "zod"))]
    const fn audit_seam() -> Vec<String> {
        Vec::new()
    }

    /// The schema line a hand-written type publishes beside its type. The service's own line
    /// carries the schemas of the messages the macro declared and nobody else's, so a type the
    /// author named is the author's to publish twice — once as a type, once as a schema — and a
    /// bundle that names only its types leaves the client and the dispatcher parsing with a value
    /// nothing declares.
    #[cfg(feature = "zod")]
    fn author_schemas() -> Vec<String> {
        vec![
            BalanceRequest::zod_schema(),
            BalanceResponse::zod_schema(),
            ApplyBundleReceipt::zod_schema(),
            ProbeError::zod_schema(),
            CreditWriteError::zod_schema(),
        ]
    }

    #[cfg(not(feature = "zod"))]
    const fn author_schemas() -> Vec<String> {
        Vec::new()
    }

    /// Every name the published result envelopes refer to, read off the two arms themselves rather
    /// than from a list written here, so a type the envelope starts naming is checked without this
    /// test being edited.
    fn referenced_types(written: &str) -> Vec<String> {
        let mut reached = Vec::new();
        for line in written.lines().map(str::trim) {
            if let Some(rest) = line.strip_prefix("| { ok: true; value: ") {
                reached.push(rest.trim_end_matches(" }").to_owned());
            }
            if let Some(rest) = line.strip_prefix("| { ok: false; error: ") {
                if let Some((declared, _)) = rest.split_once(" | {") {
                    reached.push(declared.to_owned());
                }
                reached.push("ProbeServiceFault".to_owned());
            }
        }
        reached
    }

    fn written_bundle(named: &str) -> (PathBuf, String) {
        let path = temp_dir().join(named);
        fs::write(&path, bundle()).unwrap();
        let read_back = fs::read_to_string(&path).unwrap();
        fs::remove_file(&path).unwrap();
        (path, read_back)
    }

    #[test]
    fn a_bundle_written_to_a_file_declares_every_type_it_refers_to() {
        let (path, written) = written_bundle("tixschema_service_bundle_complete.ts");
        assert!(!written.is_empty(), "wrote nothing to {}", path.display());
        let reached = referenced_types(&written);
        assert!(reached.len() >= 8, "got: {reached:?}");
        for named in reached {
            assert!(
                written.contains(&format!("export type {named} =")),
                "a bundle carrying one line per author type and one line for the service leaves \
                 `{named}` undeclared. Got: {written}"
            );
        }
    }

    /// Every name a bundle's client and dispatcher parse a payload through, read off the parse
    /// sites themselves. A schema is a value rather than a type, so none of the checks that read
    /// declared type names reach it.
    #[cfg(feature = "zod")]
    fn parsed_through(written: &str) -> Vec<&str> {
        let mut parsed: Vec<&str> = written
            .match_indices("$Schema.safeParse(")
            .filter_map(|(at, _)| {
                written[..at]
                    .rsplit(|character: char| !character.is_alphanumeric())
                    .next()
            })
            .collect();
        parsed.sort_unstable();
        parsed.dedup();
        parsed
    }

    /// Every schema the bundle's client and dispatcher parse with is declared by the same bundle.
    /// A schema is a value rather than a type, so none of the checks that read declared type names
    /// reach it: a bundle naming a `$Schema` nothing declares reads exactly like one that does,
    /// and is a file that will not compile.
    #[cfg(feature = "zod")]
    #[test]
    fn every_schema_the_bundle_parses_with_is_declared_by_the_bundle() {
        let (_, written) = written_bundle("tixschema_service_bundle_schemas.ts");
        let parsed = parsed_through(&written);
        assert!(parsed.len() >= 4, "got: {parsed:?}");
        for named in parsed {
            assert!(
                written.contains(&format!("export const {named}$Schema")),
                "the bundle parses with `{named}$Schema` and declares no such value. \
                 Got: {written}"
            );
        }
    }

    /// What a bundle writer gets for leaving an author type's `zod_schema()` line out: the bundle
    /// above with `author_schemas()` dropped and nothing else changed.
    ///
    /// The service's own line carries the schemas of the messages the macro declared and nobody
    /// else's — it does not own a type the author named and cannot publish its schema line — so a
    /// bundle naming only its types parses through a value it never declares. Nothing on the Rust
    /// side refuses that bundle; it is a file the consuming codebase fails to compile.
    #[cfg(feature = "zod")]
    #[test]
    fn a_bundle_missing_an_author_type_s_schema_line_parses_through_a_value_it_never_declares() {
        let types_only = [
            BalanceRequest::ts_definition(),
            BalanceResponse::ts_definition(),
            ApplyBundleReceipt::ts_definition(),
            ProbeError::ts_definition(),
            CreditWriteError::ts_definition(),
            ProbeServiceSchema::ts_definition(),
            ProbeServiceSchema::ts_client(),
            ProbeServiceSchema::ts_service(),
        ]
        .join("\n\n");
        let undeclared: Vec<&str> = parsed_through(&types_only)
            .into_iter()
            .filter(|named| !types_only.contains(&format!("export const {named}$Schema")))
            .collect();
        assert!(
            undeclared.contains(&"BalanceRequest"),
            "the author named `BalanceRequest` on an operation, so the client and the dispatcher \
             parse through `BalanceRequest$Schema`, and the only line that could declare it is one \
             the bundle writer omitted. Got: {undeclared:?}"
        );
        assert!(
            types_only.contains("export type BalanceRequest ="),
            "the type is declared and the schema is not, which is exactly why nothing that reads \
             declared type names catches this. Got: {types_only}"
        );
        assert!(
            types_only.contains("export const ApplyBundleRequest$Schema"),
            "a message the macro declared is unaffected: the service's own line carries its \
             schema, because the service owns the type. Got: {types_only}"
        );
    }

    #[test]
    fn a_message_the_macro_declared_reaches_the_bundle_without_a_line_of_its_own() {
        let (_, written) = written_bundle("tixschema_service_bundle_declared_messages.ts");
        for declared in ["ExpireCreditRequest", "SweepRequest", "ApplyBundleRequest"] {
            assert!(
                written.contains(&format!("export type {declared} =")),
                "nobody wrote `{declared}`, so nobody could have written its registration. \
                 Got: {written}"
            );
        }
    }

    #[test]
    fn the_envelope_adds_no_field_to_the_message_it_carries() {
        let (_, written) = written_bundle("tixschema_service_bundle_untouched_messages.ts");
        let found = written
            .split("export type BalanceResponse =")
            .nth(1)
            .and_then(|rest| rest.split_once("};"))
            .map(|(body, _)| body.to_owned());
        assert!(found.is_some(), "got: {written}");
        let declared = found.unwrap();
        for injected in ["ok:", "value:", "isServiceFault", "fault:", "error:"] {
            assert!(
                !declared.contains(injected),
                "the envelope is added around the message, never into it. Got: {declared}"
            );
        }
        assert!(declared.contains("credits: number;"), "got: {declared}");
    }

    #[test]
    fn the_fault_is_declared_once_per_service_and_reachable_from_every_failure_arm() {
        let (_, written) = written_bundle("tixschema_service_bundle_fault.ts");
        assert_eq!(
            written.matches("export type ProbeServiceFault =").count(),
            1,
            "got: {written}"
        );
        assert!(
            !written.contains("export type ServiceFault"),
            "the unprefixed name is what ten services in one flat file collide on. Got: {written}"
        );
        assert_eq!(
            written
                .matches("| { isServiceFault: true; fault: ProbeServiceFault } };")
                .count(),
            4,
            "every operation that answers can answer with a fault. Got: {written}"
        );
    }

    /// The seal on the published fault, read off the bundle a consuming codebase writes.
    ///
    /// **What this proves and what it cannot.** No TypeScript toolchain is reachable from this
    /// repository, so nothing here compiles the bundle. What it reads is the structure the refusal
    /// rests on: `ProbeServiceFault` is not an object type but an intersection, one half of which is
    /// a required property keyed on a symbol the bundle declares and exports nowhere. An object
    /// literal cannot carry that property, because a module outside the bundle cannot name the
    /// symbol to write it and a module inside has no value to write. Whether `tsc` then rejects a
    /// given fabrication is a claim only `tsc` can settle.
    #[test]
    fn the_published_fault_is_the_fields_under_a_brand_the_bundle_exports_nowhere() {
        let (_, written) = written_bundle("tixschema_service_bundle_seal.ts");
        assert!(
            written.contains("declare const probeServiceFaultSeal: unique symbol;"),
            "the brand is keyed on a symbol, and a `unique symbol` is one no other declaration \
             spells. Got: {written}"
        );
        assert!(
            !written.contains("export declare const probeServiceFaultSeal"),
            "an exported symbol is one an implementation can name, and a property it can write. \
             Got: {written}"
        );
        assert!(
            written.contains(
                "export type ProbeServiceFault = ProbeServiceFaultFields & {\n  readonly \
                 [probeServiceFaultSeal]: true;\n};"
            ),
            "the fault a caller names is the fields the Rust declaration published, plus the \
             brand. Got: {written}"
        );
        assert!(
            written.contains("export type ProbeServiceFaultFields = {"),
            "the members still come from the Rust declaration and are still readable. \
             Got: {written}"
        );
    }

    /// Every fault the bundle builds is minted the one way: the fields, then the assertion into the
    /// sealed type. Read off the emitted text, so a constructor added later is compared without
    /// this test being edited.
    ///
    /// The assertion is what the seal costs. TypeScript cannot write a property keyed on a symbol
    /// with no runtime value, so the generated code asserts from the fields type — the one
    /// direction an assertion is unambiguously sound in, the sealed type being assignable to the
    /// type it is asserted from. Keeping every mint to this form is what makes fabricating a fault
    /// a greppable act rather than something an annotated literal does silently.
    #[cfg(feature = "zod")]
    #[test]
    fn every_fault_the_bundle_builds_is_minted_from_the_fields_and_sealed() {
        let (_, written) = written_bundle("tixschema_service_bundle_mint.ts");
        let answering = written.matches("): ProbeServiceFault {").count();
        assert_eq!(
            answering, 3,
            "the client refuses an outbound message, and the dispatcher answers an unrecognised \
             operation and a payload that failed. Got: {written}"
        );
        assert_eq!(
            written
                .matches("const built: ProbeServiceFaultFields = {")
                .count(),
            answering,
            "every constructor builds the fields the Rust declaration published. Got: {written}"
        );
        assert_eq!(
            written
                .matches("return built as ProbeServiceFault;")
                .count(),
            answering,
            "one assertion per constructor and nowhere else. Got: {written}"
        );
        assert_eq!(
            written.matches(" as ProbeServiceFault").count(),
            answering,
            "an assertion anywhere else in the bundle is a fault built outside the two places \
             entitled to build one. Got: {written}"
        );
    }

    /// The brand is a type and never a value, so nothing the wire carries changed: the symbol's
    /// name appears in no reply the dispatcher writes, and the keys a fault carries are the ones
    /// the *fields* type declares.
    #[test]
    fn the_brand_reaches_no_reply_the_dispatcher_writes() {
        let encoded = super::dispatched("nothing-answers-to-this", b"{}", "probe");
        let written = String::from_utf8_lossy(&encoded).into_owned();
        assert!(
            !written.contains("probeServiceFaultSeal") && !written.contains("Symbol("),
            "a brand with a runtime value would be a key on the wire the Rust side never writes. \
             Got: {written}"
        );
        let (_, bundle) = written_bundle("tixschema_service_bundle_brand_offwire.ts");
        assert!(
            bundle.contains("declare const probeServiceFaultSeal"),
            "`declare const` is what makes the symbol a type-level name with nothing emitted for \
             it. Got: {bundle}"
        );
    }

    /// The bundle a consuming codebase with more than one service writes. Every published name
    /// carries its service, so nothing here is declared twice — which is the whole reason for the
    /// prefix, TypeScript having no per-service scope to lean on.
    #[test]
    fn two_services_in_one_bundle_declare_nothing_twice() {
        let mut both = vec![
            BalanceRequest::ts_definition(),
            BalanceResponse::ts_definition(),
            ApplyBundleReceipt::ts_definition(),
            ProbeError::ts_definition(),
            CreditWriteError::ts_definition(),
            ProbeServiceSchema::ts_definition(),
        ];
        both.extend(probe_seam());
        both.push(AuditServiceSchema::ts_definition());
        both.extend(audit_seam());
        let two = both.join("\n\n");
        let mut declared: Vec<&str> = two
            .lines()
            .filter_map(|line| {
                line.strip_prefix("export type ")
                    .or_else(|| line.strip_prefix("export interface "))
                    .or_else(|| line.strip_prefix("export function "))
                    .or_else(|| line.strip_prefix("export const "))
                    .or_else(|| line.strip_prefix("declare const "))
                    .or_else(|| line.strip_prefix("function "))
                    .or_else(|| line.strip_prefix("const "))
            })
            .map(|rest| {
                rest.split_once(|written: char| !written.is_alphanumeric() && written != '$')
                    .map_or(rest, |(named, _)| named)
            })
            .collect();
        let written = declared.len();
        declared.sort_unstable();
        declared.dedup();
        assert_eq!(
            declared.len(),
            written,
            "a bundle is one flat file, so a name declared twice does not compile. Got: {declared:?}"
        );
        assert!(
            declared.contains(&"ProbeServiceFault") && declared.contains(&"AuditServiceFault"),
            "got: {declared:?}"
        );
        // The seal is a declaration in the flat file like any other, so two services carry two of
        // them and the dedup above is what says so. A shared symbol would let one service's
        // generated code mint the other's fault.
        assert!(
            declared.contains(&"probeServiceFaultSeal")
                && declared.contains(&"auditServiceFaultSeal"),
            "each service brands its own fault with its own symbol. Got: {declared:?}"
        );
        assert!(
            declared.contains(&"ProbeServiceGetBalanceResult")
                && declared.contains(&"AuditServiceGetBalanceResult"),
            "two services declaring one operation name publish two result types. Got: {declared:?}"
        );
        // A generated message publishes under the operation's own name, with no service prefix to
        // separate it, so two services' generated messages sharing one flat file is the case that
        // has to be seen rather than assumed. Both are here, and the dedup above covers them.
        for message in ["SweepRequest", "ApplyBundleRequest", "ReconcileRequest"] {
            assert_eq!(
                two.matches(&format!("export type {message} =")).count(),
                1,
                "a generated message is declared once in a bundle carrying two services. \
                 Got: {declared:?}"
            );
        }
    }

    #[test]
    fn the_result_keeps_ok_a_two_value_discriminant() {
        let written = ProbeServiceSchema::ts_definition();
        for arm in written
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("| { ok:"))
        {
            assert!(
                arm.starts_with("| { ok: true; value: ")
                    || arm.starts_with("| { ok: false; error: "),
                "a third arm would stop `ok` discriminating anything. Got: {arm}"
            );
        }
        assert_eq!(
            written.matches("| { ok: true; value: ").count(),
            written.matches("| { ok: false; error: ").count(),
            "got: {written}"
        );
    }

    #[cfg(feature = "zod")]
    #[test]
    fn the_client_and_the_implementable_service_reach_the_bundle_through_their_own_lines() {
        let (_, written) = written_bundle("tixschema_service_bundle_client_and_service.ts");
        for declared in SEAM_DECLARATIONS {
            assert!(written.contains(declared), "got: {written}");
        }
    }

    /// The other half: a bundle written by a build with no Zod surface carries none of the five.
    ///
    /// Both the client and the dispatcher parse a message against the schema `#[model_schema()]`
    /// writes for it, and this build writes none. What used to be published here was a client that
    /// forwarded whatever it was handed and a dispatcher that narrowed an unread payload with `as`
    /// — a bundle that read like the checked one and admitted anything, while the Rust half of the
    /// same service went on validating.
    #[cfg(not(feature = "zod"))]
    #[test]
    fn a_bundle_written_without_the_zod_surface_carries_no_client_and_no_dispatcher() {
        let (_, written) = written_bundle("tixschema_service_bundle_no_seam.ts");
        for withheld in SEAM_DECLARATIONS {
            assert!(
                !written.contains(withheld),
                "`{withheld}` cannot be published without the schema it parses against. \
                 Got: {written}"
            );
        }
        assert!(
            written.contains("export type ProbeServiceGetBalanceResult ="),
            "the types describe what the Rust half puts on the wire and are published either way. \
             Got: {written}"
        );
    }

    /// Every name the client and the implementable service refer to is declared by the same
    /// bundle: the message each member takes, the result types, the outcome types and the fault.
    /// Read off the text rather than from a list written here, so a name they start referring to
    /// is checked without this test being edited.
    #[cfg(feature = "zod")]
    #[test]
    fn the_client_and_the_service_name_only_types_the_bundle_declares() {
        let (_, written) = written_bundle("tixschema_service_bundle_reachable.ts");
        let client = ProbeServiceSchema::ts_client();
        let service = ProbeServiceSchema::ts_service();
        let mut reached: Vec<String> = Vec::new();
        for line in client.lines().chain(service.lines()).map(str::trim) {
            if let Some(rest) = line.strip_prefix("return transport.request<") {
                reached.push(rest.split_once('>').unwrap_or((rest, "")).0.to_owned());
            }
            // A member of the client type or of the interface, which is where a method names both
            // the message it takes and the type it answers with. The transport's own members take
            // an `unknown` payload and answer a type parameter, so neither reaches this.
            let Some((taken, answered)) = line.split_once("): Promise<") else {
                continue;
            };
            let Some((_, message)) = taken.split_once("req: ") else {
                continue;
            };
            reached.push(message.to_owned());
            let named = answered.trim_end_matches(';').trim_end_matches('>');
            if named != "void" {
                reached.push(named.to_owned());
            }
        }
        assert!(reached.len() >= 18, "got: {reached:?}");
        for named in reached {
            assert!(
                written.contains(&format!("export type {named} =")),
                "the client and the service refer to `{named}` and the bundle declares no such \
                 type. Got: {written}"
            );
        }
    }
}

#[cfg(all(feature = "typescript", feature = "zod"))]
mod the_schema_that_rides_with_the_type {
    use super::ProbeServiceSchema;

    #[test]
    fn a_declared_message_publishes_its_schema_through_the_same_line() {
        let written = ProbeServiceSchema::ts_definition();
        for declared in ["ExpireCreditRequest", "SweepRequest", "ApplyBundleRequest"] {
            assert!(
                written.contains(&format!("{declared}$Schema")),
                "a client on the far side validates what it sends. Got: {written}"
            );
        }
    }
}

/// The one test group that reads both halves of the seam against each other: what the Rust
/// dispatcher actually serializes, and what the TypeScript this same service publishes says a
/// caller will find there.
///
/// Nothing here is compared against prose. The envelope's keys are read off the bytes serde wrote,
/// the arms' members are read off the emitted text, and the two sets are compared.
#[cfg(feature = "typescript")]
mod the_envelope_typescript_declares_is_the_one_rust_writes {
    #[cfg(feature = "zod")]
    use super::{BalanceRequest, PreparedAnswer, poll_once, settlements};
    use super::{ProbeServiceSchema, dispatched, probe_service_schema};
    use core::mem::take;

    /// The arm of a published result type whose members start with the given discriminant, read
    /// off the emitted text.
    fn arm(published: &str, discriminant: &str) -> String {
        let declared = ProbeServiceSchema::ts_definition();
        let body = declared
            .split(&format!("export type {published} ="))
            .nth(1)
            .unwrap()
            .to_owned();
        let found = body
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with(&format!("| {{ ok: {discriminant};")))
            .map(ToOwned::to_owned);
        assert!(found.is_some(), "no `ok: {discriminant}` arm in: {body}");
        found.unwrap()
    }

    /// The keys one JSON object carries, sorted, so a set is compared rather than a spelling.
    fn keys(encoded: &[u8]) -> Vec<String> {
        let read: serde_json::Value = serde_json::from_slice(encoded).unwrap();
        let mut carried: Vec<String> = read.as_object().unwrap().keys().cloned().collect();
        carried.sort_unstable();
        carried
    }

    /// The literals a published string union declares, read off the emitted text.
    fn literals(published: &str) -> Vec<String> {
        let declared = ProbeServiceSchema::ts_definition();
        let body = declared
            .split(&format!("export type {published} ="))
            .nth(1)
            .unwrap();
        body.split(';')
            .next()
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter_map(|line| line.strip_prefix("| \""))
            .filter_map(|rest| rest.split_once('"').map(|(named, _)| named.to_owned()))
            .collect()
    }

    /// The members one inline object arm declares at its own level — `| { ok: true; value: X }`
    /// answers `ok` and `value`, and the fault's own members inside the failure arm are not the
    /// arm's. Sorted, so the comparison is against a set.
    fn members(arm: &str) -> Vec<String> {
        let mut declared = Vec::new();
        let mut depth = 0_usize;
        let mut part = String::new();
        for written in arm.trim_start_matches("| ").chars() {
            match written {
                '{' => {
                    depth += 1;
                    if depth > 1 {
                        part.push(written);
                    }
                }
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        declared.push(take(&mut part));
                    } else {
                        part.push(written);
                    }
                }
                ';' if depth == 1 => declared.push(take(&mut part)),
                _ => part.push(written),
            }
        }
        let mut named: Vec<String> = declared
            .iter()
            .filter_map(|carried| carried.split_once(':'))
            .map(|(key, _)| key.trim().to_owned())
            .filter(|key| !key.is_empty())
            .collect();
        named.sort_unstable();
        named
    }

    /// The members a published object type declares, read off the two-space indented lines the
    /// emitter writes them on.
    fn object_members(published: &str) -> Vec<String> {
        let declared = ProbeServiceSchema::ts_definition();
        let body = declared
            .split(&format!("export type {published} = {{"))
            .nth(1)
            .unwrap()
            .split_once("\n};")
            .unwrap()
            .0
            .to_owned();
        let mut carried: Vec<String> = body
            .lines()
            .filter(|line| line.starts_with("  ") && line.trim_end().ends_with(';'))
            .filter_map(|line| line.trim().split_once(':'))
            .map(|(named, _)| named.trim().to_owned())
            .collect();
        carried.sort_unstable();
        carried
    }

    #[test]
    fn a_declared_failure_is_written_and_declared_as_ok_false_with_an_error() {
        let encoded = dispatched("settle", br#"{"organization_id":"acme"}"#, "");
        assert_eq!(
            keys(&encoded),
            vec!["error".to_owned(), "ok".to_owned()],
            "the value is omitted rather than written as null. Got: {}",
            String::from_utf8_lossy(&encoded)
        );
        let read: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(read["ok"], serde_json::json!(false));
        assert_eq!(
            members(&arm("ProbeServiceSettleResult", "false")),
            keys(&encoded),
            "what the dispatcher writes and what a caller narrows on are one envelope"
        );
    }

    #[test]
    fn a_success_is_written_and_declared_as_ok_true_with_a_value() {
        let encoded = dispatched("get-balance", br#"{"organization_id":"acme"}"#, "probe");
        assert_eq!(
            keys(&encoded),
            vec!["ok".to_owned(), "value".to_owned()],
            "the error is omitted rather than written as null. Got: {}",
            String::from_utf8_lossy(&encoded)
        );
        let read: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(read["ok"], serde_json::json!(true));
        assert_eq!(
            members(&arm("ProbeServiceGetBalanceResult", "true")),
            keys(&encoded),
            "what the dispatcher writes and what a caller narrows on are one envelope"
        );
    }

    #[test]
    fn a_fault_carries_exactly_the_keys_its_typescript_declares() {
        let encoded = dispatched("nothing-answers-to-this", b"{}", "probe");
        let carried = keys(&encoded);
        let declared = object_members("ProbeServiceFaultFields");
        for named in &carried {
            assert!(
                declared.contains(named),
                "the wire carries `{named}` and the TypeScript declares {declared:?}"
            );
        }
        for named in &declared {
            assert!(
                carried.contains(named) || named == "field",
                "`{named}` is declared and never written; only `field` may be absent. \
                 Got: {carried:?}"
            );
        }
        assert!(
            !carried.contains(&"field".to_owned()),
            "an absent field is omitted, which is what lets the TypeScript spell it \
             `string | undefined`. Got: {}",
            String::from_utf8_lossy(&encoded)
        );
    }

    #[test]
    fn the_kind_a_fault_carries_is_one_the_published_union_admits() {
        let encoded = dispatched("nothing-answers-to-this", b"{}", "probe");
        let read: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        let carried = read["kind"].as_str().unwrap().to_owned();
        let admitted = literals("ProbeServiceFaultKind");
        assert!(
            admitted.contains(&carried),
            "got: {carried} of {admitted:?}"
        );
        assert_eq!(carried, "unknown-operation");
    }

    /// Every kind the Rust enum can be, as serde writes it. Read off the values rather than from
    /// spellings written here, so a variant renamed on either side lands in the comparison below.
    fn serialized_kinds() -> Vec<String> {
        let mut written: Vec<String> = [
            probe_service_schema::ProbeServiceFaultKind::FailedValidation,
            probe_service_schema::ProbeServiceFaultKind::HandlerPanic,
            probe_service_schema::ProbeServiceFaultKind::TransportFailure,
            probe_service_schema::ProbeServiceFaultKind::UndeserializablePayload,
            probe_service_schema::ProbeServiceFaultKind::UnknownOperation,
        ]
        .iter()
        .map(|kind| {
            serde_json::to_value(kind)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
        written.sort_unstable();
        written
    }

    /// The published union and the wire, compared as sets rather than one sampled value.
    ///
    /// The kind a dispatch happens to produce is one of four, and a test that reads only that one
    /// would pass while the other three drifted. Both sides here are derived: the literals come off
    /// the emitted text, the values off serde.
    #[test]
    fn the_kinds_the_published_union_declares_are_exactly_the_ones_serde_writes() {
        let mut admitted = literals("ProbeServiceFaultKind");
        admitted.sort_unstable();
        assert_eq!(
            admitted,
            serialized_kinds(),
            "the fault's TypeScript comes from the Rust declaration, so a kind on one side and \
             not the other means the two stopped being one type"
        );
    }

    /// The kinds a dispatcher can actually reach, read off bytes it wrote rather than off the enum.
    ///
    /// `handler-panic` is not among them: nothing builds one today, which is tracked separately.
    #[test]
    fn each_kind_a_dispatch_can_produce_is_one_the_published_union_admits() {
        let admitted = literals("ProbeServiceFaultKind");
        for (operation, payload, expected) in [
            (
                "nothing-answers-to-this",
                b"{}".as_slice(),
                "unknown-operation",
            ),
            (
                "get-balance",
                br#"{"organization_id":42}"#.as_slice(),
                "undeserializable-payload",
            ),
        ] {
            let encoded = dispatched(operation, payload, "probe");
            let read: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
            let carried = read["kind"].as_str().unwrap().to_owned();
            assert_eq!(
                carried,
                expected,
                "got: {}",
                String::from_utf8_lossy(&encoded)
            );
            assert!(
                admitted.contains(&carried),
                "the wire carries `{carried}` and the published union admits {admitted:?}"
            );
        }
    }

    /// The one operation shape with no envelope at all, read on both sides.
    ///
    /// A one-way arm answers nothing on the Rust side, and the TypeScript for the same operation
    /// says so twice — the method answers `Promise<void>` and the dispatcher arm returns
    /// `undefined`. If either side started carrying a value the other would be wrong about the
    /// wire, and no envelope comparison would catch it, there being no envelope.
    #[cfg(feature = "zod")]
    #[test]
    fn a_one_way_operation_puts_nothing_on_the_wire_and_says_so_in_both_languages() {
        let settled = settlements(
            "apply-bundle",
            br#"{"organizationId":"acme","bundleId":"b-1"}"#,
        );
        assert!(
            settled.is_empty(),
            "a one-way arm publishes nothing, so there is no envelope for a caller to read. \
             Got: {settled:?}"
        );
        let client = ProbeServiceSchema::ts_client();
        assert!(
            client.contains("applyBundle(req: ApplyBundleRequest): Promise<void>;"),
            "got: {client}"
        );
        let service = ProbeServiceSchema::ts_service();
        assert!(
            service.contains("applyBundle(ctx: Ctx, req: ApplyBundleRequest): Promise<void>;"),
            "got: {service}"
        );
        let arm = service
            .split("      case \"apply-bundle\": {")
            .nth(1)
            .and_then(|rest| rest.split_once("\n      }"))
            .map(|(body, _)| body.to_owned());
        assert!(arm.is_some(), "got: {service}");
        let body = arm.unwrap();
        assert!(
            body.contains("return undefined;") && !body.contains("return { ok:"),
            "the arm answers with nothing, which is what `Promise<void>` promises. Got: {body}"
        );
    }

    /// Every operation name the emitted TypeScript dispatcher switches on, read off its own text.
    #[cfg(feature = "zod")]
    fn typescript_operation_names() -> Vec<String> {
        let written = ProbeServiceSchema::ts_service();
        let mut named: Vec<String> = written
            .lines()
            .map(str::trim)
            .filter_map(|line| line.strip_prefix("case \""))
            .filter_map(|rest| rest.split_once('"').map(|(name, _)| name.to_owned()))
            .collect();
        named.sort_unstable();
        named
    }

    /// A name only one of the two dispatchers answers to is a call that cannot cross.
    ///
    /// The names come off the emitted TypeScript; the verdict comes off the Rust dispatcher driven
    /// over each one. Neither side is a list written here, so an operation renamed on one side and
    /// not the other lands as a fault this reads.
    #[cfg(feature = "zod")]
    #[test]
    fn every_operation_the_typescript_dispatcher_answers_to_is_one_the_rust_one_answers_to() {
        let named = typescript_operation_names();
        assert_eq!(named.len(), 5, "got: {named:?}");
        for operation in &named {
            let settled = settlements(operation, b"{}");
            let unknown = settled.iter().any(|encoded| {
                serde_json::from_slice::<serde_json::Value>(encoded)
                    .ok()
                    .and_then(|read| read["kind"].as_str().map(ToOwned::to_owned))
                    .is_some_and(|kind| kind == "unknown-operation")
            });
            assert!(
                !unknown,
                "the TypeScript dispatcher routes `{operation}` and the Rust one answers to no \
                 such operation"
            );
        }
        // And the reverse: a name neither side declares is refused, so the check above is reading
        // a real verdict rather than one the dispatcher gives everything.
        let strange = settlements("reconcile", b"{}");
        let refused = strange.iter().any(|encoded| {
            serde_json::from_slice::<serde_json::Value>(encoded)
                .ok()
                .and_then(|read| read["kind"].as_str().map(ToOwned::to_owned))
                .is_some_and(|kind| kind == "unknown-operation")
        });
        assert!(
            refused && !named.iter().any(|operation| operation == "reconcile"),
            "got: {strange:?}"
        );
    }

    /// The other half of the framing: a fault written the way the emitted TypeScript dispatcher
    /// writes one, read back by the generated Rust client. If the tag key or the member the fault
    /// rides in disagreed, this would not narrow.
    #[cfg(feature = "zod")]
    #[test]
    fn a_fault_framed_the_way_typescript_frames_one_is_read_back_by_the_rust_client() {
        let fault = dispatched("nothing-answers-to-this", b"{}", "probe");
        let written = ProbeServiceSchema::ts_service();
        assert!(
            written.contains("return { ok: false, error: { isServiceFault: true, fault } };"),
            "the framing this test writes by hand is the one the emitter writes. Got: {written}"
        );
        let framed = serde_json::to_vec(&serde_json::json!({
            "ok": false,
            "error": {
                "isServiceFault": true,
                "fault": serde_json::from_slice::<serde_json::Value>(&fault).unwrap(),
            },
        }))
        .unwrap();
        let client =
            probe_service_schema::ProbeServiceClient::new(PreparedAnswer { encoded: framed });
        let answered = poll_once(client.get_balance(BalanceRequest {
            organization_id: "acme".to_owned(),
        }))
        .unwrap();
        let reported = match answered {
            Err(probe_service_schema::CallError::Fault(carried)) => Some(carried),
            Ok(_) | Err(probe_service_schema::CallError::Operation(_)) => None,
        };
        assert!(
            reported.is_some(),
            "a framed fault is a fault, not the operation's declared error"
        );
        let read = reported.unwrap();
        assert_eq!(
            read.kind(),
            probe_service_schema::ProbeServiceFaultKind::UnknownOperation
        );
        assert_eq!(read.operation(), "nothing-answers-to-this");
    }
}

use core::future::{Future, ready};
use core::pin::pin;
use core::task::{Context as PollContext, Poll, Waker};
#[cfg(feature = "typescript")]
use std::sync::Mutex;
use tixschema::{model_schema, service_schema};

// Only the group that reads the wire against the published TypeScript drives these, and that
// group is asked of a build that writes TypeScript at all.
#[cfg(feature = "typescript")]
/// What a reply handle was handed, encoded exactly as a transport would put it on the wire.
pub struct Capture {
    answered: Mutex<Vec<Vec<u8>>>,
}

// Only the group that reads the wire against the published TypeScript drives these, and that
// group is asked of a build that writes TypeScript at all.
#[cfg(feature = "typescript")]
impl Capture {
    // Everything a dispatch settled, read only by the group that compares the wire against the
    // published client and dispatcher — and those are published only where the Zod surface they
    // parse against is.
    #[cfg(feature = "zod")]
    fn answered(&self) -> Vec<Vec<u8>> {
        self.answered.lock().unwrap().clone()
    }

    fn new() -> Self {
        Self {
            answered: Mutex::new(Vec::new()),
        }
    }

    fn only(&self) -> Vec<u8> {
        let held = self.answered.lock().unwrap();
        assert_eq!(
            held.len(),
            1,
            "a request-and-reply arm answers exactly once, through one of the two"
        );
        held[0].clone()
    }
}

// Only the group that reads the wire against the published TypeScript drives these, and that
// group is asked of a build that writes TypeScript at all.
#[cfg(feature = "typescript")]
impl probe_service_schema::Reply for Capture {
    async fn fault(&self, fault: probe_service_schema::ServiceFault) {
        ready(()).await;
        // The fault alone, unframed. What frames it is the transport, and what that framing has to
        // be is exactly what the TypeScript side is read against below.
        self.answered
            .lock()
            .unwrap()
            .push(serde_json::to_vec(&fault).unwrap());
    }

    async fn send<T>(&self, value: T)
    where
        T: serde::Serialize + Send,
    {
        ready(()).await;
        self.answered
            .lock()
            .unwrap()
            .push(serde_json::to_vec(&value).unwrap());
    }
}

// Read only by the group that compares the wire against the published client and dispatcher, and
// those are published only where the Zod surface they parse against is.
#[cfg(all(feature = "typescript", feature = "zod"))]
/// A transport that hands the client one prepared answer, so an envelope written by hand from the
/// emitted TypeScript's own shape can be read back by the generated Rust client.
pub struct PreparedAnswer {
    encoded: Vec<u8>,
}

// Read only by the group that compares the wire against the published client and dispatcher, and
// those are published only where the Zod surface they parse against is.
#[cfg(all(feature = "typescript", feature = "zod"))]
impl probe_service_schema::Transport for PreparedAnswer {
    async fn notify<T>(&self, _operation: &str, _payload: T) -> Result<(), String>
    where
        T: serde::Serialize + Send,
    {
        ready(()).await;
        Ok(())
    }

    async fn request<T>(&self, _operation: &str, _payload: T) -> Result<Vec<u8>, String>
    where
        T: serde::Serialize + Send,
    {
        ready(()).await;
        Ok(self.encoded.clone())
    }
}

#[model_schema()]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ApplyBundleReceipt {
    pub applied: bool,
}

#[model_schema()]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct BalanceRequest {
    pub organization_id: String,
}

#[model_schema()]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct BalanceResponse {
    pub credits: u32,
}

/// A second error type, so the published results are read for keeping each operation's declared
/// error to that operation rather than folding them into one service-wide union.
#[model_schema()]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "errorCode", rename_all = "kebab-case")]
pub enum CreditWriteError {
    Conflict,
    NotFound,
}

#[model_schema()]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "errorCode", rename_all = "kebab-case")]
pub enum ProbeError {
    DbError,
    InsufficientBalance,
}

pub struct ProbeContext {
    pub logger_name: String,
}

#[service_schema()]
pub trait ProbeService<Ctx> {
    /// Answers nothing, and still receives a message a caller has to construct.
    #[service_schema_op(one_way)]
    async fn apply_bundle(&self, ctx: &Ctx, organization_id: String, bundle_id: String);

    /// Two arguments after the context: the message is declared from the argument list, and the
    /// operation names an error unrelated to the others'.
    async fn expire_credit(
        &self,
        ctx: &Ctx,
        organization_id: String,
        credit_id: String,
    ) -> Result<BalanceResponse, CreditWriteError>;

    /// One argument after the context: the argument already is the message.
    async fn get_balance(
        &self,
        ctx: &Ctx,
        req: BalanceRequest,
    ) -> Result<BalanceResponse, ProbeError>;

    /// A fourth operation that answers, so the fault reaches four failure arms rather than three.
    async fn settle(
        &self,
        ctx: &Ctx,
        req: BalanceRequest,
    ) -> Result<ApplyBundleReceipt, ProbeError>;

    /// None at all: an empty message is declared for it.
    async fn sweep(&self, ctx: &Ctx) -> Result<BalanceResponse, ProbeError>;
}

/// A second service in the same bundle, declaring an operation the first one declares too. It
/// exists to be read, not to be driven: what it proves is that two services publishing a
/// `get_balance` each land two distinct result types and two distinct faults in one flat file.
///
/// It also leaves one operation's message to the macro. A generated message carries no service
/// prefix — it publishes under the operation's own name — so this is what puts two services'
/// generated messages into one flat file at once. `reconcile` is not an operation the other
/// service declares, and two services that *did* declare one name cannot be written at all: the
/// second declaration of the message is a duplicate definition in Rust long before a bundle exists
/// to collide in, which the compile-fail run on `messages::emit` pins.
#[service_schema()]
pub trait AuditService<Ctx> {
    async fn get_balance(
        &self,
        ctx: &Ctx,
        req: BalanceRequest,
    ) -> Result<BalanceResponse, CreditWriteError>;

    async fn reconcile(&self, ctx: &Ctx) -> Result<BalanceResponse, CreditWriteError>;
}

pub struct AuditBackEnd;

impl AuditService<ProbeContext> for AuditBackEnd {
    async fn get_balance(
        &self,
        ctx: &ProbeContext,
        req: BalanceRequest,
    ) -> Result<BalanceResponse, CreditWriteError> {
        let seen = ready(req.organization_id.len() + ctx.logger_name.len()).await;
        Ok(BalanceResponse {
            credits: u32::try_from(seen).unwrap_or(0),
        })
    }

    async fn reconcile(&self, ctx: &ProbeContext) -> Result<BalanceResponse, CreditWriteError> {
        let seen = ready(ctx.logger_name.len()).await;
        Ok(BalanceResponse {
            credits: u32::try_from(seen).unwrap_or(0),
        })
    }
}

pub struct ProbeBackEnd {
    pub granted_credits: u32,
}

impl ProbeService<ProbeContext> for ProbeBackEnd {
    async fn apply_bundle(&self, ctx: &ProbeContext, organization_id: String, bundle_id: String) {
        let _settled = ready(ctx.logger_name.len() + organization_id.len() + bundle_id.len()).await;
    }

    async fn expire_credit(
        &self,
        ctx: &ProbeContext,
        organization_id: String,
        credit_id: String,
    ) -> Result<BalanceResponse, CreditWriteError> {
        let seen = ready(organization_id.len() + credit_id.len()).await;
        if ctx.logger_name.is_empty() {
            Err(CreditWriteError::Conflict)
        } else {
            Ok(BalanceResponse {
                credits: u32::try_from(seen).unwrap_or(0),
            })
        }
    }

    async fn get_balance(
        &self,
        ctx: &ProbeContext,
        req: BalanceRequest,
    ) -> Result<BalanceResponse, ProbeError> {
        let seen = ready(req.organization_id.len() + ctx.logger_name.len()).await;
        Ok(BalanceResponse {
            credits: self.granted_credits + u32::try_from(seen).unwrap_or(0),
        })
    }

    async fn settle(
        &self,
        ctx: &ProbeContext,
        req: BalanceRequest,
    ) -> Result<ApplyBundleReceipt, ProbeError> {
        let seen = ready(req.organization_id.len()).await;
        if ctx.logger_name.is_empty() {
            Err(ProbeError::DbError)
        } else {
            Ok(ApplyBundleReceipt { applied: seen > 0 })
        }
    }

    async fn sweep(&self, ctx: &ProbeContext) -> Result<BalanceResponse, ProbeError> {
        let _settled = ready(ctx.logger_name.len()).await;
        Ok(BalanceResponse {
            credits: self.granted_credits,
        })
    }
}

/// The probe never suspends, so one poll answers it; `None` says an assumption about the bodies
/// above stopped holding rather than that the runtime is missing.
fn poll_once<Answered>(answering: Answered) -> Option<Answered::Output>
where
    Answered: Future,
{
    let mut pinned = pin!(answering);
    let mut polling = PollContext::from_waker(Waker::noop());
    match pinned.as_mut().poll(&mut polling) {
        Poll::Ready(answer) => Some(answer),
        Poll::Pending => None,
    }
}

/// Read in every feature combination: the TypeScript emission is additive, so the trait the macro
/// emits is still the trait an implementation satisfies and a caller calls.
#[test]
fn the_second_service_answers_the_operation_its_generated_message_was_declared_for() {
    let answered = poll_once(AuditBackEnd.reconcile(&ProbeContext {
        logger_name: "audit".to_owned(),
    }))
    .unwrap();
    assert_eq!(
        answered.map_or(u32::MAX, |balance| balance.credits),
        5,
        "the operation whose message the macro declared for the *second* service is one the \
         second service answers"
    );
}

#[test]
fn the_service_is_still_implementable_and_callable_alongside_its_published_typescript() {
    let service = ProbeBackEnd { granted_credits: 5 };
    let ctx = ProbeContext {
        logger_name: "probe".to_owned(),
    };

    let answered = poll_once(service.get_balance(
        &ctx,
        BalanceRequest {
            organization_id: "acme".to_owned(),
        },
    ))
    .unwrap();
    assert_eq!(answered.unwrap().credits, 14);

    let refused = poll_once(service.expire_credit(
        &ProbeContext {
            logger_name: String::new(),
        },
        "acme".to_owned(),
        "cr-1".to_owned(),
    ))
    .unwrap();
    assert!(matches!(refused, Err(CreditWriteError::Conflict)));

    let swept = poll_once(service.sweep(&ctx)).unwrap();
    assert_eq!(swept.unwrap().credits, 5);

    let settled = poll_once(service.settle(
        &ctx,
        BalanceRequest {
            organization_id: "acme".to_owned(),
        },
    ))
    .unwrap();
    assert!(settled.unwrap().applied);

    assert!(poll_once(service.apply_bundle(&ctx, "acme".to_owned(), "b-1".to_owned())).is_some());

    // The second service in the bundle is a service, not just an expansion: it is implemented and
    // called like the first, and the two publish types that do not collide.
    let audited = poll_once(AuditBackEnd.get_balance(
        &ctx,
        BalanceRequest {
            organization_id: "acme".to_owned(),
        },
    ))
    .unwrap();
    assert_eq!(audited.unwrap().credits, 9);
}

// Read only by the group that compares the wire against the published client and dispatcher, and
// those are published only where the Zod surface they parse against is.
#[cfg(all(feature = "typescript", feature = "zod"))]
/// Everything one dispatch put on the reply handle, which for a one-way arm that ran is nothing.
fn settlements(operation: &str, payload: &[u8]) -> Vec<Vec<u8>> {
    let capture = Capture::new();
    let settled = poll_once(probe_service_schema::dispatch(
        &ProbeBackEnd { granted_credits: 5 },
        &ProbeContext {
            logger_name: "probe".to_owned(),
        },
        &probe_service_schema::IncomingMessage {
            operation: operation.to_owned(),
            payload: payload.to_vec(),
        },
        &capture,
    ));
    assert!(settled.is_some(), "the probe never suspends");
    capture.answered()
}

#[cfg(feature = "typescript")]
/// Drives the generated dispatcher over one message and answers with what the reply handle was
/// handed.
fn dispatched(operation: &str, payload: &[u8], logger_name: &str) -> Vec<u8> {
    let capture = Capture::new();
    let settled = poll_once(probe_service_schema::dispatch(
        &ProbeBackEnd { granted_credits: 5 },
        &ProbeContext {
            logger_name: logger_name.to_owned(),
        },
        &probe_service_schema::IncomingMessage {
            operation: operation.to_owned(),
            payload: payload.to_vec(),
        },
        &capture,
    ));
    assert!(settled.is_some(), "the probe never suspends");
    capture.only()
}
