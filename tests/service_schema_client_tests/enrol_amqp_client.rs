//! The second service's client, in a module of its own — the same crate carrying two of them
//! without either naming the other's module.

use crate::tests::a_bound_the_fields_own_type_declares::{EnrolError, EnrolRequest, Enrolled};

enrol_service_amqp_rpc_client!();
