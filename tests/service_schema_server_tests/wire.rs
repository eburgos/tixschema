//! What the server macro puts on the queue, asserted against what a caller on this bus has always
//! read: every vector the wire framing carries, built from `serde_json::json!` literals rather
//! than from any one service's own message types, since the framing reads structurally and names
//! none.

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::amqp_server::{framed_fault, legacy_reply};

    const CORRELATION: &str = "req-12345";

    fn answered(envelope: &Value) -> Value {
        legacy_reply(envelope, Some(CORRELATION))
    }

    fn faulted(fault: &Value) -> Value {
        legacy_reply(&framed_fault(fault), Some(CORRELATION))
    }

    #[test]
    fn a_success_that_is_already_a_bus_message_crosses_as_it_was_built() {
        assert_eq!(
            answered(&json!({
                "ok": true,
                "value": { "type": "response", "correlationId": CORRELATION },
            })),
            json!({ "type": "response", "correlationId": CORRELATION }),
        );
    }

    #[test]
    fn a_value_crosses_with_every_field_the_caller_reads() {
        assert_eq!(
            answered(&json!({
                "ok": true,
                "value": {
                    "type": "response",
                    "correlationId": CORRELATION,
                    "organizationId": "acme",
                    "availableCredits": [{
                        "creditId": "64de3d95ff45b119e5b53ad1",
                        "isPostPaid": false,
                        "priority": 0_i32,
                        "remainingCredits": 4_250_i32,
                    }],
                },
            })),
            json!({
                "type": "response",
                "correlationId": CORRELATION,
                "organizationId": "acme",
                "availableCredits": [{
                    "creditId": "64de3d95ff45b119e5b53ad1",
                    "isPostPaid": false,
                    "priority": 0_i32,
                    "remainingCredits": 4_250_i32,
                }],
            }),
        );
    }

    #[test]
    fn a_success_that_was_never_a_bus_message_gains_the_marker_a_caller_narrows_on() {
        assert_eq!(
            answered(&json!({ "ok": true, "value": { "creditId": "64de3d95ff45b119e5b53ad1" } })),
            json!({
                "type": "response",
                "creditId": "64de3d95ff45b119e5b53ad1",
                "correlationId": CORRELATION,
            }),
        );
    }

    #[test]
    fn a_declared_error_crosses_with_the_code_and_message_a_caller_branches_on() {
        assert_eq!(
            answered(&json!({
                "ok": false,
                "error": {
                    "errorCode": "insufficient-balance",
                    "errorMessage":
                        "generation request needs 3 credits, but organization has 0 credits available",
                },
            })),
            json!({
                "type": "error",
                "errorCode": "insufficient-balance",
                "errorMessage":
                    "generation request needs 3 credits, but organization has 0 credits available",
                "isError": true,
                "correlationId": CORRELATION,
            }),
        );
    }

    #[test]
    fn an_error_naming_no_code_falls_back_to_the_one_the_runtime_has_always_sent() {
        assert_eq!(
            answered(&json!({ "ok": false, "error": { "errorMessage": "Forbidden" } })),
            json!({
                "type": "error",
                "errorCode": "server-error",
                "errorMessage": "Forbidden",
                "isError": true,
                "correlationId": CORRELATION,
            }),
        );
    }

    #[test]
    fn a_message_that_failed_its_own_schema_comes_back_naming_the_field() {
        assert_eq!(
            faulted(&json!({
                "detail": "expected number, received string",
                "field": "creditCount",
                "kind": "failed-validation",
                "operation": "usage-generation-request",
            })),
            json!({
                "type": "invalid-request",
                "errors": [{
                    "errorCode": "failed-validation",
                    "message": "'creditCount': expected number, received string",
                }],
                "correlationId": CORRELATION,
            }),
        );
    }

    #[test]
    fn an_operation_nothing_answers_to_comes_back_rather_than_going_unanswered() {
        assert_eq!(
            faulted(&json!({
                "detail": "the service answers to no operation by that name",
                "kind": "unknown-operation",
                "operation": "expire-generation-credit",
            })),
            json!({
                "type": "invalid-request",
                "errors": [{
                    "errorCode": "unknown-operation",
                    "message": "the service answers to no operation by that name",
                }],
                "correlationId": CORRELATION,
            }),
        );
    }

    #[test]
    fn a_fault_is_never_mistaken_for_an_error_the_operation_declared() {
        let refused = faulted(&json!({
            "detail": "invalid type: string, expected u32",
            "kind": "undeserializable-payload",
            "operation": "add-generation-credit",
        }));

        assert_eq!(refused["type"], json!("invalid-request"));
        assert_eq!(refused.get("isError"), None);
    }

    #[test]
    fn a_reply_with_no_correlation_carries_none() {
        assert_eq!(
            legacy_reply(
                &json!({ "ok": true, "value": { "type": "response" } }),
                None
            ),
            json!({ "type": "response" }),
        );
    }

    #[test]
    fn an_answer_that_is_no_message_is_reported_rather_than_dropped() {
        assert_eq!(
            answered(&json!("not a message")),
            json!({
                "type": "invalid-request",
                "errors": [{
                    "errorCode": "undeserializable-payload",
                    "message": "the service answered with no message",
                }],
                "correlationId": CORRELATION,
            }),
        );
    }

    #[test]
    fn a_failure_arm_carrying_no_error_is_reported_rather_than_dropped() {
        assert_eq!(
            answered(&json!({ "ok": false })),
            json!({
                "type": "invalid-request",
                "errors": [{
                    "errorCode": "undeserializable-payload",
                    "message": "the service answered with no error",
                }],
                "correlationId": CORRELATION,
            }),
        );
    }
}
