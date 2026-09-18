//! The emitted TypeScript client run by node, with a real message object.
//!
//! `String(sending)` renders every object as the constant `[object Object]`, so a client that
//! stringifies the whole message sends the same segment for every request.

use super::runtime::ran;
use super::tests::ConversationClientServiceSchema;

/// Names the runtime to run, for a machine that has one somewhere other than `PATH`.
const RUNTIME_VAR: &str = "TIXSCHEMA_NODE";

/// The schema surface the emitted client names, passing the message through untouched: the URL is
/// what is under test, and the real `$Schema` consts are zod expressions no bare runtime can
/// evaluate.
const SCHEMA_STUBS: &str = "const WindowRequest$Schema = {
  safeParse: (value) => ({ success: true, data: value }),
};
const ConversationId$Schema = {
  safeParse: (value) => ({ success: true, data: value }),
};
";

/// Records the request it is handed and answers each operation's own declared status — `204` for
/// the one-way `DELETE`, where `200` would be a fault the client throws instead.
const DRIVER: &str = r#"
const sent = [];
const transport = {
  async send(request) {
    sent.push(request);
    return request.method === "DELETE"
      ? { status: 204, headers: [], body: "" }
      : { status: 200, headers: [], body: JSON.stringify({ items: [] }) };
  },
};
const client = createConversationClientServiceHttpClient(transport);
await client.window({ conversation_id: "652f1a3b4c5d6e7f8a9b0c1d", limit: 10 });
await client.window({ conversation_id: "652f1a3b4c5d6e7f8a9b0c1d" });
await client.purgeConversation("652f1a3b4c5d6e7f8a9b0c1d");
console.log(JSON.stringify(sent));
"#;

/// The emitted client between the stubs it names and the driver that calls it.
fn module() -> String {
    format!(
        "{SCHEMA_STUBS}\n{client}\n{DRIVER}",
        client = ConversationClientServiceSchema::ts_http_client()
    )
}

/// The requests the driver recorded, or `None` where no runtime was reachable.
fn sent() -> Option<Vec<serde_json::Value>> {
    let wrote = ran("node", RUNTIME_VAR, "node", "client.mts", &module())?;
    Some(serde_json::from_str(wrote.trim()).unwrap())
}

#[test]
fn a_lone_placeholder_sends_the_field_it_names_and_never_the_stringified_message() {
    let Some(sent) = sent() else {
        return;
    };
    assert_eq!(
        sent[0]["path"], "/v1/conversations/652f1a3b4c5d6e7f8a9b0c1d/window",
        "the placeholder is filled by the field it names. Got: {sent:#?}"
    );
    assert!(
        !sent[0]["path"].as_str().unwrap().contains("object"),
        "`String(sending)` on a message object is the constant `[object Object]`, which reaches \
         the wire as `%5Bobject%20Object%5D` — the same segment for every request the operation \
         ever makes. Got: {sent:#?}"
    );
}

#[test]
fn a_field_the_path_does_not_bind_reaches_the_query_string() {
    let Some(sent) = sent() else {
        return;
    };
    assert_eq!(
        sent[0]["query"], "limit=10",
        "`limit` is bound to no placeholder, so the query string is the only place left for it. \
         Got: {sent:#?}"
    );
    assert_eq!(
        sent[1]["query"], "",
        "the same operation with `limit` absent sends no key for it rather than `limit=undefined`. \
         Got: {sent:#?}"
    );
}

#[test]
fn a_scalar_message_is_still_the_whole_segment() {
    let Some(sent) = sent() else {
        return;
    };
    assert_eq!(
        sent[2]["path"], "/v1/conversations/652f1a3b4c5d6e7f8a9b0c1d",
        "a message that already is a wire scalar has no field to read: it is the segment. \
         Got: {sent:#?}"
    );
    assert_eq!(
        sent[2]["query"], "",
        "and no key is left over for a query. Got: {sent:#?}"
    );
}
