//! The same client a second time, in a module named differently. `#[macro_export]` puts one name
//! at the crate root and the macro emits bare items, so two placements are two transport seams and
//! two client types that share nothing.

use crate::tests::{AdmitRequest, BalanceRequest, BalanceResponse, ProbeError};

probe_service_amqp_rpc_client!();
