//! Two services declared through the macro, one transport serving both, and the call error a
//! client hands back.
//!
//! The reply handle is exercised rather than merely implemented: `send` is handed a value the
//! transport serializes itself, which is what keeps the wire format out of the generator, and a
//! one-way operation reaches it with nothing at all.
//!
//! This file is outside the module the fault is declared in, so what it builds through the fault's
//! own constructors is what a hand-written dispatcher can build.

#![cfg(feature = "serde")]

use crate::{amqp_transport, usage_amqp_transport};
use core::future::{Future, ready};
use core::pin::pin;
use core::task::{Context as PollContext, Poll, Waker};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tixschema::service_schema;

#[derive(Deserialize, Serialize)]
pub struct PurgeRequest {
    pub organization_id: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum SweepError {
    DbError,
}

#[derive(Deserialize, Serialize)]
pub struct SweepReport {
    pub swept: u32,
}

pub struct ProbeBackEnd {
    purged: Mutex<Vec<String>>,
    swept: u32,
}

/// A transport that writes down what it was asked to do instead of publishing it.
pub struct ProbeTransport {
    settled: Mutex<Vec<String>>,
}

#[service_schema(transports = ["amqp_rpc"])]
pub trait SweepService<Ctx> {
    #[service_schema_op(one_way)]
    async fn purge(&self, ctx: &Ctx, req: PurgeRequest);

    async fn sweep(&self, ctx: &Ctx) -> Result<SweepReport, SweepError>;
}

#[service_schema(transports = ["amqp_rpc"])]
pub trait UsageService<Ctx> {
    async fn count(&self, ctx: &Ctx) -> Result<SweepReport, SweepError>;
}

impl SweepService<String> for ProbeBackEnd {
    async fn purge(&self, ctx: &String, req: PurgeRequest) {
        let _read = ready(ctx.len()).await;
        self.purged.lock().unwrap().push(req.organization_id);
    }

    async fn sweep(&self, ctx: &String) -> Result<SweepReport, SweepError> {
        let named = ready(ctx.is_empty()).await;
        if named {
            Err(SweepError::DbError)
        } else {
            Ok(SweepReport { swept: self.swept })
        }
    }
}

impl UsageService<String> for ProbeBackEnd {
    async fn count(&self, ctx: &String) -> Result<SweepReport, SweepError> {
        SweepService::sweep(self, ctx).await
    }
}

// One transport, two services, two unrelated reply handles: the handle travels with each service's
// own dispatcher, so serving both means implementing both.
impl amqp_transport::Reply for ProbeTransport {
    async fn fault(&self, fault: sweep_service_schema::ServiceFault) {
        self.record(fault.to_string());
    }

    async fn send<T>(&self, value: T)
    where
        T: Serialize + Send,
    {
        self.record(serde_json::to_string(&value).unwrap());
    }
}

impl usage_amqp_transport::Reply for ProbeTransport {
    async fn fault(&self, fault: usage_service_schema::ServiceFault) {
        self.record(fault.to_string());
    }

    async fn send<T>(&self, value: T)
    where
        T: Serialize + Send,
    {
        self.record(serde_json::to_string(&value).unwrap());
    }
}

impl ProbeBackEnd {
    fn new(swept: u32) -> Self {
        Self {
            purged: Mutex::new(Vec::new()),
            swept,
        }
    }

    fn purged(&self) -> Vec<String> {
        self.purged.lock().unwrap().clone()
    }
}

impl ProbeTransport {
    fn new() -> Self {
        Self {
            settled: Mutex::new(Vec::new()),
        }
    }

    fn record(&self, what: String) {
        self.settled.lock().unwrap().push(what);
    }

    fn settled(&self) -> Vec<String> {
        self.settled.lock().unwrap().clone()
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
fn a_one_way_operation_runs_and_leaves_the_handle_it_was_given_untouched() {
    let service = ProbeBackEnd::new(12);
    let transport = ProbeTransport::new();
    let ctx = "probe".to_owned();
    poll_once(amqp_transport::dispatch(
        &service,
        &ctx,
        &amqp_transport::IncomingMessage {
            operation: "purge".to_owned(),
            payload: br#"{"organization_id":"acme"}"#.to_vec(),
        },
        &transport,
    ))
    .unwrap();
    assert_eq!(
        service.purged(),
        vec!["acme".to_owned()],
        "the implementation is the whole of what a one-way arm reaches"
    );
    assert!(
        transport.settled().is_empty(),
        "a one-way operation answers nothing, so the handle it was given is never touched at \
         all; acknowledgement is the transport adapter's, after `dispatch` returns. Got: {:?}",
        transport.settled()
    );
}

#[test]
fn a_reply_is_handed_the_value_and_the_transport_serializes_it() {
    use amqp_transport::Reply as _;

    let service = ProbeBackEnd::new(12);
    let transport = ProbeTransport::new();
    let ctx = "probe".to_owned();
    let answered = poll_once(service.sweep(&ctx)).unwrap().unwrap();
    poll_once(transport.send(answered)).unwrap();
    assert_eq!(
        transport.settled(),
        vec![r#"{"swept":12}"#.to_owned()],
        "the encoding sits behind the trait, so `send` takes the value rather than a buffer"
    );
}

#[test]
fn one_transport_serves_two_services_through_two_reply_handles() {
    let service = ProbeBackEnd::new(12);
    let transport = ProbeTransport::new();
    let ctx = "probe".to_owned();
    let counted = poll_once(service.count(&ctx)).unwrap().unwrap();
    poll_once(usage_amqp_transport::Reply::send(&transport, counted)).unwrap();
    poll_once(amqp_transport::Reply::send(
        &transport,
        SweepReport { swept: 7 },
    ))
    .unwrap();
    assert_eq!(
        transport.settled(),
        vec![r#"{"swept":12}"#.to_owned(), r#"{"swept":7}"#.to_owned()],
        "each service's dispatcher declares its own `Reply`, so nothing is shared between the two"
    );
}

#[test]
fn a_second_service_s_dispatcher_stands_beside_the_first_in_one_crate() {
    let service = ProbeBackEnd::new(12);
    let transport = ProbeTransport::new();
    let ctx = "probe".to_owned();
    poll_once(usage_amqp_transport::dispatch(
        &service,
        &ctx,
        &usage_amqp_transport::IncomingMessage {
            operation: "count".to_owned(),
            payload: b"{}".to_vec(),
        },
        &transport,
    ))
    .unwrap();
    assert_eq!(
        transport.settled(),
        vec![r#"{"ok":true,"value":{"swept":12}}"#.to_owned()],
        "the second placement is its own set of items, and answers through the handle it declared \
         itself"
    );
}

/// The call site the design writes out: three outcomes matched at two levels, the declared error
/// and the defect reaching separate arms.
fn acted_on(answered: Result<SweepReport, sweep_service_schema::CallError<SweepError>>) -> String {
    match answered {
        Ok(report) => format!("rendered {}", report.swept),
        Err(sweep_service_schema::CallError::Operation(SweepError::DbError)) => {
            "retried later".to_owned()
        }
        Err(sweep_service_schema::CallError::Fault(defect)) => format!("paged a human: {defect}"),
    }
}

#[test]
fn a_call_error_carries_the_error_the_operation_declared() {
    assert_eq!(
        acted_on(Err(sweep_service_schema::CallError::Operation(
            SweepError::DbError,
        ))),
        "retried later",
        "the declared arm carries the operation's own error, and the caller acts on it"
    );
    assert_eq!(
        acted_on(Ok(SweepReport { swept: 3 })),
        "rendered 3",
        "and the success arm is untouched by the failure arm gaining a second shape"
    );
}

/// Every kind a fault reports, built from here rather than from inside the generated module.
///
/// The path resolving is the whole of what this pins: a dispatcher that cannot name a constructor
/// has no way to answer a defect, and spelling `pub` in the expansion says nothing about whether
/// the name reaches a caller.
#[test]
fn every_kind_of_fault_is_built_through_its_own_constructor_from_outside_the_module() {
    use sweep_service_schema::{ServiceFault, ServiceFaultKind};

    let refused = ServiceFault::failed_validation("sweep", Some("organization_id"), "is empty");
    assert_eq!(refused.kind(), ServiceFaultKind::FailedValidation);
    assert_eq!(refused.operation(), "sweep");
    assert_eq!(refused.field(), Some("organization_id"));
    assert_eq!(refused.detail(), "is empty");

    let unread = ServiceFault::undeserializable_payload("sweep", "expected value at line 1");
    assert_eq!(unread.kind(), ServiceFaultKind::UndeserializablePayload);
    assert_eq!(unread.operation(), "sweep");
    assert_eq!(unread.field(), None, "nothing was read far enough to name");
    assert_eq!(unread.detail(), "expected value at line 1");

    let panicked = ServiceFault::handler_panic("purge", "index out of bounds");
    assert_eq!(panicked.kind(), ServiceFaultKind::HandlerPanic);
    assert_eq!(panicked.operation(), "purge");
    assert_eq!(panicked.field(), None);
    assert_eq!(panicked.detail(), "index out of bounds");

    let uncarried = ServiceFault::transport_failure("sweep", "the connection went away");
    assert_eq!(uncarried.kind(), ServiceFaultKind::TransportFailure);
    assert_eq!(uncarried.operation(), "sweep");
    assert_eq!(uncarried.field(), None);
    assert_eq!(uncarried.detail(), "the connection went away");

    let unrecognised = ServiceFault::unknown_operation("rebuild");
    assert_eq!(unrecognised.kind(), ServiceFaultKind::UnknownOperation);
    assert_eq!(unrecognised.operation(), "rebuild");
    assert_eq!(unrecognised.field(), None);
    assert_eq!(
        unrecognised.detail(),
        "the service answers to no operation by that name",
        "the one constructor that writes its own detail, the name being all it was given"
    );
}

#[test]
fn a_fault_built_outside_the_module_settles_through_the_reply_handle_a_transport_implements() {
    use amqp_transport::Reply as _;

    let transport = ProbeTransport::new();
    poll_once(
        transport.fault(sweep_service_schema::ServiceFault::unknown_operation(
            "rebuild",
        )),
    )
    .unwrap();
    assert_eq!(
        transport.settled(),
        vec![
            "unknown operation in operation `rebuild`: the service answers to no operation by \
             that name"
                .to_owned()
        ],
        "a hand-written dispatcher builds the fault and hands it to the same handle the generated \
         one does"
    );
}
