//! The `ws_rpc` TypeScript transport, read off the emitted text.
//!
//! What these prove and what they cannot: the structure of the emitted TypeScript — the seam's own
//! shape, that it names no platform class, that the schema tables carry one entry per
//! request-and-reply operation and none for a one-way one. No TypeScript toolchain is reachable
//! here, so none of them type-checks the bundle; `tests/service_schema_typescript_tests/type_check.rs`
//! is what proves a browser `WebSocket` satisfies the seam it names.

use super::{MIXED_SERVICE, ws_client_of};

#[test]
fn the_seam_is_structural_and_names_no_platform_class() {
    let written = ws_client_of(MIXED_SERVICE);
    let seam = seam_of(&written, "UsageServiceWsSocket");
    assert!(
        !seam.contains("WebSocket"),
        "a platform `WebSocket` satisfies the seam structurally; naming the class in the type \
         itself would couple the seam to one. Got: {seam}"
    );
}

#[test]
fn the_seam_carries_the_four_members_a_platform_socket_already_has() {
    let written = ws_client_of(MIXED_SERVICE);
    let seam = seam_of(&written, "UsageServiceWsSocket");
    assert!(
        seam.contains("send(text: string): void;") && seam.contains("close(): void;"),
        "got: {seam}"
    );
    for member in [
        "addEventListener(type: \"message\", listener: (event: { data: unknown }) => void): void;",
        "addEventListener(type: \"close\", listener: () => void): void;",
        "removeEventListener(type: \"message\", listener: (event: { data: unknown }) => void): \
         void;",
        "removeEventListener(type: \"close\", listener: () => void): void;",
    ] {
        assert!(seam.contains(member), "got: {seam}");
    }
}

#[test]
fn the_options_type_carries_an_optional_heartbeat_or_false() {
    let written = ws_client_of(MIXED_SERVICE);
    assert!(
        written.contains("export type UsageServiceWsOptions = {")
            && written.contains("heartbeat?: { intervalMs: number; timeoutMs: number } | false;"),
        "got: {written}"
    );
}

#[test]
fn the_schema_tables_carry_one_entry_per_request_and_reply_operation() {
    let written = ws_client_of(MIXED_SERVICE);
    for wire in ["get-available-balance", "expire-credit", "sweep"] {
        assert!(
            written.contains(&format!("\"{wire}\": ")),
            "a request-and-reply operation names an entry in both tables. Got: {written}"
        );
    }
    assert!(
        !written.contains("\"apply-bundle\""),
        "a one-way operation has no reply to check, so it names no table entry. Got: {written}"
    );
}

#[test]
fn each_table_entry_names_the_operation_s_own_declared_schema() {
    let written = ws_client_of(MIXED_SERVICE);
    let success = table_of(&written, "usageServiceSuccessSchemas");
    assert!(
        success.contains("\"get-available-balance\": AvailableBalanceResponse$Schema,"),
        "got: {success}"
    );
    let error = table_of(&written, "usageServiceErrorSchemas");
    assert!(
        error.contains("\"expire-credit\": CreditWriteError$Schema,"),
        "the success and the error table read from the operation's two declared arms \
         separately. Got: {error}"
    );
}

/// A unit success (`Result<(), E>`) carries no `value` on the wire — the envelope is `ok` alone —
/// so a table entry for it would check a real reply against a schema nothing on the wire is meant
/// to satisfy, failing every valid one.
#[test]
fn a_unit_success_names_no_entry_in_the_success_table_but_its_error_still_does() {
    const UNIT_SUCCESS_SERVICE: &str = "
        pub trait AckService<Ctx> {
            async fn ack(&self, ctx: &Ctx, req: AckRequest) -> Result<(), AckError>;
        }
    ";
    let written = ws_client_of(UNIT_SUCCESS_SERVICE);
    let success = table_of(&written, "ackServiceSuccessSchemas");
    assert!(
        !success.contains("\"ack\""),
        "a unit success has no value to check, so it names no entry. Got: {success}"
    );
    let error = table_of(&written, "ackServiceErrorSchemas");
    assert!(
        error.contains("\"ack\": AckError$Schema,"),
        "the operation's declared error still names an entry. Got: {error}"
    );
}

#[test]
fn a_service_with_no_request_and_reply_operation_emits_empty_tables() {
    const ONE_WAY_ONLY: &str = "
        pub trait PurgeService<Ctx> {
            #[service_schema_op(one_way)]
            async fn purge(&self, ctx: &Ctx, req: PurgeRequest);
        }
    ";
    let written = ws_client_of(ONE_WAY_ONLY);
    assert!(
        written.contains(
            "const purgeServiceSuccessSchemas: { readonly [operation: string]: ZodType<unknown> \
             | undefined } = {};"
        ) && written.contains(
            "const purgeServiceErrorSchemas: { readonly [operation: string]: ZodType<unknown> | \
             undefined } = {};"
        ),
        "an empty table is still declared, so `operation[table]` still resolves rather than \
         naming nothing. Got: {written}"
    );
}

#[test]
fn the_general_fault_builder_takes_a_kind_an_operation_a_detail_and_an_optional_field() {
    let written = ws_client_of(MIXED_SERVICE);
    assert!(
        written.contains(
            "function usageServiceFault(\n  \
             kind: UsageServiceFaultKind,\n  \
             operation: string,\n  \
             detail: string,\n  \
             field?: string,\n\
             ): UsageServiceFault {"
        ),
        "got: {written}"
    );
    assert!(
        written.find("const built: UsageServiceFaultFields = {")
            < written.find("return built as UsageServiceFault;"),
        "it mints through the same seal every other generated constructor does. Got: {written}"
    );
}

#[test]
fn the_issues_fault_folds_a_failed_parse_into_a_failed_validation_fault_naming_the_first_key() {
    let written = ws_client_of(MIXED_SERVICE);
    assert!(
        written.contains("function usageServiceIssuesFault(")
            && written.contains("kind: \"failed-validation\",")
            && written.contains("field: failedAt === \"\" ? undefined : failedAt,"),
        "got: {written}"
    );
}

#[test]
fn the_factory_binds_a_socket_and_options_and_returns_the_transport_extended_with_close() {
    let written = ws_client_of(MIXED_SERVICE);
    assert!(
        written.contains(
            "export function createUsageServiceWsTransport(\n  \
             socket: UsageServiceWsSocket,\n  \
             options: UsageServiceWsOptions = {},\n\
             ): UsageServiceTransport & { close(): void } {"
        ),
        "the seam it extends is the one `ts_client()` already publishes, never a seam of its \
         own. Got: {written}"
    );
}

#[test]
fn the_heartbeat_defaults_to_thirty_and_ten_seconds_and_a_missed_pong_closes_the_socket() {
    let written = ws_client_of(MIXED_SERVICE);
    assert!(
        written.contains("{ intervalMs: 30_000, timeoutMs: 10_000 }"),
        "got: {written}"
    );
    assert!(
        written.contains("pongDeadline = setTimeout(() => socket.close(), heartbeat.timeoutMs);"),
        "a socket that misses its pong is closed through the seam's own `close()`. Got: {written}"
    );
}

#[test]
fn every_reply_is_checked_against_the_operation_s_own_table_before_a_caller_sees_it() {
    let written = ws_client_of(MIXED_SERVICE);
    assert!(
        written.contains("usageServiceSuccessSchemas[operation]?.safeParse(envelope.value)")
            && written.contains("usageServiceErrorSchemas[operation]?.safeParse(error)"),
        "got: {written}"
    );
}

#[test]
fn a_close_settles_every_pending_request_with_a_transport_failure_fault() {
    let written = ws_client_of(MIXED_SERVICE);
    assert_eq!(
        written
            .matches("fault: usageServiceFault(\"transport-failure\", waiting.operation, detail)")
            .count(),
        1,
        "one reader serves both the socket's own close and the transport's own `close()`. \
         Got: {written}"
    );
    assert!(
        written.contains("the socket closed before the reply arrived")
            && written.contains("the transport was closed before the reply arrived"),
        "the two settle the same way with a different detail, so a caller can tell which \
         happened. Got: {written}"
    );
}

#[test]
fn an_inbound_ping_is_answered_with_a_pong_and_an_inbound_pong_re_arms_the_next_ping() {
    let written = ws_client_of(MIXED_SERVICE);
    assert!(
        written.contains(
            "if (kind === \"ping\") { socket.send(JSON.stringify({ kind: \"pong\" })); return; }"
        ),
        "got: {written}"
    );
    assert!(
        written.contains("if (kind === \"pong\") {") && written.contains("schedulePing();"),
        "got: {written}"
    );
}

/// One published type's own body, read out between its opening brace and its closing one.
fn seam_of(written: &str, named: &str) -> String {
    let found = written
        .split(&format!("export type {named} = {{"))
        .nth(1)
        .and_then(|rest| rest.split_once("\n};"))
        .map(|(body, _)| body.to_owned());
    assert!(found.is_some(), "no `{named}` type found. Got: {written}");
    found.unwrap()
}

/// One schema table's own body, read out between its opening brace and its closing one.
fn table_of(written: &str, named: &str) -> String {
    let found = written
        .split(&format!("const {named}"))
        .nth(1)
        .and_then(|rest| rest.split_once(" = {"))
        .and_then(|(_, rest)| rest.split_once("};"))
        .map(|(body, _)| body.to_owned());
    assert!(found.is_some(), "no `{named}` table found. Got: {written}");
    found.unwrap()
}
