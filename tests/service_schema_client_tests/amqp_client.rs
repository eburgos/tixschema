//! The client, expanded out of the transport's macro into a module this harness names.
//!
//! The `use` is what resolves the types the author declared: the macro spells them exactly as they
//! were written, no crate prefix being true of both `BalanceRequest` and `String`.

use crate::tests::{AdmitRequest, BalanceRequest, BalanceResponse, ProbeError};

probe_service_amqp_rpc_client!();
