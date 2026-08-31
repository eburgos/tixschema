//! The emitted bundle put through a real TypeScript compiler.
//!
//! Every other assertion about the published TypeScript in this repository reads strings. This
//! group does not: it writes the bundle a consuming codebase would write, hands it to `tsc
//! --strict`, and reads the verdict. What that settles is the one claim the construct rests on and
//! no string test can reach — an implementation missing a single operation is refused where it
//! reaches the dispatcher factory, and the same implementation with the operation present compiles
//! clean.
//!
//! **Where the compiler comes from.** `tsc` is looked up on `PATH`, or at whatever
//! `TIXSCHEMA_TSC` names. A repository cannot assume one is installed, so a build that finds none
//! stands down rather than failing: the notice below goes to the process's own stderr, which
//! `cargo test` does not capture, so a run that proved nothing here says so on the terminal.
//! `just typecheck-ts` is the entry point that refuses to stand down.
//!
//! **What is checked and what is not.** The bundle names `z` and `ZodType` without importing
//! them — by design, the crate emits no preamble — so this group supplies an ambient declaration
//! of the surface the emitter actually uses. That declaration is a floor, not `zod`: it makes the
//! schema *expressions* well-typed without claiming each one infers its own type. Everything else
//! is checked for real, the interface and the factory included, and neither mentions `zod`.

use super::the_bundle_one_registration_line_produces::{
    audit_seam, author_schemas, bundle, probe_seam,
};
use super::{
    ApplyBundleReceipt, AuditServiceSchema, BalanceRequest, BalanceResponse, CreditWriteError,
    ProbeError, ProbeServiceSchema,
};
use std::env;
use std::env::temp_dir;
use std::fs;
use std::io::Write as _;
use std::io::stderr;
use std::path::PathBuf;
use std::process::{Command, id};
use std::sync::Once;

/// Names the compiler to run, for a machine that has one somewhere other than `PATH`. Set, and a
/// compiler that cannot be started is a failure rather than a stand-down: somebody said where it
/// was.
const COMPILER_VAR: &str = "TIXSCHEMA_TSC";

/// What every check compiles under. `--strict` is the bar a consuming codebase sets; `--pretty
/// false` keeps a diagnostic readable when it lands in an assertion message.
const UNDER: [&str; 10] = [
    "--noEmit", "--strict", "--pretty", "false", "--target", "es2020", "--lib", "es2020",
    "--module", "preserve",
];

/// Said once per test binary when no compiler is reachable.
static STOOD_DOWN: Once = Once::new();

/// The operation the incomplete implementation below leaves out.
#[cfg(feature = "zod")]
const OMITTED: &str = "sweep";

/// The surface of `zod` the emitter actually names, declared globally because the bundle names
/// `z` and `ZodType` without importing them.
///
/// `ZodBuilder` answers `never`, which is assignable into every `ZodType<T>` the bundle annotates
/// a schema with — so a builder chain satisfies its annotation without this declaration having to
/// reimplement zod's inference. `safeParse` is typed exactly as zod types it, which is what makes
/// the dispatcher's `impl.getBalance(ctx, received.data)` a real check rather than one against
/// `unknown`.
#[cfg(feature = "zod")]
const ZOD_SURFACE: &str = "type ZodIssue = { path: ReadonlyArray<PropertyKey>; message: string };

type ZodParsed<Parsed> =
  | { success: true; data: Parsed }
  | { success: false; error: { issues: ReadonlyArray<ZodIssue> } };

declare type ZodType<Parsed> = {
  safeParse(value: unknown): ZodParsed<Parsed>;
};

declare type ZodBuilder = ZodType<never> & {
  int(): ZodBuilder;
};

declare const z: {
  boolean(): ZodBuilder;
  discriminatedUnion(key: string, arms: ReadonlyArray<ZodBuilder>): ZodBuilder;
  literal(value: string): ZodBuilder;
  number(): ZodBuilder;
  string(): ZodBuilder;
  strictObject(shape: Record<string, ZodBuilder>): ZodBuilder;
};
";

/// Everything above the implementation's members. The object literal reaches the factory
/// unannotated and with the context named explicitly, so what refuses an incomplete one is the
/// call rather than an annotation written here.
#[cfg(feature = "zod")]
const IMPLEMENTATION_HEAD: &str = r#"import {
  createProbeServiceDispatcher,
  type ProbeServiceExpireCreditOutcome,
  type ProbeServiceGetBalanceOutcome,
  type ProbeServiceSettleOutcome,
  type ProbeServiceSweepOutcome,
} from "./bundle";

type ProbeContext = { loggerName: string };

export const dispatch = createProbeServiceDispatcher<ProbeContext>({
"#;

#[cfg(feature = "zod")]
const IMPLEMENTATION_TAIL: &str = "});\n";

/// One member per operation the service declares. The incomplete fixture is this list with
/// [`OMITTED`] dropped and nothing else changed, so the two files differ by exactly one member and
/// a slip in either is a slip in both.
#[cfg(feature = "zod")]
const IMPLEMENTATION_MEMBERS: [(&str, &str); 5] = [
    (
        "applyBundle",
        "  async applyBundle(ctx, req): Promise<void> {
    void `${ctx.loggerName}:${req.organizationId}:${req.bundleId}`;
  },
",
    ),
    (
        "expireCredit",
        r#"  async expireCredit(ctx, req): Promise<ProbeServiceExpireCreditOutcome> {
    void `${ctx.loggerName}:${req.organizationId}:${req.creditId}`;
    return { ok: false, error: { errorCode: "conflict" } };
  },
"#,
    ),
    (
        "getBalance",
        "  async getBalance(ctx, req): Promise<ProbeServiceGetBalanceOutcome> {
    void `${ctx.loggerName}:${req.organization_id}`;
    return { ok: true, value: { credits: 1 } };
  },
",
    ),
    (
        "settle",
        "  async settle(ctx, req): Promise<ProbeServiceSettleOutcome> {
    void `${ctx.loggerName}:${req.organization_id}`;
    return { ok: true, value: { applied: true } };
  },
",
    ),
    (
        OMITTED,
        r#"  async sweep(ctx, req): Promise<ProbeServiceSweepOutcome> {
    void `${ctx.loggerName}:${Object.keys(req).length}`;
    return { ok: false, error: { errorCode: "db-error" } };
  },
"#,
    ),
];

/// A caller reading what the client answers with: the value, the operation's own declared error,
/// or the fault behind the literal it narrows on. Nothing here asserts — it compiling at all is
/// what says the published result types narrow the way the design claims.
#[cfg(feature = "zod")]
const CALLER: &str = r#"import {
  createProbeServiceClient,
  type ProbeServiceFaultKind,
  type ProbeServiceTransport,
} from "./bundle";

const transport: ProbeServiceTransport = {
  async notify(operation, payload): Promise<void> {
    void `${operation}:${JSON.stringify(payload)}`;
  },
  async request<Answered>(operation: string, payload: unknown): Promise<Answered> {
    throw new Error(`${operation}:${JSON.stringify(payload)}`);
  },
};

export async function read(): Promise<string> {
  const answered = await createProbeServiceClient(transport).getBalance({
    organization_id: "acme",
  });
  if (answered.ok) {
    const credits: number = answered.value.credits;
    return `${credits}`;
  }
  if ("isServiceFault" in answered.error) {
    const kind: ProbeServiceFaultKind = answered.error.fault.kind;
    const field: string | undefined = answered.error.fault.field;
    return `${kind}:${field ?? ""}:${answered.error.fault.operation}`;
  }
  return answered.error.errorCode;
}
"#;

/// The implementation fixture, with every named operation but the ones listed as left out.
#[cfg(feature = "zod")]
fn implementation(without: &[&str]) -> String {
    let mut written = String::from(IMPLEMENTATION_HEAD);
    for (named, member) in IMPLEMENTATION_MEMBERS {
        if !without.contains(&named) {
            written.push_str(member);
        }
    }
    written.push_str(IMPLEMENTATION_TAIL);
    written
}

/// Said on the process's own stderr rather than through `eprintln!`, which `cargo test` captures
/// and only shows for a test that failed. A stand-down is a pass that proved nothing, so it has to
/// be visible on a run where everything passed.
fn stand_down() {
    STOOD_DOWN.call_once(|| {
        let notice = format!(
            "\ntixschema: no TypeScript compiler is reachable, so the emitted bundle was NOT \
             type-checked.\n  The `service_schema` type-check group stood down. Put `tsc` on PATH, \
             or name one in {COMPILER_VAR}, and run `just typecheck-ts`, which refuses to stand \
             down.\n\n"
        );
        drop(stderr().write_all(notice.as_bytes()));
    });
}

/// A directory of its own per check, named for the check and the process, so combinations running
/// one after another and tests running beside each other never share a file.
fn workspace(named: &str) -> PathBuf {
    let at = temp_dir().join(format!("tixschema-typecheck-{named}-{run}", run = id()));
    if at.exists() {
        fs::remove_dir_all(&at).unwrap();
    }
    fs::create_dir_all(&at).unwrap();
    at
}

/// Writes the named files into a workspace of their own, compiles them together, and answers
/// whether the compiler accepted them and everything it reported.
///
/// `None` says no compiler was reachable and nothing was compiled — never that a compile passed.
fn compiled(named: &str, files: &[(&str, String)]) -> Option<(bool, String)> {
    let named_compiler = env::var(COMPILER_VAR).ok();
    let compiler = named_compiler.clone().unwrap_or_else(|| "tsc".to_owned());
    let at = workspace(named);
    for (called, written) in files {
        fs::write(at.join(called), written).unwrap();
    }
    let run = Command::new(&compiler)
        .args(UNDER)
        .args(files.iter().map(|(called, _)| *called))
        .current_dir(&at)
        .output();
    fs::remove_dir_all(&at).unwrap();
    let Ok(reported) = run else {
        assert!(
            named_compiler.is_none(),
            "{COMPILER_VAR} names `{compiler}`, and no compiler could be started there: {}",
            run.unwrap_err()
        );
        stand_down();
        return None;
    };
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&reported.stdout),
        String::from_utf8_lossy(&reported.stderr)
    );
    Some((reported.status.success(), said))
}

/// The bundle beside whatever it needs in order to be read at all, which is the file set every
/// check starts from: an ambient declaration of the `zod` surface where the build publishes
/// schemas, and nothing whatever where it does not.
#[cfg(feature = "zod")]
fn bundled(written: String) -> Vec<(&'static str, String)> {
    vec![("zod.d.ts", ZOD_SURFACE.to_owned()), ("bundle.ts", written)]
}

/// The other half: a bundle written by a build with no schema surface names nothing it does not
/// itself declare, so it is compiled alone. Nothing supplies it a preamble, because the claim is
/// that it needs none.
#[cfg(not(feature = "zod"))]
fn bundled(written: String) -> Vec<(&'static str, String)> {
    vec![("bundle.ts", written)]
}

/// The bundle a consuming codebase writes, compiled.
///
/// This is what the structural checks in the file beside this one cannot do: a bundle whose
/// emitted text is well-formed to a reader and rejected by a parser reads identically to one that
/// compiles. It runs in every build that writes TypeScript, and in a build with no schema surface
/// the bundle is handed to the compiler entirely on its own — nothing declares a name for it, so
/// a clean compile is also what says it carries no unresolved one.
#[test]
fn the_bundle_a_consuming_codebase_writes_compiles_under_strict() {
    let Some((accepted, said)) = compiled("bundle", &bundled(bundle())) else {
        return;
    };
    assert!(accepted, "the emitted bundle does not compile:\n{said}");
}

/// Two services in one flat file, compiled. The check beside this one reads the declared names and
/// compares them for duplicates; this one asks the compiler, which also sees a collision between a
/// name one service declares and one the other's generated code refers to.
#[test]
fn two_services_in_one_bundle_compile_together() {
    let mut both = vec![
        BalanceRequest::ts_definition(),
        BalanceResponse::ts_definition(),
        ApplyBundleReceipt::ts_definition(),
        ProbeError::ts_definition(),
        CreditWriteError::ts_definition(),
    ];
    both.extend(author_schemas());
    both.push(ProbeServiceSchema::ts_definition());
    both.extend(probe_seam());
    both.push(AuditServiceSchema::ts_definition());
    both.extend(audit_seam());
    let Some((accepted, said)) = compiled("two-services", &bundled(both.join("\n\n"))) else {
        return;
    };
    assert!(
        accepted,
        "a bundle carrying two services does not compile:\n{said}"
    );
}

/// The positive half of the seal, which no string test can give: an implementation that answers
/// every operation is accepted where it reaches the dispatcher factory, and a caller narrows what
/// the client answers with.
///
/// Compiled together, so the run that says the incomplete implementation below is refused is a run
/// against a file set that is otherwise known to compile.
#[cfg(feature = "zod")]
#[test]
fn a_complete_implementation_is_accepted_at_the_factory_call() {
    let mut files = bundled(bundle());
    files.push(("implementation.ts", implementation(&[])));
    files.push(("caller.ts", CALLER.to_owned()));
    let Some((accepted, said)) = compiled("complete", &files) else {
        return;
    };
    assert!(
        accepted,
        "an implementation answering every operation, and a caller reading what the client \
         answers with, do not compile against the bundle:\n{said}"
    );
}

/// The claim the whole construct rests on, settled by a compiler rather than read off a string:
/// an implementation missing one operation does not reach the dispatcher factory.
///
/// The file compiled here is the accepted one above with `sweep` dropped and nothing else changed,
/// so a refusal for any other reason is a different diagnostic. Recorded verbatim from tsc 7.0.2
/// under `--strict`:
///
/// ```text
/// implementation.ts(11,68): error TS2741: Property 'sweep' is missing in type '{ applyBundle(ctx: ProbeContext, req: ApplyBundleRequest): Promise<void>; expireCredit(ctx: ProbeContext, req: ExpireCreditRequest): Promise<...>; getBalance(ctx: ProbeContext, req: BalanceRequest): Promise<...>; settle(ctx: ProbeContext, req: BalanceRequest): Promise<...>; }' but required in type 'ProbeServiceImpl<ProbeContext>'.
/// ```
///
/// The assertions read the member's name and the interface's out of that text rather than the
/// error code, so the refusal has to be about the operation left out — a fixture that failed to
/// compile for some unrelated reason names neither.
#[cfg(feature = "zod")]
#[test]
fn an_implementation_missing_one_operation_is_refused_at_the_factory_call() {
    let mut files = bundled(bundle());
    files.push(("implementation.ts", implementation(&[OMITTED])));
    let Some((accepted, said)) = compiled("incomplete", &files) else {
        return;
    };
    assert!(
        !accepted,
        "an implementation answering four of five operations reached \
         `createProbeServiceDispatcher` and the compiler allowed it:\n{said}"
    );
    assert!(
        said.contains(OMITTED),
        "the refusal has to name the operation left out; a fixture refused for some other reason \
         would not. Got:\n{said}"
    );
    assert!(
        said.contains("is missing") && said.contains("ProbeServiceImpl"),
        "the refusal has to be a member missing from the service's own interface. Got:\n{said}"
    );
}
