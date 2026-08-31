//! One service, one generated client, and two transports written by hand.
//!
//! `ProbeTransport` hands out prepared answers and writes down what it was asked to send, which is
//! how a test reads the operation name travelling beside the payload rather than inside it. Built
//! with no answers it panics the moment either method is reached, which is how the
//! outbound-validation tests prove the transport was never touched.
//!
//! `Loopback` sends the message straight into the generated dispatcher and hands back what the
//! reply handle captured. It is the only place both halves of the seam meet, and it is where the
//! envelope one writes and the other reads is proven to be one envelope.
//!
//! `DeadlineTransport` is the bus this design has to live on when a reply does not come: it records
//! the call, waits out a deadline it imposed itself, and reports that nothing landed. It is the
//! only transport here that answers in the failure arm, and it is what the fault a caller reads
//! for a call that never completed is measured against.

#![cfg(feature = "serde")]

use core::future::{Future, ready};
use core::pin::pin;
use core::task::{Context as PollContext, Poll, Waker};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tixschema::{model_schema, service_schema};

/// A message that publishes a validator of its own, so the client's outbound check has something
/// to refuse. The report shape is the one a generated validator writes: the field first and in
/// single quotes, which is where the fault reads the name it carries.
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
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BalanceResponse {
    pub credits: u32,
}

/// What a reply handle captured, encoded as the transport would put it on the wire.
pub struct Capture {
    answered: Mutex<Vec<Vec<u8>>>,
}

/// A transport that imposed a deadline of its own and hit it: the call went out and no reply came
/// back. It records what it was asked to carry, so a test can tell a call that never landed from
/// one that was never made.
pub struct DeadlineTransport {
    calls: Mutex<Vec<String>>,
}

/// A transport that answers by dispatching into the service itself.
pub struct Loopback {
    service: ProbeBackEnd,
}

/// Writes down every operation that reached the far side.
pub struct ProbeBackEnd {
    reached: Mutex<Vec<String>>,
}

#[model_schema()]
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum ProbeError {
    DbError,
}

/// A transport that hands out prepared answers, one per call, and records what it was asked to
/// send. Every method takes an answer off the list, so a probe built with none panics the moment
/// it is reached at all.
pub struct ProbeTransport {
    answers: Mutex<Vec<Vec<u8>>>,
    calls: Mutex<Vec<(String, String)>>,
}

#[service_schema()]
pub trait ProbeService<Ctx> {
    /// A message that validates itself, which is what the client checks before it sends.
    async fn admit(&self, ctx: &Ctx, req: AdmitRequest) -> Result<BalanceResponse, ProbeError>;

    /// No reply, so nothing to await beyond the send.
    #[service_schema_op(one_way)]
    async fn apply_bundle(&self, ctx: &Ctx, req: AdmitRequest);

    /// Several arguments after the context: the caller still passes them separately.
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

impl AdmitRequest {
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

impl ProbeService<String> for ProbeBackEnd {
    async fn admit(&self, _ctx: &String, req: AdmitRequest) -> Result<BalanceResponse, ProbeError> {
        ready(()).await;
        self.reach(format!("admit {}", req.organization_id));
        Ok(BalanceResponse { credits: 3 })
    }

    async fn apply_bundle(&self, ctx: &String, req: AdmitRequest) {
        let _read = ready(ctx.len()).await;
        self.reach(format!("apply_bundle {}", req.organization_id));
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

impl probe_service_schema::Reply for Capture {
    async fn fault(&self, fault: probe_service_schema::ServiceFault) {
        ready(()).await;
        // The framing is the transport's business: a fault rides tagged inside the failure arm,
        // which is the shape a caller in either language narrows on.
        let framed = serde_json::json!({
            "ok": false,
            "error": { "isServiceFault": true, "fault": fault },
        });
        self.answered
            .lock()
            .unwrap()
            .push(serde_json::to_vec(&framed).unwrap());
    }

    async fn send<T>(&self, value: T)
    where
        T: Serialize + Send,
    {
        ready(()).await;
        self.answered
            .lock()
            .unwrap()
            .push(serde_json::to_vec(&value).unwrap());
    }
}

impl probe_service_schema::Transport for DeadlineTransport {
    async fn notify<T>(&self, operation: &str, _payload: T) -> Result<(), String>
    where
        T: Serialize + Send,
    {
        ready(()).await;
        self.record(operation);
        Err("no confirmation within 30s".to_owned())
    }

    async fn request<T>(&self, operation: &str, _payload: T) -> Result<Vec<u8>, String>
    where
        T: Serialize + Send,
    {
        ready(()).await;
        self.record(operation);
        Err("no reply within 30s".to_owned())
    }
}

impl probe_service_schema::Transport for Loopback {
    async fn notify<T>(&self, operation: &str, payload: T) -> Result<(), String>
    where
        T: Serialize + Send,
    {
        let capture = Capture::new();
        probe_service_schema::dispatch(
            &self.service,
            &"probe".to_owned(),
            &incoming(operation, &payload),
            &capture,
        )
        .await;
        Ok(())
    }

    async fn request<T>(&self, operation: &str, payload: T) -> Result<Vec<u8>, String>
    where
        T: Serialize + Send,
    {
        let capture = Capture::new();
        probe_service_schema::dispatch(
            &self.service,
            &"probe".to_owned(),
            &incoming(operation, &payload),
            &capture,
        )
        .await;
        Ok(capture.answered())
    }
}

impl probe_service_schema::Transport for ProbeTransport {
    async fn notify<T>(&self, operation: &str, payload: T) -> Result<(), String>
    where
        T: Serialize + Send,
    {
        ready(()).await;
        self.record(operation, &payload);
        self.answer();
        Ok(())
    }

    async fn request<T>(&self, operation: &str, payload: T) -> Result<Vec<u8>, String>
    where
        T: Serialize + Send,
    {
        ready(()).await;
        self.record(operation, &payload);
        Ok(self.answer())
    }
}

impl Capture {
    fn answered(&self) -> Vec<u8> {
        self.answered.lock().unwrap().pop().unwrap()
    }

    fn new() -> Self {
        Self {
            answered: Mutex::new(Vec::new()),
        }
    }
}

impl DeadlineTransport {
    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    const fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
        }
    }

    fn record(&self, operation: &str) {
        self.calls.lock().unwrap().push(operation.to_owned());
    }
}

impl Loopback {
    fn new() -> Self {
        Self {
            service: ProbeBackEnd::new(),
        }
    }

    fn reached(&self) -> Vec<String> {
        self.service.reached()
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

impl ProbeTransport {
    /// The next prepared answer. A transport built with none never returns from here, which is the
    /// failure a test that reaches the transport is meant to report.
    fn answer(&self) -> Vec<u8> {
        self.answers.lock().unwrap().pop().unwrap()
    }

    fn calls(&self) -> Vec<(String, String)> {
        self.calls.lock().unwrap().clone()
    }

    /// The answers this transport will give, in the order it will give them.
    fn new(answers: &[&str]) -> Self {
        Self {
            answers: Mutex::new(
                answers
                    .iter()
                    .rev()
                    .map(|answer| answer.as_bytes().to_vec())
                    .collect(),
            ),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn record<T>(&self, operation: &str, payload: &T)
    where
        T: Serialize,
    {
        self.calls.lock().unwrap().push((
            operation.to_owned(),
            serde_json::to_string(payload).unwrap(),
        ));
    }
}

/// One message as the dispatcher on the far side reads it.
fn incoming<T>(operation: &str, payload: &T) -> probe_service_schema::IncomingMessage
where
    T: Serialize,
{
    probe_service_schema::IncomingMessage {
        operation: operation.to_owned(),
        payload: serde_json::to_vec(payload).unwrap(),
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

/// The fault a call error carries, or nothing when it carries the operation's own error.
fn reported_fault(
    answered: &Result<BalanceResponse, probe_service_schema::CallError<ProbeError>>,
) -> Option<&probe_service_schema::ServiceFault> {
    match answered {
        Err(probe_service_schema::CallError::Fault(reported)) => Some(reported),
        Ok(_) | Err(probe_service_schema::CallError::Operation(_)) => None,
    }
}

#[test]
fn an_answer_becomes_the_success_type_the_operation_declared() {
    let transport = ProbeTransport::new(&[r#"{"ok":true,"value":{"credits":7}}"#]);
    let client = probe_service_schema::ProbeServiceClient::new(transport);
    let answered = poll_once(client.get_balance(BalanceRequest {
        organization_id: "acme".to_owned(),
    }))
    .unwrap();
    assert_eq!(answered, Ok(BalanceResponse { credits: 7 }));
}

#[test]
fn the_error_the_operation_declared_comes_back_in_the_operation_arm() {
    let transport = ProbeTransport::new(&[r#"{"ok":false,"error":{"errorCode":"db-error"}}"#]);
    let client = probe_service_schema::ProbeServiceClient::new(transport);
    let answered = poll_once(client.get_balance(BalanceRequest {
        organization_id: "unlucky".to_owned(),
    }))
    .unwrap();
    assert_eq!(
        answered,
        Err(probe_service_schema::CallError::Operation(
            ProbeError::DbError
        )),
        "the thing the operation said it could fail at is not a defect"
    );
}

#[test]
fn a_fault_the_remote_produced_comes_back_in_the_fault_arm() {
    let transport = ProbeTransport::new(&[concat!(
        r#"{"ok":false,"error":{"isServiceFault":true,"fault":{"#,
        r#""detail":"the service answers to no operation by that name","field":null,"#,
        r#""kind":"unknown-operation","operation":"get-balance"}}}"#
    )]);
    let client = probe_service_schema::ProbeServiceClient::new(transport);
    let answered = poll_once(client.get_balance(BalanceRequest {
        organization_id: "acme".to_owned(),
    }))
    .unwrap();
    let reported = reported_fault(&answered).unwrap();
    assert_eq!(
        reported.kind(),
        probe_service_schema::ServiceFaultKind::UnknownOperation,
        "a fault is read back through the private mirror, the fault itself deriving no \
         `Deserialize`"
    );
    assert_eq!(reported.operation(), "get-balance");
    assert_eq!(reported.field(), None);
}

#[test]
fn an_envelope_that_contradicts_itself_is_a_defect_rather_than_an_answer() {
    let transport = ProbeTransport::new(&[r#"{"ok":true}"#]);
    let client = probe_service_schema::ProbeServiceClient::new(transport);
    let answered = poll_once(client.get_balance(BalanceRequest {
        organization_id: "acme".to_owned(),
    }))
    .unwrap();
    let reported = reported_fault(&answered).unwrap();
    assert_eq!(
        reported.kind(),
        probe_service_schema::ServiceFaultKind::UndeserializablePayload
    );
    assert!(
        reported.detail().contains("carried no value"),
        "got: {}",
        reported.detail()
    );
}

#[test]
fn the_operation_name_travels_beside_the_payload_and_never_inside_it() {
    let transport = ProbeTransport::new(&[r#"{"ok":true,"value":{"credits":7}}"#]);
    let client = probe_service_schema::ProbeServiceClient::new(transport);
    poll_once(client.get_balance(BalanceRequest {
        organization_id: "acme".to_owned(),
    }))
    .unwrap()
    .unwrap();
    let calls = client.transport().calls();
    assert_eq!(
        calls,
        vec![(
            "get-balance".to_owned(),
            r#"{"organization_id":"acme"}"#.to_owned()
        )],
        "the payload reserves no key for routing, and the name is the transport's own argument"
    );
}

#[test]
fn a_method_declared_from_several_arguments_still_takes_them_separately() {
    let transport = ProbeTransport::new(&[r#"{"ok":true,"value":{"credits":1}}"#]);
    let client = probe_service_schema::ProbeServiceClient::new(transport);
    poll_once(client.expire_credit("acme".to_owned(), "cr-1".to_owned()))
        .unwrap()
        .unwrap();
    assert_eq!(
        client.transport().calls(),
        vec![(
            "expire-credit".to_owned(),
            r#"{"organizationId":"acme","creditId":"cr-1"}"#.to_owned()
        )],
        "the packing into the declared message is the macro's job"
    );
}

#[test]
fn a_method_for_an_operation_that_takes_nothing_still_sends_a_payload() {
    let transport = ProbeTransport::new(&[r#"{"ok":true,"value":{"credits":0}}"#]);
    let client = probe_service_schema::ProbeServiceClient::new(transport);
    poll_once(client.sweep()).unwrap().unwrap();
    assert_eq!(
        client.transport().calls(),
        vec![("sweep".to_owned(), "{}".to_owned())],
        "an operation that later gains a field must not change from no payload to a payload"
    );
}

#[test]
fn a_one_way_method_sends_and_answers_with_nothing_beyond_that() {
    let transport = ProbeTransport::new(&[""]);
    let client = probe_service_schema::ProbeServiceClient::new(transport);
    let answered = poll_once(client.apply_bundle(AdmitRequest {
        organization_id: "acme".to_owned(),
    }))
    .unwrap();
    assert!(answered.is_ok(), "got: {answered:?}");
    assert_eq!(
        client.transport().calls(),
        vec![(
            "apply-bundle".to_owned(),
            r#"{"organization_id":"acme"}"#.to_owned()
        )],
        "there is no reply to carry an error, so there is nothing else to answer with"
    );
}

#[test]
fn a_message_failing_its_own_validation_is_a_fault_and_the_transport_is_never_reached() {
    // Built with no answers: reaching either of its methods panics, so this test passing at all
    // is the proof that the transport was not touched.
    let transport = ProbeTransport::new(&[]);
    let client = probe_service_schema::ProbeServiceClient::new(transport);
    let answered = poll_once(client.admit(AdmitRequest {
        organization_id: "ab".to_owned(),
    }))
    .unwrap();
    let reported = reported_fault(&answered).unwrap();
    assert_eq!(
        reported.kind(),
        probe_service_schema::ServiceFaultKind::FailedValidation,
        "the operation never ran, so this is not one of its declared errors"
    );
    assert_eq!(
        reported.field(),
        Some("organization_id"),
        "a malformed request fails at the caller with the field named, rather than a round trip \
         later"
    );
    assert_eq!(reported.operation(), "admit");
    assert!(
        client.transport().calls().is_empty(),
        "got: {:?}",
        client.transport().calls()
    );
}

#[test]
fn a_one_way_message_failing_its_own_validation_is_a_fault_and_sends_nothing() {
    let transport = ProbeTransport::new(&[]);
    let client = probe_service_schema::ProbeServiceClient::new(transport);
    let answered = poll_once(client.apply_bundle(AdmitRequest {
        organization_id: "ab".to_owned(),
    }))
    .unwrap();
    let reported = answered.unwrap_err();
    assert_eq!(
        reported.kind(),
        probe_service_schema::ServiceFaultKind::FailedValidation
    );
    assert_eq!(reported.field(), Some("organization_id"));
    assert!(
        client.transport().calls().is_empty(),
        "a defect it can see for itself is not something to publish"
    );
}

#[test]
fn a_call_the_transport_could_not_carry_comes_back_as_a_fault_rather_than_never_answering() {
    let client = probe_service_schema::ProbeServiceClient::new(DeadlineTransport::new());
    let answered = poll_once(client.get_balance(BalanceRequest {
        organization_id: "acme".to_owned(),
    }))
    .unwrap();
    let reported = reported_fault(&answered).unwrap();
    assert_eq!(
        reported.kind(),
        probe_service_schema::ServiceFaultKind::TransportFailure,
        "a reply that is never coming is a defect the caller has to be told about, not a call \
         left to sit"
    );
    assert_eq!(
        reported.operation(),
        "get-balance",
        "the fault names the call that did not land, not the transport"
    );
    assert_eq!(
        reported.detail(),
        "no reply within 30s",
        "what the transport said is what the log line carries: the client knows nothing about \
         why a call did not travel"
    );
    assert_eq!(
        reported.field(),
        None,
        "nothing in the message was wrong, so there is no field to name"
    );
    assert_eq!(
        client.transport().calls(),
        vec!["get-balance".to_owned()],
        "the message passed its own validator and went out, which is what tells this fault apart \
         from a refusal"
    );
}

#[test]
fn a_one_way_send_the_transport_could_not_put_out_comes_back_as_a_fault() {
    let client = probe_service_schema::ProbeServiceClient::new(DeadlineTransport::new());
    let answered = poll_once(client.apply_bundle(AdmitRequest {
        organization_id: "acme".to_owned(),
    }))
    .unwrap();
    let reported = answered.unwrap_err();
    assert_eq!(
        reported.kind(),
        probe_service_schema::ServiceFaultKind::TransportFailure,
        "a one-way operation has no reply to carry an error, and the send itself failing is still \
         something the caller is owed"
    );
    assert_eq!(reported.operation(), "apply-bundle");
    assert_eq!(reported.detail(), "no confirmation within 30s");
    assert_eq!(
        client.transport().calls(),
        vec!["apply-bundle".to_owned()],
        "the transport was reached, unlike the message this client refuses for itself"
    );
}

#[test]
fn a_transport_failure_reads_the_same_way_to_a_caller_as_every_other_defect() {
    let client = probe_service_schema::ProbeServiceClient::new(DeadlineTransport::new());
    let answered = poll_once(client.get_balance(BalanceRequest {
        organization_id: "acme".to_owned(),
    }))
    .unwrap();
    assert!(
        !matches!(answered, Err(probe_service_schema::CallError::Operation(_))),
        "a call that never landed is not one of the things the operation said it could fail at"
    );
    assert_eq!(
        reported_fault(&answered).unwrap().to_string(),
        "transport failure in operation `get-balance`: no reply within 30s",
        "the one line a fault renders to names the kind before the operation and the detail"
    );
}

#[test]
fn a_transport_failure_is_reported_only_for_a_message_the_client_did_not_refuse_first() {
    let client = probe_service_schema::ProbeServiceClient::new(DeadlineTransport::new());
    let answered = poll_once(client.admit(AdmitRequest {
        organization_id: "ab".to_owned(),
    }))
    .unwrap();
    assert_eq!(
        reported_fault(&answered).unwrap().kind(),
        probe_service_schema::ServiceFaultKind::FailedValidation,
        "the outbound check still runs first, so a message this client can see is wrong never \
         becomes a transport failure a round trip later"
    );
    assert!(
        client.transport().calls().is_empty(),
        "got: {:?}",
        client.transport().calls()
    );
}

#[test]
fn what_the_dispatcher_writes_is_what_the_client_reads() {
    let client = probe_service_schema::ProbeServiceClient::new(Loopback::new());
    let answered = poll_once(client.get_balance(BalanceRequest {
        organization_id: "acme".to_owned(),
    }))
    .unwrap();
    assert_eq!(
        answered,
        Ok(BalanceResponse { credits: 7 }),
        "one envelope, written by the dispatcher and read by the client"
    );

    let failed = poll_once(client.get_balance(BalanceRequest {
        organization_id: "unlucky".to_owned(),
    }))
    .unwrap();
    assert_eq!(
        failed,
        Err(probe_service_schema::CallError::Operation(
            ProbeError::DbError
        )),
        "and the failure arm survives the round trip as the error the operation declared"
    );

    poll_once(client.apply_bundle(AdmitRequest {
        organization_id: "acme".to_owned(),
    }))
    .unwrap()
    .unwrap();
    assert_eq!(
        client.transport().reached(),
        vec![
            "get_balance acme".to_owned(),
            "get_balance unlucky".to_owned(),
            "apply_bundle acme".to_owned(),
        ],
        "every method reached the operation it names"
    );
}
