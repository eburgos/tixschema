//! The client the service publishes, read off the emitted text.
//!
//! What these prove and what they cannot: the structure of the emitted TypeScript — the spelling of
//! a method, that the transport is named only on the far side of the validation check, that a
//! one-way method answers nothing and publishes the shape it throws instead. No TypeScript
//! toolchain is reachable here, so none of them type-checks the bundle.

use super::{MIXED_SERVICE, client_of};

#[test]
fn a_one_way_method_answers_nothing_and_a_replying_one_answers_its_result() {
    let written = client_of(MIXED_SERVICE);
    assert!(
        written.contains("applyBundle(req: ApplyBundleRequest): Promise<void>;"),
        "got: {written}"
    );
    assert!(
        written.contains(
            "getAvailableBalance(req: AvailableBalanceRequest): \
             Promise<UsageServiceGetAvailableBalanceResult>;"
        ),
        "got: {written}"
    );
}

#[test]
fn every_operation_is_spelled_the_way_a_typescript_caller_types_it() {
    let written = client_of(MIXED_SERVICE);
    for called in [
        "applyBundle",
        "expireCredit",
        "getAvailableBalance",
        "sweep",
    ] {
        assert!(
            written.contains(&format!("  {called}(req:")),
            "got: {written}"
        );
    }
    assert!(
        !written.contains("get_available_balance"),
        "emitting the Rust spelling into TypeScript is as wrong as the reverse. Got: {written}"
    );
}

#[test]
fn the_factory_binds_a_transport_and_answers_with_the_client() {
    let written = client_of(MIXED_SERVICE);
    assert!(
        written.contains(
            "export function createUsageServiceClient(transport: UsageServiceTransport): \
             UsageServiceClient {"
        ),
        "got: {written}"
    );
}

#[test]
fn the_operation_name_travels_beside_the_payload() {
    let written = client_of(MIXED_SERVICE);
    assert!(
        written.contains(
            "return transport.request<UsageServiceGetAvailableBalanceResult>\
             (\"get-available-balance\", "
        ),
        "the name is an argument of its own, never a key inside the message. Got: {written}"
    );
    assert!(
        written.contains("await transport.notify(\"apply-bundle\", "),
        "got: {written}"
    );
}

#[test]
fn the_transport_seam_carries_the_service_name() {
    let written = client_of(MIXED_SERVICE);
    assert!(
        written.contains("export type UsageServiceTransport = {"),
        "got: {written}"
    );
    assert!(
        !written.contains("export type Transport = {"),
        "two services in one bundle would declare it twice. Got: {written}"
    );
}

#[test]
fn a_message_that_fails_its_schema_answers_a_fault_before_the_transport_is_named() {
    let written = client_of(MIXED_SERVICE);
    let method = written
        .split("    async getAvailableBalance(req) {")
        .nth(1)
        .and_then(|rest| rest.split_once("\n    },"))
        .map(|(body, _)| body.to_owned());
    assert!(method.is_some(), "got: {written}");
    let body = method.unwrap();
    let refusal = body.find("isServiceFault: true");
    let reached = body.find("transport.request");
    assert!(refusal.is_some() && reached.is_some(), "got: {body}");
    assert!(
        refusal < reached,
        "the transport is reached only once the message has passed its own validator. \
         Got: {body}"
    );
    assert!(
        body.contains("AvailableBalanceRequest$Schema.safeParse(req)"),
        "got: {body}"
    );
}

#[test]
fn a_refused_one_way_message_is_thrown_because_there_is_no_arm_to_return_it_in() {
    let written = client_of(MIXED_SERVICE);
    let method = written
        .split("    async applyBundle(req) {")
        .nth(1)
        .and_then(|rest| rest.split_once("\n    },"))
        .map(|(body, _)| body.to_owned());
    assert!(method.is_some(), "got: {written}");
    let body = method.unwrap();
    assert!(
        body.contains("throw usageServiceRefused(usageServiceOutboundFault(\"apply-bundle\""),
        "got: {body}"
    );
    assert!(
        body.find("throw") < body.find("transport.notify"),
        "the transport is never reached by a message the client refused. Got: {body}"
    );
}

#[test]
fn the_shape_a_refused_one_way_message_is_thrown_as_is_published() {
    let written = client_of(MIXED_SERVICE);
    assert!(
        written.contains("export type UsageServiceRefusal = Error & { fault: UsageServiceFault };"),
        "what a method throws is part of its surface, not something to read out of its body. \
         Got: {written}"
    );
    assert!(
        written.contains(
            "function usageServiceRefused(fault: UsageServiceFault): UsageServiceRefusal {"
        ),
        "got: {written}"
    );
    assert!(
        !written.contains("): Error {"),
        "annotating the thrower `Error` widens the fault property away again, and a caller that \
         caught one would have nothing to read. Got: {written}"
    );
}

#[test]
fn a_one_way_method_says_in_its_own_documentation_what_it_throws() {
    let written = client_of(MIXED_SERVICE);
    let declared = members_of(&written);
    let documented = declared
        .rsplit_once("  applyBundle(req: ApplyBundleRequest): Promise<void>;")
        .and_then(|(before, _)| before.rsplit_once("  /**"))
        .map(|(_, doc)| doc.to_owned());
    assert!(documented.is_some(), "got: {declared}");
    let block = documented.unwrap();
    assert!(
        block.contains("@throws {UsageServiceRefusal} when the message fails its own schema."),
        "`Promise<void>` has nowhere to say it, so the documentation is the only place left. \
         Got: {block}"
    );
    assert!(
        declared.contains(
            "  /** Calls `get-available-balance` on `UsageService` and waits for the answer. */\n  \
             getAvailableBalance(req:"
        ),
        "an operation with a failure arm answers its refusal into that arm and throws nothing, \
         so it says nothing about throwing. Got: {declared}"
    );
}

#[test]
fn a_service_with_no_one_way_operation_publishes_no_refusal() {
    const REPLYING_ONLY: &str = "
        pub trait UsageService<Ctx> {
            async fn sweep(&self, ctx: &Ctx) -> Result<SweepReport, BalanceError>;
        }
    ";
    let written = client_of(REPLYING_ONLY);
    assert!(
        !written.contains("UsageServiceRefusal"),
        "nothing here can throw, so a type naming what would be thrown is a type nothing \
         produces. Got: {written}"
    );
    assert!(
        written.contains("function usageServiceOutboundFault("),
        "the fault a replying operation answers with is still built the same way. Got: {written}"
    );
}

#[test]
fn the_fault_a_client_builds_names_the_key_that_failed() {
    let written = client_of(MIXED_SERVICE);
    assert!(
        written.contains("function usageServiceOutboundFault("),
        "got: {written}"
    );
    assert!(
        written.contains("kind: \"failed-validation\","),
        "the operation never ran, so this is not one of the errors it declared. Got: {written}"
    );
    assert!(
        written.contains("field: failedAt === \"\" ? undefined : failedAt,"),
        "got: {written}"
    );
}

/// The members of the client type, which is where a method's own documentation is written.
fn members_of(written: &str) -> String {
    written
        .split_once("export type UsageServiceClient = {")
        .and_then(|(_, rest)| rest.split_once("\n};"))
        .map_or_else(String::new, |(declared, _)| declared.to_owned())
}

/// The client's one constructor mints the same way the dispatcher's two do — the fields the Rust
/// declaration published, then the assertion into the sealed type.
#[test]
fn the_fault_the_client_builds_is_minted_from_the_fields_and_sealed() {
    let written = client_of(MIXED_SERVICE);
    assert_eq!(
        written.matches("): UsageServiceFault {").count(),
        1,
        "the client builds a fault in one place: the message it refused. Got: {written}"
    );
    assert!(
        written.find("const built: UsageServiceFaultFields = {")
            < written.find("return built as UsageServiceFault;"),
        "the fields are built, then sealed. Got: {written}"
    );
    assert!(
        !written.contains("usageServiceFaultSeal"),
        "the seal is declared beside the fault, not written into the client: a bundle declaring \
         it twice does not compile. Got: {written}"
    );
}

/// A refusal carries a fault, so a hand-written refusal needs a hand-written fault — which is what
/// the seal stops. The type is unchanged; what it now demands is a value only the generated code
/// can produce.
#[test]
fn the_refusal_a_one_way_method_throws_carries_a_sealed_fault() {
    let written = client_of(MIXED_SERVICE);
    assert!(
        written.contains("export type UsageServiceRefusal = Error & { fault: UsageServiceFault };"),
        "got: {written}"
    );
    assert!(
        written.contains("throw usageServiceRefused(usageServiceOutboundFault("),
        "the thrower is handed a fault the client minted, never one written at the throw site. \
         Got: {written}"
    );
}
