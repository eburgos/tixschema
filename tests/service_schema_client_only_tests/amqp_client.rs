//! The client, expanded out of the transport's macro into a module this harness names. The
//! dispatcher's macro is never invoked here, and nothing below reaches for it.

use crate::tests::{CallAnswer, CallFailure, CallRequest};

call_service_amqp_rpc_client!();
