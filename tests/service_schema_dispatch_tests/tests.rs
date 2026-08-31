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

    /// A brand carrying its own bound, and a plain `#[model_schema()]` type carrying one on a field
    /// of its own. Neither is annotated where it is *held*, which is what used to leave a message
    /// holding one publishing no validator at all.
    #[model_schema(minLength = 3)]
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(transparent)]
    pub struct Slug(pub String);

    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    pub struct EnrolRequest {
        pub slug: Slug,
    }

    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    pub struct Held {
        #[model_schema_prop(minLength = 3)]
        pub name: String,
    }

    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    pub struct HoldRequest {
        pub holds: Held,
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

    /// The claims every account context carries. `jti` is the field the port's gate emptied on
    /// each of the five operations it sent, and the bound it broke.
    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    pub struct WireClaims {
        #[model_schema_prop(minLength = 1)]
        pub jti: String,
    }

    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct AppUserAccount {
        pub aud: String,
        #[serde(flatten)]
        pub claims: WireClaims,
    }

    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct AdminAccount {
        #[serde(flatten)]
        pub claims: WireClaims,
        pub sys_admin_username: String,
    }

    /// The account a request carries: `#[serde(untagged)]`, every variant a newtype. This is the
    /// shape the member walk used to stop at, and it stopped there for both reasons at once — the
    /// union published no arm for a slot with no ident, so the union published no `validate()`, so
    /// the field above it walked into a blanket `Ok(())`.
    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    #[serde(untagged)]
    pub enum ScopedAccount {
        Admin(AdminAccount),
        AppUser(AppUserAccount),
    }

    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ScopedBalanceRequest {
        pub account: ScopedAccount,
        pub organization_id: String,
    }

    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct AdminBalanceRequest {
        pub account: AdminAccount,
        pub organization_id: String,
    }

    /// A request that *is* an untagged enum, which is the second shape and the worse one: nothing
    /// checked one field of it, not even the fields its own variants declare bounds on.
    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    #[serde(untagged)]
    pub enum BalanceRequest {
        Admin(AdminBalanceRequest),
        Scoped(ScopedBalanceRequest),
    }

    /// A union whose first member's own bound is what takes it out of the running: the read tries
    /// `Attributed`, the bound refuses the value, and the payload is `Unattributed` instead. That
    /// check stays on the read, and moving it would change which variant a payload *is* rather than
    /// how a violation is worded.
    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    #[serde(untagged)]
    pub enum Caller {
        Attributed {
            #[serde(flatten)]
            claims: WireClaims,
            #[model_schema_prop(minLength = 3)]
            name: String,
        },
        Unattributed {
            #[serde(flatten)]
            claims: WireClaims,
        },
    }

    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct CallRequest {
        pub caller: Caller,
        pub organization_id: String,
    }

    /// The same carve-out in the position this change newly walks: a *newtype* member whose bound
    /// selects it. `Tag`'s own read hook is what takes `Tagged` out of the running, and the arm
    /// added for the slot runs after that choice is settled.
    #[model_schema(minLength = 3)]
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(transparent)]
    pub struct Tag(pub String);

    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    #[serde(untagged)]
    /// `Bounded` is declared first because an untagged read tries its members in order and `Free`
    /// admits every string: the bound is what takes the first member out of the running.
    pub enum Label {
        Bounded(Tag),
        Free(String),
    }

    #[model_schema()]
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct LabelRequest {
        pub label: Label,
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

        async fn enrol(&self, ctx: &Ctx, req: EnrolRequest) -> Result<Admitted, GateError>;

        async fn hold(&self, ctx: &Ctx, req: HoldRequest) -> Result<Admitted, GateError>;

        async fn label(&self, ctx: &Ctx, req: LabelRequest) -> Result<Admitted, GateError>;

        async fn open_ledger(&self, ctx: &Ctx, req: LedgerRequest) -> Result<Admitted, GateError>;

        async fn place_call(&self, ctx: &Ctx, req: CallRequest) -> Result<Admitted, GateError>;

        async fn read_admin_balance(
            &self,
            ctx: &Ctx,
            req: AdminBalanceRequest,
        ) -> Result<Admitted, GateError>;

        async fn read_balance(&self, ctx: &Ctx, req: BalanceRequest)
        -> Result<Admitted, GateError>;

        async fn read_scoped_balance(
            &self,
            ctx: &Ctx,
            req: ScopedBalanceRequest,
        ) -> Result<Admitted, GateError>;
    }

    impl GateService<()> for GateBackEnd {
        async fn admit(&self, _ctx: &(), req: GateRequest) -> Result<Admitted, GateError> {
            ready(()).await;
            self.reached.lock().unwrap().push(req.organization_id);
            Ok(Admitted { admitted: true })
        }

        async fn enrol(&self, _ctx: &(), req: EnrolRequest) -> Result<Admitted, GateError> {
            ready(()).await;
            self.reached.lock().unwrap().push(req.slug.0);
            Ok(Admitted { admitted: true })
        }

        async fn hold(&self, _ctx: &(), req: HoldRequest) -> Result<Admitted, GateError> {
            ready(()).await;
            self.reached.lock().unwrap().push(req.holds.name);
            Ok(Admitted { admitted: true })
        }

        async fn label(&self, _ctx: &(), req: LabelRequest) -> Result<Admitted, GateError> {
            ready(()).await;
            self.reached.lock().unwrap().push(match req.label {
                Label::Bounded(tag) => format!("tagged:{}", tag.0),
                Label::Free(free) => format!("free:{free}"),
            });
            Ok(Admitted { admitted: true })
        }

        async fn open_ledger(&self, _ctx: &(), req: LedgerRequest) -> Result<Admitted, GateError> {
            ready(()).await;
            self.reached.lock().unwrap().push(req.ledger_id);
            Ok(Admitted { admitted: true })
        }

        async fn place_call(&self, _ctx: &(), req: CallRequest) -> Result<Admitted, GateError> {
            ready(()).await;
            // Which variant the read chose, so a test can say the bound still selects rather than
            // merely that a good payload got through.
            self.reached.lock().unwrap().push(match req.caller {
                Caller::Attributed { name, .. } => format!("named:{name}"),
                Caller::Unattributed { .. } => "anonymous".to_owned(),
            });
            Ok(Admitted { admitted: true })
        }

        async fn read_admin_balance(
            &self,
            _ctx: &(),
            req: AdminBalanceRequest,
        ) -> Result<Admitted, GateError> {
            ready(()).await;
            self.reached.lock().unwrap().push(req.organization_id);
            Ok(Admitted { admitted: true })
        }

        async fn read_balance(
            &self,
            _ctx: &(),
            req: BalanceRequest,
        ) -> Result<Admitted, GateError> {
            ready(()).await;
            self.reached.lock().unwrap().push(match req {
                BalanceRequest::Admin(admin) => admin.organization_id,
                BalanceRequest::Scoped(scoped) => scoped.organization_id,
            });
            Ok(Admitted { admitted: true })
        }

        async fn read_scoped_balance(
            &self,
            _ctx: &(),
            req: ScopedBalanceRequest,
        ) -> Result<Admitted, GateError> {
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

    /// Refuses a short ledger id as the payload is read, in the words a generated validator would
    /// have used.
    fn refuse_a_short_ledger<'de, D>(deserializer: D) -> Result<String, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let read = String::deserialize(deserializer)?;
        if read.len() < 3 {
            return Err(DeError::custom(format!(
                "'ledger_id': too short: minimum length is 3, got {}",
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
            reported[0].detail().contains("too short"),
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
        // Wrong about the type its field declared, missing the key entirely, and not a document
        // in the first place. None of the three is a message, and moving the constraint checks to
        // the validator moved none of them with it — a required key in particular must not have
        // become defaultable on the way.
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

    /// A message whose bound is declared on a field's *type* rather than on the field. Until the
    /// message's own validator reached it, nothing did: the nested type's bound is enforced nowhere
    /// on the read, so this payload reached the implementation carrying a `Held` violating the
    /// pattern `Held` itself declares.
    #[test]
    fn a_bound_a_fields_own_type_declares_fails_validation_and_names_the_field_that_held_it() {
        let service = GateBackEnd::new();
        let reply = GateReply::new();
        poll_once(gate_service_schema::dispatch(
            &service,
            &(),
            &gate_service_schema::IncomingMessage {
                operation: "hold".to_owned(),
                payload: br#"{"holds":{"name":"a"}}"#.to_vec(),
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
        let reported = reply.faults();
        assert_eq!(
            reported[0].kind(),
            gate_service_schema::ServiceFaultKind::FailedValidation,
            "every key is present and every value is of the type its field declared, so this is a \
             message — one whose value broke a rule. Got detail: {}",
            reported[0].detail()
        );
        assert_eq!(
            reported[0].field(),
            Some("holds.name"),
            "the path to the member that was actually wrong, and not the hop above it: a caller \
             looks the name up in the payload it sent, where `holds` is an object rather than the \
             thing out of range. It is also the string the TypeScript schema published from this \
             same declaration reports. Got: {}",
            reported[0].detail()
        );
        assert_eq!(
            reported[0].detail(),
            "'holds.name': too short: minimum length is 3, got 1"
        );
    }

    /// The same payload with a value the bound admits reaches the implementation, which is what
    /// says the validator refuses something rather than everything.
    #[test]
    fn a_message_whose_nested_bound_is_satisfied_reaches_the_implementation() {
        let service = GateBackEnd::new();
        let reply = GateReply::new();
        poll_once(gate_service_schema::dispatch(
            &service,
            &(),
            &gate_service_schema::IncomingMessage {
                operation: "hold".to_owned(),
                payload: br#"{"holds":{"name":"abc"}}"#.to_vec(),
            },
            &reply,
        ))
        .unwrap();
        assert_eq!(service.reached(), vec!["abc".to_owned()]);
        assert!(reply.faults().is_empty(), "got: {:?}", reply.settled());
    }

    /// The brand's hook was not removed, and this is what says so: the refusal still carries the
    /// brand's own message, which only the read produces.
    ///
    /// Where it is *reported* is a separate question from where it is caught. The bytes read as a
    /// document and the value inside broke a bound, so the fault says the value someone supplied
    /// was not admitted — which is the kind the TypeScript service answers the same payload under.
    /// A brand's message names no field, having no field, so this fault carries none.
    ///
    /// Moving the check itself is a further ruling again, and the order matters: taking the hook
    /// off before the validator could reach the field would have left the bound enforced by
    /// nothing at all.
    #[test]
    fn a_brands_bound_still_refuses_the_payload_on_the_read() {
        let service = GateBackEnd::new();
        let reply = GateReply::new();
        poll_once(gate_service_schema::dispatch(
            &service,
            &(),
            &gate_service_schema::IncomingMessage {
                operation: "enrol".to_owned(),
                payload: br#"{"slug":"ab"}"#.to_vec(),
            },
            &reply,
        ))
        .unwrap();
        assert!(service.reached().is_empty(), "got: {:?}", service.reached());
        let reported = reply.faults();
        assert_eq!(
            reported[0].kind(),
            gate_service_schema::ServiceFaultKind::FailedValidation,
            "got detail: {}",
            reported[0].detail()
        );
        assert!(
            reported[0].detail().contains("too short"),
            "the hook hands serde the brand's own message verbatim. Got: {}",
            reported[0].detail()
        );
        assert_eq!(
            reported[0].field(),
            None,
            "a brand's report names no field, having no field of its own to name. Got: {}",
            reported[0].detail()
        );
        assert!(
            !reported[0].detail().contains("at line"),
            "the byte offset locates the failure inside an encoding the caller never saw. \
             Got: {}",
            reported[0].detail()
        );
    }

    /// A helper the four shapes below share: dispatch one payload and answer what the arm did with
    /// it — which messages reached the implementation, and which faults were raised.
    fn dispatched(
        operation: &str,
        payload: &[u8],
    ) -> (Vec<String>, Vec<gate_service_schema::ServiceFault>) {
        let service = GateBackEnd::new();
        let reply = GateReply::new();
        poll_once(gate_service_schema::dispatch(
            &service,
            &(),
            &gate_service_schema::IncomingMessage {
                operation: operation.to_owned(),
                payload: payload.to_vec(),
            },
            &reply,
        ))
        .unwrap();
        (service.reached(), reply.faults())
    }

    /// The account a request carries is an `#[serde(untagged)]` enum, and the bound broken is one
    /// its variant's type declares — the first of the two shapes the walk stopped at, and the one
    /// three of the port's five operations carry.
    ///
    /// The payload is a message: every key present, every value of its declared type. What it is
    /// not is one satisfying the bound `jti` declares, and until the walk reached through the union
    /// nothing said so — the request executed, and one of the three that executed was a write.
    #[test]
    fn a_bound_inside_an_untagged_variant_fails_validation_and_names_the_whole_path() {
        let (reached, reported) = dispatched(
            "read-scoped-balance",
            br#"{"account":{"aud":"app-user","jti":""},"organizationId":"gate-org"}"#,
        );
        assert!(
            reached.is_empty(),
            "the union answered Ok(()) and the message executed. Got: {reached:?}"
        );
        assert_eq!(
            reported[0].kind(),
            gate_service_schema::ServiceFaultKind::FailedValidation,
            "got detail: {}",
            reported[0].detail()
        );
        assert_eq!(
            reported[0].field(),
            Some("account.jti"),
            "an untagged newtype member writes no key of its own — what it puts on the wire is the              inner value — so the union contributes no segment and the path is the one the payload              actually spells. Got: {}",
            reported[0].detail()
        );
        assert_eq!(
            reported[0].detail(),
            "'account.jti': too short: minimum length is 1, got 0"
        );
    }

    /// The message itself is an `#[serde(untagged)]` enum — the second shape, where the union
    /// published no `validate()` at all and the dispatcher's blanket fallback answered for every
    /// payload, so not one field of the message was checked.
    ///
    /// It must now answer exactly what the variant it holds answers on its own, which is the whole
    /// content of "the walk dispatches to whichever variant deserialized".
    #[test]
    fn a_message_that_is_itself_untagged_answers_what_the_variant_it_holds_answers() {
        let payload = br#"{"account":{"aud":"app-user","jti":""},"organizationId":"gate-org"}"#;
        let (reached, reported) = dispatched("read-balance", payload);
        assert!(
            reached.is_empty(),
            "nothing checked one field of this message. Got: {reached:?}"
        );
        assert_eq!(
            reported[0].kind(),
            gate_service_schema::ServiceFaultKind::FailedValidation,
            "got detail: {}",
            reported[0].detail()
        );
        assert_eq!(reported[0].field(), Some("account.jti"));

        let (_, direct) = dispatched("read-scoped-balance", payload);
        assert_eq!(
            reported[0].detail(),
            direct[0].detail(),
            "the union has no report of its own to write: what it answers is the variant's,              unchanged"
        );
    }

    /// The operations that already refused have to keep refusing, in the same words and naming the
    /// same field. Their account is a plain struct rather than a union, which is exactly why they
    /// refused while the other three executed, and nothing here was allowed to disturb them.
    #[test]
    fn a_request_whose_account_is_no_union_keeps_refusing_what_it_already_refused() {
        let (reached, reported) = dispatched(
            "read-admin-balance",
            br#"{"account":{"sysAdminUsername":"ops","jti":""},"organizationId":"gate-org"}"#,
        );
        assert!(reached.is_empty(), "got: {reached:?}");
        assert_eq!(reported[0].field(), Some("account.jti"));
        assert_eq!(
            reported[0].detail(),
            "'account.jti': too short: minimum length is 1, got 0"
        );
    }

    /// The carve-out, from both sides. A member's own bound still runs on the read, where it takes
    /// its variant out of the running — so `name` too short is not a violation to report but a
    /// payload that is the *other* variant. A bound a member's type declares is the validator's
    /// either way, and the arm that runs it is the one the read chose.
    ///
    /// The two valid payloads are what says the selection is real: the same key, two lengths, two
    /// different variants reaching the implementation. Were that check moved to the validator,
    /// both would arrive as `Named` and one of them would then be refused — a value changing, not
    /// a message.
    #[test]
    fn a_bound_that_selects_the_variant_stays_on_the_read() {
        assert_eq!(
            dispatched(
                "place-call",
                br#"{"caller":{"name":"ab","jti":"a"},"organizationId":"gate-org"}"#
            )
            .0,
            vec!["anonymous".to_owned()],
            "the bound took the first member out of the running rather than ending the read"
        );
        assert_eq!(
            dispatched(
                "place-call",
                br#"{"caller":{"name":"abc","jti":"a"},"organizationId":"gate-org"}"#
            )
            .0,
            vec!["named:abc".to_owned()],
            "the same key one character longer is the first member, which is what the bound              decides and what only the read can decide"
        );

        // And the member's *type* is still walked in whichever variant was chosen: the bound below
        // the hop is reported, under the path the payload spells, in both.
        for (name, chosen) in [("ab", "the second member"), ("abc", "the first")] {
            let payload =
                format!(r#"{{"caller":{{"name":"{name}","jti":""}},"organizationId":"gate-org"}}"#);
            let (reached, reported) = dispatched("place-call", payload.as_bytes());
            assert!(reached.is_empty(), "{chosen} executed. Got: {reached:?}");
            assert_eq!(
                reported[0].detail(),
                "'caller.jti': too short: minimum length is 1, got 0",
                "{chosen} was chosen and its own members went unwalked"
            );
        }

        // And in the position this change newly walks: a newtype member whose brand's own hook is
        // what selects it. Adding an arm for the slot must not have moved that decision.
        assert_eq!(
            dispatched("label", br#"{"label":"ab"}"#).0,
            vec!["free:ab".to_owned()],
            "the brand's hook took the newtype member out of the running"
        );
        assert_eq!(
            dispatched("label", br#"{"label":"abc"}"#).0,
            vec!["tagged:abc".to_owned()],
            "one character longer and the same key is the newtype member instead"
        );
    }

    /// The other direction, so that what the walk refuses is something rather than everything: the
    /// same three shapes with a value the bound admits reach their implementations.
    #[test]
    fn a_message_whose_bound_beneath_a_union_is_satisfied_reaches_the_implementation() {
        for (operation, payload) in [
            (
                "read-scoped-balance",
                r#"{"account":{"aud":"app-user","jti":"a"},"organizationId":"gate-org"}"#,
            ),
            (
                "read-balance",
                r#"{"account":{"aud":"app-user","jti":"a"},"organizationId":"gate-org"}"#,
            ),
            (
                "read-admin-balance",
                r#"{"account":{"sysAdminUsername":"ops","jti":"a"},"organizationId":"gate-org"}"#,
            ),
        ] {
            let (reached, reported) = dispatched(operation, payload.as_bytes());
            assert!(
                reported.is_empty(),
                "`{operation}` refused a payload its bounds admit: {:?}",
                reported.iter().map(ToString::to_string).collect::<Vec<_>>()
            );
            assert_eq!(reached, vec!["gate-org".to_owned()], "`{operation}`");
        }
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
             inside a document that read perfectly well — the class serde_json reports it under, \
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
                "'organization_id': too short: minimum length is 3, got {}",
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
        reported.detail().contains("too short"),
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
