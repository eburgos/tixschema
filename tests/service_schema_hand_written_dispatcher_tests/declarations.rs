//! The service the module beside this one answers for: two operations, and no transport asked for.

use serde::{Deserialize, Serialize};
use tixschema::service_schema;

/// A message that checks itself. An inherent `validate()` is what `#[model_schema()]` writes onto a
/// type with constrained fields, and it beats the fallback the service's own module publishes.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BalanceRequest {
    pub organization_id: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BalanceResponse {
    pub credits: u32,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum ProbeError {
    DbError,
}

/// A message that declares no check of its own, so asking it to validate reaches the fallback.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SweepRequest {
    pub since: u32,
}

#[service_schema()]
pub trait ProbeService<Ctx> {
    async fn get_balance(
        &self,
        ctx: &Ctx,
        req: BalanceRequest,
    ) -> Result<BalanceResponse, ProbeError>;

    async fn sweep(&self, ctx: &Ctx, req: SweepRequest) -> Result<BalanceResponse, ProbeError>;
}

impl BalanceRequest {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        if self.organization_id.is_empty() {
            return Err(vec![
                "'organization_id': too short: minimum length is 1, got 0".to_owned(),
            ]);
        }
        Ok(())
    }
}
