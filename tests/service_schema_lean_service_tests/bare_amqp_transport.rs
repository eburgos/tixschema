//! The dispatcher for a service that declares no operation: `IncomingMessage`, `Reply` and a
//! `dispatch` whose only arm answers a name nothing recognises.

bare_service_amqp_rpc_dispatcher!();
