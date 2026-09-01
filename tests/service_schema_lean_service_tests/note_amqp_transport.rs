//! The dispatcher for a service whose every operation expects no reply. Every arm still guards its
//! implementation against a panic and still reads a refused payload for a field.

note_service_amqp_rpc_dispatcher!();
