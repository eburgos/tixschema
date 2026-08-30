//! The `ServiceFault` TypeScript type: what comes back when the failure is not one the operation
//! declared.
//!
//! Written as a literal rather than derived from anything, because there is nothing to derive it
//! from — a fault is the same four cases for every service, and the Rust side that constructs them
//! is emitted by its own task. The two must keep describing one wire; the discriminant and the
//! carried keys below are what a Rust `ServiceFault` has to serialize to.

/// The four cases a fault distinguishes, each naming what it was reading when it failed. `field`
/// is on the validation case alone: it is the only one that failed at a particular key rather than
/// at the message or the operation as a whole.
pub const TYPESCRIPT: &str = r#"/**
 * A failure no operation declared: a payload that would not deserialize, a message that failed its
 * own validation, an operation name nothing recognises, or a handler that panicked.
 *
 * It rides inside a result's failure arm behind `isServiceFault: true`, so a caller handling only
 * the declared errors still compiles and one that wants to tell them apart narrows on that literal.
 * No service implementation can produce a fault; the generated dispatcher and the generated client
 * are the only two places that construct one.
 */
export type ServiceFault =
  | { faultKind: "undeserializable-payload"; operation: string; detail: string }
  | { faultKind: "invalid-message"; operation: string; field: string; detail: string }
  | { faultKind: "unknown-operation"; operation: string; detail: string }
  | { faultKind: "handler-panic"; operation: string; detail: string };"#;
