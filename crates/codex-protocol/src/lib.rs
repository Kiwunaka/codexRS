use std::error::Error;
use std::fmt;
use std::io::{self, BufRead, Write};
use std::mem;
use std::num::NonZeroUsize;
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
pub const DEFAULT_MESSAGE_CHANNEL_CAPACITY: usize = 1;
pub const DEFAULT_COMMAND_CHANNEL_CAPACITY: usize = 32;
pub const DEFAULT_EVENT_CHANNEL_CAPACITY: usize = 512;
pub const MAX_PENDING_REQUESTS: usize = 64;
pub const MAX_INTERLEAVED_MESSAGES_PER_REQUEST: usize = 256;

const MAX_METHOD_BYTES: usize = 256;
const MAX_STRING_REQUEST_ID_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameTooLarge {
    pub limit: usize,
    pub observed_at_least: usize,
}

impl fmt::Display for FrameTooLarge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "protocol frame exceeded {} bytes (observed at least {})",
            self.limit, self.observed_at_least
        )
    }
}

impl Error for FrameTooLarge {}

#[derive(Debug)]
pub enum ProtocolError {
    Io(io::Error),
    FrameTooLarge(FrameTooLarge),
    InvalidJson,
    InvalidEnvelope(&'static str),
    InvalidResult,
    Serialization,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(_) => formatter.write_str("protocol I/O failed"),
            Self::FrameTooLarge(error) => error.fmt(formatter),
            Self::InvalidJson => formatter.write_str("app-server sent invalid JSON"),
            Self::InvalidEnvelope(reason) => {
                write!(formatter, "app-server sent an invalid envelope: {reason}")
            }
            Self::InvalidResult => {
                formatter.write_str("app-server response did not match the expected schema")
            }
            Self::Serialization => formatter.write_str("could not encode an app-server request"),
        }
    }
}

impl Error for ProtocolError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::FrameTooLarge(error) => Some(error),
            Self::InvalidJson
            | Self::InvalidEnvelope(_)
            | Self::InvalidResult
            | Self::Serialization => None,
        }
    }
}

impl From<io::Error> for ProtocolError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<FrameTooLarge> for ProtocolError {
    fn from(error: FrameTooLarge) -> Self {
        Self::FrameTooLarge(error)
    }
}

/// Incrementally decodes newline-delimited frames without an unbounded line
/// allocation.
#[derive(Debug)]
pub struct BoundedLineDecoder {
    max_frame_bytes: NonZeroUsize,
    buffer: Vec<u8>,
}

impl BoundedLineDecoder {
    #[must_use]
    pub fn new(max_frame_bytes: NonZeroUsize) -> Self {
        Self {
            max_frame_bytes,
            buffer: Vec::new(),
        }
    }

    pub fn feed(&mut self, chunk: &[u8]) -> Result<Vec<Vec<u8>>, FrameTooLarge> {
        let mut frames = Vec::new();

        for &byte in chunk {
            if byte == b'\n' {
                if self.buffer.last() == Some(&b'\r') {
                    self.buffer.pop();
                }
                frames.push(mem::take(&mut self.buffer));
                continue;
            }

            if self.buffer.len() == self.max_frame_bytes.get() {
                self.buffer.clear();
                return Err(frame_too_large(self.max_frame_bytes.get()));
            }

            self.buffer.push(byte);
        }

        Ok(frames)
    }

    #[must_use]
    pub fn finish(&mut self) -> Option<Vec<u8>> {
        if self.buffer.is_empty() {
            None
        } else {
            Some(mem::take(&mut self.buffer))
        }
    }
}

/// Reads one JSONL frame while keeping both the allocation and oversize
/// recovery bounded. An oversized line is consumed through its delimiter so a
/// caller can decide whether to continue.
pub fn read_bounded_frame<R: BufRead>(
    reader: &mut R,
    max_frame_bytes: NonZeroUsize,
) -> Result<Option<Vec<u8>>, ProtocolError> {
    let limit = max_frame_bytes.get();
    let mut frame = Vec::with_capacity(limit.min(8 * 1024));

    loop {
        let (available_len, newline_index) = {
            let available = reader.fill_buf()?;
            if available.is_empty() {
                if frame.is_empty() {
                    return Ok(None);
                }
                trim_carriage_return(&mut frame);
                return Ok(Some(frame));
            }
            (
                available.len(),
                available.iter().position(|byte| *byte == b'\n'),
            )
        };

        let content_len = newline_index.unwrap_or(available_len);
        if frame.len().saturating_add(content_len) > limit {
            let consume_len = newline_index.map_or(available_len, |index| index + 1);
            reader.consume(consume_len);
            if newline_index.is_none() {
                discard_through_newline(reader)?;
            }
            return Err(frame_too_large(limit).into());
        }

        {
            let available = reader.fill_buf()?;
            frame.extend_from_slice(&available[..content_len]);
        }
        let consume_len = newline_index.map_or(available_len, |index| index + 1);
        reader.consume(consume_len);

        if newline_index.is_some() {
            trim_carriage_return(&mut frame);
            return Ok(Some(frame));
        }
    }
}

fn discard_through_newline<R: BufRead>(reader: &mut R) -> Result<(), io::Error> {
    loop {
        let (available_len, newline_index) = {
            let available = reader.fill_buf()?;
            if available.is_empty() {
                return Ok(());
            }
            (
                available.len(),
                available.iter().position(|byte| *byte == b'\n'),
            )
        };
        reader.consume(newline_index.map_or(available_len, |index| index + 1));
        if newline_index.is_some() {
            return Ok(());
        }
    }
}

fn trim_carriage_return(frame: &mut Vec<u8>) {
    if frame.last() == Some(&b'\r') {
        frame.pop();
    }
}

fn frame_too_large(limit: usize) -> FrameTooLarge {
    FrameTooLarge {
        limit,
        observed_at_least: limit.saturating_add(1),
    }
}

#[derive(Debug, Serialize)]
pub struct ClientRequest<P> {
    pub method: &'static str,
    pub id: u64,
    pub params: P,
}

#[derive(Debug, Serialize)]
pub struct ClientNotification {
    pub method: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub name: String,
    pub title: Option<String>,
    pub version: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeCapabilities {
    pub experimental_api: bool,
    pub request_attestation: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_server_openai_form_elicitation: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opt_out_notification_methods: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub client_info: ClientInfo,
    pub capabilities: Option<InitializeCapabilities>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResponse {
    pub user_agent: String,
    pub codex_home: PathBuf,
    pub platform_family: String,
    pub platform_os: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadSortKey {
    RecencyAt,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    Desc,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadListParams {
    pub limit: u32,
    pub sort_key: ThreadSortKey,
    pub sort_direction: SortDirection,
    pub use_state_db_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_term: Option<String>,
}

impl ThreadListParams {
    #[must_use]
    pub const fn state_db_page(limit: u32) -> Self {
        Self {
            limit,
            sort_key: ThreadSortKey::RecencyAt,
            sort_direction: SortDirection::Desc,
            use_state_db_only: true,
            cursor: None,
            archived: None,
            cwd: None,
            search_term: None,
        }
    }

    #[must_use]
    pub fn with_cursor(mut self, cursor: Option<String>) -> Self {
        self.cursor = cursor;
        self
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSummary {
    pub id: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub forked_from_id: Option<String>,
    #[serde(default)]
    pub parent_thread_id: Option<String>,
    #[serde(default)]
    pub preview: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub cwd: PathBuf,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
    #[serde(default)]
    pub recency_at: Option<i64>,
    #[serde(default)]
    pub status: Value,
    #[serde(default)]
    pub git_info: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadListResponse {
    pub data: Vec<ThreadSummary>,
    pub next_cursor: Option<String>,
    pub backwards_cursor: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HistorySortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadTurnsListParams {
    pub thread_id: String,
    pub limit: u32,
    pub sort_direction: HistorySortDirection,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items_view: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadTurnsListResponse {
    pub data: Vec<Value>,
    pub next_cursor: Option<String>,
    pub backwards_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadItemsListParams {
    pub thread_id: String,
    pub limit: u32,
    pub sort_direction: HistorySortDirection,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadItemEntry {
    pub turn_id: String,
    pub item: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadItemsListResponse {
    pub data: Vec<ThreadItemEntry>,
    pub next_cursor: Option<String>,
    pub backwards_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadReadParams {
    pub thread_id: String,
    pub include_turns: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThreadReadResponse {
    pub thread: ThreadSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UserInput {
    Text {
        text: String,
        #[serde(rename = "text_elements")]
        text_elements: Vec<Value>,
    },
    LocalImage {
        path: PathBuf,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
}

impl UserInput {
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text {
            text: text.into(),
            text_elements: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolFunction {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defer_loading: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DynamicToolNamespaceTool {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(flatten)]
    function: DynamicToolFunction,
}

impl DynamicToolNamespaceTool {
    #[must_use]
    pub const fn new(function: DynamicToolFunction) -> Self {
        Self {
            kind: "function",
            function,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DynamicToolSpec {
    Function {
        #[serde(flatten)]
        function: DynamicToolFunction,
    },
    Namespace {
        name: String,
        description: String,
        tools: Vec<DynamicToolNamespaceTool>,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolCallParams {
    pub thread_id: String,
    pub turn_id: String,
    pub call_id: String,
    pub namespace: Option<String>,
    pub tool: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DynamicToolCallOutputContentItem {
    InputText {
        text: String,
    },
    InputImage {
        #[serde(rename = "imageUrl")]
        image_url: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolCallResponse {
    pub content_items: Vec<DynamicToolCallOutputContentItem>,
    pub success: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStartParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_workspace_roots: Option<Vec<PathBuf>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ephemeral: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_tools: Option<Vec<DynamicToolSpec>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThreadStartResponse {
    pub thread: ThreadSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadForkParams {
    pub thread_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_turn_id: Option<String>,
    pub exclude_turns: bool,
    pub defer_goal_continuation: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThreadForkResponse {
    pub thread: ThreadSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadResumeParams {
    pub thread_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_turns: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_turns_page: Option<ThreadResumeInitialTurnsPageParams>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadResumeInitialTurnsPageParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    pub limit: u32,
    pub sort_direction: HistorySortDirection,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items_view: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThreadResumeResponse {
    pub thread: ThreadSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartParams {
    pub thread_id: String,
    pub input: Vec<UserInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_workspace_roots: Option<Vec<PathBuf>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TurnStartResponse {
    pub turn: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnInterruptParams {
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnSteerParams {
    pub thread_id: String,
    pub input: Vec<UserInput>,
    pub expected_turn_id: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwds: Option<Vec<PathBuf>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marketplace_kinds: Option<Vec<String>>,
    pub force_refetch: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginListResponse {
    pub marketplaces: Vec<PluginMarketplace>,
    #[serde(default)]
    pub marketplace_load_errors: Vec<Value>,
    #[serde(default)]
    pub featured_plugin_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginMarketplace {
    pub name: String,
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub plugins: Vec<PluginSummary>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSummary {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default, rename = "interface")]
    pub presentation: Option<PluginPresentation>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPresentation {
    pub display_name: Option<String>,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub developer_name: Option<String>,
    pub category: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub logo_url: Option<String>,
    pub logo_url_dark: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marketplace_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_marketplace_name: Option<String>,
    pub plugin_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallResponse {
    #[serde(default)]
    pub apps_needing_auth: Vec<AppSummary>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub install_url: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginUninstallParams {
    pub plugin_id: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppsListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    pub limit: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    pub force_refetch: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppsListResponse {
    pub data: Vec<AppInfo>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub logo_url: Option<String>,
    pub logo_url_dark: Option<String>,
    pub install_url: Option<String>,
    #[serde(default)]
    pub is_accessible: bool,
    #[serde(default)]
    pub is_enabled: bool,
    #[serde(default)]
    pub plugin_display_names: Vec<String>,
}

#[derive(Debug)]
pub enum IncomingMessage {
    Success {
        id: Value,
        result: Value,
    },
    Failure {
        id: Value,
        code: i64,
    },
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    Notification {
        method: String,
        params: Value,
    },
}

pub fn encode_json_line<T: Serialize>(
    value: &T,
    max_frame_bytes: NonZeroUsize,
) -> Result<Vec<u8>, ProtocolError> {
    let mut writer = BoundedWriter::new(max_frame_bytes.get());
    if serde_json::to_writer(&mut writer, value).is_err() {
        if writer.exceeded {
            return Err(frame_too_large(max_frame_bytes.get()).into());
        }
        return Err(ProtocolError::Serialization);
    }
    writer.bytes.push(b'\n');
    Ok(writer.bytes)
}

pub fn decode_incoming(frame: &[u8]) -> Result<IncomingMessage, ProtocolError> {
    let value: Value = serde_json::from_slice(frame).map_err(|_| ProtocolError::InvalidJson)?;
    let mut object = match value {
        Value::Object(object) => object,
        _ => return Err(ProtocolError::InvalidEnvelope("top level is not an object")),
    };

    if let Some(method_value) = object.remove("method") {
        let method = match method_value {
            Value::String(method) if method.len() <= MAX_METHOD_BYTES => method,
            Value::String(_) => {
                return Err(ProtocolError::InvalidEnvelope("method name is too long"));
            }
            _ => return Err(ProtocolError::InvalidEnvelope("method is not a string")),
        };

        let params = object.remove("params").unwrap_or(Value::Null);
        return match object.remove("id") {
            Some(id) => {
                validate_request_id(&id)?;
                Ok(IncomingMessage::Request { id, method, params })
            }
            None => Ok(IncomingMessage::Notification { method, params }),
        };
    }

    let id = object
        .remove("id")
        .ok_or(ProtocolError::InvalidEnvelope("response has no id"))?;
    validate_request_id(&id)?;

    match (object.remove("result"), object.remove("error")) {
        (Some(result), None) => Ok(IncomingMessage::Success { id, result }),
        (None, Some(Value::Object(error))) => {
            let code =
                error
                    .get("code")
                    .and_then(Value::as_i64)
                    .ok_or(ProtocolError::InvalidEnvelope(
                        "error response has no integer code",
                    ))?;
            Ok(IncomingMessage::Failure { id, code })
        }
        (None, Some(_)) => Err(ProtocolError::InvalidEnvelope(
            "error response is not an object",
        )),
        (Some(_), Some(_)) => Err(ProtocolError::InvalidEnvelope(
            "response has both result and error",
        )),
        (None, None) => Err(ProtocolError::InvalidEnvelope(
            "response has neither result nor error",
        )),
    }
}

pub fn decode_result<T: DeserializeOwned>(result: Value) -> Result<T, ProtocolError> {
    serde_json::from_value(result).map_err(|_| ProtocolError::InvalidResult)
}

pub fn encode_unsupported_request(
    id: &Value,
    max_frame_bytes: NonZeroUsize,
) -> Result<Vec<u8>, ProtocolError> {
    validate_request_id(id)?;
    encode_json_line(
        &ErrorResponse {
            id,
            error: ErrorObject {
                code: -32601,
                message: "method not supported by this client",
            },
        },
        max_frame_bytes,
    )
}

pub fn encode_success_response<T: Serialize>(
    id: &Value,
    result: &T,
    max_frame_bytes: NonZeroUsize,
) -> Result<Vec<u8>, ProtocolError> {
    validate_request_id(id)?;
    encode_json_line(&SuccessResponse { id, result }, max_frame_bytes)
}

pub fn encode_error_response(
    id: &Value,
    code: i64,
    message: &'static str,
    max_frame_bytes: NonZeroUsize,
) -> Result<Vec<u8>, ProtocolError> {
    validate_request_id(id)?;
    encode_json_line(
        &ErrorResponse {
            id,
            error: ErrorObject { code, message },
        },
        max_frame_bytes,
    )
}

fn validate_request_id(id: &Value) -> Result<(), ProtocolError> {
    match id {
        Value::Number(_) => Ok(()),
        Value::String(value) if value.len() <= MAX_STRING_REQUEST_ID_BYTES => Ok(()),
        Value::String(_) => Err(ProtocolError::InvalidEnvelope("request id is too long")),
        _ => Err(ProtocolError::InvalidEnvelope(
            "request id is not a string or number",
        )),
    }
}

#[derive(Serialize)]
struct ErrorResponse<'a> {
    id: &'a Value,
    error: ErrorObject<'a>,
}

#[derive(Serialize)]
struct SuccessResponse<'a, T> {
    id: &'a Value,
    result: &'a T,
}

#[derive(Serialize)]
struct ErrorObject<'a> {
    code: i64,
    message: &'a str,
}

struct BoundedWriter {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}

impl BoundedWriter {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(limit.min(8 * 1024)),
            limit,
            exceeded: false,
        }
    }
}

impl Write for BoundedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.len() > self.limit.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(io::Error::other("bounded protocol writer is full"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor};
    use std::num::NonZeroUsize;

    use serde_json::json;

    use super::{
        BoundedLineDecoder, ClientInfo, ClientNotification, ClientRequest,
        DynamicToolCallOutputContentItem, DynamicToolCallParams, DynamicToolCallResponse,
        DynamicToolFunction, DynamicToolNamespaceTool, DynamicToolSpec, IncomingMessage,
        InitializeParams, ProtocolError, ThreadListParams, UserInput, decode_incoming,
        encode_json_line, read_bounded_frame,
    };

    fn limit(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).unwrap_or(NonZeroUsize::MIN)
    }

    fn encoded<T: serde::Serialize>(value: &T) -> Vec<u8> {
        match encode_json_line(value, limit(1024)) {
            Ok(frame) => frame,
            Err(error) => panic!("unexpected encoding error: {error}"),
        }
    }

    #[test]
    fn decodes_fragmented_and_crlf_frames() {
        let mut decoder = BoundedLineDecoder::new(limit(32));

        let first = decoder.feed(b"{\"id\":");
        assert!(matches!(first, Ok(frames) if frames.is_empty()));
        let frames = match decoder.feed(b"1}\r\n{\"id\":2}\n") {
            Ok(frames) => frames,
            Err(error) => panic!("unexpected frame error: {error}"),
        };

        assert_eq!(frames, [br#"{"id":1}"#.to_vec(), br#"{"id":2}"#.to_vec()]);
    }

    #[test]
    fn rejects_oversized_frame_before_growing_further() {
        let mut decoder = BoundedLineDecoder::new(limit(4));

        let error = match decoder.feed(b"12345") {
            Err(error) => error,
            Ok(frames) => panic!("expected an oversized-frame error, got {frames:?}"),
        };

        assert_eq!(error.limit, 4);
        assert_eq!(error.observed_at_least, 5);
        assert!(decoder.finish().is_none());
    }

    #[test]
    fn returns_final_unterminated_frame() {
        let mut decoder = BoundedLineDecoder::new(limit(16));
        let frames = decoder.feed(b"last frame");
        assert!(matches!(frames, Ok(frames) if frames.is_empty()));
        assert_eq!(decoder.finish(), Some(b"last frame".to_vec()));
    }

    #[test]
    fn reader_rejects_and_consumes_one_oversized_line() {
        let mut reader = BufReader::new(Cursor::new(b"12345\nok\n"));

        assert!(matches!(
            read_bounded_frame(&mut reader, limit(4)),
            Err(ProtocolError::FrameTooLarge(_))
        ));
        assert_eq!(
            read_bounded_frame(&mut reader, limit(4)).ok().flatten(),
            Some(b"ok".to_vec())
        );
    }

    #[test]
    fn initialize_wire_shape_matches_generated_schema() {
        let request = ClientRequest {
            method: "initialize",
            id: 0,
            params: InitializeParams {
                client_info: ClientInfo {
                    name: "codex-rs".to_owned(),
                    title: Some("codexRS".to_owned()),
                    version: "0.1.0".to_owned(),
                },
                capabilities: None,
            },
        };

        assert_eq!(
            encoded(&request),
            b"{\"method\":\"initialize\",\"id\":0,\"params\":{\"clientInfo\":{\"name\":\"codex-rs\",\"title\":\"codexRS\",\"version\":\"0.1.0\"},\"capabilities\":null}}\n"
        );
        assert_eq!(
            encoded(&ClientNotification {
                method: "initialized"
            }),
            b"{\"method\":\"initialized\"}\n"
        );
    }

    #[test]
    fn thread_list_is_bounded_to_state_database_metadata() {
        let request = ClientRequest {
            method: "thread/list",
            id: 7,
            params: ThreadListParams::state_db_page(20),
        };

        assert_eq!(
            encoded(&request),
            b"{\"method\":\"thread/list\",\"id\":7,\"params\":{\"limit\":20,\"sortKey\":\"recency_at\",\"sortDirection\":\"desc\",\"useStateDbOnly\":true}}\n"
        );
    }

    #[test]
    fn decodes_success_without_requiring_jsonrpc_field() {
        let message = match decode_incoming(br#"{"id":3,"result":{"ok":true}}"#) {
            Ok(message) => message,
            Err(error) => panic!("unexpected decoding error: {error}"),
        };

        assert!(matches!(
            message,
            IncomingMessage::Success { id, result }
                if id == json!(3) && result == json!({"ok": true})
        ));
    }

    #[test]
    fn preserves_server_event_params_for_the_router() {
        let message = match decode_incoming(
            br#"{"method":"item/tool/call","id":"call-1","params":{"tool":"screenshot"}}"#,
        ) {
            Ok(message) => message,
            Err(error) => panic!("unexpected decoding error: {error}"),
        };

        assert!(matches!(
            message,
            IncomingMessage::Request { id, method, params }
                if id == json!("call-1")
                    && method == "item/tool/call"
                    && params == json!({"tool": "screenshot"})
        ));
    }

    #[test]
    fn dynamic_tool_call_matches_the_stable_app_server_schema() {
        let params = serde_json::from_value::<DynamicToolCallParams>(json!({
            "threadId": "thread-1",
            "turnId": "turn-1",
            "callId": "call-1",
            "namespace": "computer_use",
            "tool": "screenshot",
            "arguments": {}
        }));
        assert!(matches!(
            params,
            Ok(params)
                if params.thread_id == "thread-1"
                    && params.turn_id == "turn-1"
                    && params.call_id == "call-1"
                    && params.namespace.as_deref() == Some("computer_use")
                    && params.tool == "screenshot"
        ));

        let response = DynamicToolCallResponse {
            content_items: vec![
                DynamicToolCallOutputContentItem::InputText {
                    text: "1600x900".to_owned(),
                },
                DynamicToolCallOutputContentItem::InputImage {
                    image_url: "data:image/jpeg;base64,AA==".to_owned(),
                },
            ],
            success: true,
        };
        assert_eq!(
            serde_json::to_value(response).ok(),
            Some(json!({
                "contentItems": [
                    {"type": "inputText", "text": "1600x900"},
                    {"type": "inputImage", "imageUrl": "data:image/jpeg;base64,AA=="}
                ],
                "success": true
            }))
        );
    }

    #[test]
    fn computer_use_namespace_matches_dynamic_tool_schema() {
        let specification = DynamicToolSpec::Namespace {
            name: "computer_use".to_owned(),
            description: "Control a user-selected desktop window.".to_owned(),
            tools: vec![DynamicToolNamespaceTool::new(DynamicToolFunction {
                name: "screenshot".to_owned(),
                description: "Capture the selected window.".to_owned(),
                input_schema: json!({"type": "object"}),
                defer_loading: None,
            })],
        };

        assert_eq!(
            serde_json::to_value(specification).ok(),
            Some(json!({
                "type": "namespace",
                "name": "computer_use",
                "description": "Control a user-selected desktop window.",
                "tools": [{
                    "type": "function",
                    "name": "screenshot",
                    "description": "Capture the selected window.",
                    "inputSchema": {"type": "object"}
                }]
            }))
        );
    }

    #[test]
    fn text_input_includes_required_empty_element_list() {
        assert_eq!(
            serde_json::to_value(UserInput::text("hello")).ok(),
            Some(json!({
                "type": "text",
                "text": "hello",
                "text_elements": []
            }))
        );
    }

    #[test]
    fn bounded_encoder_refuses_oversized_output() {
        assert!(matches!(
            encode_json_line(&"12345", limit(4)),
            Err(ProtocolError::FrameTooLarge(_))
        ));
    }
}
