//! The client for the browser-implemented `SessionEvents` service, expanded into a module this
//! harness names — the one a server holds to push a `FrameWriter` event.

use crate::tests::DocumentChangedRequest;

session_events_ws_rpc_client!();
