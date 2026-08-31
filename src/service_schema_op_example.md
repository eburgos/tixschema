```rust
use tixschema::{model_schema, service_schema};

#[model_schema()]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ApplyBundleRequest {
    pub organization_id: String,
}

#[service_schema()]
pub trait OrganizationService<Ctx> {
    #[service_schema_op(one_way)]
    async fn apply_bundle(&self, ctx: &Ctx, req: ApplyBundleRequest);
}

fn main() {}
```
