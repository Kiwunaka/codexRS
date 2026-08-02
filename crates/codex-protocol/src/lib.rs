use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::io::{self, BufRead, Write};
use std::mem;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::str;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<P>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanType {
    Free,
    Go,
    Plus,
    Pro,
    Prolite,
    Team,
    SelfServeBusinessUsageBased,
    Business,
    EnterpriseCbpUsageBased,
    Enterprise,
    Edu,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type")]
pub enum Account {
    #[serde(rename = "apiKey")]
    ApiKey,
    #[serde(rename = "chatgpt")]
    ChatGpt {
        email: Option<String>,
        #[serde(rename = "planType")]
        plan_type: PlanType,
    },
    #[serde(rename = "amazonBedrock")]
    AmazonBedrock {
        #[serde(rename = "usesCodexManagedCredentials")]
        uses_codex_managed_credentials: bool,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAccountParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAccountResponse {
    pub account: Option<Account>,
    pub requires_openai_auth: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAuthStatusParams {
    pub include_token: bool,
    pub refresh_token: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffToRemoteParams {
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffToRemoteResponse {
    pub sha: String,
    pub diff: String,
}

pub struct SecretString {
    bytes: Vec<u8>,
}

impl SecretString {
    #[must_use]
    pub fn new(value: String) -> Option<Self> {
        if value.is_empty() || value.len() > 512 {
            let mut bytes = value.into_bytes();
            bytes.fill(0);
            return None;
        }

        Some(Self {
            bytes: value.into_bytes(),
        })
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        str::from_utf8(&self.bytes).unwrap_or_default()
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

impl Serialize for SecretString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.expose())
    }
}

impl<'de> Deserialize<'de> for SecretString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer).map(|value| Self {
            bytes: value.into_bytes(),
        })
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        self.bytes.fill(0);
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAuthStatusResponse {
    pub auth_method: Option<String>,
    pub auth_token: Option<SecretString>,
    pub account_id: Option<String>,
    pub requires_openai_auth: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum LoginAccountParams {
    #[serde(rename = "apiKey")]
    ApiKey {
        #[serde(rename = "apiKey")]
        api_key: SecretString,
    },
    #[serde(rename = "amazonBedrock")]
    AmazonBedrock {
        #[serde(rename = "apiKey")]
        api_key: SecretString,
        region: String,
    },
    #[serde(rename = "chatgpt")]
    ChatGpt {
        #[serde(
            rename = "codexStreamlinedLogin",
            skip_serializing_if = "Option::is_none"
        )]
        codex_streamlined_login: Option<bool>,
        #[serde(
            rename = "useHostedLoginSuccessPage",
            skip_serializing_if = "Option::is_none"
        )]
        use_hosted_login_success_page: Option<bool>,
        #[serde(rename = "appBrand", skip_serializing_if = "Option::is_none")]
        app_brand: Option<LoginAppBrand>,
    },
    #[serde(rename = "chatgptDeviceCode")]
    ChatGptDeviceCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LoginAppBrand {
    Codex,
    ChatGpt,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type")]
pub enum LoginAccountResponse {
    #[serde(rename = "apiKey")]
    ApiKey,
    #[serde(rename = "chatgpt")]
    ChatGpt {
        #[serde(rename = "loginId")]
        login_id: String,
        #[serde(rename = "authUrl")]
        auth_url: String,
    },
    #[serde(rename = "chatgptDeviceCode")]
    ChatGptDeviceCode {
        #[serde(rename = "loginId")]
        login_id: String,
        #[serde(rename = "verificationUrl")]
        verification_url: String,
        #[serde(rename = "userCode")]
        user_code: String,
    },
    #[serde(rename = "chatgptAuthTokens")]
    ChatGptAuthTokens,
    #[serde(rename = "amazonBedrock")]
    AmazonBedrock,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelLoginAccountParams {
    pub login_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CancelLoginAccountStatus {
    Canceled,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CancelLoginAccountResponse {
    pub status: CancelLoginAccountStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LogoutAccountResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackUploadParams {
    pub classification: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_log_files: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_logs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackUploadResponse {
    pub thread_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountLoginCompletedNotification {
    pub login_id: Option<String>,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitWindow {
    pub used_percent: f64,
    pub window_duration_mins: Option<i64>,
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditsSnapshot {
    pub has_credits: bool,
    pub unlimited: bool,
    pub balance: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitResetCreditsSummary {
    pub available_count: i64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitSnapshot {
    pub limit_id: Option<String>,
    pub limit_name: Option<String>,
    pub primary: Option<RateLimitWindow>,
    pub secondary: Option<RateLimitWindow>,
    pub credits: Option<CreditsSnapshot>,
    pub plan_type: Option<PlanType>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAccountRateLimitsResponse {
    pub rate_limits: RateLimitSnapshot,
    pub rate_limit_reset_credits: Option<RateLimitResetCreditsSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAccountTokenUsageResponse {
    pub summary: AccountTokenUsageSummary,
    #[serde(default)]
    pub daily_usage_buckets: Vec<AccountTokenUsageDailyBucket>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountTokenUsageSummary {
    pub lifetime_tokens: Option<i64>,
    pub peak_daily_tokens: Option<i64>,
    pub longest_running_turn_sec: Option<i64>,
    pub current_streak_days: Option<i64>,
    pub longest_streak_days: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountTokenUsageDailyBucket {
    pub start_date: String,
    pub tokens: i64,
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

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ThreadSourceKind {
    Cli,
    #[serde(rename = "vscode")]
    VsCode,
    Exec,
    AppServer,
    SubAgent,
    SubAgentReview,
    SubAgentCompact,
    SubAgentThreadSpawn,
    SubAgentOther,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSearchParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_key: Option<ThreadSortKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_direction: Option<SortDirection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_kinds: Option<Vec<ThreadSourceKind>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
    pub search_term: String,
}

impl ThreadSearchParams {
    #[must_use]
    pub fn interactive_page(search_term: String, limit: u32) -> Self {
        Self {
            cursor: None,
            limit: Some(limit),
            sort_key: Some(ThreadSortKey::RecencyAt),
            sort_direction: Some(SortDirection::Desc),
            source_kinds: Some(Vec::new()),
            archived: Some(false),
            search_term,
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
    #[serde(default)]
    pub turns: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadListResponse {
    pub data: Vec<ThreadSummary>,
    pub next_cursor: Option<String>,
    pub backwards_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadLoadedListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    pub limit: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadLoadedListResponse {
    pub data: Vec<String>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSearchResult {
    pub thread: ThreadSummary,
    pub snippet: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSearchResponse {
    pub data: Vec<ThreadSearchResult>,
    pub next_cursor: Option<String>,
    pub backwards_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FuzzyFileSearchSessionStartParams {
    pub session_id: String,
    pub roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FuzzyFileSearchSessionUpdateParams {
    pub session_id: String,
    pub query: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FuzzyFileSearchSessionStopParams {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FuzzyFileSearchParams {
    pub query: String,
    pub roots: Vec<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancellation_token: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FuzzyFileSearchMatchType {
    File,
    Directory,
}

/// Superset of the file-search engine's native match payload.
#[derive(Debug, Clone, Deserialize)]
pub struct FuzzyFileSearchResult {
    pub file_name: String,
    pub indices: Option<Vec<u32>>,
    pub match_type: FuzzyFileSearchMatchType,
    pub path: PathBuf,
    pub root: PathBuf,
    pub score: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FuzzyFileSearchResponse {
    pub files: Vec<FuzzyFileSearchResult>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FuzzyFileSearchSessionUpdatedNotification {
    pub session_id: String,
    pub query: String,
    pub files: Vec<FuzzyFileSearchResult>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FuzzyFileSearchSessionCompletedNotification {
    pub session_id: String,
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
pub struct ThreadBackgroundTerminalsListParams {
    pub thread_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadBackgroundTerminal {
    pub item_id: String,
    pub process_id: String,
    pub command: String,
    pub cwd: PathBuf,
    pub os_pid: Option<u32>,
    pub cpu_percent: Option<f64>,
    pub rss_kb: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadBackgroundTerminalsListResponse {
    pub data: Vec<ThreadBackgroundTerminal>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadBackgroundTerminalsTerminateParams {
    pub thread_id: String,
    pub process_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThreadBackgroundTerminalsTerminateResponse {
    pub terminated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadBackgroundTerminalsCleanParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThreadBackgroundTerminalsCleanResponse {}

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
    Mention {
        name: String,
        path: PathBuf,
    },
    Skill {
        name: String,
        path: PathBuf,
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

    #[must_use]
    pub fn local_image(path: PathBuf) -> Self {
        Self::LocalImage { path, detail: None }
    }

    #[must_use]
    pub fn mention(name: impl Into<String>, path: PathBuf) -> Self {
        Self::Mention {
            name: name.into(),
            path,
        }
    }

    #[must_use]
    pub fn skill(name: impl Into<String>, path: PathBuf) -> Self {
        Self::Skill {
            name: name.into(),
            path,
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

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequestUserInputParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub questions: Vec<ToolRequestUserInputQuestion>,
    #[serde(default)]
    pub auto_resolution_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequestUserInputQuestion {
    pub id: String,
    pub header: String,
    pub question: String,
    #[serde(default)]
    pub options: Option<Vec<ToolRequestUserInputOption>>,
    #[serde(default)]
    pub is_other: bool,
    #[serde(default)]
    pub is_secret: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ToolRequestUserInputOption {
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolRequestUserInputResponse {
    pub answers: BTreeMap<String, ToolRequestUserInputAnswer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolRequestUserInputAnswer {
    pub answers: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecutionRequestApprovalParams {
    #[serde(default)]
    pub approval_id: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub command_actions: Option<Vec<CommandAction>>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub environment_id: Option<String>,
    pub item_id: String,
    #[serde(default)]
    pub network_approval_context: Option<NetworkApprovalContext>,
    #[serde(default)]
    pub proposed_execpolicy_amendment: Option<Vec<String>>,
    #[serde(default)]
    pub proposed_network_policy_amendments: Option<Vec<NetworkPolicyAmendment>>,
    #[serde(default)]
    pub reason: Option<String>,
    pub started_at_ms: i64,
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandAction {
    #[serde(rename = "type")]
    pub kind: CommandActionKind,
    pub command: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub query: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CommandActionKind {
    Read,
    ListFiles,
    Search,
    Unknown,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct NetworkApprovalContext {
    pub host: String,
    pub protocol: NetworkApprovalProtocol,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum NetworkApprovalProtocol {
    Http,
    Https,
    Socks5Tcp,
    Socks5Udp,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
pub struct NetworkPolicyAmendment {
    pub action: NetworkPolicyRuleAction,
    pub host: String,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NetworkPolicyRuleAction {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeRequestApprovalParams {
    #[serde(default)]
    pub grant_root: Option<String>,
    pub item_id: String,
    #[serde(default)]
    pub reason: Option<String>,
    pub started_at_ms: i64,
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsRequestApprovalParams {
    pub cwd: String,
    #[serde(default)]
    pub environment_id: Option<String>,
    pub item_id: String,
    pub permissions: PermissionProfile,
    #[serde(default)]
    pub reason: Option<String>,
    pub started_at_ms: i64,
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_system: Option<AdditionalFileSystemPermissions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<AdditionalNetworkPermissions>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdditionalFileSystemPermissions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entries: Option<Vec<FileSystemSandboxEntry>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glob_scan_max_depth: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub write: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct AdditionalNetworkPermissions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
pub struct FileSystemSandboxEntry {
    pub access: FileSystemAccessMode,
    pub path: FileSystemPath,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FileSystemAccessMode {
    Read,
    Write,
    Deny,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FileSystemPath {
    Path { path: String },
    GlobPattern { pattern: String },
    Special { value: FileSystemSpecialPath },
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FileSystemSpecialPath {
    Root,
    Minimal,
    ProjectRoots {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subpath: Option<String>,
    },
    Tmpdir,
    SlashTmp,
    Unknown {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subpath: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandExecutionRequestApprovalResponse {
    pub decision: CommandExecutionApprovalDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum CommandExecutionApprovalDecision {
    Value(CommandExecutionApprovalDecisionValue),
    AcceptWithExecpolicyAmendment {
        #[serde(rename = "acceptWithExecpolicyAmendment")]
        accept_with_execpolicy_amendment: ExecpolicyAmendment,
    },
    ApplyNetworkPolicyAmendment {
        #[serde(rename = "applyNetworkPolicyAmendment")]
        apply_network_policy_amendment: NetworkPolicyAmendmentDecision,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandExecutionApprovalDecisionValue {
    Accept,
    AcceptForSession,
    Decline,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExecpolicyAmendment {
    pub execpolicy_amendment: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NetworkPolicyAmendmentDecision {
    pub network_policy_amendment: NetworkPolicyAmendment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileChangeRequestApprovalResponse {
    pub decision: FileChangeApprovalDecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FileChangeApprovalDecision {
    Accept,
    AcceptForSession,
    Decline,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsRequestApprovalResponse {
    pub permissions: PermissionProfile,
    pub scope: PermissionGrantScope,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict_auto_review: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionGrantScope {
    Turn,
    Session,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStartParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_provider_model_fallback: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_workspace_roots: Option<Vec<PathBuf>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approvals_reviewer: Option<ApprovalsReviewer>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub personality: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental_raw_events: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
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
    pub cwd: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_turns: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defer_goal_continuation: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThreadForkResponse {
    pub thread: ThreadSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRollbackParams {
    pub thread_id: String,
    pub num_turns: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThreadRollbackResponse {
    pub thread: ThreadSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadCompactStartParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThreadCompactStartResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStartParams {
    pub thread_id: String,
    pub target: ReviewTarget,
    #[serde(default)]
    pub delivery: Option<ReviewDelivery>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReviewDelivery {
    Inline,
    Detached,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ReviewTarget {
    UncommittedChanges,
    BaseBranch { branch: String },
    Commit { sha: String, title: Option<String> },
    Custom { instructions: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStartResponse {
    pub turn: Value,
    pub review_thread_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadShellCommandParams {
    pub thread_id: String,
    pub command: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThreadShellCommandResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsageBreakdown {
    pub total_tokens: i64,
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub cache_write_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadTokenUsage {
    pub total: TokenUsageBreakdown,
    pub last: TokenUsageBreakdown,
    pub model_context_window: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadTokenUsageUpdatedNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub token_usage: ThreadTokenUsage,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSafetyBufferingUpdatedNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub model: String,
    pub use_cases: Vec<String>,
    pub reasons: Vec<String>,
    pub show_buffering_ui: bool,
    pub faster_model: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelVerification {
    TrustedAccessForCyber,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelVerificationNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub verifications: Vec<ModelVerification>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnDiffUpdatedNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub diff: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadArchiveParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadUnsubscribeParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThreadUnsubscribeStatus {
    NotLoaded,
    NotSubscribed,
    Unsubscribed,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ThreadUnsubscribeResponse {
    pub status: ThreadUnsubscribeStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadUnarchiveParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThreadUnarchiveResponse {
    pub thread: ThreadSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadDeleteParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSetNameParams {
    pub thread_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThreadGoalStatus {
    Active,
    Paused,
    Blocked,
    UsageLimited,
    BudgetLimited,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadGoal {
    pub thread_id: String,
    pub objective: String,
    pub status: ThreadGoalStatus,
    pub tokens_used: i64,
    pub token_budget: Option<i64>,
    pub time_used_seconds: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadGoalSetParams {
    pub thread_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub objective: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ThreadGoalStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<Option<i64>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThreadGoalSetResponse {
    pub goal: ThreadGoal,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadGoalGetParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThreadGoalGetResponse {
    #[serde(default)]
    pub goal: Option<ThreadGoal>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadGoalClearParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThreadGoalClearResponse {
    pub cleared: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadGoalUpdatedNotification {
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub goal: ThreadGoal,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadGoalClearedNotification {
    pub thread_id: String,
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
#[serde(rename_all = "camelCase")]
pub struct ThreadResumeResponse {
    pub thread: ThreadSummary,
    #[serde(default)]
    pub initial_turns_page: Option<ThreadTurnsListResponse>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    #[serde(default)]
    pub service_tier: Option<String>,
    #[serde(default)]
    pub approval_policy: Option<Value>,
    #[serde(default)]
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    #[serde(default)]
    pub active_permission_profile: Option<ActivePermissionProfile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ActivePermissionProfile {
    pub id: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSettingsUpdateParams {
    pub thread_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<Option<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThreadSettingsUpdateResponse {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThreadMemoryMode {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadMemoryModeSetParams {
    pub thread_id: String,
    pub mode: ThreadMemoryMode,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThreadMemoryModeSetResponse {}

#[derive(Debug, Clone, Deserialize)]
pub struct MemoryResetResponse {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExternalAgentConfigMigrationItemType {
    AgentsMd,
    Config,
    Skills,
    Plugins,
    McpServerConfig,
    Subagents,
    Hooks,
    Commands,
    Memory,
    Sessions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalAgentNamedMigration {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentPluginsMigration {
    pub marketplace_name: String,
    pub plugin_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalAgentSessionMigration {
    pub cwd: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentMigrationDetails {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<ExternalAgentNamedMigration>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hooks: Vec<ExternalAgentNamedMigration>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp_servers: Vec<ExternalAgentNamedMigration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugins: Vec<ExternalAgentPluginsMigration>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sessions: Vec<ExternalAgentSessionMigration>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<ExternalAgentNamedMigration>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subagents: Vec<ExternalAgentNamedMigration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentConfigMigrationItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<ExternalAgentMigrationDetails>,
    pub item_type: ExternalAgentConfigMigrationItemType,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentConfigDetectParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwds: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_home: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_session_age_days: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_sessions: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migration_source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ExternalAgentConfigDetectResponse {
    pub items: Vec<ExternalAgentConfigMigrationItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentConfigImportParams {
    pub migration_items: Vec<ExternalAgentConfigMigrationItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migration_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentConfigImportResponse {
    pub import_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentConfigImportItemTypeSuccess {
    pub item_type: ExternalAgentConfigMigrationItemType,
    pub cwd: Option<String>,
    pub source: Option<String>,
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentConfigImportItemTypeFailure {
    pub item_type: ExternalAgentConfigMigrationItemType,
    pub cwd: Option<String>,
    pub source: Option<String>,
    pub failure_stage: String,
    pub message: String,
    pub error_type: Option<String>,
    pub sub_error_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentConfigImportTypeResult {
    pub item_type: ExternalAgentConfigMigrationItemType,
    pub successes: Vec<ExternalAgentConfigImportItemTypeSuccess>,
    pub failures: Vec<ExternalAgentConfigImportItemTypeFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentConfigImportProgressNotification {
    pub import_id: String,
    pub item_type_results: Vec<ExternalAgentConfigImportTypeResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentConfigImportCompletedNotification {
    pub import_id: String,
    pub item_type_results: Vec<ExternalAgentConfigImportTypeResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentConfigImportHistory {
    pub import_id: String,
    pub completed_at_ms: i64,
    pub successes: Vec<ExternalAgentConfigImportItemTypeSuccess>,
    pub failures: Vec<ExternalAgentConfigImportItemTypeFailure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExternalAgentImportedConnectorSource {
    RemoteMcpServersConfig,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentImportedConnectorCandidate {
    pub name: String,
    pub session_count: u32,
    pub source: ExternalAgentImportedConnectorSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ExternalAgentConfigImportHistoriesReadResponse {
    pub data: Vec<ExternalAgentConfigImportHistory>,
    pub connectors: Vec<ExternalAgentImportedConnectorCandidate>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartParams {
    pub thread_id: String,
    pub input: Vec<UserInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_user_message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_workspace_roots: Option<Vec<PathBuf>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub personality: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collaboration_mode: Option<CollaborationMode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationMode {
    pub mode: CollaborationModeKind,
    pub settings: CollaborationModeSettings,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CollaborationModeKind {
    Default,
    Plan,
}

#[derive(Debug, Clone, Serialize)]
pub struct CollaborationModeSettings {
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub developer_instructions: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TurnStartResponse {
    pub turn: Value,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_hidden: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelListResponse {
    pub data: Vec<ModelSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSummary {
    pub id: String,
    pub model: String,
    pub display_name: String,
    pub description: String,
    pub hidden: bool,
    pub is_default: bool,
    pub default_reasoning_effort: String,
    pub supported_reasoning_efforts: Vec<ReasoningEffortOption>,
    #[serde(default)]
    pub service_tiers: Vec<ModelServiceTier>,
    #[serde(default)]
    pub default_service_tier: Option<String>,
    #[serde(default)]
    pub upgrade_info: Option<ModelUpgradeInfo>,
    #[serde(default)]
    pub availability_nux: Option<ModelAvailabilityNux>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelAvailabilityNux {
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUpgradeInfo {
    pub model: String,
    pub upgrade_copy: Option<String>,
    pub model_link: Option<String>,
    pub migration_markdown: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReasoningEffortOption {
    pub reasoning_effort: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelServiceTier {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionProfileListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionProfileListResponse {
    pub data: Vec<PermissionProfileSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PermissionProfileSummary {
    pub id: String,
    pub allowed: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalsReviewer {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "auto_review", alias = "guardian_subagent")]
    AutoReview,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRequirementsReadResponse {
    pub requirements: Option<ConfigRequirements>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRequirements {
    pub allow_remote_control: Option<bool>,
    pub allowed_approval_policies: Option<Vec<Value>>,
    pub allowed_approvals_reviewers: Option<Vec<ApprovalsReviewer>>,
    pub allowed_sandbox_modes: Option<Vec<String>>,
    pub default_permissions: Option<String>,
    pub models: Option<ModelsRequirements>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsRequirements {
    pub new_thread: Option<NewThreadModelDefaults>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewThreadModelDefaults {
    pub model: Option<String>,
    pub model_reasoning_effort: Option<String>,
    pub service_tier: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteControlConnectionStatus {
    Disabled,
    Connecting,
    Connected,
    Errored,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteControlStatusReadResponse {
    pub status: RemoteControlConnectionStatus,
    pub installation_id: String,
    pub environment_id: Option<String>,
    pub server_name: String,
}

pub type RemoteControlEnableResponse = RemoteControlStatusReadResponse;
pub type RemoteControlDisableResponse = RemoteControlStatusReadResponse;
pub type RemoteControlStatusChangedNotification = RemoteControlStatusReadResponse;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteControlEnableParams {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ephemeral: bool,
}

pub type NullableRemoteControlEnableParams = Option<RemoteControlEnableParams>;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteControlDisableParams {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ephemeral: bool,
}

pub type NullableRemoteControlDisableParams = Option<RemoteControlDisableParams>;

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteControlPairingStartParams {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub manual_code: bool,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteControlPairingStartResponse {
    pub pairing_code: String,
    pub manual_pairing_code: Option<String>,
    pub environment_id: String,
    pub expires_at: i64,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteControlPairingStatusParams {
    pub pairing_code: Option<String>,
    pub manual_pairing_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteControlPairingStatusResponse {
    pub claimed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteControlClientsListOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteControlClientsListParams {
    pub environment_id: String,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub order: Option<RemoteControlClientsListOrder>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteControlClientsListResponse {
    pub data: Vec<RemoteControlClient>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteControlClient {
    pub client_id: String,
    pub display_name: Option<String>,
    pub device_type: Option<String>,
    pub device_model: Option<String>,
    pub platform: Option<String>,
    pub os_version: Option<String>,
    pub app_version: Option<String>,
    pub last_seen_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteControlClientsRevokeParams {
    pub environment_id: String,
    pub client_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteControlClientsRevokeResponse {}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigReadParams {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub include_layers: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigReadResponse {
    pub config: ConfigDefaults,
    #[serde(default)]
    pub origins: BTreeMap<String, ConfigLayerMetadata>,
    pub layers: Option<Vec<ConfigLayer>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigLayerMetadata {
    pub name: Value,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigLayer {
    pub config: Value,
    #[serde(default)]
    pub name: Value,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub disabled_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConfigDefaults {
    pub model: Option<String>,
    pub model_reasoning_effort: Option<String>,
    pub service_tier: Option<String>,
    pub profile: Option<String>,
    pub personality: Option<String>,
    pub model_personality: Option<String>,
    pub approval_policy: Option<Value>,
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    pub sandbox_mode: Option<String>,
    pub sandbox_workspace_write: Option<SandboxWorkspaceWriteConfig>,
    #[serde(default)]
    pub features: ConfigFeatureDefaults,
    pub memories: Option<MemoryConfigDefaults>,
    #[serde(default, alias = "mcpServers")]
    pub mcp_servers: BTreeMap<String, McpServerConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ConfigFeatureDefaults {
    pub memories: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MemoryConfigDefaults {
    pub generate_memories: Option<bool>,
    pub use_memories: Option<bool>,
    pub disable_on_external_context: Option<bool>,
    pub no_memories_if_mcp_or_web_search: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SandboxWorkspaceWriteConfig {
    pub network_access: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub env_vars: Vec<Value>,
    pub cwd: Option<String>,
    pub url: Option<String>,
    pub bearer_token_env_var: Option<String>,
    #[serde(default)]
    pub http_headers: BTreeMap<String, String>,
    #[serde(default)]
    pub env_http_headers: BTreeMap<String, String>,
    pub startup_timeout_sec: Option<f64>,
    pub startup_timeout_ms: Option<u64>,
    pub tool_timeout_sec: Option<f64>,
    pub enabled_tools: Option<Vec<String>>,
    pub disabled_tools: Option<Vec<String>>,
    #[serde(flatten)]
    pub additional: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigMergeStrategy {
    Replace,
    Upsert,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigEdit {
    pub key_path: String,
    pub value: Value,
    pub merge_strategy: ConfigMergeStrategy,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigBatchWriteParams {
    pub edits: Vec<ConfigEdit>,
    pub file_path: Option<String>,
    pub expected_version: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reload_user_config: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigWriteStatus {
    Ok,
    OkOverridden,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigWriteResponse {
    pub status: ConfigWriteStatus,
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
    pub marketplace_kinds: Option<Vec<PluginListMarketplaceKind>>,
    pub force_refetch: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PluginListMarketplaceKind {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "vertical")]
    Vertical,
    #[serde(rename = "workspace-directory")]
    WorkspaceDirectory,
    #[serde(rename = "shared-with-me")]
    SharedWithMe,
    #[serde(rename = "created-by-me-remote")]
    CreatedByMeRemote,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceAddParams {
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sparse_paths: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceAddResponse {
    pub marketplace_name: String,
    pub installed_root: PathBuf,
    pub already_added: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceRemoveParams {
    pub marketplace_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceRemoveResponse {
    pub marketplace_name: String,
    pub installed_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceUpgradeParams {
    pub marketplace_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceUpgradeResponse {
    #[serde(default)]
    pub selected_marketplaces: Vec<String>,
    #[serde(default)]
    pub upgraded_roots: Vec<PathBuf>,
    #[serde(default)]
    pub errors: Vec<MarketplaceUpgradeErrorInfo>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceUpgradeErrorInfo {
    pub marketplace_name: String,
    pub message: String,
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
    pub availability: Option<String>,
    #[serde(default)]
    pub install_policy: Option<String>,
    #[serde(default)]
    pub must_show_installation_interstitial: Option<bool>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub local_version: Option<String>,
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
    pub website_url: Option<String>,
    pub privacy_policy_url: Option<String>,
    pub terms_of_service_url: Option<String>,
    #[serde(default)]
    pub default_prompt: Option<Vec<String>>,
    pub logo_url: Option<String>,
    pub logo_url_dark: Option<String>,
    #[serde(default)]
    pub screenshot_urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginReadParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marketplace_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_marketplace_name: Option<String>,
    pub plugin_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginReadResponse {
    pub plugin: PluginDetail,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDetail {
    pub marketplace_name: String,
    pub marketplace_path: Option<PathBuf>,
    pub summary: PluginSummary,
    pub share_url: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub skills: Vec<PluginSkillSummary>,
    #[serde(default)]
    pub hooks: Vec<PluginHookSummary>,
    #[serde(default)]
    pub apps: Vec<AppSummary>,
    #[serde(default)]
    pub app_templates: Vec<AppTemplateSummary>,
    #[serde(default)]
    pub mcp_servers: Vec<String>,
    #[serde(default)]
    pub scheduled_tasks: Option<Vec<PluginScheduledTaskSummary>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSkillSummary {
    pub name: String,
    pub description: String,
    pub short_description: Option<String>,
    pub enabled: bool,
    pub path: Option<PathBuf>,
    #[serde(rename = "interface")]
    pub presentation: Option<SkillPresentation>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginHookSummary {
    pub key: String,
    pub event_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppTemplateSummary {
    pub template_id: String,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub canonical_connector_id: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginScheduledTaskSummary {
    pub key: String,
    pub name: String,
    pub prompt: String,
    pub schedule: PluginScheduledTaskSchedule,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PluginScheduledTaskSchedule {
    Hourly {
        interval_hours: u32,
        days: Option<Vec<PluginScheduledTaskWeekday>>,
    },
    Daily {
        time: String,
    },
    Weekdays {
        time: String,
    },
    Weekly {
        days: Vec<PluginScheduledTaskWeekday>,
        time: String,
    },
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PluginScheduledTaskWeekday {
    Mo,
    Tu,
    We,
    Th,
    Fr,
    Sa,
    Su,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsListParams {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cwds: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub force_reload: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsListResponse {
    pub data: Vec<SkillsListEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsListEntry {
    pub cwd: PathBuf,
    #[serde(default)]
    pub skills: Vec<SkillMetadata>,
    #[serde(default)]
    pub errors: Vec<SkillErrorInfo>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillErrorInfo {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillScope {
    User,
    Repo,
    System,
    Admin,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub short_description: Option<String>,
    #[serde(default, rename = "interface")]
    pub presentation: Option<SkillPresentation>,
    pub path: PathBuf,
    pub scope: SkillScope,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillPresentation {
    pub display_name: Option<String>,
    pub short_description: Option<String>,
    pub icon_small: Option<PathBuf>,
    pub icon_large: Option<PathBuf>,
    pub brand_color: Option<String>,
    pub default_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsConfigWriteParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsConfigWriteResponse {
    pub effective_enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HooksListParams {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cwds: Vec<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HooksListResponse {
    pub data: Vec<HooksListEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HooksListEntry {
    pub cwd: PathBuf,
    #[serde(default)]
    pub hooks: Vec<HookMetadata>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub errors: Vec<HookErrorInfo>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookErrorInfo {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HookEventName {
    PreToolUse,
    PermissionRequest,
    PostToolUse,
    PreCompact,
    PostCompact,
    SessionStart,
    SessionEnd,
    UserPromptSubmit,
    SubagentStart,
    SubagentStop,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HookHandlerType {
    Command,
    Prompt,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HookSource {
    System,
    User,
    Project,
    Mdm,
    SessionFlags,
    Plugin,
    CloudRequirements,
    CloudManagedConfig,
    LegacyManagedConfigFile,
    LegacyManagedConfigMdm,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HookTrustStatus {
    Managed,
    Untrusted,
    Trusted,
    Modified,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookMetadata {
    pub key: String,
    pub event_name: HookEventName,
    pub handler_type: HookHandlerType,
    pub is_managed: bool,
    pub matcher: Option<String>,
    pub command: Option<String>,
    pub timeout_sec: u64,
    pub status_message: Option<String>,
    pub additional_context_limit: Option<u32>,
    pub source_path: PathBuf,
    pub source: HookSource,
    pub plugin_id: Option<String>,
    pub display_order: i64,
    pub enabled: bool,
    pub current_hash: String,
    pub trust_status: HookTrustStatus,
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

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppsInstalledParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub force_refresh: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppsInstalledResponse {
    pub apps: Vec<InstalledApp>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub id: String,
    pub runtime_name: Option<String>,
    pub enabled: bool,
    pub callable: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppsListResponse {
    pub data: Vec<AppInfo>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppListUpdatedNotification {
    pub data: Vec<AppInfo>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub logo_url: Option<String>,
    pub logo_url_dark: Option<String>,
    pub icon_assets: Option<BTreeMap<String, String>>,
    pub icon_dark_assets: Option<BTreeMap<String, String>>,
    pub install_url: Option<String>,
    #[serde(default)]
    pub is_accessible: bool,
    #[serde(default = "default_app_enabled")]
    pub is_enabled: bool,
    #[serde(default)]
    pub plugin_display_names: Vec<String>,
}

const fn default_app_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppsReadParams {
    pub app_ids: Vec<String>,
    pub include_tools: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppsReadResponse {
    pub apps: Vec<ConnectorMetadata>,
    #[serde(default)]
    pub missing_app_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorMetadata {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub distribution_channel: Option<String>,
    pub icon_url: Option<String>,
    pub icon_url_dark: Option<String>,
    pub install_url: Option<String>,
    #[serde(default)]
    pub plugin_display_names: Vec<String>,
    pub tool_summaries: Option<Vec<AppToolSummary>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppToolSummary {
    pub name: String,
    pub title: Option<String>,
    pub description: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListMcpServerStatusParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    pub limit: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<McpServerStatusDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum McpServerStatusDetail {
    Full,
    ToolsAndAuthOnly,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListMcpServerStatusResponse {
    pub data: Vec<McpServerStatus>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerStatus {
    pub name: String,
    pub auth_status: McpAuthStatus,
    pub server_info: Option<McpServerInfo>,
    #[serde(default)]
    pub tools: BTreeMap<String, McpTool>,
    #[serde(default)]
    pub resources: Vec<McpResource>,
    #[serde(default)]
    pub resource_templates: Vec<McpResourceTemplate>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerInfo {
    pub name: String,
    pub version: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub website_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTool {
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResource {
    pub name: String,
    pub uri: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub mime_type: Option<String>,
    pub size: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResourceTemplate {
    pub name: String,
    pub uri_template: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResourceReadParams {
    pub server: String,
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpResourceReadResponse {
    #[serde(default)]
    pub contents: Vec<McpResourceContent>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResourceContent {
    pub uri: String,
    pub mime_type: Option<String>,
    pub text: Option<String>,
    pub blob: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerElicitationRequestParams {
    pub server_name: String,
    pub thread_id: String,
    pub turn_id: Option<String>,
    #[serde(flatten)]
    pub request: McpServerElicitationRequest,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "mode")]
pub enum McpServerElicitationRequest {
    #[serde(rename = "form")]
    Form {
        message: String,
        #[serde(rename = "requestedSchema")]
        requested_schema: McpElicitationSchema,
        #[serde(rename = "_meta", default)]
        metadata: Option<Value>,
    },
    #[serde(rename = "openai/form")]
    OpenAiForm {
        message: String,
        #[serde(rename = "requestedSchema")]
        requested_schema: McpOpenAiElicitationSchema,
        #[serde(rename = "_meta", default)]
        metadata: Option<Value>,
    },
    #[serde(rename = "url")]
    Url {
        #[serde(rename = "elicitationId")]
        elicitation_id: String,
        message: String,
        url: String,
        #[serde(rename = "_meta", default)]
        metadata: Option<Value>,
    },
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpElicitationSchema {
    #[serde(rename = "$schema", default)]
    pub schema_uri: Option<String>,
    pub r#type: McpElicitationObjectType,
    pub properties: BTreeMap<String, McpElicitationPrimitiveSchema>,
    #[serde(default)]
    pub required: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpOpenAiElicitationSchema {
    #[serde(rename = "$schema", default)]
    pub schema_uri: Option<String>,
    pub r#type: McpElicitationObjectType,
    pub properties: BTreeMap<String, McpOpenAiElicitationFieldSchema>,
    #[serde(default)]
    pub required: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum McpOpenAiElicitationFieldSchema {
    Primitive(McpElicitationPrimitiveSchema),
    ImagePicker(McpOpenAiImagePickerSchema),
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpOpenAiImagePickerSchema {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub r#type: McpOpenAiImagePickerType,
    pub items: Vec<McpOpenAiImagePickerItem>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub enum McpOpenAiImagePickerType {
    #[serde(rename = "openai/imagePicker")]
    ImagePicker,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct McpOpenAiImagePickerItem {
    pub id: String,
    pub title: String,
    pub image: String,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub enum McpElicitationObjectType {
    #[serde(rename = "object")]
    Object,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpElicitationPrimitiveSchema {
    String {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        description: Option<String>,
        #[serde(rename = "minLength", default)]
        min_length: Option<u32>,
        #[serde(rename = "maxLength", default)]
        max_length: Option<u32>,
        #[serde(default)]
        format: Option<McpElicitationStringFormat>,
        #[serde(default)]
        default: Option<String>,
        #[serde(rename = "enum", default)]
        enum_values: Option<Vec<String>>,
        #[serde(rename = "enumNames", default)]
        enum_names: Option<Vec<String>>,
        #[serde(rename = "oneOf", default)]
        one_of: Option<Vec<McpElicitationConstOption>>,
    },
    Array {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        description: Option<String>,
        #[serde(rename = "minItems", default)]
        min_items: Option<u64>,
        #[serde(rename = "maxItems", default)]
        max_items: Option<u64>,
        items: McpElicitationArrayItems,
        #[serde(default)]
        default: Option<Vec<String>>,
    },
    Boolean {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        default: Option<bool>,
    },
    Number {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        minimum: Option<f64>,
        #[serde(default)]
        maximum: Option<f64>,
        #[serde(default)]
        default: Option<f64>,
    },
    Integer {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        minimum: Option<f64>,
        #[serde(default)]
        maximum: Option<f64>,
        #[serde(default)]
        default: Option<f64>,
    },
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum McpElicitationStringFormat {
    Email,
    Uri,
    Date,
    DateTime,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum McpElicitationArrayItems {
    Untitled {
        r#type: McpElicitationStringType,
        #[serde(rename = "enum")]
        values: Vec<String>,
    },
    Titled {
        #[serde(rename = "anyOf")]
        any_of: Vec<McpElicitationConstOption>,
    },
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub enum McpElicitationStringType {
    #[serde(rename = "string")]
    String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct McpElicitationConstOption {
    #[serde(rename = "const")]
    pub value: String,
    pub title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum McpServerElicitationAction {
    Accept,
    Decline,
    Cancel,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpServerElicitationRequestResponse {
    pub action: McpServerElicitationAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Value>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum McpAuthStatus {
    #[serde(rename = "unsupported")]
    Unsupported,
    #[serde(rename = "notLoggedIn")]
    NotLoggedIn,
    #[serde(rename = "bearerToken")]
    BearerToken,
    #[serde(rename = "oAuth")]
    OAuth,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerOauthLoginParams {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerOauthLoginResponse {
    pub authorization_url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerOauthLoginCompletedNotification {
    pub name: String,
    pub thread_id: Option<String>,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum McpServerStartupState {
    Starting,
    Ready,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum McpServerStartupFailureReason {
    ReauthenticationRequired,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerStatusUpdatedNotification {
    pub thread_id: Option<String>,
    pub name: String,
    pub status: McpServerStartupState,
    pub error: Option<String>,
    pub failure_reason: Option<McpServerStartupFailureReason>,
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
    use std::collections::BTreeMap;
    use std::io::{BufReader, Cursor};
    use std::num::NonZeroUsize;
    use std::path::PathBuf;

    use serde_json::{Value, json};

    use super::{
        Account, AccountLoginCompletedNotification, AccountTokenUsageDailyBucket,
        ApprovalsReviewer, AppsInstalledParams, AppsInstalledResponse, AppsReadParams,
        AppsReadResponse, BoundedLineDecoder, CancelLoginAccountParams, CancelLoginAccountResponse,
        CancelLoginAccountStatus, ClientInfo, ClientNotification, ClientRequest, CollaborationMode,
        CollaborationModeKind, CollaborationModeSettings, CommandExecutionApprovalDecision,
        CommandExecutionApprovalDecisionValue, CommandExecutionRequestApprovalParams,
        CommandExecutionRequestApprovalResponse, ConfigBatchWriteParams, ConfigEdit,
        ConfigMergeStrategy, ConfigReadParams, ConfigReadResponse, ConfigRequirementsReadResponse,
        ConfigWriteResponse, ConfigWriteStatus, DynamicToolCallOutputContentItem,
        DynamicToolCallParams, DynamicToolCallResponse, DynamicToolFunction,
        DynamicToolNamespaceTool, DynamicToolSpec, ExecpolicyAmendment,
        ExternalAgentConfigDetectParams, ExternalAgentConfigDetectResponse,
        ExternalAgentConfigImportCompletedNotification,
        ExternalAgentConfigImportHistoriesReadResponse, ExternalAgentConfigImportParams,
        ExternalAgentConfigImportProgressNotification, ExternalAgentConfigImportResponse,
        ExternalAgentConfigMigrationItemType, FeedbackUploadParams, FeedbackUploadResponse,
        FileChangeApprovalDecision, FileChangeRequestApprovalResponse, FuzzyFileSearchMatchType,
        FuzzyFileSearchParams, FuzzyFileSearchResponse,
        FuzzyFileSearchSessionCompletedNotification, FuzzyFileSearchSessionStartParams,
        FuzzyFileSearchSessionUpdateParams, FuzzyFileSearchSessionUpdatedNotification,
        GetAccountParams, GetAccountRateLimitsResponse, GetAccountResponse,
        GetAccountTokenUsageResponse, GetAuthStatusParams, GetAuthStatusResponse,
        GitDiffToRemoteParams, GitDiffToRemoteResponse, HistorySortDirection, HookEventName,
        HookHandlerType, HookSource, HookTrustStatus, HooksListParams, HooksListResponse,
        IncomingMessage, InitializeParams, InstalledApp, ListMcpServerStatusParams,
        ListMcpServerStatusResponse, LoginAccountParams, LoginAccountResponse,
        MarketplaceAddParams, MarketplaceRemoveParams, MarketplaceUpgradeParams, McpAuthStatus,
        McpElicitationPrimitiveSchema, McpResourceReadParams, McpResourceReadResponse,
        McpServerElicitationAction, McpServerElicitationRequest, McpServerElicitationRequestParams,
        McpServerElicitationRequestResponse, McpServerOauthLoginParams,
        McpServerStartupFailureReason, McpServerStartupState, McpServerStatusDetail,
        McpServerStatusUpdatedNotification, MemoryResetResponse, ModelListParams,
        ModelListResponse, ModelSafetyBufferingUpdatedNotification, ModelVerification,
        ModelVerificationNotification, NetworkPolicyAmendment, NetworkPolicyAmendmentDecision,
        NetworkPolicyRuleAction, NullableRemoteControlDisableParams,
        NullableRemoteControlEnableParams, PermissionGrantScope, PermissionProfile,
        PermissionProfileListParams, PermissionProfileListResponse,
        PermissionsRequestApprovalParams, PermissionsRequestApprovalResponse, PlanType,
        PluginListMarketplaceKind, PluginReadParams, PluginReadResponse,
        PluginScheduledTaskSchedule, PluginScheduledTaskSummary, PluginSummary, ProtocolError,
        RemoteControlClientsListOrder, RemoteControlClientsListParams,
        RemoteControlClientsListResponse, RemoteControlClientsRevokeParams,
        RemoteControlClientsRevokeResponse, RemoteControlConnectionStatus,
        RemoteControlDisableParams, RemoteControlDisableResponse, RemoteControlEnableParams,
        RemoteControlEnableResponse, RemoteControlPairingStartParams,
        RemoteControlPairingStartResponse, RemoteControlPairingStatusParams,
        RemoteControlPairingStatusResponse, RemoteControlStatusChangedNotification,
        RemoteControlStatusReadResponse, ReviewDelivery, ReviewStartParams, ReviewStartResponse,
        ReviewTarget, SecretString, SkillScope, SkillsConfigWriteParams, SkillsConfigWriteResponse,
        SkillsListParams, SkillsListResponse, ThreadArchiveParams,
        ThreadBackgroundTerminalsCleanResponse, ThreadBackgroundTerminalsListParams,
        ThreadBackgroundTerminalsListResponse, ThreadBackgroundTerminalsTerminateParams,
        ThreadBackgroundTerminalsTerminateResponse, ThreadCompactStartParams,
        ThreadCompactStartResponse, ThreadDeleteParams, ThreadForkParams, ThreadGoal,
        ThreadGoalGetResponse, ThreadGoalSetParams, ThreadGoalStatus, ThreadListParams,
        ThreadLoadedListParams, ThreadLoadedListResponse, ThreadMemoryMode,
        ThreadMemoryModeSetParams, ThreadMemoryModeSetResponse, ThreadResumeInitialTurnsPageParams,
        ThreadResumeParams, ThreadResumeResponse, ThreadRollbackParams, ThreadRollbackResponse,
        ThreadSearchParams, ThreadSetNameParams, ThreadSettingsUpdateParams,
        ThreadShellCommandParams, ThreadShellCommandResponse, ThreadStartParams,
        ThreadTokenUsageUpdatedNotification, ThreadUnarchiveParams, ThreadUnsubscribeParams,
        ThreadUnsubscribeResponse, ThreadUnsubscribeStatus, ToolRequestUserInputAnswer,
        ToolRequestUserInputParams, ToolRequestUserInputResponse, TurnInterruptParams,
        TurnStartParams, TurnSteerParams, UserInput, decode_incoming, encode_json_line,
        read_bounded_frame,
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
            params: Some(InitializeParams {
                client_info: ClientInfo {
                    name: "codex-rs".to_owned(),
                    title: Some("codexRS".to_owned()),
                    version: "0.1.0".to_owned(),
                },
                capabilities: None,
            }),
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
    fn apps_installed_wire_shape_matches_generated_schema() {
        assert_eq!(
            serde_json::to_value(AppsInstalledParams {
                thread_id: Some("thread-1".to_owned()),
                force_refresh: false,
            })
            .ok(),
            Some(json!({ "threadId": "thread-1" }))
        );
        assert_eq!(
            encoded(&ClientRequest {
                method: "app/installed",
                id: 7,
                params: Some(AppsInstalledParams {
                    thread_id: Some("thread-1".to_owned()),
                    force_refresh: true,
                }),
            }),
            b"{\"method\":\"app/installed\",\"id\":7,\"params\":{\"threadId\":\"thread-1\",\"forceRefresh\":true}}\n"
        );
        assert!(matches!(
            serde_json::from_value::<AppsInstalledResponse>(json!({
                "apps": [{
                    "id": "connector_calendar",
                    "runtimeName": "calendar",
                    "enabled": true,
                    "callable": false
                }]
            })),
            Ok(AppsInstalledResponse { apps })
                if matches!(apps.as_slice(), [InstalledApp { id, runtime_name: Some(runtime_name), enabled: true, callable: false }]
                    if id == "connector_calendar" && runtime_name == "calendar")
        ));
    }

    #[test]
    fn account_and_rate_limit_types_match_the_stable_schema() {
        assert_eq!(
            encoded(&ClientRequest {
                method: "account/read",
                id: 1,
                params: Some(GetAccountParams {
                    refresh_token: Some(false),
                }),
            }),
            b"{\"method\":\"account/read\",\"id\":1,\"params\":{\"refreshToken\":false}}\n"
        );

        let account = serde_json::from_value::<GetAccountResponse>(json!({
            "account": {
                "type": "chatgpt",
                "email": "developer@example.com",
                "planType": "plus"
            },
            "requiresOpenaiAuth": true
        }));
        assert!(matches!(
            account,
            Ok(GetAccountResponse {
                account: Some(Account::ChatGpt {
                    plan_type: PlanType::Plus,
                    ..
                }),
                requires_openai_auth: true,
            })
        ));

        assert_eq!(
            encoded(&ClientRequest {
                method: "getAuthStatus",
                id: 2,
                params: Some(GetAuthStatusParams {
                    include_token: true,
                    refresh_token: false,
                }),
            }),
            b"{\"method\":\"getAuthStatus\",\"id\":2,\"params\":{\"includeToken\":true,\"refreshToken\":false}}\n"
        );
        let auth = serde_json::from_value::<GetAuthStatusResponse>(json!({
            "authMethod": "chatgpt",
            "authToken": "fixture-token",
            "accountId": "fixture-account",
            "requiresOpenaiAuth": true
        }))
        .unwrap_or_else(|error| panic!("valid auth fixture failed to decode: {error}"));
        assert_eq!(
            auth.auth_token.as_ref().map(|token| token.expose()),
            Some("fixture-token")
        );
        assert!(!format!("{auth:?}").contains("fixture-token"));

        let limits = serde_json::from_value::<GetAccountRateLimitsResponse>(json!({
            "rateLimits": {
                "limitId": "codex",
                "limitName": "Codex",
                "primary": {
                    "usedPercent": 37.5,
                    "windowDurationMins": 300,
                    "resetsAt": 1_900_000_000
                },
                "secondary": null,
                "credits": {
                    "hasCredits": true,
                    "unlimited": false,
                    "balance": "125.5"
                },
                "individualLimit": null,
                "spendControlReached": null,
                "planType": "plus",
                "rateLimitReachedType": null
            },
            "rateLimitsByLimitId": null,
            "rateLimitResetCredits": {
                "availableCount": 2,
                "credits": null
            }
        }));
        assert!(matches!(
            limits,
            Ok(GetAccountRateLimitsResponse {
                rate_limits,
                rate_limit_reset_credits: Some(reset_credits),
            })
                if rate_limits.primary.as_ref().is_some_and(|window| {
                    window.used_percent == 37.5 && window.window_duration_mins == Some(300)
                })
                    && rate_limits
                        .credits
                        .as_ref()
                        .and_then(|credits| credits.balance.as_deref())
                        == Some("125.5")
                    && reset_credits.available_count == 2
        ));

        let token_usage = serde_json::from_value::<GetAccountTokenUsageResponse>(json!({
            "summary": {
                "lifetimeTokens": 1250000,
                "peakDailyTokens": 250000,
                "longestRunningTurnSec": 5400,
                "currentStreakDays": 7,
                "longestStreakDays": 21
            },
            "dailyUsageBuckets": [{
                "startDate": "2026-07-01",
                "tokens": 250000
            }]
        }));
        assert!(matches!(
            token_usage,
            Ok(GetAccountTokenUsageResponse {
                summary,
                daily_usage_buckets,
            }) if summary.lifetime_tokens == Some(1_250_000)
                && summary.peak_daily_tokens == Some(250_000)
                && summary.longest_running_turn_sec == Some(5_400)
                && summary.current_streak_days == Some(7)
                && summary.longest_streak_days == Some(21)
                && matches!(
                    daily_usage_buckets.as_slice(),
                    [AccountTokenUsageDailyBucket { start_date, tokens }]
                        if start_date == "2026-07-01" && *tokens == 250_000
                )
        ));
    }

    #[test]
    fn account_login_types_match_the_stable_schema() {
        let api_key = SecretString::new("sk-test-key".to_owned());
        assert!(api_key.is_some());
        let Some(api_key) = api_key else {
            return;
        };
        assert_eq!(format!("{api_key:?}"), "[REDACTED]");
        assert!(!format!("{api_key:?}").contains("sk-test-key"));
        assert_eq!(
            encoded(&ClientRequest {
                method: "account/login/start",
                id: 1,
                params: Some(LoginAccountParams::ApiKey { api_key }),
            }),
            b"{\"method\":\"account/login/start\",\"id\":1,\"params\":{\"type\":\"apiKey\",\"apiKey\":\"sk-test-key\"}}\n"
        );
        assert_eq!(
            encoded(&ClientRequest {
                method: "account/login/start",
                id: 2,
                params: Some(LoginAccountParams::ChatGpt {
                    codex_streamlined_login: None,
                    use_hosted_login_success_page: None,
                    app_brand: None,
                }),
            }),
            b"{\"method\":\"account/login/start\",\"id\":2,\"params\":{\"type\":\"chatgpt\"}}\n"
        );
        assert_eq!(
            encoded(&ClientRequest {
                method: "account/login/start",
                id: 3,
                params: Some(LoginAccountParams::ChatGptDeviceCode),
            }),
            b"{\"method\":\"account/login/start\",\"id\":3,\"params\":{\"type\":\"chatgptDeviceCode\"}}\n"
        );
        let bedrock_api_key = SecretString::new("bedrock-test-key".to_owned());
        assert!(bedrock_api_key.is_some());
        let Some(bedrock_api_key) = bedrock_api_key else {
            return;
        };
        assert_eq!(format!("{bedrock_api_key:?}"), "[REDACTED]");
        assert!(!format!("{bedrock_api_key:?}").contains("bedrock-test-key"));
        assert_eq!(
            encoded(&ClientRequest {
                method: "account/login/start",
                id: 4,
                params: Some(LoginAccountParams::AmazonBedrock {
                    api_key: bedrock_api_key,
                    region: "us-west-2".to_owned(),
                }),
            }),
            b"{\"method\":\"account/login/start\",\"id\":4,\"params\":{\"type\":\"amazonBedrock\",\"apiKey\":\"bedrock-test-key\",\"region\":\"us-west-2\"}}\n"
        );
        assert!(matches!(
            serde_json::from_value::<LoginAccountResponse>(json!({ "type": "apiKey" })),
            Ok(LoginAccountResponse::ApiKey)
        ));
        assert!(matches!(
            serde_json::from_value::<LoginAccountResponse>(json!({
                "type": "chatgpt",
                "loginId": "login-1",
                "authUrl": "https://auth.openai.com/"
            })),
            Ok(LoginAccountResponse::ChatGpt { login_id, auth_url })
                if login_id == "login-1" && auth_url == "https://auth.openai.com/"
        ));
        assert!(matches!(
            serde_json::from_value::<LoginAccountResponse>(json!({
                "type": "chatgptDeviceCode",
                "loginId": "login-2",
                "verificationUrl": "https://auth.openai.com/device",
                "userCode": "ABCD-EFGH"
            })),
            Ok(LoginAccountResponse::ChatGptDeviceCode {
                login_id,
                verification_url,
                user_code,
            }) if login_id == "login-2"
                && verification_url == "https://auth.openai.com/device"
                && user_code == "ABCD-EFGH"
        ));
        assert_eq!(
            encoded(&ClientRequest {
                method: "account/login/cancel",
                id: 4,
                params: Some(CancelLoginAccountParams {
                    login_id: "login-1".to_owned(),
                }),
            }),
            b"{\"method\":\"account/login/cancel\",\"id\":4,\"params\":{\"loginId\":\"login-1\"}}\n"
        );
        assert!(matches!(
            serde_json::from_value::<CancelLoginAccountResponse>(json!({
                "status": "notFound"
            })),
            Ok(CancelLoginAccountResponse {
                status: CancelLoginAccountStatus::NotFound
            })
        ));
        assert!(matches!(
            serde_json::from_value::<AccountLoginCompletedNotification>(json!({
                "loginId": "login-1",
                "success": false,
                "error": "provider detail"
            })),
            Ok(AccountLoginCompletedNotification {
                login_id: Some(login_id),
                success: false,
                error: Some(_),
            }) if login_id == "login-1"
        ));
    }

    #[test]
    fn secret_string_rejects_empty_and_oversize_owned_values() {
        assert!(SecretString::new(String::new()).is_none());
        assert!(SecretString::new("x".repeat(513)).is_none());
    }

    #[test]
    fn git_diff_to_remote_types_match_the_pinned_app_server_schema() {
        assert_eq!(
            encoded(&ClientRequest {
                method: "gitDiffToRemote",
                id: 4,
                params: Some(GitDiffToRemoteParams {
                    cwd: PathBuf::from(r"C:\work\codexRS"),
                }),
            }),
            b"{\"method\":\"gitDiffToRemote\",\"id\":4,\"params\":{\"cwd\":\"C:\\\\work\\\\codexRS\"}}\n"
        );
        assert!(matches!(
            serde_json::from_value::<GitDiffToRemoteResponse>(json!({
                "sha": "0123456789abcdef",
                "diff": "diff --git a/src/lib.rs b/src/lib.rs\n"
            })),
            Ok(GitDiffToRemoteResponse { sha, diff })
                if sha == "0123456789abcdef" && diff.starts_with("diff --git")
        ));
    }

    #[test]
    fn feedback_upload_types_match_the_pinned_app_server_schema() {
        assert_eq!(
            encoded(&ClientRequest {
                method: "feedback/upload",
                id: 4,
                params: Some(FeedbackUploadParams {
                    classification: "bad-result".to_owned(),
                    extra_log_files: None,
                    include_logs: Some(true),
                    reason: Some("The result missed the requested file.".to_owned()),
                    tags: Some(BTreeMap::from([(
                        "app_version".to_owned(),
                        "0.1.0".to_owned(),
                    )])),
                    thread_id: Some("thread-1".to_owned()),
                }),
            }),
            b"{\"method\":\"feedback/upload\",\"id\":4,\"params\":{\"classification\":\"bad-result\",\"includeLogs\":true,\"reason\":\"The result missed the requested file.\",\"tags\":{\"app_version\":\"0.1.0\"},\"threadId\":\"thread-1\"}}\n"
        );
        assert!(matches!(
            serde_json::from_value::<FeedbackUploadResponse>(json!({
                "threadId": "feedback-1"
            })),
            Ok(FeedbackUploadResponse { thread_id }) if thread_id == "feedback-1"
        ));
    }

    #[test]
    fn thread_list_is_bounded_to_state_database_metadata() {
        let request = ClientRequest {
            method: "thread/list",
            id: 7,
            params: Some(ThreadListParams::state_db_page(20)),
        };

        assert_eq!(
            encoded(&request),
            b"{\"method\":\"thread/list\",\"id\":7,\"params\":{\"limit\":20,\"sortKey\":\"recency_at\",\"sortDirection\":\"desc\",\"useStateDbOnly\":true}}\n"
        );
    }

    #[test]
    fn thread_loaded_list_is_explicitly_bounded() {
        assert_eq!(
            encoded(&ClientRequest {
                method: "thread/loaded/list",
                id: 9,
                params: Some(ThreadLoadedListParams {
                    cursor: None,
                    limit: 20,
                }),
            }),
            b"{\"method\":\"thread/loaded/list\",\"id\":9,\"params\":{\"limit\":20}}\n"
        );
        assert!(matches!(
            serde_json::from_value::<ThreadLoadedListResponse>(json!({
                "data": ["thread-1", "thread-2"],
                "nextCursor": "next"
            })),
            Ok(ThreadLoadedListResponse { data, next_cursor })
                if data == ["thread-1", "thread-2"] && next_cursor.as_deref() == Some("next")
        ));
    }

    #[test]
    fn background_terminal_management_matches_the_experimental_app_server_contract() {
        assert_eq!(
            encoded(&ClientRequest {
                method: "thread/backgroundTerminals/list",
                id: 10,
                params: Some(ThreadBackgroundTerminalsListParams {
                    thread_id: "thread-1".to_owned(),
                    cursor: None,
                    limit: Some(64),
                }),
            }),
            b"{\"method\":\"thread/backgroundTerminals/list\",\"id\":10,\"params\":{\"threadId\":\"thread-1\",\"limit\":64}}\n"
        );

        let response = serde_json::from_value::<ThreadBackgroundTerminalsListResponse>(json!({
            "data": [{
                "itemId": "item-1",
                "processId": "42",
                "command": "python -m http.server",
                "cwd": "C:\\isolated\\repo",
                "osPid": 4242,
                "cpuPercent": 1.25,
                "rssKb": 2048
            }],
            "nextCursor": "next"
        }));
        assert!(matches!(
            response,
            Ok(response)
                if response.data.len() == 1
                    && response.data[0].process_id == "42"
                    && response.data[0].os_pid == Some(4242)
                    && response.data[0].rss_kb == Some(2048)
                    && response.next_cursor.as_deref() == Some("next")
        ));

        assert_eq!(
            serde_json::to_value(ThreadBackgroundTerminalsTerminateParams {
                thread_id: "thread-1".to_owned(),
                process_id: "42".to_owned(),
            })
            .ok(),
            Some(json!({ "threadId": "thread-1", "processId": "42" }))
        );
        assert!(matches!(
            serde_json::from_value::<ThreadBackgroundTerminalsTerminateResponse>(json!({
                "terminated": true
            })),
            Ok(ThreadBackgroundTerminalsTerminateResponse { terminated: true })
        ));
        assert!(
            serde_json::from_value::<ThreadBackgroundTerminalsCleanResponse>(json!({})).is_ok()
        );
    }

    #[test]
    fn thread_rollback_matches_the_stable_edit_contract() {
        assert_eq!(
            encoded(&ClientRequest {
                method: "thread/rollback",
                id: 10,
                params: Some(ThreadRollbackParams {
                    thread_id: "thread-1".to_owned(),
                    num_turns: 1,
                }),
            }),
            b"{\"method\":\"thread/rollback\",\"id\":10,\"params\":{\"threadId\":\"thread-1\",\"numTurns\":1}}\n"
        );
        assert!(
            serde_json::from_value::<ThreadRollbackResponse>(json!({
                "thread": {
                    "id": "thread-1",
                    "turns": []
                }
            }))
            .is_ok()
        );
    }

    #[test]
    fn thread_compact_start_matches_the_stable_slash_command_contract() {
        assert_eq!(
            encoded(&ClientRequest {
                method: "thread/compact/start",
                id: 11,
                params: Some(ThreadCompactStartParams {
                    thread_id: "thread-1".to_owned(),
                }),
            }),
            b"{\"method\":\"thread/compact/start\",\"id\":11,\"params\":{\"threadId\":\"thread-1\"}}\n"
        );
        assert!(serde_json::from_value::<ThreadCompactStartResponse>(json!({})).is_ok());
    }

    #[test]
    fn review_start_targets_and_deliveries_match_the_pinned_contract() {
        let cases = [
            (
                ReviewTarget::UncommittedChanges,
                ReviewDelivery::Inline,
                json!({ "type": "uncommittedChanges" }),
                json!("inline"),
            ),
            (
                ReviewTarget::BaseBranch {
                    branch: "main".to_owned(),
                },
                ReviewDelivery::Detached,
                json!({ "type": "baseBranch", "branch": "main" }),
                json!("detached"),
            ),
            (
                ReviewTarget::Commit {
                    sha: "abc123".to_owned(),
                    title: Some("Fix review flow".to_owned()),
                },
                ReviewDelivery::Inline,
                json!({ "type": "commit", "sha": "abc123", "title": "Fix review flow" }),
                json!("inline"),
            ),
            (
                ReviewTarget::Custom {
                    instructions: "Check error handling".to_owned(),
                },
                ReviewDelivery::Detached,
                json!({ "type": "custom", "instructions": "Check error handling" }),
                json!("detached"),
            ),
        ];

        for (target, delivery, expected_target, expected_delivery) in cases {
            assert_eq!(serde_json::to_value(target).ok(), Some(expected_target));
            assert_eq!(serde_json::to_value(delivery).ok(), Some(expected_delivery));
        }
    }

    #[test]
    fn review_start_delivery_is_nullable_and_optional_when_deserializing() {
        let params = ReviewStartParams {
            thread_id: "thread-1".to_owned(),
            target: ReviewTarget::Commit {
                sha: "abc123".to_owned(),
                title: None,
            },
            delivery: None,
        };
        assert_eq!(
            serde_json::to_value(params).ok(),
            Some(json!({
                "threadId": "thread-1",
                "target": { "type": "commit", "sha": "abc123", "title": null },
                "delivery": null
            }))
        );

        assert!(matches!(
            serde_json::from_value::<ReviewStartParams>(json!({
                "threadId": "thread-1",
                "target": { "type": "uncommittedChanges" }
            })),
            Ok(ReviewStartParams {
                thread_id,
                target: ReviewTarget::UncommittedChanges,
                delivery: None,
            }) if thread_id == "thread-1"
        ));
    }

    #[test]
    fn review_start_response_matches_the_pinned_contract() {
        assert!(matches!(
            serde_json::from_value::<ReviewStartResponse>(json!({
                "turn": { "id": "turn-1", "items": [], "status": "inProgress" },
                "reviewThreadId": "review-thread-1"
            })),
            Ok(ReviewStartResponse { turn, review_thread_id })
                if turn == json!({ "id": "turn-1", "items": [], "status": "inProgress" })
                    && review_thread_id == "review-thread-1"
        ));
    }

    #[test]
    fn thread_resume_uses_a_bounded_initial_turns_page() {
        assert_eq!(
            serde_json::to_value(ThreadResumeParams {
                thread_id: "thread-1".to_owned(),
                exclude_turns: Some(true),
                initial_turns_page: Some(ThreadResumeInitialTurnsPageParams {
                    cursor: None,
                    limit: 1,
                    sort_direction: HistorySortDirection::Desc,
                    items_view: Some("notLoaded".to_owned()),
                }),
            })
            .ok(),
            Some(json!({
                "threadId": "thread-1",
                "excludeTurns": true,
                "initialTurnsPage": {
                    "limit": 1,
                    "sortDirection": "desc",
                    "itemsView": "notLoaded"
                }
            }))
        );
        let response = serde_json::from_value::<ThreadResumeResponse>(json!({
            "thread": { "id": "thread-1", "turns": [] },
            "initialTurnsPage": {
                "data": [{ "id": "turn-2", "items": [], "status": "inProgress" }],
                "nextCursor": null,
                "backwardsCursor": "turn-cursor"
            }
        }));
        assert!(matches!(
            response,
            Ok(ThreadResumeResponse { thread, initial_turns_page: Some(page), .. })
                if thread.turns.is_empty()
                    && page.data.len() == 1
                    && page.data[0].get("id").and_then(Value::as_str) == Some("turn-2")
        ));
    }

    #[test]
    fn memory_controls_match_the_stable_contract() {
        assert_eq!(
            encoded(&ClientRequest {
                method: "thread/memoryMode/set",
                id: 12,
                params: Some(ThreadMemoryModeSetParams {
                    thread_id: "thread-1".to_owned(),
                    mode: ThreadMemoryMode::Disabled,
                }),
            }),
            b"{\"method\":\"thread/memoryMode/set\",\"id\":12,\"params\":{\"threadId\":\"thread-1\",\"mode\":\"disabled\"}}\n"
        );
        assert!(serde_json::from_value::<ThreadMemoryModeSetResponse>(json!({})).is_ok());
        assert!(serde_json::from_value::<MemoryResetResponse>(json!({})).is_ok());
    }

    #[test]
    fn external_agent_import_matches_the_stable_contract() {
        assert_eq!(
            encoded(&ClientRequest {
                method: "externalAgentConfig/detect",
                id: 13,
                params: Some(ExternalAgentConfigDetectParams {
                    cwds: Some(vec!["C:\\work\\codexrs".to_owned()]),
                    include_home: Some(true),
                    max_session_age_days: Some(30),
                    max_sessions: Some(50),
                    migration_source: Some("cursor".to_owned()),
                }),
            }),
            b"{\"method\":\"externalAgentConfig/detect\",\"id\":13,\"params\":{\"cwds\":[\"C:\\\\work\\\\codexrs\"],\"includeHome\":true,\"maxSessionAgeDays\":30,\"maxSessions\":50,\"migrationSource\":\"cursor\"}}\n"
        );

        let Ok(detected) = serde_json::from_value::<ExternalAgentConfigDetectResponse>(json!({
            "items": [{
                "cwd": "C:\\work\\codexrs",
                "description": "Recent chats",
                "details": {
                    "sessions": [{
                        "cwd": "C:\\work\\codexrs",
                        "path": "C:\\fixture\\session.jsonl",
                        "title": "Import parity"
                    }]
                },
                "itemType": "SESSIONS"
            }]
        })) else {
            panic!("detect response should match the generated schema");
        };
        assert_eq!(
            detected.items[0].item_type,
            ExternalAgentConfigMigrationItemType::Sessions
        );

        assert_eq!(
            encoded(&ClientRequest {
                method: "externalAgentConfig/import",
                id: 14,
                params: Some(ExternalAgentConfigImportParams {
                    migration_items: detected.items,
                    migration_source: Some("cursor".to_owned()),
                    provider_id: Some("cursor".to_owned()),
                    source: Some("settings".to_owned()),
                }),
            }),
            b"{\"method\":\"externalAgentConfig/import\",\"id\":14,\"params\":{\"migrationItems\":[{\"cwd\":\"C:\\\\work\\\\codexrs\",\"description\":\"Recent chats\",\"details\":{\"sessions\":[{\"cwd\":\"C:\\\\work\\\\codexrs\",\"path\":\"C:\\\\fixture\\\\session.jsonl\",\"title\":\"Import parity\"}]},\"itemType\":\"SESSIONS\"}],\"migrationSource\":\"cursor\",\"providerId\":\"cursor\",\"source\":\"settings\"}}\n"
        );
        let Ok(response) = serde_json::from_value::<ExternalAgentConfigImportResponse>(json!({
            "importId": "import-1"
        })) else {
            panic!("import response should match the generated schema");
        };
        assert_eq!(response.import_id, "import-1");

        let notification = json!({
            "importId": "import-1",
            "itemTypeResults": [{
                "itemType": "SESSIONS",
                "successes": [{
                    "itemType": "SESSIONS",
                    "source": "C:\\fixture\\session.jsonl",
                    "target": "thread-1"
                }],
                "failures": []
            }]
        });
        assert!(
            serde_json::from_value::<ExternalAgentConfigImportProgressNotification>(
                notification.clone()
            )
            .is_ok()
        );
        assert!(
            serde_json::from_value::<ExternalAgentConfigImportCompletedNotification>(notification)
                .is_ok()
        );
        let Ok(history) =
            serde_json::from_value::<ExternalAgentConfigImportHistoriesReadResponse>(json!({
                "data": [{
                    "importId": "import-1",
                    "completedAtMs": 1_700_000_000_000_i64,
                    "successes": [{"itemType": "SESSIONS", "target": "thread-1"}],
                    "failures": []
                }],
                "connectors": [{
                    "name": "github",
                    "sessionCount": 1,
                    "source": "remoteMcpServersConfig"
                }]
            }))
        else {
            panic!("history response should match the generated schema");
        };
        assert_eq!(history.data.len(), 1);
        assert_eq!(history.connectors.len(), 1);
    }

    #[test]
    fn thread_shell_command_preserves_shell_syntax_in_the_stable_contract() {
        assert_eq!(
            encoded(&ClientRequest {
                method: "thread/shellCommand",
                id: 12,
                params: Some(ThreadShellCommandParams {
                    thread_id: "thread-1".to_owned(),
                    command: "git status --short | rg src".to_owned(),
                }),
            }),
            b"{\"method\":\"thread/shellCommand\",\"id\":12,\"params\":{\"threadId\":\"thread-1\",\"command\":\"git status --short | rg src\"}}\n"
        );
        assert!(serde_json::from_value::<ThreadShellCommandResponse>(json!({})).is_ok());
    }

    #[test]
    fn thread_token_usage_notification_matches_the_stable_contract() {
        let Ok(notification) =
            serde_json::from_value::<ThreadTokenUsageUpdatedNotification>(json!({
                "threadId": "thread-1",
                "turnId": "turn-1",
                "tokenUsage": {
                    "total": {
                        "totalTokens": 310,
                        "inputTokens": 240,
                        "cachedInputTokens": 100,
                        "cacheWriteInputTokens": 20,
                        "outputTokens": 50,
                        "reasoningOutputTokens": 25
                    },
                    "last": {
                        "totalTokens": 125,
                        "inputTokens": 90,
                        "cachedInputTokens": 40,
                        "cacheWriteInputTokens": 10,
                        "outputTokens": 35,
                        "reasoningOutputTokens": 15
                    },
                    "modelContextWindow": 1000
                }
            }))
        else {
            panic!("stable token usage notification should decode");
        };

        assert_eq!(notification.thread_id, "thread-1");
        assert_eq!(notification.turn_id, "turn-1");
        assert_eq!(notification.token_usage.last.total_tokens, 125);
        assert_eq!(notification.token_usage.model_context_window, Some(1000));
    }

    #[test]
    fn model_safety_buffering_notification_matches_the_stable_contract() {
        let Ok(notification) =
            serde_json::from_value::<ModelSafetyBufferingUpdatedNotification>(json!({
                "threadId": "thread-1",
                "turnId": "turn-1",
                "model": "gpt-5.6-sol",
                "useCases": ["complex_reasoning"],
                "reasons": ["safety_buffering"],
                "showBufferingUi": true,
                "fasterModel": "gpt-5.6-terra"
            }))
        else {
            panic!("stable safety buffering notification should decode");
        };

        assert_eq!(notification.thread_id, "thread-1");
        assert_eq!(notification.turn_id, "turn-1");
        assert_eq!(notification.model, "gpt-5.6-sol");
        assert_eq!(notification.use_cases, ["complex_reasoning"]);
        assert_eq!(notification.reasons, ["safety_buffering"]);
        assert!(notification.show_buffering_ui);
        assert_eq!(notification.faster_model.as_deref(), Some("gpt-5.6-terra"));
    }

    #[test]
    fn model_verification_notification_matches_the_stable_contract() {
        let Ok(notification) = serde_json::from_value::<ModelVerificationNotification>(json!({
            "threadId": "thread-1",
            "turnId": "turn-1",
            "verifications": ["trustedAccessForCyber"]
        })) else {
            panic!("stable model verification notification should decode");
        };

        assert_eq!(notification.thread_id, "thread-1");
        assert_eq!(notification.turn_id, "turn-1");
        assert_eq!(
            notification.verifications,
            [ModelVerification::TrustedAccessForCyber]
        );
    }

    #[test]
    fn thread_search_matches_the_stable_interactive_contract() {
        let request = ClientRequest {
            method: "thread/search",
            id: 8,
            params: Some(ThreadSearchParams::interactive_page(
                "native ui".to_owned(),
                50,
            )),
        };

        assert_eq!(
            encoded(&request),
            b"{\"method\":\"thread/search\",\"id\":8,\"params\":{\"limit\":50,\"sortKey\":\"recency_at\",\"sortDirection\":\"desc\",\"sourceKinds\":[],\"archived\":false,\"searchTerm\":\"native ui\"}}\n"
        );
    }

    #[test]
    fn fuzzy_file_search_matches_the_stable_session_and_legacy_contracts() {
        let start = ClientRequest {
            method: "fuzzyFileSearch/sessionStart",
            id: 9,
            params: Some(FuzzyFileSearchSessionStartParams {
                session_id: "files-1".to_owned(),
                roots: vec![PathBuf::from("C:\\repo")],
            }),
        };
        assert_eq!(
            encoded(&start),
            b"{\"method\":\"fuzzyFileSearch/sessionStart\",\"id\":9,\"params\":{\"sessionId\":\"files-1\",\"roots\":[\"C:\\\\repo\"]}}\n"
        );

        let update = ClientRequest {
            method: "fuzzyFileSearch/sessionUpdate",
            id: 10,
            params: Some(FuzzyFileSearchSessionUpdateParams {
                session_id: "files-1".to_owned(),
                query: "agents".to_owned(),
            }),
        };
        assert_eq!(
            encoded(&update),
            b"{\"method\":\"fuzzyFileSearch/sessionUpdate\",\"id\":10,\"params\":{\"sessionId\":\"files-1\",\"query\":\"agents\"}}\n"
        );

        let legacy = ClientRequest {
            method: "fuzzyFileSearch",
            id: 11,
            params: Some(FuzzyFileSearchParams {
                query: "agents".to_owned(),
                roots: vec![PathBuf::from("C:\\repo")],
                cancellation_token: Some("vscode-fuzzy-file-search".to_owned()),
            }),
        };
        assert_eq!(
            encoded(&legacy),
            b"{\"method\":\"fuzzyFileSearch\",\"id\":11,\"params\":{\"query\":\"agents\",\"roots\":[\"C:\\\\repo\"],\"cancellationToken\":\"vscode-fuzzy-file-search\"}}\n"
        );

        let update =
            match serde_json::from_value::<FuzzyFileSearchSessionUpdatedNotification>(json!({
                "sessionId": "files-1",
                "query": "agents",
                "files": [{
                    "file_name": "AGENTS.md",
                    "indices": [0, 1],
                    "match_type": "file",
                    "path": "AGENTS.md",
                    "root": "C:\\repo",
                    "score": 87
                }]
            })) {
                Ok(update) => update,
                Err(error) => panic!("stable fuzzy update should decode: {error}"),
            };
        assert_eq!(update.session_id, "files-1");
        assert_eq!(update.files[0].match_type, FuzzyFileSearchMatchType::File);

        let legacy = match serde_json::from_value::<FuzzyFileSearchResponse>(json!({
            "files": [{
                "file_name": "src",
                "indices": null,
                "match_type": "directory",
                "path": "src",
                "root": "C:\\repo",
                "score": 64
            }]
        })) {
            Ok(legacy) => legacy,
            Err(error) => panic!("stable fuzzy response should decode: {error}"),
        };
        assert_eq!(
            legacy.files[0].match_type,
            FuzzyFileSearchMatchType::Directory
        );

        let completed =
            match serde_json::from_value::<FuzzyFileSearchSessionCompletedNotification>(json!({
                "sessionId": "files-1"
            })) {
                Ok(completed) => completed,
                Err(error) => panic!("stable fuzzy completion should decode: {error}"),
            };
        assert_eq!(completed.session_id, "files-1");
    }

    #[test]
    fn composer_catalog_types_match_the_stable_schema() {
        let model_request = ClientRequest {
            method: "model/list",
            id: 8,
            params: Some(ModelListParams {
                cursor: None,
                limit: Some(64),
                include_hidden: Some(false),
            }),
        };
        assert_eq!(
            encoded(&model_request),
            b"{\"method\":\"model/list\",\"id\":8,\"params\":{\"limit\":64,\"includeHidden\":false}}\n"
        );

        let models = serde_json::from_value::<ModelListResponse>(json!({
            "data": [{
                "id": "gpt-5.6-sol",
                "model": "gpt-5.6-sol",
                "displayName": "GPT-5.6-Sol",
                "description": "Frontier coding model",
                "hidden": false,
                "isDefault": true,
                "defaultReasoningEffort": "low",
                "supportedReasoningEfforts": [{
                    "reasoningEffort": "low",
                    "description": "Fast"
                }],
                "serviceTiers": [{
                    "id": "priority",
                    "name": "Fast",
                    "description": "1.5x speed, increased usage"
                }],
                "defaultServiceTier": "priority",
                "upgradeInfo": {
                    "model": "gpt-5.7",
                    "upgradeCopy": "GPT-5.7 is ready for your next chat.",
                    "modelLink": "https://platform.openai.com/docs/models",
                    "migrationMarkdown": "# Migration notes"
                },
                "availabilityNux": {
                    "message": "Try the newest model."
                }
            }],
            "nextCursor": null
        }));
        assert!(matches!(
            models,
            Ok(response)
                if response.data.len() == 1
                    && response.data[0].model == "gpt-5.6-sol"
                    && response.data[0].supported_reasoning_efforts[0].reasoning_effort == "low"
                    && response.data[0].service_tiers[0].id == "priority"
                    && response.data[0].default_service_tier.as_deref() == Some("priority")
                    && response.data[0]
                        .upgrade_info
                        .as_ref()
                        .is_some_and(|upgrade| {
                            upgrade.model == "gpt-5.7"
                                && upgrade.upgrade_copy.as_deref()
                                    == Some("GPT-5.7 is ready for your next chat.")
                                && upgrade.model_link.as_deref()
                                    == Some("https://platform.openai.com/docs/models")
                                && upgrade.migration_markdown.as_deref()
                                    == Some("# Migration notes")
                        })
                    && response.data[0]
                        .availability_nux
                        .as_ref()
                        .is_some_and(|nux| nux.message == "Try the newest model.")
        ));

        let profiles = serde_json::from_value::<PermissionProfileListResponse>(json!({
            "data": [{
                "id": ":workspace",
                "allowed": true,
                "description": null
            }],
            "nextCursor": null
        }));
        assert!(matches!(
            profiles,
            Ok(response)
                if response.data.len() == 1
                    && response.data[0].id == ":workspace"
                    && response.data[0].allowed
        ));

        assert_eq!(
            encoded(&ClientRequest {
                method: "permissionProfile/list",
                id: 9,
                params: Some(PermissionProfileListParams {
                    cursor: None,
                    limit: Some(64),
                    cwd: None,
                }),
            }),
            b"{\"method\":\"permissionProfile/list\",\"id\":9,\"params\":{\"limit\":64}}\n"
        );

        assert_eq!(
            encoded(&ClientRequest::<()> {
                method: "configRequirements/read",
                id: 10,
                params: None,
            }),
            b"{\"method\":\"configRequirements/read\",\"id\":10}\n"
        );

        let requirements = serde_json::from_value::<ConfigRequirementsReadResponse>(json!({
            "requirements": {
                "allowRemoteControl": true,
                "allowedApprovalPolicies": ["on-request", "never"],
                "allowedApprovalsReviewers": ["user", "auto_review"],
                "allowedSandboxModes": ["read-only", "workspace-write"],
                "defaultPermissions": ":workspace",
                "models": {
                    "newThread": {
                        "model": "gpt-5.6-sol",
                        "modelReasoningEffort": "high",
                        "serviceTier": "priority"
                    }
                }
            }
        }));
        assert!(matches!(
            requirements,
            Ok(response)
                if response.requirements.as_ref().is_some_and(|requirements| {
                    requirements.allow_remote_control == Some(true)
                        && requirements.default_permissions.as_deref() == Some(":workspace")
                        && requirements.allowed_approvals_reviewers.as_deref()
                            == Some(&[
                                ApprovalsReviewer::User,
                                ApprovalsReviewer::AutoReview,
                            ])
                        && requirements.allowed_sandbox_modes.as_deref()
                            == Some(&["read-only".to_owned(), "workspace-write".to_owned()])
                        && requirements.models.as_ref()
                            .and_then(|models| models.new_thread.as_ref())
                            .and_then(|defaults| defaults.service_tier.as_deref())
                            == Some("priority")
                })
        ));

        let legacy_reviewer =
            serde_json::from_value::<ApprovalsReviewer>(json!("guardian_subagent"));
        assert_eq!(legacy_reviewer.ok(), Some(ApprovalsReviewer::AutoReview));

        assert_eq!(
            encoded(&ClientRequest {
                method: "config/read",
                id: 11,
                params: Some(ConfigReadParams::default()),
            }),
            b"{\"method\":\"config/read\",\"id\":11,\"params\":{}}\n"
        );
        let config = serde_json::from_value::<ConfigReadResponse>(json!({
            "config": {
                "model": "gpt-5.6-sol",
                "model_reasoning_effort": "high",
                "service_tier": "priority",
                "profile": "work",
                "personality": "pragmatic",
                "model_personality": "friendly",
                "approval_policy": "on-request",
                "approvals_reviewer": "auto_review",
                "sandbox_mode": "workspace-write",
                "sandbox_workspace_write": {
                    "network_access": true
                },
                "features": {
                    "memories": true
                },
                "memories": {
                    "generate_memories": true,
                    "use_memories": false,
                    "disable_on_external_context": true,
                    "no_memories_if_mcp_or_web_search": false
                },
                "mcp_servers": {
                    "calendar": {
                        "name": "Calendar",
                        "enabled": false,
                        "command": "calendar-mcp"
                    }
                }
            },
            "origins": {
                "mcp_servers.calendar.command": {
                    "name": {
                        "type": "project",
                        "dotCodexFolder": "C:\\isolated\\repo\\.codex"
                    },
                    "version": "sha256:fixture"
                }
            },
            "layers": [{
                "name": {
                    "type": "user",
                    "file": "C:\\isolated\\.codex\\config.toml"
                },
                "version": "sha256:fixture",
                "config": {
                    "computer_use": {
                        "windows": {
                            "always_allowed_app_ids": {
                                "mspaint.exe": true,
                                "blocked.exe": false
                            }
                        }
                    }
                }
            }]
        }));
        assert!(matches!(
            config,
            Ok(response)
                if response.config.model.as_deref() == Some("gpt-5.6-sol")
                    && response.config.model_reasoning_effort.as_deref() == Some("high")
                    && response.config.service_tier.as_deref() == Some("priority")
                    && response.config.profile.as_deref() == Some("work")
                    && response.config.personality.as_deref() == Some("pragmatic")
                    && response.config.model_personality.as_deref() == Some("friendly")
                    && response.config.approval_policy.as_ref() == Some(&json!("on-request"))
                    && response.config.approvals_reviewer
                        == Some(ApprovalsReviewer::AutoReview)
                    && response.config.sandbox_mode.as_deref() == Some("workspace-write")
                    && response.config.sandbox_workspace_write.as_ref()
                        .and_then(|workspace| workspace.network_access) == Some(true)
                    && response.config.features.memories == Some(true)
                    && response.config.memories.as_ref().is_some_and(|memories| {
                        memories.generate_memories == Some(true)
                            && memories.use_memories == Some(false)
                            && memories.disable_on_external_context == Some(true)
                            && memories.no_memories_if_mcp_or_web_search == Some(false)
                    })
                    && response.config.mcp_servers["calendar"].name.as_deref()
                        == Some("Calendar")
                    && response.config.mcp_servers["calendar"].enabled == Some(false)
                    && response.config.mcp_servers["calendar"].command.as_deref()
                        == Some("calendar-mcp")
                    && response.origins["mcp_servers.calendar.command"]
                        .name
                        .get("type")
                        .and_then(Value::as_str)
                        == Some("project")
                    && response.layers.as_ref()
                        .and_then(|layers| layers.first())
                        .and_then(|layer| layer.config.pointer(
                            "/computer_use/windows/always_allowed_app_ids/mspaint.exe"
                        ))
                        .and_then(Value::as_bool)
                        == Some(true)
        ));
    }

    #[test]
    fn remote_control_types_match_the_stable_schema() {
        assert_eq!(
            encoded(&ClientRequest {
                method: "remoteControl/enable",
                id: 14,
                params: Some(RemoteControlEnableParams { ephemeral: true }),
            }),
            b"{\"method\":\"remoteControl/enable\",\"id\":14,\"params\":{\"ephemeral\":true}}\n"
        );
        assert_eq!(
            encoded(&ClientRequest {
                method: "remoteControl/disable",
                id: 15,
                params: Some(RemoteControlDisableParams { ephemeral: false }),
            }),
            b"{\"method\":\"remoteControl/disable\",\"id\":15,\"params\":{}}\n"
        );
        let nullable_enable: NullableRemoteControlEnableParams = None;
        assert_eq!(
            encoded(&ClientRequest {
                method: "remoteControl/enable",
                id: 21,
                params: Some(nullable_enable),
            }),
            b"{\"method\":\"remoteControl/enable\",\"id\":21,\"params\":null}\n"
        );
        let nullable_disable: NullableRemoteControlDisableParams = None;
        assert_eq!(
            encoded(&ClientRequest {
                method: "remoteControl/disable",
                id: 22,
                params: Some(nullable_disable),
            }),
            b"{\"method\":\"remoteControl/disable\",\"id\":22,\"params\":null}\n"
        );
        assert_eq!(
            encoded(&ClientRequest::<()> {
                method: "remoteControl/status/read",
                id: 16,
                params: None,
            }),
            b"{\"method\":\"remoteControl/status/read\",\"id\":16}\n"
        );
        assert_eq!(
            encoded(&ClientRequest {
                method: "remoteControl/pairing/start",
                id: 17,
                params: Some(RemoteControlPairingStartParams { manual_code: true }),
            }),
            b"{\"method\":\"remoteControl/pairing/start\",\"id\":17,\"params\":{\"manualCode\":true}}\n"
        );
        assert_eq!(
            encoded(&ClientRequest {
                method: "remoteControl/pairing/status",
                id: 18,
                params: Some(RemoteControlPairingStatusParams {
                    pairing_code: None,
                    manual_pairing_code: None,
                }),
            }),
            b"{\"method\":\"remoteControl/pairing/status\",\"id\":18,\"params\":{\"pairingCode\":null,\"manualPairingCode\":null}}\n"
        );
        assert_eq!(
            encoded(&ClientRequest {
                method: "remoteControl/client/list",
                id: 19,
                params: Some(RemoteControlClientsListParams {
                    environment_id: "environment-1".to_owned(),
                    cursor: Some("cursor-1".to_owned()),
                    limit: Some(64),
                    order: Some(RemoteControlClientsListOrder::Desc),
                }),
            }),
            b"{\"method\":\"remoteControl/client/list\",\"id\":19,\"params\":{\"environmentId\":\"environment-1\",\"cursor\":\"cursor-1\",\"limit\":64,\"order\":\"desc\"}}\n"
        );
        assert_eq!(
            encoded(&ClientRequest {
                method: "remoteControl/client/list",
                id: 23,
                params: Some(RemoteControlClientsListParams {
                    environment_id: "environment-1".to_owned(),
                    cursor: None,
                    limit: None,
                    order: None,
                }),
            }),
            b"{\"method\":\"remoteControl/client/list\",\"id\":23,\"params\":{\"environmentId\":\"environment-1\",\"cursor\":null,\"limit\":null,\"order\":null}}\n"
        );
        assert_eq!(
            encoded(&ClientRequest {
                method: "remoteControl/client/revoke",
                id: 20,
                params: Some(RemoteControlClientsRevokeParams {
                    environment_id: "environment-1".to_owned(),
                    client_id: "client-1".to_owned(),
                }),
            }),
            b"{\"method\":\"remoteControl/client/revoke\",\"id\":20,\"params\":{\"environmentId\":\"environment-1\",\"clientId\":\"client-1\"}}\n"
        );

        let status = json!({
            "status": "connected",
            "installationId": "installation-1",
            "environmentId": "environment-1",
            "serverName": "server-1"
        });
        let response = serde_json::from_value::<RemoteControlStatusReadResponse>(status.clone());
        assert!(matches!(
            response,
            Ok(RemoteControlStatusReadResponse {
                status: RemoteControlConnectionStatus::Connected,
                environment_id: Some(environment_id),
                ..
            }) if environment_id == "environment-1"
        ));
        assert!(serde_json::from_value::<RemoteControlEnableResponse>(status.clone()).is_ok());
        assert!(serde_json::from_value::<RemoteControlDisableResponse>(status.clone()).is_ok());
        assert!(serde_json::from_value::<RemoteControlStatusChangedNotification>(status).is_ok());

        let pairing = serde_json::from_value::<RemoteControlPairingStartResponse>(json!({
            "environmentId": "environment-1",
            "expiresAt": 123,
            "pairingCode": "pairing-code",
            "manualPairingCode": null
        }));
        assert!(matches!(
            pairing,
            Ok(RemoteControlPairingStartResponse {
                environment_id,
                expires_at: 123,
                manual_pairing_code: None,
                ..
            }) if environment_id == "environment-1"
        ));
        assert!(matches!(
            serde_json::from_value::<RemoteControlPairingStatusResponse>(
                json!({ "claimed": true })
            ),
            Ok(RemoteControlPairingStatusResponse { claimed: true })
        ));
        assert!(matches!(
            serde_json::from_value::<RemoteControlClientsListResponse>(json!({
                "data": [{
                    "clientId": "client-1",
                    "displayName": "Phone",
                    "deviceType": "mobile",
                    "deviceModel": null,
                    "platform": "android",
                    "osVersion": "1",
                    "appVersion": "2",
                    "lastSeenAt": 456
                }],
                "nextCursor": null
            })),
            Ok(RemoteControlClientsListResponse { data, next_cursor: None })
                if data.len() == 1 && data[0].client_id == "client-1"
        ));
        assert!(serde_json::from_value::<RemoteControlClientsRevokeResponse>(json!({})).is_ok());
    }

    #[test]
    fn config_batch_write_matches_the_stable_schema() {
        let request = ClientRequest {
            method: "config/batchWrite",
            id: 12,
            params: Some(ConfigBatchWriteParams {
                edits: vec![
                    ConfigEdit {
                        key_path: "profiles.work.model".to_owned(),
                        value: json!("gpt-5.6-sol"),
                        merge_strategy: ConfigMergeStrategy::Upsert,
                    },
                    ConfigEdit {
                        key_path: "profiles.work.model_reasoning_effort".to_owned(),
                        value: json!("high"),
                        merge_strategy: ConfigMergeStrategy::Upsert,
                    },
                ],
                file_path: None,
                expected_version: None,
                reload_user_config: true,
            }),
        };

        assert_eq!(
            encoded(&request),
            b"{\"method\":\"config/batchWrite\",\"id\":12,\"params\":{\"edits\":[{\"keyPath\":\"profiles.work.model\",\"value\":\"gpt-5.6-sol\",\"mergeStrategy\":\"upsert\"},{\"keyPath\":\"profiles.work.model_reasoning_effort\",\"value\":\"high\",\"mergeStrategy\":\"upsert\"}],\"filePath\":null,\"expectedVersion\":null,\"reloadUserConfig\":true}}\n"
        );

        let response = serde_json::from_value::<ConfigWriteResponse>(json!({
            "status": "okOverridden",
            "version": "sha256:fixture",
            "filePath": "C:\\isolated\\config.toml",
            "overriddenMetadata": null
        }));
        assert!(matches!(
            response,
            Ok(ConfigWriteResponse {
                status: ConfigWriteStatus::OkOverridden
            })
        ));
    }

    #[test]
    fn mcp_management_types_match_the_stable_schema() {
        assert_eq!(
            encoded(&ClientRequest {
                method: "mcpServerStatus/list",
                id: 13,
                params: Some(ListMcpServerStatusParams {
                    cursor: None,
                    limit: 100,
                    detail: Some(McpServerStatusDetail::Full),
                    thread_id: None,
                }),
            }),
            b"{\"method\":\"mcpServerStatus/list\",\"id\":13,\"params\":{\"limit\":100,\"detail\":\"full\"}}\n"
        );
        let response = serde_json::from_value::<ListMcpServerStatusResponse>(json!({
            "data": [{
                "name": "calendar",
                "serverInfo": {
                    "name": "Calendar MCP",
                    "version": "1.2.3",
                    "title": "Calendar",
                    "description": "Calendar tools",
                    "websiteUrl": "https://example.com"
                },
                "tools": {
                    "calendar.list": {
                        "name": "calendar.list",
                        "title": "List events",
                        "description": "Lists calendar events",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        },
                        "outputSchema": null
                    }
                },
                "resources": [{
                    "name": "today",
                    "uri": "calendar://today",
                    "title": "Today's events",
                    "description": null,
                    "mimeType": "application/json",
                    "size": 128
                }],
                "resourceTemplates": [{
                    "name": "day",
                    "uriTemplate": "calendar://day/{date}",
                    "title": "Events by day",
                    "description": null,
                    "mimeType": "application/json"
                }],
                "authStatus": "notLoggedIn"
            }],
            "nextCursor": null
        }));
        assert!(matches!(
            response,
            Ok(response)
                if response.data.len() == 1
                    && response.data[0].name == "calendar"
                    && response.data[0].auth_status == McpAuthStatus::NotLoggedIn
                    && response.data[0].server_info.as_ref()
                        .map(|info| info.version.as_str()) == Some("1.2.3")
                    && response.data[0].tools["calendar.list"].title.as_deref()
                        == Some("List events")
                    && response.data[0].resources[0].uri == "calendar://today"
                    && response.data[0].resource_templates[0].uri_template
                        == "calendar://day/{date}"
                    && response.next_cursor.is_none()
        ));
        assert_eq!(
            encoded(&ClientRequest {
                method: "mcpServer/resource/read",
                id: 14,
                params: Some(McpResourceReadParams {
                    server: "calendar".to_owned(),
                    uri: "calendar://today".to_owned(),
                    thread_id: None,
                }),
            }),
            b"{\"method\":\"mcpServer/resource/read\",\"id\":14,\"params\":{\"server\":\"calendar\",\"uri\":\"calendar://today\"}}\n"
        );
        let resource = serde_json::from_value::<McpResourceReadResponse>(json!({
            "contents": [{
                "uri": "calendar://today",
                "mimeType": "application/json",
                "text": "{\"events\":[]}"
            }, {
                "uri": "calendar://image",
                "mimeType": "image/png",
                "blob": "AA=="
            }]
        }));
        assert!(matches!(
            resource,
            Ok(response)
                if response.contents.len() == 2
                    && response.contents[0].text.as_deref() == Some("{\"events\":[]}")
                    && response.contents[1].blob.as_deref() == Some("AA==")
        ));
        assert_eq!(
            serde_json::to_value(McpServerOauthLoginParams {
                name: "calendar".to_owned(),
                thread_id: None,
                scopes: None,
                timeout_secs: None,
            })
            .ok(),
            Some(json!({ "name": "calendar" }))
        );
        let notification = serde_json::from_value::<McpServerStatusUpdatedNotification>(json!({
            "threadId": null,
            "name": "calendar",
            "status": "failed",
            "error": "OAuth token expired",
            "failureReason": "reauthenticationRequired"
        }));
        assert!(matches!(
            notification,
            Ok(McpServerStatusUpdatedNotification {
                status: McpServerStartupState::Failed,
                failure_reason: Some(McpServerStartupFailureReason::ReauthenticationRequired),
                ..
            })
        ));
    }

    #[test]
    fn mcp_elicitation_types_match_the_stable_schema() {
        let request = serde_json::from_value::<McpServerElicitationRequestParams>(json!({
            "serverName": "calendar",
            "threadId": "thread-1",
            "turnId": "turn-1",
            "mode": "url",
            "elicitationId": "elicitation-1",
            "message": "Connect your calendar to continue.",
            "url": "https://example.com/connect",
            "_meta": {"source": "fixture"}
        }));
        assert!(matches!(
            request,
            Ok(McpServerElicitationRequestParams {
                server_name,
                thread_id,
                turn_id: Some(turn_id),
                request: McpServerElicitationRequest::Url {
                    elicitation_id,
                    message,
                    url,
                    metadata: Some(_),
                },
            }) if server_name == "calendar"
                && thread_id == "thread-1"
                && turn_id == "turn-1"
                && elicitation_id == "elicitation-1"
                && message == "Connect your calendar to continue."
                && url == "https://example.com/connect"
        ));

        let form = serde_json::from_value::<McpServerElicitationRequestParams>(json!({
            "serverName": "calendar",
            "threadId": "thread-1",
            "mode": "form",
            "message": "Choose a calendar and date.",
            "requestedSchema": {
                "type": "object",
                "properties": {
                    "calendar": {
                        "type": "string",
                        "title": "Calendar",
                        "oneOf": [
                            {"const": "work", "title": "Work"},
                            {"const": "personal", "title": "Personal"}
                        ],
                        "default": "work"
                    },
                    "date": {
                        "type": "string",
                        "format": "date"
                    },
                    "notify": {
                        "type": "boolean",
                        "default": true
                    }
                },
                "required": ["calendar", "date"]
            }
        }));
        assert!(matches!(
            form,
            Ok(McpServerElicitationRequestParams {
                request: McpServerElicitationRequest::Form {
                    requested_schema,
                    ..
                },
                ..
            }) if matches!(
                requested_schema.properties.get("calendar"),
                Some(McpElicitationPrimitiveSchema::String {
                    one_of: Some(options),
                    default: Some(default),
                    ..
                }) if options.len() == 2 && default == "work"
            ) && matches!(
                requested_schema.properties.get("notify"),
                Some(McpElicitationPrimitiveSchema::Boolean {
                    default: Some(true),
                    ..
                })
            )
        ));

        let openai_form = serde_json::from_value::<McpServerElicitationRequestParams>(json!({
            "serverName": "templates",
            "threadId": "thread-1",
            "mode": "openai/form",
            "message": "Choose a template.",
            "requestedSchema": {
                "type": "object",
                "properties": {
                    "template": {
                        "type": "openai/imagePicker",
                        "title": "Template",
                        "items": [
                            {
                                "id": "clean",
                                "title": "Clean",
                                "image": "data:image/png;base64,AA=="
                            }
                        ]
                    }
                },
                "required": ["template"]
            }
        }));
        assert!(matches!(
            openai_form,
            Ok(McpServerElicitationRequestParams {
                request: McpServerElicitationRequest::OpenAiForm {
                    requested_schema,
                    ..
                },
                ..
            }) if matches!(
                requested_schema.properties.get("template"),
                Some(super::McpOpenAiElicitationFieldSchema::ImagePicker(schema))
                    if schema.items.len() == 1
                        && schema.items[0].id == "clean"
                        && schema.items[0].title == "Clean"
            )
        ));

        let response = serde_json::to_value(McpServerElicitationRequestResponse {
            action: McpServerElicitationAction::Accept,
            content: Some(json!({"calendar": "work", "notify": true})),
            metadata: None,
        });
        assert!(matches!(
            response,
            Ok(value) if value == json!({
                "action": "accept",
                "content": {"calendar": "work", "notify": true}
            })
        ));
    }

    #[test]
    fn skills_catalog_types_match_the_stable_schema() {
        assert_eq!(
            encoded(&ClientRequest {
                method: "skills/list",
                id: 13,
                params: Some(SkillsListParams {
                    cwds: vec![PathBuf::from("C:\\isolated\\repo")],
                    force_reload: true,
                }),
            }),
            b"{\"method\":\"skills/list\",\"id\":13,\"params\":{\"cwds\":[\"C:\\\\isolated\\\\repo\"],\"forceReload\":true}}\n"
        );

        let response = serde_json::from_value::<SkillsListResponse>(json!({
            "data": [{
                "cwd": "C:\\isolated\\repo",
                "skills": [{
                    "name": "review",
                    "description": "Review a change",
                    "shortDescription": "Review changes",
                    "interface": {
                        "displayName": "Code review",
                        "shortDescription": "Review changes",
                        "iconSmall": null,
                        "iconLarge": null,
                        "brandColor": null,
                        "defaultPrompt": "Review this change"
                    },
                    "dependencies": null,
                    "path": "C:\\isolated\\repo\\.agents\\skills\\review\\SKILL.md",
                    "scope": "repo",
                    "enabled": true
                }],
                "errors": []
            }]
        }));
        assert!(matches!(
            response,
            Ok(response)
                if response.data.len() == 1
                    && response.data[0].skills[0].scope == SkillScope::Repo
                    && response.data[0].skills[0]
                        .presentation
                        .as_ref()
                        .and_then(|presentation| presentation.display_name.as_deref())
                        == Some("Code review")
        ));

        assert_eq!(
            encoded(&ClientRequest {
                method: "skills/config/write",
                id: 14,
                params: Some(SkillsConfigWriteParams {
                    path: Some(PathBuf::from(
                        "C:\\isolated\\repo\\.agents\\skills\\review\\SKILL.md",
                    )),
                    name: None,
                    enabled: false,
                }),
            }),
            b"{\"method\":\"skills/config/write\",\"id\":14,\"params\":{\"path\":\"C:\\\\isolated\\\\repo\\\\.agents\\\\skills\\\\review\\\\SKILL.md\",\"enabled\":false}}\n"
        );
        let write_response =
            serde_json::from_value::<SkillsConfigWriteResponse>(json!({"effectiveEnabled": false}));
        assert!(matches!(
            write_response,
            Ok(SkillsConfigWriteResponse {
                effective_enabled: false
            })
        ));
    }

    #[test]
    fn hooks_catalog_types_match_the_stable_schema() {
        assert_eq!(
            encoded(&ClientRequest {
                method: "hooks/list",
                id: 15,
                params: Some(HooksListParams {
                    cwds: vec![PathBuf::from("C:\\isolated\\repo")],
                }),
            }),
            b"{\"method\":\"hooks/list\",\"id\":15,\"params\":{\"cwds\":[\"C:\\\\isolated\\\\repo\"]}}\n"
        );

        let response = serde_json::from_value::<HooksListResponse>(json!({
            "data": [{
                "cwd": "C:\\isolated\\repo",
                "hooks": [{
                    "key": "C:\\isolated\\repo\\.codex\\hooks.json:pre_tool_use:0:0",
                    "eventName": "preToolUse",
                    "handlerType": "command",
                    "isManaged": false,
                    "matcher": "shell",
                    "command": "python hook.py",
                    "timeoutSec": 5,
                    "statusMessage": "Checking command",
                    "additionalContextLimit": null,
                    "sourcePath": "C:\\isolated\\repo\\.codex\\hooks.json",
                    "source": "project",
                    "pluginId": null,
                    "displayOrder": 0,
                    "enabled": true,
                    "currentHash": "sha256:fixture",
                    "trustStatus": "untrusted"
                }],
                "warnings": ["warning"],
                "errors": [{
                    "path": "C:\\isolated\\repo\\.codex\\hooks.json",
                    "message": "fixture issue"
                }]
            }]
        }));
        assert!(matches!(
            response,
            Ok(response)
                if response.data.len() == 1
                    && response.data[0].hooks[0].event_name == HookEventName::PreToolUse
                    && response.data[0].hooks[0].handler_type == HookHandlerType::Command
                    && response.data[0].hooks[0].source == HookSource::Project
                    && response.data[0].hooks[0].trust_status == HookTrustStatus::Untrusted
                    && response.data[0].warnings == ["warning"]
                    && response.data[0].errors[0].message == "fixture issue"
        ));
    }

    #[test]
    fn plugin_detail_types_match_the_stable_schema() {
        assert_eq!(
            encoded(&ClientRequest {
                method: "plugin/read",
                id: 15,
                params: Some(PluginReadParams {
                    marketplace_path: None,
                    remote_marketplace_name: Some("openai-curated-remote".to_owned()),
                    plugin_name: "gmail".to_owned(),
                }),
            }),
            b"{\"method\":\"plugin/read\",\"id\":15,\"params\":{\"remoteMarketplaceName\":\"openai-curated-remote\",\"pluginName\":\"gmail\"}}\n"
        );

        let response = serde_json::from_value::<PluginReadResponse>(json!({
            "plugin": {
                "marketplaceName": "openai-curated-remote",
                "marketplacePath": null,
                "summary": {
                    "id": "gmail@openai-curated-remote",
                    "name": "gmail",
                    "installed": false,
                    "enabled": true,
                    "interface": {
                        "displayName": "Gmail",
                        "shortDescription": "Work with Gmail",
                        "longDescription": "Search mail and draft replies.",
                        "developerName": "OpenAI",
                        "category": "Productivity",
                        "capabilities": ["Search", "Draft"],
                        "websiteUrl": "https://example.test",
                        "privacyPolicyUrl": null,
                        "termsOfServiceUrl": null,
                        "defaultPrompt": ["Find the latest vendor email"],
                        "logoUrl": null,
                        "logoUrlDark": null,
                        "screenshotUrls": []
                    }
                },
                "shareUrl": "https://example.test/plugins/gmail/share",
                "description": "Search mail and draft replies.",
                "skills": [{
                    "name": "draft-reply",
                    "description": "Draft a contextual reply",
                    "shortDescription": "Draft replies",
                    "enabled": true
                }],
                "hooks": [],
                "apps": [{
                    "id": "gmail",
                    "name": "Gmail",
                    "description": "Connect Gmail",
                    "installUrl": "https://example.test/connect",
                    "category": "Productivity"
                }],
                "appTemplates": [],
                "mcpServers": ["gmail"],
                "scheduledTasks": [{
                    "key": "daily-inbox",
                    "name": "Daily inbox",
                    "prompt": "Summarize new mail",
                    "schedule": {
                        "type": "daily",
                        "time": "09:00"
                    }
                }]
            }
        }));

        assert!(matches!(
            response,
            Ok(response)
                if response.plugin.summary.id == "gmail@openai-curated-remote"
                    && response.plugin.summary.must_show_installation_interstitial.is_none()
                    && response.plugin.share_url.as_deref()
                        == Some("https://example.test/plugins/gmail/share")
                    && response.plugin.skills.len() == 1
                    && response.plugin.apps.len() == 1
                    && response.plugin.mcp_servers == ["gmail"]
                    && matches!(
                        response.plugin.scheduled_tasks.as_deref(),
                        Some([PluginScheduledTaskSummary {
                            schedule: PluginScheduledTaskSchedule::Daily { time },
                            ..
                        }]) if time == "09:00"
                    )
        ));
    }

    #[test]
    fn plugin_installation_interstitial_policy_preserves_true_false_null_and_absence() {
        for (value, expected) in [
            (Some(json!(true)), Some(true)),
            (Some(json!(false)), Some(false)),
            (Some(json!(null)), None),
            (None, None),
        ] {
            let mut summary = json!({
                "id": "example@marketplace",
                "name": "example"
            });
            if let Some(value) = value {
                summary["mustShowInstallationInterstitial"] = value;
            }
            let summary = serde_json::from_value::<PluginSummary>(summary);
            assert_eq!(
                summary
                    .ok()
                    .and_then(|summary| summary.must_show_installation_interstitial),
                expected
            );
        }
    }

    #[test]
    fn thread_lifecycle_params_match_the_stable_schema() {
        assert_eq!(
            serde_json::to_value(ThreadForkParams {
                thread_id: "thread-1".to_owned(),
                cwd: Some(PathBuf::from("C:\\repo-worktree")),
                last_turn_id: None,
                before_turn_id: None,
                exclude_turns: Some(true),
                defer_goal_continuation: Some(true),
            })
            .ok(),
            Some(json!({
                "threadId": "thread-1",
                "cwd": "C:\\repo-worktree",
                "excludeTurns": true,
                "deferGoalContinuation": true
            }))
        );
        assert_eq!(
            serde_json::to_value(ThreadArchiveParams {
                thread_id: "thread-1".to_owned(),
            })
            .ok(),
            Some(json!({"threadId": "thread-1"}))
        );
        assert_eq!(
            serde_json::to_value(ThreadUnsubscribeParams {
                thread_id: "thread-1".to_owned(),
            })
            .ok(),
            Some(json!({"threadId": "thread-1"}))
        );
        assert!(matches!(
            serde_json::from_value::<ThreadUnsubscribeResponse>(json!({
                "status": "unsubscribed"
            })),
            Ok(ThreadUnsubscribeResponse {
                status: ThreadUnsubscribeStatus::Unsubscribed
            })
        ));
        assert_eq!(
            serde_json::to_value(ThreadUnarchiveParams {
                thread_id: "thread-1".to_owned(),
            })
            .ok(),
            Some(json!({"threadId": "thread-1"}))
        );
        assert_eq!(
            serde_json::to_value(ThreadDeleteParams {
                thread_id: "thread-1".to_owned(),
            })
            .ok(),
            Some(json!({"threadId": "thread-1"}))
        );
        assert_eq!(
            serde_json::to_value(ThreadSetNameParams {
                thread_id: "thread-1".to_owned(),
                name: "Native parity".to_owned(),
            })
            .ok(),
            Some(json!({
                "threadId": "thread-1",
                "name": "Native parity"
            }))
        );
    }

    #[test]
    fn thread_goal_contract_matches_the_stable_schema() {
        assert_eq!(
            serde_json::to_value(ThreadGoalSetParams {
                thread_id: "thread-1".to_owned(),
                objective: Some("Reach native parity".to_owned()),
                status: Some(ThreadGoalStatus::Active),
                token_budget: Some(Some(50_000)),
            })
            .ok(),
            Some(json!({
                "threadId": "thread-1",
                "objective": "Reach native parity",
                "status": "active",
                "tokenBudget": 50_000
            }))
        );
        assert_eq!(
            serde_json::to_value(ThreadGoalSetParams {
                thread_id: "thread-1".to_owned(),
                objective: None,
                status: Some(ThreadGoalStatus::Paused),
                token_budget: None,
            })
            .ok(),
            Some(json!({
                "threadId": "thread-1",
                "status": "paused"
            }))
        );
        assert_eq!(
            serde_json::to_value(ThreadGoalSetParams {
                thread_id: "thread-1".to_owned(),
                objective: None,
                status: None,
                token_budget: Some(None),
            })
            .ok(),
            Some(json!({
                "threadId": "thread-1",
                "tokenBudget": null
            }))
        );

        let response = serde_json::from_value::<ThreadGoalGetResponse>(json!({
            "goal": {
                "threadId": "thread-1",
                "objective": "Reach native parity",
                "status": "usageLimited",
                "tokensUsed": 12_500,
                "tokenBudget": 50_000,
                "timeUsedSeconds": 3600,
                "createdAt": 10,
                "updatedAt": 20
            }
        }));
        assert!(matches!(
            response,
            Ok(ThreadGoalGetResponse {
                goal: Some(ThreadGoal {
                    status: ThreadGoalStatus::UsageLimited,
                    tokens_used: 12_500,
                    ..
                })
            })
        ));
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
    fn composer_attachments_and_plan_mode_match_the_stable_schema() {
        let params = TurnStartParams {
            thread_id: "thread-1".to_owned(),
            input: vec![
                UserInput::text("inspect these"),
                UserInput::mention("AGENTS.md", PathBuf::from("/repo/AGENTS.md")),
                UserInput::local_image(PathBuf::from("/repo/screen.png")),
            ],
            client_user_message_id: None,
            cwd: None,
            runtime_workspace_roots: None,
            approval_policy: None,
            approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
            permissions: Some(":workspace".to_owned()),
            model: Some("gpt-5.6-sol".to_owned()),
            effort: Some("high".to_owned()),
            service_tier: Some(Some("priority".to_owned())),
            summary: None,
            personality: None,
            output_schema: None,
            collaboration_mode: Some(CollaborationMode {
                mode: CollaborationModeKind::Plan,
                settings: CollaborationModeSettings {
                    model: "gpt-5.6-sol".to_owned(),
                    reasoning_effort: Some("high".to_owned()),
                    developer_instructions: None,
                },
            }),
        };

        assert_eq!(
            serde_json::to_value(params).ok(),
            Some(json!({
                "threadId": "thread-1",
                "input": [
                    {"type": "text", "text": "inspect these", "text_elements": []},
                    {"type": "mention", "name": "AGENTS.md", "path": "/repo/AGENTS.md"},
                    {"type": "localImage", "path": "/repo/screen.png"}
                ],
                "approvalsReviewer": "auto_review",
                "permissions": ":workspace",
                "model": "gpt-5.6-sol",
                "effort": "high",
                "serviceTier": "priority",
                "collaborationMode": {
                    "mode": "plan",
                    "settings": {
                        "model": "gpt-5.6-sol",
                        "reasoning_effort": "high",
                        "developer_instructions": null
                    }
                }
            }))
        );
    }

    #[test]
    fn ephemeral_generation_thread_matches_the_stable_schema() {
        let params = ThreadStartParams {
            model: Some("gpt-5.6-luna".to_owned()),
            model_provider: Some(None),
            allow_provider_model_fallback: Some(true),
            service_tier: Some(None),
            cwd: None,
            runtime_workspace_roots: Some(Vec::new()),
            approval_policy: Some("never".to_owned()),
            approvals_reviewer: None,
            sandbox: None,
            permissions: Some(":read-only".to_owned()),
            ephemeral: Some(true),
            history_mode: None,
            dynamic_tools: None,
            config: Some(json!({
                "features.enable_fanout": false,
                "features.multi_agent": false,
                "features.multi_agent_v2": false,
                "web_search": "disabled",
                "model_reasoning_effort": "low"
            })),
            personality: Some(None),
            thread_source: Some("system".to_owned()),
            experimental_raw_events: Some(false),
            service_name: None,
        };

        assert_eq!(
            serde_json::to_value(params).ok(),
            Some(json!({
                "model": "gpt-5.6-luna",
                "modelProvider": null,
                "allowProviderModelFallback": true,
                "serviceTier": null,
                "runtimeWorkspaceRoots": [],
                "approvalPolicy": "never",
                "permissions": ":read-only",
                "ephemeral": true,
                "config": {
                    "features.enable_fanout": false,
                    "features.multi_agent": false,
                    "features.multi_agent_v2": false,
                    "web_search": "disabled",
                    "model_reasoning_effort": "low"
                },
                "personality": null,
                "threadSource": "system",
                "experimentalRawEvents": false
            }))
        );
    }

    #[test]
    fn active_turn_controls_match_the_stable_schema() {
        assert_eq!(
            serde_json::to_value(ThreadSettingsUpdateParams {
                thread_id: "thread-1".to_owned(),
                approval_policy: Some("on-request".to_owned()),
                approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                permissions: Some(":workspace".to_owned()),
                model: Some("gpt-5.6-sol".to_owned()),
                effort: Some("high".to_owned()),
                service_tier: Some(Some("priority".to_owned())),
            })
            .ok(),
            Some(json!({
                "threadId": "thread-1",
                "approvalPolicy": "on-request",
                "approvalsReviewer": "auto_review",
                "permissions": ":workspace",
                "model": "gpt-5.6-sol",
                "effort": "high",
                "serviceTier": "priority"
            }))
        );
        assert_eq!(
            serde_json::to_value(ThreadSettingsUpdateParams {
                thread_id: "thread-1".to_owned(),
                service_tier: Some(None),
                ..ThreadSettingsUpdateParams::default()
            })
            .ok(),
            Some(json!({
                "threadId": "thread-1",
                "serviceTier": null
            }))
        );
        assert_eq!(
            serde_json::to_value(TurnSteerParams {
                thread_id: "thread-1".to_owned(),
                input: vec![UserInput::text("Focus on the failing test")],
                expected_turn_id: "turn-1".to_owned(),
            })
            .ok(),
            Some(json!({
                "threadId": "thread-1",
                "input": [{
                    "type": "text",
                    "text": "Focus on the failing test",
                    "text_elements": []
                }],
                "expectedTurnId": "turn-1"
            }))
        );
        assert_eq!(
            serde_json::to_value(TurnInterruptParams {
                thread_id: "thread-1".to_owned(),
                turn_id: "turn-1".to_owned(),
            })
            .ok(),
            Some(json!({
                "threadId": "thread-1",
                "turnId": "turn-1"
            }))
        );

        let resumed = serde_json::from_value::<ThreadResumeResponse>(json!({
            "model": "gpt-5.6-sol",
            "reasoningEffort": "high",
            "serviceTier": "priority",
            "approvalPolicy": "on-request",
            "approvalsReviewer": "auto_review",
            "activePermissionProfile": {
                "id": ":workspace"
            },
            "thread": {
                "id": "thread-1",
                "turns": [{
                    "id": "turn-1",
                    "status": "inProgress",
                    "items": []
                }]
            }
        }));
        assert!(matches!(
            resumed,
            Ok(response)
                if response.thread.turns.first()
                    .and_then(|turn| turn.get("status"))
                    == Some(&json!("inProgress"))
                    && response.model.as_deref() == Some("gpt-5.6-sol")
                    && response.reasoning_effort.as_deref() == Some("high")
                    && response.service_tier.as_deref() == Some("priority")
                    && response.approval_policy.as_ref() == Some(&json!("on-request"))
                    && response.approvals_reviewer == Some(ApprovalsReviewer::AutoReview)
                    && response.active_permission_profile.as_ref()
                        .map(|profile| profile.id.as_str()) == Some(":workspace")
        ));
    }

    #[test]
    fn plugin_marketplace_kinds_match_the_stable_schema() {
        assert_eq!(
            serde_json::to_value([
                PluginListMarketplaceKind::Local,
                PluginListMarketplaceKind::Vertical,
                PluginListMarketplaceKind::WorkspaceDirectory,
                PluginListMarketplaceKind::SharedWithMe,
                PluginListMarketplaceKind::CreatedByMeRemote,
            ])
            .ok(),
            Some(json!([
                "local",
                "vertical",
                "workspace-directory",
                "shared-with-me",
                "created-by-me-remote"
            ]))
        );
    }

    #[test]
    fn marketplace_add_params_match_the_stable_schema() {
        assert_eq!(
            serde_json::to_value(MarketplaceAddParams {
                source: "openai/plugins".to_owned(),
                ref_name: Some("main".to_owned()),
                sparse_paths: Some(vec!["plugins/codex".to_owned()]),
            })
            .ok(),
            Some(json!({
                "source": "openai/plugins",
                "refName": "main",
                "sparsePaths": ["plugins/codex"],
            }))
        );
    }

    #[test]
    fn marketplace_management_params_match_the_stable_schema() {
        assert_eq!(
            serde_json::to_value(MarketplaceRemoveParams {
                marketplace_name: "my-marketplace".to_owned(),
            })
            .ok(),
            Some(json!({ "marketplaceName": "my-marketplace" }))
        );
        assert_eq!(
            serde_json::to_value(MarketplaceUpgradeParams {
                marketplace_name: None,
            })
            .ok(),
            Some(json!({ "marketplaceName": null }))
        );
        assert_eq!(
            serde_json::to_value(MarketplaceUpgradeParams {
                marketplace_name: Some("my-marketplace".to_owned()),
            })
            .ok(),
            Some(json!({ "marketplaceName": "my-marketplace" }))
        );
    }

    #[test]
    fn app_read_matches_the_stable_schema() {
        assert_eq!(
            serde_json::to_value(AppsReadParams {
                app_ids: vec!["connector_calendar".to_owned()],
                include_tools: true,
            })
            .ok(),
            Some(json!({
                "appIds": ["connector_calendar"],
                "includeTools": true,
            }))
        );

        let response = serde_json::from_value::<AppsReadResponse>(json!({
            "apps": [{
                "id": "connector_calendar",
                "name": "Calendar",
                "description": "Read and update events.",
                "iconUrl": "https://example.com/calendar.png",
                "iconUrlDark": null,
                "distributionChannel": "openai",
                "installUrl": "https://chatgpt.com/apps/calendar",
                "pluginDisplayNames": ["Calendar"],
                "toolSummaries": [{
                    "name": "list_events",
                    "title": "List events",
                    "description": "Lists calendar events."
                }]
            }],
            "missingAppIds": ["connector_missing"]
        }));
        assert!(matches!(
            response,
            Ok(response)
                if response.apps.len() == 1
                    && response.apps[0].id == "connector_calendar"
                    && response.apps[0]
                        .tool_summaries
                        .as_ref()
                        .and_then(|tools| tools.first())
                        .is_some_and(|tool| tool.name == "list_events")
                    && response.missing_app_ids == ["connector_missing"]
        ));
    }

    #[test]
    fn structured_user_input_matches_the_stable_schema() {
        let params = serde_json::from_value::<ToolRequestUserInputParams>(json!({
            "threadId": "thread-1",
            "turnId": "turn-1",
            "itemId": "item-1",
            "autoResolutionMs": 60_000,
            "questions": [{
                "id": "scope",
                "header": "Scope",
                "question": "Which scope should I use?",
                "options": [{
                    "label": "Focused (Recommended)",
                    "description": "Change only the selected module."
                }],
                "isOther": true,
                "isSecret": false
            }]
        }));
        assert!(matches!(
            params,
            Ok(params)
                if params.thread_id == "thread-1"
                    && params.questions[0].is_other
                    && params.questions[0]
                        .options
                        .as_ref()
                        .is_some_and(|options| options[0].label == "Focused (Recommended)")
        ));

        assert_eq!(
            serde_json::to_value(ToolRequestUserInputResponse {
                answers: std::collections::BTreeMap::from([(
                    "scope".to_owned(),
                    ToolRequestUserInputAnswer {
                        answers: vec!["Focused (Recommended)".to_owned()],
                    },
                )]),
            })
            .ok(),
            Some(json!({
                "answers": {
                    "scope": {
                        "answers": ["Focused (Recommended)"]
                    }
                }
            }))
        );
    }

    #[test]
    fn approval_requests_and_responses_match_the_stable_schema() {
        let command = serde_json::from_value::<CommandExecutionRequestApprovalParams>(json!({
            "threadId": "thread-1",
            "turnId": "turn-1",
            "itemId": "command-1",
            "startedAtMs": 1_727_000_000_000_i64,
            "command": "curl https://example.com",
            "commandActions": [{
                "type": "unknown",
                "command": "curl https://example.com"
            }],
            "networkApprovalContext": {
                "host": "example.com",
                "protocol": "https"
            },
            "proposedExecpolicyAmendment": ["curl", "https://example.com"],
            "proposedNetworkPolicyAmendments": [{
                "action": "allow",
                "host": "example.com"
            }]
        }));
        assert!(matches!(
            command,
            Ok(command)
                if command.item_id == "command-1"
                    && command
                        .network_approval_context
                        .as_ref()
                        .is_some_and(|context| context.host == "example.com")
        ));

        assert_eq!(
            serde_json::to_value(CommandExecutionRequestApprovalResponse {
                decision: CommandExecutionApprovalDecision::AcceptWithExecpolicyAmendment {
                    accept_with_execpolicy_amendment: ExecpolicyAmendment {
                        execpolicy_amendment: vec![
                            "curl".to_owned(),
                            "https://example.com".to_owned(),
                        ],
                    },
                },
            })
            .ok(),
            Some(json!({
                "decision": {
                    "acceptWithExecpolicyAmendment": {
                        "execpolicy_amendment": ["curl", "https://example.com"]
                    }
                }
            }))
        );
        assert_eq!(
            serde_json::to_value(CommandExecutionRequestApprovalResponse {
                decision: CommandExecutionApprovalDecision::ApplyNetworkPolicyAmendment {
                    apply_network_policy_amendment: NetworkPolicyAmendmentDecision {
                        network_policy_amendment: NetworkPolicyAmendment {
                            action: NetworkPolicyRuleAction::Allow,
                            host: "example.com".to_owned(),
                        },
                    },
                },
            })
            .ok(),
            Some(json!({
                "decision": {
                    "applyNetworkPolicyAmendment": {
                        "network_policy_amendment": {
                            "action": "allow",
                            "host": "example.com"
                        }
                    }
                }
            }))
        );
        assert_eq!(
            serde_json::to_value(CommandExecutionRequestApprovalResponse {
                decision: CommandExecutionApprovalDecision::Value(
                    CommandExecutionApprovalDecisionValue::AcceptForSession,
                ),
            })
            .ok(),
            Some(json!({ "decision": "acceptForSession" }))
        );
        assert_eq!(
            serde_json::to_value(FileChangeRequestApprovalResponse {
                decision: FileChangeApprovalDecision::AcceptForSession,
            })
            .ok(),
            Some(json!({ "decision": "acceptForSession" }))
        );

        let permissions = serde_json::from_value::<PermissionsRequestApprovalParams>(json!({
            "threadId": "thread-1",
            "turnId": "turn-1",
            "itemId": "permission-1",
            "startedAtMs": 1_727_000_000_000_i64,
            "cwd": "C:\\work",
            "permissions": {
                "network": { "enabled": true },
                "fileSystem": {
                    "entries": [{
                        "access": "read",
                        "path": {
                            "type": "special",
                            "value": {
                                "kind": "project_roots",
                                "subpath": "docs"
                            }
                        }
                    }]
                }
            }
        }));
        assert!(matches!(
            permissions,
            Ok(permissions)
                if permissions
                    .permissions
                    .file_system
                    .as_ref()
                    .and_then(|file_system| file_system.entries.as_ref())
                    .is_some_and(|entries| entries.len() == 1)
        ));
        assert_eq!(
            serde_json::to_value(PermissionsRequestApprovalResponse {
                permissions: PermissionProfile::default(),
                scope: PermissionGrantScope::Turn,
                strict_auto_review: None,
            })
            .ok(),
            Some(json!({
                "permissions": {},
                "scope": "turn"
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
