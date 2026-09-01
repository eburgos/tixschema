//! The client for a service that declares no operation: the seam, the client type, and no method
//! at all. Nothing here reads a reply, so nothing here is emitted to read one.

bare_service_amqp_rpc_client!();
