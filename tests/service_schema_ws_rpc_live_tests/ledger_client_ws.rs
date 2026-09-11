//! The client, expanded out of the transport's macro into a module this harness names — the one
//! the tokio-tungstenite client loop calls into.
//!
//! The `use` is what resolves the types the author declared: the macro spells them exactly as
//! they were written, no crate prefix being true of any of them.

use crate::tests::{
    CreateError, CreateTransactionRequest, ListError, ListTransactionsRequest, MarkSeen,
    Transaction, TransactionList,
};

ledger_ws_rpc_client!();
