```rust
use tixschema::{model_schema, service_schema};

#[model_schema()]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct BalanceRequest {
    pub organization_id: String,
}

#[model_schema()]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct BalanceResponse {
    pub credits: u32,
}

#[model_schema()]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "errorCode", rename_all = "kebab-case")]
pub enum UsageError {
    DbError,
}

#[service_schema()]
pub trait UsageService<Ctx> {
    /// One argument after the context: that argument already is the message.
    async fn get_balance(
        &self,
        ctx: &Ctx,
        req: BalanceRequest,
    ) -> Result<BalanceResponse, UsageError>;

    /// Several arguments: the message is declared from the argument list.
    async fn expire_credit(
        &self,
        ctx: &Ctx,
        organization_id: String,
        credit_id: String,
    ) -> Result<BalanceResponse, UsageError>;

    /// Carried on the wire as `usage-generation-request` rather than `can-generate`.
    #[service_schema_op(message = "usage-generation-request")]
    async fn can_generate(
        &self,
        ctx: &Ctx,
        req: BalanceRequest,
    ) -> Result<BalanceResponse, UsageError>;

    /// No reply, so no return type and no error arm.
    #[service_schema_op(one_way)]
    async fn apply_bundle(&self, ctx: &Ctx, req: BalanceRequest);
}

// Declared at module scope, which is where the generated module reaches for them.
fn main() {}
```
