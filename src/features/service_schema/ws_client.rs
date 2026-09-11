//! The TypeScript `ws_rpc` transport: a socket seam a platform `WebSocket` satisfies as it is, a
//! heartbeat, and a check of every reply against the operation's own declared schemas.
//!
//! # The seam is structural, not an adapter
//!
//! [`socket_type`] names four members every platform `WebSocket` already has (`send`, `close`,
//! `addEventListener`/`removeEventListener` for `"message"` and `"close"`), so `new WebSocket(url)`
//! plugs into [`transport_factory`] with nothing written in between. Nothing generated here names
//! `WebSocket` itself.
//!
//! # One socket, one transport, one heartbeat
//!
//! The returned transport owns a per-socket request counter and a correlation map keyed by frame
//! id, exactly as [`super::client`]'s own AMQP-shaped transport is bound to reach an
//! application-supplied one; the difference here is that this transport also owns the liveness
//! probe, since nothing upstream of a raw socket does it for it. A `ping` goes out every
//! `heartbeat.intervalMs` (default 30 s), a `pong` re-arms the next one, and a socket that misses
//! one within `heartbeat.timeoutMs` (default 10 s) is closed through the seam's own `close()` —
//! which settles every request still waiting with a `transport-failure` fault, the same fault every
//! other close reaches for.
//!
//! # Every reply is checked before a caller sees it
//!
//! A generated client already validates what it is about to send; this transport is what validates
//! what comes back. [`schemas_table`] builds one lookup per direction from the service's
//! request-and-reply operations, keyed by the operation's own wire name, and the transport's
//! `checked` reader parses a success `value` or a declared `error` against it — a mismatch becomes
//! a `failed-validation` fault naming the first offending key, through [`issues_fault_fn`].
//!
//! # Gated with the schemas it reads
//!
//! Every table entry and every check reaches for a message's own `$Schema` const, so this module is
//! emitted only where [`crate::features::service_schema::seam`] is: a build with no Zod surface has
//! nothing to check a reply against.

use super::fault;
use crate::field_type::get_field_def;
use crate::rename_rule::RenameRule;
use crate::service_schema::parse::{OperationDef, OperationOutcome, ServiceDef, is_unit_type};
use core::fmt::Write as _;
use syn::Type;

pub fn emit(service: &ServiceDef) -> Vec<String> {
    let named = service.ident.to_string();
    vec![
        socket_type(&named),
        options_type(&named),
        schemas_table(service, "Success", success_type),
        schemas_table(service, "Error", error_type),
        fault_fn(service),
        issues_fault_fn(service),
        transport_factory(service),
    ]
}

/// `None` for a one-way operation (no reply to check) and for a unit success: the envelope carries
/// a unit success as `ok` alone, with no `value` to parse, so a table entry there would fail every
/// valid reply against a schema nothing on the wire is meant to satisfy.
fn success_type(operation: &OperationDef) -> Option<&Type> {
    match &operation.outcome {
        OperationOutcome::Reply { success, .. } if !is_unit_type(success) => Some(success),
        OperationOutcome::Reply { .. } | OperationOutcome::OneWay => None,
    }
}

fn error_type(operation: &OperationDef) -> Option<&Type> {
    match &operation.outcome {
        OperationOutcome::Reply { error, .. } => Some(error),
        OperationOutcome::OneWay => None,
    }
}

// ---------------------------------------------------------------------------------------------
// The socket seam and the heartbeat options
// ---------------------------------------------------------------------------------------------

/// The socket seam: four members every platform `WebSocket` already has, so a browser's
/// `new WebSocket(url)` or the Node `ws` package's socket plugs in with no adapter. `close` is what
/// the transport calls on a socket that stopped answering its heartbeat.
fn socket_type(named: &str) -> String {
    format!(
        "/**\n \
         * What binds a `{named}` socket transport to a socket, in plain terms: the four members \
         a\n \
         * platform `WebSocket` already has, so a browser's `new WebSocket(url)` or the Node \
         `ws`\n \
         * package's socket is passed in as it is. `close` is what the transport calls on a \
         socket\n \
         * that stopped answering its heartbeat.\n \
         */\n\
         export type {named}WsSocket = {{\n  \
         send(text: string): void;\n  \
         close(): void;\n  \
         addEventListener(type: \"message\", listener: (event: {{ data: unknown }}) => void): \
         void;\n  \
         addEventListener(type: \"close\", listener: () => void): void;\n  \
         removeEventListener(type: \"message\", listener: (event: {{ data: unknown }}) => void): \
         void;\n  \
         removeEventListener(type: \"close\", listener: () => void): void;\n\
         }};"
    )
}

/// How often the transport probes the socket, and how long it waits for the answer; `false` turns
/// the probe off.
fn options_type(named: &str) -> String {
    format!(
        "/** How often the transport probes the socket, and how long it waits for the answer. */\n\
         export type {named}WsOptions = {{\n  \
         heartbeat?: {{ intervalMs: number; timeoutMs: number }} | false;\n\
         }};"
    )
}

// ---------------------------------------------------------------------------------------------
// The schema tables the transport checks a reply against
// ---------------------------------------------------------------------------------------------

/// One table, keyed by wire name, naming the schema a reply for that operation must parse as. Built
/// from the service's request-and-reply operations only — a one-way operation has no reply to
/// check, so `pick` answers it `None` and it contributes no entry. Indexed by a plain `string` at
/// the call site, so the type carries its own index signature rather than the narrower literal type
/// an object initializer would otherwise infer.
fn schemas_table(
    service: &ServiceDef,
    direction: &str,
    pick: impl Fn(&OperationDef) -> Option<&Type>,
) -> String {
    let named = service.ident.to_string();
    let prefix = RenameRule::CamelCase.apply_to_variant(&named);
    let field = if direction == "Success" {
        "value"
    } else {
        "error"
    };
    let mut entries = String::new();
    for operation in &service.operations {
        let Some(ty) = pick(operation) else {
            continue;
        };
        let schema = get_field_def(field, ty, "").zod_type();
        let wire = &operation.wire_name;
        let _ = writeln!(entries, "  \"{wire}\": {schema},");
    }
    let body = if entries.is_empty() {
        String::new()
    } else {
        format!("\n{entries}")
    };
    format!(
        "const {prefix}{direction}Schemas: {{ readonly [operation: string]: ZodType<unknown> | \
         undefined }} = {{{body}}};"
    )
}

// ---------------------------------------------------------------------------------------------
// The fault builders the transport reaches for
// ---------------------------------------------------------------------------------------------

/// Builds a `{named}Fault` of the given kind, sealed the way every other generated constructor
/// mints one. The transport reaches for this directly where the kind is already known — a closed
/// socket, always `transport-failure` — and [`issues_fault_fn`] reaches for it once it has folded a
/// failed Zod parse into one.
fn fault_fn(service: &ServiceDef) -> String {
    let named = service.ident.to_string();
    let prefix = RenameRule::CamelCase.apply_to_variant(&named);
    let kind_ty = format!("{named}FaultKind");
    format!(
        "/**\n \
         * Builds a `{named}Fault` of the given kind. Used directly where the kind is already \
         known\n \
         * (a closed socket, always `transport-failure`), and by `{prefix}IssuesFault` for the \
         one\n \
         * kind a failed Zod parse reports.\n \
         */\n\
         function {prefix}Fault(\n  \
         kind: {kind_ty},\n  \
         operation: string,\n  \
         detail: string,\n  \
         field?: string,\n\
         ): {named}Fault {{\n\
         {minted}\n\
         }}",
        minted = fault::minted(&named, "    detail,\n    field,\n    kind,\n    operation,")
    )
}

/// Folds a failed Zod parse into one `failed-validation` fault naming the first failing key — the
/// same folding the generated client and dispatcher already do for a message that failed outbound
/// or inbound validation.
fn issues_fault_fn(service: &ServiceDef) -> String {
    let named = service.ident.to_string();
    let prefix = RenameRule::CamelCase.apply_to_variant(&named);
    format!(
        "/**\n \
         * The fault a reply produces when it will not become the operation's own declared success \
         or\n \
         * error: a server that answered the wrong shape. Names the first failing key.\n \
         */\n\
         function {prefix}IssuesFault(\n  \
         operation: string,\n  \
         issues: ReadonlyArray<{{ path: ReadonlyArray<PropertyKey>; message: string }}>,\n\
         ): {named}Fault {{\n  \
         const [first] = issues;\n  \
         const failedAt = first === undefined ? \"\" : first.path.join(\".\");\n\
         {minted}\n\
         }}",
        minted = fault::minted(
            &named,
            "    detail: issues\n      \
             .map((issue) =>\n        \
             issue.path.length === 0 ? issue.message : `'${issue.path.join(\".\")}': \
             ${issue.message}`,\n      \
             )\n      \
             .join(\"; \"),\n    \
             field: failedAt === \"\" ? undefined : failedAt,\n    \
             kind: \"failed-validation\",\n    \
             operation,"
        )
    )
}

// ---------------------------------------------------------------------------------------------
// The transport factory
// ---------------------------------------------------------------------------------------------

/// Binds a `{named}` transport to one socket for its life: writes a `request` frame and settles on
/// the matching `reply`, after checking it against the operation's own schema; writes a `notify`
/// frame and settles at once; answers an inbound `ping` with a `pong` and treats an inbound `pong`
/// as re-arming the next one; probes the socket on `heartbeat.intervalMs` and closes it on a missed
/// `heartbeat.timeoutMs`; settles every request still waiting with a `transport-failure` fault on
/// close, whether the socket closed on its own or `close()` was called.
///
/// Composed from the pieces below in the order a reader meets them: the header opens the function
/// and declares the correlation map and the heartbeat state, `settleAll` and `checked` are the two
/// readers the wiring below reaches for, `onMessage` and `onClose` are that wiring, and the
/// returned object is the transport itself.
fn transport_factory(service: &ServiceDef) -> String {
    let named = service.ident.to_string();
    let prefix = RenameRule::CamelCase.apply_to_variant(&named);
    format!(
        "{header}{state}{settle_all}{checked}{on_message}{on_close}{returned}}}",
        header = transport_header(&named),
        state = heartbeat_state_stmt(&named),
        settle_all = settle_all_stmt(&prefix),
        checked = checked_reader_stmt(&prefix),
        on_message = on_message_stmt(),
        on_close = on_close_stmt(),
        returned = returned_transport_stmt(),
    )
}

/// The factory's own `JSDoc`, its signature, and the opening brace of its body.
fn transport_header(named: &str) -> String {
    format!(
        "/**\n \
         * A `{named}` transport over one socket. `request` writes a `request` frame and settles \
         on\n \
         * the `reply` frame carrying the same `id`, after checking the reply against the \
         operation's\n \
         * own declared success or error schema; `notify` writes a `notify` frame and settles at \
         once.\n \
         * A `ping` every `heartbeat.intervalMs` expects a `pong` within `heartbeat.timeoutMs`, and \
         a\n \
         * socket that misses one is closed. A request still waiting when the socket closes settles\n \
         * with a `transport-failure` fault in its failure arm, so no caller hangs.\n \
         */\n\
         export function create{named}WsTransport(\n  \
         socket: {named}WsSocket,\n  \
         options: {named}WsOptions = {{}},\n\
         ): {named}Transport & {{ close(): void }} {{\n"
    )
}

/// The transport's own state: the wire's `service` name, the resolved heartbeat options, the
/// per-transport request counter and its correlation map, and the two heartbeat timers with the
/// readers that stop and (re)schedule them.
fn heartbeat_state_stmt(named: &str) -> String {
    format!(
        "  const service = \"{named}\";\n  \
         const heartbeat = options.heartbeat === undefined ? {{ intervalMs: 30_000, timeoutMs: \
         10_000 }} : options.heartbeat;\n  \
         let next = 0;\n  \
         const pending = new Map<string, {{ operation: string; settle: (answered: unknown) => \
         void }}>();\n  \
         let nextPing: ReturnType<typeof setTimeout> | undefined;\n  \
         let pongDeadline: ReturnType<typeof setTimeout> | undefined;\n  \
         const stopHeartbeat = () => {{\n    \
         if (nextPing !== undefined) clearTimeout(nextPing);\n    \
         if (pongDeadline !== undefined) clearTimeout(pongDeadline);\n    \
         nextPing = pongDeadline = undefined;\n  \
         }};\n  \
         const schedulePing = () => {{\n    \
         if (heartbeat === false) return;\n    \
         nextPing = setTimeout(() => {{\n      \
         socket.send(JSON.stringify({{ kind: \"ping\" }}));\n      \
         pongDeadline = setTimeout(() => socket.close(), heartbeat.timeoutMs);\n    \
         }}, heartbeat.intervalMs);\n  \
         }};\n"
    )
}

/// Fails every request still waiting, each with a `transport-failure` fault carrying the given
/// detail — reached on the socket's own close and on the transport's own `close()`, with a
/// different detail each time.
fn settle_all_stmt(prefix: &str) -> String {
    format!(
        "  const settleAll = (detail: string) => {{\n    \
         for (const [id, waiting] of pending) {{\n      \
         pending.delete(id);\n      \
         waiting.settle({{ ok: false, error: {{ isServiceFault: true, fault: {prefix}Fault(\
         \"transport-failure\", waiting.operation, detail) }} }});\n    \
         }}\n  \
         }};\n"
    )
}

/// Checks a reply's envelope against the operation's own declared schema before a caller sees it: a
/// success `value` against the success table, a declared error against the error table — a fault
/// already sealed by the far side (`isServiceFault`) passes through unchecked, since it is not a
/// declared error to validate against. A mismatch on either side becomes a `failed-validation`
/// fault through [`issues_fault_fn`].
fn checked_reader_stmt(prefix: &str) -> String {
    format!(
        "  const checked = (operation: string, envelope: Record<string, unknown>): unknown => {{\n    \
         if (envelope.ok === true) {{\n      \
         const parsed = {prefix}SuccessSchemas[operation]?.safeParse(envelope.value);\n      \
         if (parsed !== undefined && !parsed.success) {{\n        \
         return {{ ok: false, error: {{ isServiceFault: true, fault: {prefix}IssuesFault(operation, \
         parsed.error.issues) }} }};\n      \
         }}\n      \
         return envelope;\n    \
         }}\n    \
         const error = envelope.error;\n    \
         if (typeof error === \"object\" && error !== null && \"isServiceFault\" in error) return \
         envelope;\n    \
         const parsed = {prefix}ErrorSchemas[operation]?.safeParse(error);\n    \
         if (parsed !== undefined && !parsed.success) {{\n      \
         return {{ ok: false, error: {{ isServiceFault: true, fault: {prefix}IssuesFault(operation, \
         parsed.error.issues) }} }};\n    \
         }}\n    \
         return envelope;\n  \
         }};\n"
    )
}

/// The socket's `"message"` listener: reads one JSON frame, answers a `ping` with a `pong` and
/// treats a `pong` as re-arming the next one, drops a frame naming another service or one nothing
/// is waiting on, and otherwise settles the waiting request with the checked envelope. Non-JSON
/// text and a frame that is not an object are both dropped at the top.
fn on_message_stmt() -> String {
    "  const onMessage = (event: { data: unknown }) => {\n    \
     let frame: unknown;\n    \
     try { frame = JSON.parse(String(event.data)); } catch { return; }\n    \
     if (typeof frame !== \"object\" || frame === null) return;\n    \
     const { kind, id, service: named, ...envelope } = frame as Record<string, unknown>;\n    \
     if (kind === \"ping\") { socket.send(JSON.stringify({ kind: \"pong\" })); return; }\n    \
     if (kind === \"pong\") {\n      \
     if (pongDeadline !== undefined) clearTimeout(pongDeadline);\n      \
     pongDeadline = undefined;\n      \
     schedulePing();\n      \
     return;\n    \
     }\n    \
     if (kind !== \"reply\" || named !== service || typeof id !== \"string\") return;\n    \
     const waiting = pending.get(id);\n    \
     if (waiting === undefined) return;\n    \
     pending.delete(id);\n    \
     waiting.settle(checked(waiting.operation, envelope));\n  \
     };\n"
        .to_owned()
}

/// The socket's `"close"` listener, and the wiring that starts the transport listening and probing
/// the moment it is built.
fn on_close_stmt() -> String {
    "  const onClose = () => {\n    \
     stopHeartbeat();\n    \
     settleAll(\"the socket closed before the reply arrived\");\n  \
     };\n  \
     socket.addEventListener(\"message\", onMessage);\n  \
     socket.addEventListener(\"close\", onClose);\n  \
     schedulePing();\n"
        .to_owned()
}

/// The transport itself: `notify` writes a one-way frame, `request` writes a request frame and
/// waits on the correlation map, `close` tears down the wiring and fails everything still waiting.
fn returned_transport_stmt() -> String {
    "  return {\n    \
     async notify(operation, payload) {\n      \
     socket.send(JSON.stringify({ kind: \"notify\", service, operation, payload }));\n    \
     },\n    \
     request<Answered>(operation: string, payload: unknown): Promise<Answered> {\n      \
     const id = String(++next);\n      \
     return new Promise<Answered>((resolve) => {\n        \
     pending.set(id, { operation, settle: (answered) => resolve(answered as Answered) });\n        \
     socket.send(JSON.stringify({ kind: \"request\", id, service, operation, payload }));\n      \
     });\n    \
     },\n    \
     close() {\n      \
     stopHeartbeat();\n      \
     socket.removeEventListener(\"message\", onMessage);\n      \
     socket.removeEventListener(\"close\", onClose);\n      \
     settleAll(\"the transport was closed before the reply arrived\");\n    \
     },\n  \
     };\n"
        .to_owned()
}
