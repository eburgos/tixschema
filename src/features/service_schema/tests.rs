//! What the service publishes to TypeScript, read off the strings themselves.
//!
//! The rendered TypeScript is asserted as text rather than as tokens, because text is what a bundle
//! writes to a `.ts` file and what a TypeScript compiler then reads.
//!
//! **What text assertions do and do not prove.** Nothing here type-checks the bundle. These tests
//! read structure: that a member is required rather than optional, that a name carries the
//! service, that the transport is named only on the far side of the validation check. They cannot
//! prove the emitted file compiles, and they cannot prove that an implementation missing a method
//! is rejected where it reaches the factory — only a compiler can, and one does: the type-check
//! group in `tests/service_schema_typescript_tests/type_check.rs` hands the bundle and two
//! implementations to a real `tsc` wherever one is reachable.

#[cfg(feature = "zod")]
mod client_tests;
#[cfg(feature = "dart")]
mod dart_http_client_tests;
#[cfg(feature = "zod")]
mod http_client_tests;
#[cfg(feature = "zod")]
mod service_tests;
#[cfg(feature = "zod")]
mod ws_client_tests;

#[cfg(feature = "zod")]
use super::client;
#[cfg(feature = "dart")]
use super::dart_http_client;
#[cfg(feature = "zod")]
use super::http_client;
#[cfg(feature = "zod")]
use super::service;
#[cfg(feature = "zod")]
use super::ws_client;
use super::{emit, result};
use crate::service_schema::parse::{ServiceDef, parse_service};
use quote::ToTokens as _;
use syn::ItemTrait;

/// A service with one of every input shape and one of every outcome: a named message, an argument
/// list, no arguments at all, and a one-way operation that answers nothing.
const MIXED_SERVICE: &str = "
    pub trait UsageService<Ctx> {
        async fn get_available_balance(
            &self,
            ctx: &Ctx,
            req: AvailableBalanceRequest,
        ) -> Result<AvailableBalanceResponse, BalanceError>;

        async fn expire_credit(
            &self,
            ctx: &Ctx,
            organization_id: OrganizationId,
            credit_id: CreditId,
        ) -> Result<ExpiredCredit, CreditWriteError>;

        async fn sweep(&self, ctx: &Ctx) -> Result<SweepReport, BalanceError>;

        #[service_schema_op(one_way)]
        async fn apply_bundle(&self, ctx: &Ctx, req: ApplyBundleRequest);
    }
";

/// A service exercising every `http(...)` shape: a bodied `POST` naming its own message with a
/// mapped error, a bodyless `GET` with a path carrying two placeholders on a `Named` message, a
/// `header_in` binding, a `header_out` tuple success and two mapped errors (one of which shares a
/// code with a fixed fault status), a one-way `DELETE` whose one argument is the message and the
/// whole placeholder at once, and an operation naming no `http(...)` group at all.
#[cfg(feature = "zod")]
const MIXED_HTTP_SERVICE: &str = "
    pub trait DocumentClientService<Ctx> {
        #[service_schema_op(http(
            method = \"POST\",
            path = \"/documents\",
            error_status(TitleTaken = 409)
        ))]
        async fn create_document(
            &self,
            ctx: &Ctx,
            req: CreateDocumentRequest,
        ) -> Result<CreateDocumentResponse, CreateDocumentError>;

        #[service_schema_op(http(
            method = \"GET\",
            path = \"/documents/{document_id}/versions/{version_id}\",
            ok_status = 200,
            header_in(\"range\" = byte_range),
            header_out(\"etag\"),
            error_status(NotFound = 404, VersionGone = 410),
        ))]
        async fn get_version(
            &self,
            ctx: &Ctx,
            req: GetVersionRequest,
            byte_range: Option<String>,
        ) -> Result<(VersionResponse, String), GetVersionError>;

        #[service_schema_op(one_way, http(method = \"DELETE\", path = \"/documents/{document_id}\"))]
        async fn purge_document(&self, ctx: &Ctx, document_id: String);

        async fn sweep_documents(&self, ctx: &Ctx) -> Result<SweepReport, SweepError>;
    }
";

/// A service exercising every `http(...)` shape the Dart client answers for: a bodied `POST`
/// naming its own message with a mapped error, a bodyless `GET` with a path carrying two
/// placeholders on a `Named` message plus a `header_in` binding and a `header_out` tuple success,
/// a bodyless `GET` whose unbound optional fields (a scalar and a `Vec`) build a query string, a
/// `body = \"bytes\"` `GET` whose one argument is the message and the whole placeholder at once, a
/// one-way `DELETE` in that same single-placeholder shape, and an operation naming no `http(...)`
/// group at all.
#[cfg(feature = "dart")]
const DART_HTTP_SERVICE: &str = "
    pub trait DocumentClientService<Ctx> {
        #[service_schema_op(http(
            method = \"POST\",
            path = \"/documents\",
            error_status(TitleTaken = 409)
        ))]
        async fn create_document(
            &self,
            ctx: &Ctx,
            req: CreateDocumentRequest,
        ) -> Result<CreateDocumentResponse, CreateDocumentError>;

        #[service_schema_op(http(
            method = \"GET\",
            path = \"/documents/{document_id}/versions/{version_id}\",
            ok_status = 200,
            header_in(\"range\" = byte_range),
            header_out(\"etag\"),
            error_status(NotFound = 404, VersionGone = 410),
        ))]
        async fn get_version(
            &self,
            ctx: &Ctx,
            req: GetVersionRequest,
            byte_range: Option<String>,
        ) -> Result<(VersionResponse, String), GetVersionError>;

        #[service_schema_op(http(
            method = \"GET\",
            path = \"/documents/search\",
            error_status(SearchFailed = 500),
        ))]
        async fn search_documents(
            &self,
            ctx: &Ctx,
            q: Option<String>,
            tags: Option<Vec<String>>,
        ) -> Result<SearchResponse, SearchError>;

        #[service_schema_op(http(
            method = \"GET\",
            path = \"/documents/{document_id}/thumbnail\",
            error_status(NotFound = 404),
            body = \"bytes\",
        ))]
        async fn get_thumbnail(
            &self,
            ctx: &Ctx,
            document_id: String,
        ) -> Result<(Vec<u8>, String), ThumbnailError>;

        #[service_schema_op(one_way, http(method = \"DELETE\", path = \"/documents/{document_id}\"))]
        async fn purge_document(&self, ctx: &Ctx, document_id: String);

        async fn sweep_documents(&self, ctx: &Ctx) -> Result<SweepReport, SweepError>;
    }
";

/// A service declaring one `body = \"bytes\"` operation composing `header_out` onto its own tuple:
/// the bytes, their content type, then the declared header.
#[cfg(feature = "zod")]
const BYTES_HTTP_SERVICE: &str = "
    pub trait ThumbnailClientService<Ctx> {
        #[service_schema_op(http(
            method = \"GET\",
            path = \"/documents/{document_id}/thumbnail\",
            body = \"bytes\",
            header_out(\"x-document-id\"),
            error_status(NotFound = 404),
        ))]
        async fn get_thumbnail(
            &self,
            ctx: &Ctx,
            document_id: String,
        ) -> Result<(Vec<u8>, String, String), ThumbnailError>;
    }
";

/// A service declaring two `body = \"stream\"` operations: one answering the bare streamed answer,
/// one composing a declared `header_out` onto it. Zod-gated mirror of `DART_STREAM_HTTP_SERVICE`,
/// since a build can carry `zod` without `dart`.
#[cfg(feature = "zod")]
const STREAM_HTTP_SERVICE: &str = "
    pub trait ContentClientService<Ctx> {
        #[service_schema_op(http(
            method = \"GET\",
            path = \"/files/{file_id}\",
            body = \"stream\",
            error_status(NotFound = 404),
        ))]
        async fn get_file(
            &self,
            ctx: &Ctx,
            file_id: String,
        ) -> Result<StreamedAnswer, ContentError>;

        #[service_schema_op(http(
            method = \"GET\",
            path = \"/files/{file_id}/tagged\",
            body = \"stream\",
            header_out(\"x-checksum\"),
            error_status(NotFound = 404),
        ))]
        async fn get_tagged_file(
            &self,
            ctx: &Ctx,
            file_id: String,
        ) -> Result<(StreamedAnswer, String), ContentError>;
    }
";

/// A service declaring one `body = \"multipart\"` operation: a path placeholder, two scalar
/// `Generated` fields (one required, one optional) and a `part` binding for the file itself.
#[cfg(feature = "zod")]
const MULTIPART_HTTP_SERVICE: &str = "
    pub trait UploadClientService<Ctx> {
        #[service_schema_op(http(
            method = \"POST\",
            path = \"/folders/{folder_id}/documents\",
            body = \"multipart\",
            part(\"file\" = attachment),
            error_status(TooLarge = 413),
        ))]
        async fn upload_document(
            &self,
            ctx: &Ctx,
            folder_id: String,
            title: String,
            description: Option<String>,
            attachment: Box<dyn upload_client_service_schema::BodySource + Send>,
        ) -> Result<UploadResponse, UploadError>;
    }
";

/// A service declaring one `body = "bytes"` operation composing `header_out` onto its own tuple:
/// the bytes, their content type, then the declared header. Dart-gated mirror of
/// `BYTES_HTTP_SERVICE`, since a build can carry `dart` without `zod`.
#[cfg(feature = "dart")]
const DART_BYTES_HEADER_OUT_SERVICE: &str = "
    pub trait ThumbnailClientService<Ctx> {
        #[service_schema_op(http(
            method = \"GET\",
            path = \"/documents/{document_id}/thumbnail\",
            body = \"bytes\",
            header_out(\"x-document-id\"),
            error_status(NotFound = 404),
        ))]
        async fn get_thumbnail(
            &self,
            ctx: &Ctx,
            document_id: String,
        ) -> Result<(Vec<u8>, String, String), ThumbnailError>;
    }
";

/// A service declaring two `body = "stream"` operations: one answering the bare streamed answer,
/// one composing a declared `header_out` onto it.
#[cfg(feature = "dart")]
const DART_STREAM_HTTP_SERVICE: &str = "
    pub trait ContentClientService<Ctx> {
        #[service_schema_op(http(
            method = \"GET\",
            path = \"/files/{file_id}\",
            body = \"stream\",
            error_status(NotFound = 404),
        ))]
        async fn get_file(
            &self,
            ctx: &Ctx,
            file_id: String,
        ) -> Result<StreamedAnswer, ContentError>;

        #[service_schema_op(http(
            method = \"GET\",
            path = \"/files/{file_id}/tagged\",
            body = \"stream\",
            header_out(\"x-checksum\"),
            error_status(NotFound = 404),
        ))]
        async fn get_tagged_file(
            &self,
            ctx: &Ctx,
            file_id: String,
        ) -> Result<(StreamedAnswer, String), ContentError>;
    }
";

/// A service declaring one `body = "multipart"` operation: a path placeholder, two scalar
/// `Generated` fields (one required, one optional) and a `part` binding for the file itself.
/// Dart-gated mirror of `MULTIPART_HTTP_SERVICE`.
#[cfg(feature = "dart")]
const DART_MULTIPART_HTTP_SERVICE: &str = "
    pub trait UploadClientService<Ctx> {
        #[service_schema_op(http(
            method = \"POST\",
            path = \"/folders/{folder_id}/documents\",
            body = \"multipart\",
            part(\"file\" = attachment),
            error_status(TooLarge = 413),
        ))]
        async fn upload_document(
            &self,
            ctx: &Ctx,
            folder_id: String,
            title: String,
            description: Option<String>,
            attachment: Box<dyn upload_client_service_schema::BodySource + Send>,
        ) -> Result<UploadResponse, UploadError>;
    }
";

#[cfg(feature = "zod")]
fn client_of(source: &str) -> String {
    client::emit(&parsed(source)).join("\n\n")
}

#[cfg(feature = "zod")]
fn http_client_of(source: &str) -> String {
    http_client::emit(&parsed(source)).join("\n\n")
}

#[cfg(feature = "zod")]
fn ws_client_of(source: &str) -> String {
    ws_client::emit(&parsed(source)).join("\n\n")
}

#[cfg(feature = "dart")]
fn dart_http_client_of(source: &str) -> String {
    dart_http_client::emit(&parsed(source)).join("\n\n")
}

fn parsed(source: &str) -> ServiceDef {
    parse_service(&syn::parse_str::<ItemTrait>(source).unwrap()).unwrap()
}

fn registration(source: &str) -> String {
    emit(&parsed(source)).to_token_stream().to_string()
}

#[cfg(feature = "zod")]
fn service_of(source: &str) -> String {
    service::emit(&parsed(source)).join("\n\n")
}

#[test]
fn a_one_way_operation_gets_no_result_type() {
    let published = result::emit(&parsed(MIXED_SERVICE));
    assert_eq!(published.len(), 3, "got: {published:?}");
    assert!(
        !published
            .iter()
            .any(|ts| ts.contains("UsageServiceApplyBundleResult")),
        "an operation that declared no reply has no arms to join. Got: {published:?}"
    );
}

#[test]
fn every_declared_message_is_registered_with_the_service() {
    let rendered = registration(MIXED_SERVICE);
    for declared in ["ExpireCreditRequest", "SweepRequest"] {
        assert!(
            rendered.contains(&format!("{declared} :: ts_definition")),
            "a message the macro declared reaches the bundle through the service's own line. \
             Got: {rendered}"
        );
    }
    assert!(
        !rendered.contains("AvailableBalanceRequest :: ts_definition"),
        "the message the author declared is registered by the author, not here. Got: {rendered}"
    );
}

#[test]
fn the_bundle_line_hangs_off_a_struct_named_for_the_service() {
    let rendered = registration(MIXED_SERVICE);
    assert!(
        rendered.contains("pub struct UsageServiceSchema"),
        "got: {rendered}"
    );
    assert!(
        rendered.contains("impl UsageServiceSchema"),
        "got: {rendered}"
    );
}

#[test]
fn the_fault_s_fields_are_asked_for_rather_than_written_here() {
    let rendered = registration(MIXED_SERVICE);
    for asked in [
        "usage_service_schema :: UsageServiceFaultFields :: ts_definition",
        "usage_service_schema :: UsageServiceFaultKind :: ts_definition",
    ] {
        assert!(
            rendered.contains(asked),
            "the fault's TypeScript comes from the same declaration the Rust dispatcher builds \
             faults from, never from a literal beside it. Got: {rendered}"
        );
    }
    assert!(
        !rendered.contains("export type ServiceFault ="),
        "a hand-maintained literal beside a generated type is how the two drift. Got: {rendered}"
    );
    // The seal is written here and the fields are not, so the sealed alias names the asked-for
    // type and spells no member of its own. A field written here is a field that can drift.
    for member in ["detail:", "field:", "kind:", "operation:"] {
        assert!(
            !rendered.contains(&format!("export type UsageServiceFault = {{\\n  {member}")),
            "the seal adds a brand and nothing else; the members stay the Rust declaration's. \
             Got: {rendered}"
        );
    }
}

/// The two declarations the seal is: a symbol the bundle exports nowhere, and the fault a caller
/// names, declared as the asked-for fields plus one property keyed on that symbol.
///
/// This is what TypeScript is given in place of the private fields Rust has. The Rust fault refuses
/// the literal an implementation would write with `E0451`, and a plain structural object type
/// refuses nothing at all.
#[test]
fn the_published_fault_is_the_asked_for_fields_under_a_brand_the_bundle_exports_nowhere() {
    let rendered = registration(MIXED_SERVICE);
    assert!(
        rendered.contains("declare const usageServiceFaultSeal: unique symbol;"),
        "a brand keyed on an exported name is a brand anyone can write. Got: {rendered}"
    );
    assert!(
        !rendered.contains("export declare const usageServiceFaultSeal"),
        "an exported symbol is one an implementation can name, and a property it can write. \
         Got: {rendered}"
    );
    assert!(
        rendered.contains(
            "export type UsageServiceFault = UsageServiceFaultFields & {\\n  readonly \
             [usageServiceFaultSeal]: true;\\n};"
        ),
        "the fault a caller names is the fields the Rust declaration published, plus the brand. \
         Got: {rendered}"
    );
    // The README states both halves where it documents the fault — what the seal stops, and the
    // assertion it does not — so the two cannot drift.
    let readme = include_str!("../../../README.md");
    assert!(
        readme.contains("**And the fault type itself refuses to be written.**")
            && readme.contains("declare const usageServiceFaultSeal: unique symbol;")
            && readme.contains("`built as UsageServiceFault` compiles"),
        "the README no longer says what the seal is, or that a type assertion still gets past it"
    );
}

#[test]
fn the_result_joins_the_two_declared_arms_and_adds_nothing_to_either() {
    let published = result::emit(&parsed(MIXED_SERVICE));
    let found = published
        .iter()
        .find(|ts| ts.contains("export type UsageServiceGetAvailableBalanceResult ="));
    assert!(found.is_some(), "got: {published:?}");
    let balance = found.unwrap();
    assert!(
        balance.contains("| { ok: true; value: AvailableBalanceResponse }"),
        "got: {balance}"
    );
    assert!(
        balance.contains(
            "| { ok: false; error: BalanceError | { isServiceFault: true; fault: \
             UsageServiceFault } };"
        ),
        "got: {balance}"
    );
}

/// `StreamedAnswer` carries no `#[model_schema()]` of its own, so a bare `value: StreamedAnswer`
/// would publish a TypeScript reference nothing declares. The result type stands in the fixed
/// streamed record instead.
#[cfg(feature = "zod")]
#[test]
fn the_result_answers_the_streamed_record_rather_than_the_undescribable_rust_type() {
    let published = result::emit(&parsed(STREAM_HTTP_SERVICE));
    let found = published
        .iter()
        .find(|ts| ts.contains("export type ContentClientServiceGetFileResult ="));
    assert!(found.is_some(), "got: {published:?}");
    let result = found.unwrap();
    assert!(
        result.contains(
            "| { ok: true; value: { contentRange: string | undefined; body: \
             ReadableStream<Uint8Array> } }"
        ),
        "got: {result}"
    );
    assert!(
        !result.contains("StreamedAnswer"),
        "the Rust-only seam type never leaks into the published TypeScript. Got: {result}"
    );
}

/// A declared `header_out` wraps the streamed record in a tuple, exactly as the JSON and bytes
/// paths compose theirs.
#[cfg(feature = "zod")]
#[test]
fn the_result_composes_header_out_onto_the_streamed_record_in_a_tuple() {
    let published = result::emit(&parsed(STREAM_HTTP_SERVICE));
    let found = published
        .iter()
        .find(|ts| ts.contains("export type ContentClientServiceGetTaggedFileResult ="));
    assert!(found.is_some(), "got: {published:?}");
    let result = found.unwrap();
    assert!(
        result.contains(
            "| { ok: true; value: [{ contentRange: string | undefined; body: \
             ReadableStream<Uint8Array> }, string] }"
        ),
        "got: {result}"
    );
}

#[test]
fn the_result_takes_its_name_from_the_service_and_the_operation() {
    let published = result::emit(&parsed(MIXED_SERVICE));
    for named in [
        "UsageServiceGetAvailableBalanceResult",
        "UsageServiceExpireCreditResult",
        "UsageServiceSweepResult",
    ] {
        assert!(
            published
                .iter()
                .any(|ts| ts.contains(&format!("export type {named} ="))),
            "a bundle carrying ten services is one flat file, so every name carries the service. \
             Got: {published:?}"
        );
    }
    assert!(
        !published
            .iter()
            .any(|ts| ts.contains("export type SweepResult =")),
        "an unprefixed result collides with any other service declaring the same operation. \
         Got: {published:?}"
    );
}

#[test]
fn two_operations_naming_unrelated_errors_keep_them_apart() {
    let published = result::emit(&parsed(MIXED_SERVICE));
    let found = published
        .iter()
        .find(|ts| ts.contains("export type UsageServiceExpireCreditResult ="));
    assert!(found.is_some(), "got: {published:?}");
    let expire = found.unwrap();
    assert!(
        expire.contains("error: CreditWriteError |"),
        "an operation's failure arm carries the error that operation declared, not the service's. \
         Got: {expire}"
    );
}

/// The pair that says a client and a dispatcher are published exactly where their check can be.
/// This is the half that runs in a build with the Zod surface; the one below it is the same
/// registration read in a build without it, and neither could pass alone.
#[cfg(feature = "zod")]
#[test]
fn a_build_that_publishes_a_schema_publishes_the_client_and_the_dispatcher_that_parse_it() {
    let rendered = registration(MIXED_SERVICE);
    for published in [
        "pub fn ts_client",
        "pub fn ts_http_client",
        "pub fn ts_service",
        "pub fn ts_ws_client",
        "pub fn ts_definition",
    ] {
        assert!(
            rendered.contains(published),
            "a build with a schema to parse against publishes all five artifacts. \
             Got: {rendered}"
        );
    }
}

/// A build with `typescript` on and `zod` off publishes the service's types and neither seam
/// artifact.
///
/// Both of them parse a message against the `<Message>$Schema` const `#[model_schema()]` writes,
/// and this build writes none. Emitting them without the parse is what this replaced: a client that
/// forwarded whatever it was handed, and a dispatcher that narrowed an unread payload with `as` and
/// gave it to an implementation entitled to assume it was valid. Both compiled, both read like the
/// checked ones, and the Rust half of the same service went on validating — so the two halves
/// disagreed about what they accept and nothing said so.
#[cfg(not(feature = "zod"))]
#[test]
fn a_build_that_publishes_no_schema_publishes_no_client_and_no_dispatcher() {
    let rendered = registration(MIXED_SERVICE);
    for withheld in [
        "pub fn ts_client",
        "pub fn ts_http_client",
        "pub fn ts_service",
        "pub fn ts_ws_client",
    ] {
        assert!(
            !rendered.contains(withheld),
            "an artifact that cannot hold the guarantee its callers are written against is not \
             published at all. Got: {rendered}"
        );
    }
    assert!(
        rendered.contains("pub fn ts_definition"),
        "the types describe what the Rust half puts on the wire and are true either way. \
         Got: {rendered}"
    );
    // The README states the consequence where it documents the requirement, so the two cannot
    // drift. Newlines collapse to spaces first: the sentence may wrap anywhere in the source.
    let readme = include_str!("../../../README.md").replace('\n', " ");
    assert!(
        readme.contains("**A service that publishes TypeScript needs the `zod` feature too.**")
            && readme.contains(
                "no `<Service>Schema::ts_client()`, no `<Service>Schema::ts_http_client()`, and"
            )
            && readme.contains("no `<Service>Schema::ts_service()`"),
        "the README no longer says what a build without the Zod surface publishes"
    );
}

/// The missing methods are the one thing a reader of this build's registry goes looking for, so
/// the reason they are missing is written on the registry itself rather than left to an
/// `E0599` naming the method and nothing else.
#[cfg(not(feature = "zod"))]
#[test]
fn a_build_that_publishes_no_client_says_on_the_registry_why_not() {
    let rendered = registration(MIXED_SERVICE);
    for said in [
        "This build publishes no `UsageServiceSchema::ts_client()`, no \
         `UsageServiceSchema::ts_http_client()`, and no `UsageServiceSchema::ts_service()`.",
        "only a build with tixschema's `zod` feature writes one",
        "Add `features = [\\\"zod\\\"]` to the tixschema dependency to get them.",
    ] {
        assert!(
            rendered.contains(said),
            "the registry's own rustdoc names the feature and what to add. Got: {rendered}"
        );
    }
}

/// What the Zod-less build still publishes, and therefore why it is not refused outright: the
/// message types and the result envelopes describe what the *Rust* dispatcher and client put on the
/// wire, and that half validates in this build exactly as it does in any other.
#[cfg(not(feature = "zod"))]
#[test]
fn a_build_that_publishes_no_client_still_publishes_every_type_the_wire_carries() {
    let rendered = registration(MIXED_SERVICE);
    for asked in [
        "ExpireCreditRequest :: ts_definition",
        "usage_service_schema :: UsageServiceFaultFields :: ts_definition",
        "declare const usageServiceFaultSeal: unique symbol;",
        "export type UsageServiceGetAvailableBalanceResult =",
    ] {
        assert!(
            rendered.contains(asked),
            "the types are what a hand-written caller of this service reads, and nothing about \
             them depends on the Zod surface. Got: {rendered}"
        );
    }
}

#[cfg(feature = "zod")]
#[test]
fn a_declared_message_brings_its_schema_along_with_its_type() {
    let rendered = registration(MIXED_SERVICE);
    assert!(
        rendered.contains("ExpireCreditRequest :: zod_schema"),
        "the schema has no registration line of its own either. Got: {rendered}"
    );
}
