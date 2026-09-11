//! The Dart `ws_rpc` client: a transport over the sink and stream a `WebSocketChannel` already
//! exposes, a per-operation client that calls out, and a dispatcher attachment for a service the
//! app implements — one service, one `emit()`, emitting both halves together.
//!
//! # Sink and stream, not a socket class
//!
//! Nothing here names `web_socket_channel` or `dart:io`: the transport's constructor asks for
//! `StreamSink<dynamic>`/`Stream<dynamic>`, the pair every `WebSocketChannel` already exposes, so a
//! caller hands over `channel.sink`/`channel.stream` and no adapter has to be written.
//!
//! # The attachment's seam is a record, not the transport class
//!
//! `WebSocketChannel.stream` is single-subscription: a second `stream.listen` on the same stream
//! throws, so only the transport itself may call `listen`. A second service sharing that socket
//! still needs a way to reach frames it did not ask for, but Dart has no structural class typing
//! and each service's [`emit`] is independent, so `attach{Named}WsDispatcher` can name neither the
//! calling service's own transport class nor a shared base class without this module emitting the
//! same class twice for every pair of services on one crate. A record is structural: every
//! `{Named}WsTransport` exposes the identically-shaped `frames` — a broadcast `Stream` fed by every
//! inbound frame the transport does not itself correlate to a pending `request`, paired with the
//! function that writes a frame back — and `attach{Named}WsDispatcher` takes that record rather
//! than a transport, so it composes across services sharing one socket the same way the
//! `http_rest` Dart client's own structural transport seam does.
//!
//! # A caller throws, exactly as the `http_rest` Dart client does
//!
//! A reply operation answers `Future<Success>` and throws `{Named}WsError<Declared>` — the declared
//! error, or a fault behind `isServiceFault` — instead of returning a result union; a one-way
//! operation answers `Future<void>` and throws the fault-only `{Named}WsRefusal`. Both exceptions,
//! and the fault type they carry (`{Named}FaultFields`), are the same shapes
//! [`super::dart_http_client`] already throws.
//!
//! # A decode failure is a failed-validation fault, not an undeserializable-payload one
//!
//! `http_rest` answers a body that will not decode with `undeserializablePayload` — a status
//! answered a shape its own code did not promise. `ws_rpc` has no status to make that distinction
//! with: a reply that will not decode is a reply that failed the check the transport was always
//! going to make against the declared type, which is `failedValidation` under this crate's own
//! vocabulary instead.
//!
//! # The wire's own fault shape
//!
//! A reply's `error` key carries the operation's declared error verbatim, or
//! `{ isServiceFault: true, fault: <FaultFields> }` in its place — the same convention
//! [`super::client`] and [`super::service`] (`ts_client()`/`ts_service()`) already write for an
//! outbound or a dispatcher-detected fault on a bus-style transport.
//!
//! # A handler signals its declared error the same way a caller reads it
//!
//! `{Named}Handlers` answers a reply operation with `Future<Success>` and throws the operation's own
//! declared error to signal it — Dart's idiom for a `Future`, matching every other exception this
//! module and [`super::dart_http_client`] throw. Anything else a handler throws is unexpected and
//! reaches `onFault` instead. Whenever the inbound frame carried an id — a caller waiting on a
//! reply, whether the operation is one-way or not — the attachment answers it: `ok: true, value:
//! null` once a one-way handler returns, a fault reply for anything that goes wrong before or
//! during dispatch, so a pending caller is never left hanging.

use crate::features::dart::dart_typename;
use crate::field_type::get_field_def;
use crate::rename_rule::RenameRule;
use crate::service_schema::parse::{
    OperationDef, OperationInputs, OperationOutcome, ServiceDef, is_unit_type,
};
use crate::service_schema::support::fault_fields_typescript_name;
use core::fmt::Write as _;
use syn::Type;

/// The structural seam `{Named}WsTransport.frames` publishes and `attach{Named}WsDispatcher`
/// takes: identically spelled for every service, so an attachment for one service composes over
/// any other service's own transport sharing its connection without naming that transport's type.
const FRAMES_RECORD_TYPE: &str =
    "({Stream<Map<String, dynamic>> inbound, void Function(Map<String, dynamic>) send})";

pub fn emit(service: &ServiceDef) -> Vec<String> {
    let named = service.ident.to_string();
    let fn_prefix = RenameRule::CamelCase.apply_to_variant(&named);
    let mut published = vec![
        heartbeat_class(&named),
        transport_class(&named),
        error_class(&named),
    ];
    if has_one_way(service) {
        published.push(refusal_class(&named));
    }
    published.push(client_class(service));
    published.push(handlers_class(service));
    published.push(attach_dispatcher_fn(service));
    published.extend(fault_helpers(&named, &fn_prefix));
    published
}

fn has_one_way(service: &ServiceDef) -> bool {
    service
        .operations
        .iter()
        .any(|operation| matches!(operation.outcome, OperationOutcome::OneWay))
}

// ---------------------------------------------------------------------------------------------
// Liveness: how often a transport pings, and how long it waits for the answering pong.
// ---------------------------------------------------------------------------------------------

fn heartbeat_class(named: &str) -> String {
    format!(
        "/// How often a `{named}` `ws_rpc` transport pings the far side, and how long it waits\n\
         /// for the answering pong before treating the connection as dead.\n\
         /// `{named}WsHeartbeat.off()` turns liveness checking off entirely.\n\
         class {named}WsHeartbeat {{\n  \
         const {named}WsHeartbeat({{\n    \
         this.interval = const Duration(seconds: 30),\n    \
         this.timeout = const Duration(seconds: 10),\n  \
         }});\n  \
         const {named}WsHeartbeat.off()\n      \
         : interval = Duration.zero,\n        \
         timeout = Duration.zero;\n  \
         final Duration interval;\n  \
         final Duration timeout;\n  \
         bool get _off => interval == Duration.zero;\n\
         }}"
    )
}

// ---------------------------------------------------------------------------------------------
// The transport: one `stream.listen`, request/reply correlation, the heartbeat, and the fan-out
// an attachment hangs off.
// ---------------------------------------------------------------------------------------------

fn transport_class(named: &str) -> String {
    format!(
        "/// A `{named}` `ws_rpc` transport over the sink and stream a `WebSocketChannel` already\n\
         /// exposes. `heartbeat` left `null` takes the default; pass `{named}WsHeartbeat.off()` to\n\
         /// turn liveness checking off.\n\
         ///\n\
         /// Owns the one `stream.listen` a non-broadcast `Stream` allows: `{named}WsClient` reads a\n\
         /// reply back off `request`, and an attachment reaches every other inbound frame through\n\
         /// `frames` instead of listening a second time.\n\
         class {named}WsTransport {{\n  \
         {named}WsTransport({{\n    \
         required StreamSink<dynamic> sink,\n    \
         required Stream<dynamic> stream,\n    \
         {named}WsHeartbeat? heartbeat,\n  \
         }})  : _sink = sink,\n        \
         _heartbeat = heartbeat ?? const {named}WsHeartbeat() {{\n    \
         _subscription = stream.listen(_onFrame, onDone: _onClose, onError: (_) => _onClose());\n    \
         _schedulePing();\n  \
         }}\n\n  \
         final StreamSink<dynamic> _sink;\n  \
         final {named}WsHeartbeat _heartbeat;\n  \
         final Map<String, Completer<Map<String, dynamic>>> _pending = {{}};\n  \
         final StreamController<Map<String, dynamic>> _controller =\n      \
         StreamController<Map<String, dynamic>>.broadcast();\n  \
         late final StreamSubscription<dynamic> _subscription;\n  \
         Timer? _pingTimer;\n  \
         Timer? _pongTimer;\n  \
         int _nextId = 1;\n  \
         bool _closed = false;\n\n  \
         /// Sends `operation` with `payload` as a `request` frame and answers with the matching\n  \
         /// `reply` frame's own fields.\n  \
         Future<Map<String, dynamic>> request(String operation, Object? payload) {{\n    \
         final id = '${{_nextId++}}';\n    \
         final completer = Completer<Map<String, dynamic>>();\n    \
         _pending[id] = completer;\n    \
         send({{\n      \
         'kind': 'request',\n      \
         'id': id,\n      \
         'service': '{named}',\n      \
         'operation': operation,\n      \
         'payload': payload,\n    \
         }});\n    \
         return completer.future;\n  \
         }}\n\n  \
         /// Sends `operation` with `payload` as a `notify` frame. No reply is expected.\n  \
         Future<void> notify(String operation, Object? payload) async {{\n    \
         send({{'kind': 'notify', 'service': '{named}', 'operation': operation, 'payload': payload}});\n  \
         }}\n\n  \
         /// Writes one frame straight onto the sink.\n  \
         void send(Map<String, dynamic> frame) {{\n    \
         _sink.add(jsonEncode(frame));\n  \
         }}\n\n  \
         /// The structural seam an attachment dispatches over: every inbound frame this transport\n  \
         /// does not itself correlate to a pending `request` (a ping and a pong included\n  \
         /// nowhere), paired with the function that writes a frame back. Every `{named}WsTransport`\n  \
         /// exposes the same shape, so an attachment for any service sharing this connection\n  \
         /// reaches it here rather than through a transport type of its own.\n  \
         {FRAMES_RECORD_TYPE} get frames => (inbound: _controller.stream, send: send);\n\n  \
         void _onFrame(dynamic raw) {{\n    \
         final Map<String, dynamic> frame;\n    \
         try {{\n      \
         frame = jsonDecode(raw as String) as Map<String, dynamic>;\n    \
         }} catch (_) {{\n      \
         return;\n    \
         }}\n    \
         switch (frame['kind']) {{\n      \
         case 'pong':\n        \
         _pongTimer?.cancel();\n        \
         return;\n      \
         case 'ping':\n        \
         send({{'kind': 'pong'}});\n        \
         return;\n      \
         case 'reply':\n        \
         final id = frame['id'];\n        \
         final completer = id is String ? _pending.remove(id) : null;\n        \
         if (completer != null) {{\n          \
         completer.complete(frame);\n          \
         return;\n        \
         }}\n        \
         _controller.add(frame);\n        \
         return;\n      \
         default:\n        \
         _controller.add(frame);\n    \
         }}\n  \
         }}\n\n  \
         void _onClose() {{\n    \
         if (_closed) return;\n    \
         _closed = true;\n    \
         _pingTimer?.cancel();\n    \
         _pongTimer?.cancel();\n    \
         _subscription.cancel();\n    \
         for (final completer in _pending.values) {{\n      \
         completer.completeError(StateError('the {named} ws_rpc connection closed'));\n    \
         }}\n    \
         _pending.clear();\n    \
         _controller.close();\n  \
         }}\n\n  \
         void _schedulePing() {{\n    \
         if (_heartbeat._off) return;\n    \
         _pingTimer = Timer.periodic(_heartbeat.interval, (_) {{\n      \
         send({{'kind': 'ping'}});\n      \
         _pongTimer?.cancel();\n      \
         _pongTimer = Timer(_heartbeat.timeout, _onClose);\n    \
         }});\n  \
         }}\n\n  \
         /// Cancels the subscription and the heartbeat, and settles every pending `request` with\n  \
         /// an error: the transport-failure fault every waiting `{named}WsClient` method wraps it\n  \
         /// into.\n  \
         void close() => _onClose();\n\
         }}"
    )
}

// ---------------------------------------------------------------------------------------------
// The two exceptions a call throws: the declared error or a fault, and (one-way only) a fault
// with nowhere else to be returned. Identical in shape to `dart_http_client`'s own.
// ---------------------------------------------------------------------------------------------

fn error_class(named: &str) -> String {
    let fields = fault_fields_typescript_name(named);
    format!(
        "/// What a `{named}` `ws_rpc` client throws for a request-and-reply operation: the error\n\
         /// the operation declared, or a fault it never declared.\n\
         class {named}WsError<E> implements Exception {{\n  \
         {named}WsError.declared(E declared)\n    \
         : error = declared,\n      \
         fault = null;\n  \
         {named}WsError.fault({fields} reported)\n    \
         : error = null,\n      \
         fault = reported;\n  \
         final E? error;\n  \
         final {fields}? fault;\n  \
         bool get isServiceFault => fault != null;\n  \
         @override\n  \
         String toString() => isServiceFault\n      \
         ? '{named}WsError: service fault ${{fault!.kind}} in `${{fault!.operation}}`: ${{fault!.detail}}'\n      \
         : '{named}WsError: $error';\n\
         }}"
    )
}

fn refusal_class(named: &str) -> String {
    let fields = fault_fields_typescript_name(named);
    format!(
        "/// What a one-way `{named}` `ws_rpc` method throws when it cannot deliver its `notify`\n\
         /// frame. A one-way operation declares no error, so there is nothing else to throw.\n\
         class {named}WsRefusal implements Exception {{\n  \
         {named}WsRefusal(this.fault);\n  \
         final {fields} fault;\n  \
         @override\n  \
         String toString() =>\n      \
         '{named}WsRefusal: ${{fault.kind}} in `${{fault.operation}}`: ${{fault.detail}}';\n\
         }}"
    )
}

// ---------------------------------------------------------------------------------------------
// The client: one class, one constructor, one method per operation, calling out over the
// transport's own `request`/`notify`.
// ---------------------------------------------------------------------------------------------

fn client_class(service: &ServiceDef) -> String {
    let named = service.ident.to_string();
    let methods = service
        .operations
        .iter()
        .map(|operation| client_method(&named, operation))
        .collect::<Vec<_>>()
        .join("\n\n");
    format!(
        "/// A `{named}` caller over `ws_rpc`.\n\
         class {named}WsClient {{\n  \
         {named}WsClient(this._transport);\n  \
         final {named}WsTransport _transport;\n\n\
         {methods}\n\
         }}"
    )
}

fn return_type(operation: &OperationDef) -> String {
    match &operation.outcome {
        OperationOutcome::OneWay => "Future<void>".to_owned(),
        OperationOutcome::Reply { success, .. } => {
            if is_unit_type(success) {
                "Future<void>".to_owned()
            } else {
                format!("Future<{}>", dart_type_of(success))
            }
        }
    }
}

fn client_method(named: &str, operation: &OperationDef) -> String {
    let fn_prefix = RenameRule::CamelCase.apply_to_variant(named);
    let wire = &operation.wire_name;
    let call = &operation.ts_name;
    let param = message_dart_typename(operation);
    let returns = return_type(operation);
    let doc = format!("  /// Calls `{wire}` over `ws_rpc`.");
    match &operation.outcome {
        OperationOutcome::OneWay => format!(
            "{doc}\n  \
             {returns} {call}({param} req) async {{\n    \
             try {{\n      \
             await _transport.notify('{wire}', req.toJson());\n    \
             }} catch (uncarried) {{\n      \
             throw {named}WsRefusal(_{fn_prefix}WsTransportFailure('{wire}', '$uncarried'));\n    \
             }}\n  \
             }}"
        ),
        OperationOutcome::Reply { error, success } => {
            let error_ty = dart_type_of(error);
            let decode = reply_decode_stmt(named, &fn_prefix, wire, &error_ty, success);
            format!(
                "{doc}\n  \
                 {returns} {call}({param} req) async {{\n    \
                 late final Map<String, dynamic> reply;\n    \
                 try {{\n      \
                 reply = await _transport.request('{wire}', req.toJson());\n    \
                 }} catch (uncarried) {{\n      \
                 throw {named}WsError<{error_ty}>.fault(_{fn_prefix}WsTransportFailure('{wire}', '$uncarried'));\n    \
                 }}\n\
{decode}\
                 }}"
            )
        }
    }
}

/// Reads the reply's own `ok`/`value`/`error` keys: the declared success on `true`, the wire's own
/// fault shape (`error['isServiceFault']`) or the declared error otherwise — mirrors
/// `dart_http_client`'s own `reply_decode_stmt`, over the reply frame rather than a status ladder.
fn reply_decode_stmt(
    named: &str,
    fn_prefix: &str,
    wire: &str,
    error_ty: &str,
    success: &Type,
) -> String {
    let fields = fault_fields_typescript_name(named);
    let success_block = success_decode_block(named, fn_prefix, wire, error_ty, success);
    format!(
        "    if (reply['ok'] == true) {{\n{success_block}    }}\n    \
         final error = reply['error'];\n    \
         if (error is Map<String, dynamic> && error['isServiceFault'] == true) {{\n      \
         late final {fields} fault;\n      \
         try {{\n        \
         fault = {fields}.fromJson(error['fault'] as Map<String, dynamic>);\n      \
         }} catch (rejected) {{\n        \
         throw {named}WsError<{error_ty}>.fault(_{fn_prefix}WsFailedValidation('{wire}', '$rejected'));\n      \
         }}\n      \
         throw {named}WsError<{error_ty}>.fault(fault);\n    \
         }}\n    \
         late final {error_ty} declared;\n    \
         try {{\n      \
         declared = {error_ty}.fromJson(error as Map<String, dynamic>);\n    \
         }} catch (rejected) {{\n      \
         throw {named}WsError<{error_ty}>.fault(_{fn_prefix}WsFailedValidation('{wire}', '$rejected'));\n    \
         }}\n    \
         throw {named}WsError<{error_ty}>.declared(declared);\n"
    )
}

fn success_decode_block(
    named: &str,
    fn_prefix: &str,
    wire: &str,
    error_ty: &str,
    success: &Type,
) -> String {
    if is_unit_type(success) {
        return "      return;\n".to_owned();
    }
    let success_ty = dart_type_of(success);
    format!(
        "      try {{\n        \
         return {success_ty}.fromJson(reply['value'] as Map<String, dynamic>);\n      \
         }} catch (rejected) {{\n        \
         throw {named}WsError<{error_ty}>.fault(_{fn_prefix}WsFailedValidation('{wire}', '$rejected'));\n      \
         }}\n"
    )
}

// ---------------------------------------------------------------------------------------------
// Handlers: what an app implementing this service answers inbound frames with, and the
// attachment that dispatches them.
// ---------------------------------------------------------------------------------------------

/// A handler's own signature: the operation's message in, its declared success out — a reply
/// operation signals its declared error by throwing it, the same idiom every exception in this
/// module and in `dart_http_client` already uses.
fn handler_signature(operation: &OperationDef) -> String {
    let req_ty = message_dart_typename(operation);
    match &operation.outcome {
        OperationOutcome::OneWay => format!("Future<void> Function(Ctx ctx, {req_ty} req)"),
        OperationOutcome::Reply { success, .. } => {
            let ret = if is_unit_type(success) {
                "void".to_owned()
            } else {
                dart_type_of(success)
            };
            format!("Future<{ret}> Function(Ctx ctx, {req_ty} req)")
        }
    }
}

fn handlers_class(service: &ServiceDef) -> String {
    let named = service.ident.to_string();
    let mut params = String::new();
    let mut fields = String::new();
    for operation in &service.operations {
        let call = &operation.ts_name;
        let signature = handler_signature(operation);
        let _ = writeln!(params, "    required this.{call},");
        let _ = writeln!(fields, "  final {signature} {call};");
    }
    format!(
        "/// What a `{named}` `ws_rpc` attachment dispatches an inbound frame to: one handler per\n\
         /// declared operation.\n\
         class {named}Handlers<Ctx> {{\n  \
         {named}Handlers({{\n\
{params}  \
         }});\n\n\
{fields}\
         }}"
    )
}

/// Writes a reply frame back over `frames.send`, correlated to the inbound frame's own id --
/// guarded on `replyId != null`, which is `null` only for a `notify` frame.
fn reply_write_stmt(named: &str, key: &str, ok: &str, value_expr: &str) -> String {
    format!(
        "          if (replyId != null) {{\n            \
         frames.send({{\n              \
         'kind': 'reply',\n              \
         'id': replyId,\n              \
         'service': '{named}',\n              \
         'ok': {ok},\n              \
         '{key}': {value_expr},\n            \
         }});\n          \
         }}\n"
    )
}

fn attach_dispatcher_fn(service: &ServiceDef) -> String {
    let named = service.ident.to_string();
    let fn_prefix = RenameRule::CamelCase.apply_to_variant(&named);
    let fields = fault_fields_typescript_name(&named);
    let arms = service
        .operations
        .iter()
        .map(|operation| dispatch_arm(&named, &fn_prefix, operation))
        .collect::<Vec<_>>()
        .join("\n");
    let mut written = format!(
        "/// Attaches `handlers` to `frames`, dispatching every inbound `{named}` frame, decoded\n\
         /// through the generated `fromJson` codecs, to its own handler. A frame this cannot\n\
         /// dispatch, or a handler raising anything but its own declared error, reaches `onFault`\n\
         /// rather than vanishing; a request left waiting on either is answered with the fault\n\
         /// instead of hanging. Answers with the function that detaches it.\n\
         void Function() attach{named}WsDispatcher<Ctx>(\n  \
         {FRAMES_RECORD_TYPE} frames,\n  \
         Ctx ctx,\n  \
         {named}Handlers<Ctx> handlers, {{\n  \
         required void Function({fields}) onFault,\n\
         }}) {{\n  \
         final subscription = frames.inbound.listen((frame) async {{\n    \
         if (frame['service'] != '{named}') return;\n    \
         final rawId = frame['id'];\n    \
         final replyId = rawId is String ? rawId : null;\n    \
         switch (frame['operation']) {{\n\
{arms}\n      \
         default:\n        \
         {{\n          \
         final fault = _{fn_prefix}WsUnknownOperation(\n            \
         '${{frame['operation']}}',\n            \
         'this service answers to no operation by that name',\n          \
         );\n          \
         onFault(fault);\n"
    );
    written.push_str(&reply_write_stmt(
        &named,
        "error",
        "false",
        "{'isServiceFault': true, 'fault': fault.toJson()}",
    ));
    written.push_str(
        "          }\n      \
         }\n    \
         });\n  \
         return () {\n    \
         subscription.cancel();\n  \
         };\n\
         }",
    );
    written
}

fn dispatch_arm(named: &str, fn_prefix: &str, operation: &OperationDef) -> String {
    let wire = &operation.wire_name;
    let call = &operation.ts_name;
    let req_ty = message_dart_typename(operation);
    let mut arm = format!(
        "      case '{wire}':\n        \
         {{\n          \
         final {req_ty} decoded;\n          \
         try {{\n            \
         decoded = {req_ty}.fromJson(frame['payload'] as Map<String, dynamic>);\n          \
         }} catch (rejected) {{\n            \
         final fault = _{fn_prefix}WsFailedValidation('{wire}', '$rejected');\n            \
         onFault(fault);\n"
    );
    arm.push_str(&reply_write_stmt(
        named,
        "error",
        "false",
        "{'isServiceFault': true, 'fault': fault.toJson()}",
    ));
    arm.push_str("            return;\n          }\n");
    match &operation.outcome {
        OperationOutcome::OneWay => {
            let _ = write!(
                arm,
                "          try {{\n            \
                 await handlers.{call}(ctx, decoded);\n"
            );
            arm.push_str(&reply_write_stmt(named, "value", "true", "null"));
            let _ = write!(
                arm,
                "          }} catch (unexpected) {{\n            \
                 final fault = _{fn_prefix}WsHandlerPanic('{wire}', '$unexpected');\n            \
                 onFault(fault);\n"
            );
            arm.push_str(&reply_write_stmt(
                named,
                "error",
                "false",
                "{'isServiceFault': true, 'fault': fault.toJson()}",
            ));
            arm.push_str("          }\n          return;\n        }\n");
        }
        OperationOutcome::Reply { error, success } => {
            let error_ty = dart_type_of(error);
            let unit = is_unit_type(success);
            let call_stmt = if unit {
                format!("await handlers.{call}(ctx, decoded);\n")
            } else {
                format!("final answered = await handlers.{call}(ctx, decoded);\n")
            };
            let value_expr = if unit {
                "null".to_owned()
            } else {
                "answered.toJson()".to_owned()
            };
            let _ = write!(
                arm,
                "          try {{\n            \
                 {call_stmt}"
            );
            arm.push_str(&reply_write_stmt(named, "value", "true", &value_expr));
            let _ = writeln!(arm, "          }} on {error_ty} catch (declared) {{");
            arm.push_str(&reply_write_stmt(
                named,
                "error",
                "false",
                "declared.toJson()",
            ));
            let _ = write!(
                arm,
                "          }} catch (unexpected) {{\n            \
                 final fault = _{fn_prefix}WsHandlerPanic('{wire}', '$unexpected');\n            \
                 onFault(fault);\n"
            );
            arm.push_str(&reply_write_stmt(
                named,
                "error",
                "false",
                "{'isServiceFault': true, 'fault': fault.toJson()}",
            ));
            arm.push_str("          }\n          return;\n        }\n");
        }
    }
    arm
}

// ---------------------------------------------------------------------------------------------
// The faults every method and every dispatch arm reaches for.
// ---------------------------------------------------------------------------------------------

fn fault_helpers(named: &str, fn_prefix: &str) -> Vec<String> {
    vec![
        fault_helper(
            named,
            fn_prefix,
            "WsTransportFailure",
            "transportFailure",
            &format!(
                "The fault a `{named}` `ws_rpc` client answers with when the transport could not \
                 carry a call: the frame never went out, or the reply never came back."
            ),
        ),
        fault_helper(
            named,
            fn_prefix,
            "WsFailedValidation",
            "failedValidation",
            &format!(
                "The fault a `{named}` `ws_rpc` reply answers with when it will not become the \
                 operation's own declared type through the generated `fromJson` codec."
            ),
        ),
        fault_helper(
            named,
            fn_prefix,
            "WsUnknownOperation",
            "unknownOperation",
            &format!(
                "The fault a `{named}` `ws_rpc` attachment answers with when an inbound frame \
                 names an operation nothing on this service declares."
            ),
        ),
        fault_helper(
            named,
            fn_prefix,
            "WsHandlerPanic",
            "handlerPanic",
            &format!(
                "The fault a `{named}` `ws_rpc` attachment answers with when a handler raises \
                 anything other than its own declared error."
            ),
        ),
    ]
}

fn fault_helper(named: &str, fn_prefix: &str, suffix: &str, kind: &str, doc: &str) -> String {
    let fields = fault_fields_typescript_name(named);
    format!(
        "/// {doc}\n\
         {fields} _{fn_prefix}{suffix}(String operation, String detail) => {fields}(\n  \
         detail: detail,\n  \
         kind: {named}FaultKind.{kind},\n  \
         operation: operation,\n\
         );"
    )
}

// ---------------------------------------------------------------------------------------------
// Small, Dart-flavored value rendering, duplicated from `dart_http_client` rather than shared
// with it: this module needs none of its header/query/body-kind machinery, only a type's name.
// ---------------------------------------------------------------------------------------------

/// The message's Dart type: the type the operation named, or the one the macro declared for an
/// operation that named none — mirrors `dart_http_client`'s own `message_dart_typename`.
fn message_dart_typename(operation: &OperationDef) -> String {
    match &operation.inputs {
        OperationInputs::Named(declared) => dart_type_of(declared),
        OperationInputs::Empty | OperationInputs::Generated(_) => {
            operation.generated_message_ident().map_or_else(
                || "dynamic".to_owned(),
                |ident| {
                    let named: Type = syn::parse_quote! { #ident };
                    dart_type_of(&named)
                },
            )
        }
    }
}

/// `ty`'s own Dart type name, read through the same `FieldDef` walk every field's type goes
/// through — mirrors `dart_http_client`'s own `dart_type_of`.
fn dart_type_of(ty: &Type) -> String {
    dart_typename(&get_field_def("value", ty, ""))
}
