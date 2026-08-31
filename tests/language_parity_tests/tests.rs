//! One declaration, two validators, one sentence.
//!
//! Everything here compares the two things `#[model_schema()]` emits for the same bound against
//! each other: the report the generated Rust validator answers with, and the report the generated
//! Zod schema would answer with. Nothing asserts that the Rust text merely *changed* — a test that
//! reads only one language cannot see the two drift apart, which is how they drifted in the first
//! place.
//!
//! **How the Zod half is read.** No zod runs here, so what it would say is read off the schema the
//! macro published: the `{ error: … }` argument written for each check, rendered the way zod
//! renders one. That leaves exactly the two holes the emitter can write — the length of the value
//! and the value itself — and filling them is [`zod_sentence`]. The strings the tests assert are
//! the ones zod 4 actually produced for these schemas.
//!
//! **What the dispatcher adds.** Neither sentence names its field. The Rust validator writes
//! `'{field}': ` in front of its own, and the generated TypeScript dispatcher writes
//! `'${issue.path.join(".")}': ` in front of zod's, so the two lines agree once the same path is
//! written in front of both. Each comparison below writes that path itself, from the key the
//! payload spells.

#[cfg(all(feature = "serde", feature = "zod"))]
mod declarations {
    use serde::{Deserialize, Serialize};
    use tixschema::model_schema;

    /// A brand carrying two bounds that one value breaks both of.
    #[model_schema(minLength = 3, pattern = "^[a-z][a-z0-9_-]+$")]
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(transparent)]
    pub struct OrganizationId(pub String);

    /// A bound two hops down, one of them flattened, named by the path the payload spells rather
    /// than by the field it was declared on.
    #[model_schema()]
    #[derive(Debug, Deserialize, Serialize)]
    pub struct Claims {
        #[model_schema_prop(minLength = 1)]
        pub jti: String,
    }

    #[model_schema()]
    #[derive(Debug, Deserialize, Serialize)]
    pub struct Account {
        #[serde(flatten)]
        pub claims: Claims,
    }

    #[model_schema()]
    #[derive(Debug, Deserialize, Serialize)]
    pub struct Held {
        pub account: Account,
        pub organization_id: OrganizationId,
    }

    /// A field carrying its own bounds, in the two pairings a single value can break at once.
    #[model_schema()]
    #[derive(Debug, Deserialize, Serialize)]
    pub struct Direct {
        #[model_schema_prop(minLength = 3, pattern = "^[a-z][a-z0-9_-]+$")]
        pub organization_id: String,
    }

    #[model_schema()]
    #[derive(Debug, Deserialize, Serialize)]
    pub struct Capped {
        #[model_schema_prop(maxLength = 5, pattern = "^[a-z]+$")]
        pub bio: String,
    }

    /// The numeric pair, whose sentences quote the value back rather than its length.
    #[model_schema()]
    #[derive(Debug, Deserialize, Serialize)]
    pub struct Floor {
        #[model_schema_prop(minimum = 10)]
        pub credit_count: i64,
    }

    #[model_schema()]
    #[derive(Debug, Deserialize, Serialize)]
    pub struct Ceiling {
        #[model_schema_prop(maximum = 100)]
        pub credit_count: i64,
    }
}

/// The refusal path: one declaration, two dispatchers, one answer.
///
/// Everything above compares the two *validators* a bound publishes. What a caller actually reads
/// when it sends a bad value is a fault, and a fault is built by the dispatcher — so this compares
/// those: the fault the generated Rust dispatcher answers a payload with, against what the
/// generated TypeScript dispatcher would answer the same payload, read off the TypeScript that
/// dispatcher published rather than written down here.
///
/// The `typescript` feature is part of the gate, unlike everything above it. What the comparison
/// reads the TypeScript half from is `GateServiceSchema`, and only a build emitting TypeScript
/// declares that type at all — a build without it has the Rust dispatcher and nothing to hold it
/// against, which is a comparison that cannot be written rather than one that passes trivially.
#[cfg(all(feature = "serde", feature = "typescript", feature = "zod"))]
mod refusals {
    use super::declarations::{Account, OrganizationId};
    // The schema module each held type publishes is named beside the type, and the emitted code
    // reaches it unqualified — so a type declared elsewhere is held by bringing its module along.
    // Only the JSON-schema emission reaches for a sibling module that way; the Zod and TypeScript
    // surfaces name the published type itself and need nothing brought along for it.
    #[cfg(feature = "jsonschema")]
    use super::declarations::{account_schema, organization_id_schema};
    use core::future::{Future, ready};
    use core::pin::pin;
    use core::task::{Context as PollContext, Poll, Waker};
    use serde::{Deserialize, Serialize};
    use std::sync::Mutex;
    use tixschema::{model_schema, service_schema};

    /// The message the gate sends, spelled on the wire the way the service it was captured from
    /// spells it: `rename_all` makes every key differ from the Rust field it stands for, which is
    /// what a fault naming the wrong one of the two is caught by.
    #[model_schema()]
    #[derive(Debug, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct AdmitRequest {
        pub account: Account,
        pub organization_id: OrganizationId,
    }

    #[model_schema()]
    #[derive(Debug, Deserialize, Serialize)]
    pub struct Admitted {
        pub admitted: bool,
    }

    #[service_schema]
    pub trait GateService<Ctx> {
        async fn admit(&self, ctx: &Ctx, req: AdmitRequest) -> Result<Admitted, String>;
    }

    /// An implementation that writes down every message it was handed, so a payload that should
    /// have been refused before it is read as a comparison against the wrong thing.
    #[derive(Default)]
    pub struct NeverEntered {
        reached: Mutex<Vec<String>>,
    }

    /// The reply handle, keeping what was answered as the bytes a caller reads.
    #[derive(Default)]
    pub struct Recorder {
        settled: Mutex<Vec<serde_json::Value>>,
    }

    impl<Ctx> GateService<Ctx> for NeverEntered
    where
        Ctx: Sync,
    {
        async fn admit(&self, _ctx: &Ctx, req: AdmitRequest) -> Result<Admitted, String> {
            ready(()).await;
            self.reached.lock().unwrap().push(format!("{req:?}"));
            Ok(Admitted { admitted: true })
        }
    }

    impl gate_service_schema::Reply for Recorder {
        async fn fault(&self, fault: gate_service_schema::ServiceFault) {
            ready(()).await;
            self.settled
                .lock()
                .unwrap()
                .push(serde_json::to_value(&fault).unwrap());
        }

        async fn send<T>(&self, value: T)
        where
            T: Serialize + Send,
        {
            ready(()).await;
            self.settled
                .lock()
                .unwrap()
                .push(serde_json::to_value(&value).unwrap());
        }
    }

    /// The body of the fault constructor the emitted TypeScript dispatcher answers a refused
    /// payload with, cut out of the bundle it published.
    pub fn inbound_fault() -> String {
        let written = GateServiceSchema::ts_service();
        let opened = written
            .split_once("function gateServiceInboundFault(")
            .unwrap()
            .1;
        opened.split_once("\n}").unwrap().0.to_owned()
    }

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

    /// The one thing the Rust dispatcher answered `payload` with, as the bytes a caller reads.
    pub fn refused(payload: &str) -> serde_json::Value {
        let service = NeverEntered::default();
        let reply = Recorder::default();
        poll_once(gate_service_schema::dispatch(
            &service,
            &(),
            &gate_service_schema::IncomingMessage {
                operation: "admit".to_owned(),
                payload: payload.as_bytes().to_vec(),
            },
            &reply,
        ))
        .unwrap();
        let reached = service.reached.lock().unwrap().clone();
        assert!(
            reached.is_empty(),
            "`{payload}` reached the implementation: {reached:?}"
        );
        let mut settled = reply.settled.lock().unwrap().clone();
        assert_eq!(
            settled.len(),
            1,
            "the arm answers exactly once: {settled:?}"
        );
        settled.pop().unwrap()
    }

    /// The `kind` that constructor writes. It has to be a constant for it to be the kind of every
    /// payload the constructor answers — a conditional leaves this reader with nothing to return
    /// rather than a branch to pick, which is the point.
    pub fn ts_kind(body: &str) -> String {
        let written = body
            .split_once("kind: ")
            .unwrap()
            .1
            .split_once(',')
            .unwrap()
            .0
            .trim();
        written
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
            .unwrap()
            .to_owned()
    }

    /// The key the emitted Zod object writes `member` under, which is the spelling on the wire.
    pub fn zod_key_holding(schema: &str, member: &str) -> String {
        let closing = format!(": {member},");
        schema
            .lines()
            .find_map(|line| line.trim().strip_suffix(&closing))
            .unwrap()
            .to_owned()
    }
}

/// The sentence one emitted Zod check reports for a value, read off the `{ error: … }` argument the
/// macro wrote for it.
///
/// zod hands a check's own error the value the check was given and takes back a string, so
/// rendering one here is filling the holes: `rendered` is the value as `String(…)` writes it, and
/// its `.length` is JavaScript's, counted in UTF-16 code units. An `error` that is a constant has
/// no hole and is its own answer.
#[cfg(all(feature = "serde", feature = "zod"))]
fn zod_sentence(error_argument: &str, rendered: &str) -> String {
    if let Some(constant) = error_argument
        .strip_prefix("{ error: \"")
        .and_then(|rest| rest.strip_suffix("\" }"))
    {
        return unescape_js(constant);
    }
    assert!(
        error_argument.starts_with("{ error: (issue) => `"),
        "not an error argument this emitter writes: {error_argument}"
    );
    let template = error_argument
        .strip_prefix("{ error: (issue) => `")
        .and_then(|rest| rest.strip_suffix("` }"))
        .unwrap();
    template
        .replace(
            "${String(issue.input).length}",
            &rendered.encode_utf16().count().to_string(),
        )
        .replace("${String(issue.input)}", rendered)
}

/// A JavaScript quoted string read back to the text it stands for, which is what zod reports.
#[cfg(all(feature = "serde", feature = "zod"))]
fn unescape_js(quoted: &str) -> String {
    let mut read = String::with_capacity(quoted.len());
    let mut chars = quoted.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            read.push(ch);
            continue;
        }
        let escaped = chars.next().unwrap();
        match escaped {
            'n' => read.push('\n'),
            'r' => read.push('\r'),
            'u' => {
                let code: String = chars.by_ref().take(4).collect();
                read.push(char::from_u32(u32::from_str_radix(&code, 16).unwrap()).unwrap());
            }
            _ => read.push(escaped),
        }
    }
    read
}

/// Every `{ error: … }` argument `schema` carries, in the order they are written — which is the
/// order zod runs the checks and so the order it reports them in.
#[cfg(all(feature = "serde", feature = "zod"))]
fn error_arguments(schema: &str) -> Vec<String> {
    let chars: Vec<char> = schema.chars().collect();
    let opener: Vec<char> = "{ error: ".chars().collect();
    let mut found = Vec::new();
    let mut at = 0;
    while at + opener.len() <= chars.len() {
        if chars[at..at + opener.len()] != opener[..] {
            at += 1;
            continue;
        }
        let end = closing_brace(&chars, at).unwrap();
        found.push(chars[at..=end].iter().collect());
        at = end + 1;
    }
    found
}

/// The index of the `}` closing the object literal that opens at `from`, reading the JavaScript
/// between them: a quoted string and a template literal hide their braces from the count, and a
/// template's `${…}` hole is skipped whole — every hole this emitter writes holds one expression
/// and no brace of its own. `None` where the literal is never closed.
#[cfg(all(feature = "serde", feature = "zod"))]
fn closing_brace(chars: &[char], from: usize) -> Option<usize> {
    let mut depth = 0_usize;
    let mut quoted: Option<char> = None;
    let mut here = from;
    while here < chars.len() {
        let ch = chars[here];
        match quoted {
            Some(delimiter) => match ch {
                '\\' => here += 1,
                '$' if delimiter == '`' && chars.get(here + 1) == Some(&'{') => {
                    here += chars[here..]
                        .iter()
                        .position(|&candidate| candidate == '}')?;
                }
                _ if ch == delimiter => quoted = None,
                _ => {}
            },
            None => match ch {
                '"' | '`' => quoted = Some(ch),
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(here);
                    }
                }
                _ => {}
            },
        }
        here += 1;
    }
    None
}

/// What the TypeScript dispatcher would report for `schema` under `path`, given the value each
/// check was handed: one line per violation, in the order zod reports them.
#[cfg(all(feature = "serde", feature = "zod"))]
fn typescript_report(schema: &str, path: &str, rendered: &str) -> Vec<String> {
    error_arguments(schema)
        .iter()
        .map(|argument| format!("'{path}': {}", zod_sentence(argument, rendered)))
        .collect()
}

/// A field's own bounds, both broken by one value: the Rust validator names both, in the order the
/// Zod schema's checks would name them, in the same words.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_value_breaking_two_of_a_fields_bounds_reads_the_same_in_both_languages() {
    let broken = declarations::Direct {
        organization_id: "A!".to_owned(),
    };
    let rust = broken.validate().unwrap_err();

    assert_eq!(
        rust,
        vec![
            "'organization_id': too short: minimum length is 3, got 2",
            "'organization_id': does not match pattern '^[a-z][a-z0-9_-]+$'",
        ],
    );
    assert_eq!(
        rust,
        typescript_report(&declarations::Direct::zod_schema(), "organization_id", "A!"),
    );
}

/// The other pairing one value can break at once, so `maxLength` is compared beside `minLength`.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_value_over_a_length_and_off_a_pattern_reads_the_same_in_both_languages() {
    let broken = declarations::Capped {
        bio: "ABCDEFG".to_owned(),
    };
    let rust = broken.validate().unwrap_err();

    assert_eq!(
        rust,
        vec![
            "'bio': too long: maximum length is 5, got 7",
            "'bio': does not match pattern '^[a-z]+$'",
        ],
    );
    assert_eq!(
        rust,
        typescript_report(&declarations::Capped::zod_schema(), "bio", "ABCDEFG"),
    );
}

/// A brand's own report names no field on either side — the field it is held in supplies the
/// name — so both are compared under that field.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_brands_two_bounds_read_the_same_in_both_languages() {
    let rust = declarations::OrganizationId("A!".to_owned())
        .validate()
        .unwrap_err();

    assert_eq!(
        rust,
        vec![
            "too short: minimum length is 3, got 2",
            "does not match pattern '^[a-z][a-z0-9_-]+$'",
        ],
    );

    let under_the_field: Vec<String> = rust
        .iter()
        .map(|violation| format!("'organization_id': {violation}"))
        .collect();
    assert_eq!(
        under_the_field,
        typescript_report(
            &declarations::OrganizationId::zod_schema(),
            "organization_id",
            "A!"
        ),
    );
}

/// The same brand reached as a field of a message: what the enclosing validator reports is the
/// line the dispatcher builds on the other side, byte for byte.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_brand_held_in_a_field_reads_the_same_in_both_languages() {
    let broken = declarations::Held {
        account: declarations::Account {
            claims: declarations::Claims {
                jti: "ok".to_owned(),
            },
        },
        organization_id: declarations::OrganizationId("A!".to_owned()),
    };

    assert_eq!(
        broken.validate().unwrap_err(),
        typescript_report(
            &declarations::OrganizationId::zod_schema(),
            "organization_id",
            "A!"
        ),
    );
}

/// A bound two hops down, one of them flattened, named by the path the payload spells. The Zod
/// schema carrying the check is the one the inner type published, which is the schema the outer
/// one composes.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_bound_reached_through_a_flattened_hop_reads_the_same_in_both_languages() {
    let broken = declarations::Held {
        account: declarations::Account {
            claims: declarations::Claims { jti: String::new() },
        },
        organization_id: declarations::OrganizationId("acme".to_owned()),
    };
    let rust = broken.validate().unwrap_err();

    assert_eq!(
        rust,
        vec!["'account.jti': too short: minimum length is 1, got 0"]
    );
    assert_eq!(
        rust,
        typescript_report(&declarations::Claims::zod_schema(), "account.jti", ""),
    );
}

/// A numeric bound quotes the value back rather than its length, and does so identically.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_numeric_bound_reads_the_same_in_both_languages() {
    let under_the_floor = declarations::Floor { credit_count: 2 }
        .validate()
        .unwrap_err();
    assert_eq!(
        under_the_floor,
        vec!["'credit_count': too small: minimum is 10, got 2"]
    );
    assert_eq!(
        under_the_floor,
        typescript_report(&declarations::Floor::zod_schema(), "credit_count", "2"),
    );

    let over_the_ceiling = declarations::Ceiling { credit_count: 150 }
        .validate()
        .unwrap_err();
    assert_eq!(
        over_the_ceiling,
        vec!["'credit_count': too large: maximum is 100, got 150"]
    );
    assert_eq!(
        over_the_ceiling,
        typescript_report(&declarations::Ceiling::zod_schema(), "credit_count", "150"),
    );
}

/// A bound emitted without a sentence would report zod's words on one side and the macro's on the
/// other, and every comparison above would miss it — each reads the checks it is given. This one
/// reads the schema instead: every check the emitter can write carries an `error`.
#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn no_emitted_check_is_left_to_report_in_zods_own_words() {
    for schema in [
        declarations::Capped::zod_schema(),
        declarations::Ceiling::zod_schema(),
        declarations::Claims::zod_schema(),
        declarations::Direct::zod_schema(),
        declarations::Floor::zod_schema(),
        declarations::OrganizationId::zod_schema(),
    ] {
        let checks = [".min(", ".max(", "z.minLength(", "z.maxLength(", "z.regex("]
            .iter()
            .map(|check| schema.matches(check).count())
            .sum::<usize>();
        assert_eq!(
            checks,
            error_arguments(&schema).len(),
            "a check reports in zod's own words in:\n{schema}"
        );
    }
}

/// A brand's bound, broken twice by one value, refused as the payload is read.
///
/// The brand gates its own read, so this payload never reaches the message's validator — and the
/// brand's report names no field, the brand being the value rather than a member of anything. What
/// a caller is owed is still the key it got wrong, and it is owed the same one from either
/// language: the field the value was held in, spelled the way the wire spells it.
#[cfg(all(feature = "serde", feature = "typescript", feature = "zod"))]
#[test]
fn a_brand_refused_on_the_read_answers_the_same_fault_in_both_languages() {
    let answered = refusals::refused(r#"{"account":{"jti":"a"},"organizationId":"A!"}"#);
    let constructor = refusals::inbound_fault();
    let held_under = refusals::zod_key_holding(
        &refusals::AdmitRequest::zod_schema(),
        "OrganizationId$Schema",
    );
    assert_eq!(held_under, "organizationId");

    assert!(
        constructor
            .contains("const failedAt = first === undefined ? \"\" : first.path.join(\".\");"),
        "the field is the path of the first issue. Got: {constructor}"
    );
    assert!(
        constructor.contains("field: failedAt === \"\" ? undefined : failedAt,"),
        "got: {constructor}"
    );
    assert!(constructor.contains(".join(\"; \"),"), "got: {constructor}");

    // What TypeScript answers, read off what it published: one line per issue, under the key the
    // Zod object wrote, joined the way the constructor joins them.
    let reported = typescript_report(
        &declarations::OrganizationId::zod_schema(),
        &held_under,
        "A!",
    );
    assert_eq!(answered["kind"], refusals::ts_kind(&constructor));
    assert_eq!(answered["field"], held_under);
    assert_eq!(answered["detail"], reported.join("; "));
    assert_eq!(answered["operation"], "admit");
    assert!(
        !answered["detail"].as_str().unwrap().contains("at line"),
        "the byte offset serde appends locates the failure inside an encoding the caller never \
         saw. Got: {answered}"
    );
}

/// A bound two hops down, one of them flattened, reached by the message's own validator rather than
/// on the read — the other half of the same comparison, so the two paths cannot be shown to agree
/// with TypeScript one at a time while disagreeing with each other.
#[cfg(all(feature = "serde", feature = "typescript", feature = "zod"))]
#[test]
fn a_bound_below_a_flattened_hop_answers_the_same_fault_in_both_languages() {
    let answered = refusals::refused(r#"{"account":{"jti":""},"organizationId":"acme"}"#);
    let constructor = refusals::inbound_fault();
    let reported = typescript_report(&declarations::Claims::zod_schema(), "account.jti", "");

    assert_eq!(answered["kind"], refusals::ts_kind(&constructor));
    assert_eq!(answered["field"], "account.jti");
    assert_eq!(answered["detail"], reported.join("; "));
}

/// A payload that parsed and is still not the message: not an object at all.
///
/// It is the one shape the two languages classified differently. TypeScript read "failed at no key"
/// as bytes that were never the message, where the Rust side had already established that the bytes
/// *were* a document and answered for what the document said. Only one of those can be true of a
/// value that parsed, so both answer the one kind now — and neither names a field, there being no
/// key to send a caller to.
#[cfg(all(feature = "serde", feature = "typescript", feature = "zod"))]
#[test]
fn a_payload_that_parsed_and_is_not_the_message_answers_the_same_kind_in_both_languages() {
    let constructor = refusals::inbound_fault();
    for payload in [r#""just a string""#, "42", "[1,2,3]"] {
        let answered = refusals::refused(payload);
        assert_eq!(
            answered["kind"],
            refusals::ts_kind(&constructor),
            "`{payload}` was answered under a kind the other language does not answer it under, \
             and the kind is what a caller branches on. Got: {answered}"
        );
        assert_eq!(
            answered["field"],
            serde_json::Value::Null,
            "`{payload}` failed at no key, and there is no key to send a caller to. Got: {answered}"
        );
        assert!(
            !answered["detail"].as_str().unwrap().contains("at line"),
            "got: {answered}"
        );
    }
}
