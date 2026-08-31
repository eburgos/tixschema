//! One service covering every input shape and both outcomes, dispatched over payloads that take
//! every path through an arm: the answer, the operation's own error, an undeserializable payload,
//! a name nothing answers to, a handler that panicked, and a one-way operation that settles
//! without publishing.
//!
//! The probe reply handle records which of `send` and `fault` was called, so a test can assert not
//! only what was answered but that a request-and-reply arm answered exactly once and that a
//! one-way arm that reached its implementation answered nothing at all.

#![cfg(feature = "serde")]

/// A message annotated with `#[model_schema_prop]`, and where a violation of it is actually
/// caught.
///
/// Gated because the annotation is: the constraint is read, and the deserializer that enforces it
/// written, only when `serde` is on beside a surface that reads the constraint. What this module
/// records is that serde's own hook rejects the payload *before* it becomes a message at all, so
/// the arm never reaches its `validate()` and the fault reports an undeserializable payload. The
/// implementation is not reached either way, which is the guarantee that matters; the arm's own
/// validator is read outside this module, over a message that validates itself.
///
/// The kind differs between the two paths and the field does not. A constrained field's serde hook
/// runs the very check `validate()` runs and hands serde that check's message verbatim, so the
/// refusal names the field in the same words a violation report does and the fault reads the name
/// back off either. That is what makes "a payload failing validation produces a fault naming the
/// field" true on the path a service written with annotations actually takes — which is every
/// service, the annotation being how a constraint is declared.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
mod a_message_annotated_with_a_constraint {
    use super::poll_once;
    use core::future::ready;
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
    }

    impl GateService<()> for GateBackEnd {
        async fn admit(&self, _ctx: &(), req: GateRequest) -> Result<Admitted, GateError> {
            ready(()).await;
            self.reached.lock().unwrap().push(req.organization_id);
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

    #[test]
    fn a_payload_violating_it_is_refused_before_it_ever_becomes_a_message() {
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
            "an implementation may assume its incoming message is valid, and this one never \
             deserialized. Got: {:?}",
            service.reached()
        );
        assert_eq!(reply.settled().len(), 1, "got: {:?}", reply.settled());
        let reported = reply.faults();
        assert_eq!(
            reported[0].kind(),
            gate_service_schema::ServiceFaultKind::UndeserializablePayload,
            "the annotation writes a serde hook, which refuses the bytes before the arm can run \
             the message's own validator"
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
            "the caller has to be told which field it got wrong, and which of the two guards \
             caught it is not the caller's business. The hook serde ran is the constraint's own \
             check and its message is that check's, so the name is there to be read. Got: {}",
            reported[0].detail()
        );
    }

    #[test]
    fn a_payload_wrong_about_the_field_s_type_rather_than_its_value_names_no_field() {
        let service = GateBackEnd::new();
        let reply = GateReply::new();
        poll_once(gate_service_schema::dispatch(
            &service,
            &(),
            &gate_service_schema::IncomingMessage {
                operation: "admit".to_owned(),
                payload: br#"{"organization_id":42}"#.to_vec(),
            },
            &reply,
        ))
        .unwrap();
        assert!(service.reached().is_empty(), "got: {:?}", service.reached());
        let reported = reply.faults();
        assert_eq!(
            reported[0].kind(),
            gate_service_schema::ServiceFaultKind::UndeserializablePayload
        );
        assert_eq!(
            reported[0].field(),
            None,
            "serde refused the shape rather than the value, so no constraint's check ran and \
             there is no field name to carry. A fault that named one here would be naming it out \
             of a message it never read. Got: {}",
            reported[0].detail()
        );
    }
}

use core::future::{Future, ready};
use core::pin::pin;
use core::task::{Context as PollContext, Poll, Waker};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tixschema::{model_schema, service_schema};

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
fn a_payload_that_will_not_deserialize_becomes_a_fault_and_reaches_no_implementation() {
    let (reached, settled) = dispatched("get-balance", r#"{"organization_id":42}"#);
    assert!(reached.is_empty(), "got: {reached:?}");
    assert_eq!(settled.len(), 1, "got: {settled:?}");
    let reported = only_fault(&settled).unwrap();
    assert_eq!(
        reported.kind(),
        probe_service_schema::ServiceFaultKind::UndeserializablePayload
    );
    assert_eq!(reported.operation(), "get-balance");
    assert_eq!(reported.field(), None);
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
        probe_service_schema::ServiceFaultKind::UndeserializablePayload,
        "the operation never ran, so the defect is the arm's to report even though the operation \
         itself declares no reply"
    );
    assert_eq!(reported.operation(), "apply-bundle");
}
