//! One service covering every input shape and both outcomes, dispatched over payloads that take
//! every path through an arm: the answer, the operation's own error, an undeserializable payload,
//! a name nothing answers to, a handler that panicked, and a one-way operation that settles
//! without publishing.
//!
//! The probe reply handle records which of `send` and `fault` was called, so a test can assert not
//! only what was answered but that a request-and-reply arm answered exactly once and that a
//! one-way arm that reached its implementation answered nothing at all.

#![cfg(feature = "serde")]

/// A message annotated with `#[model_schema_prop]`, and where a violation of it is caught.
///
/// Gated because the annotation is: the constraint is read, and the validator that enforces it
/// written, only when `serde` is on beside a surface that reads the constraint.
///
/// **The two answers a payload can earn here are different answers, and the arm gives whichever
/// one is true.** Bytes that are not a document at all never become anything, and the fault says
/// the sender's serialization is broken. Everything that *does* read as a document and is then
/// turned away — a field carrying the wrong type of value, a key that is missing, a value a
/// constraint refuses — is a value someone supplied that the message does not admit, and the fault
/// says that instead and names the field wherever the refusal named one.
///
/// A receiver acts differently on each, and the line between them is `serde_json`'s own
/// classification of its refusal rather than the shape of the sentence it wrote. It is also where
/// the TypeScript service serving the same operation draws it: its reader parses the payload and
/// its schema then judges what was read, so a type mismatch and a broken bound are one kind there
/// and are one kind here. Constraints stay enforced by the validator alone and never as the
/// payload is read, which is what lets a broken bound name its field at all.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
mod a_message_annotated_with_a_constraint {
    use super::poll_once;
    use core::future::ready;
    use serde::de::Error as DeError;
    use serde::{Deserialize, Serialize};
    use std::sync::Mutex;
    use tixschema::{model_schema, service_schema};

    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    pub struct GateRequest {
        #[model_schema_prop(minLength = 3)]
        pub organization_id: String,
    }

    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    pub struct Admitted {
        pub admitted: bool,
    }

    /// A message whose author wrote the serde hook by hand rather than annotating the field.
    ///
    /// Annotating a field no longer puts a check on the read, so this is what is left that can
    /// refuse a payload in a validator's words: an author who wants the wire itself gated, and who
    /// reports it in the shape a generated validator reports in — the field first and in single
    /// quotes. A fault built from such a refusal reads the name back off it.
    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    pub struct LedgerRequest {
        #[serde(deserialize_with = "refuse_a_short_ledger")]
        pub ledger_id: String,
    }

    /// The claims an account context carries, and the only bound anywhere on the message below.
    ///
    /// This is the shape a live bus was replayed against: a message declaring no constraint of its
    /// own, an account context beneath it, and the constrained field one level below *that*. The
    /// composed Zod schema the same declaration publishes checks it, so a Rust service that did
    /// not would accept a message the TypeScript one refuses.
    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    pub struct GateClaims {
        #[model_schema_prop(minLength = 1)]
        pub jti: String,
    }

    /// The claims are `#[serde(flatten)]`, so `jti` is a key of the account object itself and the
    /// path a violation is reported under has to skip the hop, exactly as the wire does.
    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    pub struct GateAccount {
        pub aud: String,
        #[serde(flatten)]
        pub claims: GateClaims,
    }

    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct MintRequest {
        pub account: GateAccount,
        pub credit_count: u32,
    }

    /// Records every message that reached it, which is how a test says an invalid one did not.
    pub struct GateBackEnd {
        reached: Mutex<Vec<String>>,
    }

    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "kebab-case", tag = "errorCode")]
    pub enum GateError {
        DbError,
    }

    /// This service's own reply handle: the types are generated per service, so serving a second
    /// one means implementing a second `Reply`.
    pub struct GateReply {
        faults: Mutex<Vec<gate_service_schema::ServiceFault>>,
        settled: Mutex<Vec<String>>,
    }

    #[service_schema()]
    pub trait GateService<Ctx> {
        async fn admit(&self, ctx: &Ctx, req: GateRequest) -> Result<Admitted, GateError>;

        async fn mint(&self, ctx: &Ctx, req: MintRequest) -> Result<Admitted, GateError>;

        async fn open_ledger(&self, ctx: &Ctx, req: LedgerRequest) -> Result<Admitted, GateError>;
    }

    impl GateService<()> for GateBackEnd {
        async fn admit(&self, _ctx: &(), req: GateRequest) -> Result<Admitted, GateError> {
            ready(()).await;
            self.reached.lock().unwrap().push(req.organization_id);
            Ok(Admitted { admitted: true })
        }

        async fn mint(&self, _ctx: &(), req: MintRequest) -> Result<Admitted, GateError> {
            ready(()).await;
            self.reached.lock().unwrap().push(format!(
                "minted {} for {}",
                req.credit_count, req.account.aud
            ));
            Ok(Admitted { admitted: true })
        }

        async fn open_ledger(&self, _ctx: &(), req: LedgerRequest) -> Result<Admitted, GateError> {
            ready(()).await;
            self.reached.lock().unwrap().push(req.ledger_id);
            Ok(Admitted { admitted: true })
        }
    }

    impl gate_service_schema::Reply for GateReply {
        async fn fault(&self, fault: gate_service_schema::ServiceFault) {
            ready(()).await;
            self.settled.lock().unwrap().push(fault.to_string());
            self.faults.lock().unwrap().push(fault);
        }

        async fn send<T>(&self, value: T)
        where
            T: Serialize + Send,
        {
            ready(()).await;
            self.settled
                .lock()
                .unwrap()
                .push(serde_json::to_string(&value).unwrap());
        }
    }

    impl GateBackEnd {
        fn new() -> Self {
            Self {
                reached: Mutex::new(Vec::new()),
            }
        }

        fn reached(&self) -> Vec<String> {
            self.reached.lock().unwrap().clone()
        }
    }

    impl GateReply {
        fn faults(&self) -> Vec<gate_service_schema::ServiceFault> {
            self.faults.lock().unwrap().clone()
        }

        fn new() -> Self {
            Self {
                faults: Mutex::new(Vec::new()),
                settled: Mutex::new(Vec::new()),
            }
        }

        fn settled(&self) -> Vec<String> {
            self.settled.lock().unwrap().clone()
        }
    }

    /// Refuses a short ledger id as the payload is read, in the words a generated validator would
    /// have used.
    fn refuse_a_short_ledger<'de, D>(deserializer: D) -> Result<String, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let read = String::deserialize(deserializer)?;
        if read.len() < 3 {
            return Err(DeError::custom(format!(
                "'ledger_id' is too short: minimum length is 3, got {}",
                read.len()
            )));
        }
        Ok(read)
    }

    #[test]
    fn a_payload_carrying_a_value_the_constraint_refuses_fails_validation_and_names_the_field() {
        let service = GateBackEnd::new();
        let reply = GateReply::new();
        poll_once(gate_service_schema::dispatch(
            &service,
            &(),
            &gate_service_schema::IncomingMessage {
                operation: "admit".to_owned(),
                payload: br#"{"organization_id":"ab"}"#.to_vec(),
            },
            &reply,
        ))
        .unwrap();
        assert!(
            service.reached().is_empty(),
            "an implementation may assume its incoming message is valid, and this one is not. \
             Got: {:?}",
            service.reached()
        );
        assert_eq!(reply.settled().len(), 1, "got: {:?}", reply.settled());
        let reported = reply.faults();
        assert_eq!(
            reported[0].kind(),
            gate_service_schema::ServiceFaultKind::FailedValidation,
            "this payload *is* a message — every key is present and every value is of the type \
             the field declared. What it is not is a message satisfying the constraint, and \
             telling the sender its serialization was broken would send it looking in the wrong \
             place entirely. Got detail: {}",
            reported[0].detail()
        );
        assert_eq!(reported[0].operation(), "admit");
        assert!(
            reported[0].detail().contains("is too short"),
            "got: {}",
            reported[0].detail()
        );
        assert_eq!(
            reported[0].field(),
            Some("organization_id"),
            "the caller has to be told which field it got wrong. Got: {}",
            reported[0].detail()
        );
    }

    #[test]
    fn a_payload_that_is_not_a_message_is_refused_under_the_class_of_failure_it_is() {
        // Wrong about the type its field declared, missing the key entirely, and not a document in
        // the first place. None of the three is a message, and none reaches the implementation —
        // a required key in particular must not have become defaultable when the constraint checks
        // moved to the validator. What differs is the class each one is: the first two read as a
        // document and did not match, which is what a caller supplying a bad value produces, while
        // the third never read at all.
        for (payload, kind, field) in [
            (
                br#"{"organization_id":42}"#.to_vec(),
                gate_service_schema::ServiceFaultKind::FailedValidation,
                None,
            ),
            (
                b"{}".to_vec(),
                gate_service_schema::ServiceFaultKind::FailedValidation,
                Some("organization_id"),
            ),
            (
                b"not a document at all".to_vec(),
                gate_service_schema::ServiceFaultKind::UndeserializablePayload,
                None,
            ),
        ] {
            let service = GateBackEnd::new();
            let reply = GateReply::new();
            poll_once(gate_service_schema::dispatch(
                &service,
                &(),
                &gate_service_schema::IncomingMessage {
                    operation: "admit".to_owned(),
                    payload: payload.clone(),
                },
                &reply,
            ))
            .unwrap();
            let sent = String::from_utf8_lossy(&payload).into_owned();
            assert!(
                service.reached().is_empty(),
                "`{sent}` reached the implementation: {:?}",
                service.reached()
            );
            let reported = reply.faults();
            assert_eq!(
                reported[0].kind(),
                kind,
                "`{sent}` was answered under the wrong class of failure, and the class is what a \
                 caller branches on. Got detail: {}",
                reported[0].detail()
            );
            assert_eq!(
                reported[0].field(),
                field,
                "`{sent}` must name the field its refusal named and no other: a name invented for \
                 a refusal that carried none would be read out of a message nobody read. Got: {}",
                reported[0].detail()
            );
            assert!(
                !reported[0].detail().contains("at line"),
                "the byte offset serde appends locates the failure inside an encoding the caller \
                 never saw. Got: {}",
                reported[0].detail()
            );
        }
    }

    /// The defect a live bus found: the message's own fields declare nothing, the bound is two
    /// hops down, and the operation ran anyway.
    ///
    /// A message reports what a walk of its whole tree reports, not what its top level reports, so
    /// the path a violation carries is the path it was reached through — and the flattened hop
    /// writes no key, so it contributes no segment either. `account.jti` is what the composed Zod
    /// schema names for the same payload, which is the point: one declaration, one answer.
    #[test]
    fn a_bound_two_levels_down_refuses_the_message_before_the_implementation_runs() {
        let service = GateBackEnd::new();
        let reply = GateReply::new();
        poll_once(gate_service_schema::dispatch(
            &service,
            &(),
            &gate_service_schema::IncomingMessage {
                operation: "mint".to_owned(),
                payload: br#"{"account":{"aud":"acme","jti":""},"creditCount":1}"#.to_vec(),
            },
            &reply,
        ))
        .unwrap();
        assert!(
            service.reached().is_empty(),
            "the operation ran on a message the same declaration's Zod schema refuses, which is \
             the two ends of one declaration disagreeing about what a message is. Got: {:?}",
            service.reached()
        );
        let reported = reply.faults();
        assert_eq!(
            reported[0].kind(),
            gate_service_schema::ServiceFaultKind::FailedValidation,
            "got detail: {}",
            reported[0].detail()
        );
        assert_eq!(
            reported[0].field(),
            Some("account.jti"),
            "the caller has to be told which field it got wrong, and where. Got: {}",
            reported[0].detail()
        );
        assert_eq!(
            reported[0].detail(),
            "'account.jti' is too short: minimum length is 1, got 0"
        );
    }

    /// The other direction, without which the walk above could be refusing everything.
    #[test]
    fn a_message_whose_nested_bound_holds_reaches_the_implementation() {
        let service = GateBackEnd::new();
        let reply = GateReply::new();
        poll_once(gate_service_schema::dispatch(
            &service,
            &(),
            &gate_service_schema::IncomingMessage {
                operation: "mint".to_owned(),
                payload: br#"{"account":{"aud":"acme","jti":"a"},"creditCount":2}"#.to_vec(),
            },
            &reply,
        ))
        .unwrap();
        assert_eq!(service.reached(), vec!["minted 2 for acme".to_owned()]);
        assert!(reply.faults().is_empty(), "got: {:?}", reply.settled());
    }

    #[test]
    fn a_refusal_written_in_a_validator_s_words_still_names_the_field_it_refused() {
        let service = GateBackEnd::new();
        let reply = GateReply::new();
        poll_once(gate_service_schema::dispatch(
            &service,
            &(),
            &gate_service_schema::IncomingMessage {
                operation: "open-ledger".to_owned(),
                payload: br#"{"ledger_id":"ab"}"#.to_vec(),
            },
            &reply,
        ))
        .unwrap();
        assert!(service.reached().is_empty(), "got: {:?}", service.reached());
        let reported = reply.faults();
        assert_eq!(
            reported[0].kind(),
            gate_service_schema::ServiceFaultKind::FailedValidation,
            "the author put this check on the read, but what it refused is a value out of range \
             inside a document that read perfectly well — the class `serde_json` reports it under, \
             and the one a caller acts on. Got detail: {}",
            reported[0].detail()
        );
        assert_eq!(
            reported[0].field(),
            Some("ledger_id"),
            "a refusal written in the shape a validator reports in names its field, and the \
             fault reads the name back off it wherever the refusal came from. Got: {}",
            reported[0].detail()
        );
    }
}

use core::cell::RefCell;
use core::fmt::{self, Debug, Display, Write as _};
use core::future::{Future, ready};
use core::pin::pin;
use core::task::{Context as PollContext, Poll, Waker};
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, Once};
use tixschema::{model_schema, service_schema};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::subscriber::set_global_default;
use tracing::{Event, Metadata, Subscriber};

thread_local! {
    /// What the dispatcher wrote down on *this* thread.
    ///
    /// A subscriber installed per test would not do. `tracing` caches each callsite's interest
    /// globally and recomputes it against the set of dispatchers currently registered, so a
    /// callsite first reached while no subscriber existed caches as never-interested, and a
    /// dispatcher registered and dropped by a neighbouring test moves the answer under a test
    /// that is mid-dispatch. One subscriber for the whole binary is registered once and never
    /// dropped, which leaves the interest settled; libtest gives each test its own thread, and
    /// that is what keeps one test's records out of another's.
    static WRITTEN: RefCell<Vec<Recorded>> = const { RefCell::new(Vec::new()) };
}

/// A message that publishes a validator of its own, written by hand rather than annotated, so
/// that what the arm does with a violation is read off the arm rather than off the serde hook
/// `#[model_schema_prop]` writes. An inherent `validate()` is exactly what `#[model_schema()]`
/// publishes for a constrained type, and exactly what the arm calls.
#[derive(Deserialize, Serialize)]
pub struct AdmitRequest {
    pub organization_id: String,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
pub struct BalanceRequest {
    pub organization_id: String,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
pub struct BalanceResponse {
    pub credits: u32,
}

impl AdmitRequest {
    /// The same report shape a generated validator writes: the field first and in single quotes,
    /// which is where the fault reads the name it carries.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        if self.organization_id.len() < 3 {
            return Err(vec![format!(
                "'organization_id' is too short: minimum length is 3, got {}",
                self.organization_id.len()
            )]);
        }
        Ok(())
    }
}

/// Writes down every operation that reached it, so a test can say what the dispatcher let through.
pub struct ProbeBackEnd {
    reached: Mutex<Vec<String>>,
}

#[model_schema()]
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum ProbeError {
    DbError,
}

/// The handle a transport would give the dispatcher, recording how each message was settled
/// instead of publishing anything.
pub struct ProbeReply {
    settled: Mutex<Vec<Settled>>,
}

/// One of the two ways an arm answers. Exactly one lands per request-and-reply dispatch, and none
/// at all where a one-way operation reached its implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Settled {
    Fault(probe_service_schema::ServiceFault),
    Sent(String),
}

#[service_schema()]
pub trait ProbeService<Ctx> {
    /// A message that validates itself, and an arm that runs that validator before entering here.
    async fn admit(&self, ctx: &Ctx, req: AdmitRequest) -> Result<BalanceResponse, ProbeError>;

    /// No reply: the arm still has to settle the delivery.
    #[service_schema_op(one_way)]
    async fn apply_bundle(&self, ctx: &Ctx, req: BalanceRequest);

    /// An implementation that comes apart rather than answering. Nothing the operation declared
    /// covers it, so what the arm does with it is the fault.
    async fn collapse(&self, ctx: &Ctx, req: BalanceRequest)
    -> Result<BalanceResponse, ProbeError>;

    /// The same, on an arm that declared no reply.
    #[service_schema_op(one_way)]
    async fn discard(&self, ctx: &Ctx, req: BalanceRequest);

    /// Several arguments after the context: the message is unpacked back into them.
    async fn expire_credit(
        &self,
        ctx: &Ctx,
        organization_id: String,
        credit_id: String,
    ) -> Result<BalanceResponse, ProbeError>;

    /// One argument, which already is the message.
    async fn get_balance(
        &self,
        ctx: &Ctx,
        req: BalanceRequest,
    ) -> Result<BalanceResponse, ProbeError>;

    /// None at all, so the message declared for it is empty.
    async fn sweep(&self, ctx: &Ctx) -> Result<BalanceResponse, ProbeError>;
}

impl ProbeService<String> for ProbeBackEnd {
    async fn admit(&self, _ctx: &String, req: AdmitRequest) -> Result<BalanceResponse, ProbeError> {
        ready(()).await;
        self.reach(format!("admit {}", req.organization_id));
        Ok(BalanceResponse { credits: 3 })
    }

    async fn apply_bundle(&self, ctx: &String, req: BalanceRequest) {
        let _read = ready(ctx.len()).await;
        self.reach(format!("apply_bundle {}", req.organization_id));
    }

    async fn collapse(
        &self,
        _ctx: &String,
        req: BalanceRequest,
    ) -> Result<BalanceResponse, ProbeError> {
        ready(()).await;
        self.reach(format!("collapse {}", req.organization_id));
        come_apart(req.organization_id == "formatted", &req.organization_id);
        Ok(BalanceResponse { credits: 0 })
    }

    async fn discard(&self, _ctx: &String, req: BalanceRequest) {
        ready(()).await;
        self.reach(format!("discard {}", req.organization_id));
        come_apart(false, &req.organization_id);
    }

    async fn expire_credit(
        &self,
        _ctx: &String,
        organization_id: String,
        credit_id: String,
    ) -> Result<BalanceResponse, ProbeError> {
        ready(()).await;
        self.reach(format!("expire_credit {organization_id} {credit_id}"));
        Ok(BalanceResponse { credits: 1 })
    }

    async fn get_balance(
        &self,
        _ctx: &String,
        req: BalanceRequest,
    ) -> Result<BalanceResponse, ProbeError> {
        ready(()).await;
        self.reach(format!("get_balance {}", req.organization_id));
        if req.organization_id == "unlucky" {
            Err(ProbeError::DbError)
        } else {
            Ok(BalanceResponse { credits: 7 })
        }
    }

    async fn sweep(&self, _ctx: &String) -> Result<BalanceResponse, ProbeError> {
        ready(()).await;
        self.reach("sweep".to_owned());
        Ok(BalanceResponse { credits: 0 })
    }
}

impl probe_service_schema::Reply for ProbeReply {
    async fn fault(&self, fault: probe_service_schema::ServiceFault) {
        ready(()).await;
        self.record(Settled::Fault(fault));
    }

    async fn send<T>(&self, value: T)
    where
        T: Serialize + Send,
    {
        ready(()).await;
        self.record(Settled::Sent(serde_json::to_string(&value).unwrap()));
    }
}

impl ProbeBackEnd {
    fn new() -> Self {
        Self {
            reached: Mutex::new(Vec::new()),
        }
    }

    fn reach(&self, what: String) {
        self.reached.lock().unwrap().push(what);
    }

    fn reached(&self) -> Vec<String> {
        self.reached.lock().unwrap().clone()
    }
}

impl ProbeReply {
    fn new() -> Self {
        Self {
            settled: Mutex::new(Vec::new()),
        }
    }

    fn record(&self, what: Settled) {
        self.settled.lock().unwrap().push(what);
    }

    fn settled(&self) -> Vec<Settled> {
        self.settled.lock().unwrap().clone()
    }
}

/// One event as it reached the subscriber, so a test says what was written down rather than that
/// something was written somewhere.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Recorded {
    detail: String,
    level: String,
    message: String,
    operation: String,
}

/// The fields off one event, read by name. A field the event did not carry stays empty, which is
/// itself something a test can assert.
struct ReadFields {
    detail: String,
    message: String,
    operation: String,
}

/// Stands in for the subscriber a service would really install, and files every event it is
/// handed under the thread that produced it.
struct Recorder;

/// A field's value under the one rendering `Visit` offers for it.
///
/// `Visit::record_debug` hands over a `&dyn Debug` and nothing else — the event's message arrives
/// that way, as the `format_args!` the macro built — so the `Debug` rendering *is* the value here
/// rather than a stand-in for a `Display` that exists. This says so in the type instead of writing
/// `{:?}` at a call site that reads like a slip.
struct Shown<'reading>(&'reading dyn Debug);

impl Display for Shown<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(self.0, f)
    }
}

impl Subscriber for Recorder {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn enter(&self, _span: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut read = ReadFields::new();
        event.record(&mut read);
        let written = Recorded {
            detail: read.detail,
            level: event.metadata().level().to_string(),
            message: read.message,
            operation: read.operation,
        };
        WRITTEN.with_borrow_mut(|events| events.push(written));
    }

    fn exit(&self, _span: &Id) {}

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}
}

impl Visit for ReadFields {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        let mut rendered = String::new();
        write!(rendered, "{}", Shown(value)).unwrap();
        self.put(field.name(), rendered);
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.put(field.name(), value.to_owned());
    }
}

impl ReadFields {
    fn new() -> Self {
        Self {
            detail: String::new(),
            message: String::new(),
            operation: String::new(),
        }
    }

    fn put(&mut self, named: &str, value: String) {
        match named {
            "detail" => self.detail = value,
            "message" => self.message = value,
            "operation" => self.operation = value,
            _ => {}
        }
    }
}

/// Comes apart the way a handler does when something the compiler was expected to prevent gets
/// through.
///
/// The two conditions are exact negations, so one of them always fires. Which one decides the shape
/// of the panic payload: `assert!` panics with exactly the message it was given and nothing around
/// it, so a formatted message reaches the panic hook as a `String` and a literal one as a `&str`.
/// A fault's detail has to read back off either, and a reader that knew only one shape would report
/// nothing for half the panics a service can raise.
fn come_apart(formatted: bool, organization_id: &str) {
    assert!(
        !formatted,
        "the ledger for {organization_id} is not a ledger"
    );
    assert!(formatted, "the ledger is not a ledger");
}

/// Dispatches one message and answers with what the service saw and how the message was settled.
fn dispatched(operation: &str, payload: &str) -> (Vec<String>, Vec<Settled>) {
    let service = ProbeBackEnd::new();
    let reply = ProbeReply::new();
    let ctx = "probe".to_owned();
    poll_once(probe_service_schema::dispatch(
        &service,
        &ctx,
        &probe_service_schema::IncomingMessage {
            operation: operation.to_owned(),
            payload: payload.as_bytes().to_vec(),
        },
        &reply,
    ))
    .unwrap();
    (service.reached(), reply.settled())
}

/// The one fault a settlement list holds, or nothing when it holds something else.
fn only_fault(settled: &[Settled]) -> Option<&probe_service_schema::ServiceFault> {
    match settled {
        [Settled::Fault(reported)] => Some(reported),
        _ => None,
    }
}

/// Dispatches one message with a subscriber of our own in place, and answers with both accounts of
/// it: what the caller was told, and what the operator's records hold.
fn recorded(operation: &str, payload: &str) -> (Vec<Settled>, Vec<Recorded>) {
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| set_global_default(Recorder).unwrap());
    WRITTEN.with_borrow_mut(Vec::clear);
    let (_reached, settled) = dispatched(operation, payload);
    (settled, WRITTEN.with_borrow(Clone::clone))
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

#[test]
fn an_operation_that_names_its_own_message_is_called_with_it_and_answers_through_send() {
    let (reached, settled) = dispatched("get-balance", r#"{"organization_id":"acme"}"#);
    assert_eq!(reached, vec!["get_balance acme".to_owned()]);
    assert_eq!(
        settled,
        vec![Settled::Sent(
            r#"{"ok":true,"value":{"credits":7}}"#.to_owned()
        )],
        "the answer rides in the envelope both languages read"
    );
}

#[test]
fn the_error_an_operation_declared_rides_in_the_failure_arm_rather_than_becoming_a_fault() {
    let (reached, settled) = dispatched("get-balance", r#"{"organization_id":"unlucky"}"#);
    assert_eq!(reached, vec!["get_balance unlucky".to_owned()]);
    assert_eq!(
        settled,
        vec![Settled::Sent(
            r#"{"error":{"errorCode":"db-error"},"ok":false}"#.to_owned()
        )],
        "an operation's own error is a condition it declared, not a defect"
    );
}

#[test]
fn a_message_the_macro_declared_is_unpacked_back_into_the_arguments_it_was_declared_from() {
    let (reached, settled) = dispatched(
        "expire-credit",
        r#"{"organizationId":"acme","creditId":"cr-1"}"#,
    );
    assert_eq!(
        reached,
        vec!["expire_credit acme cr-1".to_owned()],
        "the packing is the macro's job and the implementation still takes its arguments"
    );
    assert_eq!(settled.len(), 1, "got: {settled:?}");
}

#[test]
fn an_operation_that_takes_nothing_is_still_dispatched_from_a_payload() {
    let (reached, settled) = dispatched("sweep", "{}");
    assert_eq!(reached, vec!["sweep".to_owned()]);
    assert_eq!(settled.len(), 1, "got: {settled:?}");
}

#[test]
fn a_one_way_operation_runs_and_answers_nothing_on_the_handle() {
    let (reached, settled) = dispatched("apply-bundle", r#"{"organization_id":"acme"}"#);
    assert_eq!(reached, vec!["apply_bundle acme".to_owned()]);
    assert!(
        settled.is_empty(),
        "nothing about replying belongs on a path that never replies; the transport adapter \
         acknowledges the delivery after `dispatch` returns. Got: {settled:?}"
    );
}

#[test]
fn a_payload_carrying_the_wrong_type_of_value_becomes_a_fault_and_reaches_no_implementation() {
    let (reached, settled) = dispatched("get-balance", r#"{"organization_id":42}"#);
    assert!(reached.is_empty(), "got: {reached:?}");
    assert_eq!(settled.len(), 1, "got: {settled:?}");
    let reported = only_fault(&settled).unwrap();
    assert_eq!(
        reported.kind(),
        probe_service_schema::ServiceFaultKind::FailedValidation,
        "the bytes read as a document and did not match the message, which is a value someone \
         supplied rather than a sender whose serialization is broken — and is the kind the \
         TypeScript service serving the same operation answers it under. Got detail: {}",
        reported.detail()
    );
    assert_eq!(reported.operation(), "get-balance");
    assert_eq!(
        reported.field(),
        None,
        "a type mismatch says what serde expected and not where, so there is no name to carry and \
         one carried anyway would be invented. Got: {}",
        reported.detail()
    );
    assert!(
        !reported.detail().contains("at line"),
        "the byte offset locates the failure inside an encoding the caller never saw. Got: {}",
        reported.detail()
    );
}

#[test]
fn an_operation_name_nothing_answers_to_becomes_a_fault_through_the_same_handle() {
    let (reached, settled) = dispatched("get-the-balance", r#"{"organization_id":"acme"}"#);
    assert!(reached.is_empty(), "got: {reached:?}");
    assert_eq!(settled.len(), 1, "got: {settled:?}");
    let reported = only_fault(&settled).unwrap();
    assert_eq!(
        reported.kind(),
        probe_service_schema::ServiceFaultKind::UnknownOperation
    );
    assert_eq!(
        reported.operation(),
        "get-the-balance",
        "the fault names what arrived, that being the only thing known about it"
    );
}

#[test]
fn the_operation_is_read_from_the_name_it_was_given_and_never_out_of_the_payload() {
    // The payload says one thing and the transport says another; the transport wins, because the
    // operation travels beside the payload rather than inside it.
    let (reached, settled) = dispatched(
        "sweep",
        r#"{"operation":"get-balance","type":"get-balance"}"#,
    );
    assert_eq!(
        reached,
        vec!["sweep".to_owned()],
        "a key inside the payload is the message's own business and routes nothing"
    );
    assert_eq!(settled.len(), 1, "got: {settled:?}");
}

#[test]
fn a_message_that_passes_its_own_validator_reaches_the_implementation() {
    let (reached, settled) = dispatched("admit", r#"{"organization_id":"acme"}"#);
    assert_eq!(
        reached,
        vec!["admit acme".to_owned()],
        "a valid message is exactly the one the implementation is meant to see"
    );
    assert_eq!(
        settled,
        vec![Settled::Sent(
            r#"{"ok":true,"value":{"credits":3}}"#.to_owned()
        )]
    );
}

#[test]
fn a_message_that_fails_its_own_validator_never_reaches_it_and_the_fault_names_the_field() {
    let (reached, settled) = dispatched("admit", r#"{"organization_id":"ab"}"#);
    assert!(
        reached.is_empty(),
        "an implementation may assume its incoming message is valid, which only holds if an \
         invalid one is stopped in the arm. Got: {reached:?}"
    );
    assert_eq!(settled.len(), 1, "got: {settled:?}");
    let reported = only_fault(&settled).unwrap();
    assert_eq!(
        reported.kind(),
        probe_service_schema::ServiceFaultKind::FailedValidation
    );
    assert_eq!(
        reported.field(),
        Some("organization_id"),
        "the caller has to be told which field it got wrong"
    );
    assert_eq!(reported.operation(), "admit");
    assert!(
        reported.detail().contains("is too short"),
        "got: {}",
        reported.detail()
    );
}

#[test]
fn a_handler_that_panics_becomes_a_fault_rather_than_unwinding_out_of_dispatch() {
    let (reached, settled) = dispatched("collapse", r#"{"organization_id":"acme"}"#);
    assert_eq!(
        reached,
        vec!["collapse acme".to_owned()],
        "the message was valid and the implementation was entered; what it did after that is the \
         defect this reports"
    );
    assert_eq!(settled.len(), 1, "got: {settled:?}");
    let reported = only_fault(&settled).unwrap();
    assert_eq!(
        reported.kind(),
        probe_service_schema::ServiceFaultKind::HandlerPanic,
        "a panic is a failure the operation never declared, which is what the kind is for"
    );
    assert_eq!(reported.operation(), "collapse");
    assert_eq!(reported.field(), None, "a panic names no field");
    assert_eq!(
        reported.detail(),
        "the ledger is not a ledger",
        "a literal panic message arrives as a `&str`, and the detail is what a receiver has to \
         page a human with"
    );
}

#[test]
fn dispatch_returns_after_a_handler_panics_so_the_transport_can_still_settle_the_delivery() {
    let service = ProbeBackEnd::new();
    let reply = ProbeReply::new();
    let ctx = "probe".to_owned();
    let returned = poll_once(probe_service_schema::dispatch(
        &service,
        &ctx,
        &probe_service_schema::IncomingMessage {
            operation: "collapse".to_owned(),
            payload: br#"{"organization_id":"acme"}"#.to_vec(),
        },
        &reply,
    ));
    assert_eq!(
        returned,
        Some(()),
        "the transport acknowledges after `dispatch` returns, so a panic that unwound past it \
         would never be acknowledged at all. There is no `nack` on the bus this was measured \
         against, no dead-letter exchange, no message TTL and no timeout, so that delivery would \
         sit outstanding against the prefetch until the channel closed."
    );
}

#[test]
fn a_formatted_panic_message_reaches_the_fault_as_the_message_rather_than_as_its_shape() {
    let (_reached, settled) = dispatched("collapse", r#"{"organization_id":"formatted"}"#);
    let reported = only_fault(&settled).unwrap();
    assert_eq!(
        reported.kind(),
        probe_service_schema::ServiceFaultKind::HandlerPanic
    );
    assert_eq!(
        reported.detail(),
        "the ledger for formatted is not a ledger",
        "a formatted panic message arrives as a `String`, and a fault that could only read a \
         `&str` would report nothing for the half of panics that carry one"
    );
}

#[test]
fn a_one_way_handler_that_panics_still_answers_nothing_and_still_lets_dispatch_return() {
    let (reached, settled) = dispatched("discard", r#"{"organization_id":"acme"}"#);
    assert_eq!(
        reached,
        vec!["discard acme".to_owned()],
        "the implementation was entered, which is what makes this the one-way case rather than a \
         refusal before it"
    );
    assert!(
        settled.is_empty(),
        "a one-way arm has answered once its implementation was entered, a panic included: the \
         operation declared no reply and the delivery carries no queue for one to go to. What the \
         guard buys here is the return itself, which is what the transport settles on. Got: \
         {settled:?}"
    );
}

#[test]
fn a_one_way_handler_that_panics_is_written_down_even_though_nobody_is_answered() {
    let (settled, written) = recorded("discard", r#"{"organization_id":"acme"}"#);
    assert!(
        settled.is_empty(),
        "the operation declared no reply, which is exactly why the record is the only account \
         there is. Got: {settled:?}"
    );
    assert_eq!(
        written,
        vec![Recorded {
            detail: "the ledger is not a ledger".to_owned(),
            level: "ERROR".to_owned(),
            message: "the handler for this operation panicked".to_owned(),
            operation: "discard".to_owned(),
        }],
        "catching a panic so the transport can settle the delivery must not be the same as losing \
         it. The record names the operation, because the panic hook's own line does not."
    );
}

#[test]
fn a_request_and_reply_panic_is_written_down_as_well_as_answered() {
    let (settled, written) = recorded("collapse", r#"{"organization_id":"acme"}"#);
    let reported = only_fault(&settled).unwrap();
    assert_eq!(
        reported.kind(),
        probe_service_schema::ServiceFaultKind::HandlerPanic
    );
    assert_eq!(
        written,
        vec![Recorded {
            detail: "the ledger is not a ledger".to_owned(),
            level: "ERROR".to_owned(),
            message: "the handler for this operation panicked".to_owned(),
            operation: "collapse".to_owned(),
        }],
        "the fault answers the caller and the record answers the operator, and the two are \
         frequently not the same party. A panic is a defect in this service whichever way its \
         operation was declared, so both outcomes write the same event."
    );
    assert_eq!(
        written[0].detail,
        reported.detail(),
        "one panic, one account of what it said, so a record and a fault cannot disagree about it"
    );
}

#[test]
fn nothing_but_a_panic_is_written_down() {
    // Every other path through an arm, including the two that fault: a fault is a defect the
    // caller is told about, and only a panic is one the caller may never hear of at all.
    for (operation, payload) in [
        ("get-balance", r#"{"organization_id":"acme"}"#),
        ("get-balance", r#"{"organization_id":"unlucky"}"#),
        ("get-balance", r#"{"organization_id":42}"#),
        ("get-the-balance", "{}"),
        ("admit", r#"{"organization_id":"ab"}"#),
        ("apply-bundle", r#"{"organization_id":"acme"}"#),
        ("sweep", "{}"),
    ] {
        let (_settled, written) = recorded(operation, payload);
        assert!(
            written.is_empty(),
            "`{operation}` on `{payload}` wrote a record. An arm that logged every message it \
             settled would bury the one event that means a handler died. Got: {written:?}"
        );
    }
}

#[test]
fn every_arm_answers_exactly_the_number_of_times_its_outcome_allows() {
    // Every arm the service has, on every path through it: the answer, the operation's own error,
    // a message its validator refuses, bytes that were never the message at all, a name nothing
    // answers to, and an implementation that came apart. The last field is how many times the
    // handle may be reached — once for a request-and-reply arm however it goes, and for a one-way
    // arm only where the message was refused before the implementation ever ran.
    for (operation, payload, answers) in [
        ("admit", r#"{"organization_id":"acme"}"#, 1),
        ("admit", r#"{"organization_id":"ab"}"#, 1),
        ("apply-bundle", r#"{"organization_id":"acme"}"#, 0),
        ("apply-bundle", r#"{"organization_id":42}"#, 1),
        ("collapse", r#"{"organization_id":"acme"}"#, 1),
        ("discard", r#"{"organization_id":"acme"}"#, 0),
        ("discard", r#"{"organization_id":42}"#, 1),
        (
            "expire-credit",
            r#"{"organizationId":"acme","creditId":"cr-1"}"#,
            1,
        ),
        ("get-balance", r#"{"organization_id":"acme"}"#, 1),
        ("get-balance", r#"{"organization_id":"unlucky"}"#, 1),
        ("get-balance", r#"{"organization_id":42}"#, 1),
        ("get-the-balance", "{}", 1),
        ("sweep", "{}", 1),
        ("sweep", "not a document at all", 1),
    ] {
        let (_reached, settled) = dispatched(operation, payload);
        assert_eq!(
            settled.len(),
            answers,
            "`{operation}` on `{payload}` reached the handle {} times. Answering twice answers a \
             message that was already answered, and answering a message the operation declared no \
             reply for puts a reply on a queue nothing is reading. Got: {settled:?}",
            settled.len()
        );
        if operation == "apply-bundle" || operation == "discard" {
            assert!(
                !settled.iter().any(|what| matches!(*what, Settled::Sent(_))),
                "a one-way arm publishes nothing on any path, whether the message reached the \
                 implementation, came apart inside it, or was refused before it. Got: {settled:?}"
            );
        }
    }
}

#[test]
fn a_one_way_message_refused_before_it_ran_is_the_one_thing_that_arm_answers() {
    let (never_reached, refused) = dispatched("apply-bundle", r#"{"organization_id":42}"#);
    assert!(never_reached.is_empty(), "got: {never_reached:?}");
    let reported = only_fault(&refused).unwrap();
    assert_eq!(
        reported.kind(),
        probe_service_schema::ServiceFaultKind::FailedValidation,
        "the operation never ran, so the defect is the arm's to report even though the operation \
         itself declares no reply"
    );
    assert_eq!(reported.operation(), "apply-bundle");
}
