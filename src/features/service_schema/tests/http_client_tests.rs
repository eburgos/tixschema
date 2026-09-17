//! The `http_rest` TypeScript client, read off the emitted text.
//!
//! What these prove and what they cannot: the structure of the emitted TypeScript — the seam's own
//! shape, how a path, a query string and a request header get built, how a status decodes into a
//! success, a declared error or a fault. No TypeScript toolchain is reachable here, so none of them
//! type-checks the bundle.

use super::{
    BYTES_HTTP_SERVICE, MIXED_HTTP_SERVICE, MULTIPART_HTTP_SERVICE,
    SINGLE_PLACEHOLDER_HTTP_SERVICE, STREAM_HTTP_SERVICE, http_client_of,
};

#[test]
fn exactly_one_seam_type_is_emitted_and_it_names_no_http_library() {
    let written = http_client_of(MIXED_HTTP_SERVICE);
    assert_eq!(
        written
            .matches("export type DocumentClientServiceHttpTransport = {")
            .count(),
        1,
        "one seam type serves every operation on the service. Got: {written}"
    );
    for named in ["fetch", "axios", "Fetch", "Axios"] {
        assert!(
            !written.contains(named),
            "the seam speaks only in plain terms; the library that finally carries the call is \
             an adapter's business, never this crate's. Got: {written}"
        );
    }
}

#[test]
fn the_seam_carries_a_request_record_in_and_a_response_record_out() {
    let written = http_client_of(MIXED_HTTP_SERVICE);
    let found = written
        .split("export type DocumentClientServiceHttpTransport = {")
        .nth(1)
        .and_then(|rest| rest.split_once("\n};"))
        .map(|(body, _)| body.to_owned());
    assert!(found.is_some(), "got: {written}");
    let seam = found.unwrap();
    assert!(
        seam.contains("send(request: {")
            && seam.contains("method: string;")
            && seam.contains("path: string;")
            && seam.contains("query: string;")
            && seam.contains("headers: ReadonlyArray<readonly [string, string]>;")
            && seam.contains("body: string;"),
        "got: {seam}"
    );
    assert!(
        seam.contains("}): Promise<{") && seam.contains("status: number;"),
        "got: {seam}"
    );
}

#[test]
fn a_bodied_operation_fills_a_literal_path_and_serializes_the_validated_message_as_the_body() {
    let written = http_client_of(MIXED_HTTP_SERVICE);
    let method = method_body(&written, "createDocument");
    assert!(
        method.contains("path += \"/documents\";"),
        "a path with no placeholder is pushed exactly as written. Got: {method}"
    );
    assert!(
        method.contains("const query = \"\";"),
        "a bodied method carries no query string. Got: {method}"
    );
    assert!(
        method.contains("const body = JSON.stringify(sending);"),
        "the body is the validated message, not the raw argument. Got: {method}"
    );
    assert!(method.contains("method: \"POST\","), "got: {method}");
}

#[test]
fn a_path_placeholder_is_filled_by_exact_segment_substitution() {
    let written = http_client_of(MIXED_HTTP_SERVICE);
    let method = method_body(&written, "getVersion");
    assert!(
        method.contains("path += \"/documents/\";")
            && method.contains("path += encodeURIComponent(String(sending.document_id));")
            && method.contains("path += \"/versions/\";")
            && method.contains("path += encodeURIComponent(String(sending.version_id));"),
        "each segment is pushed in template order, a placeholder reading its own field off the \
         validated message rather than splitting a shared prefix. Got: {method}"
    );
}

#[test]
fn a_lone_placeholder_on_an_author_s_own_message_reads_the_field_it_names() {
    let written = http_client_of(SINGLE_PLACEHOLDER_HTTP_SERVICE);
    let method = method_body(&written, "window");
    assert!(
        method.contains("path += encodeURIComponent(String(sending.conversation_id));"),
        "a message the author declared is an object whatever the path is shaped like, so the one \
         placeholder reads the field it names off it. Got: {method}"
    );
    assert!(
        !method.contains("path += encodeURIComponent(String(sending));"),
        "stringifying the whole message would send `[object Object]` as the segment. \
         Got: {method}"
    );
}

#[test]
fn a_lone_placeholder_on_a_scalar_message_still_is_the_whole_message() {
    let written = http_client_of(SINGLE_PLACEHOLDER_HTTP_SERVICE);
    let method = method_body(&written, "purgeConversation");
    assert!(
        method.contains("path += encodeURIComponent(String(sending));"),
        "a message that already is a wire scalar has no field to read: it is the segment. \
         Got: {method}"
    );
}

#[test]
fn a_named_message_s_unbound_fields_build_the_query_string_of_a_bodyless_method() {
    let written = http_client_of(SINGLE_PLACEHOLDER_HTTP_SERVICE);
    let method = method_body(&written, "window");
    assert!(
        method.contains("const pathBound: ReadonlyArray<string> = [\"conversation_id\"];")
            && method.contains("for (const [key, value] of Object.entries(sending)) {")
            && method.contains("queryParts.push(`${key}=${encodeURIComponent(rendered)}`);")
            && method.contains("const query = queryParts.join(\"&\");"),
        "a field the path does not bind is a query parameter; the message is walked because this \
         macro cannot name an author's own fields. Got: {method}"
    );
    assert!(
        !method.contains("const query = \"\";"),
        "a message carrying more than the path spends does not send an empty query. \
         Got: {method}"
    );
}

#[test]
fn a_scalar_named_message_builds_no_query_string() {
    let written = http_client_of(SINGLE_PLACEHOLDER_HTTP_SERVICE);
    let method = method_body(&written, "purgeConversation");
    assert!(
        method.contains("const query = \"\";"),
        "a message that already is a wire scalar has no keys left over: the path spent it. \
         Got: {method}"
    );
}

#[test]
fn a_header_in_binding_becomes_an_extra_argument_and_a_built_header() {
    let written = http_client_of(MIXED_HTTP_SERVICE);
    assert!(
        written.contains("getVersion(req: GetVersionRequest, byteRange: string | undefined)"),
        "the client type spells the extra argument beside the message. Got: {written}"
    );
    let method = method_body(&written, "getVersion");
    assert!(
        method.contains("const headers: Array<[string, string]> = [];")
            && method.contains(
                "if (byteRange !== undefined) {\n        \
                 headers.push([\"range\", String(byteRange)]);\n      \
                 }"
            ),
        "the header is built from the extra argument, never from the message. Got: {method}"
    );
}

#[test]
fn a_bodyless_operation_with_no_header_in_builds_an_empty_header_array() {
    let written = http_client_of(MIXED_HTTP_SERVICE);
    let method = method_body(&written, "createDocument");
    assert!(
        method.contains("const headers: Array<[string, string]> = [];"),
        "got: {method}"
    );
}

#[test]
fn the_declared_ok_status_decodes_into_the_success_type() {
    let written = http_client_of(MIXED_HTTP_SERVICE);
    let method = method_body(&written, "createDocument");
    assert!(
        method.contains("if (status === 200) {")
            && method.contains("value = JSON.parse(response.body) as CreateDocumentResponse;")
            && method.contains("return { ok: true, value };"),
        "got: {method}"
    );
}

#[test]
fn a_mapped_status_decodes_into_the_declared_error_type() {
    let written = http_client_of(MIXED_HTTP_SERVICE);
    let method = method_body(&written, "createDocument");
    assert!(
        method.contains("if (status === 409) {")
            && method.contains("declared = JSON.parse(response.body) as CreateDocumentError;")
            && method.contains("return { ok: false, error: declared };"),
        "got: {method}"
    );
}

#[test]
fn a_fixed_fault_status_decodes_through_the_shared_fault_reader() {
    let written = http_client_of(MIXED_HTTP_SERVICE);
    let method = method_body(&written, "createDocument");
    assert!(
        method.contains("if (status === 400 || status === 404 || status === 500) {")
            && method.contains(
                "fault: documentClientServiceHttpFaultFromBody(\"create-document\", response.body),"
            ),
        "got: {method}"
    );
}

#[test]
fn a_status_naming_no_declared_or_fixed_outcome_becomes_a_typed_undeserializable_payload_fault() {
    let written = http_client_of(MIXED_HTTP_SERVICE);
    let method = method_body(&written, "createDocument");
    assert!(
        method.contains(
            "fault: documentClientServiceHttpUndeserializablePayload(\n            \
             \"create-document\",\n            \
             `an unexpected status (${status}) answered`,\n          \
             ),"
        ),
        "got: {method}"
    );
}

#[test]
fn an_operation_naming_no_http_group_defaults_to_post_and_the_fixed_binding_error_status() {
    let written = http_client_of(MIXED_HTTP_SERVICE);
    let method = method_body(&written, "sweepDocuments");
    assert!(
        method.contains("method: \"POST\","),
        "an operation naming no `http(...)` group defaults to `POST /{{wire-name}}`. \
         Got: {method}"
    );
    assert!(
        method.contains("path += \"/sweep-documents\";"),
        "got: {method}"
    );
    assert!(
        method.contains("if (status === 422) {")
            && method.contains("declared = JSON.parse(response.body) as SweepError;"),
        "an operation naming no `http(...)` group declares no `error_status` table either, so \
         its declared error answers at the fixed binding-error status. Got: {method}"
    );
}

#[test]
fn a_tuple_success_reads_its_header_out_element_back_off_the_response() {
    let written = http_client_of(MIXED_HTTP_SERVICE);
    let method = method_body(&written, "getVersion");
    assert!(
        method.contains(
            "const rawHeaderOut0 = response.headers.find(\n          \
             ([name]) => name.toLowerCase() === \"etag\",\n        \
             );"
        ),
        "got: {method}"
    );
    assert!(
        method.contains("if (rawHeaderOut0 === undefined) {")
            && method.contains("\"a declared response header was missing\","),
        "a missing declared header is a defect, not a silently absent value. Got: {method}"
    );
    assert!(
        method.contains("const headerOut0 = rawHeaderOut0[1] as string;")
            && method.contains("return { ok: true, value: [value, headerOut0] };"),
        "the body and the header ride the same tuple the result type declares. Got: {method}"
    );
}

#[test]
fn a_no_payload_operation_resolves_on_its_declared_status_and_reads_no_body() {
    let written = http_client_of(MIXED_HTTP_SERVICE);
    let method = method_body(&written, "purgeDocument");
    assert!(
        method.contains("if (status === 204) {") && method.contains("return;"),
        "got: {method}"
    );
    assert!(
        !method.contains("JSON.parse(response.body)"),
        "a one-way method's success case has no body to decode. Got: {method}"
    );
}

#[test]
fn a_one_way_operation_answers_promise_void_and_throws_instead_of_returning_a_fault() {
    let written = http_client_of(MIXED_HTTP_SERVICE);
    assert!(
        written.contains("purgeDocument(req: string): Promise<void>;"),
        "got: {written}"
    );
    let method = method_body(&written, "purgeDocument");
    assert!(
        method.contains(
            "throw documentClientServiceHttpRefused(\n          \
             documentClientServiceHttpOutboundFault(\"purge-document\", validated.error.issues),\n        \
             );"
        ),
        "a refused outbound message throws before the transport is ever reached. Got: {method}"
    );
    assert!(
        method.contains(
            "throw documentClientServiceHttpRefused(\n          \
             documentClientServiceHttpTransportFailure(\"purge-document\", String(uncarried)),\n        \
             );"
        ),
        "got: {method}"
    );
}

#[test]
fn a_named_message_answering_to_one_placeholder_is_read_back_as_the_whole_value() {
    let written = http_client_of(MIXED_HTTP_SERVICE);
    let method = method_body(&written, "purgeDocument");
    assert!(
        method.contains("path += encodeURIComponent(String(sending));"),
        "a `Named` message with exactly one placeholder is that placeholder's own value, not a \
         field read off it. Got: {method}"
    );
}

#[test]
fn every_method_validates_before_it_ever_names_the_transport() {
    let written = http_client_of(MIXED_HTTP_SERVICE);
    for (call, schema) in [
        ("createDocument", "CreateDocumentRequest$Schema"),
        ("getVersion", "GetVersionRequest$Schema"),
    ] {
        let method = method_body(&written, call);
        let checked = method.find(&format!("{schema}.safeParse(req)"));
        let reached = method.find("transport.send(");
        assert!(checked.is_some() && reached.is_some(), "got: {method}");
        assert!(
            checked.unwrap() < reached.unwrap(),
            "the transport is reached only once the message has passed its own validator. \
             Got: {method}"
        );
    }
}

#[test]
fn the_factory_binds_a_transport_and_answers_with_the_client() {
    let written = http_client_of(MIXED_HTTP_SERVICE);
    assert!(
        written.contains(
            "export function createDocumentClientServiceHttpClient(\n  \
             transport: DocumentClientServiceHttpTransport,\n\
             ): DocumentClientServiceHttpClient {"
        ),
        "got: {written}"
    );
}

#[test]
fn a_bytes_operation_reads_the_body_content_type_and_header_out_back() {
    let written = http_client_of(BYTES_HTTP_SERVICE);
    let method = method_body(&written, "getThumbnail");
    assert!(
        method.contains(
            "const contentType = response.headers.find(\n          \
             ([name]) => name.toLowerCase() === \"content-type\",\n        \
             )?.[1] ?? \"\";"
        ),
        "content-type is read back off the response headers. Got: {method}"
    );
    assert!(
        method.contains(
            "const rawHeaderOut0 = response.headers.find(\n          \
             ([name]) => name.toLowerCase() === \"x-document-id\",\n        \
             );"
        ),
        "the declared header_out entry is read back the same way the JSON path reads one. \
         Got: {method}"
    );
    assert!(
        method.contains("return { ok: true, value: [response.body, contentType, headerOut0] };"),
        "the body pair rides first, then the declared header - body-pair-first ordering. \
         Got: {method}"
    );
}

#[test]
fn a_multipart_operation_carries_parts_on_the_seam_and_takes_an_extra_file_argument() {
    let written = http_client_of(MULTIPART_HTTP_SERVICE);
    assert!(
        written.contains("parts: ReadonlyArray<readonly [string, unknown]>;"),
        "the seam's own request record carries `parts` for a multipart-declaring service. \
         Got: {written}"
    );
    assert!(
        written
            .contains("uploadDocument(req: UploadDocumentRequest, attachment: unknown): Promise<"),
        "the client type spells the extra file argument beside the message. Got: {written}"
    );
}

#[test]
fn a_multipart_method_builds_one_text_part_per_field_and_one_file_part_per_binding() {
    let written = http_client_of(MULTIPART_HTTP_SERVICE);
    let method = method_body(&written, "uploadDocument");
    assert!(
        method.contains("const body = \"\";"),
        "a multipart method's content rides in `parts`, never `body`. Got: {method}"
    );
    assert!(
        method.contains("const parts: Array<[string, unknown]> = [];")
            && method.contains("parts.push([\"title\", String(sending.title)]);")
            && method.contains(
                "if (sending.description !== undefined) {\n        \
                 parts.push([\"description\", String(sending.description)]);\n      \
                 }"
            )
            && method.contains("parts.push([\"file\", attachment]);"),
        "one text part per carried field not otherwise placeholder-bound, then one file part per \
         `part` binding. Got: {method}"
    );
    assert!(
        !method.contains("parts.push([\"folderId\""),
        "the path-bound field is read off the path, not sent as a text part too. Got: {method}"
    );
    assert!(
        method.contains("parts,\n        });"),
        "`parts` rides beside `body` in the request the seam is handed. Got: {method}"
    );
}

#[test]
fn a_stream_operation_answers_a_content_range_and_body_record_at_200_and_206() {
    let written = http_client_of(STREAM_HTTP_SERVICE);
    let method = method_body(&written, "getFile");
    assert!(
        method.contains("if (status === 206) {")
            && method.contains(
                "const contentRange =\n          \
                 response.headers.find(\n            \
                 ([name]) => name.toLowerCase() === \"content-range\",\n          \
                 )?.[1] ?? \"\";"
            )
            && method.contains("const answer = { contentRange, body: response.bodyStream };")
            && method.contains("return { ok: true, value: answer };"),
        "a `206` answers the record with `contentRange` read back off the response. Got: {method}"
    );
    assert!(
        method.contains("if (status === 200) {")
            && method.contains("const contentRange: string | undefined = undefined;"),
        "the declared `ok_status` answers the same record with `contentRange` left `undefined`. \
         Got: {method}"
    );
}

#[test]
fn a_206_answer_reads_content_range_ahead_of_naming_the_body() {
    let written = http_client_of(STREAM_HTTP_SERVICE);
    let method = method_body(&written, "getFile");
    let read_at = method.find("name.toLowerCase() === \"content-range\"");
    let consumed_at = method.find("response.bodyStream");
    assert!(read_at.is_some() && consumed_at.is_some(), "got: {method}");
    assert!(
        read_at.unwrap() < consumed_at.unwrap(),
        "`content-range` is read back off the response before the stream body is ever named, \
         mirroring the Rust client's own `into_body` (which consumes the response and so must run \
         last). Got: {method}"
    );
}

#[test]
fn a_stream_operation_with_header_out_composes_the_answer_and_the_header() {
    let written = http_client_of(STREAM_HTTP_SERVICE);
    let method = method_body(&written, "getTaggedFile");
    assert!(
        method.contains(
            "const rawHeaderOut0 = response.headers.find(\n          \
             ([name]) => name.toLowerCase() === \"x-checksum\",\n        \
             );"
        ),
        "the declared header is read back the same way the JSON and bytes paths read one. \
         Got: {method}"
    );
    assert!(
        method.contains("const headerOut0 = rawHeaderOut0[1] as string;"),
        "got: {method}"
    );
    assert_eq!(
        method
            .matches("return { ok: true, value: [answer, headerOut0] };")
            .count(),
        2,
        "the header composes onto the answer in both the `206` and `200` arms. Got: {method}"
    );
}

#[test]
fn the_seam_carries_a_body_stream_field_only_where_a_service_declares_one() {
    let plain = http_client_of(MIXED_HTTP_SERVICE);
    assert!(
        !plain.contains("bodyStream"),
        "a service with no streamed operation carries no `bodyStream` field. Got: {plain}"
    );
    let streamed = http_client_of(STREAM_HTTP_SERVICE);
    assert!(
        streamed.contains(
            "}): Promise<{\n    \
             status: number;\n    \
             headers: ReadonlyArray<readonly [string, string]>;\n    \
             body: string;\n    \
             bodyStream: ReadableStream<Uint8Array>;\n  \
             }>;"
        ),
        "a service with a streamed operation carries `bodyStream` on the seam's own response \
         type. Got: {streamed}"
    );
    for named in ["fetch", "axios", "Fetch", "Axios"] {
        assert!(
            !streamed.contains(named),
            "the seam still names no HTTP library once it carries a stream - `ReadableStream` is \
             the platform's own type. Got: {streamed}"
        );
    }
}

/// One method's body, read out of the factory's own object literal by the call it is declared
/// under.
fn method_body(written: &str, call: &str) -> String {
    let found = written
        .split(&format!("async {call}("))
        .nth(1)
        .and_then(|rest| rest.split_once("\n},"))
        .map(|(body, _)| body.to_owned());
    assert!(found.is_some(), "no `{call}` method found. Got: {written}");
    found.unwrap()
}
