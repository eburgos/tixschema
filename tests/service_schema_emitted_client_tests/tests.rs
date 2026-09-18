//! The contract the runtime groups beside this one exercise: one operation whose single argument
//! is the author's own struct, bound to a path with one placeholder, carrying one field the path
//! does not bind.

use core::future::ready;
use serde::{Deserialize, Serialize};
use tixschema::{model_schema, service_schema};

/// `conversation_id` is what the placeholder names; `limit` has nowhere to go but the query.
#[model_schema()]
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WindowRequest {
    pub conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[model_schema()]
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WindowPage {
    pub items: Vec<String>,
}

#[model_schema()]
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
pub enum WindowError {
    NotFound,
}

/// A message that already is a wire scalar: the whole message is the segment.
#[model_schema()]
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ConversationId(pub String);

#[service_schema(transports = ["http_rest"])]
pub trait ConversationClientService<Ctx> {
    #[service_schema_op(
        one_way,
        http(method = "DELETE", path = "/v1/conversations/{conversation_id}",)
    )]
    async fn purge_conversation(&self, ctx: &Ctx, conversation_id: ConversationId);

    #[service_schema_op(http(
        method = "GET",
        path = "/v1/conversations/{conversation_id}/window",
        error_status(NotFound = 404),
    ))]
    async fn window(&self, ctx: &Ctx, req: WindowRequest) -> Result<WindowPage, WindowError>;
}

/// A backend answering the contract, so the trait is implementable rather than merely declared.
struct ConversationBackEnd;

impl ConversationClientService<()> for ConversationBackEnd {
    async fn purge_conversation(&self, _ctx: &(), _conversation_id: ConversationId) {
        ready(()).await;
    }

    async fn window(&self, _ctx: &(), req: WindowRequest) -> Result<WindowPage, WindowError> {
        ready(()).await;
        Ok(WindowPage {
            items: vec![req.conversation_id],
        })
    }
}

/// Every declared type is constructible — the groups beside this one read only emitted text.
#[test]
fn every_declared_type_is_constructible() {
    let asked = WindowRequest {
        conversation_id: "652f1a3b4c5d6e7f8a9b0c1d".to_owned(),
        limit: Some(10),
    };
    assert_eq!(asked.limit, Some(10));
    assert_eq!(
        WindowPage {
            items: vec!["one".to_owned()]
        }
        .items
        .len(),
        1
    );
    assert_eq!(WindowError::NotFound, WindowError::NotFound);
    assert_eq!(
        ConversationId("652f1a3b4c5d6e7f8a9b0c1d".to_owned())
            .0
            .len(),
        24
    );
    drop(ConversationBackEnd.window(&(), asked));
    drop(
        ConversationBackEnd
            .purge_conversation(&(), ConversationId("652f1a3b4c5d6e7f8a9b0c1d".to_owned())),
    );
}
