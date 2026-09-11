//! The `ws_rpc` dispatcher attachment, read off the emitted text.
//!
//! What these prove and what they cannot: the attachment's own signature, that it drives the
//! generated dispatcher rather than a service-specific switch, and that a refused `notify` and an
//! answered `request` go where the design says. No TypeScript toolchain is reachable here, so none
//! of them type-checks the bundle; `tests/service_schema_typescript_tests/type_check.rs` is what
//! proves a complete implementation compiles at the attachment and an incomplete one is refused.

use super::{MIXED_SERVICE, ws_service_of};

#[test]
fn the_signature_takes_a_socket_a_context_an_implementation_and_a_required_on_fault() {
    let written = ws_service_of(MIXED_SERVICE);
    assert!(
        written.contains(
            "export function attachUsageServiceWsDispatcher<Ctx>(\n  \
             socket: UsageServiceWsSocket,\n  \
             ctx: Ctx,\n  \
             impl: UsageServiceImpl<Ctx>,\n  \
             onFault: (fault: UsageServiceFault) => void,\n\
             ): () => void {"
        ),
        "the socket is the seam `ts_ws_client()` publishes and the implementation is the one \
         `ts_service()` publishes, so an attachment names both without declaring either. \
         Got: {written}"
    );
}

#[test]
fn it_drives_the_generated_dispatcher_rather_than_a_switch_of_its_own() {
    let written = ws_service_of(MIXED_SERVICE);
    assert!(
        written.contains("const dispatch = createUsageServiceDispatcher(impl);"),
        "routing an operation to its handler, and parsing its payload first, is the dispatcher's \
         job already — this attaches to it rather than repeating it. Got: {written}"
    );
}

#[test]
fn it_registers_one_message_listener_and_returns_the_function_that_removes_it() {
    let written = ws_service_of(MIXED_SERVICE);
    assert_eq!(
        written
            .matches("addEventListener(\"message\", onMessage)")
            .count(),
        1,
        "got: {written}"
    );
    assert!(
        written.contains("return () => socket.removeEventListener(\"message\", onMessage);"),
        "got: {written}"
    );
}

#[test]
fn a_frame_naming_another_service_and_non_json_text_are_both_dropped_before_the_kind_is_read() {
    let written = ws_service_of(MIXED_SERVICE);
    assert!(
        written.contains("try { frame = JSON.parse(String(event.data)); } catch { return; }")
            && written.contains("if (typeof frame !== \"object\" || frame === null) return;"),
        "non-JSON text, and a JSON value that is not an object, are dropped before a service or a \
         kind is read. Got: {written}"
    );
    assert!(
        written.contains("if (named !== service || typeof operation !== \"string\") return;"),
        "a frame naming another service is dropped before its kind decides anything. \
         Got: {written}"
    );
}

#[test]
fn a_refused_notify_reaches_on_fault_and_a_notify_that_ran_replies_to_nobody() {
    let written = ws_service_of(MIXED_SERVICE);
    let found = written
        .split("if (kind === \"notify\") {")
        .nth(1)
        .and_then(|rest| rest.split_once("if (kind !== \"request\""))
        .map(|(body, _)| body);
    assert!(found.is_some(), "no notify branch found. Got: {written}");
    let notify_branch = found.unwrap();
    assert!(
        notify_branch.contains("if (answered === undefined) return;")
            && notify_branch.contains("onFault(error.fault);"),
        "a notify that the dispatcher answered nothing for is not a refusal; one that answered a \
         framed fault has nobody else to tell. Got: {notify_branch}"
    );
    assert!(
        !notify_branch.contains("socket.send"),
        "a notify frame carries no id, so nothing here writes a reply for one. \
         Got: {notify_branch}"
    );
}

#[test]
fn a_request_is_answered_with_a_reply_frame_and_a_one_way_answer_is_synthesized() {
    let written = ws_service_of(MIXED_SERVICE);
    assert!(
        written.contains(
            "const envelope = answered === undefined ? { ok: true, value: null } : (answered as \
             object);"
        ),
        "a one-way operation's dispatcher answers `undefined`, and a request naming it still \
         gets a reply rather than being left to hang. Got: {written}"
    );
    assert!(
        written.contains(
            "socket.send(JSON.stringify({ kind: \"reply\", id, service, ...envelope }));"
        ),
        "got: {written}"
    );
}
