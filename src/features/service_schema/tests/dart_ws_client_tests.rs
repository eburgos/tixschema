//! The `ws_rpc` Dart client, read off the emitted text.
//!
//! No Dart toolchain is reachable here, so nothing here type-checks the emitted source — these
//! tests read structure, the same way `dart_http_client_tests` reads the `http_rest` sibling's own
//! output: a substring that must appear, and a name that must not.

use super::{DART_WS_SERVICE, dart_ws_client_of};

/// The body of one method or dispatch arm, from its own start marker through the closing brace of
/// whatever follows — mirrors `dart_http_client_tests`'s own `method_body`.
fn body_from<'written>(written: &'written str, marker: &str) -> &'written str {
    let start = written.find(marker);
    assert!(start.is_some(), "no `{marker}` in: {written}");
    let rest = &written[start.unwrap()..];
    let end = rest.find("\n\n").unwrap_or(rest.len());
    &rest[..end]
}

#[test]
fn exactly_one_transport_class_is_emitted_and_it_names_no_socket_package() {
    let written = dart_ws_client_of(DART_WS_SERVICE);
    assert_eq!(
        written.matches("class LedgerWsTransport {").count(),
        1,
        "one transport class serves every operation on the service. Got: {written}"
    );
    for named in [
        "package:web_socket_channel",
        "web_socket_channel",
        "dart:io",
        "import '",
    ] {
        assert!(
            !written.contains(named),
            "the transport speaks only in sink/stream terms; the socket package that finally \
             carries the call is an adapter's business, never this crate's. Got: {written}"
        );
    }
}

#[test]
fn the_heartbeat_class_carries_the_declared_defaults_and_a_const_off_constructor() {
    let written = dart_ws_client_of(DART_WS_SERVICE);
    assert!(
        written.contains("class LedgerWsHeartbeat {"),
        "got: {written}"
    );
    assert!(
        written.contains("final Duration interval;") && written.contains("final Duration timeout;"),
        "got: {written}"
    );
    assert!(
        written.contains("this.interval = const Duration(seconds: 30),")
            && written.contains("this.timeout = const Duration(seconds: 10),"),
        "the default ping interval is 30 seconds and the default pong timeout is 10. Got: {written}"
    );
    assert!(
        written.contains("const LedgerWsHeartbeat.off()"),
        "a const constructor turns the heartbeat off. Got: {written}"
    );
}

#[test]
fn the_transport_is_constructed_over_a_sink_and_a_stream_with_an_optional_heartbeat() {
    let written = dart_ws_client_of(DART_WS_SERVICE);
    assert!(
        written.contains("LedgerWsTransport({")
            && written.contains("required StreamSink<dynamic> sink,")
            && written.contains("required Stream<dynamic> stream,")
            && written.contains("LedgerWsHeartbeat? heartbeat,"),
        "a null heartbeat takes the default, constructed beside the required sink and stream. \
         Got: {written}"
    );
    assert!(
        written.contains("_heartbeat = heartbeat ?? const LedgerWsHeartbeat()"),
        "got: {written}"
    );
    assert!(
        written.contains(
            "_subscription = stream.listen(_onFrame, onDone: _onClose, onError: (_) => _onClose());"
        ),
        "the transport owns the one `stream.listen` a non-broadcast stream allows. Got: {written}"
    );
}

#[test]
fn the_transport_holds_pending_completers_keyed_by_frame_id_and_can_close() {
    let written = dart_ws_client_of(DART_WS_SERVICE);
    assert!(
        written.contains("final Map<String, Completer<Map<String, dynamic>>> _pending = {};"),
        "got: {written}"
    );
    assert!(
        written.contains("Timer? _pingTimer;") && written.contains("Timer? _pongTimer;"),
        "the heartbeat is timer-driven. Got: {written}"
    );
    assert!(
        written.contains("void close() => _onClose();"),
        "got: {written}"
    );
    let close_body = body_from(&written, "void _onClose()");
    assert!(
        close_body.contains("_subscription.cancel();")
            && close_body.contains("completer.completeError(")
            && close_body.contains("_controller.close();"),
        "closing settles every pending call and closes the frames controller rather than leaving \
         either hanging. Got: {close_body}"
    );
}

#[test]
fn request_ids_start_at_one() {
    let written = dart_ws_client_of(DART_WS_SERVICE);
    assert!(written.contains("int _nextId = 1;"), "got: {written}");
}

#[test]
fn the_transport_exposes_frames_as_a_structural_record_over_a_broadcast_controller() {
    let written = dart_ws_client_of(DART_WS_SERVICE);
    assert!(
        written.contains(
            "final StreamController<Map<String, dynamic>> _controller =\n      \
             StreamController<Map<String, dynamic>>.broadcast();"
        ),
        "got: {written}"
    );
    assert!(
        written.contains(
            "({Stream<Map<String, dynamic>> inbound, void Function(Map<String, dynamic>) send}) \
             get frames => (inbound: _controller.stream, send: send);"
        ),
        "every transport publishes the same structural seam, keyed on `inbound` and `send`. \
         Got: {written}"
    );
    assert!(
        written.contains("void send(Map<String, dynamic> frame) {")
            && !written.contains("_write(")
            && !written.contains("_attach")
            && !written.contains("_attachments"),
        "`_write` is now the public `send`, and there is no separate attachment list any more. \
         Got: {written}"
    );
    let frame_body = body_from(&written, "void _onFrame(dynamic raw)");
    assert!(
        frame_body.contains("_pongTimer?.cancel();")
            && frame_body.matches("_controller.add(frame);").count() == 2,
        "a ping and a pong reach neither the pending map nor the controller; a correlated reply \
         settles its own completer and nothing else; every other frame, including an \
         uncorrelated reply, reaches the controller. Got: {frame_body}"
    );
}

#[test]
fn a_reply_operation_answers_future_of_success_and_decodes_through_the_generated_codec() {
    let written = dart_ws_client_of(DART_WS_SERVICE);
    assert!(
        written.contains(
            "Future<TransactionList> listTransactions(ListTransactionsRequest req) async {"
        ),
        "got: {written}"
    );
    let method = body_from(&written, " listTransactions(");
    assert!(
        method.contains("reply = await _transport.request('list-transactions', req.toJson());"),
        "got: {method}"
    );
    assert!(
        method.contains("if (reply['ok'] == true) {")
            && method.contains(
                "return TransactionList.fromJson(reply['value'] as Map<String, dynamic>);"
            ),
        "a successful reply decodes through the generated `fromJson` codec. Got: {method}"
    );
}

#[test]
fn a_reply_operation_throws_the_declared_error_or_a_fault_behind_is_service_fault() {
    let written = dart_ws_client_of(DART_WS_SERVICE);
    let method = body_from(&written, " listTransactions(");
    assert!(
        method.contains("throw LedgerWsError<ListError>.fault(_ledgerWsTransportFailure('list-transactions', '$uncarried'));"),
        "a transport failure is a fault, never the declared error. Got: {method}"
    );
    assert!(
        method.contains("if (error is Map<String, dynamic> && error['isServiceFault'] == true) {")
            && method.contains(
                "fault = LedgerFaultFields.fromJson(error['fault'] as Map<String, dynamic>);"
            )
            && method.contains("throw LedgerWsError<ListError>.fault(fault);"),
        "a wire fault behind `isServiceFault` is read back through the generated `FaultFields` \
         codec. Got: {method}"
    );
    assert!(
        method.contains("declared = ListError.fromJson(error as Map<String, dynamic>);")
            && method.contains("throw LedgerWsError<ListError>.declared(declared);"),
        "otherwise the wire's `error` is the operation's own declared error. Got: {method}"
    );
}

#[test]
fn a_reply_decode_failure_is_a_failed_validation_fault_not_an_undeserializable_payload_one() {
    let written = dart_ws_client_of(DART_WS_SERVICE);
    let method = body_from(&written, " listTransactions(");
    assert!(
        method.matches("_ledgerWsFailedValidation(").count() >= 2,
        "both a bad success value and a bad declared error decode to `failedValidation`. \
         Got: {method}"
    );
    assert!(
        !written.contains("UndeserializablePayload")
            && !written.contains("undeserializablePayload"),
        "`ws_rpc` has no status ladder, so this crate's own decision names every reply decode \
         failure `failedValidation` instead. Got: {written}"
    );
}

#[test]
fn a_one_way_operation_answers_future_void_and_throws_a_refusal() {
    let written = dart_ws_client_of(DART_WS_SERVICE);
    assert!(
        written.contains("Future<void> applyBundle(ApplyBundleRequest req) async {"),
        "got: {written}"
    );
    let method = body_from(&written, " applyBundle(");
    assert!(
        method.contains("await _transport.notify('apply-bundle', req.toJson());")
            && method.contains(
                "throw LedgerWsRefusal(_ledgerWsTransportFailure('apply-bundle', '$uncarried'));"
            ),
        "a one-way method throws the fault-only refusal, having no declared error to carry one \
         in. Got: {method}"
    );
}

#[test]
fn the_client_class_holds_one_constructor_and_every_operation_as_a_method() {
    let written = dart_ws_client_of(DART_WS_SERVICE);
    assert!(
        written.contains("class LedgerWsClient {")
            && written.contains("LedgerWsClient(this._transport);")
            && written.contains("final LedgerWsTransport _transport;"),
        "got: {written}"
    );
    for call in ["listTransactions", "applyBundle"] {
        assert!(
            written.contains(&format!(" {call}(")),
            "operation `{call}` should have a method on the client. Got: {written}"
        );
    }
}

#[test]
fn the_handlers_class_carries_one_member_per_operation_typed_for_its_own_outcome() {
    let written = dart_ws_client_of(DART_WS_SERVICE);
    assert!(
        written.contains("class LedgerHandlers<Ctx> {"),
        "got: {written}"
    );
    assert!(
        written.contains("required this.listTransactions,")
            && written.contains("required this.applyBundle,"),
        "got: {written}"
    );
    assert!(
        written.contains(
            "final Future<TransactionList> Function(Ctx ctx, ListTransactionsRequest req) listTransactions;"
        ),
        "a reply operation's own handler answers the declared success. Got: {written}"
    );
    assert!(
        written
            .contains("final Future<void> Function(Ctx ctx, ApplyBundleRequest req) applyBundle;"),
        "a one-way operation's own handler answers nothing. Got: {written}"
    );
}

#[test]
fn the_attachment_takes_the_structural_frames_record_and_requires_on_fault() {
    let written = dart_ws_client_of(DART_WS_SERVICE);
    assert!(
        written.contains("void Function() attachLedgerWsDispatcher<Ctx>(")
            && written.contains(
                "({Stream<Map<String, dynamic>> inbound, void Function(Map<String, dynamic>) \
                 send}) frames,"
            )
            && written.contains("Ctx ctx,")
            && written.contains("LedgerHandlers<Ctx> handlers, {")
            && written.contains("required void Function(LedgerFaultFields) onFault,"),
        "the attachment names no transport class of its own, so it composes over any service's \
         own `frames`, not just this one's. Got: {written}"
    );
    assert!(
        written.contains("final subscription = frames.inbound.listen((frame) async {")
            && written.contains("return () {\n    subscription.cancel();\n  };"),
        "the attachment listens on the already-shared broadcast stream rather than the socket \
         itself, and answers with the function that cancels that subscription. Got: {written}"
    );
}

#[test]
fn an_unknown_operation_on_a_request_frame_reports_and_replies_with_the_fault() {
    let written = dart_ws_client_of(DART_WS_SERVICE);
    assert!(
        written.contains("switch (frame['operation']) {")
            && written.contains("case 'list-transactions':")
            && written.contains("case 'apply-bundle':"),
        "got: {written}"
    );
    let default_arm = body_from(&written, "default:\n        {");
    assert!(
        default_arm.contains("final fault = _ledgerWsUnknownOperation(")
            && default_arm.contains("onFault(fault);")
            && default_arm.contains("if (replyId != null) {")
            && default_arm.contains("'ok': false,")
            && default_arm.contains("'error': {'isServiceFault': true, 'fault': fault.toJson()},"),
        "an operation name nothing on the service answers to reaches `onFault`, and a request \
         left waiting on it is answered with the same fault rather than left hanging. \
         Got: {default_arm}"
    );
}

#[test]
fn a_reply_dispatch_arm_answers_a_reply_frame_on_every_outcome() {
    let written = dart_ws_client_of(DART_WS_SERVICE);
    let reply_arm = body_from(&written, "case 'list-transactions':");
    assert!(
        reply_arm.contains(
            "decoded = ListTransactionsRequest.fromJson(frame['payload'] as Map<String, dynamic>);"
        ) && reply_arm.contains("final answered = await handlers.listTransactions(ctx, decoded);")
            && reply_arm.contains("'ok': true,")
            && reply_arm.contains("'value': answered.toJson(),")
            && reply_arm.contains("} on ListError catch (declared) {")
            && reply_arm.contains("'error': declared.toJson(),"),
        "got: {reply_arm}"
    );
    assert!(
        reply_arm.matches("if (replyId != null) {").count() == 4,
        "a decode failure, a success, a declared error and a handler panic each answer a \
         waiting request. Got: {reply_arm}"
    );
}

#[test]
fn a_request_frame_naming_a_one_way_operation_is_acknowledged() {
    let written = dart_ws_client_of(DART_WS_SERVICE);
    let one_way_arm = body_from(&written, "case 'apply-bundle':");
    assert!(
        one_way_arm.contains("await handlers.applyBundle(ctx, decoded);")
            && one_way_arm.contains("'ok': true,")
            && one_way_arm.contains("'value': null,"),
        "a one-way operation reached over a `request` frame is still answered once its handler \
         returns, `ok: true` with no value, rather than left hanging. Got: {one_way_arm}"
    );
    assert!(
        one_way_arm.matches("if (replyId != null) {").count() == 3,
        "a decode failure, the success acknowledgement and a handler panic each answer a \
         waiting request; a one-way operation declares no error to carry a declared-error \
         arm. Got: {one_way_arm}"
    );
}

#[test]
fn the_fault_helpers_reuse_the_generated_fault_fields_and_kind() {
    let written = dart_ws_client_of(DART_WS_SERVICE);
    for (helper, kind) in [
        (
            "_ledgerWsTransportFailure",
            "LedgerFaultKind.transportFailure",
        ),
        (
            "_ledgerWsFailedValidation",
            "LedgerFaultKind.failedValidation",
        ),
        (
            "_ledgerWsUnknownOperation",
            "LedgerFaultKind.unknownOperation",
        ),
        ("_ledgerWsHandlerPanic", "LedgerFaultKind.handlerPanic"),
    ] {
        assert!(
            written.contains(&format!("LedgerFaultFields _{}(", &helper[1..]))
                && written.contains(kind),
            "helper `{helper}` should report `{kind}`. Got: {written}"
        );
    }
}
