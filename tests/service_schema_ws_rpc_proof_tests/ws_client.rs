//! The `DocumentSession` client, expanded out of the transport's macro into a module this harness
//! names.
//!
//! The `use` is what resolves the types the author declared: the macro spells them exactly as
//! they were written, no crate prefix being true of any of them.

use crate::tests::{
    RangeError, RangeResult, ReadRangeRequest, TouchRequest, UnwatchError, UnwatchRequest,
    WatchError, WatchRequest, WatchResult,
};

document_session_ws_rpc_client!();
