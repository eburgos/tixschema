//! The emitted Dart client run by the Dart VM, with a real message object.
//!
//! The Dart twin of [`super::run_node`], asking the same three questions. Dart erases nothing, so
//! the module carries the real generated classes: the client calls the `toJson` the `dart` backend
//! wrote, not a stand-in.

#![cfg(feature = "dart")]

use super::runtime::ran;
use super::tests::conversation_client_service_schema::{
    conversation_client_service_fault_fields_dart, conversation_client_service_fault_kind_dart,
};
use super::tests::{
    ConversationClientServiceSchema, conversation_id_dart, window_error_dart, window_page_dart,
    window_request_dart,
};

/// Names the runtime to run, for a machine that has one somewhere other than `PATH`.
const RUNTIME_VAR: &str = "TIXSCHEMA_DART";

/// Records the request it is handed and answers each operation's own declared status.
const DRIVER: &str = "
class _Recorder implements ConversationClientServiceHttpTransport {
  final List<Map<String, String>> sent = <Map<String, String>>[];

  @override
  Future<({int status, List<(String, String)> headers, List<int> body})> send(
    ({String method, String path, String query, List<(String, String)> headers, List<int> body}) request,
  ) async {
    sent.add(<String, String>{
      'method': request.method,
      'path': request.path,
      'query': request.query,
    });
    if (request.method == 'DELETE') {
      return (status: 204, headers: <(String, String)>[], body: <int>[]);
    }
    return (
      status: 200,
      headers: <(String, String)>[],
      body: utf8.encode(jsonEncode(<String, dynamic>{'items': <String>[]})),
    );
  }
}

void main() async {
  final recorder = _Recorder();
  final client = ConversationClientServiceHttpClient(recorder);
  await client.window(WindowRequest(
    conversation_id: '652f1a3b4c5d6e7f8a9b0c1d',
    limit: 10,
  ));
  await client.window(WindowRequest(conversation_id: '652f1a3b4c5d6e7f8a9b0c1d'));
  await client.purgeConversation(ConversationId('652f1a3b4c5d6e7f8a9b0c1d'));
  print(jsonEncode(recorder.sent));
}
";

/// The generated classes the client calls, the client, and the driver.
fn module() -> String {
    [
        "import 'dart:convert';".to_owned(),
        conversation_id_dart::dart_definition(),
        window_request_dart::dart_definition(),
        window_page_dart::dart_definition(),
        window_error_dart::dart_definition(),
        conversation_client_service_fault_fields_dart::dart_definition(),
        conversation_client_service_fault_kind_dart::dart_definition(),
        ConversationClientServiceSchema::dart_http_client(),
        DRIVER.to_owned(),
    ]
    .join("\n\n")
}

/// The requests the driver recorded, or `None` where no runtime was reachable.
fn sent() -> Option<Vec<serde_json::Value>> {
    let wrote = ran("dart", RUNTIME_VAR, "dart", "client.dart", &module())?;
    Some(serde_json::from_str(wrote.trim()).unwrap())
}

#[test]
fn a_lone_placeholder_sends_the_field_it_names_and_never_the_rendered_message() {
    let Some(sent) = sent() else {
        return;
    };
    assert_eq!(
        sent[0]["path"], "/v1/conversations/652f1a3b4c5d6e7f8a9b0c1d/window",
        "the placeholder is filled by the field it names. Got: {sent:#?}"
    );
    assert!(
        !sent[0]["path"].as_str().unwrap().contains("%7B"),
        "rendering the whole message puts its own map spelling in the segment. Got: {sent:#?}"
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
        "the same operation with `limit` absent sends no key for it rather than `limit=null`. \
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
