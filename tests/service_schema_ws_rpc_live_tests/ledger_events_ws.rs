//! The client for the browser-implemented `LedgerEvents` service, expanded into a module this
//! harness names — the one a server holds to push a `FrameWriter` event to the connection it came
//! in on.

use crate::tests::TransactionPosted;

ledger_events_ws_rpc_client!();
