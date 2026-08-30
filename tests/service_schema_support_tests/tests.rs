//! Two services declared through the macro, one transport serving both, and the call error a
//! client hands back.
//!
//! The reply handle is exercised rather than merely implemented: `done` settles a one-way message
//! without publishing, and `send` is handed a value the transport serializes itself, which is what
//! keeps the wire format out of the generator. `fault` has no runtime arm here on purpose — a
//! fault is constructible only inside the generated module, which is the property the compile-fail
//! run beside the constructors pins.

use core::future::{Future, ready};
use core::pin::pin;
use core::task::{Context as PollContext, Poll, Waker};
use serde::Serialize;
use std::sync::Mutex;
use tixschema::service_schema;

pub struct PurgeRequest {
    pub organization_id: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SweepError {
    DbError,
}

#[derive(Serialize)]
pub struct SweepReport {
    pub swept: u32,
}

pub struct ProbeBackEnd {
    pub swept: u32,
}

/// A transport that writes down what it was asked to do instead of publishing it.
pub struct ProbeTransport {
    settled: Mutex<Vec<String>>,
}

#[service_schema()]
pub trait SweepService<Ctx> {
    #[service_schema_op(one_way)]
    async fn purge(&self, ctx: &Ctx, req: PurgeRequest);

    async fn sweep(&self, ctx: &Ctx) -> Result<SweepReport, SweepError>;
}

#[service_schema()]
pub trait UsageService<Ctx> {
    async fn count(&self, ctx: &Ctx) -> Result<SweepReport, SweepError>;
}

impl SweepService<String> for ProbeBackEnd {
    async fn purge(&self, ctx: &String, req: PurgeRequest) {
        let _read = ready(ctx.len() + req.organization_id.len()).await;
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

// One transport, two services, two unrelated reply handles: the types are generated per service,
// so serving both means implementing both.
impl sweep_service_schema::Reply for ProbeTransport {
    async fn done(&self) {
        self.record("settled, nothing published".to_owned());
    }

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

impl usage_service_schema::Reply for ProbeTransport {
    async fn done(&self) {
        self.record("usage settled, nothing published".to_owned());
    }

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
fn a_one_way_operation_runs_and_then_settles_through_done() {
    use sweep_service_schema::Reply as _;

    let service = ProbeBackEnd { swept: 12 };
    let transport = ProbeTransport::new();
    let ctx = "probe".to_owned();
    poll_once(service.purge(
        &ctx,
        PurgeRequest {
            organization_id: "acme".to_owned(),
        },
    ))
    .unwrap();
    poll_once(transport.done()).unwrap();
    assert_eq!(
        transport.settled(),
        vec!["settled, nothing published".to_owned()],
        "a one-way operation still has to touch the handle, or its delivery is never acknowledged"
    );
}

#[test]
fn a_reply_is_handed_the_value_and_the_transport_serializes_it() {
    use sweep_service_schema::Reply as _;

    let service = ProbeBackEnd { swept: 12 };
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
    let service = ProbeBackEnd { swept: 12 };
    let transport = ProbeTransport::new();
    let ctx = "probe".to_owned();
    let counted = poll_once(service.count(&ctx)).unwrap().unwrap();
    poll_once(usage_service_schema::Reply::send(&transport, counted)).unwrap();
    poll_once(sweep_service_schema::Reply::done(&transport)).unwrap();
    assert_eq!(
        transport.settled(),
        vec![
            r#"{"swept":12}"#.to_owned(),
            "settled, nothing published".to_owned(),
        ],
        "each service generates its own `Reply`, so nothing is shared between the two"
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
