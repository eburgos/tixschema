//! The interface an implementation satisfies and the dispatcher factory that drives one, read off
//! the emitted text.
//!
//! What these prove and what they cannot: that every member is required, that no index signature is
//! written, that an implementation's return names no fault. That an implementation *missing* a
//! member is refused at the factory call is a claim only a TypeScript compiler can settle, and none
//! is reachable from this repository.

use super::{MIXED_SERVICE, service_of};

#[test]
fn an_implementation_answers_an_outcome_that_has_no_fault_in_it() {
    let written = service_of(MIXED_SERVICE);
    let found = written
        .split("export type UsageServiceGetAvailableBalanceOutcome =")
        .nth(1)
        .and_then(|rest| rest.split_once("\n\n"))
        .map(|(body, _)| body.to_owned());
    assert!(found.is_some(), "got: {written}");
    let outcome = found.unwrap();
    assert!(
        outcome.contains("| { ok: false; error: BalanceError }"),
        "got: {outcome}"
    );
    assert!(
        !outcome.contains("isServiceFault"),
        "a service that could name the member could fabricate the value. Got: {outcome}"
    );
}

#[test]
fn every_member_is_required_and_nothing_lets_a_partial_implementation_through() {
    let written = service_of(MIXED_SERVICE);
    let found = written
        .split("export interface UsageServiceImpl<Ctx> {")
        .nth(1)
        .and_then(|rest| rest.split_once("\n}"))
        .map(|(body, _)| body.to_owned());
    assert!(found.is_some(), "got: {written}");
    let members = found.unwrap();
    assert!(
        !members.contains("?("),
        "an optional member is a method an implementation may omit. Got: {members}"
    );
    assert!(
        !members.contains("[key:"),
        "an index signature admits anything and checks nothing. Got: {members}"
    );
    assert_eq!(
        members.matches("(ctx: Ctx, req: ").count(),
        4,
        "one required member per operation, no more and no fewer. Got: {members}"
    );
}

#[test]
fn the_context_comes_first_on_every_method_and_reaches_no_message() {
    let written = service_of(MIXED_SERVICE);
    assert!(
        written.contains("export interface UsageServiceImpl<Ctx> {"),
        "got: {written}"
    );
    assert!(
        written.contains(
            "getAvailableBalance(ctx: Ctx, req: AvailableBalanceRequest): \
             Promise<UsageServiceGetAvailableBalanceOutcome>;"
        ),
        "got: {written}"
    );
    assert!(
        written.contains("applyBundle(ctx: Ctx, req: ApplyBundleRequest): Promise<void>;"),
        "got: {written}"
    );
}

#[test]
fn the_dispatcher_factory_answers_with_a_dispatch_function() {
    let written = service_of(MIXED_SERVICE);
    assert!(
        written.contains("export function createUsageServiceDispatcher<Ctx>("),
        "got: {written}"
    );
    assert!(
        written
            .contains("): (ctx: Ctx, operation: string, payload: unknown) => Promise<unknown> {"),
        "got: {written}"
    );
}

#[test]
fn the_operation_is_matched_from_the_argument_beside_the_payload() {
    let written = service_of(MIXED_SERVICE);
    assert!(written.contains("switch (operation) {"), "got: {written}");
    for wire in [
        "\"apply-bundle\"",
        "\"expire-credit\"",
        "\"get-available-balance\"",
        "\"sweep\"",
    ] {
        assert!(
            written.contains(&format!("case {wire}: {{")),
            "got: {written}"
        );
    }
}

#[test]
fn a_one_way_arm_answers_nothing_at_all() {
    let written = service_of(MIXED_SERVICE);
    assert!(
        written.contains("await impl.applyBundle(ctx, ") && written.contains("return undefined;"),
        "got: {written}"
    );
}

#[test]
fn an_operation_nothing_answers_to_produces_a_framed_fault() {
    let written = service_of(MIXED_SERVICE);
    assert!(
        written.contains("return usageServiceFramed(usageServiceUnknownOperation(operation));"),
        "got: {written}"
    );
    assert!(
        written.contains("kind: \"unknown-operation\","),
        "got: {written}"
    );
    assert!(
        written.contains("return { ok: false, error: { isServiceFault: true, fault } };"),
        "a fault crosses inside the failure arm, behind the literal a caller narrows on. \
         Got: {written}"
    );
}

#[test]
fn the_payload_is_parsed_before_the_implementation_is_called() {
    let written = service_of(MIXED_SERVICE);
    let arm = written
        .split("      case \"get-available-balance\": {")
        .nth(1)
        .and_then(|rest| rest.split_once("\n      }"))
        .map(|(body, _)| body.to_owned());
    assert!(arm.is_some(), "got: {written}");
    let body = arm.unwrap();
    assert!(
        body.find("AvailableBalanceRequest$Schema.safeParse(payload)")
            < body.find("impl.getAvailableBalance"),
        "an implementation may assume its message is valid because an invalid one never \
         reaches it. Got: {body}"
    );
}

#[test]
fn a_payload_that_was_never_the_message_is_told_apart_from_one_that_failed_a_key() {
    let written = service_of(MIXED_SERVICE);
    assert!(
        written.contains(
            "kind: failedAt === \"\" ? \"undeserializable-payload\" : \"failed-validation\","
        ),
        "got: {written}"
    );
}
