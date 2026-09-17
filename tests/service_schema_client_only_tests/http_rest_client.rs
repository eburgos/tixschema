//! The `http_rest` client, expanded out of the transport's macro into a module this harness names.
//!
//! Unlike `amqp_rpc`'s client, which decodes generically through `Answered<T, E>` and so never
//! spells a success or an error type by name, `http_rest`'s client reads a response body straight
//! into the operation's own declared types (`serde_json::from_slice::<Success>`,
//! `::<Error>`), both spelled bare — the author's own types, resolved where this macro is invoked
//! rather than where the service was declared.

use crate::tests::{
    CreateDocumentError, CreateDocumentRequest, CreateDocumentResponse, GetVersionError,
    GetVersionRequest, ReadWindowRequest, ThumbnailError, VersionResponse,
};

document_client_service_http_rest_client!();
