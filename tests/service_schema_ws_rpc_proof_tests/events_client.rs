//! The `SessionEvents` client, expanded out of the transport's macro into a module this harness
//! names — held server-side, over a `FrameWriter`, to push an event with no reply to wait on.

use crate::tests::DocumentChangedRequest;

session_events_ws_rpc_client!();
