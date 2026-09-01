//! The same service's dispatcher a second time, in a module named differently. Nothing in the
//! macro names a module, so both expansions stand side by side in one crate.

probe_service_amqp_rpc_dispatcher!();
