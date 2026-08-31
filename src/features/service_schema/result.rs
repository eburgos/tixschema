//! The `<Operation>Result` type: the two arms an operation declared, joined into one return type.
//!
//! The envelope is all this adds. Whatever the operation named as its success type and as its
//! error type crosses unchanged — no field is added to either, none removed, none renamed — so a
//! message that happens to carry a key called `type` keeps it and one that does not never gains
//! one.
//!
//! The failure arm holds the declared error *or* a fault, rather than the union growing a third
//! member: two members sharing `ok: false` would stop `ok` being a discriminant at all, and
//! narrowing on the envelope would then tell a caller only that the call failed. Two arms, with the
//! fault behind the literal `isServiceFault: true`, leave the compiler something to narrow on at
//! both levels.
//!
//! A one-way operation gets no result type. It declared no reply and therefore no error, and a
//! type joining arms it does not have would be a type nothing can be assigned from.
//!
//! Both names carry the service: `UsageServiceGetBalanceResult`, and the fault it can hold is
//! `UsageServiceFault`. Two services declaring a `get_balance` each would otherwise publish one
//! `GetBalanceResult` twice into the one flat file a bundle is.

use crate::field_type::get_field_def;
use crate::rename_rule::RenameRule;
use crate::service_schema::parse::{OperationDef, OperationOutcome, ServiceDef};

pub fn emit(service: &ServiceDef) -> Vec<String> {
    let named = service.ident.to_string();
    service
        .operations
        .iter()
        .filter_map(|operation| result_type(&named, operation))
        .collect()
}

/// What one operation's result type is called: `get_balance` on `UsageService` answers a
/// `UsageServiceGetBalanceResult`. `None` for a one-way operation, which answers nothing.
pub fn result_name(service: &str, operation: &OperationDef) -> Option<String> {
    match operation.outcome {
        OperationOutcome::OneWay => None,
        OperationOutcome::Reply { .. } => Some(format!(
            "{service}{}Result",
            RenameRule::PascalCase.apply_to_field(&operation.ident.to_string())
        )),
    }
}

fn result_type(service: &str, operation: &OperationDef) -> Option<String> {
    let OperationOutcome::Reply { error, success } = &operation.outcome else {
        return None;
    };
    let published = result_name(service, operation)?;
    let value = get_field_def("value", success, "").typescript_typename();
    let failure = get_field_def("error", error, "").typescript_typename();
    let called = &operation.ts_name;
    Some(format!(
        "/**\n \
         * What `{called}` on `{service}` answers with: the value it declared, the error it \
         declared,\n \
         * or a fault it never declared.\n \
         */\n\
         export type {published} =\n  \
         | {{ ok: true; value: {value} }}\n  \
         | {{ ok: false; error: {failure} | {{ isServiceFault: true; fault: {service}Fault }} }};"
    ))
}
