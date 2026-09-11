//! The dispatcher for the browser-implemented `LedgerEvents` service, expanded into a module this
//! harness names — the one the tokio-tungstenite client loop answers every push through, on
//! `Screen`'s behalf.

ledger_events_ws_rpc_dispatcher!();
