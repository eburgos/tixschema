//! The client the service publishes, read off the emitted text.
//!
//! What these prove and what they cannot: the structure of the emitted TypeScript — the spelling of
//! a method, that the transport is named only on the far side of the validation check, that a
//! one-way method answers nothing. No TypeScript toolchain is reachable here, so none of them type-
//! checks the bundle.

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

#[cfg(feature = "zod")]
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

#[cfg(feature = "zod")]
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

#[cfg(feature = "zod")]
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

#[cfg(not(feature = "zod"))]
#[test]
fn a_build_that_publishes_no_schema_names_none() {
    let written = client_of(MIXED_SERVICE);
    assert!(
        !written.contains("$Schema"),
        "naming a const the bundle does not declare is worse than skipping the check. \
         Got: {written}"
    );
    assert!(
        written.contains("return transport.request<UsageServiceGetAvailableBalanceResult>"),
        "got: {written}"
    );
}
