use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use codex_core::{
    AccountCredits, AccountKind, AccountProfile, Action, AgentConfigScope, AgentConfigScopeKind,
    AgentConfigurationMutationKind, AppCard, AppDetailView, AppToolCard, AppearancePalette,
    AppearancePreferences, AppearanceSemanticColors, AppearanceTheme, AppearanceVariant,
    ApprovalContext, ApprovalDecision, ApprovalKind, ApprovalRequest,
    ApprovalsReviewer as CoreApprovalsReviewer, ArchivedTaskDeleteKind, ArtifactPreview,
    ArtifactPreviewKind, BackgroundTerminal, BrowserApprovalMode, BrowserDownloadPreferences,
    BrowserDownloadState, BrowserDownloadStatus as CoreBrowserDownloadStatus,
    BrowserMouseButton as CoreBrowserMouseButton, BrowserOriginElicitationDecision,
    BrowserPermissionResource, BrowserPermissionValue, BrowserPermissionsState,
    BrowserResourceElicitationDecision, BrowserSitePermission, BrowserTabState,
    CommandApprovalContext, ComposerAttachment, ComposerAttachmentKind, ComputerApplicationState,
    ComputerWindowState, DiffMarkerStyle, Effect, FileChangeApprovalContext, FuzzyFileMatchType,
    FuzzyFileResult, GitBranchState, GitCommitNextStep, GitDiffScope,
    GitFileKind as CoreGitFileKind, GitFileState, GitPreferences, GitPullRequestNextStep,
    GitPullRequestProvider, GitPullRequestState, GitReviewCommitState, GitReviewMode, GitState,
    GitWorktreeState, HookCard, HookEventName as CoreHookEventName,
    HookHandlerType as CoreHookHandlerType, HookIssue, HookProjectEntry,
    HookSource as CoreHookSource, HookTrustStatus as CoreHookTrustStatus, ImportHistory,
    ImportItemFailure, ImportItemSuccess, ImportItemType, ImportMigrationDetails,
    ImportMigrationItem, ImportPluginMigration, ImportProvider, ImportProviderItems,
    ImportSessionMigration, ImportStartFailure, ImportTypeResult, ImportedConnectorCandidate,
    InspectorPane, IntegratedTerminalShell, KEYBOARD_SHORTCUT_COMMAND_IDS,
    KeyboardShortcutPreferences, LocalProjectSummary, MAX_APPEARANCE_FONT_FAMILY_BYTES,
    MAX_ATTACHMENT_LABEL_BYTES, MAX_BACKGROUND_TERMINALS, MAX_BROWSER_DOWNLOAD_PATH_BYTES,
    MAX_BROWSER_PERMISSION_ORIGIN_BYTES, MAX_BROWSER_SITE_PERMISSIONS, MAX_COMPOSER_ATTACHMENTS,
    MAX_COMPUTER_ALLOWED_APPS, MAX_COMPUTER_APP_ID_BYTES, MAX_FUZZY_FILE_PATH_BYTES,
    MAX_FUZZY_FILE_QUERY_BYTES, MAX_FUZZY_FILE_RESULTS, MAX_FUZZY_FILE_ROOTS,
    MAX_GIT_BRANCH_PREFIX_BYTES, MAX_GIT_DIFF_BYTES, MAX_GIT_INSTRUCTIONS_BYTES, MAX_GIT_SHA_BYTES,
    MAX_HOOK_FIELD_BYTES, MAX_HOOK_ISSUES, MAX_HOOK_ITEMS, MAX_HOOK_PROJECTS,
    MAX_IMPORT_DETAIL_ITEMS, MAX_IMPORT_FIELD_BYTES, MAX_IMPORT_HISTORY_ENTRIES,
    MAX_IMPORT_MIGRATION_ITEMS, MAX_IMPORT_RESULTS_PER_HISTORY, MAX_IMPORT_SESSION_AGE_DAYS,
    MAX_IMPORT_SESSIONS, MAX_KEYBOARD_SHORTCUT_ACCELERATOR_BYTES,
    MAX_KEYBOARD_SHORTCUTS_PER_COMMAND, MAX_LOCAL_PROJECTS, MAX_MCP_FORM_FIELDS,
    MAX_MCP_FORM_IMAGE_DATA_URL_BYTES, MAX_MCP_FORM_OPTIONS, MAX_MCP_FORM_VALUE_BYTES,
    MAX_MCP_SERVER_FIELD_BYTES, MAX_MCP_SERVER_LIST_ITEMS, MAX_PENDING_APPROVALS,
    MAX_REMOTE_CURSOR_BYTES, MAX_REMOTE_DEVICE_ID_BYTES, MAX_REMOTE_DEVICE_LABEL_BYTES,
    MAX_REMOTE_ENVIRONMENT_ID_BYTES, MAX_REMOTE_PAIRING_CODE_BYTES, MAX_RETRYABLE_TURN_MESSAGES,
    MAX_TERMINAL_TABS, MAX_TIMELINE_ITEMS, MAX_TURN_DIFF_BYTES, MAX_USER_INPUT_OPTIONS,
    MAX_USER_INPUT_QUESTIONS, MAX_USER_INPUT_VALUE_BYTES, MAX_VISIBLE_THREADS,
    MAX_WORKTREE_ROOT_BYTES, MainRoute, MarketplaceSourceCard, MarketplaceUpgradeFailure,
    McpAuthStatus as CoreMcpAuthStatus, McpBrowserOriginElicitation, McpBrowserResourceElicitation,
    McpElicitation, McpElicitationContent, McpElicitationDecision, McpElicitationValue,
    McpFormElicitation, McpFormField, McpFormFieldKind, McpFormImagePickerItem, McpFormOption,
    McpFormStringFormat, McpResourceCard, McpResourceContentCard, McpResourceTemplateCard,
    McpServerCard, McpServerDraft, McpServerInfoCard,
    McpServerStartupFailureReason as CoreMcpServerStartupFailureReason,
    McpServerStartupState as CoreMcpServerStartupState, McpToolCard, McpTransportKind,
    McpUrlElicitation, ModelOption, NetworkApprovalContext as CoreNetworkApprovalContext,
    NetworkApprovalProtocol as CoreNetworkApprovalProtocol,
    NetworkPolicyAction as CoreNetworkPolicyAction,
    NetworkPolicyAmendment as CoreNetworkPolicyAmendment, OutputArtifact, OutputArtifactKind,
    PermissionFileSystemAccess, PermissionProfileOption, PermissionRequestDetail,
    PermissionRequirements, PermissionsApprovalContext, Personality, PersonalizationMutationKind,
    PluginCard, PluginDetailItem, PluginDetailView, PluginDirectoryTab, PluginScheduledTaskCard,
    PluginSkillDetail, PrimaryWindowPlacement, PullRequestActivity, PullRequestActivityKind,
    PullRequestCheck, PullRequestCheckStatus, PullRequestCiStatus, PullRequestDetail,
    PullRequestIdentity, PullRequestLifecycle, PullRequestMergeMethod, PullRequestMutation,
    PullRequestRelationship, PullRequestReviewEvent, PullRequestState, PullRequestSummary,
    ReasoningEffortOption as CoreReasoningEffortOption, ReducedMotionPreference,
    RemoteControlRuntimeStatus, RemoteDevice, RemotePairing, RetryableTurnSubmission,
    RetryableUserMessage, ReviewDelivery as CoreReviewDelivery, ReviewTarget as CoreReviewTarget,
    STANDARD_SERVICE_TIER_ID, ServiceTierOption, SkillCard, SkillScope as CoreSkillScope,
    StartedImport, TaskRunStatus, TaskSearchResult, TaskSummary, TerminalDockLocation,
    ThreadGoal as CoreThreadGoal, ThreadGoalStatus as CoreThreadGoalStatus, TimelineCitation,
    TimelineItem, TimelineKind, TimelineSource, UsageLimitWindow, UserInputAnswers,
    UserInputOption as CoreUserInputOption, UserInputQuestion as CoreUserInputQuestion,
    UserInputRequest, appearance_code_theme_supports_variant, computer_app_id_matches,
    is_appearance_code_theme_id,
};
use codex_platform::{
    AppServerConfig, AppServerConnection, AppServerError, AppServerEvent, ArtifactFileKind,
    BrowserConfig, BrowserDownloadStatus as PlatformBrowserDownloadStatus, BrowserEvent,
    BrowserKeyInput as PlatformBrowserKeyInput, BrowserMouseButton as PlatformBrowserMouseButton,
    BrowserSession, CodexHome, CodexHomeKind, ComputerAccessibilityState, ComputerApplication,
    ComputerButton, ComputerCapture, ComputerKey, ComputerUseAccessibilityClient,
    ComputerUseInterruptionMonitor, ComputerUseOverlayTarget, ComputerUseSystemOverlay,
    ComputerUseTurnKey, ComputerWindow, DEFAULT_THREAD_PAGE_LIMIT, GitBranchMutationOutcome,
    GitError, GitFileKind as PlatformGitFileKind, GitHubCheckStatus, GitHubCiStatus,
    GitHubCliAvailability, GitHubCreatePullRequest, GitHubError, GitHubPullRequestActivity,
    GitHubPullRequestActivityKind, GitHubPullRequestCheck, GitHubPullRequestDetail,
    GitHubPullRequestIdentity, GitHubPullRequestLifecycle, GitHubPullRequestMergeMethod,
    GitHubPullRequestRelationship, GitHubPullRequestReviewEvent, GitHubPullRequestReviewState,
    GitHubPullRequestSearchFilters, GitHubPullRequestState, GitHubPullRequestSummary, GitSnapshot,
    RuntimePolicy, TerminalConfig, TerminalEvent, TerminalSession, available_terminal_shells,
    browser_permission_for_url, capture_computer_window, click_computer_window, codexrs_data_dir,
    commit_diff as git_commit_diff, computer_use_platform_available,
    computer_use_target_is_forbidden, create_branch as git_create_branch,
    create_managed_worktree_cancellable as git_create_managed_worktree,
    create_worktree as git_create_worktree, default_browser_download_dir, drag_computer_window,
    git_branch_diff, git_commit, git_commit_message_diff, git_diff, git_pull_request_context,
    git_push, git_snapshot, git_stage, git_stage_all, git_unstage, git_unstage_all,
    github_create_pull_request, github_merge_pull_request, github_post_pull_request_comment,
    github_pull_request_detail, github_pull_request_diff, github_pull_request_status,
    github_search_pull_requests, github_set_pull_request_review_state,
    github_submit_pull_request_review, github_update_pull_request_body,
    github_update_pull_request_title, inspect_artifact, inspect_computer_window,
    inspect_workspace_file, is_supported_artifact_path, list_computer_windows,
    normalize_browser_origin, open_workspace_path, press_computer_key, resolve_codex_binary,
    reveal_artifact, save_artifact_copy, scroll_computer_window,
    switch_branch as git_switch_branch, type_into_computer_window,
    uncommitted_diff as git_uncommitted_diff,
};
use codex_protocol::{
    Account as ProtocolAccount, AccountLoginCompletedNotification, AppInfo,
    AppListUpdatedNotification, AppSummary, ApprovalsReviewer as ProtocolApprovalsReviewer,
    AppsListParams, AppsReadParams, CancelLoginAccountParams, ClientInfo, CollaborationMode,
    CollaborationModeKind, CollaborationModeSettings, CommandAction,
    CommandExecutionApprovalDecision, CommandExecutionApprovalDecisionValue,
    CommandExecutionRequestApprovalParams, CommandExecutionRequestApprovalResponse,
    ConfigBatchWriteParams, ConfigEdit, ConfigMergeStrategy, ConfigReadParams, ConfigReadResponse,
    ConfigWriteStatus, DynamicToolCallOutputContentItem, DynamicToolCallParams,
    DynamicToolCallResponse, ExecpolicyAmendment, ExternalAgentConfigDetectParams,
    ExternalAgentConfigImportCompletedNotification, ExternalAgentConfigImportHistoriesReadResponse,
    ExternalAgentConfigImportItemTypeFailure, ExternalAgentConfigImportItemTypeSuccess,
    ExternalAgentConfigImportParams, ExternalAgentConfigImportProgressNotification,
    ExternalAgentConfigImportTypeResult, ExternalAgentConfigMigrationItem,
    ExternalAgentConfigMigrationItemType, ExternalAgentImportedConnectorCandidate,
    ExternalAgentImportedConnectorSource, ExternalAgentMigrationDetails,
    ExternalAgentNamedMigration, ExternalAgentPluginsMigration, ExternalAgentSessionMigration,
    FeedbackUploadParams, FileChangeApprovalDecision, FileChangeRequestApprovalParams,
    FileChangeRequestApprovalResponse, FileSystemAccessMode, FileSystemPath, FileSystemSpecialPath,
    FuzzyFileSearchMatchType, FuzzyFileSearchParams, FuzzyFileSearchResponse,
    FuzzyFileSearchResult as ProtocolFuzzyFileResult, FuzzyFileSearchSessionCompletedNotification,
    FuzzyFileSearchSessionStartParams, FuzzyFileSearchSessionStopParams,
    FuzzyFileSearchSessionUpdateParams, FuzzyFileSearchSessionUpdatedNotification,
    GetAccountParams, GetAuthStatusParams, GitDiffToRemoteParams, HistorySortDirection,
    HookEventName as ProtocolHookEventName, HookHandlerType as ProtocolHookHandlerType,
    HookSource as ProtocolHookSource, HookTrustStatus as ProtocolHookTrustStatus, HooksListParams,
    InitializeCapabilities, ListMcpServerStatusParams, LoginAccountParams, LoginAccountResponse,
    MarketplaceAddParams, MarketplaceRemoveParams, MarketplaceUpgradeParams,
    McpAuthStatus as ProtocolMcpAuthStatus, McpElicitationArrayItems,
    McpElicitationPrimitiveSchema, McpElicitationStringFormat, McpOpenAiElicitationFieldSchema,
    McpOpenAiImagePickerSchema, McpResourceReadParams, McpServerConfig,
    McpServerElicitationAction as ProtocolMcpServerElicitationAction, McpServerElicitationRequest,
    McpServerElicitationRequestParams, McpServerElicitationRequestResponse,
    McpServerOauthLoginCompletedNotification, McpServerOauthLoginParams,
    McpServerStartupFailureReason as ProtocolMcpServerStartupFailureReason,
    McpServerStartupState as ProtocolMcpServerStartupState,
    McpServerStatus as ProtocolMcpServerStatus, McpServerStatusDetail,
    McpServerStatusUpdatedNotification, ModelListParams, ModelSafetyBufferingUpdatedNotification,
    ModelVerification, ModelVerificationNotification, NetworkApprovalProtocol,
    NetworkPolicyAmendment, NetworkPolicyAmendmentDecision, NetworkPolicyRuleAction,
    PermissionGrantScope, PermissionProfile, PermissionProfileListParams,
    PermissionsRequestApprovalParams, PermissionsRequestApprovalResponse, PlanType,
    PluginInstallParams, PluginListMarketplaceKind, PluginListParams, PluginReadParams,
    PluginUninstallParams, RemoteControlClient, RemoteControlClientsListParams,
    RemoteControlClientsRevokeParams, RemoteControlConnectionStatus,
    RemoteControlPairingStartParams, RemoteControlPairingStatusParams,
    RemoteControlStatusChangedNotification, ReviewDelivery as ProtocolReviewDelivery,
    ReviewStartParams, ReviewTarget as ProtocolReviewTarget, SkillScope as ProtocolSkillScope,
    SkillsConfigWriteParams, SkillsListParams, ThreadArchiveParams, ThreadBackgroundTerminal,
    ThreadBackgroundTerminalsCleanParams, ThreadBackgroundTerminalsListParams,
    ThreadBackgroundTerminalsTerminateParams, ThreadCompactStartParams, ThreadDeleteParams,
    ThreadForkParams, ThreadGoalClearParams, ThreadGoalClearedNotification, ThreadGoalGetParams,
    ThreadGoalSetParams, ThreadGoalStatus as ProtocolThreadGoalStatus,
    ThreadGoalUpdatedNotification, ThreadItemsListParams, ThreadListParams, ThreadLoadedListParams,
    ThreadMemoryMode, ThreadMemoryModeSetParams, ThreadReadParams,
    ThreadResumeInitialTurnsPageParams, ThreadResumeParams, ThreadRollbackParams,
    ThreadSearchParams, ThreadSetNameParams, ThreadSettingsUpdateParams, ThreadShellCommandParams,
    ThreadStartParams, ThreadTokenUsageUpdatedNotification, ThreadTurnsListParams,
    ThreadUnarchiveParams, ThreadUnsubscribeParams, ToolRequestUserInputAnswer,
    ToolRequestUserInputParams, ToolRequestUserInputResponse, TurnDiffUpdatedNotification,
    TurnInterruptParams, TurnStartParams, TurnSteerParams, UserInput,
};
use codex_storage::{
    BrowserDownloadRecordStatus, MAX_BROWSER_DOWNLOAD_RECORDS, Store, StoredBrowserDownload,
};
use crossbeam_channel::{Receiver, Sender, TryRecvError};
use serde_json::{Value, json};

#[cfg(any(windows, test))]
use codex_protocol::{DynamicToolFunction, DynamicToolNamespaceTool, DynamicToolSpec};

const BACKEND_COMMAND_CAPACITY: usize = 64;
const BACKEND_EVENT_CAPACITY: usize = 1_024;
const MAX_RETRYABLE_ACTIVE_TURNS: usize = 64;
const BACKEND_TICK: Duration = Duration::from_millis(25);
const APP_SERVER_RECONNECT_INITIAL_DELAY: Duration = Duration::from_secs(1);
const APP_SERVER_RECONNECT_MAX_DELAY: Duration = Duration::from_secs(20);
const UI_EVENT_TIMEOUT: Duration = Duration::from_millis(100);
const HISTORY_PAGE_LIMIT: u32 = 100;
const MAX_REVIEW_MODE_PAGES: usize = MAX_TIMELINE_ITEMS.div_ceil(HISTORY_PAGE_LIMIT as usize);
const ARCHIVED_DELETE_PAGE_LIMIT: u32 = 200;
const MAX_ARCHIVED_DELETE_PAGES: usize =
    MAX_VISIBLE_THREADS.div_ceil(ARCHIVED_DELETE_PAGE_LIMIT as usize);
const BACKGROUND_TERMINAL_PAGE_LIMIT: u32 = 64;
const COMPOSER_OPTIONS_PAGE_LIMIT: u32 = 64;
const MAX_ITEM_TEXT_BYTES: usize = 256 * 1024;
const MAX_MEMORY_CITATIONS: usize = 64;
const MAX_WEB_SEARCH_SOURCES: usize = 32;
const MAX_CITATION_FIELD_BYTES: usize = 4 * 1024;
const MAX_SOURCE_TITLE_BYTES: usize = 512;
const MAX_SOURCE_URL_BYTES: usize = 8 * 1024;
const MAX_REVIEW_ID_BYTES: usize = 256;
const CODE_REVIEW_START_FAILED: &str = "Couldn't start review.";
const TRUSTED_ACCESS_FOR_CYBER_WARNING: &str = "Your conversations have multiple flags for possible cybersecurity risk. Responses may take longer because extra safety checks are on. To get authorized for security work, join the Trusted Access for Cyber program: https://chatgpt.com/cyber";
const TRUSTED_ACCESS_FOR_CYBER_URL: &str = "https://chatgpt.com/cyber";
const MAX_STATUS_BYTES: usize = 16 * 1024;
const REMOTE_CONTROL_INVALID_RESPONSE: &str = "Remote Control returned an invalid response.";
const REMOTE_CONTROL_STATUS_FAILED: &str = "Remote Control is unavailable. Try again.";
const REMOTE_CONTROL_MUTATION_FAILED: &str = "Remote Control could not be updated. Try again.";
const REMOTE_PAIRING_FAILED: &str = "Remote pairing could not be completed. Try again.";
const REMOTE_DEVICES_FAILED: &str = "Remote devices could not be loaded. Try again.";
const MAX_CONFIG_PATH_BYTES: usize = 32 * 1024;
const MAX_CONFIG_VERSION_BYTES: usize = 512;
const MAX_INTERRUPTED_COMPUTER_TURNS: usize = 64;
const MAX_BLOCKED_COMPUTER_TURNS: usize = 64;
const MAX_SITE_STATUS_RESPONSE_BYTES: usize = 64 * 1024;
const COMPUTER_USE_ESCAPE_STOP_MESSAGE: &str = "Computer Use was stopped by the user with the physical Escape key. Stop your work, do not call further Computer Use tools in this turn, and send a final message noting that the user stopped Computer Use.";
const COMPUTER_USE_MONITOR_UNAVAILABLE_MESSAGE: &str =
    "user input monitor unavailable; guarded input cannot continue";
const COMPUTER_USE_OVERLAY_UNAVAILABLE_MESSAGE: &str =
    "system Computer Use indicator unavailable; guarded input cannot continue";
const COMPUTER_USE_USER_INPUT_STALE_MESSAGE: &str =
    "user input was detected in this window; call get_window_state before continuing";
const COMPUTER_USE_URL_FORBIDDEN_MESSAGE: &str = "Computer Use has been stopped for this turn because it is not allowed on the current browser URL. Stop your work and send a final message noting why Computer Use ended. Note that Computer Use is not allowed on this URL even if the user navigates to it themselves.";
const COMPUTER_USE_URL_VERIFICATION_FAILED_MESSAGE: &str = "Computer Use has been stopped for this turn because it could not verify whether the current browser URL is allowed. Stop your work and send a final message noting why Computer Use ended.";
const COMPUTER_USE_URL_CONFIDENCE_FAILED_MESSAGE: &str = "Computer Use has been stopped for this turn because it could not determine the current browser URL on Windows with enough confidence to enforce policy. Stop your work and send a final message noting why Computer Use ended.";
const COMPUTER_USE_URL_UNSUPPORTED_BROWSER_MESSAGE: &str = "Computer Use has been stopped for this turn because browser URL policy enforcement is not yet supported for the current Windows browser. Stop your work and send a final message noting why Computer Use ended.";
const COMPUTER_USE_SITE_STATUS_ENDPOINT: &str = "https://chatgpt.com/backend-api/aura/site_status";
const COMPUTER_USE_SITE_STATUS_SOURCE: &str = "codex_browser_use";
const MAX_APPROVAL_FIELD_BYTES: usize = 256 * 1024;
const MAX_APPROVAL_LIST_ITEMS: usize = 256;
const MAX_APPROVAL_PATH_BYTES: usize = 8 * 1024;
const MAX_GENERATED_COMMIT_RESPONSE_BYTES: usize = 32 * 1024;
const MAX_GENERATED_GIT_RESPONSE_BYTES: usize = 40 * 1024;
const MAX_TERMINAL_TITLE_BYTES: usize = 512;
const COMMIT_GENERATION_MODEL: &str = "gpt-5.6-luna";
const COMMIT_GENERATION_TIMEOUT: Duration = Duration::from_secs(45);
const COMMIT_GENERATION_PROMPT_CHARS: usize = 20_000;
const MIN_GENERATED_COMMIT_MESSAGE_CHARS: usize = 8;
const MAX_GENERATED_COMMIT_MESSAGE_CHARS: usize = 4_000;
const MAX_GENERATED_COMMIT_SUBJECT_CHARS: usize = 72;
const PULL_REQUEST_GENERATION_PROMPT_CHARS: usize = 30_000;
const COMBINED_GIT_GENERATION_PROMPT_CHARS: usize = 40_000;
const MIN_GENERATED_PULL_REQUEST_TITLE_CHARS: usize = 8;
const MAX_GENERATED_PULL_REQUEST_TITLE_CHARS: usize = 120;
const MIN_GENERATED_PULL_REQUEST_BODY_CHARS: usize = 12;
const MAX_GENERATED_PULL_REQUEST_BODY_CHARS: usize = 30_000;
const TASK_SEARCH_PAGE_LIMIT: u32 = 50;
const TASK_SEARCH_DEBOUNCE: Duration = Duration::from_millis(100);
const PULL_REQUEST_SEARCH_DEBOUNCE: Duration = Duration::from_millis(100);
const PULL_REQUEST_PAGE_LIMIT: usize = 25;
const FUZZY_FILE_SEARCH_CANCELLATION_TOKEN: &str = "vscode-fuzzy-file-search";
const FUZZY_FILE_SEARCH_EXCLUDED_COMPONENTS: &[&str] = &[
    ".git",
    ".hg",
    ".next",
    ".pnpm-store",
    ".svn",
    ".turbo",
    ".yarn",
    "build",
    "coverage",
    "dist",
    "node_modules",
];
const APP_LIST_PAGE_LIMIT: u32 = 100;
const MAX_APP_LIST_PAGES: usize = 5;
const MCP_STATUS_PAGE_LIMIT: u32 = 100;
const CURATED_MARKETPLACE_NAMES: &[&str] = &[
    "codex-official",
    "openai-bundled",
    "openai-curated",
    "openai-curated-remote",
    "openai-primary-runtime",
];
const STABLE_OPT_OUT_NOTIFICATION_METHODS: &[&str] = &[
    "thread/environment/connected",
    "thread/environment/disconnected",
    "rawResponseItem/completed",
    "command/exec/outputDelta",
    "externalAgentConfig/import/progress",
    "thread/compacted",
    "windows/worldWritableWarning",
    "turn/moderationMetadata",
    "authStatusChange",
    "loginChatGptComplete",
    "codex/event/task_started",
    "codex/event/agent_reasoning",
    "codex/event/agent_message",
    "codex/event/task_complete",
    "codex/event/mcp_tool_call_begin",
    "codex/event/mcp_tool_call_end",
    "codex/event/exec_command_begin",
    "codex/event/exec_command_end",
    "codex/event/exec_command_output_delta",
    "codex/event/exec_approval_request",
    "codex/event/apply_patch_approval_request",
    "codex/event/background_event",
    "codex/event/turn_diff",
    "codex/event/get_history_entry_response",
    "codex/event/agent_reasoning_delta",
    "codex/event/agent_reasoning_section_break",
    "codex/event/agent_message_delta",
    "codex/event/stream_error",
    "codex/event/error",
    "codex/event/turn_aborted",
    "codex/event/plan_delta",
    "codex/event/plan_update",
    "codex/event/patch_apply_begin",
    "codex/event/patch_apply_end",
    "codex/event/item_started",
    "codex/event/item_completed",
    "codex/event/user_message",
    "codex/event/agent_reasoning_raw_content",
    "codex/event/agent_reasoning_raw_content_delta",
    "codex/event/web_search_begin",
    "codex/event/web_search_end",
    "codex/event/mcp_list_tools_response",
    "codex/event/list_skills_response",
    "codex/event/list_remote_skills_response",
    "codex/event/remote_skill_downloaded",
    "codex/event/list_custom_prompts_response",
    "codex/event/raw_response_item",
    "codex/event/agent_message_content_delta",
    "codex/event/reasoning_content_delta",
    "codex/event/reasoning_raw_content_delta",
    "codex/event/warning",
    "codex/event/undo_started",
    "codex/event/undo_completed",
    "codex/event/shutdown_complete",
    "codex/event/entered_review_mode",
    "codex/event/exited_review_mode",
    "codex/event/view_image_tool_call",
    "codex/event/mcp_startup_update",
    "codex/event/mcp_startup_complete",
    "codex/event/remote_task_created",
    "codex/event/thread_rolled_back",
    "codex/event/thread_name_updated",
    "codex/event/elicitation_request",
    "codex/event/dynamic_tool_call_request",
    "codex/event/request_user_input",
    "codex/event/terminal_interaction",
    "codex/event/token_count",
    "codex/event/deprecation_notice",
    "thread/closed",
    "rawResponse/completed",
    "warning",
];

fn initialize_capabilities() -> InitializeCapabilities {
    InitializeCapabilities {
        experimental_api: true,
        request_attestation: false,
        mcp_server_openai_form_elicitation: Some(true),
        opt_out_notification_methods: Some(
            STABLE_OPT_OUT_NOTIFICATION_METHODS
                .iter()
                .map(|method| (*method).to_owned())
                .collect(),
        ),
    }
}

fn plugin_directory_marketplace_kinds(
    tab: PluginDirectoryTab,
) -> Option<Vec<PluginListMarketplaceKind>> {
    match tab {
        PluginDirectoryTab::CuratedByOpenAi => None,
        PluginDirectoryTab::SharedWithYou => Some(vec![PluginListMarketplaceKind::SharedWithMe]),
        PluginDirectoryTab::CreatedByMe => Some(vec![PluginListMarketplaceKind::CreatedByMeRemote]),
        PluginDirectoryTab::Workspace => Some(vec![PluginListMarketplaceKind::WorkspaceDirectory]),
        PluginDirectoryTab::Local => Some(vec![PluginListMarketplaceKind::Local]),
    }
}

fn plugin_directory_includes_marketplace(tab: PluginDirectoryTab, marketplace: &str) -> bool {
    tab != PluginDirectoryTab::CuratedByOpenAi || CURATED_MARKETPLACE_NAMES.contains(&marketplace)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AppLogo {
    light: Option<String>,
    dark: Option<String>,
}

struct McpServerCatalog {
    servers: Vec<McpServerCard>,
    plugin_servers: Vec<McpServerCard>,
    warnings: Vec<String>,
}

struct McpRuntimeCatalog {
    auth_status: CoreMcpAuthStatus,
    server_info: Option<McpServerInfoCard>,
    tools: Vec<McpToolCard>,
    resources: Vec<McpResourceCard>,
    resource_templates: Vec<McpResourceTemplateCard>,
    truncated: bool,
}

fn normalized_plugin_name(name: &str) -> String {
    name.trim().to_lowercase()
}

fn index_app_logos(
    logos: &mut HashMap<String, AppLogo>,
    ambiguous_names: &mut HashSet<String>,
    names: impl IntoIterator<Item = String>,
    logo: AppLogo,
) {
    for name in names {
        let name = normalized_plugin_name(&name);
        if name.is_empty() || ambiguous_names.contains(&name) {
            continue;
        }
        match logos.get(&name) {
            Some(existing) if existing != &logo => {
                logos.remove(&name);
                ambiguous_names.insert(name);
            }
            Some(_) => {}
            None => {
                logos.insert(name, logo.clone());
            }
        }
    }
}

fn load_app_logos(app_server: &AppServerConnection) -> HashMap<String, AppLogo> {
    let mut logos = HashMap::new();
    let mut ambiguous_names = HashSet::new();
    let mut cursor = None;
    for _ in 0..MAX_APP_LIST_PAGES {
        let Ok(response) = app_server.list_apps(AppsListParams {
            cursor,
            limit: APP_LIST_PAGE_LIMIT,
            thread_id: None,
            force_refetch: false,
        }) else {
            break;
        };
        for app in response.data {
            let logo = AppLogo {
                light: app.logo_url,
                dark: app.logo_url_dark,
            };
            if logo.light.is_none() && logo.dark.is_none() {
                continue;
            }
            let names = app
                .plugin_display_names
                .into_iter()
                .chain(std::iter::once(app.name));
            index_app_logos(&mut logos, &mut ambiguous_names, names, logo);
        }
        cursor = response.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    logos
}

fn load_apps(
    app_server: &AppServerConnection,
    force_refetch: bool,
) -> Result<Vec<AppCard>, codex_platform::AppServerError> {
    let mut cards = Vec::new();
    let mut cursor = None;
    for _ in 0..MAX_APP_LIST_PAGES {
        let response = app_server.list_apps(AppsListParams {
            cursor,
            limit: APP_LIST_PAGE_LIMIT,
            thread_id: None,
            force_refetch,
        })?;
        for card in map_apps(response.data) {
            cards.push(card);
            if cards.len() == codex_core::MAX_APP_ITEMS {
                return Ok(cards);
            }
        }
        cursor = response.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    Ok(cards)
}

fn map_apps(apps: Vec<AppInfo>) -> Vec<AppCard> {
    apps.into_iter()
        .take(codex_core::MAX_APP_ITEMS)
        .map(|app| AppCard {
            id: app.id,
            name: app.name,
            description: app.description.unwrap_or_default(),
            plugin_display_names: app.plugin_display_names,
            logo_url: app.logo_url,
            logo_url_dark: app.logo_url_dark,
            install_url: app.install_url,
            is_accessible: app.is_accessible,
            enabled: app.is_enabled,
        })
        .collect()
}

fn map_auth_app_summaries(apps: Vec<AppSummary>) -> Vec<AppCard> {
    apps.into_iter()
        .take(codex_core::MAX_PLUGIN_DETAIL_ITEMS)
        .map(|app| AppCard {
            id: app.id,
            name: app.name,
            description: app.description.unwrap_or_default(),
            plugin_display_names: Vec::new(),
            logo_url: None,
            logo_url_dark: None,
            install_url: app.install_url,
            is_accessible: false,
            enabled: false,
        })
        .collect()
}

fn load_composer_plugins(
    app_server: &AppServerConnection,
    cwds: Vec<PathBuf>,
    force_refetch: bool,
    marketplaces: &mut HashMap<String, Option<PathBuf>>,
) -> Result<Vec<PluginCard>, AppServerError> {
    let response = app_server.list_plugins(PluginListParams {
        cwds: (!cwds.is_empty()).then_some(cwds),
        marketplace_kinds: None,
        force_refetch,
    })?;
    let featured = response
        .featured_plugin_ids
        .into_iter()
        .enumerate()
        .map(|(rank, plugin_id)| (plugin_id, rank))
        .collect::<HashMap<_, _>>();
    let mut cards = Vec::new();
    marketplaces.clear();
    for marketplace in response.marketplaces {
        marketplaces.insert(marketplace.name.clone(), marketplace.path.clone());
        for plugin in marketplace.plugins {
            let display_name = plugin
                .presentation
                .as_ref()
                .and_then(|presentation| presentation.display_name.clone())
                .unwrap_or_else(|| plugin.name.clone());
            let description = plugin
                .presentation
                .as_ref()
                .and_then(|presentation| {
                    presentation
                        .short_description
                        .clone()
                        .or_else(|| presentation.long_description.clone())
                })
                .unwrap_or_default();
            let category = plugin
                .presentation
                .as_ref()
                .and_then(|presentation| presentation.category.clone());
            let developer = plugin
                .presentation
                .as_ref()
                .and_then(|presentation| presentation.developer_name.clone());
            let logo_url = plugin
                .presentation
                .as_ref()
                .and_then(|presentation| presentation.logo_url.clone());
            let logo_url_dark = plugin
                .presentation
                .as_ref()
                .and_then(|presentation| presentation.logo_url_dark.clone());
            let default_prompt = plugin.presentation.as_ref().and_then(|presentation| {
                presentation
                    .default_prompt
                    .as_ref()
                    .map(|parts| parts.join("\n"))
                    .filter(|prompt| !prompt.trim().is_empty())
            });
            let version = plugin.local_version.clone().or(plugin.version.clone());
            let installable = plugin.availability.as_deref() != Some("DISABLED_BY_ADMIN")
                && plugin.install_policy.as_deref() != Some("NOT_AVAILABLE");
            cards.push(PluginCard {
                id: plugin.id.clone(),
                install_name: plugin.name,
                marketplace: marketplace.name.clone(),
                name: display_name,
                description,
                category,
                developer,
                logo_url,
                logo_url_dark,
                default_prompt,
                version,
                installed: plugin.installed,
                enabled: plugin.enabled,
                installable,
                featured: featured.contains_key(&plugin.id),
                featured_rank: featured.get(&plugin.id).copied(),
            });
            if cards.len() == codex_core::MAX_MARKETPLACE_ITEMS {
                return Ok(cards);
            }
        }
    }
    Ok(cards)
}

fn load_mcp_servers(
    app_server: &AppServerConnection,
    cwd: Option<PathBuf>,
) -> Result<McpServerCatalog, codex_platform::AppServerError> {
    let config = app_server.read_config(ConfigReadParams {
        include_layers: false,
        cwd: cwd.map(|path| path.to_string_lossy().into_owned()),
    })?;
    let (statuses, warnings) = match app_server.list_mcp_server_status(ListMcpServerStatusParams {
        cursor: None,
        limit: MCP_STATUS_PAGE_LIMIT,
        detail: Some(McpServerStatusDetail::Full),
        thread_id: None,
    }) {
        Ok(response) => (response.data, Vec::new()),
        Err(error) => (
            Vec::new(),
            vec![format!("failed to load MCP runtime status: {error}")],
        ),
    };
    let mut runtime_by_name = statuses
        .into_iter()
        .map(|status| (status.name.clone(), status))
        .collect::<HashMap<_, _>>();
    let origins = config.origins;
    let mut configured_names = HashSet::new();
    let mut servers = config
        .config
        .mcp_servers
        .into_iter()
        .take(codex_core::MAX_MCP_SERVER_ITEMS)
        .map(|(key, config)| {
            let name = config
                .name
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| key.clone());
            configured_names.insert(key.clone());
            configured_names.insert(name.clone());
            let runtime = runtime_by_name
                .remove(&key)
                .or_else(|| runtime_by_name.remove(&name))
                .map(map_mcp_runtime_catalog)
                .unwrap_or_else(empty_mcp_runtime_catalog);
            let origin_prefix = format!("mcp_servers.{key}");
            let read_only = origins.iter().any(|(path, metadata)| {
                (path == &origin_prefix
                    || path
                        .strip_prefix(&origin_prefix)
                        .is_some_and(|suffix| suffix.starts_with('.')))
                    && metadata.name.get("type").and_then(Value::as_str) == Some("project")
            });
            let transport = if config.command.is_some() {
                Some(McpTransportKind::Stdio)
            } else if config.url.is_some() {
                Some(McpTransportKind::StreamableHttp)
            } else {
                None
            };
            McpServerCard {
                key,
                name,
                enabled: config.enabled.unwrap_or(true),
                read_only,
                transport,
                command: config.command.unwrap_or_default(),
                args: config.args,
                env: config.env.into_iter().collect(),
                env_vars: config
                    .env_vars
                    .into_iter()
                    .filter_map(|value| match value {
                        Value::String(name) => Some(name),
                        Value::Object(value) => value
                            .get("name")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                        _ => None,
                    })
                    .collect(),
                cwd: config.cwd.unwrap_or_default(),
                url: config.url.unwrap_or_default(),
                bearer_token_env_var: config.bearer_token_env_var.unwrap_or_default(),
                http_headers: config.http_headers.into_iter().collect(),
                env_http_headers: config.env_http_headers.into_iter().collect(),
                auth_status: runtime.auth_status,
                authorization_url: None,
                startup_state: None,
                startup_error: None,
                startup_failure_reason: None,
                server_info: runtime.server_info,
                tools: runtime.tools,
                resources: runtime.resources,
                resource_templates: runtime.resource_templates,
                inspection_truncated: runtime.truncated,
            }
        })
        .collect::<Vec<_>>();
    servers.sort_by_key(|server| (server.name.to_lowercase(), server.key.to_lowercase()));

    let mut plugin_servers = runtime_by_name
        .into_values()
        .filter(|status| !configured_names.contains(&status.name))
        .take(codex_core::MAX_MCP_SERVER_ITEMS)
        .map(|status| {
            let key = status.name.clone();
            let runtime = map_mcp_runtime_catalog(status);
            McpServerCard {
                key: key.clone(),
                name: key,
                enabled: true,
                read_only: true,
                transport: None,
                command: String::new(),
                args: Vec::new(),
                env: Vec::new(),
                env_vars: Vec::new(),
                cwd: String::new(),
                url: String::new(),
                bearer_token_env_var: String::new(),
                http_headers: Vec::new(),
                env_http_headers: Vec::new(),
                auth_status: runtime.auth_status,
                authorization_url: None,
                startup_state: None,
                startup_error: None,
                startup_failure_reason: None,
                server_info: runtime.server_info,
                tools: runtime.tools,
                resources: runtime.resources,
                resource_templates: runtime.resource_templates,
                inspection_truncated: runtime.truncated,
            }
        })
        .collect::<Vec<_>>();
    plugin_servers.sort_by_key(|server| server.name.to_lowercase());
    Ok(McpServerCatalog {
        servers,
        plugin_servers,
        warnings,
    })
}

fn empty_mcp_runtime_catalog() -> McpRuntimeCatalog {
    McpRuntimeCatalog {
        auth_status: CoreMcpAuthStatus::Unknown,
        server_info: None,
        tools: Vec::new(),
        resources: Vec::new(),
        resource_templates: Vec::new(),
        truncated: false,
    }
}

fn map_mcp_runtime_catalog(status: ProtocolMcpServerStatus) -> McpRuntimeCatalog {
    let mut truncated = status.tools.len() > MAX_MCP_SERVER_LIST_ITEMS
        || status.resources.len() > MAX_MCP_SERVER_LIST_ITEMS
        || status.resource_templates.len() > MAX_MCP_SERVER_LIST_ITEMS;
    let server_info = status.server_info.map(|info| {
        let website_url = info.website_url.and_then(|url| {
            let url = bounded_mcp_field(url, &mut truncated);
            bounded_http_url(Some(url))
        });
        McpServerInfoCard {
            name: bounded_mcp_field(info.name, &mut truncated),
            version: bounded_mcp_field(info.version, &mut truncated),
            title: bounded_mcp_optional(info.title, &mut truncated),
            description: bounded_mcp_optional(info.description, &mut truncated),
            website_url,
        }
    });
    let tools = status
        .tools
        .into_values()
        .take(MAX_MCP_SERVER_LIST_ITEMS)
        .map(|tool| McpToolCard {
            name: bounded_mcp_field(tool.name, &mut truncated),
            title: bounded_mcp_optional(tool.title, &mut truncated),
            description: bounded_mcp_optional(tool.description, &mut truncated),
            input_schema: bounded_mcp_schema(tool.input_schema, &mut truncated),
            output_schema: tool
                .output_schema
                .map(|schema| bounded_mcp_schema(schema, &mut truncated)),
        })
        .collect();
    let resources = status
        .resources
        .into_iter()
        .take(MAX_MCP_SERVER_LIST_ITEMS)
        .map(|resource| McpResourceCard {
            name: bounded_mcp_field(resource.name, &mut truncated),
            uri: bounded_mcp_field(resource.uri, &mut truncated),
            title: bounded_mcp_optional(resource.title, &mut truncated),
            description: bounded_mcp_optional(resource.description, &mut truncated),
            mime_type: bounded_mcp_optional(resource.mime_type, &mut truncated),
            size: resource.size,
        })
        .collect();
    let resource_templates = status
        .resource_templates
        .into_iter()
        .take(MAX_MCP_SERVER_LIST_ITEMS)
        .map(|template| McpResourceTemplateCard {
            name: bounded_mcp_field(template.name, &mut truncated),
            uri_template: bounded_mcp_field(template.uri_template, &mut truncated),
            title: bounded_mcp_optional(template.title, &mut truncated),
            description: bounded_mcp_optional(template.description, &mut truncated),
            mime_type: bounded_mcp_optional(template.mime_type, &mut truncated),
        })
        .collect();

    McpRuntimeCatalog {
        auth_status: map_mcp_auth_status(status.auth_status),
        server_info,
        tools,
        resources,
        resource_templates,
        truncated,
    }
}

fn bounded_mcp_field(value: String, truncated: &mut bool) -> String {
    *truncated |= value.len() > MAX_MCP_SERVER_FIELD_BYTES;
    bounded(value, MAX_MCP_SERVER_FIELD_BYTES)
}

fn bounded_mcp_optional(value: Option<String>, truncated: &mut bool) -> Option<String> {
    value.map(|value| bounded_mcp_field(value, truncated))
}

fn bounded_mcp_schema(value: Value, truncated: &mut bool) -> String {
    let encoded = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_owned());
    bounded_mcp_field(encoded, truncated)
}

fn map_mcp_resource_contents(
    contents: Vec<codex_protocol::McpResourceContent>,
) -> Vec<McpResourceContentCard> {
    let list_truncated = contents.len() > MAX_MCP_SERVER_LIST_ITEMS;
    contents
        .into_iter()
        .take(MAX_MCP_SERVER_LIST_ITEMS)
        .map(|content| {
            let mut truncated = list_truncated
                || content.uri.len() > MAX_MCP_SERVER_FIELD_BYTES
                || content
                    .mime_type
                    .as_ref()
                    .is_some_and(|value| value.len() > MAX_MCP_SERVER_FIELD_BYTES)
                || content
                    .text
                    .as_ref()
                    .is_some_and(|value| value.len() > MAX_ITEM_TEXT_BYTES);
            let uri = bounded_mcp_field(content.uri, &mut truncated);
            let mime_type = bounded_mcp_optional(content.mime_type, &mut truncated);
            let text = content.text.map(|text| bounded(text, MAX_ITEM_TEXT_BYTES));
            let blob_bytes = content.blob.as_deref().map(base64_decoded_size);
            McpResourceContentCard {
                uri,
                mime_type,
                text,
                blob_bytes,
                truncated,
            }
        })
        .collect()
}

fn base64_decoded_size(value: &str) -> u64 {
    let padding = value
        .as_bytes()
        .iter()
        .rev()
        .take_while(|byte| **byte == b'=')
        .take(2)
        .count();
    let encoded = u64::try_from(value.len()).unwrap_or(u64::MAX);
    encoded
        .saturating_div(4)
        .saturating_mul(3)
        .saturating_sub(u64::try_from(padding).unwrap_or(0))
}

fn map_mcp_auth_status(status: ProtocolMcpAuthStatus) -> CoreMcpAuthStatus {
    match status {
        ProtocolMcpAuthStatus::Unsupported => CoreMcpAuthStatus::Unsupported,
        ProtocolMcpAuthStatus::NotLoggedIn => CoreMcpAuthStatus::NotLoggedIn,
        ProtocolMcpAuthStatus::BearerToken => CoreMcpAuthStatus::BearerToken,
        ProtocolMcpAuthStatus::OAuth => CoreMcpAuthStatus::OAuth,
    }
}

fn map_mcp_startup_state(status: ProtocolMcpServerStartupState) -> CoreMcpServerStartupState {
    match status {
        ProtocolMcpServerStartupState::Starting => CoreMcpServerStartupState::Starting,
        ProtocolMcpServerStartupState::Ready => CoreMcpServerStartupState::Ready,
        ProtocolMcpServerStartupState::Failed => CoreMcpServerStartupState::Failed,
        ProtocolMcpServerStartupState::Cancelled => CoreMcpServerStartupState::Cancelled,
    }
}

fn map_mcp_startup_failure_reason(
    reason: ProtocolMcpServerStartupFailureReason,
) -> CoreMcpServerStartupFailureReason {
    match reason {
        ProtocolMcpServerStartupFailureReason::ReauthenticationRequired => {
            CoreMcpServerStartupFailureReason::ReauthenticationRequired
        }
    }
}

const fn map_import_item_type(item_type: ExternalAgentConfigMigrationItemType) -> ImportItemType {
    match item_type {
        ExternalAgentConfigMigrationItemType::AgentsMd => ImportItemType::AgentsMd,
        ExternalAgentConfigMigrationItemType::Config => ImportItemType::Config,
        ExternalAgentConfigMigrationItemType::Skills => ImportItemType::Skills,
        ExternalAgentConfigMigrationItemType::Plugins => ImportItemType::Plugins,
        ExternalAgentConfigMigrationItemType::McpServerConfig => ImportItemType::McpServerConfig,
        ExternalAgentConfigMigrationItemType::Subagents => ImportItemType::Subagents,
        ExternalAgentConfigMigrationItemType::Hooks => ImportItemType::Hooks,
        ExternalAgentConfigMigrationItemType::Commands => ImportItemType::Commands,
        ExternalAgentConfigMigrationItemType::Memory => ImportItemType::Memory,
        ExternalAgentConfigMigrationItemType::Sessions => ImportItemType::Sessions,
    }
}

const fn protocol_import_item_type(
    item_type: ImportItemType,
) -> ExternalAgentConfigMigrationItemType {
    match item_type {
        ImportItemType::AgentsMd => ExternalAgentConfigMigrationItemType::AgentsMd,
        ImportItemType::Config => ExternalAgentConfigMigrationItemType::Config,
        ImportItemType::Skills => ExternalAgentConfigMigrationItemType::Skills,
        ImportItemType::Plugins => ExternalAgentConfigMigrationItemType::Plugins,
        ImportItemType::McpServerConfig => ExternalAgentConfigMigrationItemType::McpServerConfig,
        ImportItemType::Subagents => ExternalAgentConfigMigrationItemType::Subagents,
        ImportItemType::Hooks => ExternalAgentConfigMigrationItemType::Hooks,
        ImportItemType::Commands => ExternalAgentConfigMigrationItemType::Commands,
        ImportItemType::Memory => ExternalAgentConfigMigrationItemType::Memory,
        ImportItemType::Sessions => ExternalAgentConfigMigrationItemType::Sessions,
    }
}

fn bounded_import_optional(value: Option<String>) -> Option<String> {
    value.map(|value| bounded(value, MAX_IMPORT_FIELD_BYTES))
}

fn map_import_details(details: ExternalAgentMigrationDetails) -> ImportMigrationDetails {
    let names = |items: Vec<ExternalAgentNamedMigration>| {
        items
            .into_iter()
            .take(MAX_IMPORT_DETAIL_ITEMS)
            .map(|item| bounded(item.name, MAX_IMPORT_FIELD_BYTES))
            .collect()
    };
    ImportMigrationDetails {
        commands: names(details.commands),
        hooks: names(details.hooks),
        mcp_servers: names(details.mcp_servers),
        memory: details
            .memory
            .unwrap_or_default()
            .into_iter()
            .take(MAX_IMPORT_DETAIL_ITEMS)
            .map(|item| bounded(item, MAX_IMPORT_FIELD_BYTES))
            .collect(),
        plugins: details
            .plugins
            .into_iter()
            .take(MAX_IMPORT_DETAIL_ITEMS)
            .map(|plugin| ImportPluginMigration {
                marketplace_name: bounded(plugin.marketplace_name, MAX_IMPORT_FIELD_BYTES),
                plugin_names: plugin
                    .plugin_names
                    .into_iter()
                    .take(MAX_IMPORT_DETAIL_ITEMS)
                    .map(|name| bounded(name, MAX_IMPORT_FIELD_BYTES))
                    .collect(),
            })
            .collect(),
        sessions: details
            .sessions
            .into_iter()
            .take(MAX_IMPORT_DETAIL_ITEMS)
            .map(|session| ImportSessionMigration {
                cwd: bounded(session.cwd, MAX_IMPORT_FIELD_BYTES),
                path: bounded(session.path, MAX_IMPORT_FIELD_BYTES),
                title: bounded_import_optional(session.title),
            })
            .collect(),
        skills: names(details.skills),
        subagents: names(details.subagents),
    }
}

fn map_import_item(item: ExternalAgentConfigMigrationItem) -> ImportMigrationItem {
    ImportMigrationItem {
        cwd: bounded_import_optional(item.cwd),
        description: bounded(item.description, MAX_IMPORT_FIELD_BYTES),
        details: item.details.map(map_import_details),
        item_type: map_import_item_type(item.item_type),
        selected: true,
    }
}

fn protocol_import_details(details: ImportMigrationDetails) -> ExternalAgentMigrationDetails {
    let names = |items: Vec<String>| {
        items
            .into_iter()
            .map(|name| ExternalAgentNamedMigration { name })
            .collect()
    };
    ExternalAgentMigrationDetails {
        commands: names(details.commands),
        hooks: names(details.hooks),
        mcp_servers: names(details.mcp_servers),
        memory: (!details.memory.is_empty()).then_some(details.memory),
        plugins: details
            .plugins
            .into_iter()
            .map(|plugin| ExternalAgentPluginsMigration {
                marketplace_name: plugin.marketplace_name,
                plugin_names: plugin.plugin_names,
            })
            .collect(),
        sessions: details
            .sessions
            .into_iter()
            .map(|session| ExternalAgentSessionMigration {
                cwd: session.cwd,
                path: session.path,
                title: session.title,
            })
            .collect(),
        skills: names(details.skills),
        subagents: names(details.subagents),
    }
}

fn protocol_import_item(item: ImportMigrationItem) -> ExternalAgentConfigMigrationItem {
    ExternalAgentConfigMigrationItem {
        cwd: item.cwd,
        description: item.description,
        details: item.details.map(protocol_import_details),
        item_type: protocol_import_item_type(item.item_type),
    }
}

fn map_import_success(success: ExternalAgentConfigImportItemTypeSuccess) -> ImportItemSuccess {
    ImportItemSuccess {
        item_type: map_import_item_type(success.item_type),
        cwd: bounded_import_optional(success.cwd),
        source: bounded_import_optional(success.source),
        target: bounded_import_optional(success.target),
    }
}

fn map_import_failure(failure: ExternalAgentConfigImportItemTypeFailure) -> ImportItemFailure {
    ImportItemFailure {
        item_type: map_import_item_type(failure.item_type),
    }
}

fn map_import_type_results(
    results: Vec<ExternalAgentConfigImportTypeResult>,
) -> Vec<ImportTypeResult> {
    results
        .into_iter()
        .take(ImportItemType::ALL.len())
        .map(|result| ImportTypeResult {
            item_type: map_import_item_type(result.item_type),
            successes: result
                .successes
                .into_iter()
                .take(MAX_IMPORT_RESULTS_PER_HISTORY)
                .map(map_import_success)
                .collect(),
            failures: result
                .failures
                .into_iter()
                .take(MAX_IMPORT_RESULTS_PER_HISTORY)
                .map(map_import_failure)
                .collect(),
        })
        .collect()
}

fn map_import_histories(
    response: ExternalAgentConfigImportHistoriesReadResponse,
) -> (Vec<ImportHistory>, Vec<ImportedConnectorCandidate>) {
    let histories = response
        .data
        .into_iter()
        .take(MAX_IMPORT_HISTORY_ENTRIES)
        .map(|history| ImportHistory {
            import_id: bounded(history.import_id, MAX_IMPORT_FIELD_BYTES),
            completed_at_ms: history.completed_at_ms,
            successes: history
                .successes
                .into_iter()
                .take(MAX_IMPORT_RESULTS_PER_HISTORY)
                .map(map_import_success)
                .collect(),
            failures: history
                .failures
                .into_iter()
                .take(MAX_IMPORT_RESULTS_PER_HISTORY)
                .map(map_import_failure)
                .collect(),
        })
        .collect();
    let connectors = response
        .connectors
        .into_iter()
        .filter(|connector| {
            connector.source == ExternalAgentImportedConnectorSource::RemoteMcpServersConfig
        })
        .take(MAX_IMPORT_DETAIL_ITEMS)
        .map(
            |ExternalAgentImportedConnectorCandidate {
                 name,
                 session_count,
                 ..
             }| ImportedConnectorCandidate {
                name: bounded(name, MAX_IMPORT_FIELD_BYTES),
                session_count,
            },
        )
        .collect();
    (histories, connectors)
}

fn mcp_server_config_value(existing: Option<&McpServerConfig>, draft: &McpServerDraft) -> Value {
    let mut value = serde_json::Map::new();
    if let Some(existing) = existing {
        if let Some(enabled) = existing.enabled {
            value.insert("enabled".to_owned(), Value::Bool(enabled));
        }
        if let Some(timeout) = existing
            .startup_timeout_sec
            .and_then(serde_json::Number::from_f64)
        {
            value.insert("startup_timeout_sec".to_owned(), Value::Number(timeout));
        }
        if let Some(timeout) = existing.startup_timeout_ms {
            value.insert(
                "startup_timeout_ms".to_owned(),
                Value::Number(timeout.into()),
            );
        }
        if let Some(timeout) = existing
            .tool_timeout_sec
            .and_then(serde_json::Number::from_f64)
        {
            value.insert("tool_timeout_sec".to_owned(), Value::Number(timeout));
        }
        if let Some(tools) = &existing.enabled_tools {
            value.insert("enabled_tools".to_owned(), json!(tools));
        }
        if let Some(tools) = &existing.disabled_tools {
            value.insert("disabled_tools".to_owned(), json!(tools));
        }
    } else {
        value.insert("enabled".to_owned(), Value::Bool(true));
    }

    match draft.transport {
        McpTransportKind::Stdio => {
            value.insert("command".to_owned(), Value::String(draft.command.clone()));
            if !draft.args.is_empty() {
                value.insert("args".to_owned(), json!(draft.args));
            }
            insert_mcp_record(&mut value, "env", &draft.env);
            if !draft.env_vars.is_empty() {
                value.insert("env_vars".to_owned(), json!(draft.env_vars));
            }
            if !draft.cwd.is_empty() {
                value.insert("cwd".to_owned(), Value::String(draft.cwd.clone()));
            }
        }
        McpTransportKind::StreamableHttp => {
            value.insert("url".to_owned(), Value::String(draft.url.clone()));
            if !draft.bearer_token_env_var.is_empty() {
                value.insert(
                    "bearer_token_env_var".to_owned(),
                    Value::String(draft.bearer_token_env_var.clone()),
                );
            }
            insert_mcp_record(&mut value, "http_headers", &draft.http_headers);
            insert_mcp_record(&mut value, "env_http_headers", &draft.env_http_headers);
        }
    }
    Value::Object(value)
}

fn insert_mcp_record(
    target: &mut serde_json::Map<String, Value>,
    name: &str,
    entries: &[(String, String)],
) {
    let mut record = serde_json::Map::new();
    for (key, value) in entries {
        record.insert(key.clone(), Value::String(value.clone()));
    }
    if !record.is_empty() {
        target.insert(name.to_owned(), Value::Object(record));
    }
}

const MAX_TASK_SEARCH_SNIPPET_BYTES: usize = 16 * 1024;
const GOAL_CONTINUATION_DELAY: Duration = Duration::from_millis(250);
const MAX_PENDING_GOAL_CONTINUATIONS: usize = 256;
const PINNED_TASK_IDS_PREFERENCE: &str = "pinned_task_ids_v1";
const APPEARANCE_THEME_PREFERENCE: &str = "appearance_theme";
const APPEARANCE_PREFERENCES_V1: &str = "appearance_preferences_v1";
const GIT_PREFERENCES_V1: &str = "git_preferences_v1";
const BROWSER_DOWNLOAD_PREFERENCES_V1: &str = "browser_download_preferences_v1";
const BROWSER_PERMISSIONS_V1: &str = "browser_permissions_v1";
const KEYBOARD_SHORTCUT_PREFERENCES_V1: &str = "keyboard_shortcut_preferences_v1";
const MAX_BROWSER_DOWNLOAD_PREFERENCES_BYTES: usize = 16 * 1024;
const MAX_BROWSER_PERMISSIONS_BYTES: usize = 128 * 1024;
const MAX_KEYBOARD_SHORTCUT_PREFERENCES_BYTES: usize = 32 * 1024;
const MAX_GIT_PREFERENCES_BYTES: usize = MAX_GIT_BRANCH_PREFIX_BYTES
    + MAX_GIT_INSTRUCTIONS_BYTES * 2
    + MAX_WORKTREE_ROOT_BYTES
    + 4 * 1024;
const PRIMARY_WINDOW_PLACEMENT_V1: &str = "primary_window_placement_v1";
const MAX_PRIMARY_WINDOW_PLACEMENT_BYTES: usize = 1_024;
const TERMINAL_BOTTOM_HEIGHT_PREFERENCE: &str = "terminal_bottom_height";
const TERMINAL_RIGHT_WIDTH_PREFERENCE: &str = "terminal_right_width";
const GIT_INCLUDE_UNSTAGED_PREFERENCE: &str = "git_action_include_unstaged_changes";

enum BackendCommand {
    Run(Box<Effect>),
    Shutdown,
}

enum PendingApproval {
    Command {
        id: Value,
        proposed_execpolicy_amendment: Option<Vec<String>>,
        proposed_network_policy_amendment: Option<CoreNetworkPolicyAmendment>,
    },
    FileChange {
        id: Value,
    },
    Permissions {
        id: Value,
        permissions: PermissionProfile,
    },
    McpElicitation {
        id: Value,
    },
    UserInput {
        id: Value,
    },
    ComputerUse {
        id: Value,
        params: DynamicToolCallParams,
        window_id: String,
        application_id: String,
        application_name: String,
    },
    ComputerUseLaunch {
        id: Value,
        params: DynamicToolCallParams,
        application_id: String,
        application_name: String,
    },
}

struct StartTurnRequest {
    task_id: String,
    submission: RetryableTurnSubmission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PersonalizationSnapshot {
    personality: Personality,
    memory_available: bool,
    generate_memories: bool,
    use_memories: bool,
    memories_enabled: bool,
    allow_memory_generation_from_tool_assisted_chats: bool,
}

fn personalization_snapshot(config: &ConfigReadResponse) -> PersonalizationSnapshot {
    let personality = Personality::from_config(
        config
            .config
            .personality
            .as_deref()
            .or(config.config.model_personality.as_deref()),
    );
    let memories = config.config.memories.as_ref();
    let generate_memories = memories
        .and_then(|memories| memories.generate_memories)
        .unwrap_or(true);
    let use_memories = memories
        .and_then(|memories| memories.use_memories)
        .unwrap_or(true);
    let disable_on_external_context = memories
        .and_then(|memories| {
            memories
                .disable_on_external_context
                .or(memories.no_memories_if_mcp_or_web_search)
        })
        .unwrap_or(false);
    PersonalizationSnapshot {
        personality,
        memory_available: config.config.features.memories.unwrap_or(false),
        generate_memories,
        use_memories,
        memories_enabled: generate_memories && use_memories,
        allow_memory_generation_from_tool_assisted_chats: !disable_on_external_context,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentConfigurationSnapshot {
    scopes: Vec<AgentConfigScope>,
    effective_approval_policy: String,
    effective_sandbox_mode: String,
    effective_network_access: bool,
    allowed_approval_policies: Vec<String>,
    allowed_sandbox_modes: Vec<String>,
    approval_managed: bool,
    sandbox_managed: bool,
    network_managed: bool,
}

fn agent_configuration_snapshot(
    config: &ConfigReadResponse,
    requirements: Option<&codex_protocol::ConfigRequirements>,
) -> AgentConfigurationSnapshot {
    let effective_approval_policy = config
        .config
        .approval_policy
        .as_ref()
        .and_then(Value::as_str)
        .filter(|value| matches!(*value, "untrusted" | "on-request" | "never" | "on-failure"))
        .unwrap_or("on-request")
        .to_owned();
    let effective_sandbox_mode = config
        .config
        .sandbox_mode
        .as_deref()
        .filter(|value| {
            matches!(
                *value,
                "read-only" | "workspace-write" | "danger-full-access"
            )
        })
        .unwrap_or("read-only")
        .to_owned();
    let effective_network_access = config
        .config
        .sandbox_workspace_write
        .as_ref()
        .and_then(|workspace| workspace.network_access)
        .unwrap_or(false);

    let mut scopes = Vec::new();
    let layers = config.layers.as_deref().unwrap_or_default();
    for layer in layers {
        if config_layer_type(&layer.name) != Some("project") {
            continue;
        }
        let Some(dot_codex_folder) = config_layer_path(&layer.name, "dotCodexFolder") else {
            continue;
        };
        let workspace_root = dot_codex_folder
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| dot_codex_folder.clone());
        let label = workspace_root
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
            .map(|name| bounded(name.to_owned(), 512))
            .unwrap_or_else(|| bounded(workspace_root.display().to_string(), 512));
        let file_path = dot_codex_folder.join("config.toml");
        let (approval_policy, sandbox_mode, network_access) = config_layer_values(&layer.config);
        scopes.push(AgentConfigScope {
            id: format!(
                "project:{}",
                bounded(file_path.display().to_string(), MAX_CONFIG_PATH_BYTES)
            ),
            kind: AgentConfigScopeKind::Project,
            label,
            tooltip: bounded(file_path.display().to_string(), MAX_CONFIG_PATH_BYTES),
            file_path: Some(file_path),
            expected_version: Some(bounded(layer.version.clone(), MAX_CONFIG_VERSION_BYTES)),
            approval_policy,
            sandbox_mode,
            network_access,
            disabled_reason: layer
                .disabled_reason
                .as_ref()
                .map(|reason| bounded(reason.clone(), MAX_STATUS_BYTES)),
        });
    }

    let user_layer = layers
        .iter()
        .find(|layer| config_layer_type(&layer.name) == Some("user"));
    let user_file_path = user_layer.and_then(|layer| config_layer_path(&layer.name, "file"));
    let (approval_policy, sandbox_mode, network_access) = user_layer
        .map(|layer| config_layer_values(&layer.config))
        .unwrap_or((None, None, None));
    scopes.push(AgentConfigScope {
        id: "user".to_owned(),
        kind: AgentConfigScopeKind::User,
        label: "User config".to_owned(),
        tooltip: user_file_path
            .as_ref()
            .map(|path| bounded(path.display().to_string(), MAX_CONFIG_PATH_BYTES))
            .unwrap_or_else(|| "~/.codex/config.toml".to_owned()),
        file_path: user_file_path,
        expected_version: user_layer
            .map(|layer| bounded(layer.version.clone(), MAX_CONFIG_VERSION_BYTES)),
        approval_policy,
        sandbox_mode,
        network_access,
        disabled_reason: None,
    });

    if let Some(layer) = layers
        .iter()
        .find(|layer| config_layer_is_managed(&layer.name))
    {
        let (approval_policy, sandbox_mode, network_access) = config_layer_values(&layer.config);
        let file_path = config_layer_path(&layer.name, "file");
        scopes.push(AgentConfigScope {
            id: "managed".to_owned(),
            kind: AgentConfigScopeKind::Managed,
            label: "Admin config".to_owned(),
            tooltip: file_path
                .as_ref()
                .map(|path| bounded(path.display().to_string(), MAX_CONFIG_PATH_BYTES))
                .unwrap_or_else(|| "Managed by admin policy".to_owned()),
            file_path,
            expected_version: Some(bounded(layer.version.clone(), MAX_CONFIG_VERSION_BYTES)),
            approval_policy,
            sandbox_mode,
            network_access,
            disabled_reason: Some("Managed by admin policy".to_owned()),
        });
    }

    let allowed_approval_policies = requirements
        .and_then(|requirements| requirements.allowed_approval_policies.as_ref())
        .filter(|values| !values.is_empty())
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .filter(|value| matches!(*value, "untrusted" | "on-request" | "never"))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|values| !values.is_empty())
        .unwrap_or_else(|| {
            vec![
                "untrusted".to_owned(),
                "on-request".to_owned(),
                "never".to_owned(),
            ]
        });
    let allowed_sandbox_modes = requirements
        .and_then(|requirements| requirements.allowed_sandbox_modes.as_ref())
        .filter(|values| !values.is_empty())
        .map(|values| {
            values
                .iter()
                .filter(|value| {
                    matches!(
                        value.as_str(),
                        "read-only" | "workspace-write" | "danger-full-access"
                    )
                })
                .cloned()
                .collect::<Vec<_>>()
        })
        .filter(|values| !values.is_empty())
        .unwrap_or_else(|| {
            vec![
                "read-only".to_owned(),
                "workspace-write".to_owned(),
                "danger-full-access".to_owned(),
            ]
        });

    AgentConfigurationSnapshot {
        scopes,
        effective_approval_policy,
        effective_sandbox_mode,
        effective_network_access,
        allowed_approval_policies,
        allowed_sandbox_modes,
        approval_managed: config_origin_is_managed(config, "approval_policy"),
        sandbox_managed: config_origin_is_managed(config, "sandbox_mode"),
        network_managed: config_origin_is_managed(config, "sandbox_workspace_write")
            || config_origin_is_managed(config, "sandbox_workspace_write.network_access"),
    }
}

fn config_layer_type(name: &Value) -> Option<&str> {
    name.get("type").and_then(Value::as_str)
}

fn config_layer_path(name: &Value, field: &str) -> Option<PathBuf> {
    let value = name.get(field)?.as_str()?.trim();
    if value.is_empty() || value.len() > MAX_CONFIG_PATH_BYTES {
        return None;
    }
    let path = PathBuf::from(value);
    path.is_absolute().then_some(path)
}

fn config_layer_is_managed(name: &Value) -> bool {
    matches!(
        config_layer_type(name),
        Some(
            "mdm"
                | "system"
                | "enterpriseManaged"
                | "legacyManagedConfigTomlFromFile"
                | "legacyManagedConfigTomlFromMdm"
        )
    )
}

fn config_layer_values(config: &Value) -> (Option<String>, Option<String>, Option<bool>) {
    let approval_policy = config
        .get("approval_policy")
        .and_then(Value::as_str)
        .filter(|value| matches!(*value, "untrusted" | "on-request" | "never" | "on-failure"))
        .map(str::to_owned);
    let sandbox_mode = config
        .get("sandbox_mode")
        .and_then(Value::as_str)
        .filter(|value| {
            matches!(
                *value,
                "read-only" | "workspace-write" | "danger-full-access"
            )
        })
        .map(str::to_owned);
    let network_access = config
        .get("sandbox_workspace_write")
        .and_then(Value::as_object)
        .and_then(|workspace| workspace.get("network_access"))
        .and_then(Value::as_bool);
    (approval_policy, sandbox_mode, network_access)
}

fn config_origin_is_managed(config: &ConfigReadResponse, key: &str) -> bool {
    config
        .origins
        .get(key)
        .is_some_and(|origin| config_layer_is_managed(&origin.name))
}

fn write_agent_config_value(
    app_server: &AppServerConnection,
    key_path: &str,
    value: Value,
    file_path: Option<PathBuf>,
    expected_version: Option<String>,
) -> Result<ConfigWriteStatus, String> {
    if file_path.as_ref().is_some_and(|path| !path.is_absolute()) {
        return Err("configuration target must be an absolute path".to_owned());
    }
    app_server
        .batch_write_config(ConfigBatchWriteParams {
            edits: vec![ConfigEdit {
                key_path: key_path.to_owned(),
                value,
                merge_strategy: ConfigMergeStrategy::Upsert,
            }],
            file_path: file_path
                .map(|path| bounded(path.display().to_string(), MAX_CONFIG_PATH_BYTES)),
            expected_version: expected_version
                .map(|version| bounded(version, MAX_CONFIG_VERSION_BYTES)),
            reload_user_config: true,
        })
        .map(|response| response.status)
        .map_err(|error| error.to_string())
}

fn emit_agent_configuration_mutation(
    events: &Sender<Action>,
    kind: AgentConfigurationMutationKind,
    result: Result<ConfigWriteStatus, String>,
) {
    match result {
        Ok(status) => emit(
            events,
            Action::AgentConfigurationMutationFinished {
                kind,
                overridden: status == ConfigWriteStatus::OkOverridden,
            },
        ),
        Err(message) => emit(
            events,
            Action::AgentConfigurationMutationFailed { kind, message },
        ),
    }
}

#[derive(Debug, Default)]
struct GitRefreshDebouncer {
    pending: Option<(u64, PathBuf, Instant)>,
}

impl GitRefreshDebouncer {
    fn schedule(&mut self, generation: u64, cwd: PathBuf, now: Instant, delay: Duration) {
        self.pending = Some((generation, cwd, now + delay));
    }

    fn take_due(&mut self, now: Instant) -> Option<(u64, PathBuf)> {
        if self
            .pending
            .as_ref()
            .is_some_and(|(_, _, deadline)| now >= *deadline)
        {
            return self
                .pending
                .take()
                .map(|(generation, cwd, _)| (generation, cwd));
        }
        None
    }
}

#[derive(Debug)]
struct AppServerReconnectScheduler {
    pending: Option<(u32, Instant)>,
    next_attempt: u32,
    next_delay: Duration,
}

impl Default for AppServerReconnectScheduler {
    fn default() -> Self {
        Self {
            pending: None,
            next_attempt: 1,
            next_delay: APP_SERVER_RECONNECT_INITIAL_DELAY,
        }
    }
}

impl AppServerReconnectScheduler {
    fn schedule(&mut self, now: Instant) -> Option<(u32, Duration)> {
        if self.pending.is_some() {
            return None;
        }
        let attempt = self.next_attempt;
        let delay = self.next_delay;
        self.pending = Some((attempt, now + delay));
        self.next_attempt = self.next_attempt.saturating_add(1);
        self.next_delay = self
            .next_delay
            .saturating_mul(2)
            .min(APP_SERVER_RECONNECT_MAX_DELAY);
        Some((attempt, delay))
    }

    fn take_due(&mut self, now: Instant) -> Option<u32> {
        if self
            .pending
            .as_ref()
            .is_some_and(|(_, deadline)| now >= *deadline)
        {
            return self.pending.take().map(|(attempt, _)| attempt);
        }
        None
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Default)]
struct TaskSearchDebouncer {
    pending: Option<(u64, String, Instant)>,
}

impl TaskSearchDebouncer {
    fn schedule(&mut self, generation: u64, query: String, now: Instant, delay: Duration) {
        self.pending = Some((generation, query, now + delay));
    }

    fn take_due(&mut self, now: Instant) -> Option<(u64, String)> {
        if self
            .pending
            .as_ref()
            .is_some_and(|(_, _, deadline)| now >= *deadline)
        {
            return self
                .pending
                .take()
                .map(|(generation, query, _)| (generation, query));
        }
        None
    }

    fn clear(&mut self) {
        self.pending = None;
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum FuzzyFileSearchSupport {
    #[default]
    Unknown,
    Supported,
    Unsupported,
}

#[derive(Debug, Default)]
struct FuzzyFileSearchRuntime {
    support: FuzzyFileSearchSupport,
    session_id: Option<String>,
    roots: Vec<PathBuf>,
}

impl FuzzyFileSearchRuntime {
    fn clear_session(&mut self) {
        self.session_id = None;
        self.roots.clear();
    }

    fn reset(&mut self) {
        self.support = FuzzyFileSearchSupport::Unknown;
        self.clear_session();
    }
}

#[derive(Debug, Default)]
struct PullRequestSearchDebouncer {
    pending: Option<(u64, Instant)>,
}

impl PullRequestSearchDebouncer {
    fn schedule(&mut self, generation: u64, now: Instant, delay: Duration) {
        self.pending = Some((generation, now + delay));
    }

    fn take_due(&mut self, now: Instant) -> Option<u64> {
        if self
            .pending
            .as_ref()
            .is_some_and(|(_, deadline)| now >= *deadline)
        {
            return self.pending.take().map(|(generation, _)| generation);
        }
        None
    }

    fn clear(&mut self) {
        self.pending = None;
    }
}

#[derive(Debug, Default)]
struct GoalContinuationScheduler {
    pending: HashMap<String, Instant>,
}

impl GoalContinuationScheduler {
    fn schedule(
        &mut self,
        task_id: String,
        now: Instant,
        delay: Duration,
    ) -> Result<(), &'static str> {
        if self.pending.contains_key(&task_id) {
            return Ok(());
        }
        if self.pending.len() >= MAX_PENDING_GOAL_CONTINUATIONS {
            return Err("goal continuation queue is full");
        }
        self.pending.insert(task_id, now + delay);
        Ok(())
    }

    fn take_due(&mut self, now: Instant) -> Vec<String> {
        let mut due = self
            .pending
            .iter()
            .filter(|(_, deadline)| now >= **deadline)
            .map(|(task_id, deadline)| (task_id.clone(), *deadline))
            .collect::<Vec<_>>();
        due.sort_by_key(|(_, deadline)| *deadline);
        for (task_id, _) in &due {
            self.pending.remove(task_id);
        }
        due.into_iter().map(|(task_id, _)| task_id).collect()
    }

    fn clear(&mut self) {
        self.pending.clear();
    }
}

#[derive(Debug, Clone, Default)]
struct ComputerUsePermission {
    enabled: bool,
    authorized_application_id: Option<String>,
    input_authorized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserPolicyTarget {
    NotBrowser,
    Supported,
    Unsupported,
}

struct ComputerUseUrlPolicy {
    client: Option<reqwest::blocking::Client>,
    blocked_turns: HashMap<(String, String), &'static str>,
}

impl ComputerUseUrlPolicy {
    fn new() -> Self {
        let client = reqwest::blocking::Client::builder()
            .use_rustls_tls()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .user_agent("codex-computer-use")
            .build()
            .ok();
        Self {
            client,
            blocked_turns: HashMap::new(),
        }
    }

    fn blocked_message(&self, thread_id: &str, turn_id: &str) -> Option<&'static str> {
        self.blocked_turns
            .get(&(thread_id.to_owned(), turn_id.to_owned()))
            .copied()
    }

    fn complete_turn(&mut self, thread_id: &str, turn_id: &str) {
        self.blocked_turns
            .remove(&(thread_id.to_owned(), turn_id.to_owned()));
    }

    fn clear(&mut self) {
        self.blocked_turns.clear();
    }

    fn enforce_and_block(
        &mut self,
        app_server: &AppServerConnection,
        computer_accessibility: &mut ComputerUseAccessibilityClient,
        params: &DynamicToolCallParams,
        window: &ComputerWindow,
    ) -> Result<(), &'static str> {
        let result = self.enforce(app_server, computer_accessibility, window);
        if let Err(message) = result {
            if self.blocked_turns.len() >= MAX_BLOCKED_COMPUTER_TURNS {
                self.blocked_turns.clear();
            }
            self.blocked_turns
                .insert((params.thread_id.clone(), params.turn_id.clone()), message);
        }
        result
    }

    fn enforce(
        &self,
        app_server: &AppServerConnection,
        computer_accessibility: &mut ComputerUseAccessibilityClient,
        window: &ComputerWindow,
    ) -> Result<(), &'static str> {
        let target = browser_policy_target(&window.application_id);
        if target == BrowserPolicyTarget::NotBrowser {
            return Ok(());
        }
        if target == BrowserPolicyTarget::Unsupported {
            return Err(COMPUTER_USE_URL_UNSUPPORTED_BROWSER_MESSAGE);
        }

        let first_url = computer_accessibility
            .browser_url(&window.id)
            .map_err(|_| COMPUTER_USE_URL_CONFIDENCE_FAILED_MESSAGE)?;
        let first_url =
            url::Url::parse(&first_url).map_err(|_| COMPUTER_USE_URL_CONFIDENCE_FAILED_MESSAGE)?;
        if !matches!(first_url.scheme(), "http" | "https") {
            return Ok(());
        }

        let blocked = self.site_status(app_server, &first_url)?;
        let second_url = computer_accessibility
            .browser_url(&window.id)
            .map_err(|_| COMPUTER_USE_URL_CONFIDENCE_FAILED_MESSAGE)?;
        let second_url =
            url::Url::parse(&second_url).map_err(|_| COMPUTER_USE_URL_CONFIDENCE_FAILED_MESSAGE)?;
        if first_url != second_url {
            return Err(COMPUTER_USE_URL_CONFIDENCE_FAILED_MESSAGE);
        }
        if blocked {
            Err(COMPUTER_USE_URL_FORBIDDEN_MESSAGE)
        } else {
            Ok(())
        }
    }

    fn site_status(
        &self,
        app_server: &AppServerConnection,
        site_url: &url::Url,
    ) -> Result<bool, &'static str> {
        let first_auth = app_server
            .get_auth_status(GetAuthStatusParams {
                include_token: true,
                refresh_token: false,
            })
            .map_err(|_| COMPUTER_USE_URL_VERIFICATION_FAILED_MESSAGE)?;
        match self.request_site_status(&first_auth, site_url)? {
            Some(blocked) => Ok(blocked),
            None => {
                let refreshed_auth = app_server
                    .get_auth_status(GetAuthStatusParams {
                        include_token: true,
                        refresh_token: true,
                    })
                    .map_err(|_| COMPUTER_USE_URL_VERIFICATION_FAILED_MESSAGE)?;
                self.request_site_status(&refreshed_auth, site_url)?
                    .ok_or(COMPUTER_USE_URL_VERIFICATION_FAILED_MESSAGE)
            }
        }
    }

    fn request_site_status(
        &self,
        auth: &codex_protocol::GetAuthStatusResponse,
        site_url: &url::Url,
    ) -> Result<Option<bool>, &'static str> {
        let client = self
            .client
            .as_ref()
            .ok_or(COMPUTER_USE_URL_VERIFICATION_FAILED_MESSAGE)?;
        if !matches!(
            auth.auth_method.as_deref(),
            Some("chatgpt" | "chatgptAuthTokens")
        ) {
            return Err(COMPUTER_USE_URL_VERIFICATION_FAILED_MESSAGE);
        }
        let token = auth
            .auth_token
            .as_ref()
            .map(|token| token.expose())
            .filter(|token| !token.is_empty())
            .ok_or(COMPUTER_USE_URL_VERIFICATION_FAILED_MESSAGE)?;

        let mut endpoint = url::Url::parse(COMPUTER_USE_SITE_STATUS_ENDPOINT)
            .map_err(|_| COMPUTER_USE_URL_VERIFICATION_FAILED_MESSAGE)?;
        endpoint
            .query_pairs_mut()
            .append_pair("site_url", site_url.as_str())
            .append_pair("url_request_source", COMPUTER_USE_SITE_STATUS_SOURCE);
        let mut bearer_bytes = Vec::with_capacity(7 + token.len());
        bearer_bytes.extend_from_slice(b"Bearer ");
        bearer_bytes.extend_from_slice(token.as_bytes());
        let authorization = reqwest::header::HeaderValue::from_bytes(&bearer_bytes);
        bearer_bytes.fill(0);
        let mut authorization =
            authorization.map_err(|_| COMPUTER_USE_URL_VERIFICATION_FAILED_MESSAGE)?;
        authorization.set_sensitive(true);
        let mut request = client
            .get(endpoint)
            .header(reqwest::header::AUTHORIZATION, authorization)
            .header("originator", COMPUTER_USE_SITE_STATUS_SOURCE)
            .header(reqwest::header::ACCEPT, "application/json");
        if let Some(account_id) = auth.account_id.as_deref().filter(|value| !value.is_empty()) {
            let mut account_header = reqwest::header::HeaderValue::from_str(account_id)
                .map_err(|_| COMPUTER_USE_URL_VERIFICATION_FAILED_MESSAGE)?;
            account_header.set_sensitive(true);
            request = request.header("ChatGPT-Account-ID", account_header);
        }
        let response = request
            .send()
            .map_err(|_| COMPUTER_USE_URL_VERIFICATION_FAILED_MESSAGE)?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Ok(None);
        }
        if !response.status().is_success()
            || response
                .content_length()
                .is_some_and(|length| length > MAX_SITE_STATUS_RESPONSE_BYTES as u64)
        {
            return Err(COMPUTER_USE_URL_VERIFICATION_FAILED_MESSAGE);
        }
        let mut body = Vec::with_capacity(4 * 1024);
        response
            .take((MAX_SITE_STATUS_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut body)
            .map_err(|_| COMPUTER_USE_URL_VERIFICATION_FAILED_MESSAGE)?;
        if body.len() > MAX_SITE_STATUS_RESPONSE_BYTES {
            return Err(COMPUTER_USE_URL_VERIFICATION_FAILED_MESSAGE);
        }
        let value = serde_json::from_slice::<Value>(&body)
            .map_err(|_| COMPUTER_USE_URL_VERIFICATION_FAILED_MESSAGE)?;
        value
            .pointer("/feature_status/agent")
            .and_then(Value::as_bool)
            .map(Some)
            .ok_or(COMPUTER_USE_URL_VERIFICATION_FAILED_MESSAGE)
    }
}

fn browser_policy_target(application_id: &str) -> BrowserPolicyTarget {
    let executable = application_id
        .trim()
        .rsplit(['\\', '/', '!'])
        .find(|segment| !segment.is_empty())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let executable = executable.strip_suffix(".exe").unwrap_or(&executable);
    match executable {
        "msedge" | "chrome" | "brave" | "opera" | "iexplore" | "firefox" => {
            BrowserPolicyTarget::Supported
        }
        "browser" => BrowserPolicyTarget::Unsupported,
        _ => BrowserPolicyTarget::NotBrowser,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ComputerToolRequestMeta {
    id: Value,
    thread_id: String,
    turn_id: String,
    tool: String,
    window_id: Option<String>,
}

#[derive(Default)]
struct TerminalParserCallbacks {
    title: String,
}

impl vt100::Callbacks for TerminalParserCallbacks {
    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        self.title.clear();
        for character in String::from_utf8_lossy(title).chars() {
            if character.is_control() {
                continue;
            }
            if self.title.len() + character.len_utf8() > MAX_TERMINAL_TITLE_BYTES {
                break;
            }
            self.title.push(character);
        }
    }
}

struct TerminalRuntime {
    session: TerminalSession,
    parser: vt100::Parser<TerminalParserCallbacks>,
    truncation_reported: bool,
    reported_title: String,
}

struct BrowserRuntime {
    session: BrowserSession,
    contexts: HashSet<String>,
    executable: Option<PathBuf>,
}

struct PendingWorktreeRuntime {
    request_id: u64,
    cancellation: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

pub struct Backend {
    commands: Sender<BackendCommand>,
    events: Receiver<Action>,
    thread: Option<JoinHandle<()>>,
}

impl Backend {
    pub fn load_primary_window_placement() -> Result<Option<PrimaryWindowPlacement>, String> {
        let path = codexrs_data_dir()
            .map_err(|error| error.to_string())?
            .join("state.sqlite3");
        let store = Store::open(&path).map_err(|error| error.to_string())?;
        let value = store
            .preference(PRIMARY_WINDOW_PLACEMENT_V1)
            .map_err(|error| error.to_string())?;
        Ok(value.as_deref().and_then(parse_primary_window_placement))
    }

    pub fn spawn() -> Result<Self, String> {
        let (command_sender, command_receiver) =
            crossbeam_channel::bounded(BACKEND_COMMAND_CAPACITY);
        let (event_sender, event_receiver) = crossbeam_channel::bounded(BACKEND_EVENT_CAPACITY);
        let thread = thread::Builder::new()
            .name("codex-rs-backend".to_owned())
            .spawn(move || run_backend(command_receiver, event_sender))
            .map_err(|error| format!("failed to start backend: {error}"))?;

        Ok(Self {
            commands: command_sender,
            events: event_receiver,
            thread: Some(thread),
        })
    }

    pub fn send(&self, effect: Effect) -> Result<(), &'static str> {
        self.commands
            .try_send(BackendCommand::Run(Box::new(effect)))
            .map_err(|error| match error {
                crossbeam_channel::TrySendError::Full(_) => "backend command queue is full",
                crossbeam_channel::TrySendError::Disconnected(_) => "backend is disconnected",
            })
    }

    pub fn try_recv(&self) -> Result<Option<Action>, &'static str> {
        match self.events.try_recv() {
            Ok(action) => Ok(Some(action)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err("backend is disconnected"),
        }
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        let _ = self.commands.try_send(BackendCommand::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn open_storage(events: &Sender<Action>) -> Option<Store> {
    let path = match codexrs_data_dir() {
        Ok(directory) => directory.join("state.sqlite3"),
        Err(error) => {
            emit(events, Action::StorageFailed(error.to_string()));
            return None;
        }
    };
    match Store::open(&path).and_then(|store| {
        let route = store.preference("route")?;
        let inspector = store.preference("inspector")?;
        let appearance_theme = store.preference(APPEARANCE_THEME_PREFERENCE)?;
        let appearance_preferences = store.preference(APPEARANCE_PREFERENCES_V1)?;
        let git_preferences = store.preference(GIT_PREFERENCES_V1)?;
        let browser_download_preferences = store.preference(BROWSER_DOWNLOAD_PREFERENCES_V1)?;
        let browser_permissions = store.preference(BROWSER_PERMISSIONS_V1)?;
        let keyboard_shortcut_preferences = store.preference(KEYBOARD_SHORTCUT_PREFERENCES_V1)?;
        let terminal_location = store.preference("terminal_location")?;
        let terminal_bottom_height = store.preference(TERMINAL_BOTTOM_HEIGHT_PREFERENCE)?;
        let terminal_right_width = store.preference(TERMINAL_RIGHT_WIDTH_PREFERENCE)?;
        let git_include_unstaged = store.preference(GIT_INCLUDE_UNSTAGED_PREFERENCE)?;
        let pinned_task_ids = store
            .preference(PINNED_TASK_IDS_PREFERENCE)?
            .and_then(|value| serde_json::from_str::<Vec<String>>(&value).ok())
            .unwrap_or_default();
        let recent_workspaces = store.recent_workspaces(MAX_LOCAL_PROJECTS, 0)?.items;
        let recent_workspace = recent_workspaces
            .iter()
            .map(|workspace| workspace.path.clone())
            .find(|path| path.is_absolute() && path.is_dir());
        let local_projects = recent_workspaces
            .into_iter()
            .map(|workspace| LocalProjectSummary {
                path: workspace.path,
                name: workspace.name.unwrap_or_default(),
                pinned: workspace.pinned,
                last_opened_at: workspace.last_opened_at,
            })
            .collect::<Vec<_>>();
        Ok((
            store,
            route,
            inspector,
            appearance_theme,
            appearance_preferences,
            git_preferences,
            browser_download_preferences,
            browser_permissions,
            keyboard_shortcut_preferences,
            terminal_location,
            terminal_bottom_height,
            terminal_right_width,
            git_include_unstaged,
            pinned_task_ids,
            recent_workspace,
            local_projects,
        ))
    }) {
        Ok((
            store,
            route,
            inspector,
            appearance_theme,
            appearance_preferences,
            git_preferences,
            browser_download_preferences,
            browser_permissions,
            keyboard_shortcut_preferences,
            terminal_location,
            terminal_bottom_height,
            terminal_right_width,
            git_include_unstaged,
            pinned_task_ids,
            recent_workspace,
            local_projects,
        )) => {
            emit(
                events,
                Action::StorageOpened {
                    path,
                    route: route.as_deref().and_then(parse_route),
                    inspector: inspector.as_deref().and_then(parse_inspector),
                    appearance_theme: appearance_theme.as_deref().and_then(parse_appearance_theme),
                    terminal_location: terminal_location
                        .as_deref()
                        .and_then(parse_terminal_location),
                    terminal_bottom_height: terminal_bottom_height
                        .as_deref()
                        .and_then(parse_terminal_size),
                    terminal_right_width: terminal_right_width
                        .as_deref()
                        .and_then(parse_terminal_size),
                    git_include_unstaged: git_include_unstaged
                        .as_deref()
                        .and_then(parse_bool_preference),
                    pinned_task_ids,
                    recent_workspace,
                },
            );
            emit(events, Action::LocalProjectsLoaded(local_projects));
            if let Some(preferences) = appearance_preferences
                .as_deref()
                .and_then(parse_appearance_preferences)
            {
                emit(events, Action::AppearancePreferencesLoaded(preferences));
            }
            if let Some(preferences) = git_preferences.as_deref().and_then(parse_git_preferences) {
                emit(events, Action::GitPreferencesLoaded(preferences));
            }
            if let Some(preferences) = browser_download_preferences
                .as_deref()
                .and_then(parse_browser_download_preferences)
            {
                emit(
                    events,
                    Action::BrowserDownloadPreferencesLoaded(preferences),
                );
            }
            if let Some(permissions) = browser_permissions
                .as_deref()
                .and_then(parse_browser_permissions)
            {
                emit(events, Action::BrowserPermissionsLoaded(permissions));
            }
            if let Some(preferences) = keyboard_shortcut_preferences
                .as_deref()
                .and_then(parse_keyboard_shortcut_preferences)
            {
                emit(
                    events,
                    Action::KeyboardShortcutPreferencesLoaded(preferences),
                );
            }
            Some(store)
        }
        Err(error) => {
            emit(events, Action::StorageFailed(error.to_string()));
            None
        }
    }
}

const fn route_key(route: MainRoute) -> &'static str {
    match route {
        MainRoute::Tasks => "tasks",
        MainRoute::Repository => "repository",
        MainRoute::PullRequests => "pull-requests",
        MainRoute::Marketplace => "marketplace",
        MainRoute::Settings => "settings",
    }
}

fn parse_route(value: &str) -> Option<MainRoute> {
    match value {
        "tasks" => Some(MainRoute::Tasks),
        "repository" => Some(MainRoute::Repository),
        "pull-requests" => Some(MainRoute::PullRequests),
        "marketplace" => Some(MainRoute::Marketplace),
        "settings" => Some(MainRoute::Settings),
        _ => None,
    }
}

const fn inspector_key(inspector: InspectorPane) -> &'static str {
    match inspector {
        InspectorPane::Hidden => "hidden",
        InspectorPane::Changes => "changes",
        InspectorPane::Outputs => "outputs",
        InspectorPane::Files => "hidden",
        InspectorPane::Terminal => "terminal",
        InspectorPane::ComputerUse => "computer-use",
        InspectorPane::Browser => "browser",
    }
}

fn parse_inspector(value: &str) -> Option<InspectorPane> {
    match value {
        "hidden" => Some(InspectorPane::Hidden),
        "changes" => Some(InspectorPane::Changes),
        "outputs" => Some(InspectorPane::Outputs),
        "terminal" => Some(InspectorPane::Terminal),
        "computer-use" => Some(InspectorPane::ComputerUse),
        "browser" => Some(InspectorPane::Browser),
        _ => None,
    }
}

const fn terminal_location_key(location: TerminalDockLocation) -> &'static str {
    match location {
        TerminalDockLocation::Bottom => "bottom",
        TerminalDockLocation::Right => "right",
    }
}

const fn appearance_theme_key(theme: AppearanceTheme) -> &'static str {
    match theme {
        AppearanceTheme::System => "system",
        AppearanceTheme::Light => "light",
        AppearanceTheme::Dark => "dark",
    }
}

fn parse_appearance_theme(value: &str) -> Option<AppearanceTheme> {
    match value {
        "system" => Some(AppearanceTheme::System),
        "light" => Some(AppearanceTheme::Light),
        "dark" => Some(AppearanceTheme::Dark),
        _ => None,
    }
}

fn encode_primary_window_placement(
    placement: PrimaryWindowPlacement,
) -> Result<String, serde_json::Error> {
    serde_json::to_string(&json!({
        "version": 1,
        "x": placement.x(),
        "y": placement.y(),
        "width": placement.width(),
        "height": placement.height(),
        "maximized": placement.is_maximized(),
    }))
}

fn parse_primary_window_placement(value: &str) -> Option<PrimaryWindowPlacement> {
    if value.len() > MAX_PRIMARY_WINDOW_PLACEMENT_BYTES {
        return None;
    }
    let value = serde_json::from_str::<Value>(value).ok()?;
    let object = value.as_object()?;
    if object.get("version")?.as_u64()? != 1 {
        return None;
    }
    PrimaryWindowPlacement::new(
        i32::try_from(object.get("x")?.as_i64()?).ok()?,
        i32::try_from(object.get("y")?.as_i64()?).ok()?,
        u32::try_from(object.get("width")?.as_u64()?).ok()?,
        u32::try_from(object.get("height")?.as_u64()?).ok()?,
        object.get("maximized")?.as_bool()?,
    )
}

fn encode_appearance_preferences(
    preferences: &AppearancePreferences,
) -> Result<String, serde_json::Error> {
    let preferences = preferences.clone().normalized();
    serde_json::to_string(&json!({
        "version": 1,
        "usePointerCursors": preferences.use_pointer_cursors,
        "reducedMotionPreference": reduced_motion_key(preferences.reduced_motion),
        "sansFontSize": preferences.ui_font_size,
        "codeFontSize": preferences.code_font_size,
        "diffMarkerStyle": diff_marker_style_key(preferences.diff_marker_style),
        "light": encode_appearance_palette(&preferences.light),
        "dark": encode_appearance_palette(&preferences.dark),
    }))
}

fn encode_appearance_palette(palette: &AppearancePalette) -> Value {
    json!({
        "accent": format_appearance_color(palette.accent),
        "contrast": palette.contrast,
        "fonts": {
            "code": palette.code_font,
            "ui": palette.ui_font,
        },
        "codeThemeId": palette.code_theme_id,
        "ink": format_appearance_color(palette.ink),
        "opaqueWindows": palette.opaque_windows,
        "semanticColors": {
            "diffAdded": format_appearance_color(palette.semantic_colors.diff_added),
            "diffRemoved": format_appearance_color(palette.semantic_colors.diff_removed),
            "skill": format_appearance_color(palette.semantic_colors.skill),
        },
        "surface": format_appearance_color(palette.surface),
    })
}

fn parse_appearance_preferences(value: &str) -> Option<AppearancePreferences> {
    let value = serde_json::from_str::<Value>(value).ok()?;
    let object = value.as_object()?;
    if object.get("version")?.as_u64()? != 1 {
        return None;
    }
    let reduced_motion = match object.get("reducedMotionPreference")?.as_str()? {
        "system" => ReducedMotionPreference::System,
        "on" => ReducedMotionPreference::On,
        "off" => ReducedMotionPreference::Off,
        _ => return None,
    };
    let diff_marker_style = match object.get("diffMarkerStyle")?.as_str()? {
        "color" => DiffMarkerStyle::Color,
        "symbols" => DiffMarkerStyle::Symbols,
        _ => return None,
    };
    Some(
        AppearancePreferences {
            code_font_size: parse_appearance_size(object.get("codeFontSize")?, 8, 24)?,
            dark: parse_appearance_palette(object.get("dark")?, AppearanceVariant::Dark)?,
            diff_marker_style,
            light: parse_appearance_palette(object.get("light")?, AppearanceVariant::Light)?,
            reduced_motion,
            ui_font_size: parse_appearance_size(object.get("sansFontSize")?, 11, 16)?,
            use_pointer_cursors: object.get("usePointerCursors")?.as_bool()?,
        }
        .normalized(),
    )
}

fn encode_git_preferences(preferences: &GitPreferences) -> Result<String, serde_json::Error> {
    let preferences = preferences.clone().normalized();
    serde_json::to_string(&json!({
        "version": 1,
        "branchPrefix": preferences.branch_prefix,
        "alwaysForcePush": preferences.always_force_push,
        "createPullRequestAsDraft": preferences.create_pull_request_as_draft,
        "pullRequestMergeMethod": pull_request_merge_method_key(
            preferences.pull_request_merge_method
        ),
        "reviewMode": git_review_mode_key(preferences.review_mode),
        "reviewDelivery": review_delivery_key(preferences.review_delivery),
        "commitInstructions": preferences.commit_instructions,
        "pullRequestInstructions": preferences.pull_request_instructions,
        "worktreeRoot": preferences
            .worktree_root
            .as_ref()
            .map(|path| path.to_string_lossy()),
    }))
}

fn parse_git_preferences(value: &str) -> Option<GitPreferences> {
    if value.len() > MAX_GIT_PREFERENCES_BYTES {
        return None;
    }
    let value = serde_json::from_str::<Value>(value).ok()?;
    let object = value.as_object()?;
    if object.get("version")?.as_u64()? != 1 {
        return None;
    }
    let pull_request_merge_method = match object.get("pullRequestMergeMethod")?.as_str()? {
        "merge" => PullRequestMergeMethod::Merge,
        "squash" => PullRequestMergeMethod::Squash,
        _ => return None,
    };
    let review_mode = match object
        .get("reviewMode")
        .and_then(Value::as_str)
        .unwrap_or("full")
    {
        "full" => GitReviewMode::Full,
        "last-turn-only" => GitReviewMode::LastTurnOnly,
        _ => return None,
    };
    let review_delivery = match object
        .get("reviewDelivery")
        .and_then(Value::as_str)
        .unwrap_or("inline")
    {
        "inline" => CoreReviewDelivery::Inline,
        "detached" => CoreReviewDelivery::Detached,
        _ => return None,
    };
    Some(
        GitPreferences {
            branch_prefix: object.get("branchPrefix")?.as_str()?.to_owned(),
            always_force_push: object.get("alwaysForcePush")?.as_bool()?,
            create_pull_request_as_draft: object.get("createPullRequestAsDraft")?.as_bool()?,
            pull_request_merge_method,
            review_mode,
            review_delivery,
            commit_instructions: object.get("commitInstructions")?.as_str()?.to_owned(),
            pull_request_instructions: object.get("pullRequestInstructions")?.as_str()?.to_owned(),
            worktree_root: object
                .get("worktreeRoot")
                .and_then(Value::as_str)
                .map(PathBuf::from),
        }
        .normalized(),
    )
}

const fn git_review_mode_key(mode: GitReviewMode) -> &'static str {
    match mode {
        GitReviewMode::Full => "full",
        GitReviewMode::LastTurnOnly => "last-turn-only",
    }
}

const fn review_delivery_key(delivery: CoreReviewDelivery) -> &'static str {
    match delivery {
        CoreReviewDelivery::Inline => "inline",
        CoreReviewDelivery::Detached => "detached",
    }
}

const fn pull_request_merge_method_key(method: PullRequestMergeMethod) -> &'static str {
    match method {
        PullRequestMergeMethod::Merge => "merge",
        PullRequestMergeMethod::Squash => "squash",
    }
}

fn encode_browser_download_preferences(
    preferences: &BrowserDownloadPreferences,
) -> Result<String, serde_json::Error> {
    let preferences = preferences.clone().normalized();
    serde_json::to_string(&json!({
        "version": 1,
        "downloadDirectory": preferences
            .download_directory
            .as_ref()
            .and_then(|path| path.to_str()),
        "promptForUserDownloads": preferences.prompt_for_user_downloads,
    }))
}

fn parse_browser_download_preferences(value: &str) -> Option<BrowserDownloadPreferences> {
    if value.len() > MAX_BROWSER_DOWNLOAD_PREFERENCES_BYTES {
        return None;
    }
    let value = serde_json::from_str::<Value>(value).ok()?;
    let object = value.as_object()?;
    if object.get("version")?.as_u64()? != 1 {
        return None;
    }
    let download_directory = match object.get("downloadDirectory")? {
        Value::Null => None,
        Value::String(path)
            if !path.is_empty()
                && path.len() <= MAX_BROWSER_DOWNLOAD_PATH_BYTES
                && !path.contains('\0') =>
        {
            Some(PathBuf::from(path))
        }
        _ => return None,
    };
    Some(
        BrowserDownloadPreferences {
            download_directory,
            prompt_for_user_downloads: object.get("promptForUserDownloads")?.as_bool()?,
        }
        .normalized(),
    )
}

const fn browser_approval_mode_key(mode: BrowserApprovalMode) -> &'static str {
    match mode {
        BrowserApprovalMode::AlwaysAsk => "alwaysAsk",
        BrowserApprovalMode::NeverAsk => "neverAsk",
    }
}

fn parse_browser_approval_mode(value: &Value) -> Option<BrowserApprovalMode> {
    match value.as_str()? {
        "alwaysAsk" => Some(BrowserApprovalMode::AlwaysAsk),
        "neverAsk" => Some(BrowserApprovalMode::NeverAsk),
        _ => None,
    }
}

const fn browser_permission_value_key(value: BrowserPermissionValue) -> &'static str {
    match value {
        BrowserPermissionValue::Default => "default",
        BrowserPermissionValue::Allow => "allow",
        BrowserPermissionValue::Block => "block",
    }
}

fn parse_browser_permission_value(value: Option<&Value>) -> Option<BrowserPermissionValue> {
    match value.and_then(Value::as_str).unwrap_or("default") {
        "default" => Some(BrowserPermissionValue::Default),
        "allow" => Some(BrowserPermissionValue::Allow),
        "block" => Some(BrowserPermissionValue::Block),
        _ => None,
    }
}

fn encode_browser_permissions(
    permissions: &BrowserPermissionsState,
) -> Result<String, serde_json::Error> {
    let permissions = permissions.clone().normalized();
    serde_json::to_string(&json!({
        "version": 1,
        "approvalMode": browser_approval_mode_key(permissions.approval_mode),
        "downloadApprovalMode": browser_approval_mode_key(permissions.download_approval_mode),
        "uploadApprovalMode": browser_approval_mode_key(permissions.upload_approval_mode),
        "fullCdpAccessEnabled": permissions.full_cdp_access_enabled,
        "sites": permissions.sites.iter().map(|site| json!({
            "origin": site.origin,
            "browse": browser_permission_value_key(site.browse),
            "download": browser_permission_value_key(site.download),
            "upload": browser_permission_value_key(site.upload),
            "fullCdp": browser_permission_value_key(site.full_cdp),
        })).collect::<Vec<_>>(),
    }))
}

fn parse_browser_permissions(value: &str) -> Option<BrowserPermissionsState> {
    if value.len() > MAX_BROWSER_PERMISSIONS_BYTES {
        return None;
    }
    let value = serde_json::from_str::<Value>(value).ok()?;
    let object = value.as_object()?;
    if object.get("version")?.as_u64()? != 1 {
        return None;
    }
    let sites = object.get("sites")?.as_array()?;
    if sites.len() > MAX_BROWSER_SITE_PERMISSIONS {
        return None;
    }
    let mut parsed_sites = Vec::with_capacity(sites.len());
    for site in sites {
        let site = site.as_object()?;
        let origin = site.get("origin")?.as_str()?.trim();
        if origin.is_empty()
            || origin.len() > MAX_BROWSER_PERMISSION_ORIGIN_BYTES
            || origin.chars().any(char::is_control)
        {
            return None;
        }
        parsed_sites.push(BrowserSitePermission {
            origin: origin.to_owned(),
            browse: parse_browser_permission_value(site.get("browse"))?,
            download: parse_browser_permission_value(site.get("download"))?,
            upload: parse_browser_permission_value(site.get("upload"))?,
            full_cdp: parse_browser_permission_value(site.get("fullCdp"))?,
        });
    }
    Some(
        BrowserPermissionsState {
            approval_mode: parse_browser_approval_mode(object.get("approvalMode")?)?,
            download_approval_mode: parse_browser_approval_mode(
                object.get("downloadApprovalMode")?,
            )?,
            upload_approval_mode: parse_browser_approval_mode(object.get("uploadApprovalMode")?)?,
            full_cdp_access_enabled: object.get("fullCdpAccessEnabled")?.as_bool()?,
            sites: parsed_sites,
        }
        .normalized(),
    )
}

fn stored_browser_download(download: &BrowserDownloadState) -> Option<StoredBrowserDownload> {
    let status = match download.status {
        CoreBrowserDownloadStatus::Failed => BrowserDownloadRecordStatus::Failed,
        CoreBrowserDownloadStatus::Canceled => BrowserDownloadRecordStatus::Canceled,
        CoreBrowserDownloadStatus::Complete => BrowserDownloadRecordStatus::Complete,
        CoreBrowserDownloadStatus::Started
        | CoreBrowserDownloadStatus::InProgress
        | CoreBrowserDownloadStatus::Paused => return None,
    };
    Some(StoredBrowserDownload {
        context_id: download.context_id.clone(),
        filename: download.filename.clone(),
        id: download.id.clone(),
        path: download.path.clone(),
        received_bytes: download.received_bytes,
        started_at_ms: download.started_at_ms,
        status,
        total_bytes: download.total_bytes,
        updated_at_ms: download.updated_at_ms,
        user_initiated: download.user_initiated,
    })
}

fn restored_browser_download(download: StoredBrowserDownload) -> BrowserDownloadState {
    let status = match download.status {
        BrowserDownloadRecordStatus::Failed => CoreBrowserDownloadStatus::Failed,
        BrowserDownloadRecordStatus::Canceled => CoreBrowserDownloadStatus::Canceled,
        BrowserDownloadRecordStatus::Complete => CoreBrowserDownloadStatus::Complete,
    };
    BrowserDownloadState {
        can_cancel: false,
        can_pause: false,
        can_resume: false,
        context_id: download.context_id,
        file_exists: status == CoreBrowserDownloadStatus::Complete && download.path.is_file(),
        filename: download.filename,
        id: download.id,
        path: download.path,
        received_bytes: download.received_bytes,
        started_at_ms: download.started_at_ms,
        status,
        total_bytes: download.total_bytes,
        updated_at_ms: download.updated_at_ms,
        url: String::new(),
        user_initiated: download.user_initiated,
    }
}

fn encode_keyboard_shortcut_preferences(
    preferences: &KeyboardShortcutPreferences,
) -> Result<String, serde_json::Error> {
    let preferences = preferences.clone().normalized();
    let mut bindings = serde_json::Map::new();
    for command_id in KEYBOARD_SHORTCUT_COMMAND_IDS {
        let Some(accelerators) = preferences.bindings_for(command_id) else {
            continue;
        };
        bindings.insert(
            command_id.to_owned(),
            Value::Array(
                accelerators
                    .iter()
                    .map(|accelerator| Value::String(accelerator.clone()))
                    .collect(),
            ),
        );
    }
    serde_json::to_string(&json!({
        "version": 1,
        "bindings": bindings,
    }))
}

fn parse_keyboard_shortcut_preferences(value: &str) -> Option<KeyboardShortcutPreferences> {
    if value.len() > MAX_KEYBOARD_SHORTCUT_PREFERENCES_BYTES {
        return None;
    }
    let value = serde_json::from_str::<Value>(value).ok()?;
    let object = value.as_object()?;
    if object.get("version")?.as_u64()? != 1 {
        return None;
    }
    let bindings = object.get("bindings")?.as_object()?;
    let mut overrides = HashMap::new();
    for command_id in KEYBOARD_SHORTCUT_COMMAND_IDS {
        let Some(value) = bindings.get(command_id) else {
            continue;
        };
        let values = value.as_array()?;
        if values.len() > MAX_KEYBOARD_SHORTCUTS_PER_COMMAND {
            return None;
        }
        let mut accelerators = Vec::with_capacity(values.len());
        for value in values {
            let accelerator = value.as_str()?.trim();
            if accelerator.is_empty()
                || accelerator.len() > MAX_KEYBOARD_SHORTCUT_ACCELERATOR_BYTES
                || accelerator.chars().any(char::is_control)
            {
                return None;
            }
            accelerators.push(accelerator.to_owned());
        }
        overrides.insert(command_id.to_owned(), accelerators);
    }
    Some(KeyboardShortcutPreferences { overrides }.normalized())
}

fn parse_appearance_palette(
    value: &Value,
    variant: AppearanceVariant,
) -> Option<AppearancePalette> {
    let object = value.as_object()?;
    let fonts = object.get("fonts")?.as_object()?;
    let code_theme_id = object.get("codeThemeId")?.as_str()?;
    if !is_appearance_code_theme_id(code_theme_id)
        || !appearance_code_theme_supports_variant(code_theme_id, variant)
    {
        return None;
    }
    let semantic_colors = object.get("semanticColors")?.as_object()?;
    Some(
        AppearancePalette {
            accent: parse_appearance_color(object.get("accent")?)?,
            contrast: parse_appearance_size(object.get("contrast")?, 0, 100)?,
            code_font: parse_appearance_font(fonts.get("code")?)?,
            code_theme_id: code_theme_id.to_owned(),
            ink: parse_appearance_color(object.get("ink")?)?,
            opaque_windows: object.get("opaqueWindows")?.as_bool()?,
            semantic_colors: AppearanceSemanticColors {
                diff_added: parse_appearance_color(semantic_colors.get("diffAdded")?)?,
                diff_removed: parse_appearance_color(semantic_colors.get("diffRemoved")?)?,
                skill: parse_appearance_color(semantic_colors.get("skill")?)?,
            },
            surface: parse_appearance_color(object.get("surface")?)?,
            ui_font: parse_appearance_font(fonts.get("ui")?)?,
        }
        .normalized(variant),
    )
}

fn parse_appearance_font(value: &Value) -> Option<Option<String>> {
    if value.is_null() {
        return Some(None);
    }
    let value = value.as_str()?.trim();
    if value.len() > MAX_APPEARANCE_FONT_FAMILY_BYTES {
        return None;
    }
    Some((!value.is_empty()).then(|| value.to_owned()))
}

fn parse_appearance_size(value: &Value, min: u8, max: u8) -> Option<u8> {
    let value = u8::try_from(value.as_u64()?).ok()?;
    (min..=max).contains(&value).then_some(value)
}

fn parse_appearance_color(value: &Value) -> Option<u32> {
    let value = value.as_str()?;
    if value.len() != 7 || !value.starts_with('#') {
        return None;
    }
    u32::from_str_radix(&value[1..], 16).ok()
}

fn format_appearance_color(color: u32) -> String {
    format!("#{:06X}", color & 0xff_ff_ff)
}

const fn reduced_motion_key(preference: ReducedMotionPreference) -> &'static str {
    match preference {
        ReducedMotionPreference::System => "system",
        ReducedMotionPreference::On => "on",
        ReducedMotionPreference::Off => "off",
    }
}

const fn diff_marker_style_key(style: DiffMarkerStyle) -> &'static str {
    match style {
        DiffMarkerStyle::Color => "color",
        DiffMarkerStyle::Symbols => "symbols",
    }
}

fn parse_terminal_location(value: &str) -> Option<TerminalDockLocation> {
    match value {
        "bottom" => Some(TerminalDockLocation::Bottom),
        "right" => Some(TerminalDockLocation::Right),
        _ => None,
    }
}

fn parse_terminal_size(value: &str) -> Option<u32> {
    value.parse().ok()
}

fn parse_bool_preference(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

const fn terminal_size_preference_key(location: TerminalDockLocation) -> &'static str {
    match location {
        TerminalDockLocation::Bottom => TERMINAL_BOTTOM_HEIGHT_PREFERENCE,
        TerminalDockLocation::Right => TERMINAL_RIGHT_WIDTH_PREFERENCE,
    }
}

const fn integrated_terminal_shell_key(shell: IntegratedTerminalShell) -> &'static str {
    match shell {
        IntegratedTerminalShell::PowerShell => "powershell",
        IntegratedTerminalShell::CommandPrompt => "commandPrompt",
        IntegratedTerminalShell::GitBash => "gitBash",
        IntegratedTerminalShell::Wsl => "wsl",
    }
}

fn parse_integrated_terminal_shell(value: &str) -> Option<IntegratedTerminalShell> {
    match value {
        "powershell" => Some(IntegratedTerminalShell::PowerShell),
        "commandPrompt" => Some(IntegratedTerminalShell::CommandPrompt),
        "gitBash" => Some(IntegratedTerminalShell::GitBash),
        "wsl" => Some(IntegratedTerminalShell::Wsl),
        _ => None,
    }
}

fn composer_config_key(profile: Option<&str>, key: &str) -> String {
    profile.map_or_else(
        || key.to_owned(),
        |profile| format!("profiles.{profile}.{key}"),
    )
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or_default()
}

fn run_backend(commands: Receiver<BackendCommand>, events: Sender<Action>) {
    let runtime_policy = RuntimePolicy::default();
    let mut storage = open_storage(&events);
    if let Some(store) = storage.as_ref() {
        match store.browser_downloads(MAX_BROWSER_DOWNLOAD_RECORDS, 0) {
            Ok(page) => emit(
                &events,
                Action::BrowserDownloadHistoryLoaded(
                    page.items
                        .into_iter()
                        .map(restored_browser_download)
                        .collect(),
                ),
            ),
            Err(error) => emit(&events, Action::StorageFailed(error.to_string())),
        }
    }
    let mut browser_download_preferences = storage
        .as_ref()
        .and_then(|store| {
            store
                .preference(BROWSER_DOWNLOAD_PREFERENCES_V1)
                .ok()
                .flatten()
        })
        .as_deref()
        .and_then(parse_browser_download_preferences)
        .unwrap_or_default();
    let mut browser_permissions = storage
        .as_ref()
        .and_then(|store| store.preference(BROWSER_PERMISSIONS_V1).ok().flatten())
        .as_deref()
        .and_then(parse_browser_permissions)
        .unwrap_or_default();
    let preferred_terminal_shell = storage
        .as_ref()
        .and_then(|store| store.preference("integrated_terminal_shell").ok().flatten())
        .as_deref()
        .and_then(parse_integrated_terminal_shell);
    emit(
        &events,
        Action::TerminalShellsDetected {
            available: available_terminal_shells(),
            preferred: preferred_terminal_shell,
        },
    );
    let mut connection: Option<AppServerConnection> = None;
    let mut pending_approvals = HashMap::new();
    let mut marketplaces = HashMap::new();
    let mut computer_permissions = HashMap::new();
    let mut computer_capable_threads = HashSet::new();
    let mut computer_allowed_app_ids = HashSet::new();
    let mut retryable_turns = HashMap::new();
    let mut computer_accessibility = ComputerUseAccessibilityClient::new();
    let mut computer_url_policy = ComputerUseUrlPolicy::new();
    #[cfg(windows)]
    let computer_interruption = match ComputerUseInterruptionMonitor::new() {
        Ok(monitor) => Some(monitor),
        Err(error) => {
            emit(
                &events,
                Action::SetStatus(format!(
                    "Computer Use user interruption monitor is unavailable: {error}"
                )),
            );
            None
        }
    };
    #[cfg(not(windows))]
    let computer_interruption: Option<ComputerUseInterruptionMonitor> = None;
    #[cfg(windows)]
    let mut computer_overlay = match ComputerUseSystemOverlay::new() {
        Ok(overlay) => Some(overlay),
        Err(error) => {
            emit(
                &events,
                Action::SetStatus(format!(
                    "Computer Use system indicator is unavailable: {error}"
                )),
            );
            None
        }
    };
    #[cfg(not(windows))]
    let mut computer_overlay: Option<ComputerUseSystemOverlay> = None;
    let mut interrupted_computer_turns = HashSet::new();
    let mut terminals = HashMap::new();
    let mut browser = None;
    let mut app_server_reconnect = AppServerReconnectScheduler::default();
    let mut git_refresh = GitRefreshDebouncer::default();
    let mut task_search = TaskSearchDebouncer::default();
    let mut fuzzy_file_search = FuzzyFileSearchRuntime::default();
    let mut pull_request_search = PullRequestSearchDebouncer::default();
    let mut goal_continuations = GoalContinuationScheduler::default();
    let mut personality = Personality::default();
    let mut pending_worktree_runtime = None;

    loop {
        reap_pending_worktree_runtime(&mut pending_worktree_runtime);
        if let Some(turn) = computer_interruption
            .as_ref()
            .and_then(ComputerUseInterruptionMonitor::try_recv)
        {
            if let Some(overlay) = computer_overlay.as_mut() {
                let _ = overlay.complete_turn(&turn.thread_id, &turn.turn_id);
            }
            handle_computer_use_interruption(
                turn,
                connection.as_ref(),
                &events,
                &mut pending_approvals,
                &mut interrupted_computer_turns,
            );
        }
        if let Some(turn) = computer_interruption
            .as_ref()
            .and_then(ComputerUseInterruptionMonitor::try_recv_user_input)
            && let Some(window_id) = turn.window_id
        {
            computer_accessibility.mark_user_input(&window_id);
        }
        let mut disconnected = false;
        let mut filesystem_changed = false;
        if let Some(app_server) = connection.as_ref() {
            for _ in 0..64 {
                match app_server.try_recv_event() {
                    Ok(Some(AppServerEvent::Disconnected)) => {
                        emit(&events, Action::ConnectionLost);
                        disconnected = true;
                        break;
                    }
                    Ok(Some(event)) => {
                        if let Some(turn) = completed_turn_key(&event) {
                            retryable_turns.remove(&(turn.thread_id.clone(), turn.turn_id.clone()));
                            if let Some(monitor) = computer_interruption.as_ref() {
                                monitor.disarm_turn(&turn.thread_id, &turn.turn_id);
                            }
                            computer_url_policy.complete_turn(&turn.thread_id, &turn.turn_id);
                            if let Some(overlay) = computer_overlay.as_mut() {
                                let _ = overlay.complete_turn(&turn.thread_id, &turn.turn_id);
                            }
                            interrupted_computer_turns.remove(&(turn.thread_id, turn.turn_id));
                        }
                        let computer_request =
                            computer_tool_request_meta(&event).filter(|request| {
                                computer_use_tool_supported_on_platform(&request.tool)
                            });
                        if computer_request.is_some()
                            && let Some(turn) = computer_interruption
                                .as_ref()
                                .and_then(ComputerUseInterruptionMonitor::try_recv_user_input)
                            && let Some(window_id) = turn.window_id
                        {
                            computer_accessibility.mark_user_input(&window_id);
                        }
                        if computer_request.is_some()
                            && let Some(monitor) = computer_interruption.as_ref()
                        {
                            monitor.disarm();
                        }
                        if let Some(request) = computer_request.as_ref() {
                            let interrupted = interrupted_computer_turns
                                .contains(&(request.thread_id.clone(), request.turn_id.clone()));
                            let url_policy_block = computer_url_policy
                                .blocked_message(&request.thread_id, &request.turn_id);
                            let monitor_unavailable = computer_interruption.is_none()
                                && computer_tool_requires_interruption_monitor(&request.tool);
                            let overlay_unavailable = computer_overlay.is_none()
                                && computer_tool_requires_interruption_monitor(&request.tool);
                            if interrupted
                                || url_policy_block.is_some()
                                || monitor_unavailable
                                || overlay_unavailable
                            {
                                if (interrupted || url_policy_block.is_some())
                                    && let Some(overlay) = computer_overlay.as_mut()
                                {
                                    let _ =
                                        overlay.complete_turn(&request.thread_id, &request.turn_id);
                                }
                                respond_dynamic_tool_failure(
                                    app_server,
                                    &request.id,
                                    if interrupted {
                                        COMPUTER_USE_ESCAPE_STOP_MESSAGE
                                    } else if let Some(message) = url_policy_block {
                                        message
                                    } else if monitor_unavailable {
                                        COMPUTER_USE_MONITOR_UNAVAILABLE_MESSAGE
                                    } else {
                                        COMPUTER_USE_OVERLAY_UNAVAILABLE_MESSAGE
                                    },
                                );
                                continue;
                            }
                        }
                        if !handle_fuzzy_file_search_event(&event, &fuzzy_file_search, &events) {
                            filesystem_changed |= handle_app_server_event_with_browser_permissions(
                                app_server,
                                event,
                                &events,
                                &mut pending_approvals,
                                &mut computer_permissions,
                                &mut computer_allowed_app_ids,
                                &mut computer_accessibility,
                                &mut computer_url_policy,
                                computer_overlay.as_mut(),
                                Some(&browser_permissions),
                            )
                        }
                        if let (Some(monitor), Some(request)) =
                            (computer_interruption.as_ref(), computer_request)
                        {
                            monitor.arm(request.thread_id, request.turn_id, request.window_id);
                        }
                    }
                    Ok(None) => break,
                    Err(_) => {
                        emit(
                            &events,
                            Action::SetStatus("app-server event channel closed".to_owned()),
                        );
                        emit(&events, Action::ConnectionLost);
                        disconnected = true;
                        break;
                    }
                }
            }
        }
        if filesystem_changed {
            emit(&events, Action::RefreshGit);
        }
        if disconnected {
            connection.take();
            pending_approvals.clear();
            computer_allowed_app_ids.clear();
            computer_accessibility = ComputerUseAccessibilityClient::new();
            computer_url_policy.clear();
            if let Some(monitor) = computer_interruption.as_ref() {
                monitor.disarm();
            }
            if let Some(overlay) = computer_overlay.as_mut() {
                let _ = overlay.hide();
            }
            interrupted_computer_turns.clear();
            retryable_turns.clear();
            task_search.clear();
            fuzzy_file_search.reset();
            pull_request_search.clear();
            goal_continuations.clear();
        }
        drain_terminals(&mut terminals, &events);
        if drain_browser(&mut browser, &events) {
            browser.take();
        }

        match commands.recv_timeout(BACKEND_TICK) {
            Ok(BackendCommand::Run(effect)) => match *effect {
                Effect::ConnectAppServer => {
                    app_server_reconnect.reset();
                    fuzzy_file_search.reset();
                    if let Err(error) = connect(&events, &mut connection) {
                        emit(
                            &events,
                            Action::ConnectionFailed(bounded(error, MAX_STATUS_BYTES)),
                        );
                    }
                }
                Effect::ScheduleAppServerReconnect => {
                    if connection.is_none()
                        && let Some((attempt, delay)) =
                            app_server_reconnect.schedule(Instant::now())
                    {
                        emit(
                            &events,
                            Action::ConnectionRetryScheduled {
                                attempt,
                                retry_in_ms: u64::try_from(delay.as_millis()).unwrap_or(u64::MAX),
                                last_error: None,
                            },
                        );
                    }
                }
                Effect::RefreshGit { generation, cwd } => {
                    git_refresh.schedule(
                        generation,
                        cwd,
                        Instant::now(),
                        runtime_policy.git_debounce,
                    );
                }
                Effect::ScheduleTaskSearch { generation, query } => {
                    task_search.schedule(generation, query, Instant::now(), TASK_SEARCH_DEBOUNCE);
                }
                Effect::SchedulePullRequestSearch { generation } => {
                    pull_request_search.schedule(
                        generation,
                        Instant::now(),
                        PULL_REQUEST_SEARCH_DEBOUNCE,
                    );
                }
                Effect::ScheduleGoalContinuation { task_id } => {
                    if let Err(message) = goal_continuations.schedule(
                        task_id.clone(),
                        Instant::now(),
                        GOAL_CONTINUATION_DELAY,
                    ) {
                        emit(
                            &events,
                            Action::GoalContinuationFailed {
                                task_id,
                                message: message.to_owned(),
                            },
                        );
                    }
                }
                effect => {
                    let computer_turn_to_rearm = matches!(&effect, Effect::RespondApproval { .. })
                        .then(|| {
                            computer_interruption
                                .as_ref()
                                .and_then(ComputerUseInterruptionMonitor::active_turn)
                        })
                        .flatten();
                    if computer_turn_to_rearm.is_some()
                        && let Some(monitor) = computer_interruption.as_ref()
                    {
                        monitor.disarm();
                    }
                    run_effect(
                        effect,
                        &events,
                        &mut connection,
                        &mut pending_approvals,
                        &mut marketplaces,
                        &mut computer_permissions,
                        &mut computer_capable_threads,
                        &mut computer_allowed_app_ids,
                        &mut computer_accessibility,
                        &mut computer_url_policy,
                        computer_overlay.as_mut(),
                        &mut storage,
                        &mut terminals,
                        &mut browser,
                        &mut browser_download_preferences,
                        &mut browser_permissions,
                        &mut fuzzy_file_search,
                        &mut personality,
                        &mut pending_worktree_runtime,
                        &mut retryable_turns,
                    );
                    if let (Some(monitor), Some(turn)) =
                        (computer_interruption.as_ref(), computer_turn_to_rearm)
                    {
                        monitor.arm(turn.thread_id, turn.turn_id, turn.window_id);
                    }
                }
            },
            Ok(BackendCommand::Shutdown)
            | Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                break;
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
        }

        if let Some(attempt) = app_server_reconnect.take_due(Instant::now()) {
            emit(&events, Action::ConnectionRetryStarted { attempt });
            match connect(&events, &mut connection) {
                Ok(()) => app_server_reconnect.reset(),
                Err(error) => {
                    if let Some((next_attempt, delay)) =
                        app_server_reconnect.schedule(Instant::now())
                    {
                        emit(
                            &events,
                            Action::ConnectionRetryScheduled {
                                attempt: next_attempt,
                                retry_in_ms: u64::try_from(delay.as_millis()).unwrap_or(u64::MAX),
                                last_error: Some(bounded(error, MAX_STATUS_BYTES)),
                            },
                        );
                    }
                }
            }
        }
        if let Some((generation, cwd)) = git_refresh.take_due(Instant::now()) {
            refresh_git(generation, &cwd, &events);
        }
        if let Some((generation, query)) = task_search.take_due(Instant::now()) {
            emit(&events, Action::TaskSearchDue { generation, query });
        }
        if let Some(generation) = pull_request_search.take_due(Instant::now()) {
            emit(&events, Action::PullRequestSearchDue { generation });
        }
        for task_id in goal_continuations.take_due(Instant::now()) {
            emit(&events, Action::GoalContinuationDue { task_id });
        }
    }

    if let Some(mut app_server) = connection {
        let _ = app_server.shutdown();
    }
    if let Some(runtime) = pending_worktree_runtime.take() {
        runtime.cancellation.store(true, Ordering::Release);
        let _ = runtime.thread.join();
    }
    for (_, mut runtime) in terminals {
        runtime.session.shutdown();
    }
    if let Some(mut runtime) = browser {
        runtime.session.shutdown();
    }
}

fn reap_pending_worktree_runtime(runtime: &mut Option<PendingWorktreeRuntime>) {
    if runtime
        .as_ref()
        .is_some_and(|runtime| runtime.thread.is_finished())
        && let Some(runtime) = runtime.take()
    {
        let _ = runtime.thread.join();
    }
}

fn join_pending_worktree_runtime(runtime: &mut Option<PendingWorktreeRuntime>) {
    if let Some(runtime) = runtime.take() {
        let _ = runtime.thread.join();
    }
}

fn cancel_pending_worktree_runtime(runtime: &mut Option<PendingWorktreeRuntime>, request_id: u64) {
    if runtime
        .as_ref()
        .is_some_and(|runtime| runtime.request_id == request_id)
        && let Some(runtime) = runtime.take()
    {
        runtime.cancellation.store(true, Ordering::Release);
        let _ = runtime.thread.join();
    }
}

fn computer_tool_request_meta(event: &AppServerEvent) -> Option<ComputerToolRequestMeta> {
    let AppServerEvent::Request { id, method, params } = event else {
        return None;
    };
    if method != "item/tool/call"
        || params.get("namespace").and_then(Value::as_str) != Some("computer_use")
    {
        return None;
    }
    Some(ComputerToolRequestMeta {
        id: id.clone(),
        thread_id: string_field(params, "threadId")?,
        turn_id: string_field(params, "turnId")?,
        tool: string_field(params, "tool")?,
        window_id: params
            .get("arguments")
            .and_then(|arguments| computer_window_argument(arguments).ok())
            .map(|(window_id, _)| window_id),
    })
}

fn completed_turn_key(event: &AppServerEvent) -> Option<ComputerUseTurnKey> {
    let AppServerEvent::Notification { method, params } = event else {
        return None;
    };
    if method != "turn/completed" {
        return None;
    }
    Some(ComputerUseTurnKey {
        thread_id: string_field(params, "threadId")?,
        turn_id: params
            .get("turn")
            .and_then(|turn| string_field(turn, "id"))
            .or_else(|| string_field(params, "turnId"))?,
        window_id: None,
    })
}

fn computer_tool_requires_interruption_monitor(tool: &str) -> bool {
    matches!(
        tool,
        "launch_app"
            | "activate_window"
            | "click"
            | "drag"
            | "perform_secondary_action"
            | "press_key"
            | "scroll"
            | "set_value"
            | "type_text"
    )
}

fn handle_computer_use_interruption(
    turn: ComputerUseTurnKey,
    app_server: Option<&AppServerConnection>,
    events: &Sender<Action>,
    pending_approvals: &mut HashMap<String, PendingApproval>,
    interrupted_turns: &mut HashSet<(String, String)>,
) {
    if interrupted_turns.len() >= MAX_INTERRUPTED_COMPUTER_TURNS {
        interrupted_turns.clear();
    }
    interrupted_turns.insert((turn.thread_id.clone(), turn.turn_id.clone()));

    if let Some(app_server) = app_server {
        cancel_pending_computer_use_approvals(app_server, &turn, events, pending_approvals);
        if let Err(error) = app_server.interrupt_turn(TurnInterruptParams {
            thread_id: turn.thread_id.clone(),
            turn_id: turn.turn_id.clone(),
        }) {
            emit(
                events,
                Action::TurnInterruptFailed {
                    task_id: turn.thread_id.clone(),
                    message: format!("failed to stop Computer Use after physical Escape: {error}"),
                },
            );
        }
    }
    emit(
        events,
        Action::SetStatus(
            "Computer Use was stopped by the user with the physical Escape key.".to_owned(),
        ),
    );
}

fn cancel_pending_computer_use_approvals(
    app_server: &AppServerConnection,
    turn: &ComputerUseTurnKey,
    events: &Sender<Action>,
    pending_approvals: &mut HashMap<String, PendingApproval>,
) {
    let request_ids = pending_approvals
        .iter()
        .filter_map(|(request_id, pending)| {
            let params = match pending {
                PendingApproval::ComputerUse { params, .. }
                | PendingApproval::ComputerUseLaunch { params, .. } => params,
                _ => return None,
            };
            (params.thread_id == turn.thread_id && params.turn_id == turn.turn_id)
                .then(|| request_id.clone())
        })
        .collect::<Vec<_>>();
    for request_id in request_ids {
        let Some(pending) = pending_approvals.remove(&request_id) else {
            continue;
        };
        let id = match pending {
            PendingApproval::ComputerUse { id, .. }
            | PendingApproval::ComputerUseLaunch { id, .. } => id,
            _ => continue,
        };
        respond_dynamic_tool_failure(app_server, &id, COMPUTER_USE_ESCAPE_STOP_MESSAGE);
        emit(
            events,
            Action::ResolveApproval {
                request_id,
                decision: ApprovalDecision::Decline,
            },
        );
    }
}

fn archived_task_ids_for_delete(app_server: &AppServerConnection) -> Result<Vec<String>, String> {
    let mut task_ids = Vec::new();
    let mut seen_task_ids = HashSet::new();
    let mut seen_cursors = HashSet::new();
    let mut cursor = None;

    for _ in 0..MAX_ARCHIVED_DELETE_PAGES {
        let mut params =
            ThreadListParams::state_db_page(ARCHIVED_DELETE_PAGE_LIMIT).with_cursor(cursor);
        params.archived = Some(true);
        let page = app_server
            .list_threads(params)
            .map_err(|error| format!("failed to list archived chats before deletion: {error}"))?;
        for thread in page.data {
            if seen_task_ids.insert(thread.id.clone()) {
                if task_ids.len() == MAX_VISIBLE_THREADS {
                    return Err(format!(
                        "deleting all archived chats is limited to {MAX_VISIBLE_THREADS} chats"
                    ));
                }
                task_ids.push(thread.id);
            }
        }
        let Some(next_cursor) = page.next_cursor else {
            return Ok(task_ids);
        };
        if !seen_cursors.insert(next_cursor.clone()) {
            return Err("archived chat pagination repeated a cursor".to_owned());
        }
        cursor = Some(next_cursor);
    }

    Err(format!(
        "deleting all archived chats is limited to {MAX_VISIBLE_THREADS} chats"
    ))
}

#[allow(clippy::too_many_arguments)]
fn run_effect(
    effect: Effect,
    events: &Sender<Action>,
    connection: &mut Option<AppServerConnection>,
    pending_approvals: &mut HashMap<String, PendingApproval>,
    marketplaces: &mut HashMap<String, Option<PathBuf>>,
    computer_permissions: &mut HashMap<String, ComputerUsePermission>,
    computer_capable_threads: &mut HashSet<String>,
    computer_allowed_app_ids: &mut HashSet<String>,
    computer_accessibility: &mut ComputerUseAccessibilityClient,
    computer_url_policy: &mut ComputerUseUrlPolicy,
    computer_overlay: Option<&mut ComputerUseSystemOverlay>,
    storage: &mut Option<Store>,
    terminals: &mut HashMap<u64, TerminalRuntime>,
    browser: &mut Option<BrowserRuntime>,
    browser_download_preferences: &mut BrowserDownloadPreferences,
    browser_permissions: &mut BrowserPermissionsState,
    fuzzy_file_search: &mut FuzzyFileSearchRuntime,
    personality: &mut Personality,
    pending_worktree_runtime: &mut Option<PendingWorktreeRuntime>,
    retryable_turns: &mut HashMap<(String, String), RetryableTurnSubmission>,
) {
    if let Effect::CancelPendingWorktreeFork { request_id } = &effect {
        cancel_pending_worktree_runtime(pending_worktree_runtime, *request_id);
        emit(
            events,
            Action::PendingWorktreeForkCancelled {
                request_id: *request_id,
            },
        );
        return;
    }

    if handle_browser_effect(
        &effect,
        events,
        browser,
        browser_download_preferences,
        browser_permissions,
    ) {
        return;
    }

    if let Effect::ForkTaskIntoWorktree {
        request_id,
        cwd,
        worktrees_root,
        ..
    } = &effect
    {
        join_pending_worktree_runtime(pending_worktree_runtime);
        let worktrees_root = match worktrees_root.clone() {
            Some(path) => path,
            None => match codexrs_data_dir() {
                Ok(path) => path.join("worktrees"),
                Err(error) => {
                    emit(
                        events,
                        Action::PendingWorktreeForkCreationFailed {
                            request_id: *request_id,
                            message: format!("Failed to create worktree: {error}"),
                        },
                    );
                    return;
                }
            },
        };
        let cancellation = Arc::new(AtomicBool::new(false));
        let worker_cancellation = Arc::clone(&cancellation);
        let worker_events = events.clone();
        let worker_cwd = cwd.clone();
        let worker_request_id = *request_id;
        let thread = match thread::Builder::new()
            .name("codex-rs-worktree-create".to_owned())
            .spawn(move || {
                match git_create_managed_worktree(
                    &worker_cwd,
                    &worktrees_root,
                    &worker_cancellation,
                ) {
                    Ok(worktree) => emit(
                        &worker_events,
                        Action::PendingWorktreeForkReady {
                            request_id: worker_request_id,
                            workspace_root: worktree.workspace_root,
                            git_root: worktree.git_root,
                        },
                    ),
                    Err(GitError::Cancelled) => {}
                    Err(GitError::InvalidRepository) => emit(
                        &worker_events,
                        Action::PendingWorktreeForkCreationFailed {
                            request_id: worker_request_id,
                            message: "A Git repository is required to continue in a new worktree"
                                .to_owned(),
                        },
                    ),
                    Err(error) => emit(
                        &worker_events,
                        Action::PendingWorktreeForkCreationFailed {
                            request_id: worker_request_id,
                            message: format!("Failed to create worktree: {error}"),
                        },
                    ),
                }
            }) {
            Ok(thread) => thread,
            Err(error) => {
                emit(
                    events,
                    Action::PendingWorktreeForkCreationFailed {
                        request_id: *request_id,
                        message: format!("Failed to create worktree: {error}"),
                    },
                );
                return;
            }
        };
        *pending_worktree_runtime = Some(PendingWorktreeRuntime {
            request_id: *request_id,
            cancellation,
            thread,
        });
        return;
    }

    match &effect {
        Effect::PersistPrimaryWindowPlacement(placement) => {
            let result = storage.as_mut().map_or(Ok(()), |store| {
                let encoded = encode_primary_window_placement(*placement)
                    .map_err(|error| codex_storage::StoreError::Io(std::io::Error::other(error)))?;
                store.set_preference(PRIMARY_WINDOW_PLACEMENT_V1, &encoded, unix_timestamp())
            });
            if let Err(error) = result {
                storage.take();
                emit(events, Action::StorageFailed(error.to_string()));
            }
            return;
        }
        Effect::PersistUiState { route, inspector } => {
            let result = storage.as_mut().map_or(Ok(()), |store| {
                let now = unix_timestamp();
                store.set_preference("route", route_key(*route), now)?;
                store.set_preference("inspector", inspector_key(*inspector), now)
            });
            if let Err(error) = result {
                storage.take();
                emit(events, Action::StorageFailed(error.to_string()));
            }
            return;
        }
        Effect::PersistTerminalDockLocation(location) => {
            let result = storage.as_mut().map_or(Ok(()), |store| {
                store.set_preference(
                    "terminal_location",
                    terminal_location_key(*location),
                    unix_timestamp(),
                )
            });
            if let Err(error) = result {
                storage.take();
                emit(events, Action::StorageFailed(error.to_string()));
            }
            return;
        }
        Effect::PersistAppearanceTheme(theme) => {
            let result = storage.as_mut().map_or(Ok(()), |store| {
                store.set_preference(
                    APPEARANCE_THEME_PREFERENCE,
                    appearance_theme_key(*theme),
                    unix_timestamp(),
                )
            });
            if let Err(error) = result {
                storage.take();
                emit(events, Action::StorageFailed(error.to_string()));
            }
            return;
        }
        Effect::PersistAppearancePreferences(preferences) => {
            let result = storage.as_mut().map_or(Ok(()), |store| {
                let encoded = encode_appearance_preferences(preferences)
                    .map_err(|error| codex_storage::StoreError::Io(std::io::Error::other(error)))?;
                store.set_preference(APPEARANCE_PREFERENCES_V1, &encoded, unix_timestamp())
            });
            if let Err(error) = result {
                storage.take();
                emit(events, Action::StorageFailed(error.to_string()));
            }
            return;
        }
        Effect::PersistGitPreferences(preferences) => {
            let result = storage.as_mut().map_or(Ok(()), |store| {
                let encoded = encode_git_preferences(preferences)
                    .map_err(|error| codex_storage::StoreError::Io(std::io::Error::other(error)))?;
                store.set_preference(GIT_PREFERENCES_V1, &encoded, unix_timestamp())
            });
            if let Err(error) = result {
                storage.take();
                emit(events, Action::StorageFailed(error.to_string()));
            }
            return;
        }
        Effect::PersistBrowserDownloadPreferences(preferences) => {
            let result = storage.as_mut().map_or(Ok(()), |store| {
                let encoded = encode_browser_download_preferences(preferences)
                    .map_err(|error| codex_storage::StoreError::Io(std::io::Error::other(error)))?;
                store.set_preference(BROWSER_DOWNLOAD_PREFERENCES_V1, &encoded, unix_timestamp())
            });
            if let Err(error) = result {
                storage.take();
                emit(events, Action::StorageFailed(error.to_string()));
            }
            return;
        }
        Effect::PersistBrowserPermissions(permissions) => {
            let result = storage.as_mut().map_or(Ok(()), |store| {
                let encoded = encode_browser_permissions(permissions)
                    .map_err(|error| codex_storage::StoreError::Io(std::io::Error::other(error)))?;
                store.set_preference(BROWSER_PERMISSIONS_V1, &encoded, unix_timestamp())
            });
            if let Err(error) = result {
                storage.take();
                emit(events, Action::StorageFailed(error.to_string()));
            }
            return;
        }
        Effect::PersistBrowserDownload(download) => {
            let Some(download) = stored_browser_download(download) else {
                return;
            };
            let result = storage
                .as_mut()
                .map_or(Ok(()), |store| store.upsert_browser_download(&download));
            if let Err(error) = result {
                storage.take();
                emit(events, Action::StorageFailed(error.to_string()));
            }
            return;
        }
        Effect::DeletePersistedBrowserDownload { id } => {
            let result = storage
                .as_mut()
                .map_or(Ok(()), |store| store.remove_browser_download(id));
            if let Err(error) = result {
                storage.take();
                emit(events, Action::StorageFailed(error.to_string()));
            }
            return;
        }
        Effect::PersistKeyboardShortcutPreferences {
            preferences,
            previous,
            target,
        } => {
            let result = storage.as_mut().map_or(Ok(()), |store| {
                let encoded = encode_keyboard_shortcut_preferences(preferences)
                    .map_err(|error| codex_storage::StoreError::Io(std::io::Error::other(error)))?;
                store.set_preference(KEYBOARD_SHORTCUT_PREFERENCES_V1, &encoded, unix_timestamp())
            });
            if let Err(error) = result {
                storage.take();
                emit(
                    events,
                    Action::KeyboardShortcutPreferencesPersistFailed {
                        previous: previous.clone(),
                        target: target.clone(),
                    },
                );
                emit(events, Action::StorageFailed(error.to_string()));
            } else {
                emit(
                    events,
                    Action::KeyboardShortcutPreferencesPersisted(target.clone()),
                );
            }
            return;
        }
        Effect::PersistTerminalDockSize { location, size } => {
            let result = storage.as_mut().map_or(Ok(()), |store| {
                store.set_preference(
                    terminal_size_preference_key(*location),
                    &size.to_string(),
                    unix_timestamp(),
                )
            });
            if let Err(error) = result {
                storage.take();
                emit(events, Action::StorageFailed(error.to_string()));
            }
            return;
        }
        Effect::PersistIntegratedTerminalShell(shell) => {
            let result = storage.as_mut().map_or(Ok(()), |store| {
                store.set_preference(
                    "integrated_terminal_shell",
                    integrated_terminal_shell_key(*shell),
                    unix_timestamp(),
                )
            });
            if let Err(error) = result {
                storage.take();
                emit(events, Action::StorageFailed(error.to_string()));
            }
            return;
        }
        Effect::PersistGitIncludeUnstaged(include_unstaged) => {
            let result = storage.as_mut().map_or(Ok(()), |store| {
                store.set_preference(
                    GIT_INCLUDE_UNSTAGED_PREFERENCE,
                    if *include_unstaged { "true" } else { "false" },
                    unix_timestamp(),
                )
            });
            if let Err(error) = result {
                storage.take();
                emit(events, Action::StorageFailed(error.to_string()));
            }
            return;
        }
        Effect::PersistPinnedTasks { task_ids } => {
            let result = storage.as_mut().map_or(Ok(()), |store| {
                let encoded = serde_json::to_string(task_ids)
                    .map_err(|error| codex_storage::StoreError::Io(std::io::Error::other(error)))?;
                store.set_preference(PINNED_TASK_IDS_PREFERENCE, &encoded, unix_timestamp())
            });
            if let Err(error) = result {
                storage.take();
                emit(events, Action::StorageFailed(error.to_string()));
            }
            return;
        }
        Effect::RememberWorkspace { path } => {
            let result = storage.as_mut().map_or(Ok(()), |store| {
                store.remember_workspace(path, unix_timestamp())
            });
            if let Err(error) = result {
                storage.take();
                emit(events, Action::StorageFailed(error.to_string()));
            }
            return;
        }
        Effect::RenameLocalProject { path, name } => {
            let result = storage.as_mut().map_or(Ok(()), |store| {
                store.rename_workspace(path, name, unix_timestamp())
            });
            if let Err(error) = result {
                storage.take();
                emit(events, Action::StorageFailed(error.to_string()));
            }
            return;
        }
        Effect::SetLocalProjectPinned { path, pinned } => {
            let result = storage
                .as_mut()
                .map_or(Ok(()), |store| store.set_workspace_pinned(path, *pinned));
            if let Err(error) = result {
                storage.take();
                emit(events, Action::StorageFailed(error.to_string()));
            }
            return;
        }
        Effect::RemoveLocalProject { path } => {
            let result = storage
                .as_mut()
                .map_or(Ok(()), |store| store.remove_workspace(path));
            if let Err(error) = result {
                storage.take();
                emit(events, Action::StorageFailed(error.to_string()));
            }
            return;
        }
        Effect::SearchPullRequests {
            generation,
            cwd,
            relationship,
            lifecycle,
            query,
            cursor,
            append,
        } => {
            let filters = GitHubPullRequestSearchFilters {
                relationship: platform_pull_request_relationship(*relationship),
                lifecycle: platform_pull_request_lifecycle(*lifecycle),
                text: query.clone(),
            };
            match github_search_pull_requests(
                cwd,
                &filters,
                cursor.as_deref(),
                PULL_REQUEST_PAGE_LIMIT,
            ) {
                Ok(page) => emit(
                    events,
                    Action::PullRequestsLoaded {
                        generation: *generation,
                        account_login: page.account.login,
                        account_avatar_url: page.account.avatar_url,
                        items: page
                            .items
                            .into_iter()
                            .map(core_pull_request_summary)
                            .collect(),
                        total_count: page.total_count,
                        next_cursor: page.next_cursor,
                        truncated: page.truncated,
                        append: *append,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::PullRequestsFailed {
                        generation: *generation,
                        message: bounded(error.to_string(), MAX_STATUS_BYTES),
                        append: *append,
                    },
                ),
            }
            return;
        }
        Effect::LoadPullRequestDetail {
            generation,
            cwd,
            identity,
            account_login,
        } => {
            let platform_identity = GitHubPullRequestIdentity {
                hostname: identity.hostname.clone(),
                owner: identity.owner.clone(),
                repository: identity.repository.clone(),
                number: identity.number,
            };
            match github_pull_request_detail(cwd, &platform_identity, account_login) {
                Ok(detail) => emit(
                    events,
                    Action::PullRequestDetailLoaded {
                        generation: *generation,
                        detail: core_pull_request_detail(detail),
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::PullRequestDetailFailed {
                        generation: *generation,
                        message: bounded(error.to_string(), MAX_STATUS_BYTES),
                    },
                ),
            }
            return;
        }
        Effect::LoadPullRequestDiff {
            generation,
            cwd,
            identity,
        } => {
            let platform_identity = GitHubPullRequestIdentity {
                hostname: identity.hostname.clone(),
                owner: identity.owner.clone(),
                repository: identity.repository.clone(),
                number: identity.number,
            };
            match github_pull_request_diff(cwd, &platform_identity) {
                Ok(diff) => emit(
                    events,
                    Action::PullRequestDiffLoaded {
                        generation: *generation,
                        identity: identity.clone(),
                        head_revision: diff.head_revision,
                        unified_diff: diff.unified_diff,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::PullRequestDiffFailed {
                        generation: *generation,
                        identity: identity.clone(),
                        message: bounded(error.to_string(), MAX_STATUS_BYTES),
                    },
                ),
            }
            return;
        }
        Effect::MutatePullRequest {
            generation,
            cwd,
            identity,
            expected_head_revision,
            mutation,
        } => {
            let platform_identity = GitHubPullRequestIdentity {
                hostname: identity.hostname.clone(),
                owner: identity.owner.clone(),
                repository: identity.repository.clone(),
                number: identity.number,
            };
            let result = match mutation {
                PullRequestMutation::Comment { body } => {
                    github_post_pull_request_comment(cwd, &platform_identity, body)
                }
                PullRequestMutation::Review { event, body } => github_submit_pull_request_review(
                    cwd,
                    &platform_identity,
                    expected_head_revision,
                    match event {
                        PullRequestReviewEvent::Approve => GitHubPullRequestReviewEvent::Approve,
                        PullRequestReviewEvent::Comment => GitHubPullRequestReviewEvent::Comment,
                        PullRequestReviewEvent::RequestChanges => {
                            GitHubPullRequestReviewEvent::RequestChanges
                        }
                    },
                    body,
                ),
                PullRequestMutation::SetReviewState { state } => {
                    github_set_pull_request_review_state(
                        cwd,
                        &platform_identity,
                        expected_head_revision,
                        match state {
                            codex_core::PullRequestReviewState::Draft => {
                                GitHubPullRequestReviewState::Draft
                            }
                            codex_core::PullRequestReviewState::Ready => {
                                GitHubPullRequestReviewState::Ready
                            }
                        },
                    )
                }
                PullRequestMutation::EditTitle { title } => github_update_pull_request_title(
                    cwd,
                    &platform_identity,
                    expected_head_revision,
                    title,
                ),
                PullRequestMutation::EditDescription { body } => github_update_pull_request_body(
                    cwd,
                    &platform_identity,
                    expected_head_revision,
                    body,
                ),
                PullRequestMutation::Merge { method } => github_merge_pull_request(
                    cwd,
                    &platform_identity,
                    expected_head_revision,
                    match method {
                        PullRequestMergeMethod::Merge => GitHubPullRequestMergeMethod::Merge,
                        PullRequestMergeMethod::Squash => GitHubPullRequestMergeMethod::Squash,
                    },
                ),
            };
            match result {
                Ok(()) => emit(
                    events,
                    Action::PullRequestMutationCompleted {
                        generation: *generation,
                        identity: identity.clone(),
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::PullRequestMutationFailed {
                        generation: *generation,
                        identity: identity.clone(),
                        message: bounded(error.to_string(), MAX_STATUS_BYTES),
                    },
                ),
            }
            return;
        }
        Effect::RefreshGit { generation, cwd } => {
            refresh_git(*generation, cwd, events);
            return;
        }
        Effect::LoadDiff {
            generation,
            root,
            path,
            staged,
        } => {
            match git_diff(root, path, *staged) {
                Ok(diff) => emit(
                    events,
                    Action::DiffLoaded {
                        generation: *generation,
                        text: diff.text,
                        truncated: diff.truncated,
                    },
                ),
                Err(error) => {
                    emit(
                        events,
                        Action::DiffLoaded {
                            generation: *generation,
                            text: String::new(),
                            truncated: false,
                        },
                    );
                    emit(
                        events,
                        Action::SetStatus(format!("failed to load diff: {error}")),
                    );
                }
            }
            return;
        }
        Effect::LoadUncommittedDiff { generation, root } => {
            match git_uncommitted_diff(root) {
                Ok(diff) => emit(
                    events,
                    Action::GitSourceDiffLoaded {
                        generation: *generation,
                        scope: GitDiffScope::Uncommitted,
                        text: diff.text,
                        truncated: diff.truncated,
                    },
                ),
                Err(_) => emit(
                    events,
                    Action::GitSourceDiffFailed {
                        generation: *generation,
                        scope: GitDiffScope::Uncommitted,
                        message: "Could not load uncommitted changes.".to_owned(),
                    },
                ),
            }
            return;
        }
        Effect::LoadCommitDiff {
            generation,
            root,
            sha,
        } => {
            match git_commit_diff(root, sha) {
                Ok(diff) => emit(
                    events,
                    Action::GitSourceDiffLoaded {
                        generation: *generation,
                        scope: GitDiffScope::Committed,
                        text: diff.text,
                        truncated: diff.truncated,
                    },
                ),
                Err(_) => emit(
                    events,
                    Action::GitSourceDiffFailed {
                        generation: *generation,
                        scope: GitDiffScope::Committed,
                        message: "Could not load changes for this commit.".to_owned(),
                    },
                ),
            }
            return;
        }
        Effect::StagePath { root, path } => {
            match git_stage(root, path) {
                Ok(()) => emit(events, Action::RefreshGit),
                Err(error) => emit(
                    events,
                    Action::SetStatus(format!("failed to stage path: {error}")),
                ),
            }
            return;
        }
        Effect::StageAll { root } => {
            match git_stage_all(root) {
                Ok(()) => emit(events, Action::RefreshGit),
                Err(error) => emit(
                    events,
                    Action::SetStatus(format!("failed to stage changes: {error}")),
                ),
            }
            return;
        }
        Effect::UnstagePath { root, path } => {
            match git_unstage(root, path) {
                Ok(()) => emit(events, Action::RefreshGit),
                Err(error) => emit(
                    events,
                    Action::SetStatus(format!("failed to unstage path: {error}")),
                ),
            }
            return;
        }
        Effect::UnstageAll { root } => {
            match git_unstage_all(root) {
                Ok(()) => emit(events, Action::RefreshGit),
                Err(error) => emit(
                    events,
                    Action::SetStatus(format!("failed to unstage changes: {error}")),
                ),
            }
            return;
        }
        Effect::CommitGit {
            root,
            branch,
            message,
            include_unstaged,
            next_step,
            force_push,
            commit_instructions,
        } => {
            let commits = *next_step != GitCommitNextStep::Push;
            let pushes = *next_step != GitCommitNextStep::Commit;
            let message = if commits && message.trim().is_empty() {
                let Some(app_server) = connection.as_ref() else {
                    emit(
                        events,
                        Action::GitCommitFailed {
                            message:
                                "Couldn't generate a commit message: app-server is unavailable."
                                    .to_owned(),
                        },
                    );
                    return;
                };
                match generate_commit_message(
                    app_server,
                    root,
                    *include_unstaged,
                    commit_instructions,
                    events,
                    pending_approvals,
                    computer_permissions,
                    computer_allowed_app_ids,
                    computer_accessibility,
                    computer_url_policy,
                ) {
                    Ok(message) => {
                        emit(events, Action::GitCommitMessageGenerated);
                        message
                    }
                    Err(error) => {
                        emit(
                            events,
                            Action::GitCommitFailed {
                                message: bounded(
                                    format!("Couldn't generate a commit message: {error}"),
                                    MAX_STATUS_BYTES,
                                ),
                            },
                        );
                        return;
                    }
                }
            } else {
                message.trim().to_owned()
            };
            let mut committed = false;
            if commits {
                match git_commit(root, &message, *include_unstaged) {
                    Ok(()) => committed = true,
                    Err(error) => {
                        emit(
                            events,
                            Action::GitCommitFailed {
                                message: bounded(
                                    format!("Failed to commit changes: {error}"),
                                    MAX_STATUS_BYTES,
                                ),
                            },
                        );
                        return;
                    }
                }
            }
            if pushes {
                if committed {
                    emit(events, Action::GitPushStarted);
                }
                if let Err(error) = git_push(root, *force_push) {
                    emit(
                        events,
                        Action::GitCommitFailed {
                            message: bounded(
                                format!("Failed to push changes: {error}"),
                                MAX_STATUS_BYTES,
                            ),
                        },
                    );
                    if committed {
                        emit(events, Action::RefreshGit);
                    }
                    return;
                }
            }
            emit(
                events,
                Action::GitCommitCompleted {
                    branch: branch.clone(),
                    pushed: pushes,
                },
            );
            emit(events, Action::RefreshGit);
            return;
        }
        Effect::LoadGitPullRequest { root, branch } => {
            match github_pull_request_status(root, branch) {
                Ok(status) => {
                    let provider = match status.availability {
                        GitHubCliAvailability::Available => GitPullRequestProvider::Available,
                        GitHubCliAvailability::Missing => GitPullRequestProvider::CliMissing,
                        GitHubCliAvailability::AuthenticationRequired => {
                            GitPullRequestProvider::AuthenticationRequired
                        }
                    };
                    let pull_request =
                        status.pull_request.map(|pull_request| GitPullRequestState {
                            number: Some(pull_request.number),
                            title: pull_request.title,
                            url: pull_request.url,
                            base_branch: pull_request.base_branch,
                            head_branch: pull_request.head_branch,
                            is_draft: pull_request.is_draft,
                        });
                    emit(
                        events,
                        Action::GitPullRequestStatusLoaded {
                            branch: branch.clone(),
                            provider,
                            pull_request,
                        },
                    );
                }
                Err(error) => {
                    emit(
                        events,
                        Action::GitPullRequestStatusLoaded {
                            branch: branch.clone(),
                            provider: GitPullRequestProvider::Unavailable,
                            pull_request: None,
                        },
                    );
                    emit(
                        events,
                        Action::SetStatus(bounded(
                            format!("Failed to inspect pull requests: {error}"),
                            MAX_STATUS_BYTES,
                        )),
                    );
                }
            }
            return;
        }
        Effect::CreateGitPullRequest {
            root,
            branch,
            base_branch,
            title,
            body,
            include_local_changes,
            next_step,
            is_draft,
            open_in_browser,
            force_push,
            commit_instructions,
            pull_request_instructions,
        } => {
            let commits = *next_step == GitPullRequestNextStep::CommitPushAndCreate;
            let pushes = *next_step != GitPullRequestNextStep::Create;
            let generated = if commits || title.trim().is_empty() || body.trim().is_empty() {
                let Some(app_server) = connection.as_ref() else {
                    emit(
                        events,
                        Action::GitPullRequestFailed {
                            message: "Failed to generate pull request title and body: app-server is unavailable."
                                .to_owned(),
                        },
                    );
                    return;
                };
                let result = if commits {
                    generate_commit_pull_request_messages(
                        app_server,
                        root,
                        base_branch,
                        branch,
                        title,
                        body,
                        commit_instructions,
                        pull_request_instructions,
                        events,
                        pending_approvals,
                        computer_permissions,
                        computer_allowed_app_ids,
                        computer_accessibility,
                        computer_url_policy,
                    )
                } else {
                    generate_pull_request_message(
                        app_server,
                        root,
                        base_branch,
                        branch,
                        title,
                        body,
                        *include_local_changes,
                        pull_request_instructions,
                        events,
                        pending_approvals,
                        computer_permissions,
                        computer_allowed_app_ids,
                        computer_accessibility,
                        computer_url_policy,
                    )
                    .map(|details| GeneratedGitMessages {
                        commit_message: None,
                        title: details.title,
                        body: details.body,
                    })
                };
                match result {
                    Ok(generated) => generated,
                    Err(error) => {
                        emit(
                            events,
                            Action::GitPullRequestFailed {
                                message: bounded(
                                    format!(
                                        "Failed to generate pull request title and body: {error}"
                                    ),
                                    MAX_STATUS_BYTES,
                                ),
                            },
                        );
                        return;
                    }
                }
            } else {
                GeneratedGitMessages {
                    commit_message: None,
                    title: title.trim().to_owned(),
                    body: body.trim().to_owned(),
                }
            };

            if commits {
                emit(events, Action::GitPullRequestCommitStarted);
                let Some(message) = generated.commit_message.as_deref() else {
                    emit(
                        events,
                        Action::GitPullRequestFailed {
                            message: "Couldn't generate commit and pull request messages."
                                .to_owned(),
                        },
                    );
                    return;
                };
                if let Err(error) = git_commit(root, message, true) {
                    emit(
                        events,
                        Action::GitPullRequestFailed {
                            message: bounded(
                                format!("Failed to commit changes: {error}"),
                                MAX_STATUS_BYTES,
                            ),
                        },
                    );
                    return;
                }
            }
            if pushes {
                emit(events, Action::GitPullRequestPushStarted);
                if let Err(error) = git_push(root, *force_push) {
                    emit(
                        events,
                        Action::GitPullRequestFailed {
                            message: bounded(
                                format!("Failed to push changes: {error}"),
                                MAX_STATUS_BYTES,
                            ),
                        },
                    );
                    return;
                }
            }

            emit(events, Action::GitPullRequestCreateStarted);
            match github_create_pull_request(
                root,
                &GitHubCreatePullRequest {
                    head_branch: branch.clone(),
                    base_branch: Some(base_branch.clone()),
                    is_draft: *is_draft,
                    open_in_browser: *open_in_browser,
                    title: generated.title,
                    body: generated.body,
                },
            ) {
                Ok(pull_request) => {
                    emit(
                        events,
                        Action::GitPullRequestCompleted {
                            pull_request: GitPullRequestState {
                                number: pull_request.number,
                                title: pull_request.title,
                                url: pull_request.url,
                                base_branch: base_branch.clone(),
                                head_branch: branch.clone(),
                                is_draft: *is_draft && !*open_in_browser,
                            },
                            open_in_browser: *open_in_browser,
                        },
                    );
                    emit(events, Action::RefreshGit);
                }
                Err(error) => {
                    let message = match error {
                        GitHubError::CliMissing
                        | GitHubError::AuthenticationRequired
                        | GitHubError::CreateFailed(_)
                        | GitHubError::OpenFailed(_) => error.to_string(),
                        _ => format!("Failed to create pull request: {error}"),
                    };
                    emit(
                        events,
                        Action::GitPullRequestFailed {
                            message: bounded(message, MAX_STATUS_BYTES),
                        },
                    );
                }
            }
            return;
        }
        Effect::SwitchGitBranch { root, branch } => {
            match git_switch_branch(root, branch) {
                Ok(GitBranchMutationOutcome::Switched) => {
                    emit(
                        events,
                        Action::GitBranchMutationCompleted {
                            root: root.clone(),
                            message: format!("Switched to branch {branch}"),
                        },
                    );
                }
                Ok(GitBranchMutationOutcome::Blocked { paths, truncated }) => emit(
                    events,
                    Action::GitBranchSwitchBlocked {
                        root: root.clone(),
                        branch: branch.clone(),
                        create_branch: false,
                        paths,
                        truncated,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::GitBranchMutationFailed {
                        root: root.clone(),
                        message: format!("Failed to switch branch: {error}"),
                    },
                ),
            }
            return;
        }
        Effect::CreateGitBranch { root, branch } => {
            match git_create_branch(root, branch) {
                Ok(GitBranchMutationOutcome::Switched) => {
                    emit(
                        events,
                        Action::GitBranchMutationCompleted {
                            root: root.clone(),
                            message: format!("Created and checked out {branch}"),
                        },
                    );
                }
                Ok(GitBranchMutationOutcome::Blocked { paths, truncated }) => emit(
                    events,
                    Action::GitBranchSwitchBlocked {
                        root: root.clone(),
                        branch: branch.clone(),
                        create_branch: true,
                        paths,
                        truncated,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::GitBranchMutationFailed {
                        root: root.clone(),
                        message: format!("Failed to create branch: {error}"),
                    },
                ),
            }
            return;
        }
        Effect::CreateGitWorktree {
            root,
            path,
            branch,
            create_branch,
        } => {
            match git_create_worktree(root, path, branch, *create_branch) {
                Ok(()) => {
                    emit(
                        events,
                        Action::SetStatus(format!("worktree created for {branch}")),
                    );
                    emit(events, Action::RefreshGit);
                }
                Err(error) => emit(
                    events,
                    Action::SetStatus(format!("failed to create worktree: {error}")),
                ),
            }
            return;
        }
        Effect::SpawnTerminal { tab_id, cwd, shell } => {
            start_terminal(*tab_id, cwd.clone(), *shell, terminals, events);
            return;
        }
        Effect::RestartTerminal { tab_id, cwd, shell } => {
            if let Some(mut runtime) = terminals.remove(tab_id) {
                runtime.session.shutdown();
            }
            start_terminal(*tab_id, cwd.clone(), *shell, terminals, events);
            return;
        }
        Effect::WriteTerminal { tab_id, input } => {
            if let Some(runtime) = terminals.get(tab_id) {
                let mut bytes = input.as_bytes().to_vec();
                bytes.push(b'\r');
                if let Err(error) = runtime.session.write(&bytes) {
                    emit(
                        events,
                        Action::SetStatus(format!("terminal input failed: {error}")),
                    );
                }
            }
            return;
        }
        Effect::StopTerminal { tab_id } => {
            if let Some(mut runtime) = terminals.remove(tab_id) {
                runtime.session.shutdown();
            }
            emit(
                events,
                Action::TerminalExited {
                    tab_id: *tab_id,
                    code: 0,
                },
            );
            return;
        }
        Effect::ConfigureComputerUse {
            task_id,
            enabled,
            selected_window_id: _,
            selected_application_id,
            input_authorized,
        } => {
            if computer_capable_threads.contains(task_id) {
                computer_permissions.insert(
                    task_id.clone(),
                    ComputerUsePermission {
                        enabled: *enabled,
                        authorized_application_id: input_authorized
                            .then(|| selected_application_id.clone())
                            .flatten(),
                        input_authorized: *input_authorized,
                    },
                );
            }
            return;
        }
        Effect::LoadComposerDesktopApps => {
            let applications = computer_accessibility
                .list_apps()
                .map(|applications| {
                    applications
                        .into_iter()
                        .map(map_computer_application)
                        .collect()
                })
                .unwrap_or_default();
            emit(events, Action::ComposerDesktopAppsLoaded(applications));
            return;
        }
        Effect::LoadComputerWindows { task_id } => {
            match computer_accessibility.list_apps() {
                Ok(applications) => {
                    let windows = applications
                        .iter()
                        .flat_map(|application| application.windows.iter().cloned())
                        .map(map_computer_window)
                        .collect();
                    emit(
                        events,
                        Action::ComputerWindowsLoaded {
                            task_id: task_id.clone(),
                            applications: applications
                                .into_iter()
                                .map(map_computer_application)
                                .collect(),
                            windows,
                        },
                    );
                }
                Err(error) => emit(
                    events,
                    Action::ComputerWindowsFailed {
                        task_id: task_id.clone(),
                        message: error.to_string(),
                    },
                ),
            }
            return;
        }
        Effect::LaunchComputerApp {
            task_id,
            application_id,
        } => {
            let error = computer_accessibility
                .launch_app(application_id)
                .err()
                .map(|error| error.to_string());
            emit(
                events,
                Action::ComputerAppLaunchFinished {
                    task_id: task_id.clone(),
                    application_id: application_id.clone(),
                    error,
                },
            );
            return;
        }
        Effect::CaptureComputerWindow { task_id, window_id } => {
            match capture_computer_window(window_id) {
                Ok(capture) => emit(
                    events,
                    Action::ComputerCaptureReady {
                        task_id: task_id.clone(),
                        label: capture_label(&capture),
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::ComputerWindowsFailed {
                        task_id: task_id.clone(),
                        message: error.to_string(),
                    },
                ),
            }
            return;
        }
        _ => {}
    }

    let Some(app_server) = connection.as_ref() else {
        match effect {
            Effect::LoadRemoteControlStatus { generation } => emit(
                events,
                Action::RemoteControlStatusFailed {
                    generation,
                    message: REMOTE_CONTROL_STATUS_FAILED.to_owned(),
                },
            ),
            Effect::SetRemoteControlEnabled { generation, .. }
            | Effect::RevokeRemoteDevice { generation, .. } => emit(
                events,
                Action::RemoteControlMutationFailed {
                    generation,
                    message: REMOTE_CONTROL_MUTATION_FAILED.to_owned(),
                },
            ),
            Effect::StartRemotePairing { generation, .. }
            | Effect::CheckRemotePairing { generation, .. } => emit(
                events,
                Action::RemotePairingFailed {
                    generation,
                    message: REMOTE_PAIRING_FAILED.to_owned(),
                },
            ),
            Effect::LoadRemoteDevices {
                generation,
                environment_id,
                append,
                ..
            } => emit(
                events,
                Action::RemoteDevicesFailed {
                    generation,
                    environment_id,
                    message: REMOTE_DEVICES_FAILED.to_owned(),
                    append,
                },
            ),
            Effect::LoadBranchDiff { generation, .. } => emit(
                events,
                Action::BranchDiffFailed {
                    generation,
                    message: "App server is unavailable.".to_owned(),
                },
            ),
            Effect::RetryPendingWorktreeFork { request_id, .. } => emit(
                events,
                Action::PendingWorktreeForkConversationFailed {
                    request_id,
                    message: "app-server is unavailable".to_owned(),
                },
            ),
            _ => emit(events, Action::ConnectionLost),
        }
        return;
    };

    match effect {
        Effect::ConnectAppServer => {}
        Effect::ScheduleAppServerReconnect => {}
        Effect::ScheduleTaskSearch { .. } => {}
        Effect::SearchFuzzyFiles {
            session_id,
            roots,
            query,
            start_session,
        } => search_fuzzy_files(
            app_server,
            events,
            fuzzy_file_search,
            session_id,
            roots,
            query,
            start_session,
        ),
        Effect::StopFuzzyFileSearch { session_id } => {
            stop_fuzzy_file_search(app_server, fuzzy_file_search, &session_id);
        }
        Effect::LoadComputerUsePolicy => {
            #[cfg(windows)]
            match app_server.read_config(ConfigReadParams {
                include_layers: true,
                cwd: None,
            }) {
                Ok(config) => {
                    let app_ids = computer_use_allowed_app_ids(&config);
                    *computer_allowed_app_ids = app_ids.iter().cloned().collect();
                    emit(events, Action::ComputerUsePolicyLoaded(app_ids));
                }
                Err(error) => emit(
                    events,
                    Action::ComputerUsePolicyFailed(format!(
                        "failed to load Computer Use app policy: {error}"
                    )),
                ),
            }

            #[cfg(not(windows))]
            {
                computer_allowed_app_ids.clear();
                emit(events, Action::ComputerUsePolicyLoaded(Vec::new()));
            }
        }
        Effect::LoadRemoteControlStatus { generation } => {
            match app_server.read_remote_control_status() {
                Ok(response) => {
                    match map_remote_control_snapshot(response.status, response.environment_id) {
                        Ok((status, environment_id)) => emit(
                            events,
                            Action::RemoteControlStatusLoaded {
                                generation,
                                status,
                                environment_id,
                            },
                        ),
                        Err(()) => emit(
                            events,
                            Action::RemoteControlStatusFailed {
                                generation,
                                message: REMOTE_CONTROL_INVALID_RESPONSE.to_owned(),
                            },
                        ),
                    }
                }
                Err(_) => emit(
                    events,
                    Action::RemoteControlStatusFailed {
                        generation,
                        message: REMOTE_CONTROL_STATUS_FAILED.to_owned(),
                    },
                ),
            }
        }
        Effect::SetRemoteControlEnabled {
            generation,
            enabled,
        } => {
            let result = if enabled {
                app_server.enable_remote_control(None)
            } else {
                app_server.disable_remote_control(None)
            };
            match result {
                Ok(response) => {
                    match map_remote_control_snapshot(response.status, response.environment_id) {
                        Ok((status, environment_id)) => emit(
                            events,
                            Action::RemoteControlEnabledChanged {
                                generation,
                                enabled,
                                status,
                                environment_id,
                            },
                        ),
                        Err(()) => emit(
                            events,
                            Action::RemoteControlMutationFailed {
                                generation,
                                message: REMOTE_CONTROL_INVALID_RESPONSE.to_owned(),
                            },
                        ),
                    }
                }
                Err(_) => emit(
                    events,
                    Action::RemoteControlMutationFailed {
                        generation,
                        message: REMOTE_CONTROL_MUTATION_FAILED.to_owned(),
                    },
                ),
            }
        }
        Effect::StartRemotePairing {
            generation,
            manual_code,
        } => {
            match app_server
                .start_remote_control_pairing(RemoteControlPairingStartParams { manual_code })
            {
                Ok(response) => match map_remote_pairing(
                    response.pairing_code,
                    response.manual_pairing_code,
                    response.environment_id,
                    response.expires_at,
                ) {
                    Ok(pairing) => emit(
                        events,
                        Action::RemotePairingStarted {
                            generation,
                            pairing,
                        },
                    ),
                    Err(()) => emit(
                        events,
                        Action::RemotePairingFailed {
                            generation,
                            message: REMOTE_CONTROL_INVALID_RESPONSE.to_owned(),
                        },
                    ),
                },
                Err(_) => emit(
                    events,
                    Action::RemotePairingFailed {
                        generation,
                        message: REMOTE_PAIRING_FAILED.to_owned(),
                    },
                ),
            }
        }
        Effect::CheckRemotePairing {
            generation,
            pairing,
        } => match app_server
            .read_remote_control_pairing_status(remote_pairing_status_params(&pairing))
        {
            Ok(response) => emit(
                events,
                Action::RemotePairingChecked {
                    generation,
                    claimed: response.claimed,
                },
            ),
            Err(_) => emit(
                events,
                Action::RemotePairingFailed {
                    generation,
                    message: REMOTE_PAIRING_FAILED.to_owned(),
                },
            ),
        },
        Effect::LoadRemoteDevices {
            generation,
            environment_id,
            cursor,
            limit,
            append,
        } => {
            let params = RemoteControlClientsListParams {
                environment_id: environment_id.clone(),
                cursor,
                limit: Some(limit),
                order: None,
            };
            match app_server.list_remote_control_clients(params) {
                Ok(response) => {
                    match map_remote_devices_page(response.data, response.next_cursor, limit) {
                        Ok((devices, next_cursor)) => emit(
                            events,
                            Action::RemoteDevicesLoaded {
                                generation,
                                environment_id,
                                devices,
                                next_cursor,
                                append,
                            },
                        ),
                        Err(()) => emit(
                            events,
                            Action::RemoteDevicesFailed {
                                generation,
                                environment_id,
                                message: REMOTE_CONTROL_INVALID_RESPONSE.to_owned(),
                                append,
                            },
                        ),
                    }
                }
                Err(_) => emit(
                    events,
                    Action::RemoteDevicesFailed {
                        generation,
                        environment_id,
                        message: REMOTE_DEVICES_FAILED.to_owned(),
                        append,
                    },
                ),
            }
        }
        Effect::RevokeRemoteDevice {
            generation,
            environment_id,
            client_id,
        } => match app_server.revoke_remote_control_client(RemoteControlClientsRevokeParams {
            environment_id: environment_id.clone(),
            client_id: client_id.clone(),
        }) {
            Ok(_) => emit(
                events,
                Action::RemoteDeviceRevoked {
                    generation,
                    environment_id,
                    client_id,
                },
            ),
            Err(_) => emit(
                events,
                Action::RemoteControlMutationFailed {
                    generation,
                    message: REMOTE_CONTROL_MUTATION_FAILED.to_owned(),
                },
            ),
        },
        Effect::LoadAccount => {
            match app_server.read_account(GetAccountParams {
                refresh_token: Some(false),
            }) {
                Ok(response) => {
                    let (usage_limits, credits, usage_error) = if response.account.is_some() {
                        match app_server.read_account_rate_limits() {
                            Ok(response) => {
                                let snapshot = response.rate_limits;
                                let usage_limits = [snapshot.primary, snapshot.secondary]
                                    .into_iter()
                                    .flatten()
                                    .map(map_usage_limit_window)
                                    .collect();
                                let credits = snapshot.credits.map(|credits| AccountCredits {
                                    has_credits: credits.has_credits,
                                    unlimited: credits.unlimited,
                                    balance: credits.balance.map(|balance| bounded(balance, 512)),
                                });
                                (usage_limits, credits, None)
                            }
                            Err(_) => (
                                Vec::new(),
                                None,
                                Some("Could not load usage limits.".to_owned()),
                            ),
                        }
                    } else {
                        (Vec::new(), None, None)
                    };
                    emit(
                        events,
                        Action::AccountLoaded {
                            profile: response.account.map(map_account_profile),
                            requires_openai_auth: response.requires_openai_auth,
                            usage_limits,
                            credits,
                            usage_error,
                        },
                    );
                }
                Err(_) => emit(
                    events,
                    Action::AccountLoadFailed("Could not load account details.".to_owned()),
                ),
            }
        }
        Effect::StartAccountLogin => {
            let params = LoginAccountParams::ChatGpt {
                codex_streamlined_login: None,
                use_hosted_login_success_page: None,
                app_brand: None,
            };
            match app_server.start_account_login(params) {
                Ok(LoginAccountResponse::ChatGpt { login_id, auth_url }) => emit(
                    events,
                    Action::AccountLoginStarted {
                        login_id: bounded(login_id, 512),
                        authorization_url: bounded(auth_url, 8 * 1024),
                    },
                ),
                Ok(_) | Err(_) => emit(
                    events,
                    Action::AccountLoginStartFailed(
                        "Could not start ChatGPT sign-in. Please try again.".to_owned(),
                    ),
                ),
            }
        }
        Effect::CancelAccountLogin { login_id } => {
            let result = app_server.cancel_account_login(CancelLoginAccountParams {
                login_id: login_id.clone(),
            });
            match result {
                Ok(_) => emit(events, Action::AccountLoginCanceled { login_id }),
                Err(_) => emit(
                    events,
                    Action::AccountLoginCancelFailed(
                        "Could not cancel ChatGPT sign-in. Please try again.".to_owned(),
                    ),
                ),
            }
        }
        Effect::LogoutAccount => match app_server.logout_account() {
            Ok(_) => emit(events, Action::AccountLoggedOut),
            Err(_) => emit(
                events,
                Action::AccountLogoutFailed("Could not log out. Please try again.".to_owned()),
            ),
        },
        Effect::LoadBranchDiff {
            generation,
            cwd,
            base,
        } => {
            if let Some(base) = base {
                match git_branch_diff(&cwd, &base) {
                    Ok(response) => emit(
                        events,
                        Action::BranchDiffLoaded {
                            generation,
                            base_sha: bounded(response.base_sha, MAX_GIT_SHA_BYTES),
                            text: bounded(response.text, MAX_GIT_DIFF_BYTES),
                            truncated: response.truncated,
                        },
                    ),
                    Err(_) => emit(
                        events,
                        Action::BranchDiffFailed {
                            generation,
                            message: format!("Could not compare this branch with {base}."),
                        },
                    ),
                }
            } else {
                match app_server.git_diff_to_remote(GitDiffToRemoteParams { cwd }) {
                    Ok(response) => {
                        let truncated = response.diff.len() > MAX_GIT_DIFF_BYTES;
                        emit(
                            events,
                            Action::BranchDiffLoaded {
                                generation,
                                base_sha: bounded(response.sha, MAX_GIT_SHA_BYTES),
                                text: bounded(response.diff, MAX_GIT_DIFF_BYTES),
                                truncated,
                            },
                        );
                    }
                    Err(_) => emit(
                        events,
                        Action::BranchDiffFailed {
                            generation,
                            message: "Could not load changes for this branch.".to_owned(),
                        },
                    ),
                }
            }
        }
        Effect::SubmitFeedback {
            classification,
            reason,
            include_logs,
            thread_id,
        } => {
            let response = app_server.upload_feedback(FeedbackUploadParams {
                classification: classification.as_str().to_owned(),
                extra_log_files: None,
                include_logs: Some(include_logs),
                reason: Some(reason),
                tags: Some(BTreeMap::from([(
                    "app_version".to_owned(),
                    env!("CARGO_PKG_VERSION").to_owned(),
                )])),
                thread_id,
            });
            match response {
                Ok(response) => emit(
                    events,
                    Action::FeedbackSubmitted {
                        feedback_id: bounded(response.thread_id, 512),
                    },
                ),
                Err(_) => emit(
                    events,
                    Action::FeedbackFailed(
                        "We couldn't submit your feedback. Please try again in a moment."
                            .to_owned(),
                    ),
                ),
            }
        }
        Effect::RemoveComputerUseAllowedApp {
            app_id,
            remaining_app_ids,
        } => {
            #[cfg(windows)]
            {
                let result = app_server.batch_write_config(ConfigBatchWriteParams {
                    edits: vec![ConfigEdit {
                        key_path: "computer_use.windows.always_allowed_app_ids".to_owned(),
                        value: computer_use_allowed_app_ids_value(&remaining_app_ids),
                        merge_strategy: ConfigMergeStrategy::Upsert,
                    }],
                    file_path: None,
                    expected_version: None,
                    reload_user_config: true,
                });
                match result {
                    Ok(response) => {
                        *computer_allowed_app_ids = remaining_app_ids.iter().cloned().collect();
                        emit(
                            events,
                            Action::ComputerUseAllowedAppRemoved {
                                app_id,
                                overridden: response.status == ConfigWriteStatus::OkOverridden,
                            },
                        );
                    }
                    Err(error) => emit(
                        events,
                        Action::ComputerUsePolicyMutationFailed {
                            app_id,
                            message: format!("failed to update Computer Use app policy: {error}"),
                        },
                    ),
                }
            }

            #[cfg(not(windows))]
            {
                let _ = remaining_app_ids;
                emit(
                    events,
                    Action::ComputerUsePolicyMutationFailed {
                        app_id,
                        message:
                            "Persistent Computer Use app approval is unavailable on this platform."
                                .to_owned(),
                    },
                );
            }
        }
        Effect::LoadAgentConfiguration { cwd } => {
            let cwd_string = cwd
                .as_ref()
                .map(|path| bounded(path.display().to_string(), MAX_CONFIG_PATH_BYTES));
            match app_server.read_config(ConfigReadParams {
                include_layers: true,
                cwd: cwd_string,
            }) {
                Ok(config) => match app_server.read_config_requirements() {
                    Ok(requirements) => {
                        let snapshot = agent_configuration_snapshot(
                            &config,
                            requirements.requirements.as_ref(),
                        );
                        emit(
                            events,
                            Action::AgentConfigurationLoaded {
                                cwd,
                                scopes: snapshot.scopes,
                                effective_approval_policy: snapshot.effective_approval_policy,
                                effective_sandbox_mode: snapshot.effective_sandbox_mode,
                                effective_network_access: snapshot.effective_network_access,
                                allowed_approval_policies: snapshot.allowed_approval_policies,
                                allowed_sandbox_modes: snapshot.allowed_sandbox_modes,
                                approval_managed: snapshot.approval_managed,
                                sandbox_managed: snapshot.sandbox_managed,
                                network_managed: snapshot.network_managed,
                            },
                        );
                    }
                    Err(error) => emit(
                        events,
                        Action::AgentConfigurationFailed(format!(
                            "Unable to load configuration requirements: {error}"
                        )),
                    ),
                },
                Err(error) => emit(
                    events,
                    Action::AgentConfigurationFailed(format!(
                        "Unable to load configuration settings: {error}"
                    )),
                ),
            }
        }
        Effect::SetAgentApprovalPolicy {
            value,
            file_path,
            expected_version,
        } => {
            let result = write_agent_config_value(
                app_server,
                "approval_policy",
                json!(value),
                file_path,
                expected_version,
            );
            emit_agent_configuration_mutation(
                events,
                AgentConfigurationMutationKind::ApprovalPolicy,
                result,
            );
        }
        Effect::SetAgentSandboxMode {
            value,
            file_path,
            expected_version,
        } => {
            let result = write_agent_config_value(
                app_server,
                "sandbox_mode",
                json!(value),
                file_path,
                expected_version,
            );
            emit_agent_configuration_mutation(
                events,
                AgentConfigurationMutationKind::SandboxMode,
                result,
            );
        }
        Effect::SetAgentNetworkAccess {
            value,
            file_path,
            expected_version,
        } => {
            let result = write_agent_config_value(
                app_server,
                "sandbox_workspace_write.network_access",
                json!(value),
                file_path,
                expected_version,
            );
            emit_agent_configuration_mutation(
                events,
                AgentConfigurationMutationKind::NetworkAccess,
                result,
            );
        }
        Effect::LoadPersonalization => {
            match app_server.read_config(ConfigReadParams {
                include_layers: false,
                cwd: None,
            }) {
                Ok(config) => {
                    let snapshot = personalization_snapshot(&config);
                    *personality = snapshot.personality;
                    emit(
                        events,
                        Action::PersonalizationLoaded {
                            personality: snapshot.personality,
                            memory_available: snapshot.memory_available,
                            generate_memories: snapshot.generate_memories,
                            use_memories: snapshot.use_memories,
                            memories_enabled: snapshot.memories_enabled,
                            allow_memory_generation_from_tool_assisted_chats: snapshot
                                .allow_memory_generation_from_tool_assisted_chats,
                        },
                    );
                }
                Err(error) => emit(
                    events,
                    Action::PersonalizationFailed(format!(
                        "Unable to load personalization settings: {error}"
                    )),
                ),
            }
        }
        Effect::SetPersonality(selected) => {
            let result = app_server.batch_write_config(ConfigBatchWriteParams {
                edits: vec![ConfigEdit {
                    key_path: "personality".to_owned(),
                    value: json!(selected.as_str()),
                    merge_strategy: ConfigMergeStrategy::Upsert,
                }],
                file_path: None,
                expected_version: None,
                reload_user_config: true,
            });
            match result {
                Ok(response) => {
                    if response.status == ConfigWriteStatus::Ok {
                        *personality = selected;
                    }
                    emit(
                        events,
                        Action::PersonalizationMutationFinished {
                            kind: PersonalizationMutationKind::Personality,
                            overridden: response.status == ConfigWriteStatus::OkOverridden,
                        },
                    );
                }
                Err(error) => emit(
                    events,
                    Action::PersonalizationMutationFailed {
                        kind: PersonalizationMutationKind::Personality,
                        message: error.to_string(),
                    },
                ),
            }
        }
        Effect::SetMemoriesEnabled(enabled) => {
            let result = app_server.batch_write_config(ConfigBatchWriteParams {
                edits: vec![
                    ConfigEdit {
                        key_path: "memories.generate_memories".to_owned(),
                        value: json!(enabled),
                        merge_strategy: ConfigMergeStrategy::Upsert,
                    },
                    ConfigEdit {
                        key_path: "memories.use_memories".to_owned(),
                        value: json!(enabled),
                        merge_strategy: ConfigMergeStrategy::Upsert,
                    },
                ],
                file_path: None,
                expected_version: None,
                reload_user_config: true,
            });
            match result {
                Ok(response) => emit(
                    events,
                    Action::PersonalizationMutationFinished {
                        kind: PersonalizationMutationKind::Memories,
                        overridden: response.status == ConfigWriteStatus::OkOverridden,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::PersonalizationMutationFailed {
                        kind: PersonalizationMutationKind::Memories,
                        message: error.to_string(),
                    },
                ),
            }
        }
        Effect::SetToolAssistedMemoriesEnabled(enabled) => {
            let result = app_server.batch_write_config(ConfigBatchWriteParams {
                edits: vec![
                    ConfigEdit {
                        key_path: "memories.disable_on_external_context".to_owned(),
                        value: json!(!enabled),
                        merge_strategy: ConfigMergeStrategy::Upsert,
                    },
                    ConfigEdit {
                        key_path: "memories.no_memories_if_mcp_or_web_search".to_owned(),
                        value: Value::Null,
                        merge_strategy: ConfigMergeStrategy::Replace,
                    },
                ],
                file_path: None,
                expected_version: None,
                reload_user_config: true,
            });
            match result {
                Ok(response) => emit(
                    events,
                    Action::PersonalizationMutationFinished {
                        kind: PersonalizationMutationKind::ToolAssistedMemories,
                        overridden: response.status == ConfigWriteStatus::OkOverridden,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::PersonalizationMutationFailed {
                        kind: PersonalizationMutationKind::ToolAssistedMemories,
                        message: error.to_string(),
                    },
                ),
            }
        }
        Effect::SetThreadMemoryMode {
            task_id,
            enabled,
            previous,
        } => {
            let mode = if enabled {
                ThreadMemoryMode::Enabled
            } else {
                ThreadMemoryMode::Disabled
            };
            match app_server.set_thread_memory_mode(ThreadMemoryModeSetParams {
                thread_id: task_id.clone(),
                mode,
            }) {
                Ok(_) => emit(events, Action::ThreadMemoryModeSetFinished { task_id }),
                Err(error) => emit(
                    events,
                    Action::ThreadMemoryModeSetFailed {
                        task_id,
                        previous,
                        message: error.to_string(),
                    },
                ),
            }
        }
        Effect::ResetMemories => match app_server.reset_memories() {
            Ok(_) => emit(events, Action::MemoriesReset),
            Err(error) => emit(events, Action::MemoryResetFailed(error.to_string())),
        },
        Effect::DetectExternalAgentConfig { cwds } => {
            let cwds = cwds
                .into_iter()
                .take(MAX_FUZZY_FILE_ROOTS)
                .map(|cwd| bounded(cwd.to_string_lossy().into_owned(), MAX_IMPORT_FIELD_BYTES))
                .collect::<Vec<_>>();
            let mut providers = Vec::new();
            let mut failures = Vec::new();
            let mut successful_requests = 0_usize;
            let mut remaining = MAX_IMPORT_MIGRATION_ITEMS;
            for provider in ImportProvider::ALL {
                let response =
                    app_server.detect_external_agent_config(ExternalAgentConfigDetectParams {
                        cwds: Some(cwds.clone()),
                        include_home: Some(true),
                        max_session_age_days: Some(MAX_IMPORT_SESSION_AGE_DAYS),
                        max_sessions: Some(MAX_IMPORT_SESSIONS),
                        migration_source: Some(provider.migration_source().to_owned()),
                    });
                match response {
                    Ok(response) => {
                        successful_requests = successful_requests.saturating_add(1);
                        let items = response
                            .items
                            .into_iter()
                            .take(remaining)
                            .map(map_import_item)
                            .collect::<Vec<_>>();
                        remaining = remaining.saturating_sub(items.len());
                        if !items.is_empty() {
                            providers.push(ImportProviderItems {
                                provider,
                                items,
                                selected: true,
                            });
                        }
                    }
                    Err(_) => failures.push(provider),
                }
            }
            if successful_requests == 0 {
                emit(
                    events,
                    Action::ImportDetectionFailed(
                        "Couldn't check for imports. Try again.".to_owned(),
                    ),
                );
            } else {
                emit(
                    events,
                    Action::ImportDetectionLoaded {
                        providers,
                        failures,
                    },
                );
            }
        }
        Effect::LoadExternalAgentImportHistories => {
            match app_server.read_external_agent_import_histories() {
                Ok(response) => {
                    let (histories, connectors) = map_import_histories(response);
                    emit(
                        events,
                        Action::ImportHistoriesLoaded {
                            histories,
                            connectors,
                        },
                    );
                }
                Err(_) => emit(
                    events,
                    Action::ImportHistoriesFailed(
                        "Couldn't load import history. Try again.".to_owned(),
                    ),
                ),
            }
        }
        Effect::ImportExternalAgentConfig { batches } => {
            let mut imports = Vec::new();
            let mut failures = Vec::new();
            for batch in batches.into_iter().take(ImportProvider::ALL.len()) {
                let provider = batch.provider;
                let migration_items = batch
                    .items
                    .into_iter()
                    .take(MAX_IMPORT_MIGRATION_ITEMS)
                    .map(protocol_import_item)
                    .collect::<Vec<_>>();
                if migration_items.is_empty() {
                    continue;
                }
                match app_server.import_external_agent_config(ExternalAgentConfigImportParams {
                    migration_items,
                    migration_source: Some(provider.migration_source().to_owned()),
                    provider_id: Some(provider.migration_source().to_owned()),
                    source: Some("settings".to_owned()),
                }) {
                    Ok(response) => imports.push(StartedImport {
                        import_id: bounded(response.import_id, MAX_IMPORT_FIELD_BYTES),
                        wait_for_completion: batch.wait_for_completion,
                    }),
                    Err(_) => failures.push(ImportStartFailure { provider }),
                }
            }
            emit(events, Action::ExternalImportsStarted { imports, failures });
        }
        Effect::LoadModels => {
            match app_server.list_models(ModelListParams {
                cursor: None,
                limit: Some(COMPOSER_OPTIONS_PAGE_LIMIT),
                include_hidden: Some(false),
            }) {
                Ok(response) => emit(
                    events,
                    Action::ModelsLoaded(
                        response
                            .data
                            .into_iter()
                            .filter(|model| !model.hidden)
                            .map(|model| ModelOption {
                                id: model.model,
                                display_name: model.display_name,
                                description: model.description,
                                is_default: model.is_default,
                                default_effort: model.default_reasoning_effort,
                                supported_efforts: model
                                    .supported_reasoning_efforts
                                    .into_iter()
                                    .take(COMPOSER_OPTIONS_PAGE_LIMIT as usize)
                                    .map(|effort| CoreReasoningEffortOption {
                                        id: effort.reasoning_effort,
                                        description: effort.description,
                                    })
                                    .collect(),
                                service_tiers: model
                                    .service_tiers
                                    .into_iter()
                                    .take(COMPOSER_OPTIONS_PAGE_LIMIT as usize)
                                    .map(|tier| ServiceTierOption {
                                        id: tier.id,
                                        name: tier.name,
                                        description: tier.description,
                                    })
                                    .collect(),
                                default_service_tier: model.default_service_tier,
                            })
                            .collect(),
                    ),
                ),
                Err(error) => emit(
                    events,
                    Action::ModelsFailed(format!("failed to load models: {error}")),
                ),
            }
        }
        Effect::LoadPermissionProfiles { cwd } => {
            let cwd = cwd.map(|path| path.display().to_string());
            match app_server.list_permission_profiles(PermissionProfileListParams {
                cursor: None,
                limit: Some(COMPOSER_OPTIONS_PAGE_LIMIT),
                cwd: cwd.clone(),
            }) {
                Ok(response) => match app_server.read_config_requirements() {
                    Ok(requirements_response) => {
                        let requirements = requirements_response.requirements;
                        let default_permissions = requirements
                            .as_ref()
                            .and_then(|requirements| requirements.default_permissions.clone());
                        let managed_defaults = requirements
                            .as_ref()
                            .and_then(|requirements| requirements.models.as_ref())
                            .and_then(|models| models.new_thread.as_ref())
                            .cloned();
                        emit(
                            events,
                            Action::PermissionProfilesLoaded {
                                profiles: response
                                    .data
                                    .into_iter()
                                    .map(|profile| PermissionProfileOption {
                                        id: profile.id,
                                        description: profile.description,
                                        allowed: profile.allowed,
                                    })
                                    .collect(),
                                requirements: PermissionRequirements {
                                    managed_allow_remote_control: requirements
                                        .as_ref()
                                        .and_then(|requirements| requirements.allow_remote_control),
                                    allowed_approval_policies: requirements
                                        .as_ref()
                                        .and_then(|requirements| {
                                            requirements.allowed_approval_policies.as_ref()
                                        })
                                        .map(|policies| {
                                            policies
                                                .iter()
                                                .filter_map(Value::as_str)
                                                .map(str::to_owned)
                                                .collect()
                                        }),
                                    allowed_approvals_reviewers: requirements
                                        .as_ref()
                                        .and_then(|requirements| {
                                            requirements.allowed_approvals_reviewers.as_ref()
                                        })
                                        .map(|reviewers| {
                                            reviewers
                                                .iter()
                                                .copied()
                                                .map(core_approvals_reviewer)
                                                .collect()
                                        }),
                                    default_permissions: default_permissions.clone(),
                                },
                            },
                        );
                        match app_server.read_config(ConfigReadParams {
                            include_layers: false,
                            cwd,
                        }) {
                            Ok(config_response) => emit(
                                events,
                                Action::ComposerDefaultsLoaded {
                                    model: managed_defaults
                                        .as_ref()
                                        .and_then(|defaults| defaults.model.clone())
                                        .or(config_response.config.model),
                                    effort: managed_defaults
                                        .as_ref()
                                        .and_then(|defaults| {
                                            defaults.model_reasoning_effort.clone()
                                        })
                                        .or(config_response.config.model_reasoning_effort),
                                    service_tier: managed_defaults
                                        .as_ref()
                                        .and_then(|defaults| defaults.service_tier.clone())
                                        .or(config_response.config.service_tier),
                                    profile: config_response.config.profile,
                                    has_managed_new_thread_settings: managed_defaults.is_some(),
                                    permissions: default_permissions,
                                    approval_policy: config_response
                                        .config
                                        .approval_policy
                                        .as_ref()
                                        .and_then(Value::as_str)
                                        .map(str::to_owned),
                                    approvals_reviewer: config_response
                                        .config
                                        .approvals_reviewer
                                        .map(core_approvals_reviewer),
                                },
                            ),
                            Err(error) => emit(
                                events,
                                Action::SetStatus(format!(
                                    "failed to load composer defaults: {error}"
                                )),
                            ),
                        }
                    }
                    Err(error) => emit(
                        events,
                        Action::PermissionProfilesFailed(format!(
                            "failed to load permission requirements: {error}"
                        )),
                    ),
                },
                Err(error) => emit(
                    events,
                    Action::PermissionProfilesFailed(format!(
                        "failed to load permission profiles: {error}"
                    )),
                ),
            }
        }
        Effect::LoadTasks { generation, cursor } => {
            let append = cursor.is_some();
            let params =
                ThreadListParams::state_db_page(DEFAULT_THREAD_PAGE_LIMIT).with_cursor(cursor);
            match app_server.list_threads(params) {
                Ok(page) => emit(
                    events,
                    Action::TasksLoaded {
                        generation,
                        tasks: page.data.into_iter().map(map_task).collect(),
                        next_cursor: page.next_cursor,
                        append,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::TasksFailed {
                        generation,
                        message: format!("failed to load tasks: {error}"),
                    },
                ),
            }
        }
        Effect::LoadLoadedTasks => {
            match app_server.list_loaded_threads(ThreadLoadedListParams {
                cursor: None,
                limit: DEFAULT_THREAD_PAGE_LIMIT,
            }) {
                Ok(page) => {
                    let truncated = page.next_cursor.is_some()
                        || page.data.len() > DEFAULT_THREAD_PAGE_LIMIT as usize;
                    let mut seen = HashSet::new();
                    let mut tasks = Vec::new();
                    let mut failed = 0usize;
                    for raw_task_id in page
                        .data
                        .into_iter()
                        .take(DEFAULT_THREAD_PAGE_LIMIT as usize)
                    {
                        let task_id = bounded(raw_task_id.trim().to_owned(), 512);
                        if task_id.is_empty() || !seen.insert(task_id.clone()) {
                            continue;
                        }
                        match app_server.read_thread(ThreadReadParams {
                            thread_id: task_id,
                            include_turns: false,
                        }) {
                            Ok(response) => tasks.push(map_task(response.thread)),
                            Err(_) => failed = failed.saturating_add(1),
                        }
                    }
                    emit(events, Action::LoadedTasksRestored { tasks, truncated });
                    if failed > 0 {
                        emit(
                            events,
                            Action::SetStatus(format!(
                                "{failed} loaded chat{} could not be restored",
                                if failed == 1 { "" } else { "s" }
                            )),
                        );
                    }
                }
                Err(_) => emit(
                    events,
                    Action::LoadedTasksRestoreFailed(
                        "Could not restore loaded chats after connecting.".to_owned(),
                    ),
                ),
            }
        }
        Effect::LoadPinnedTasks { task_ids } => {
            let mut tasks = Vec::with_capacity(task_ids.len());
            let mut failed = 0usize;
            for task_id in task_ids {
                match app_server.read_thread(ThreadReadParams {
                    thread_id: task_id,
                    include_turns: false,
                }) {
                    Ok(response) => tasks.push(map_task(response.thread)),
                    Err(_) => failed = failed.saturating_add(1),
                }
            }
            emit(events, Action::PinnedTasksLoaded(tasks));
            if failed > 0 {
                emit(
                    events,
                    Action::SetStatus(format!(
                        "{failed} pinned chat{} could not be loaded",
                        if failed == 1 { "" } else { "s" }
                    )),
                );
            }
        }
        Effect::SearchTasks {
            generation,
            query,
            cursor,
        } => {
            let append = cursor.is_some();
            let params = ThreadSearchParams::interactive_page(query, TASK_SEARCH_PAGE_LIMIT)
                .with_cursor(cursor);
            match app_server.search_threads(params) {
                Ok(page) => emit(
                    events,
                    Action::TaskSearchResultsLoaded {
                        generation,
                        results: page
                            .data
                            .into_iter()
                            .take(TASK_SEARCH_PAGE_LIMIT as usize)
                            .map(|result| TaskSearchResult {
                                task: map_task(result.thread),
                                snippet: bounded(result.snippet, MAX_TASK_SEARCH_SNIPPET_BYTES),
                            })
                            .collect(),
                        next_cursor: page.next_cursor,
                        append,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::TaskSearchFailed {
                        generation,
                        message: format!("failed to search chats: {error}"),
                    },
                ),
            }
        }
        Effect::LoadArchivedTasks { generation, cursor } => {
            let append = cursor.is_some();
            let mut params =
                ThreadListParams::state_db_page(DEFAULT_THREAD_PAGE_LIMIT).with_cursor(cursor);
            params.archived = Some(true);
            match app_server.list_threads(params) {
                Ok(page) => emit(
                    events,
                    Action::ArchivedTasksLoaded {
                        generation,
                        tasks: page.data.into_iter().map(map_task).collect(),
                        next_cursor: page.next_cursor,
                        append,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::ArchivedTasksFailed {
                        generation,
                        message: format!("failed to load archived chats: {error}"),
                    },
                ),
            }
        }
        Effect::CreateTask {
            cwd,
            model,
            effort,
            service_tier,
            permissions,
            approval_policy,
            approvals_reviewer,
            initial_message,
            attachments,
            plan_mode,
            goal_objective,
            memory_preferences,
        } => {
            let runtime_workspace_roots = cwd.clone().map(|path| vec![path]);
            let dynamic_tools = computer_use_dynamic_tools_for_platform();
            let computer_use_attached = dynamic_tools.is_some();
            match app_server.start_thread(ThreadStartParams {
                model: model.clone(),
                service_tier: Some(service_tier.clone()),
                cwd: cwd.map(|path| path.to_string_lossy().into_owned()),
                runtime_workspace_roots,
                permissions: permissions.clone(),
                approval_policy: approval_policy.clone(),
                approvals_reviewer: approvals_reviewer.map(protocol_approvals_reviewer),
                dynamic_tools,
                config: memory_preferences.map(|preferences| {
                    json!({
                        "memories.generate_memories": preferences.generate_memories,
                        "memories.use_memories": preferences.use_memories,
                    })
                }),
                personality: Some(Some(personality.as_str().to_owned())),
                ..ThreadStartParams::default()
            }) {
                Ok(response) => {
                    let task = map_task(response.thread);
                    let task_id = task.id.clone();
                    if computer_use_attached {
                        computer_capable_threads.insert(task_id.clone());
                    }
                    emit(events, Action::TaskCreated(task));
                    if let Some(preferences) = memory_preferences {
                        emit(
                            events,
                            Action::RememberThreadMemoryPreferences {
                                task_id: task_id.clone(),
                                preferences,
                            },
                        );
                    }
                    if computer_use_attached {
                        emit(
                            events,
                            Action::ComputerUseAvailable {
                                task_id: task_id.clone(),
                            },
                        );
                    }
                    let turn_started = match start_turn(
                        app_server,
                        StartTurnRequest {
                            task_id: task_id.clone(),
                            submission: RetryableTurnSubmission {
                                messages: vec![RetryableUserMessage {
                                    text: initial_message,
                                    attachments,
                                }],
                                model,
                                effort,
                                service_tier,
                                permissions,
                                approval_policy,
                                approvals_reviewer,
                                plan_mode,
                                personality: *personality,
                            },
                        },
                        events,
                        browser,
                        browser_download_preferences,
                        browser_permissions,
                        retryable_turns,
                    ) {
                        Ok(()) => true,
                        Err(error) => {
                            emit(
                                events,
                                Action::SetStatus(format!("failed to start turn: {error}")),
                            );
                            false
                        }
                    };
                    if turn_started && let Some(objective) = goal_objective {
                        match app_server.set_thread_goal(ThreadGoalSetParams {
                            thread_id: task_id.clone(),
                            objective: Some(objective),
                            status: Some(ProtocolThreadGoalStatus::Active),
                            token_budget: None,
                        }) {
                            Ok(response) => {
                                emit(events, Action::GoalUpdated(map_thread_goal(response.goal)));
                                emit(events, Action::MaybeContinueGoal { task_id });
                            }
                            Err(error) => emit(
                                events,
                                Action::GoalLoadFailed {
                                    task_id,
                                    message: format!("failed to set initial goal: {error}"),
                                },
                            ),
                        }
                    }
                }
                Err(error) => emit(
                    events,
                    Action::SetStatus(format!("failed to create task: {error}")),
                ),
            }
        }
        Effect::ForkTask {
            task_id,
            cwd,
            title,
        } => {
            if let Err(error) = fork_app_server_task(
                app_server,
                &task_id,
                cwd.clone(),
                &title,
                computer_capable_threads,
                events,
            ) {
                emit(
                    events,
                    Action::SetStatus(format!("Failed to create chat: {error}")),
                );
            }
        }
        Effect::ForkTaskIntoWorktree { .. } | Effect::CancelPendingWorktreeFork { .. } => {}
        Effect::RetryPendingWorktreeFork {
            request_id,
            task_id,
            cwd,
            title,
        } => {
            if let Err(error) = fork_app_server_task(
                app_server,
                &task_id,
                Some(cwd),
                &title,
                computer_capable_threads,
                events,
            ) {
                emit(
                    events,
                    Action::PendingWorktreeForkConversationFailed {
                        request_id,
                        message: bounded(error.to_string(), MAX_STATUS_BYTES),
                    },
                );
            } else {
                emit(events, Action::PendingWorktreeForkCompleted { request_id });
            }
        }
        Effect::ArchiveTask { task_id } => {
            match app_server.archive_thread(ThreadArchiveParams {
                thread_id: task_id.clone(),
            }) {
                Ok(_) => {
                    computer_capable_threads.remove(&task_id);
                    computer_permissions.remove(&task_id);
                    emit(events, Action::TaskArchived(task_id));
                }
                Err(error) => emit(
                    events,
                    Action::SetStatus(format!("failed to archive chat: {error}")),
                ),
            }
        }
        Effect::RenameTask { task_id, name } => {
            match app_server.set_thread_name(ThreadSetNameParams {
                thread_id: task_id.clone(),
                name: name.clone(),
            }) {
                Ok(_) => emit(events, Action::TaskRenamed { task_id, name }),
                Err(error) => emit(
                    events,
                    Action::SetStatus(format!("failed to rename chat: {error}")),
                ),
            }
        }
        Effect::UnarchiveTask { task_id } => {
            match app_server.unarchive_thread(ThreadUnarchiveParams {
                thread_id: task_id.clone(),
            }) {
                Ok(response) => emit(events, Action::TaskUnarchived(map_task(response.thread))),
                Err(error) => emit(
                    events,
                    Action::SetStatus(format!("failed to unarchive chat: {error}")),
                ),
            }
        }
        Effect::DeleteArchivedTasks { task_ids, kind } => {
            let result = archived_task_ids_for_delete(app_server).and_then(|archived_task_ids| {
                let delete_task_ids = match kind {
                    ArchivedTaskDeleteKind::All if archived_task_ids.is_empty() => {
                        return Err("no archived chats remain to delete".to_owned());
                    }
                    ArchivedTaskDeleteKind::All => archived_task_ids,
                    ArchivedTaskDeleteKind::Single | ArchivedTaskDeleteKind::Project => {
                        let archived_task_ids: HashSet<_> = archived_task_ids.into_iter().collect();
                        if !task_ids
                            .iter()
                            .all(|task_id| archived_task_ids.contains(task_id))
                        {
                            return Err(
                                "selected chats are no longer archived; refresh and try again"
                                    .to_owned(),
                            );
                        }
                        task_ids
                    }
                };
                for task_id in &delete_task_ids {
                    app_server
                        .delete_thread(ThreadDeleteParams {
                            thread_id: task_id.clone(),
                        })
                        .map_err(|error| format!("failed to delete archived chat: {error}"))?;
                }
                Ok(delete_task_ids)
            });
            match result {
                Ok(task_ids) => emit(events, Action::ArchivedTasksDeleted { task_ids, kind }),
                Err(message) => {
                    emit(events, Action::SetStatus(message));
                    emit(events, Action::RefreshArchivedTasks);
                }
            }
        }
        Effect::ResumeTask { task_id } => {
            match app_server.resume_thread(ThreadResumeParams {
                thread_id: task_id.clone(),
                exclude_turns: Some(true),
                initial_turns_page: Some(ThreadResumeInitialTurnsPageParams {
                    cursor: None,
                    limit: 1,
                    sort_direction: HistorySortDirection::Desc,
                    items_view: Some("notLoaded".to_owned()),
                }),
            }) {
                Ok(response) => {
                    emit(
                        events,
                        Action::TaskSettingsLoaded {
                            task_id: task_id.clone(),
                            model: response.model.clone(),
                            effort: response.reasoning_effort.clone(),
                            service_tier: response.service_tier.clone(),
                            permissions: response
                                .active_permission_profile
                                .as_ref()
                                .map(|profile| profile.id.clone()),
                            approval_policy: response
                                .approval_policy
                                .as_ref()
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                            approvals_reviewer: response
                                .approvals_reviewer
                                .map(core_approvals_reviewer),
                        },
                    );
                    let latest_turn = response
                        .initial_turns_page
                        .as_ref()
                        .and_then(|page| page.data.first());
                    let active_turn_id = latest_turn
                        .filter(|turn| {
                            string_field(turn, "status").as_deref() == Some("inProgress")
                        })
                        .and_then(|turn| string_field(turn, "id"));
                    let active_turn_is_review = active_turn_id.as_ref().is_some_and(|turn_id| {
                        load_active_turn_review_mode(app_server, &task_id, turn_id)
                    });
                    let run_status = latest_turn
                        .and_then(|turn| string_field(turn, "status"))
                        .and_then(|status| map_turn_status(&status));
                    emit(
                        events,
                        Action::TaskRuntimeLoaded {
                            task_id,
                            active_turn_id,
                            active_turn_is_review,
                            run_status,
                        },
                    );
                }
                Err(error) => emit(
                    events,
                    Action::SetStatus(format!("failed to resume chat: {error}")),
                ),
            }
        }
        Effect::LoadTimeline {
            task_id,
            generation,
            cursor,
        } => {
            let append = cursor.is_some();
            let params = ThreadItemsListParams {
                thread_id: task_id.clone(),
                limit: HISTORY_PAGE_LIMIT,
                sort_direction: HistorySortDirection::Asc,
                turn_id: None,
                cursor,
            };
            match app_server.list_thread_items(params) {
                Ok(page) => emit(
                    events,
                    Action::TimelineLoaded {
                        task_id,
                        generation,
                        items: page
                            .data
                            .into_iter()
                            .filter(|entry| !is_hidden_timeline_item(&entry.item))
                            .map(|entry| map_timeline_item(entry.turn_id, entry.item, true))
                            .collect(),
                        next_cursor: page.next_cursor,
                        append,
                    },
                ),
                Err(_) => emit(
                    events,
                    Action::TimelineFailed {
                        task_id,
                        generation,
                    },
                ),
            }
        }
        Effect::LoadBackgroundTerminals { task_id, cursor } => {
            let append = cursor.is_some();
            match app_server.list_background_terminals(ThreadBackgroundTerminalsListParams {
                thread_id: task_id.clone(),
                cursor,
                limit: Some(BACKGROUND_TERMINAL_PAGE_LIMIT),
            }) {
                Ok(page) => emit(
                    events,
                    Action::BackgroundTerminalsLoaded {
                        task_id,
                        terminals: page
                            .data
                            .into_iter()
                            .take(MAX_BACKGROUND_TERMINALS)
                            .map(map_background_terminal)
                            .collect(),
                        next_cursor: page.next_cursor.map(|cursor| bounded(cursor, 8 * 1024)),
                        append,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::BackgroundTerminalsFailed {
                        task_id,
                        message: format!("Unable to load background terminals: {error}"),
                    },
                ),
            }
        }
        Effect::TerminateBackgroundTerminal {
            task_id,
            process_id,
        } => {
            match app_server.terminate_background_terminal(
                ThreadBackgroundTerminalsTerminateParams {
                    thread_id: task_id.clone(),
                    process_id: process_id.clone(),
                },
            ) {
                Ok(response) => emit(
                    events,
                    Action::BackgroundTerminalTerminated {
                        task_id,
                        process_id,
                        terminated: response.terminated,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::BackgroundTerminalTerminateFailed {
                        task_id,
                        process_id,
                        message: format!("Unable to stop process: {error}"),
                    },
                ),
            }
        }
        Effect::CleanBackgroundTerminals { task_id } => {
            match app_server.clean_background_terminals(ThreadBackgroundTerminalsCleanParams {
                thread_id: task_id.clone(),
            }) {
                Ok(_) => emit(events, Action::BackgroundTerminalsCleaned { task_id }),
                Err(error) => emit(
                    events,
                    Action::BackgroundTerminalsCleanFailed {
                        task_id,
                        message: format!("Unable to stop background terminals: {error}"),
                    },
                ),
            }
        }
        Effect::LoadOutputPreview {
            task_id,
            root,
            path,
        } => match inspect_artifact(&root, &path) {
            Ok(preview) => emit(
                events,
                Action::OutputPreviewLoaded {
                    task_id,
                    requested_path: path,
                    preview: ArtifactPreview {
                        path: preview.path,
                        file_name: bounded(preview.file_name, 512),
                        extension: bounded(preview.extension, 32),
                        size_bytes: preview.size_bytes,
                        kind: match preview.kind {
                            ArtifactFileKind::Text => ArtifactPreviewKind::Text,
                            ArtifactFileKind::Image => ArtifactPreviewKind::Image,
                            ArtifactFileKind::TooLarge => ArtifactPreviewKind::TooLarge,
                            ArtifactFileKind::Unsupported => ArtifactPreviewKind::Unsupported,
                        },
                        text: preview.text,
                        truncated: preview.truncated,
                    },
                },
            ),
            Err(error) => emit(
                events,
                Action::OutputPreviewFailed {
                    task_id,
                    requested_path: path,
                    message: format!("Unable to open output: {error}"),
                },
            ),
        },
        Effect::LoadWorkspaceFilePreview { root, path } => {
            match inspect_workspace_file(&root, &path) {
                Ok(preview) => emit(
                    events,
                    Action::WorkspaceFilePreviewLoaded {
                        requested_path: path,
                        preview: ArtifactPreview {
                            path: preview.path,
                            file_name: bounded(preview.file_name, 512),
                            extension: bounded(preview.extension, 32),
                            size_bytes: preview.size_bytes,
                            kind: match preview.kind {
                                ArtifactFileKind::Text => ArtifactPreviewKind::Text,
                                ArtifactFileKind::Image => ArtifactPreviewKind::Image,
                                ArtifactFileKind::TooLarge => ArtifactPreviewKind::TooLarge,
                                ArtifactFileKind::Unsupported => ArtifactPreviewKind::Unsupported,
                            },
                            text: preview.text,
                            truncated: preview.truncated,
                        },
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::WorkspaceFilePreviewFailed {
                        requested_path: path,
                        message: format!("Unable to open file: {error}"),
                    },
                ),
            }
        }
        Effect::RevealOutput { root, path } => {
            let message = match reveal_artifact(&root, &path) {
                Ok(()) => "Opened output in the file manager".to_owned(),
                Err(error) => format!("Unable to reveal output: {error}"),
            };
            emit(events, Action::SetStatus(message));
        }
        Effect::DownloadOutput {
            root,
            path,
            destination,
        } => {
            if save_artifact_copy(&root, &path, &destination).is_err() {
                emit(
                    events,
                    Action::SetStatus("Could not download image".to_owned()),
                );
            }
        }
        Effect::OpenWorkspacePath { root, path } => {
            let message = match open_workspace_path(&root, &path) {
                Ok(()) => "Opened in the file manager".to_owned(),
                Err(error) => format!("Unable to open path: {error}"),
            };
            emit(events, Action::SetStatus(message));
        }
        Effect::UpdateThreadSettings {
            task_id,
            model,
            effort,
            service_tier,
            permissions,
            approval_policy,
            approvals_reviewer,
        } => {
            if let Err(error) = app_server.update_thread_settings(ThreadSettingsUpdateParams {
                thread_id: task_id.clone(),
                approval_policy,
                approvals_reviewer: approvals_reviewer.map(protocol_approvals_reviewer),
                permissions,
                model,
                effort,
                service_tier,
            }) {
                emit(
                    events,
                    Action::ThreadSettingsUpdateFailed {
                        task_id,
                        message: format!("failed to update chat settings: {error}"),
                    },
                );
            }
        }
        Effect::PersistComposerModelDefaults {
            model,
            effort,
            profile,
        } => {
            let result = app_server.batch_write_config(ConfigBatchWriteParams {
                edits: vec![
                    ConfigEdit {
                        key_path: composer_config_key(profile.as_deref(), "model"),
                        value: json!(model),
                        merge_strategy: ConfigMergeStrategy::Upsert,
                    },
                    ConfigEdit {
                        key_path: composer_config_key(profile.as_deref(), "model_reasoning_effort"),
                        value: json!(effort),
                        merge_strategy: ConfigMergeStrategy::Upsert,
                    },
                ],
                file_path: None,
                expected_version: None,
                reload_user_config: true,
            });
            match result {
                Ok(response) if response.status == ConfigWriteStatus::OkOverridden => emit(
                    events,
                    Action::SetStatus(
                        "New-chat model defaults are overridden by higher-priority configuration."
                            .to_owned(),
                    ),
                ),
                Ok(_) => {}
                Err(error) => emit(
                    events,
                    Action::ComposerDefaultsWriteFailed(format!(
                        "failed to save new-chat model defaults: {error}"
                    )),
                ),
            }
        }
        Effect::PersistComposerServiceTierDefault {
            service_tier,
            profile,
        } => {
            let result = app_server.batch_write_config(ConfigBatchWriteParams {
                edits: vec![ConfigEdit {
                    key_path: composer_config_key(profile.as_deref(), "service_tier"),
                    value: json!(
                        service_tier.unwrap_or_else(|| STANDARD_SERVICE_TIER_ID.to_owned())
                    ),
                    merge_strategy: ConfigMergeStrategy::Upsert,
                }],
                file_path: None,
                expected_version: None,
                reload_user_config: true,
            });
            match result {
                Ok(response) if response.status == ConfigWriteStatus::OkOverridden => emit(
                    events,
                    Action::SetStatus(
                        "New-chat speed default is overridden by higher-priority configuration."
                            .to_owned(),
                    ),
                ),
                Ok(_) => {}
                Err(error) => emit(
                    events,
                    Action::ComposerDefaultsWriteFailed(format!(
                        "failed to save new-chat speed default: {error}"
                    )),
                ),
            }
        }
        Effect::LoadGoal { task_id } => {
            match app_server.get_thread_goal(ThreadGoalGetParams {
                thread_id: task_id.clone(),
            }) {
                Ok(response) => emit(
                    events,
                    Action::GoalLoaded {
                        task_id,
                        goal: response.goal.map(map_thread_goal),
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::GoalLoadFailed {
                        task_id,
                        message: format!("failed to load goal: {error}"),
                    },
                ),
            }
        }
        Effect::SetGoal {
            task_id,
            objective,
            status,
            token_budget,
        } => {
            let continue_if_active = status == Some(CoreThreadGoalStatus::Active);
            match app_server.set_thread_goal(ThreadGoalSetParams {
                thread_id: task_id.clone(),
                objective,
                status: status.map(map_goal_status_to_protocol),
                token_budget,
            }) {
                Ok(response) => {
                    emit(events, Action::GoalUpdated(map_thread_goal(response.goal)));
                    if continue_if_active {
                        emit(
                            events,
                            Action::MaybeContinueGoal {
                                task_id: task_id.clone(),
                            },
                        );
                    }
                }
                Err(error) => {
                    emit(
                        events,
                        Action::GoalLoadFailed {
                            task_id,
                            message: format!("failed to update goal: {error}"),
                        },
                    );
                }
            }
        }
        Effect::ContinueGoal { task_id } => {
            match app_server.set_thread_goal(ThreadGoalSetParams {
                thread_id: task_id.clone(),
                objective: None,
                status: Some(ProtocolThreadGoalStatus::Active),
                token_budget: None,
            }) {
                Ok(response) => emit(events, Action::GoalUpdated(map_thread_goal(response.goal))),
                Err(error) => emit(
                    events,
                    Action::GoalContinuationFailed {
                        task_id,
                        message: format!("failed to continue goal: {error}"),
                    },
                ),
            }
        }
        Effect::ClearGoal { task_id } => {
            match app_server.clear_thread_goal(ThreadGoalClearParams {
                thread_id: task_id.clone(),
            }) {
                Ok(response) => {
                    if response.cleared {
                        emit(events, Action::GoalCleared { task_id });
                    } else {
                        emit(
                            events,
                            Action::GoalLoaded {
                                task_id,
                                goal: None,
                            },
                        );
                    }
                }
                Err(error) => emit(
                    events,
                    Action::GoalLoadFailed {
                        task_id,
                        message: format!("failed to clear goal: {error}"),
                    },
                ),
            }
        }
        Effect::EditLastUserMessage {
            task_id,
            turn_id,
            text,
            attachments,
            rollback_required,
            model,
            effort,
            service_tier,
            permissions,
            approval_policy,
            approvals_reviewer,
            plan_mode,
        } => {
            let rollback_applied = if rollback_required {
                match app_server.rollback_thread(ThreadRollbackParams {
                    thread_id: task_id.clone(),
                    num_turns: 1,
                }) {
                    Ok(_) => {
                        emit(
                            events,
                            Action::LastUserMessageRollbackApplied {
                                task_id: task_id.clone(),
                                turn_id: turn_id.clone(),
                            },
                        );
                        true
                    }
                    Err(_) => {
                        emit(
                            events,
                            Action::LastUserMessageEditFailed {
                                task_id,
                                turn_id,
                                rollback_applied: false,
                                message: "Could not edit the previous message.".to_owned(),
                            },
                        );
                        return;
                    }
                }
            } else {
                true
            };
            let started = start_turn(
                app_server,
                StartTurnRequest {
                    task_id: task_id.clone(),
                    submission: RetryableTurnSubmission {
                        messages: vec![RetryableUserMessage { text, attachments }],
                        model,
                        effort,
                        service_tier,
                        permissions,
                        approval_policy,
                        approvals_reviewer,
                        plan_mode,
                        personality: *personality,
                    },
                },
                events,
                browser,
                browser_download_preferences,
                browser_permissions,
                retryable_turns,
            )
            .is_ok();
            if started {
                emit(
                    events,
                    Action::LastUserMessageEditSucceeded { task_id, turn_id },
                );
            } else {
                emit(
                    events,
                    Action::LastUserMessageEditFailed {
                        task_id,
                        turn_id,
                        rollback_applied,
                        message:
                            "The message was edited, but its replacement turn could not be started. Try Send again."
                                .to_owned(),
                    },
                );
            }
        }
        Effect::CompactThread { task_id } => {
            if app_server
                .compact_thread(ThreadCompactStartParams {
                    thread_id: task_id.clone(),
                })
                .is_err()
            {
                emit(
                    events,
                    Action::CompactThreadFailed {
                        task_id,
                        message: "Could not compact this chat's context.".to_owned(),
                    },
                );
            }
        }
        Effect::StartReview {
            generation,
            source_task_id,
            target,
            delivery,
        } => {
            let target = match target {
                CoreReviewTarget::UncommittedChanges => ProtocolReviewTarget::UncommittedChanges,
                CoreReviewTarget::BaseBranch { branch } => {
                    ProtocolReviewTarget::BaseBranch { branch }
                }
            };
            let delivery = match delivery {
                CoreReviewDelivery::Inline => ProtocolReviewDelivery::Inline,
                CoreReviewDelivery::Detached => ProtocolReviewDelivery::Detached,
            };
            match app_server.start_review(ReviewStartParams {
                thread_id: source_task_id.clone(),
                target,
                delivery: Some(delivery),
            }) {
                Ok(response) => {
                    let turn_id = bounded(
                        string_field(&response.turn, "id").unwrap_or_default(),
                        MAX_REVIEW_ID_BYTES,
                    );
                    let turn_active =
                        string_field(&response.turn, "status").as_deref() == Some("inProgress");
                    let review_thread_id = bounded(response.review_thread_id, MAX_REVIEW_ID_BYTES);
                    if turn_id.is_empty() || review_thread_id.is_empty() {
                        emit(
                            events,
                            Action::ReviewStartFailed {
                                generation,
                                source_task_id,
                                message: CODE_REVIEW_START_FAILED.to_owned(),
                            },
                        );
                    } else {
                        let review_task = if delivery == ProtocolReviewDelivery::Detached {
                            app_server
                                .read_thread(ThreadReadParams {
                                    thread_id: review_thread_id.clone(),
                                    include_turns: false,
                                })
                                .ok()
                                .map(|response| map_task(response.thread))
                        } else {
                            None
                        };
                        emit(
                            events,
                            Action::ReviewStarted {
                                generation,
                                source_task_id,
                                review_thread_id,
                                turn_id,
                                turn_active,
                                review_task,
                            },
                        );
                    }
                }
                Err(_) => emit(
                    events,
                    Action::ReviewStartFailed {
                        generation,
                        source_task_id,
                        message: CODE_REVIEW_START_FAILED.to_owned(),
                    },
                ),
            }
        }
        Effect::RunThreadShellCommand { task_id, command } => {
            if app_server
                .run_thread_shell_command(ThreadShellCommandParams {
                    thread_id: task_id.clone(),
                    command,
                })
                .is_err()
            {
                emit(
                    events,
                    Action::ThreadShellCommandFailed {
                        task_id,
                        message: "Could not run the shell command.".to_owned(),
                    },
                );
            }
        }
        Effect::StartTurn {
            task_id,
            text,
            model,
            effort,
            service_tier,
            permissions,
            approval_policy,
            approvals_reviewer,
            attachments,
            plan_mode,
        } => {
            if let Err(error) = start_turn(
                app_server,
                StartTurnRequest {
                    task_id,
                    submission: RetryableTurnSubmission {
                        messages: vec![RetryableUserMessage { text, attachments }],
                        model,
                        effort,
                        service_tier,
                        permissions,
                        approval_policy,
                        approvals_reviewer,
                        plan_mode,
                        personality: *personality,
                    },
                },
                events,
                browser,
                browser_download_preferences,
                browser_permissions,
                retryable_turns,
            ) {
                emit(
                    events,
                    Action::SetStatus(format!("failed to start turn: {error}")),
                );
            }
        }
        Effect::SteerTurn {
            task_id,
            expected_turn_id,
            text,
            attachments,
        } => {
            let message = RetryableUserMessage { text, attachments };
            match app_server.steer_turn(TurnSteerParams {
                thread_id: task_id.clone(),
                input: composer_inputs(message.text.clone(), message.attachments.clone()),
                expected_turn_id: expected_turn_id.clone(),
            }) {
                Ok(_) => {
                    record_retryable_steer(retryable_turns, &task_id, &expected_turn_id, &message);
                    emit(
                        events,
                        Action::TurnSteerRecorded {
                            task_id,
                            turn_id: expected_turn_id,
                            message,
                        },
                    );
                }
                Err(error) => emit(
                    events,
                    Action::SetStatus(format!("failed to steer turn: {error}")),
                ),
            }
        }
        Effect::InterruptTurn { task_id, turn_id } => {
            if let Some(overlay) = computer_overlay {
                let _ = overlay.complete_turn(&task_id, &turn_id);
            }
            if let Err(error) = app_server.interrupt_turn(TurnInterruptParams {
                thread_id: task_id.clone(),
                turn_id,
            }) {
                emit(
                    events,
                    Action::TurnInterruptFailed {
                        task_id,
                        message: format!("failed to stop turn: {error}"),
                    },
                );
            }
        }
        Effect::RetrySafetyBufferedTurn {
            task_id,
            turn_id,
            faster_model,
            submission,
        } => {
            if let Some(overlay) = computer_overlay {
                let _ = overlay.complete_turn(&task_id, &turn_id);
            }
            let submission = retryable_turns
                .remove(&(task_id.clone(), turn_id.clone()))
                .unwrap_or(submission);
            retry_safety_buffered_turn(
                app_server,
                task_id,
                turn_id,
                faster_model,
                submission,
                computer_capable_threads,
                events,
                browser,
                browser_download_preferences,
                browser_permissions,
                retryable_turns,
            );
        }
        Effect::RespondApproval {
            request_id,
            decision,
        } => {
            respond_to_approval(
                app_server,
                request_id,
                decision,
                events,
                pending_approvals,
                computer_permissions,
                computer_allowed_app_ids,
                computer_accessibility,
                computer_url_policy,
                computer_overlay,
            );
        }
        Effect::RespondUserInput {
            request_id,
            answers,
        } => {
            respond_to_user_input(app_server, request_id, answers, events, pending_approvals);
        }
        Effect::RespondMcpElicitation {
            request_id,
            decision,
            content,
        } => {
            respond_to_mcp_elicitation(
                app_server,
                request_id,
                decision,
                content,
                events,
                pending_approvals,
            );
        }
        Effect::RespondBrowserOriginElicitation {
            request_id,
            decision,
        } => {
            respond_to_browser_origin_elicitation(
                app_server,
                request_id,
                decision,
                events,
                pending_approvals,
            );
        }
        Effect::RespondBrowserResourceElicitation {
            request_id,
            decision,
        } => {
            respond_to_browser_resource_elicitation(
                app_server,
                request_id,
                decision,
                events,
                pending_approvals,
            );
        }
        Effect::AddMarketplace {
            source,
            ref_name,
            sparse_paths,
        } => {
            match app_server.add_marketplace(MarketplaceAddParams {
                source,
                ref_name,
                sparse_paths,
            }) {
                Ok(response) => emit(
                    events,
                    Action::MarketplaceAdded {
                        marketplace_name: response.marketplace_name,
                        already_added: response.already_added,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::MarketplaceAddFailed(format!("Failed to add marketplace: {error}")),
                ),
            }
        }
        Effect::RemoveMarketplace { marketplace_name } => {
            match app_server.remove_marketplace(MarketplaceRemoveParams {
                marketplace_name: marketplace_name.clone(),
            }) {
                Ok(response) => emit(
                    events,
                    Action::MarketplaceRemoved(response.marketplace_name),
                ),
                Err(error) => emit(
                    events,
                    Action::MarketplaceRemoveFailed {
                        marketplace_name,
                        message: format!("Failed to remove marketplace: {error}"),
                    },
                ),
            }
        }
        Effect::UpgradeMarketplaces { marketplace_name } => {
            match app_server.upgrade_marketplaces(MarketplaceUpgradeParams { marketplace_name }) {
                Ok(response) => {
                    let upgraded_count = response.upgraded_roots.len();
                    emit(
                        events,
                        Action::MarketplacesUpgraded {
                            selected_marketplaces: response.selected_marketplaces,
                            upgraded_count,
                            errors: response
                                .errors
                                .into_iter()
                                .map(|error| MarketplaceUpgradeFailure {
                                    marketplace_name: error.marketplace_name,
                                    message: error.message,
                                })
                                .collect(),
                        },
                    );
                }
                Err(error) => emit(
                    events,
                    Action::MarketplaceUpgradeFailed(format!(
                        "Failed to upgrade marketplaces: {error}"
                    )),
                ),
            }
        }
        Effect::RefreshComposerPlugins {
            cwds,
            force_refetch,
        } => match load_composer_plugins(app_server, cwds, force_refetch, marketplaces) {
            Ok(plugins) => emit(events, Action::ComposerPluginsLoaded(plugins)),
            Err(error) => emit(
                events,
                Action::SetStatus(format!("failed to load composer plugins: {error}")),
            ),
        },
        Effect::RefreshMarketplace {
            cwds,
            directory_tab,
            force_refetch,
            include_all_marketplaces,
        } => {
            match app_server.list_plugins(PluginListParams {
                cwds: (!cwds.is_empty()).then_some(cwds),
                marketplace_kinds: (!include_all_marketplaces)
                    .then(|| plugin_directory_marketplace_kinds(directory_tab))
                    .flatten(),
                force_refetch,
            }) {
                Ok(response) => {
                    marketplaces.clear();
                    let app_logos = load_app_logos(app_server);
                    let featured = response
                        .featured_plugin_ids
                        .into_iter()
                        .enumerate()
                        .map(|(rank, plugin_id)| (plugin_id, rank))
                        .collect::<HashMap<_, _>>();
                    let mut cards = Vec::new();
                    let mut sources = Vec::new();
                    for marketplace in response.marketplaces {
                        if !include_all_marketplaces
                            && !plugin_directory_includes_marketplace(
                                directory_tab,
                                &marketplace.name,
                            )
                        {
                            continue;
                        }
                        if include_all_marketplaces
                            && marketplace.path.is_some()
                            && !CURATED_MARKETPLACE_NAMES.contains(&marketplace.name.as_str())
                        {
                            sources.push(MarketplaceSourceCard {
                                name: marketplace.name.clone(),
                                path: marketplace.path.clone(),
                                plugin_count: marketplace.plugins.len(),
                                removable: true,
                            });
                        }
                        marketplaces.insert(marketplace.name.clone(), marketplace.path.clone());
                        for plugin in marketplace.plugins {
                            let display_name = plugin
                                .presentation
                                .as_ref()
                                .and_then(|presentation| presentation.display_name.clone())
                                .unwrap_or_else(|| plugin.name.clone());
                            let description = plugin
                                .presentation
                                .as_ref()
                                .and_then(|presentation| {
                                    presentation
                                        .short_description
                                        .clone()
                                        .or_else(|| presentation.long_description.clone())
                                })
                                .unwrap_or_default();
                            let category = plugin
                                .presentation
                                .as_ref()
                                .and_then(|presentation| presentation.category.clone());
                            let developer = plugin
                                .presentation
                                .as_ref()
                                .and_then(|presentation| presentation.developer_name.clone());
                            let logo_url = plugin
                                .presentation
                                .as_ref()
                                .and_then(|presentation| presentation.logo_url.clone());
                            let logo_url_dark = plugin
                                .presentation
                                .as_ref()
                                .and_then(|presentation| presentation.logo_url_dark.clone());
                            let app_logo = app_logos
                                .get(&normalized_plugin_name(&display_name))
                                .or_else(|| app_logos.get(&normalized_plugin_name(&plugin.name)));
                            let logo_url =
                                logo_url.or_else(|| app_logo.and_then(|logo| logo.light.clone()));
                            let logo_url_dark = logo_url_dark
                                .or_else(|| app_logo.and_then(|logo| logo.dark.clone()));
                            let default_prompt =
                                plugin.presentation.as_ref().and_then(|presentation| {
                                    presentation
                                        .default_prompt
                                        .as_ref()
                                        .map(|parts| parts.join("\n"))
                                        .filter(|prompt| !prompt.trim().is_empty())
                                });
                            let version = plugin.local_version.clone().or(plugin.version.clone());
                            let installable = plugin.availability.as_deref()
                                != Some("DISABLED_BY_ADMIN")
                                && plugin.install_policy.as_deref() != Some("NOT_AVAILABLE");
                            cards.push(PluginCard {
                                id: plugin.id.clone(),
                                install_name: plugin.name,
                                marketplace: marketplace.name.clone(),
                                name: display_name,
                                description,
                                category,
                                developer,
                                logo_url,
                                logo_url_dark,
                                default_prompt,
                                version,
                                installed: plugin.installed,
                                enabled: plugin.enabled,
                                installable,
                                featured: featured.contains_key(&plugin.id),
                                featured_rank: featured.get(&plugin.id).copied(),
                            });
                        }
                    }
                    sources.sort_by(|left, right| {
                        left.name.to_lowercase().cmp(&right.name.to_lowercase())
                    });
                    emit(
                        events,
                        Action::MarketplaceLoaded {
                            plugins: cards,
                            sources,
                        },
                    );
                }
                Err(error) => emit(
                    events,
                    Action::MarketplaceFailed(format!("failed to load marketplace: {error}")),
                ),
            }
        }
        Effect::RefreshApps { force_refetch } => match load_apps(app_server, force_refetch) {
            Ok(apps) => emit(events, Action::AppsLoaded(apps)),
            Err(error) => emit(
                events,
                Action::AppsFailed(format!("failed to load apps: {error}")),
            ),
        },
        Effect::ReadApp { app_id } => {
            match app_server.read_apps(AppsReadParams {
                app_ids: vec![app_id.clone()],
                include_tools: true,
            }) {
                Ok(response) => {
                    if let Some(app) = response.apps.into_iter().find(|app| app.id == app_id) {
                        emit(
                            events,
                            Action::AppDetailLoaded {
                                app_id,
                                detail: map_app_detail(app),
                            },
                        );
                    } else {
                        emit(
                            events,
                            Action::AppDetailFailed {
                                app_id,
                                message: "The app is no longer available.".to_owned(),
                            },
                        );
                    }
                }
                Err(error) => emit(
                    events,
                    Action::AppDetailFailed {
                        app_id,
                        message: format!("failed to load app details: {error}"),
                    },
                ),
            }
        }
        Effect::RefreshMcpServers { cwd } => match load_mcp_servers(app_server, cwd) {
            Ok(catalog) => emit(
                events,
                Action::McpServersLoaded {
                    servers: catalog.servers,
                    plugin_servers: catalog.plugin_servers,
                    warnings: catalog.warnings,
                },
            ),
            Err(error) => emit(
                events,
                Action::McpServersFailed(format!("failed to load MCP servers: {error}")),
            ),
        },
        Effect::ReadMcpResource { server, uri } => {
            match app_server.read_mcp_resource(McpResourceReadParams {
                server: server.clone(),
                uri: uri.clone(),
                thread_id: None,
            }) {
                Ok(response) => emit(
                    events,
                    Action::McpResourceLoaded {
                        server,
                        uri,
                        contents: map_mcp_resource_contents(response.contents),
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::McpResourceFailed {
                        server,
                        uri,
                        message: format!("failed to read MCP resource: {error}"),
                    },
                ),
            }
        }
        Effect::ReadPlugin {
            plugin_id,
            plugin_name,
            marketplace,
        } => {
            let marketplace_path = marketplaces.get(&marketplace).cloned().flatten();
            let remote_marketplace_name = marketplaces
                .get(&marketplace)
                .is_none_or(Option::is_none)
                .then_some(marketplace);
            match app_server.read_plugin(PluginReadParams {
                marketplace_path,
                remote_marketplace_name,
                plugin_name,
            }) {
                Ok(response) => emit(
                    events,
                    Action::PluginDetailLoaded {
                        plugin_id: plugin_id.clone(),
                        detail: map_plugin_detail(plugin_id, response.plugin),
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::PluginDetailFailed {
                        plugin_id,
                        message: format!("failed to load plugin details: {error}"),
                    },
                ),
            }
        }
        Effect::RefreshSkills { cwds, force_reload } => {
            match app_server.list_skills(SkillsListParams { cwds, force_reload }) {
                Ok(response) => {
                    let mut skills = Vec::new();
                    let mut errors = Vec::new();
                    for entry in response.data {
                        for error in entry.errors {
                            errors.push(format!(
                                "{}: {}",
                                error.path.display(),
                                bounded(error.message, MAX_STATUS_BYTES)
                            ));
                        }
                        for skill in entry.skills {
                            let display_name = skill
                                .presentation
                                .as_ref()
                                .and_then(|presentation| presentation.display_name.clone())
                                .unwrap_or_else(|| skill.name.clone());
                            let description = skill
                                .presentation
                                .as_ref()
                                .and_then(|presentation| presentation.short_description.clone())
                                .or(skill.short_description)
                                .unwrap_or(skill.description);
                            skills.push(SkillCard {
                                name: skill.name,
                                display_name,
                                description,
                                path: skill.path,
                                scope: map_skill_scope(skill.scope),
                                enabled: skill.enabled,
                            });
                        }
                    }
                    skills.sort_by(|left, right| {
                        left.display_name
                            .to_lowercase()
                            .cmp(&right.display_name.to_lowercase())
                            .then_with(|| left.path.cmp(&right.path))
                    });
                    emit(events, Action::SkillsLoaded { skills, errors });
                }
                Err(error) => emit(
                    events,
                    Action::SkillsFailed(format!("failed to load skills: {error}")),
                ),
            }
        }
        Effect::RefreshHooks { cwds } => match app_server.list_hooks(HooksListParams { cwds }) {
            Ok(response) => {
                let mut entries = Vec::new();
                let mut remaining_hooks = MAX_HOOK_ITEMS;
                let mut remaining_issues = MAX_HOOK_ISSUES;
                for entry in response.data.into_iter().take(MAX_HOOK_PROJECTS) {
                    if !entry.cwd.is_absolute()
                        || entry.cwd.to_string_lossy().len() > MAX_HOOK_FIELD_BYTES
                    {
                        continue;
                    }
                    let mut hooks = Vec::new();
                    for hook in entry.hooks.into_iter().take(remaining_hooks) {
                        if hook.source_path.to_string_lossy().len() > MAX_HOOK_FIELD_BYTES {
                            continue;
                        }
                        let key = bounded(hook.key, MAX_HOOK_FIELD_BYTES);
                        if key.is_empty() {
                            continue;
                        }
                        hooks.push(HookCard {
                            key,
                            event_name: map_hook_event_name(hook.event_name),
                            handler_type: map_hook_handler_type(hook.handler_type),
                            is_managed: hook.is_managed,
                            matcher: hook
                                .matcher
                                .map(|value| bounded(value, MAX_HOOK_FIELD_BYTES)),
                            command: hook
                                .command
                                .map(|value| bounded(value, MAX_HOOK_FIELD_BYTES)),
                            timeout_sec: hook.timeout_sec,
                            status_message: hook
                                .status_message
                                .map(|value| bounded(value, MAX_HOOK_FIELD_BYTES)),
                            source_path: hook.source_path,
                            source: map_hook_source(hook.source),
                            plugin_id: hook
                                .plugin_id
                                .map(|value| bounded(value, MAX_HOOK_FIELD_BYTES)),
                            display_order: hook.display_order,
                            enabled: hook.enabled,
                            current_hash: bounded(hook.current_hash, MAX_HOOK_FIELD_BYTES),
                            trust_status: map_hook_trust_status(hook.trust_status),
                        });
                    }
                    remaining_hooks = remaining_hooks.saturating_sub(hooks.len());
                    let warnings = entry
                        .warnings
                        .into_iter()
                        .take(remaining_issues)
                        .map(|warning| bounded(warning, MAX_HOOK_FIELD_BYTES))
                        .collect::<Vec<_>>();
                    remaining_issues = remaining_issues.saturating_sub(warnings.len());
                    let errors = entry
                        .errors
                        .into_iter()
                        .take(remaining_issues)
                        .map(|error| HookIssue {
                            path: error.path,
                            message: bounded(error.message, MAX_HOOK_FIELD_BYTES),
                        })
                        .collect::<Vec<_>>();
                    remaining_issues = remaining_issues.saturating_sub(errors.len());
                    entries.push(HookProjectEntry {
                        cwd: entry.cwd,
                        hooks,
                        warnings,
                        errors,
                    });
                    if remaining_hooks == 0 && remaining_issues == 0 {
                        break;
                    }
                }
                emit(events, Action::HooksLoaded(entries));
            }
            Err(error) => emit(
                events,
                Action::HooksFailed(format!("failed to load hooks: {error}")),
            ),
        },
        Effect::SetHookEnabled { key, enabled } => {
            let result = app_server.batch_write_config(ConfigBatchWriteParams {
                edits: vec![ConfigEdit {
                    key_path: "hooks.state".to_owned(),
                    value: hook_state_config_value(&key, Some(enabled), None),
                    merge_strategy: ConfigMergeStrategy::Upsert,
                }],
                file_path: None,
                expected_version: None,
                reload_user_config: true,
            });
            match result {
                Ok(response) => emit(
                    events,
                    Action::HookMutationFinished {
                        key,
                        overridden: response.status == ConfigWriteStatus::OkOverridden,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::HookMutationFailed {
                        key,
                        message: format!("failed to update hook: {error}"),
                    },
                ),
            }
        }
        Effect::TrustHook { key, current_hash } => {
            let result = app_server.batch_write_config(ConfigBatchWriteParams {
                edits: vec![ConfigEdit {
                    key_path: "hooks.state".to_owned(),
                    value: hook_state_config_value(&key, None, Some(&current_hash)),
                    merge_strategy: ConfigMergeStrategy::Upsert,
                }],
                file_path: None,
                expected_version: None,
                reload_user_config: true,
            });
            match result {
                Ok(response) => emit(
                    events,
                    Action::HookMutationFinished {
                        key,
                        overridden: response.status == ConfigWriteStatus::OkOverridden,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::HookMutationFailed {
                        key,
                        message: format!("failed to trust hook: {error}"),
                    },
                ),
            }
        }
        Effect::InstallPlugin {
            plugin_id,
            plugin_name,
            marketplace,
        } => {
            let path = marketplaces.get(&marketplace).cloned().flatten();
            let result = app_server.install_plugin(PluginInstallParams {
                marketplace_path: path,
                remote_marketplace_name: marketplaces
                    .get(&marketplace)
                    .is_none_or(Option::is_none)
                    .then_some(marketplace),
                plugin_name,
            });
            match result {
                Ok(response) => emit(
                    events,
                    Action::PluginMutationFinished {
                        plugin_id,
                        installed: true,
                        apps_needing_auth: map_auth_app_summaries(response.apps_needing_auth),
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::PluginMutationFailed {
                        plugin_id,
                        message: format!("plugin installation failed: {error}"),
                    },
                ),
            }
        }
        Effect::UninstallPlugin { plugin_id } => {
            match app_server.uninstall_plugin(PluginUninstallParams {
                plugin_id: plugin_id.clone(),
            }) {
                Ok(_) => emit(
                    events,
                    Action::PluginMutationFinished {
                        plugin_id,
                        installed: false,
                        apps_needing_auth: Vec::new(),
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::PluginMutationFailed {
                        plugin_id,
                        message: format!("plugin removal failed: {error}"),
                    },
                ),
            }
        }
        Effect::SetPluginEnabled { plugin_id, enabled } => {
            let result = app_server.batch_write_config(ConfigBatchWriteParams {
                edits: vec![ConfigEdit {
                    key_path: format!("plugins.{plugin_id}.enabled"),
                    value: json!(enabled),
                    merge_strategy: ConfigMergeStrategy::Upsert,
                }],
                file_path: None,
                expected_version: None,
                reload_user_config: true,
            });
            match result {
                Ok(response) => emit(
                    events,
                    Action::PluginEnabledChanged {
                        plugin_id,
                        enabled,
                        overridden: response.status == ConfigWriteStatus::OkOverridden,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::PluginMutationFailed {
                        plugin_id,
                        message: format!("failed to update plugin: {error}"),
                    },
                ),
            }
        }
        Effect::SetAppEnabled { app_id, enabled } => {
            let result = app_server.batch_write_config(ConfigBatchWriteParams {
                edits: vec![ConfigEdit {
                    key_path: format!("apps.{app_id}.enabled"),
                    value: json!(enabled),
                    merge_strategy: ConfigMergeStrategy::Upsert,
                }],
                file_path: None,
                expected_version: None,
                reload_user_config: true,
            });
            match result {
                Ok(response) => emit(
                    events,
                    Action::AppEnabledChanged {
                        app_id,
                        enabled,
                        overridden: response.status == ConfigWriteStatus::OkOverridden,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::AppMutationFailed {
                        app_id,
                        message: format!("failed to update app: {error}"),
                    },
                ),
            }
        }
        Effect::SetMcpServerEnabled { key, enabled } => {
            let result = app_server.batch_write_config(ConfigBatchWriteParams {
                edits: vec![ConfigEdit {
                    key_path: format!("mcp_servers.{key}.enabled"),
                    value: json!(enabled),
                    merge_strategy: ConfigMergeStrategy::Upsert,
                }],
                file_path: None,
                expected_version: None,
                reload_user_config: true,
            });
            match result {
                Ok(response) => emit(
                    events,
                    Action::McpServerEnabledChanged {
                        key,
                        enabled,
                        overridden: response.status == ConfigWriteStatus::OkOverridden,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::McpServerMutationFailed {
                        key,
                        message: format!("failed to update MCP server: {error}"),
                    },
                ),
            }
        }
        Effect::SaveMcpServer {
            existing_key,
            key,
            draft,
            cwd,
        } => {
            let result = (|| {
                let config = app_server.read_config(ConfigReadParams {
                    include_layers: false,
                    cwd: cwd.map(|path| path.to_string_lossy().into_owned()),
                })?;
                let existing = existing_key
                    .as_deref()
                    .and_then(|existing_key| config.config.mcp_servers.get(existing_key));
                app_server.batch_write_config(ConfigBatchWriteParams {
                    edits: vec![ConfigEdit {
                        key_path: format!("mcp_servers.{key}"),
                        value: mcp_server_config_value(existing, &draft),
                        merge_strategy: ConfigMergeStrategy::Replace,
                    }],
                    file_path: None,
                    expected_version: None,
                    reload_user_config: true,
                })
            })();
            match result {
                Ok(response) => emit(
                    events,
                    Action::McpServerSaved {
                        key,
                        overridden: response.status == ConfigWriteStatus::OkOverridden,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::McpServerMutationFailed {
                        key,
                        message: format!("failed to save MCP server: {error}"),
                    },
                ),
            }
        }
        Effect::RemoveMcpServer { key } => {
            let result = app_server.batch_write_config(ConfigBatchWriteParams {
                edits: vec![ConfigEdit {
                    key_path: format!("mcp_servers.{key}"),
                    value: Value::Null,
                    merge_strategy: ConfigMergeStrategy::Replace,
                }],
                file_path: None,
                expected_version: None,
                reload_user_config: true,
            });
            match result {
                Ok(response) => emit(
                    events,
                    Action::McpServerRemoved {
                        key,
                        overridden: response.status == ConfigWriteStatus::OkOverridden,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::McpServerMutationFailed {
                        key,
                        message: format!("failed to uninstall MCP server: {error}"),
                    },
                ),
            }
        }
        Effect::AuthenticateMcpServer { name } => {
            match app_server.login_mcp_server(McpServerOauthLoginParams {
                name: name.clone(),
                thread_id: None,
                scopes: None,
                timeout_secs: None,
            }) {
                Ok(response) => emit(
                    events,
                    Action::McpServerAuthenticationStarted {
                        name,
                        authorization_url: response.authorization_url,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::McpServerAuthenticationCompleted {
                        name,
                        success: false,
                        error: Some(format!(
                            "failed to start MCP server authentication: {error}"
                        )),
                    },
                ),
            }
        }
        Effect::ReloadMcpServers { cwd } => match app_server.reload_mcp_servers() {
            Ok(()) => match load_mcp_servers(app_server, cwd) {
                Ok(catalog) => emit(
                    events,
                    Action::McpServersLoaded {
                        servers: catalog.servers,
                        plugin_servers: catalog.plugin_servers,
                        warnings: catalog.warnings,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::McpServersFailed(format!("failed to reload MCP servers: {error}")),
                ),
            },
            Err(error) => emit(
                events,
                Action::McpServersFailed(format!("failed to restart MCP servers: {error}")),
            ),
        },
        Effect::SetPluginSkillEnabled {
            plugin_id,
            skill_name,
            enabled,
        } => {
            match app_server.write_skill_config(SkillsConfigWriteParams {
                path: None,
                name: Some(skill_name.clone()),
                enabled,
            }) {
                Ok(response) => emit(
                    events,
                    Action::PluginSkillEnabledChanged {
                        plugin_id,
                        skill_name,
                        requested_enabled: enabled,
                        effective_enabled: response.effective_enabled,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::PluginSkillMutationFailed {
                        plugin_id,
                        skill_name,
                        message: format!("failed to update plugin skill: {error}"),
                    },
                ),
            }
        }
        Effect::SetSkillEnabled { path, enabled } => {
            match app_server.write_skill_config(SkillsConfigWriteParams {
                path: Some(path.clone()),
                name: None,
                enabled,
            }) {
                Ok(response) => emit(
                    events,
                    Action::SkillEnabledChanged {
                        path,
                        requested_enabled: enabled,
                        effective_enabled: response.effective_enabled,
                    },
                ),
                Err(error) => emit(
                    events,
                    Action::SkillMutationFailed {
                        path,
                        message: format!("failed to update skill: {error}"),
                    },
                ),
            }
        }
        Effect::RefreshGit { .. }
        | Effect::SchedulePullRequestSearch { .. }
        | Effect::SearchPullRequests { .. }
        | Effect::LoadPullRequestDetail { .. }
        | Effect::LoadPullRequestDiff { .. }
        | Effect::MutatePullRequest { .. }
        | Effect::LoadDiff { .. }
        | Effect::LoadUncommittedDiff { .. }
        | Effect::LoadCommitDiff { .. }
        | Effect::StagePath { .. }
        | Effect::StageAll { .. }
        | Effect::UnstagePath { .. }
        | Effect::UnstageAll { .. }
        | Effect::CommitGit { .. }
        | Effect::LoadGitPullRequest { .. }
        | Effect::CreateGitPullRequest { .. }
        | Effect::SwitchGitBranch { .. }
        | Effect::CreateGitBranch { .. }
        | Effect::CreateGitWorktree { .. }
        | Effect::PersistPrimaryWindowPlacement(_)
        | Effect::PersistUiState { .. }
        | Effect::PersistTerminalDockLocation(_)
        | Effect::PersistAppearanceTheme(_)
        | Effect::PersistAppearancePreferences(_)
        | Effect::PersistGitPreferences(_)
        | Effect::PersistBrowserDownloadPreferences(_)
        | Effect::PersistBrowserPermissions(_)
        | Effect::PersistBrowserDownload(_)
        | Effect::DeletePersistedBrowserDownload { .. }
        | Effect::PersistKeyboardShortcutPreferences { .. }
        | Effect::PersistTerminalDockSize { .. }
        | Effect::PersistIntegratedTerminalShell(_)
        | Effect::PersistGitIncludeUnstaged(_)
        | Effect::PersistPinnedTasks { .. }
        | Effect::RememberWorkspace { .. }
        | Effect::RenameLocalProject { .. }
        | Effect::SetLocalProjectPinned { .. }
        | Effect::RemoveLocalProject { .. }
        | Effect::ScheduleGoalContinuation { .. }
        | Effect::ConfigureComputerUse { .. }
        | Effect::StartBrowser { .. }
        | Effect::ActivateBrowser { .. }
        | Effect::BrowserNavigate { .. }
        | Effect::BrowserBack { .. }
        | Effect::BrowserForward { .. }
        | Effect::BrowserReload { .. }
        | Effect::BrowserStop { .. }
        | Effect::BrowserOpenTab { .. }
        | Effect::BrowserSelectTab { .. }
        | Effect::BrowserCloseTab { .. }
        | Effect::BrowserResize { .. }
        | Effect::BrowserSurfaceState { .. }
        | Effect::BrowserClick { .. }
        | Effect::BrowserScroll { .. }
        | Effect::BrowserKey { .. }
        | Effect::ConfigureBrowserDownloads(_)
        | Effect::ConfigureBrowserPermissions(_)
        | Effect::BrowserCancelDownload { .. }
        | Effect::BrowserPauseDownload { .. }
        | Effect::BrowserResumeDownload { .. }
        | Effect::BrowserOpenDownload { .. }
        | Effect::BrowserRemoveDownload { .. }
        | Effect::BrowserShowDownloadInFolder { .. }
        | Effect::BrowserShowDownloadsFolder
        | Effect::BrowserSetDownloadDestination { .. }
        | Effect::LoadComposerDesktopApps
        | Effect::LoadComputerWindows { .. }
        | Effect::LaunchComputerApp { .. }
        | Effect::CaptureComputerWindow { .. }
        | Effect::SpawnTerminal { .. }
        | Effect::RestartTerminal { .. }
        | Effect::WriteTerminal { .. }
        | Effect::StopTerminal { .. } => {
            unreachable!("local effects return before app-server routing")
        }
    }
}

fn fork_app_server_task(
    app_server: &AppServerConnection,
    task_id: &str,
    cwd: Option<PathBuf>,
    source_title: &str,
    computer_capable_threads: &mut HashSet<String>,
    events: &Sender<Action>,
) -> Result<(), AppServerError> {
    let response = app_server.fork_thread(ThreadForkParams {
        thread_id: task_id.to_owned(),
        cwd,
        last_turn_id: None,
        before_turn_id: None,
        exclude_turns: Some(true),
        defer_goal_continuation: Some(true),
    })?;
    let mut task = map_task(response.thread);
    let fork_id = task.id.clone();
    let title_error = if !source_title.trim().is_empty() && task.title != source_title {
        match app_server.set_thread_name(ThreadSetNameParams {
            thread_id: fork_id.clone(),
            name: source_title.to_owned(),
        }) {
            Ok(_) => {
                task.title = source_title.to_owned();
                None
            }
            Err(error) => Some(error),
        }
    } else {
        None
    };
    let inherits_computer_use = computer_capable_threads.contains(task_id);
    if inherits_computer_use {
        computer_capable_threads.insert(fork_id.clone());
    }
    emit(events, Action::TaskCreated(task));
    emit(events, Action::SelectTask(fork_id.clone()));
    if inherits_computer_use {
        emit(events, Action::ComputerUseAvailable { task_id: fork_id });
    }
    if let Some(error) = title_error {
        emit(
            events,
            Action::SetStatus(format!(
                "Chat created, but its title could not be copied: {error}"
            )),
        );
    } else {
        emit(events, Action::ClearStatus);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn retry_safety_buffered_turn(
    app_server: &AppServerConnection,
    source_task_id: String,
    turn_id: String,
    faster_model: String,
    mut submission: RetryableTurnSubmission,
    computer_capable_threads: &mut HashSet<String>,
    events: &Sender<Action>,
    browser: &mut Option<BrowserRuntime>,
    browser_download_preferences: &BrowserDownloadPreferences,
    browser_permissions: &BrowserPermissionsState,
    retryable_turns: &mut HashMap<(String, String), RetryableTurnSubmission>,
) {
    let prompt = submission.messages.first().cloned();
    if app_server
        .interrupt_turn(TurnInterruptParams {
            thread_id: source_task_id.clone(),
            turn_id: turn_id.clone(),
        })
        .is_err()
    {
        emit_safety_buffered_retry_failure(
            events,
            source_task_id,
            turn_id,
            None,
            None,
            "Could not stop the buffered response. Try the faster model again.",
        );
        return;
    }

    let turns = match app_server.list_thread_turns(ThreadTurnsListParams {
        thread_id: source_task_id.clone(),
        limit: 2,
        sort_direction: HistorySortDirection::Desc,
        cursor: None,
        items_view: Some("summary".to_owned()),
    }) {
        Ok(page) => page.data,
        Err(_) => {
            emit_safety_buffered_retry_failure(
                events,
                source_task_id.clone(),
                turn_id.clone(),
                Some(source_task_id),
                prompt,
                "Could not verify the interrupted response. Your prompt was restored.",
            );
            return;
        }
    };
    if safety_retry_fork_point(&turns, &turn_id).is_err() {
        emit_safety_buffered_retry_failure(
            events,
            source_task_id.clone(),
            turn_id.clone(),
            Some(source_task_id),
            prompt,
            "The buffered response changed before it could be retried. Your prompt was restored.",
        );
        return;
    }

    let response = match app_server.fork_thread(ThreadForkParams {
        thread_id: source_task_id.clone(),
        cwd: None,
        last_turn_id: None,
        before_turn_id: Some(turn_id.clone()),
        exclude_turns: Some(true),
        defer_goal_continuation: Some(true),
    }) {
        Ok(response) => response,
        Err(_) => {
            emit_safety_buffered_retry_failure(
                events,
                source_task_id.clone(),
                turn_id.clone(),
                Some(source_task_id),
                prompt,
                "Could not create the faster-model chat. Your prompt was restored.",
            );
            return;
        }
    };

    let task = map_task(response.thread);
    let retry_task_id = task.id.clone();
    let inherits_computer_use = computer_capable_threads.contains(&source_task_id);
    if inherits_computer_use {
        computer_capable_threads.insert(retry_task_id.clone());
    }
    submission.model = Some(faster_model);
    submission.effort = Some("low".to_owned());
    emit(events, Action::TaskCreated(task));
    if inherits_computer_use {
        emit(
            events,
            Action::ComputerUseAvailable {
                task_id: retry_task_id.clone(),
            },
        );
    }
    emit(
        events,
        Action::TaskSettingsLoaded {
            task_id: retry_task_id.clone(),
            model: submission.model.clone(),
            effort: submission.effort.clone(),
            service_tier: submission.service_tier.clone(),
            permissions: submission.permissions.clone(),
            approval_policy: submission.approval_policy.clone(),
            approvals_reviewer: submission.approvals_reviewer,
        },
    );

    if start_turn(
        app_server,
        StartTurnRequest {
            task_id: retry_task_id.clone(),
            submission,
        },
        events,
        browser,
        browser_download_preferences,
        browser_permissions,
        retryable_turns,
    )
    .is_err()
    {
        emit_safety_buffered_retry_failure(
            events,
            source_task_id,
            turn_id,
            Some(retry_task_id),
            prompt,
            "The faster-model chat was created, but the response could not start. Your prompt was restored.",
        );
    }
}

fn safety_retry_fork_point(turns: &[Value], turn_id: &str) -> Result<(), &'static str> {
    let Some(latest) = turns.first() else {
        return Err("the interrupted turn is missing");
    };
    if string_field(latest, "id").as_deref() != Some(turn_id) {
        return Err("the interrupted turn is no longer latest");
    }
    if string_field(latest, "status").as_deref() == Some("inProgress") {
        return Err("the interrupted turn is still in progress");
    }
    if turns
        .get(1)
        .and_then(|turn| string_field(turn, "status"))
        .as_deref()
        == Some("inProgress")
    {
        return Err("the previous turn is still in progress");
    }
    Ok(())
}

fn emit_safety_buffered_retry_failure(
    events: &Sender<Action>,
    source_task_id: String,
    turn_id: String,
    restore_task_id: Option<String>,
    prompt: Option<RetryableUserMessage>,
    message: &str,
) {
    emit(
        events,
        Action::SafetyBufferedRetryFailed {
            source_task_id,
            turn_id,
            restore_task_id,
            prompt,
            message: message.to_owned(),
        },
    );
}

fn start_turn(
    app_server: &AppServerConnection,
    request: StartTurnRequest,
    events: &Sender<Action>,
    browser: &mut Option<BrowserRuntime>,
    browser_download_preferences: &BrowserDownloadPreferences,
    browser_permissions: &BrowserPermissionsState,
    retryable_turns: &mut HashMap<(String, String), RetryableTurnSubmission>,
) -> Result<(), AppServerError> {
    let StartTurnRequest {
        task_id,
        submission,
    } = request;
    if let Err(message) = ensure_browser_context(
        browser,
        &task_id,
        events,
        browser_download_preferences,
        browser_permissions,
    ) {
        emit(
            events,
            Action::BrowserFailed {
                task_id: Some(task_id.clone()),
                message,
            },
        );
    }
    let _ = app_server.resume_thread(ThreadResumeParams {
        thread_id: task_id.clone(),
        exclude_turns: Some(true),
        initial_turns_page: Some(ThreadResumeInitialTurnsPageParams {
            cursor: None,
            limit: 1,
            sort_direction: HistorySortDirection::Desc,
            items_view: Some("summary".to_owned()),
        }),
    });
    let collaboration_mode = submission
        .plan_mode
        .then(|| {
            submission.model.clone().map(|model| CollaborationMode {
                mode: CollaborationModeKind::Plan,
                settings: CollaborationModeSettings {
                    model,
                    reasoning_effort: submission.effort.clone(),
                    developer_instructions: None,
                },
            })
        })
        .flatten();
    let response = app_server.start_turn(TurnStartParams {
        thread_id: task_id.clone(),
        input: retryable_submission_inputs(&submission.messages),
        client_user_message_id: None,
        cwd: None,
        runtime_workspace_roots: None,
        approval_policy: submission.approval_policy.clone(),
        approvals_reviewer: submission
            .approvals_reviewer
            .map(protocol_approvals_reviewer),
        permissions: submission.permissions.clone(),
        model: submission.model.clone(),
        effort: submission.effort.clone(),
        service_tier: Some(submission.service_tier.clone()),
        summary: None,
        personality: Some(Some(submission.personality.as_str().to_owned())),
        output_schema: None,
        collaboration_mode,
    })?;
    let turn_id = string_field(&response.turn, "id").unwrap_or_default();
    if !turn_id.is_empty() {
        retryable_turns.retain(|(thread_id, _), _| thread_id != &task_id);
        if retryable_turns.len() == MAX_RETRYABLE_ACTIVE_TURNS
            && let Some(entry_to_evict) = retryable_turns.keys().next().cloned()
        {
            retryable_turns.remove(&entry_to_evict);
        }
        retryable_turns.insert((task_id.clone(), turn_id.clone()), submission.clone());
        emit(
            events,
            Action::TurnStarted {
                task_id: task_id.clone(),
                turn_id: turn_id.clone(),
            },
        );
        emit(
            events,
            Action::TurnSubmissionRecorded {
                task_id,
                turn_id,
                submission,
            },
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn generate_commit_message(
    app_server: &AppServerConnection,
    root: &Path,
    include_unstaged: bool,
    commit_instructions: &str,
    events: &Sender<Action>,
    pending_approvals: &mut HashMap<String, PendingApproval>,
    computer_permissions: &mut HashMap<String, ComputerUsePermission>,
    computer_allowed_app_ids: &mut HashSet<String>,
    computer_accessibility: &mut ComputerUseAccessibilityClient,
    computer_url_policy: &mut ComputerUseUrlPolicy,
) -> Result<String, String> {
    let diff = git_commit_message_diff(root, include_unstaged)
        .map_err(|error| format!("failed to inspect changes: {error}"))?;
    let prompt = commit_generation_prompt(&diff.text, commit_instructions);
    let output = generate_structured_git_output(
        app_server,
        prompt,
        commit_message_output_schema(),
        MAX_GENERATED_COMMIT_RESPONSE_BYTES,
        events,
        pending_approvals,
        computer_permissions,
        computer_allowed_app_ids,
        computer_accessibility,
        computer_url_policy,
    )?;
    parse_generated_commit_message(&output)
        .ok_or_else(|| "the generator returned no valid commit message".to_owned())
}

#[allow(clippy::too_many_arguments)]
fn generate_structured_git_output(
    app_server: &AppServerConnection,
    prompt: String,
    output_schema: Value,
    response_limit: usize,
    events: &Sender<Action>,
    pending_approvals: &mut HashMap<String, PendingApproval>,
    computer_permissions: &mut HashMap<String, ComputerUsePermission>,
    computer_allowed_app_ids: &mut HashSet<String>,
    computer_accessibility: &mut ComputerUseAccessibilityClient,
    computer_url_policy: &mut ComputerUseUrlPolicy,
) -> Result<String, String> {
    let thread = app_server
        .start_thread(ThreadStartParams {
            model: Some(COMMIT_GENERATION_MODEL.to_owned()),
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
        })
        .map_err(|error| format!("failed to start the message generator: {error}"))?
        .thread;
    let thread_id = thread.id;
    let turn = match app_server.start_turn(TurnStartParams {
        thread_id: thread_id.clone(),
        input: vec![UserInput::text(prompt)],
        client_user_message_id: None,
        cwd: None,
        runtime_workspace_roots: Some(Vec::new()),
        approval_policy: None,
        approvals_reviewer: None,
        permissions: Some(":read-only".to_owned()),
        model: None,
        effort: None,
        service_tier: Some(None),
        summary: Some("none".to_owned()),
        personality: Some(None),
        output_schema: Some(output_schema),
        collaboration_mode: None,
    }) {
        Ok(response) => response.turn,
        Err(error) => {
            unsubscribe_thread(app_server, &thread_id);
            return Err(format!(
                "failed to start the message generator turn: {error}"
            ));
        }
    };
    let mut turn_id = string_field(&turn, "id");
    let mut output = String::new();
    let mut generation_error = None;
    let deadline = Instant::now() + COMMIT_GENERATION_TIMEOUT;
    let result = loop {
        let now = Instant::now();
        if now >= deadline {
            if let Some(turn_id) = turn_id.as_ref() {
                let _ = app_server.interrupt_turn(TurnInterruptParams {
                    thread_id: thread_id.clone(),
                    turn_id: turn_id.clone(),
                });
            }
            break Err("timed out waiting for generated Git messages".to_owned());
        }
        let wait = deadline
            .saturating_duration_since(now)
            .min(Duration::from_millis(50));
        let event = match app_server.recv_event_timeout(wait) {
            Ok(Some(event)) => event,
            Ok(None) => continue,
            Err(error) => {
                break Err(format!("lost the app-server connection: {error}"));
            }
        };
        match event {
            AppServerEvent::Notification { method, params }
                if string_field(&params, "threadId").as_deref() == Some(thread_id.as_str()) =>
            {
                let event_turn_id = string_field(&params, "turnId")
                    .or_else(|| params.get("turn").and_then(|turn| string_field(turn, "id")));
                if let (Some(expected), Some(actual)) = (turn_id.as_ref(), event_turn_id.as_ref())
                    && expected != actual
                {
                    continue;
                }
                match method.as_str() {
                    "turn/started" => {
                        turn_id = params
                            .get("turn")
                            .and_then(|turn| string_field(turn, "id"))
                            .or(turn_id);
                    }
                    "item/agentMessage/delta" => {
                        if let Some(delta) = string_field(&params, "delta") {
                            push_bounded(&mut output, &delta, response_limit);
                        }
                    }
                    "item/completed" => {
                        if let Some(item) = params.get("item")
                            && string_field(item, "type").as_deref() == Some("agentMessage")
                            && let Some(text) = string_field(item, "text")
                        {
                            output = bounded(text, response_limit);
                        }
                    }
                    "error" => {
                        generation_error = params.get("error").and_then(app_server_error_text);
                    }
                    "turn/completed" => {
                        let Some(turn) = params.get("turn") else {
                            continue;
                        };
                        let status = string_field(turn, "status").unwrap_or_default();
                        if status == "completed" {
                            break Ok(output);
                        }
                        let detail = turn
                            .get("error")
                            .and_then(app_server_error_text)
                            .or(generation_error);
                        break Err(structured_turn_error(&status, detail.as_deref()));
                    }
                    _ => {}
                }
            }
            AppServerEvent::Request { id, method, params }
                if string_field(&params, "threadId").as_deref() == Some(thread_id.as_str()) =>
            {
                let _ = app_server.respond_error(
                    &id,
                    -32601,
                    "Git message generation does not support tool calls",
                );
                generation_error = Some(format!(
                    "the generator requested unsupported client method {method}"
                ));
            }
            AppServerEvent::Disconnected => {
                handle_app_server_event(
                    app_server,
                    AppServerEvent::Disconnected,
                    events,
                    pending_approvals,
                    computer_permissions,
                    computer_allowed_app_ids,
                    computer_accessibility,
                    computer_url_policy,
                );
                break Err("the app-server connection closed".to_owned());
            }
            event => {
                if handle_app_server_event(
                    app_server,
                    event,
                    events,
                    pending_approvals,
                    computer_permissions,
                    computer_allowed_app_ids,
                    computer_accessibility,
                    computer_url_policy,
                ) {
                    emit(events, Action::RefreshGit);
                }
            }
        }
    };
    unsubscribe_thread(app_server, &thread_id);
    result
}

fn commit_generation_prompt(diff: &str, commit_instructions: &str) -> String {
    let context = diff.chars().take(COMMIT_GENERATION_PROMPT_CHARS);
    let header = [
        "Using the supplied git context below, generate a git commit message.",
        "Write the result into the structured response field message.",
        "message must contain plain commit-message text only, not JSON, field labels, or code fences.",
        "Custom commit instructions for message content and formatting override the fallback rules below.",
        "Make 0 tool calls.",
        "Bounds:",
        "- Keep the complete message under 4000 characters.",
        "- Keep the subject under 72 characters.",
        "Fallback rules:",
        "- Generate a concise single-line subject.",
        "- Use an imperative verb first.",
        "- Do not add a scope prefix unless the context already clearly uses one.",
        "- Do not include markdown, quotes, or trailing punctuation.",
    ]
    .join("\n");
    let commit_instructions = bounded(
        commit_instructions.trim().to_owned(),
        MAX_GIT_INSTRUCTIONS_BYTES,
    );
    format!(
        "{header}\n\nCustom commit instructions:\n{}\n\nDiff context:\n{}",
        if commit_instructions.is_empty() {
            "- (none)"
        } else {
            &commit_instructions
        },
        context.collect::<String>()
    )
}

fn commit_message_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "message": {
                "type": "string",
                "minLength": MIN_GENERATED_COMMIT_MESSAGE_CHARS,
                "maxLength": MAX_GENERATED_COMMIT_MESSAGE_CHARS,
                "pattern": "^[^\\r\\n]{1,72}(?:\\r?\\n[\\s\\S]*)?$"
            }
        },
        "required": ["message"],
        "additionalProperties": false
    })
}

fn parse_generated_commit_message(output: &str) -> Option<String> {
    let parsed = serde_json::from_str::<Value>(output.trim()).ok()?;
    let object = parsed.as_object()?;
    if object.len() != 1 {
        return None;
    }
    let message = object.get("message")?.as_str()?.trim();
    let message_chars = message.chars().count();
    let subject_chars = message.lines().next()?.chars().count();
    ((MIN_GENERATED_COMMIT_MESSAGE_CHARS..=MAX_GENERATED_COMMIT_MESSAGE_CHARS)
        .contains(&message_chars)
        && subject_chars > 0
        && subject_chars <= MAX_GENERATED_COMMIT_SUBJECT_CHARS
        && !message.contains('\0'))
    .then(|| message.to_owned())
}

struct GeneratedPullRequestDetails {
    title: String,
    body: String,
}

struct GeneratedGitMessages {
    commit_message: Option<String>,
    title: String,
    body: String,
}

#[allow(clippy::too_many_arguments)]
fn generate_pull_request_message(
    app_server: &AppServerConnection,
    root: &Path,
    base_branch: &str,
    head_branch: &str,
    title: &str,
    body: &str,
    include_uncommitted: bool,
    pull_request_instructions: &str,
    events: &Sender<Action>,
    pending_approvals: &mut HashMap<String, PendingApproval>,
    computer_permissions: &mut HashMap<String, ComputerUsePermission>,
    computer_allowed_app_ids: &mut HashSet<String>,
    computer_accessibility: &mut ComputerUseAccessibilityClient,
    computer_url_policy: &mut ComputerUseUrlPolicy,
) -> Result<GeneratedPullRequestDetails, String> {
    let context =
        pull_request_generation_context(root, base_branch, head_branch, include_uncommitted)?;
    let output = generate_structured_git_output(
        app_server,
        pull_request_generation_prompt(&context, pull_request_instructions),
        pull_request_output_schema(),
        MAX_GENERATED_GIT_RESPONSE_BYTES,
        events,
        pending_approvals,
        computer_permissions,
        computer_allowed_app_ids,
        computer_accessibility,
        computer_url_policy,
    )?;
    let generated = parse_generated_pull_request_message(&output)
        .ok_or_else(|| "the generator returned no valid pull request details".to_owned())?;
    let title = if title.trim().is_empty() {
        generated.title
    } else {
        title.trim().to_owned()
    };
    let body = if body.trim().is_empty() {
        generated.body
    } else {
        body.trim().to_owned()
    };
    valid_pull_request_values(&title, &body)
        .then_some(GeneratedPullRequestDetails { title, body })
        .ok_or_else(|| "the generator returned invalid pull request details".to_owned())
}

#[allow(clippy::too_many_arguments)]
fn generate_commit_pull_request_messages(
    app_server: &AppServerConnection,
    root: &Path,
    base_branch: &str,
    head_branch: &str,
    title: &str,
    body: &str,
    commit_instructions: &str,
    pull_request_instructions: &str,
    events: &Sender<Action>,
    pending_approvals: &mut HashMap<String, PendingApproval>,
    computer_permissions: &mut HashMap<String, ComputerUsePermission>,
    computer_allowed_app_ids: &mut HashSet<String>,
    computer_accessibility: &mut ComputerUseAccessibilityClient,
    computer_url_policy: &mut ComputerUseUrlPolicy,
) -> Result<GeneratedGitMessages, String> {
    let commit_diff = git_commit_message_diff(root, true)
        .map_err(|error| format!("failed to inspect changes: {error}"))?;
    let pull_request_context =
        pull_request_generation_context(root, base_branch, head_branch, true)?;
    let commit_instructions = bounded(
        commit_instructions.trim().to_owned(),
        MAX_GIT_INSTRUCTIONS_BYTES,
    );
    let pull_request_instructions = bounded(
        pull_request_instructions.trim().to_owned(),
        MAX_GIT_INSTRUCTIONS_BYTES,
    );
    let context = [
        "Custom instructions:",
        "Custom commit instructions:",
        if commit_instructions.is_empty() {
            "- (none)"
        } else {
            &commit_instructions
        },
        "Pull request instructions:",
        if pull_request_instructions.is_empty() {
            "- (none)"
        } else {
            &pull_request_instructions
        },
        "",
        "Commit message context:",
        &format!("Changes:\n{}", commit_diff.text),
        "",
        "Pull request context:",
        &pull_request_context,
    ]
    .join("\n");
    let output = generate_structured_git_output(
        app_server,
        combined_git_generation_prompt(&context),
        combined_git_output_schema(),
        MAX_GENERATED_GIT_RESPONSE_BYTES,
        events,
        pending_approvals,
        computer_permissions,
        computer_allowed_app_ids,
        computer_accessibility,
        computer_url_policy,
    )?;
    let mut generated = parse_generated_commit_pull_request_messages(&output).ok_or_else(|| {
        "the generator returned no valid commit and pull request details".to_owned()
    })?;
    if !title.trim().is_empty() {
        generated.title = title.trim().to_owned();
    }
    if !body.trim().is_empty() {
        generated.body = body.trim().to_owned();
    }
    if generated
        .commit_message
        .as_deref()
        .is_none_or(|message| parse_commit_message_value(message).is_none())
        || !valid_pull_request_values(&generated.title, &generated.body)
    {
        return Err("the generator returned invalid commit or pull request details".to_owned());
    }
    Ok(generated)
}

fn pull_request_generation_context(
    root: &Path,
    base_branch: &str,
    head_branch: &str,
    include_uncommitted: bool,
) -> Result<String, String> {
    let diff = git_pull_request_context(root, base_branch, include_uncommitted)
        .map_err(|error| format!("failed to inspect pull request changes: {error}"))?;
    let changes = if diff.text.trim().is_empty() {
        "- (no files listed)".to_owned()
    } else {
        diff.text
    };
    Ok([
        "Branches:",
        &format!("- Head: {head_branch}"),
        &format!("- Base: {base_branch}"),
        "",
        "Changes:",
        &changes,
    ]
    .join("\n"))
}

fn pull_request_generation_prompt(context: &str, pull_request_instructions: &str) -> String {
    let context = context
        .chars()
        .take(PULL_REQUEST_GENERATION_PROMPT_CHARS)
        .collect::<String>();
    let pull_request_instructions = bounded(
        pull_request_instructions.trim().to_owned(),
        MAX_GIT_INSTRUCTIONS_BYTES,
    );
    [
        "You are a helpful assistant. Generate a pull request title and body.",
        "Write the result into the structured response fields title and body.",
        "Make 0 tool calls.",
        "If context includes pull request instructions, follow them even when they conflict with the default rules below.",
        "Language rules:",
        "- Match the primary language of the supplied context; default to English.",
        "- Translate standard section headings such as Summary and Testing when writing in another language.",
        "Fallback PR title rules:",
        "- title must contain only the PR title, not JSON, field labels, or body content.",
        "- Use an imperative or action-oriented phrasing first.",
        "- Keep the title under 120 characters.",
        "- No trailing punctuation.",
        "Body rules:",
        "- body must contain only the PR body, not JSON, field labels, the title, or a full PR draft.",
        "- Do not repeat, restate, or label the title inside body.",
        "- Keep the body concise and scannable.",
        "- Keep the body under 30000 characters.",
        "- Use Markdown with short bullets.",
        "- Include a Summary section and a Testing section.",
        "- In Testing, describe meaningful validation at a high level, such as new unit or integration tests, or local UI testing with Playwright.",
        "- Do not paste command transcripts. For routine checks, summarize the result, for example: lint and formatting passed.",
        "- If tests were not run, say \"Not run (not requested)\".",
        "- If context includes pull request instructions, apply them to the title/body content only.",
        "",
        "Pull request instructions:",
        if pull_request_instructions.is_empty() {
            "- (none)"
        } else {
            &pull_request_instructions
        },
        "",
        "Context:",
        &context,
    ]
    .join("\n")
}

fn combined_git_generation_prompt(context: &str) -> String {
    let context = context
        .chars()
        .take(COMBINED_GIT_GENERATION_PROMPT_CHARS)
        .collect::<String>();
    [
        "Using the supplied commit and pull request contexts below, generate one git commit message plus one pull request title and body.",
        "Write the result into the structured response fields message, title, and body.",
        "message must contain plain commit-message text only, not JSON, field labels, or code fences.",
        "Make 0 tool calls.",
        "If context includes pull request instructions, follow them even when they conflict with the default pull request rules below.",
        "Custom commit instructions for message content and formatting apply to message only and override the fallback commit message rules below.",
        "Commit message bounds:",
        "- Keep the complete message under 4000 characters.",
        "- Keep the subject under 72 characters.",
        "Fallback commit message rules:",
        "- Generate a concise single-line subject.",
        "- Use an imperative verb first.",
        "- Do not add a scope prefix unless the context already clearly uses one.",
        "- Do not include markdown, quotes, or trailing punctuation.",
        "Pull request language rules:",
        "- Match the primary language of the supplied context; default to English.",
        "- Translate standard section headings such as Summary and Testing when writing in another language.",
        "Fallback PR title rules:",
        "- title must contain only the PR title, not JSON, field labels, or body content.",
        "- Use an imperative or action-oriented phrasing first.",
        "- Keep title under 120 characters.",
        "- No trailing punctuation.",
        "Pull request body rules:",
        "- body must contain only the PR body, not JSON, field labels, the title, or a full PR draft.",
        "- Do not repeat, restate, or label the title inside body.",
        "- Keep the body concise and scannable.",
        "- Keep the body under 30000 characters.",
        "- Use Markdown with short bullets.",
        "- Include a Summary section and a Testing section.",
        "- In Testing, describe meaningful validation at a high level, such as new unit or integration tests, or local UI testing with Playwright.",
        "- Do not paste command transcripts. For routine checks, summarize the result, for example: lint and formatting passed.",
        "- If tests were not run, say \"Not run (not requested)\".",
        "- If context includes pull request instructions, apply them to title/body only.",
        "",
        "Commit and pull request context:",
        &context,
    ]
    .join("\n")
}

fn pull_request_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "title": {
                "type": "string",
                "minLength": MIN_GENERATED_PULL_REQUEST_TITLE_CHARS,
                "maxLength": MAX_GENERATED_PULL_REQUEST_TITLE_CHARS
            },
            "body": {
                "type": "string",
                "minLength": MIN_GENERATED_PULL_REQUEST_BODY_CHARS,
                "maxLength": MAX_GENERATED_PULL_REQUEST_BODY_CHARS
            }
        },
        "required": ["title", "body"],
        "additionalProperties": false
    })
}

fn combined_git_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "message": commit_message_output_schema()["properties"]["message"].clone(),
            "title": {
                "type": "string",
                "minLength": MIN_GENERATED_PULL_REQUEST_TITLE_CHARS,
                "maxLength": MAX_GENERATED_PULL_REQUEST_TITLE_CHARS
            },
            "body": {
                "type": "string",
                "minLength": MIN_GENERATED_PULL_REQUEST_BODY_CHARS,
                "maxLength": MAX_GENERATED_PULL_REQUEST_BODY_CHARS
            }
        },
        "required": ["message", "title", "body"],
        "additionalProperties": false
    })
}

fn parse_generated_pull_request_message(output: &str) -> Option<GeneratedPullRequestDetails> {
    let parsed = serde_json::from_str::<Value>(output.trim()).ok()?;
    let object = parsed.as_object()?;
    if object.len() != 2 {
        return None;
    }
    let title = object.get("title")?.as_str()?.trim().to_owned();
    let body = object.get("body")?.as_str()?.trim().to_owned();
    valid_pull_request_details(&title, &body).then_some(GeneratedPullRequestDetails { title, body })
}

fn parse_generated_commit_pull_request_messages(output: &str) -> Option<GeneratedGitMessages> {
    let parsed = serde_json::from_str::<Value>(output.trim()).ok()?;
    let object = parsed.as_object()?;
    if object.len() != 3 {
        return None;
    }
    let commit_message = parse_commit_message_value(object.get("message")?.as_str()?)?;
    let title = object.get("title")?.as_str()?.trim().to_owned();
    let body = object.get("body")?.as_str()?.trim().to_owned();
    valid_pull_request_details(&title, &body).then_some(GeneratedGitMessages {
        commit_message: Some(commit_message),
        title,
        body,
    })
}

fn parse_commit_message_value(message: &str) -> Option<String> {
    let message = message.trim();
    let message_chars = message.chars().count();
    let subject_chars = message.lines().next()?.chars().count();
    ((MIN_GENERATED_COMMIT_MESSAGE_CHARS..=MAX_GENERATED_COMMIT_MESSAGE_CHARS)
        .contains(&message_chars)
        && subject_chars > 0
        && subject_chars <= MAX_GENERATED_COMMIT_SUBJECT_CHARS
        && !message.contains('\0'))
    .then(|| message.to_owned())
}

fn valid_pull_request_details(title: &str, body: &str) -> bool {
    (MIN_GENERATED_PULL_REQUEST_TITLE_CHARS..=MAX_GENERATED_PULL_REQUEST_TITLE_CHARS)
        .contains(&title.chars().count())
        && (MIN_GENERATED_PULL_REQUEST_BODY_CHARS..=MAX_GENERATED_PULL_REQUEST_BODY_CHARS)
            .contains(&body.chars().count())
        && !title.contains('\0')
        && !body.contains('\0')
}

fn valid_pull_request_values(title: &str, body: &str) -> bool {
    !title.is_empty()
        && title.chars().count() <= MAX_GENERATED_PULL_REQUEST_TITLE_CHARS
        && !body.is_empty()
        && body.chars().count() <= MAX_GENERATED_PULL_REQUEST_BODY_CHARS
        && !title.contains('\0')
        && !body.contains('\0')
}

fn app_server_error_text(error: &Value) -> Option<String> {
    let message = string_field(error, "message");
    let details = string_field(error, "additionalDetails");
    match (message, details) {
        (Some(message), Some(details)) => Some(format!("{message} {details}")),
        (Some(message), None) => Some(message),
        (None, Some(details)) => Some(details),
        (None, None) => None,
    }
}

fn structured_turn_error(status: &str, detail: Option<&str>) -> String {
    let status = if status.is_empty() { "unknown" } else { status };
    match detail.filter(|detail| !detail.is_empty()) {
        Some(detail) => format!("structured turn ended with status {status}: {detail}"),
        None => format!("structured turn ended with status {status}"),
    }
}

fn unsubscribe_thread(app_server: &AppServerConnection, thread_id: &str) {
    let _ = app_server.unsubscribe_thread(ThreadUnsubscribeParams {
        thread_id: thread_id.to_owned(),
    });
}

fn composer_rich_mention(name: &str, path: &Path) -> String {
    let label = name
        .replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]");
    let destination = path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace(')', "\\)");
    format!("[@{label}]({destination})")
}

fn composer_inputs(text: String, attachments: Vec<ComposerAttachment>) -> Vec<UserInput> {
    let mut input = Vec::with_capacity(attachments.len().saturating_add(1));
    let mut rich_mentions = Vec::new();
    let mut structured = Vec::new();
    for attachment in attachments {
        match attachment.kind {
            ComposerAttachmentKind::App | ComposerAttachmentKind::Plugin => {
                rich_mentions.push(composer_rich_mention(&attachment.name, &attachment.path));
            }
            ComposerAttachmentKind::Mention => {
                structured.push(UserInput::mention(attachment.name, attachment.path));
            }
            ComposerAttachmentKind::LocalImage => {
                structured.push(UserInput::local_image(attachment.path));
            }
            ComposerAttachmentKind::Skill => {
                structured.push(UserInput::skill(attachment.name, attachment.path));
            }
        }
    }
    let text = if rich_mentions.is_empty() {
        text
    } else if text.is_empty() {
        rich_mentions.join(" ")
    } else {
        format!("{} {text}", rich_mentions.join(" "))
    };
    if !text.is_empty() {
        input.push(UserInput::text(text));
    }
    input.extend(structured);
    input
}

fn retryable_submission_inputs(messages: &[RetryableUserMessage]) -> Vec<UserInput> {
    let mut input = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        if index > 0 {
            input.push(UserInput::text("\n"));
        }
        input.extend(composer_inputs(
            message.text.clone(),
            message.attachments.clone(),
        ));
    }
    input
}

fn record_retryable_steer(
    retryable_turns: &mut HashMap<(String, String), RetryableTurnSubmission>,
    task_id: &str,
    turn_id: &str,
    message: &RetryableUserMessage,
) {
    let key = (task_id.to_owned(), turn_id.to_owned());
    let cache_overflowed = retryable_turns.get_mut(&key).is_some_and(|submission| {
        if submission.messages.len() == MAX_RETRYABLE_TURN_MESSAGES {
            true
        } else {
            submission.messages.push(message.clone());
            false
        }
    });
    if cache_overflowed {
        retryable_turns.remove(&key);
    }
}

fn start_terminal(
    tab_id: u64,
    cwd: PathBuf,
    shell: Option<IntegratedTerminalShell>,
    terminals: &mut HashMap<u64, TerminalRuntime>,
    events: &Sender<Action>,
) {
    if terminals.len() >= MAX_TERMINAL_TABS {
        emit(
            events,
            Action::TerminalStartFailed {
                tab_id,
                message: format!(
                    "failed to open terminal: maximum of {MAX_TERMINAL_TABS} sessions reached"
                ),
            },
        );
        return;
    }
    match TerminalSession::spawn(TerminalConfig::new(cwd).with_shell(shell)) {
        Ok(session) => {
            let process_id = session
                .process_id()
                .map_or_else(|| "terminal".to_owned(), |id| id.to_string());
            terminals.insert(
                tab_id,
                TerminalRuntime {
                    session,
                    parser: vt100::Parser::new_with_callbacks(
                        24,
                        80,
                        2_000,
                        TerminalParserCallbacks::default(),
                    ),
                    truncation_reported: false,
                    reported_title: String::new(),
                },
            );
            emit(events, Action::TerminalStarted { tab_id, process_id });
        }
        Err(error) => emit(
            events,
            Action::TerminalStartFailed {
                tab_id,
                message: format!("failed to open terminal: {error}"),
            },
        ),
    }
}

fn handle_browser_effect(
    effect: &Effect,
    events: &Sender<Action>,
    browser: &mut Option<BrowserRuntime>,
    browser_download_preferences: &mut BrowserDownloadPreferences,
    browser_permissions: &mut BrowserPermissionsState,
) -> bool {
    match effect {
        Effect::StartBrowser { task_id } | Effect::ActivateBrowser { task_id } => {
            if let Err(message) = ensure_browser_context(
                browser,
                task_id,
                events,
                browser_download_preferences,
                browser_permissions,
            ) {
                emit(
                    events,
                    Action::BrowserFailed {
                        task_id: Some(task_id.clone()),
                        message,
                    },
                );
            }
            true
        }
        Effect::BrowserNavigate { task_id, url } => {
            run_browser_command(browser, task_id, events, |session| {
                session.navigate(task_id, url)
            });
            true
        }
        Effect::BrowserBack { task_id } => {
            run_browser_command(browser, task_id, events, |session| session.back(task_id));
            true
        }
        Effect::BrowserForward { task_id } => {
            run_browser_command(browser, task_id, events, |session| session.forward(task_id));
            true
        }
        Effect::BrowserReload { task_id } => {
            run_browser_command(browser, task_id, events, |session| session.reload(task_id));
            true
        }
        Effect::BrowserStop { task_id } => {
            run_browser_command(browser, task_id, events, |session| session.stop(task_id));
            true
        }
        Effect::BrowserOpenTab { task_id, url } => {
            let already_running = browser.is_some();
            match ensure_browser_context(
                browser,
                task_id,
                events,
                browser_download_preferences,
                browser_permissions,
            ) {
                Ok(()) if already_running => {
                    run_browser_command(browser, task_id, events, |session| {
                        session.open_tab(task_id, url.as_deref())
                    });
                }
                Ok(()) => {}
                Err(message) => emit(
                    events,
                    Action::BrowserFailed {
                        task_id: Some(task_id.clone()),
                        message,
                    },
                ),
            }
            true
        }
        Effect::BrowserSelectTab { task_id, tab_id } => {
            run_browser_command(browser, task_id, events, |session| {
                session.select_tab(task_id, tab_id)
            });
            true
        }
        Effect::BrowserCloseTab { task_id, tab_id } => {
            run_browser_command(browser, task_id, events, |session| {
                session.close_tab(task_id, tab_id)
            });
            true
        }
        Effect::BrowserResize {
            task_id,
            width,
            height,
        } => {
            run_browser_command(browser, task_id, events, |session| {
                session.resize(*width, *height)
            });
            true
        }
        Effect::BrowserSurfaceState { task_id, visible } => {
            let Some(runtime) = browser.as_ref() else {
                return true;
            };
            if let Err(error) = runtime
                .session
                .sync_surface_state(task_id.as_deref(), *visible)
            {
                emit(
                    events,
                    Action::BrowserFailed {
                        task_id: task_id.clone(),
                        message: format!("Unable to synchronize the Browser surface: {error}"),
                    },
                );
            }
            true
        }
        Effect::BrowserClick {
            task_id,
            x,
            y,
            button,
        } => {
            let button = match button {
                CoreBrowserMouseButton::Left => PlatformBrowserMouseButton::Left,
                CoreBrowserMouseButton::Middle => PlatformBrowserMouseButton::Middle,
                CoreBrowserMouseButton::Right => PlatformBrowserMouseButton::Right,
            };
            run_browser_command(browser, task_id, events, |session| {
                session.click(task_id, *x, *y, button)
            });
            true
        }
        Effect::BrowserScroll {
            task_id,
            x,
            y,
            delta_x,
            delta_y,
        } => {
            run_browser_command(browser, task_id, events, |session| {
                session.scroll(task_id, *x, *y, *delta_x, *delta_y)
            });
            true
        }
        Effect::BrowserKey { task_id, input } => {
            run_browser_command(browser, task_id, events, |session| {
                session.key(
                    task_id,
                    PlatformBrowserKeyInput {
                        key: input.key.clone(),
                        text: input.text.clone(),
                        alt: input.alt,
                        control: input.control,
                        meta: input.meta,
                        shift: input.shift,
                    },
                )
            });
            true
        }
        Effect::ConfigureBrowserDownloads(preferences) => {
            let preferences = preferences.clone().normalized();
            *browser_download_preferences = preferences.clone();
            if browser.is_some() {
                let download_directory = preferences
                    .download_directory
                    .clone()
                    .or_else(default_browser_download_dir);
                if let Some(download_directory) = download_directory {
                    run_browser_download_command(browser, events, |session| {
                        session.set_download_directory(&download_directory)
                    });
                }
                run_browser_download_command(browser, events, |session| {
                    session.set_prompt_for_user_downloads(preferences.prompt_for_user_downloads)
                });
            }
            true
        }
        Effect::ConfigureBrowserPermissions(permissions) => {
            let permissions = permissions.clone().normalized();
            *browser_permissions = permissions.clone();
            if browser.is_some() {
                run_browser_download_command(browser, events, |session| {
                    session.set_permissions(permissions)
                });
            }
            true
        }
        Effect::BrowserCancelDownload { id } => {
            run_browser_download_command(browser, events, |session| session.cancel_download(id));
            true
        }
        Effect::BrowserPauseDownload { id } => {
            run_browser_download_command(browser, events, |session| session.pause_download(id));
            true
        }
        Effect::BrowserResumeDownload { id } => {
            run_browser_download_command(browser, events, |session| session.resume_download(id));
            true
        }
        Effect::BrowserOpenDownload { id } => {
            run_browser_download_command(browser, events, |session| session.open_download(id));
            true
        }
        Effect::BrowserRemoveDownload { id } => {
            run_browser_download_command(browser, events, |session| session.remove_download(id));
            true
        }
        Effect::BrowserShowDownloadInFolder { id } => {
            run_browser_download_command(browser, events, |session| {
                session.show_download_in_folder(id)
            });
            true
        }
        Effect::BrowserShowDownloadsFolder => {
            run_browser_download_command(browser, events, BrowserSession::show_downloads_folder);
            true
        }
        Effect::BrowserSetDownloadDestination { id, path } => {
            run_browser_download_command(browser, events, |session| {
                session.set_download_destination(id, path.as_deref())
            });
            true
        }
        _ => false,
    }
}

fn ensure_browser_context(
    browser: &mut Option<BrowserRuntime>,
    task_id: &str,
    events: &Sender<Action>,
    browser_download_preferences: &BrowserDownloadPreferences,
    browser_permissions: &BrowserPermissionsState,
) -> Result<(), String> {
    if let Some(runtime) = browser.as_mut() {
        runtime.contexts.insert(task_id.to_owned());
        runtime
            .session
            .activate_context(task_id)
            .map_err(|error| format!("Unable to activate the Browser: {error}"))?;
        if let Some(executable) = runtime.executable.clone() {
            emit(
                events,
                Action::BrowserReady {
                    task_id: task_id.to_owned(),
                    executable,
                },
            );
        }
        return Ok(());
    }

    let profile_dir = codexrs_data_dir()
        .map_err(|error| format!("Unable to prepare the Browser profile: {error}"))?
        .join("browser")
        .join("profile");
    let session = BrowserSession::spawn(
        BrowserConfig::new(profile_dir, task_id.to_owned())
            .with_download_dir(
                browser_download_preferences
                    .download_directory
                    .clone()
                    .or_else(default_browser_download_dir),
            )
            .with_prompt_for_user_downloads(browser_download_preferences.prompt_for_user_downloads)
            .with_permissions(browser_permissions.clone()),
    )
    .map_err(|error| format!("Unable to start the Browser: {error}"))?;
    *browser = Some(BrowserRuntime {
        session,
        contexts: HashSet::from([task_id.to_owned()]),
        executable: None,
    });
    Ok(())
}

fn run_browser_command(
    browser: &mut Option<BrowserRuntime>,
    task_id: &str,
    events: &Sender<Action>,
    command: impl FnOnce(&BrowserSession) -> Result<(), codex_platform::BrowserCommandError>,
) {
    let Some(runtime) = browser.as_ref() else {
        emit(
            events,
            Action::BrowserFailed {
                task_id: Some(task_id.to_owned()),
                message: "The Browser is not running.".to_owned(),
            },
        );
        return;
    };
    if let Err(error) = command(&runtime.session) {
        emit(
            events,
            Action::BrowserFailed {
                task_id: Some(task_id.to_owned()),
                message: format!("Unable to control the Browser: {error}"),
            },
        );
    }
}

fn run_browser_download_command(
    browser: &mut Option<BrowserRuntime>,
    events: &Sender<Action>,
    command: impl FnOnce(&BrowserSession) -> Result<(), codex_platform::BrowserCommandError>,
) {
    let Some(runtime) = browser.as_ref() else {
        emit(
            events,
            Action::BrowserOperationFailed("The Browser is not running.".to_owned()),
        );
        return;
    };
    if let Err(error) = command(&runtime.session) {
        emit(
            events,
            Action::BrowserOperationFailed(format!("Unable to manage Browser downloads: {error}")),
        );
    }
}

fn drain_browser(browser: &mut Option<BrowserRuntime>, events: &Sender<Action>) -> bool {
    let Some(runtime) = browser.as_mut() else {
        return false;
    };
    for _ in 0..64 {
        match runtime.session.try_recv_event() {
            Ok(Some(BrowserEvent::Ready { executable })) => {
                runtime.executable = Some(executable.clone());
                let mut contexts = runtime.contexts.iter().cloned().collect::<Vec<_>>();
                contexts.sort();
                for task_id in contexts {
                    emit(
                        events,
                        Action::BrowserReady {
                            task_id,
                            executable: executable.clone(),
                        },
                    );
                }
            }
            Ok(Some(BrowserEvent::TabsChanged {
                context_id,
                tabs,
                active_tab_id,
            })) => {
                runtime.contexts.insert(context_id.clone());
                emit(
                    events,
                    Action::BrowserTabsChanged {
                        task_id: context_id,
                        tabs: tabs
                            .into_iter()
                            .map(|tab| BrowserTabState {
                                id: tab.id,
                                url: tab.url,
                                title: tab.title,
                                loading: tab.loading,
                                can_go_back: tab.can_go_back,
                                can_go_forward: tab.can_go_forward,
                            })
                            .collect(),
                        active_tab_id,
                    },
                );
            }
            Ok(Some(BrowserEvent::Frame {
                context_id,
                tab_id,
                jpeg,
                width,
                height,
            })) => emit(
                events,
                Action::BrowserFrameReady {
                    task_id: context_id,
                    tab_id,
                    jpeg: Arc::from(jpeg),
                    width,
                    height,
                },
            ),
            Ok(Some(BrowserEvent::VisibilityRequested {
                context_id,
                visible,
            })) => emit(
                events,
                Action::BrowserVisibilityRequested {
                    task_id: context_id,
                    visible,
                },
            ),
            Ok(Some(BrowserEvent::DownloadChanged(download))) => emit(
                events,
                Action::BrowserDownloadChanged(BrowserDownloadState {
                    can_cancel: download.can_cancel,
                    can_pause: download.can_pause,
                    can_resume: download.can_resume,
                    context_id: download.context_id,
                    file_exists: download.file_exists,
                    filename: download.filename,
                    id: download.id,
                    path: download.path,
                    received_bytes: download.received_bytes,
                    started_at_ms: download.started_at_ms,
                    status: match download.status {
                        PlatformBrowserDownloadStatus::Started => {
                            CoreBrowserDownloadStatus::Started
                        }
                        PlatformBrowserDownloadStatus::InProgress => {
                            CoreBrowserDownloadStatus::InProgress
                        }
                        PlatformBrowserDownloadStatus::Paused => CoreBrowserDownloadStatus::Paused,
                        PlatformBrowserDownloadStatus::Failed => CoreBrowserDownloadStatus::Failed,
                        PlatformBrowserDownloadStatus::Canceled => {
                            CoreBrowserDownloadStatus::Canceled
                        }
                        PlatformBrowserDownloadStatus::Complete => {
                            CoreBrowserDownloadStatus::Complete
                        }
                    },
                    total_bytes: download.total_bytes,
                    updated_at_ms: download.updated_at_ms,
                    url: download.url,
                    user_initiated: download.user_initiated,
                }),
            ),
            Ok(Some(BrowserEvent::DownloadSaveRequested {
                directory,
                filename,
                id,
            })) => emit(
                events,
                Action::BrowserDownloadSaveRequested {
                    directory,
                    filename,
                    id,
                },
            ),
            Ok(Some(BrowserEvent::DownloadRemoved { id })) => {
                emit(events, Action::BrowserDownloadRemoved { id });
            }
            Ok(Some(BrowserEvent::OperationFailed(message))) => {
                emit(events, Action::BrowserOperationFailed(message));
            }
            Ok(Some(BrowserEvent::Failed(message))) => emit(
                events,
                Action::BrowserFailed {
                    task_id: None,
                    message,
                },
            ),
            Ok(Some(BrowserEvent::Exited)) => {
                emit(events, Action::BrowserExited);
                return true;
            }
            Ok(None) => break,
            Err(_) => {
                emit(events, Action::BrowserExited);
                return true;
            }
        }
    }
    false
}

fn drain_terminals(terminals: &mut HashMap<u64, TerminalRuntime>, events: &Sender<Action>) {
    let tab_ids = terminals.keys().copied().collect::<Vec<_>>();
    for tab_id in tab_ids {
        let Some(runtime) = terminals.get_mut(&tab_id) else {
            continue;
        };
        let mut screen_changed = false;
        let mut exit_code = None;
        for _ in 0..64 {
            match runtime.session.try_recv_event() {
                Ok(Some(TerminalEvent::Output(bytes))) => {
                    runtime.parser.process(&bytes);
                    screen_changed = true;
                }
                Ok(Some(TerminalEvent::Exited { code })) => {
                    exit_code = Some(code);
                    break;
                }
                Ok(Some(TerminalEvent::Failed(message))) => {
                    emit(events, Action::SetStatus(message.to_owned()));
                }
                Ok(None) => break,
                Err(_) => {
                    exit_code = Some(127);
                    break;
                }
            }
        }
        if screen_changed {
            emit(
                events,
                Action::TerminalScreen {
                    tab_id,
                    screen: runtime.parser.screen().contents(),
                },
            );
        }
        let title = runtime.parser.callbacks().title.trim();
        if !title.is_empty() && title != runtime.reported_title {
            runtime.reported_title = title.to_owned();
            emit(
                events,
                Action::TerminalTitleChanged {
                    tab_id,
                    title: title.to_owned(),
                },
            );
        }
        if runtime.session.output_was_truncated() && !runtime.truncation_reported {
            runtime.truncation_reported = true;
            emit(events, Action::TerminalOutputTruncated(tab_id));
        }
        if let Some(code) = exit_code {
            terminals.remove(&tab_id);
            emit(events, Action::TerminalExited { tab_id, code });
        }
    }
}

const fn platform_pull_request_relationship(
    relationship: PullRequestRelationship,
) -> GitHubPullRequestRelationship {
    match relationship {
        PullRequestRelationship::All => GitHubPullRequestRelationship::All,
        PullRequestRelationship::Authored => GitHubPullRequestRelationship::Authored,
        PullRequestRelationship::ReviewRequested => GitHubPullRequestRelationship::ReviewRequested,
        PullRequestRelationship::Reviewed => GitHubPullRequestRelationship::Reviewed,
    }
}

const fn platform_pull_request_lifecycle(
    lifecycle: PullRequestLifecycle,
) -> GitHubPullRequestLifecycle {
    match lifecycle {
        PullRequestLifecycle::All => GitHubPullRequestLifecycle::All,
        PullRequestLifecycle::Open => GitHubPullRequestLifecycle::Open,
        PullRequestLifecycle::Merged => GitHubPullRequestLifecycle::Merged,
        PullRequestLifecycle::Closed => GitHubPullRequestLifecycle::Closed,
    }
}

const fn core_pull_request_state(state: GitHubPullRequestState) -> PullRequestState {
    match state {
        GitHubPullRequestState::Open => PullRequestState::Open,
        GitHubPullRequestState::Closed => PullRequestState::Closed,
        GitHubPullRequestState::Merged => PullRequestState::Merged,
    }
}

const fn core_pull_request_ci_status(status: GitHubCiStatus) -> PullRequestCiStatus {
    match status {
        GitHubCiStatus::None => PullRequestCiStatus::None,
        GitHubCiStatus::Pending => PullRequestCiStatus::Pending,
        GitHubCiStatus::Passing => PullRequestCiStatus::Passing,
        GitHubCiStatus::Failing => PullRequestCiStatus::Failing,
    }
}

fn core_pull_request_summary(summary: GitHubPullRequestSummary) -> PullRequestSummary {
    PullRequestSummary {
        identity: PullRequestIdentity {
            hostname: summary.identity.hostname,
            owner: summary.identity.owner,
            repository: summary.identity.repository,
            number: summary.identity.number,
        },
        node_id: summary.node_id,
        title: summary.title,
        url: summary.url,
        state: core_pull_request_state(summary.state),
        is_draft: summary.is_draft,
        author_login: summary.author_login,
        base_branch: summary.base_branch,
        head_branch: summary.head_branch,
        additions: summary.additions,
        deletions: summary.deletions,
        created_at: summary.created_at,
        updated_at: summary.updated_at,
        ci_status: core_pull_request_ci_status(summary.ci_status),
        is_author: summary.is_author,
    }
}

fn core_pull_request_detail(detail: GitHubPullRequestDetail) -> PullRequestDetail {
    PullRequestDetail {
        summary: core_pull_request_summary(detail.summary),
        body: detail.body,
        head_revision: detail.head_revision,
        review_decision: detail.review_decision,
        mergeable: detail.mergeable,
        merge_state_status: detail.merge_state_status,
        checks: detail
            .checks
            .into_iter()
            .map(core_pull_request_check)
            .collect(),
        activity: detail
            .activity
            .into_iter()
            .map(core_pull_request_activity)
            .collect(),
        checks_partial: detail.checks_partial,
        activity_partial: detail.activity_partial,
    }
}

fn core_pull_request_check(check: GitHubPullRequestCheck) -> PullRequestCheck {
    PullRequestCheck {
        name: check.name,
        workflow: check.workflow,
        status: match check.status {
            GitHubCheckStatus::Pending => PullRequestCheckStatus::Pending,
            GitHubCheckStatus::Passing => PullRequestCheckStatus::Passing,
            GitHubCheckStatus::Failing => PullRequestCheckStatus::Failing,
            GitHubCheckStatus::Neutral => PullRequestCheckStatus::Neutral,
            GitHubCheckStatus::Skipped => PullRequestCheckStatus::Skipped,
            GitHubCheckStatus::Unknown => PullRequestCheckStatus::Unknown,
        },
        description: check.description,
        link: check.link,
        started_at: check.started_at,
        completed_at: check.completed_at,
    }
}

fn core_pull_request_activity(activity: GitHubPullRequestActivity) -> PullRequestActivity {
    PullRequestActivity {
        id: activity.id,
        kind: match activity.kind {
            GitHubPullRequestActivityKind::Event => PullRequestActivityKind::Event,
            GitHubPullRequestActivityKind::Comment => PullRequestActivityKind::Comment,
            GitHubPullRequestActivityKind::Review => PullRequestActivityKind::Review,
            GitHubPullRequestActivityKind::ReviewComment => PullRequestActivityKind::ReviewComment,
        },
        actor_login: activity.actor_login,
        body: activity.body,
        created_at: activity.created_at,
        event: activity.event,
        url: activity.url,
        path: activity.path,
        line: activity.line,
        start_line: activity.start_line,
        review_thread_id: activity.review_thread_id,
    }
}

fn map_git_snapshot(snapshot: GitSnapshot) -> GitState {
    let changed_files = snapshot.files.len();
    let staged_files = snapshot.files.iter().filter(|file| file.staged).count();
    GitState {
        refresh_generation: 0,
        repository_root: Some(snapshot.repository_root),
        branch: snapshot.branch,
        default_branch: snapshot.default_branch,
        review_default_base: snapshot.review_default_base,
        upstream_ref: snapshot.upstream_ref,
        ahead: snapshot.ahead,
        behind: snapshot.behind,
        changed_files,
        staged_files,
        files: snapshot
            .files
            .into_iter()
            .map(|file| GitFileState {
                path: file.path,
                old_path: file.old_path,
                kind: match file.kind {
                    PlatformGitFileKind::Added => CoreGitFileKind::Added,
                    PlatformGitFileKind::Modified => CoreGitFileKind::Modified,
                    PlatformGitFileKind::Deleted => CoreGitFileKind::Deleted,
                    PlatformGitFileKind::Renamed => CoreGitFileKind::Renamed,
                    PlatformGitFileKind::Copied => CoreGitFileKind::Copied,
                    PlatformGitFileKind::Untracked => CoreGitFileKind::Untracked,
                    PlatformGitFileKind::Conflicted => CoreGitFileKind::Conflicted,
                    PlatformGitFileKind::TypeChanged => CoreGitFileKind::TypeChanged,
                },
                staged: file.staged,
                unstaged: file.unstaged,
                staged_additions: file.staged_additions,
                staged_deletions: file.staged_deletions,
                unstaged_additions: file.unstaged_additions,
                unstaged_deletions: file.unstaged_deletions,
            })
            .collect(),
        branches: snapshot
            .branches
            .into_iter()
            .map(|branch| GitBranchState {
                name: branch.name,
                commit: branch.commit,
                current: branch.current,
            })
            .collect(),
        review_branches: snapshot.review_branches,
        worktrees: snapshot
            .worktrees
            .into_iter()
            .map(|worktree| GitWorktreeState {
                path: worktree.path,
                branch: worktree.branch,
                bare: worktree.bare,
                detached: worktree.detached,
                locked: worktree.locked,
            })
            .collect(),
        commits: snapshot
            .commits
            .into_iter()
            .map(|commit| GitReviewCommitState {
                sha: commit.sha,
                subject: commit.subject,
                message: commit.message,
                committed_at: commit.committed_at,
            })
            .collect(),
        selected_commit_sha: None,
        selected_review_base: None,
        diff_generation: 0,
        selected_scope: codex_core::GitDiffScope::default(),
        selected_path: None,
        unified_diff: String::new(),
        diff_status: None,
        diff_base_sha: None,
        diff_error: None,
        diff_truncated: false,
        truncated: snapshot.truncated,
        pending_branch_operation: None,
        branch_mutation_error: None,
        branch_conflict: None,
        pending_commit: None,
        commit_error: None,
        pull_request_provider: GitPullRequestProvider::Unknown,
        pull_request: None,
        pending_pull_request: None,
        pull_request_error: None,
    }
}

fn connect(
    events: &Sender<Action>,
    connection: &mut Option<AppServerConnection>,
) -> Result<(), String> {
    if connection.is_some() {
        emit(events, Action::Connected);
        return Ok(());
    }

    let binary = resolve_codex_binary(None);
    let home = CodexHome::resolve(None).map_err(|error| error.to_string())?;
    let runtime_binary = binary.clone();
    let runtime_home = home.path().to_path_buf();
    let runtime_home_default = home.kind() == CodexHomeKind::Default;
    let app_server = AppServerConnection::spawn(AppServerConfig::new(binary, home))
        .map_err(|error| error.to_string())?;
    match app_server.initialize_with_capabilities(
        ClientInfo {
            name: "codex-rs".to_owned(),
            title: Some("codexRS".to_owned()),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        },
        Some(initialize_capabilities()),
    ) {
        Ok(_) => {
            *connection = Some(app_server);
            emit(
                events,
                Action::RuntimeResolved {
                    codex_binary: runtime_binary,
                    codex_home: runtime_home,
                    codex_home_default: runtime_home_default,
                },
            );
            emit(events, Action::Connected);
            Ok(())
        }
        Err(error) => Err(error.to_string()),
    }
}

fn computer_use_dynamic_tools_for_platform() -> Option<Vec<DynamicToolSpec>> {
    computer_use_dynamic_tools_for_platform_with_available(computer_use_platform_available())
}

fn computer_use_dynamic_tools_for_platform_with_available(
    platform_available: bool,
) -> Option<Vec<DynamicToolSpec>> {
    #[cfg(windows)]
    {
        let _ = platform_available;
        Some(computer_use_dynamic_tools())
    }

    #[cfg(target_os = "linux")]
    {
        platform_available.then(linux_computer_use_dynamic_tools)
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = platform_available;
        None
    }
}

fn computer_use_tool_supported_on_platform(tool: &str) -> bool {
    #[cfg(windows)]
    {
        let _ = tool;
        true
    }

    #[cfg(target_os = "linux")]
    {
        matches!(
            tool,
            "list_windows" | "get_window" | "list_apps" | "get_window_state"
        )
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = tool;
        false
    }
}

#[cfg(any(windows, test))]
fn computer_use_dynamic_tools() -> Vec<DynamicToolSpec> {
    vec![DynamicToolSpec::Namespace {
        name: "computer_use".to_owned(),
        description: "List and launch installed apps, inspect open desktop windows, and control \
                      an exact Window object returned by discovery. The client asks for app \
                      permission before launch, reading, or control."
            .to_owned(),
        tools: vec![
            dynamic_tool(
                "list_windows",
                "List the bounded set of currently open targetable desktop windows.",
                json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "get_window",
                "Rehydrate one currently open window by its opaque id.",
                json!({
                    "type": "object",
                    "required": ["id"],
                    "properties": {
                        "id": {"type": "integer", "minimum": 0, "maximum": u32::MAX},
                        "app": {"type": "string", "minLength": 1, "maxLength": 512}
                    },
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "list_apps",
                "List the bounded installed-app catalog with currently open targetable windows.",
                json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "launch_app",
                "Launch an app by canonical id from list_apps, or by an explicit absolute .exe identifier.",
                json!({
                    "type": "object",
                    "required": ["app"],
                    "properties": {
                        "app": {"type": "string", "minLength": 1, "maxLength": 512}
                    },
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "get_window_state",
                "Capture bounded screenshot and accessibility state for an exact open window.",
                json!({
                    "type": "object",
                    "required": ["window"],
                    "properties": {
                        "include_text": {"type": "boolean", "default": false},
                        "include_screenshot": {"type": "boolean", "default": true},
                        "window": computer_window_schema()
                    },
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "click",
                "Click an indexed accessibility element or a coordinate in the exact target window.",
                json!({
                    "type": "object",
                    "required": ["window"],
                    "properties": {
                        "element_index": {"type": "integer", "minimum": 0},
                        "x": {"type": "number", "minimum": i32::MIN, "maximum": i32::MAX},
                        "y": {"type": "number", "minimum": i32::MIN, "maximum": i32::MAX},
                        "screenshotId": {"type": "string", "minLength": 1, "maxLength": 128},
                        "mouse_button": {
                            "type": "string",
                            "enum": ["left", "right", "middle", "l", "r", "m"]
                        },
                        "click_count": {"type": "number", "minimum": 1, "maximum": 3},
                        "window": computer_window_schema()
                    },
                    "anyOf": [
                        {"required": ["element_index"]},
                        {"required": ["x", "y"]}
                    ],
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "press_key",
                "Press one key or a plus-separated X keysym-style chord in the exact target window.",
                json!({
                    "type": "object",
                    "required": ["key", "window"],
                    "properties": {
                        "key": {"type": "string", "minLength": 1, "maxLength": 64},
                        "window": computer_window_schema()
                    },
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "type_text",
                "Type bounded Unicode text into the current focus in the exact target window.",
                json!({
                    "type": "object",
                    "required": ["text", "window"],
                    "properties": {
                        "text": {"type": "string", "maxLength": 16384},
                        "window": computer_window_schema()
                    },
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "scroll",
                "Scroll from a coordinate in the exact target window. Positive y scrolls down and positive x scrolls right.",
                json!({
                    "type": "object",
                    "required": ["scrollX", "scrollY", "window", "x", "y"],
                    "properties": {
                        "x": {"type": "number", "minimum": i32::MIN, "maximum": i32::MAX},
                        "y": {"type": "number", "minimum": i32::MIN, "maximum": i32::MAX},
                        "screenshotId": {"type": "string", "minLength": 1, "maxLength": 128},
                        "scrollX": {"type": "number", "minimum": -10000, "maximum": 10000},
                        "scrollY": {"type": "number", "minimum": -10000, "maximum": 10000},
                        "window": computer_window_schema()
                    },
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "set_value",
                "Replace the value of an indexed editable accessibility element.",
                json!({
                    "type": "object",
                    "required": ["element_index", "value", "window"],
                    "properties": {
                        "element_index": {"type": "integer", "minimum": 0},
                        "value": {"type": "string", "maxLength": 16384},
                        "window": computer_window_schema()
                    },
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "drag",
                "Drag between two coordinates in the exact target window.",
                json!({
                    "type": "object",
                    "required": ["from_x", "from_y", "to_x", "to_y", "window"],
                    "properties": {
                        "from_x": {"type": "number", "minimum": i32::MIN, "maximum": i32::MAX},
                        "from_y": {"type": "number", "minimum": i32::MIN, "maximum": i32::MAX},
                        "to_x": {"type": "number", "minimum": i32::MIN, "maximum": i32::MAX},
                        "to_y": {"type": "number", "minimum": i32::MIN, "maximum": i32::MAX},
                        "screenshotId": {"type": "string", "minLength": 1, "maxLength": 128},
                        "window": computer_window_schema()
                    },
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "perform_secondary_action",
                "Invoke a secondary action exactly as listed for an indexed accessibility element.",
                json!({
                    "type": "object",
                    "required": ["element_index", "action", "window"],
                    "properties": {
                        "element_index": {"type": "integer", "minimum": 0},
                        "action": {"type": "string", "minLength": 1, "maxLength": 512},
                        "window": computer_window_schema()
                    },
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "activate_window",
                "Bring an exact open window to the foreground.",
                json!({
                    "type": "object",
                    "required": ["window"],
                    "properties": {
                        "window": computer_window_schema()
                    },
                    "additionalProperties": false
                }),
            ),
        ],
    }]
}

#[cfg(any(target_os = "linux", test))]
fn linux_computer_use_dynamic_tools() -> Vec<DynamicToolSpec> {
    vec![DynamicToolSpec::Namespace {
        name: "computer_use".to_owned(),
        description: "List open X11/XWayland desktop windows and capture a bounded screenshot of an exact approved window."
            .to_owned(),
        tools: vec![
            dynamic_tool(
                "list_windows",
                "List the bounded set of currently open targetable desktop windows.",
                json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "get_window",
                "Rehydrate one currently open window by its opaque id.",
                json!({
                    "type": "object",
                    "required": ["id"],
                    "properties": {
                        "id": {"type": "integer", "minimum": 0, "maximum": u32::MAX},
                        "app": {"type": "string", "minLength": 1, "maxLength": 512}
                    },
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "list_apps",
                "List running targetable applications and their open windows.",
                json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
            ),
            dynamic_tool(
                "get_window_state",
                "Capture a bounded screenshot of an exact open X11/XWayland window.",
                json!({
                    "type": "object",
                    "required": ["include_screenshot", "window"],
                    "properties": {
                        "include_text": {"enum": [false], "default": false},
                        "include_screenshot": {"enum": [true]},
                        "window": computer_window_schema()
                    },
                    "additionalProperties": false
                }),
            ),
        ],
    }]
}

#[cfg(any(windows, target_os = "linux", test))]
fn computer_window_schema() -> Value {
    json!({
        "type": "object",
        "required": ["app", "id"],
        "properties": {
            "app": {"type": "string", "minLength": 1, "maxLength": 512},
            "id": {"type": "integer", "minimum": 0, "maximum": u32::MAX},
            "title": {"type": "string", "maxLength": 512}
        },
        "additionalProperties": false
    })
}

#[cfg(any(windows, target_os = "linux", test))]
fn dynamic_tool(name: &str, description: &str, input_schema: Value) -> DynamicToolNamespaceTool {
    DynamicToolNamespaceTool::new(DynamicToolFunction {
        name: name.to_owned(),
        description: description.to_owned(),
        input_schema,
        defer_loading: None,
    })
}

#[allow(clippy::too_many_arguments)]
fn handle_dynamic_tool_call(
    app_server: &AppServerConnection,
    id: &Value,
    params: Value,
    events: &Sender<Action>,
    pending_approvals: &mut HashMap<String, PendingApproval>,
    permissions: &mut HashMap<String, ComputerUsePermission>,
    always_allowed_app_ids: &mut HashSet<String>,
    computer_accessibility: &mut ComputerUseAccessibilityClient,
    computer_url_policy: &mut ComputerUseUrlPolicy,
    mut computer_overlay: Option<&mut ComputerUseSystemOverlay>,
) {
    let params = match serde_json::from_value::<DynamicToolCallParams>(params) {
        Ok(params) => params,
        Err(_) => {
            let _ = app_server.respond_error(id, -32602, "invalid dynamic tool arguments");
            return;
        }
    };
    if params.namespace.as_deref() != Some("computer_use") {
        respond_dynamic_tool_failure(app_server, id, "unsupported dynamic tool namespace");
        return;
    }
    if !computer_use_tool_supported_on_platform(&params.tool) {
        respond_dynamic_tool_failure(
            app_server,
            id,
            "this Computer Use tool is not supported on this platform",
        );
        return;
    }
    let Some(permission) = permissions.get_mut(&params.thread_id) else {
        respond_dynamic_tool_failure(app_server, id, "Computer Use is disabled for this task");
        return;
    };
    if !permission.enabled {
        respond_dynamic_tool_failure(app_server, id, "Computer Use is disabled for this task");
        return;
    }
    if matches!(
        params.tool.as_str(),
        "list_apps" | "list_windows" | "get_window"
    ) {
        complete_computer_discovery_tool_call(app_server, id, &params, computer_accessibility);
        return;
    }
    if params.tool == "launch_app" {
        handle_computer_launch_call(
            app_server,
            id,
            params,
            events,
            pending_approvals,
            permission,
            always_allowed_app_ids,
            computer_accessibility,
            computer_overlay,
        );
        return;
    }
    let (window_id, requested_application_id) = match computer_window_argument(&params.arguments) {
        Ok(window) => window,
        Err(message) => {
            respond_dynamic_tool_failure(app_server, id, &message);
            return;
        }
    };
    let window = match inspect_computer_window(&window_id) {
        Ok(window) => window,
        Err(error) => {
            respond_dynamic_tool_failure(app_server, id, &error.to_string());
            return;
        }
    };
    if !computer_app_id_matches(&requested_application_id, &window.application_id) {
        respond_dynamic_tool_failure(
            app_server,
            id,
            "the open window no longer belongs to the requested app",
        );
        return;
    }
    if let Some(message) =
        forbidden_computer_target_message(&window.application_id, &window.application)
    {
        respond_dynamic_tool_failure(app_server, id, &message);
        return;
    }
    let Some(application_id) = normalized_computer_app_id(&window.application_id) else {
        respond_dynamic_tool_failure(
            app_server,
            id,
            "the requested app does not expose a stable application identifier",
        );
        return;
    };
    let application_name = if window.application.trim().is_empty() {
        application_id.clone()
    } else {
        bounded(
            window.application.trim().to_owned(),
            MAX_COMPUTER_APP_ID_BYTES,
        )
    };
    if params.tool != "get_window_state"
        && computer_accessibility.user_input_requires_refresh(&window_id)
    {
        respond_dynamic_tool_failure(app_server, id, COMPUTER_USE_USER_INPUT_STALE_MESSAGE);
        return;
    }
    if let Err(message) =
        computer_url_policy.enforce_and_block(app_server, computer_accessibility, &params, &window)
    {
        if let Some(overlay) = computer_overlay.as_deref_mut() {
            let _ = overlay.complete_turn(&params.thread_id, &params.turn_id);
        }
        respond_dynamic_tool_failure(app_server, id, message);
        return;
    }

    let persistently_allowed =
        computer_use_policy_contains(always_allowed_app_ids, &application_id);
    if computer_use_app_authorized(permission, always_allowed_app_ids, &application_id) {
        permission.input_authorized = true;
        permission.authorized_application_id = Some(application_id.clone());
        if persistently_allowed {
            emit(
                events,
                Action::ComputerUseAppAuthorized {
                    task_id: params.thread_id.clone(),
                    application_id,
                    always_allowed: true,
                },
            );
        }
        complete_computer_tool_call(
            app_server,
            id,
            &params,
            &window,
            events,
            computer_accessibility,
            computer_overlay,
        );
        return;
    }

    if pending_approvals.len() >= MAX_PENDING_APPROVALS {
        respond_dynamic_tool_failure(
            app_server,
            id,
            "the approval queue is full; retry after resolving another request",
        );
        return;
    }
    let request_id = request_key(id);
    if pending_approvals.contains_key(&request_id) {
        respond_dynamic_tool_failure(app_server, id, "duplicate approval request");
        return;
    }
    pending_approvals.insert(
        request_id.clone(),
        PendingApproval::ComputerUse {
            id: id.clone(),
            params: params.clone(),
            window_id,
            application_id: application_id.clone(),
            application_name: application_name.clone(),
        },
    );
    emit(
        events,
        Action::ApprovalRequested(ApprovalRequest {
            request_id,
            task_id: params.thread_id,
            turn_id: Some(params.turn_id),
            kind: ApprovalKind::DynamicTool,
            title: format!("Allow ChatGPT to use “{application_name}”?"),
            detail: computer_use_approval_detail(&application_name),
            context: ApprovalContext::DynamicTool,
        }),
    );
}

fn complete_computer_discovery_tool_call(
    app_server: &AppServerConnection,
    id: &Value,
    params: &DynamicToolCallParams,
    computer_accessibility: &mut ComputerUseAccessibilityClient,
) {
    match run_computer_discovery_tool(&params.tool, &params.arguments, computer_accessibility) {
        Ok(content_items) => {
            let _ = app_server.respond_success(
                id,
                &DynamicToolCallResponse {
                    content_items,
                    success: true,
                },
            );
        }
        Err(message) => respond_dynamic_tool_failure(app_server, id, &message),
    }
}

fn run_computer_discovery_tool(
    tool: &str,
    arguments: &Value,
    computer_accessibility: &mut ComputerUseAccessibilityClient,
) -> Result<Vec<DynamicToolCallOutputContentItem>, String> {
    match tool {
        "list_apps" => {
            let applications = computer_accessibility
                .list_apps()
                .map_err(|error| error.to_string())?;
            let value = applications
                .iter()
                .map(computer_application_value)
                .collect::<Vec<_>>();
            Ok(vec![text_content(Value::Array(value).to_string())])
        }
        "list_windows" => {
            let windows = list_computer_windows().map_err(|error| error.to_string())?;
            let value = windows
                .iter()
                .map(computer_window_value)
                .collect::<Vec<_>>();
            Ok(vec![text_content(Value::Array(value).to_string())])
        }
        "get_window" => {
            let window_id = arguments
                .get("id")
                .and_then(|value| {
                    value
                        .as_u64()
                        .and_then(|value| u32::try_from(value).ok())
                        .or_else(|| value.as_str()?.trim().parse::<u32>().ok())
                })
                .map(|value| value.to_string())
                .ok_or_else(|| "id must be a non-negative window id".to_owned())?;
            let window = inspect_computer_window(&window_id).map_err(|error| error.to_string())?;
            if let Some(expected_app) = optional_string_argument(arguments, "app")?
                && !computer_app_id_matches(&expected_app, &window.application_id)
            {
                return Err("the open window no longer belongs to the requested app".to_owned());
            }
            Ok(vec![text_content(
                computer_window_value(&window).to_string(),
            )])
        }
        _ => Err("unsupported Computer Use discovery tool".to_owned()),
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_computer_launch_call(
    app_server: &AppServerConnection,
    id: &Value,
    params: DynamicToolCallParams,
    events: &Sender<Action>,
    pending_approvals: &mut HashMap<String, PendingApproval>,
    permission: &ComputerUsePermission,
    always_allowed_app_ids: &mut HashSet<String>,
    computer_accessibility: &mut ComputerUseAccessibilityClient,
    computer_overlay: Option<&mut ComputerUseSystemOverlay>,
) {
    if !permission.enabled {
        respond_dynamic_tool_failure(app_server, id, "Computer Use is disabled for this task");
        return;
    }
    let requested_id = match string_argument(&params.arguments, "app") {
        Ok(value) => value.trim(),
        Err(message) => {
            respond_dynamic_tool_failure(app_server, id, &message);
            return;
        }
    };
    let applications = match computer_accessibility.list_apps() {
        Ok(applications) => applications,
        Err(error) => {
            respond_dynamic_tool_failure(app_server, id, &error.to_string());
            return;
        }
    };
    let discovered = applications
        .iter()
        .find(|application| application.id.eq_ignore_ascii_case(requested_id));
    let application_id = discovered
        .map(|application| application.id.as_str())
        .unwrap_or(requested_id);
    if let Err(error) = computer_accessibility.validate_app_launch(application_id) {
        respond_dynamic_tool_failure(app_server, id, &error.to_string());
        return;
    }
    let Some(application_id) = normalized_computer_app_id(application_id) else {
        respond_dynamic_tool_failure(
            app_server,
            id,
            "app must be a canonical id from list_apps or an absolute .exe identifier",
        );
        return;
    };
    let application_name = discovered
        .and_then(|application| application.display_name.clone())
        .or_else(|| {
            PathBuf::from(requested_id)
                .file_stem()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| application_id.clone());
    if let Some(message) = forbidden_computer_target_message(&application_id, &application_name) {
        respond_dynamic_tool_failure(app_server, id, &message);
        return;
    }

    if computer_use_policy_contains(always_allowed_app_ids, &application_id) {
        if let Err(message) = begin_computer_overlay(computer_overlay, &params, None, events) {
            respond_dynamic_tool_failure(app_server, id, &message);
            return;
        }
        match computer_accessibility.launch_app(&application_id) {
            Ok(()) => {
                let _ = app_server.respond_success(
                    id,
                    &DynamicToolCallResponse {
                        content_items: vec![text_content(format!(
                            "Launched “{application_name}”."
                        ))],
                        success: true,
                    },
                );
            }
            Err(error) => respond_dynamic_tool_failure(app_server, id, &error.to_string()),
        }
        return;
    }
    if pending_approvals.len() >= MAX_PENDING_APPROVALS {
        respond_dynamic_tool_failure(
            app_server,
            id,
            "the approval queue is full; retry after resolving another request",
        );
        return;
    }
    let request_id = request_key(id);
    if pending_approvals.contains_key(&request_id) {
        respond_dynamic_tool_failure(app_server, id, "duplicate approval request");
        return;
    }
    pending_approvals.insert(
        request_id.clone(),
        PendingApproval::ComputerUseLaunch {
            id: id.clone(),
            params: params.clone(),
            application_id: application_id.clone(),
            application_name: application_name.clone(),
        },
    );
    emit(
        events,
        Action::ApprovalRequested(ApprovalRequest {
            request_id,
            task_id: params.thread_id,
            turn_id: Some(params.turn_id),
            kind: ApprovalKind::DynamicTool,
            title: format!("Allow ChatGPT to open “{application_name}”?"),
            detail: format!(
                "ChatGPT wants to launch “{application_name}”. Allow once for this task or always allow this app."
            ),
            context: ApprovalContext::DynamicTool,
        }),
    );
}

fn computer_use_approval_detail(application_name: &str) -> String {
    #[cfg(windows)]
    {
        format!(
            "ChatGPT can see and control “{application_name}” on your computer. Allow once for this task or always allow this app."
        )
    }

    #[cfg(not(windows))]
    {
        format!(
            "ChatGPT can observe “{application_name}” on your computer. Allow once for this task."
        )
    }
}

fn computer_application_value(application: &ComputerApplication) -> Value {
    let mut value = serde_json::Map::from_iter([
        ("id".to_owned(), Value::String(application.id.clone())),
        ("isRunning".to_owned(), Value::Bool(application.is_running)),
        (
            "windows".to_owned(),
            Value::Array(
                application
                    .windows
                    .iter()
                    .map(computer_window_value)
                    .collect(),
            ),
        ),
    ]);
    if let Some(display_name) = application.display_name.as_ref() {
        value.insert(
            "displayName".to_owned(),
            Value::String(display_name.clone()),
        );
    }
    if let Some(last_used_date) = application.last_used_date.as_ref() {
        value.insert(
            "lastUsedDate".to_owned(),
            Value::String(last_used_date.clone()),
        );
    }
    if let Some(use_count) = application.use_count {
        value.insert("useCount".to_owned(), Value::from(use_count));
    }
    Value::Object(value)
}

fn computer_window_value(window: &ComputerWindow) -> Value {
    let id = window
        .id
        .parse::<u64>()
        .map(Value::from)
        .unwrap_or_else(|_| Value::String(window.id.clone()));
    json!({
        "app": window.application_id,
        "id": id,
        "title": window.title
    })
}

fn complete_computer_tool_call(
    app_server: &AppServerConnection,
    id: &Value,
    params: &DynamicToolCallParams,
    window: &ComputerWindow,
    events: &Sender<Action>,
    computer_accessibility: &mut ComputerUseAccessibilityClient,
    computer_overlay: Option<&mut ComputerUseSystemOverlay>,
) {
    if computer_tool_requires_interruption_monitor(&params.tool)
        && let Err(message) = begin_computer_overlay(computer_overlay, params, Some(window), events)
    {
        respond_dynamic_tool_failure(app_server, id, &message);
        return;
    }
    match run_computer_tool(
        &params.tool,
        &params.arguments,
        &params.thread_id,
        &window.id,
        events,
        computer_accessibility,
    ) {
        Ok(content_items) => {
            let _ = app_server.respond_success(
                id,
                &DynamicToolCallResponse {
                    content_items,
                    success: true,
                },
            );
        }
        Err(message) => respond_dynamic_tool_failure(app_server, id, &message),
    }
}

fn begin_computer_overlay(
    computer_overlay: Option<&mut ComputerUseSystemOverlay>,
    params: &DynamicToolCallParams,
    window: Option<&ComputerWindow>,
    events: &Sender<Action>,
) -> Result<(), String> {
    let Some(overlay) = computer_overlay else {
        return Err(COMPUTER_USE_OVERLAY_UNAVAILABLE_MESSAGE.to_owned());
    };
    let target = window.map(ComputerUseOverlayTarget::from_window);
    overlay
        .begin_turn(&params.thread_id, &params.turn_id, target)
        .map_err(|error| {
            emit(
                events,
                Action::SetStatus(format!("Computer Use system indicator failed: {error}")),
            );
            COMPUTER_USE_OVERLAY_UNAVAILABLE_MESSAGE.to_owned()
        })
}

fn normalized_computer_app_id(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > MAX_COMPUTER_APP_ID_BYTES {
        return None;
    }
    Some(value.to_lowercase())
}

fn forbidden_computer_target_message(
    application_id: &str,
    application_name: &str,
) -> Option<String> {
    if !computer_use_target_is_forbidden(application_id, application_name) {
        return None;
    }
    let label = if application_name.trim().is_empty() {
        application_id.trim()
    } else {
        application_name.trim()
    };
    Some(format!(
        "Computer Use cannot operate on “{}”; product policy blocks this app",
        bounded(label.to_owned(), MAX_COMPUTER_APP_ID_BYTES)
    ))
}

fn computer_use_app_authorized(
    permission: &ComputerUsePermission,
    always_allowed_app_ids: &HashSet<String>,
    application_id: &str,
) -> bool {
    computer_use_policy_contains(always_allowed_app_ids, application_id)
        || permission.input_authorized
            && permission.authorized_application_id.as_deref() == Some(application_id)
}

fn computer_use_policy_contains(
    always_allowed_app_ids: &HashSet<String>,
    application_id: &str,
) -> bool {
    always_allowed_app_ids
        .iter()
        .any(|configured| computer_app_id_matches(configured, application_id))
}

fn computer_use_allowed_app_ids(config: &ConfigReadResponse) -> Vec<String> {
    const MAX_CONFIG_LAYERS: usize = 64;

    let Some(layers) = config.layers.as_deref() else {
        return Vec::new();
    };
    for layer in layers.iter().take(MAX_CONFIG_LAYERS) {
        if layer.disabled_reason.is_some() {
            continue;
        }
        let Some(value) = layer
            .config
            .pointer("/computer_use/windows/always_allowed_app_ids")
        else {
            continue;
        };
        let mut app_ids = match value {
            Value::Object(values) => values
                .iter()
                .take(MAX_COMPUTER_ALLOWED_APPS + 1)
                .filter(|(_, allowed)| allowed.as_bool() == Some(true))
                .filter_map(|(app_id, _)| normalized_computer_app_id(app_id))
                .collect::<Vec<_>>(),
            Value::Array(values) => values
                .iter()
                .take(MAX_COMPUTER_ALLOWED_APPS + 1)
                .filter_map(Value::as_str)
                .filter_map(normalized_computer_app_id)
                .collect::<Vec<_>>(),
            _ => return Vec::new(),
        };
        app_ids.sort();
        app_ids.dedup();
        app_ids.truncate(MAX_COMPUTER_ALLOWED_APPS);
        return app_ids;
    }
    Vec::new()
}

fn computer_use_allowed_app_ids_value(app_ids: &[String]) -> Value {
    Value::Object(
        app_ids
            .iter()
            .take(MAX_COMPUTER_ALLOWED_APPS)
            .filter_map(|app_id| normalized_computer_app_id(app_id))
            .map(|app_id| (app_id, Value::Bool(true)))
            .collect(),
    )
}

fn persist_computer_use_allowed_app(
    app_server: &AppServerConnection,
    application_id: &str,
    events: &Sender<Action>,
    computer_allowed_app_ids: &mut HashSet<String>,
) -> bool {
    if computer_use_policy_contains(computer_allowed_app_ids, application_id) {
        return true;
    }
    if computer_allowed_app_ids.len() >= MAX_COMPUTER_ALLOWED_APPS {
        emit(
            events,
            Action::ComputerUsePolicyMutationFailed {
                app_id: application_id.to_owned(),
                message: "Unable to save allowed apps because the bounded list is full.".to_owned(),
            },
        );
        return false;
    }

    let mut app_ids = computer_allowed_app_ids.iter().cloned().collect::<Vec<_>>();
    app_ids.push(application_id.to_owned());
    app_ids.sort();
    app_ids.dedup();
    match app_server.batch_write_config(ConfigBatchWriteParams {
        edits: vec![ConfigEdit {
            key_path: "computer_use.windows.always_allowed_app_ids".to_owned(),
            value: computer_use_allowed_app_ids_value(&app_ids),
            merge_strategy: ConfigMergeStrategy::Upsert,
        }],
        file_path: None,
        expected_version: None,
        reload_user_config: true,
    }) {
        Ok(response) if response.status == ConfigWriteStatus::Ok => {
            computer_allowed_app_ids.insert(application_id.to_owned());
            true
        }
        Ok(_) => {
            emit(
                events,
                Action::SetStatus(
                    "Always allow was saved but overridden by higher-priority configuration; this task remains allowed once."
                        .to_owned(),
                ),
            );
            false
        }
        Err(error) => {
            emit(
                events,
                Action::ComputerUsePolicyMutationFailed {
                    app_id: application_id.to_owned(),
                    message: format!("Unable to save Computer Use allowed app: {error}"),
                },
            );
            false
        }
    }
}

fn run_computer_tool(
    tool: &str,
    arguments: &Value,
    task_id: &str,
    window_id: &str,
    events: &Sender<Action>,
    computer_accessibility: &mut ComputerUseAccessibilityClient,
) -> Result<Vec<DynamicToolCallOutputContentItem>, String> {
    #[cfg(target_os = "linux")]
    if tool == "get_window_state" {
        if optional_bool_argument(arguments, "include_text")?.unwrap_or(false) {
            return Err("Linux Computer Use observation does not support include_text".to_owned());
        }
        if optional_bool_argument(arguments, "include_screenshot")? != Some(true) {
            return Err(
                "Linux Computer Use observation requires include_screenshot=true".to_owned(),
            );
        }
    }
    if tool != "get_window_state" && computer_accessibility.user_input_requires_refresh(window_id) {
        return Err(COMPUTER_USE_USER_INPUT_STALE_MESSAGE.to_owned());
    }
    match tool {
        "get_window_state" => {
            let include_text = optional_bool_argument(arguments, "include_text")?.unwrap_or(false);
            let include_screenshot =
                optional_bool_argument(arguments, "include_screenshot")?.unwrap_or(true);
            if !include_text && !include_screenshot {
                return Err(
                    "get_window_state must request include_text, include_screenshot, or both"
                        .to_owned(),
                );
            }
            let window = inspect_computer_window(window_id).map_err(|error| error.to_string())?;
            let accessibility = include_text
                .then(|| computer_accessibility.get_state(window_id))
                .transpose()
                .map_err(|error| error.to_string())?;
            let capture = include_screenshot
                .then(|| capture_computer_window(window_id))
                .transpose()
                .map_err(|error| error.to_string())?;
            if let Some(capture) = capture.as_ref() {
                computer_accessibility.remember_capture(capture);
                emit(
                    events,
                    Action::ComputerCaptureReady {
                        task_id: task_id.to_owned(),
                        label: capture_label(capture),
                    },
                );
            }
            let mut content = vec![text_content(computer_state_description(
                &window,
                capture.as_ref(),
                accessibility.as_ref(),
            ))];
            if let Some(capture) = capture {
                content.push(DynamicToolCallOutputContentItem::InputImage {
                    image_url: capture.image_url,
                });
            }
            Ok(content)
        }
        "click" => {
            activate_computer_input_target(window_id, computer_accessibility)?;
            let button = match optional_string_argument(arguments, "mouse_button")?.as_deref() {
                None | Some("left" | "l") => ComputerButton::Left,
                Some("right" | "r") => ComputerButton::Right,
                Some("middle" | "m") => ComputerButton::Middle,
                Some(_) => {
                    return Err("mouse_button must be left, right, middle, l, r, or m".to_owned());
                }
            };
            let clicks = optional_rounded_i32_argument(arguments, "click_count")?.unwrap_or(1);
            let clicks = u8::try_from(clicks)
                .ok()
                .filter(|clicks| (1..=3).contains(clicks))
                .ok_or_else(|| "click_count must be 1, 2, or 3".to_owned())?;
            if let Some(element_index) = optional_usize_argument(arguments, "element_index")? {
                computer_accessibility
                    .click_element(window_id, element_index, button, clicks)
                    .map_err(|error| error.to_string())?;
                return Ok(vec![text_content(format!(
                    "Clicked accessibility element [{element_index}]."
                ))]);
            }
            let (x, y) = computer_coordinates(arguments)?;
            let (x, y) = screenshot_point(arguments, window_id, x, y, computer_accessibility)?;
            click_computer_window(window_id, x, y, button, clicks)
                .map_err(|error| error.to_string())?;
            Ok(vec![text_content(format!("Clicked ({x}, {y})."))])
        }
        "drag" => {
            activate_computer_input_target(window_id, computer_accessibility)?;
            let (from_x, from_y, to_x, to_y) = drag_coordinates(arguments)?;
            let (from_x, from_y) =
                screenshot_point(arguments, window_id, from_x, from_y, computer_accessibility)?;
            let (to_x, to_y) =
                screenshot_point(arguments, window_id, to_x, to_y, computer_accessibility)?;
            drag_computer_window(window_id, from_x, from_y, to_x, to_y)
                .map_err(|error| error.to_string())?;
            Ok(vec![text_content(format!(
                "Dragged from ({from_x}, {from_y}) to ({to_x}, {to_y})."
            ))])
        }
        "scroll" => {
            activate_computer_input_target(window_id, computer_accessibility)?;
            let (x, y) = computer_coordinates(arguments)?;
            let (x, y) = screenshot_point(arguments, window_id, x, y, computer_accessibility)?;
            let delta_x = rounded_i32_argument(arguments, "scrollX")?;
            let delta_y = rounded_i32_argument(arguments, "scrollY")?;
            scroll_computer_window(window_id, x, y, delta_x, delta_y)
                .map_err(|error| error.to_string())?;
            Ok(vec![text_content(format!(
                "Scrolled at ({x}, {y}) by ({delta_x}, {delta_y})."
            ))])
        }
        "set_value" => {
            activate_computer_input_target(window_id, computer_accessibility)?;
            let element_index = usize_argument(arguments, "element_index")?;
            let value = string_argument(arguments, "value")?;
            computer_accessibility
                .set_value(window_id, element_index, value)
                .map_err(|error| error.to_string())?;
            Ok(vec![text_content(format!(
                "Set accessibility element [{element_index}]."
            ))])
        }
        "perform_secondary_action" => {
            activate_computer_input_target(window_id, computer_accessibility)?;
            let element_index = usize_argument(arguments, "element_index")?;
            let action = string_argument(arguments, "action")?;
            computer_accessibility
                .perform_secondary_action(window_id, element_index, action)
                .map_err(|error| error.to_string())?;
            Ok(vec![text_content(format!(
                "Performed {action} on accessibility element [{element_index}]."
            ))])
        }
        "type_text" => {
            activate_computer_input_target(window_id, computer_accessibility)?;
            let text = string_argument(arguments, "text")?;
            type_into_computer_window(window_id, text).map_err(|error| error.to_string())?;
            Ok(vec![text_content(format!("Typed {} bytes.", text.len()))])
        }
        "press_key" => {
            activate_computer_input_target(window_id, computer_accessibility)?;
            let (key, modifiers) = parse_computer_key_chord(string_argument(arguments, "key")?)?;
            press_computer_key(window_id, key, &modifiers).map_err(|error| error.to_string())?;
            Ok(vec![text_content("Key pressed.".to_owned())])
        }
        "activate_window" => {
            computer_accessibility
                .activate_window(window_id)
                .map_err(|error| error.to_string())?;
            Ok(vec![text_content("Window activated.".to_owned())])
        }
        _ => Err("unsupported Computer Use tool".to_owned()),
    }
}

fn activate_computer_input_target(
    window_id: &str,
    computer_accessibility: &mut ComputerUseAccessibilityClient,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        computer_accessibility
            .activate_window(window_id)
            .map_err(|error| error.to_string())
    }

    #[cfg(not(windows))]
    {
        let _ = (window_id, computer_accessibility);
        Ok(())
    }
}

fn respond_dynamic_tool_failure(app_server: &AppServerConnection, id: &Value, message: &str) {
    let _ = app_server.respond_success(
        id,
        &DynamicToolCallResponse {
            content_items: vec![text_content(message.to_owned())],
            success: false,
        },
    );
}

fn text_content(text: String) -> DynamicToolCallOutputContentItem {
    DynamicToolCallOutputContentItem::InputText { text }
}

fn computer_coordinates(arguments: &Value) -> Result<(i32, i32), String> {
    Ok((
        rounded_i32_argument(arguments, "x")?,
        rounded_i32_argument(arguments, "y")?,
    ))
}

fn drag_coordinates(arguments: &Value) -> Result<(i32, i32, i32, i32), String> {
    Ok((
        rounded_i32_argument(arguments, "from_x")?,
        rounded_i32_argument(arguments, "from_y")?,
        rounded_i32_argument(arguments, "to_x")?,
        rounded_i32_argument(arguments, "to_y")?,
    ))
}

fn rounded_i32_argument(arguments: &Value, field: &str) -> Result<i32, String> {
    let value = arguments
        .get(field)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("{field} must be a finite number"))?;
    let rounded = (value + 0.5).floor();
    if rounded < f64::from(i32::MIN) || rounded > f64::from(i32::MAX) {
        return Err(format!("{field} is outside the supported range"));
    }
    Ok(rounded as i32)
}

fn optional_rounded_i32_argument(arguments: &Value, field: &str) -> Result<Option<i32>, String> {
    arguments
        .get(field)
        .map(|_| rounded_i32_argument(arguments, field))
        .transpose()
}

fn usize_argument(arguments: &Value, field: &str) -> Result<usize, String> {
    let value = arguments
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{field} must be a non-negative integer"))?;
    usize::try_from(value).map_err(|_| format!("{field} is outside the supported range"))
}

fn optional_usize_argument(arguments: &Value, field: &str) -> Result<Option<usize>, String> {
    arguments
        .get(field)
        .map(|_| usize_argument(arguments, field))
        .transpose()
}

fn optional_bool_argument(arguments: &Value, field: &str) -> Result<Option<bool>, String> {
    arguments
        .get(field)
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| format!("{field} must be a boolean"))
        })
        .transpose()
}

fn string_argument<'a>(arguments: &'a Value, field: &str) -> Result<&'a str, String> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{field} must be a string"))
}

fn optional_string_argument(arguments: &Value, field: &str) -> Result<Option<String>, String> {
    arguments
        .get(field)
        .map(|_| string_argument(arguments, field).map(str::to_owned))
        .transpose()
}

fn computer_window_argument(arguments: &Value) -> Result<(String, String), String> {
    let window = arguments
        .get("window")
        .and_then(Value::as_object)
        .ok_or_else(|| "window must be an object returned by Computer Use discovery".to_owned())?;
    let application_id = window
        .get("app")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= MAX_COMPUTER_APP_ID_BYTES
                && !value.chars().any(char::is_control)
        })
        .ok_or_else(|| "window.app must be a bounded application identifier".to_owned())?;
    let window_id = window
        .get("id")
        .and_then(|value| {
            value
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .or_else(|| value.as_str()?.trim().parse::<u32>().ok())
        })
        .ok_or_else(|| "window.id must be a non-negative 32-bit window id".to_owned())?;
    Ok((window_id.to_string(), application_id.to_owned()))
}

fn screenshot_point(
    arguments: &Value,
    window_id: &str,
    x: i32,
    y: i32,
    computer_accessibility: &ComputerUseAccessibilityClient,
) -> Result<(i32, i32), String> {
    let Some(screenshot_id) = optional_string_argument(arguments, "screenshotId")? else {
        return Ok((x, y));
    };
    computer_accessibility
        .map_screenshot_point(window_id, &screenshot_id, x, y)
        .map_err(|error| error.to_string())
}

fn parse_computer_key_chord(value: &str) -> Result<(ComputerKey, Vec<ComputerKey>), String> {
    let parts = value.split('+').map(str::trim).collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) || parts.len() > 5 {
        return Err("key must contain one key and at most four modifiers".to_owned());
    }
    let mut modifiers = Vec::with_capacity(parts.len().saturating_sub(1));
    for value in &parts[..parts.len() - 1] {
        let modifier = parse_computer_modifier(value)?;
        if modifiers.contains(&modifier) {
            return Err("duplicate modifier".to_owned());
        }
        modifiers.push(modifier);
    }
    Ok((parse_computer_key(parts[parts.len() - 1])?, modifiers))
}

fn parse_computer_modifier(value: &str) -> Result<ComputerKey, String> {
    match value.to_ascii_lowercase().as_str() {
        "alt" | "alt_l" | "alt_r" | "option" => Ok(ComputerKey::Alt),
        "control" | "control_l" | "control_r" | "ctrl" => Ok(ComputerKey::Control),
        "cmd" | "command" | "meta" | "meta_l" | "meta_r" | "os" | "super" | "super_l"
        | "super_r" | "win" | "windows" => Err("Windows/Meta key input is not allowed".to_owned()),
        "shift" | "shift_l" | "shift_r" => Ok(ComputerKey::Shift),
        _ => Err("unsupported modifier".to_owned()),
    }
}

fn parse_computer_key(value: &str) -> Result<ComputerKey, String> {
    let key = match value.to_ascii_lowercase().as_str() {
        "alt" | "alt_l" | "alt_r" | "option" => ComputerKey::Alt,
        "backspace" => ComputerKey::Backspace,
        "control" | "control_l" | "control_r" | "ctrl" => ComputerKey::Control,
        "delete" => ComputerKey::Delete,
        "down" | "arrowdown" => ComputerKey::Down,
        "end" => ComputerKey::End,
        "enter" | "return" => ComputerKey::Enter,
        "escape" | "esc" => ComputerKey::Escape,
        "home" => ComputerKey::Home,
        "left" | "arrowleft" => ComputerKey::Left,
        "cmd" | "command" | "meta" | "meta_l" | "meta_r" | "os" | "super" | "super_l"
        | "super_r" | "win" | "windows" => {
            return Err("Windows/Meta key input is not allowed".to_owned());
        }
        "kp_0" | "numpad_0" | "numpad0" => ComputerKey::Numpad0,
        "kp_1" | "numpad_1" | "numpad1" => ComputerKey::Numpad1,
        "kp_2" | "numpad_2" | "numpad2" => ComputerKey::Numpad2,
        "kp_3" | "numpad_3" | "numpad3" => ComputerKey::Numpad3,
        "kp_4" | "numpad_4" | "numpad4" => ComputerKey::Numpad4,
        "kp_5" | "numpad_5" | "numpad5" => ComputerKey::Numpad5,
        "kp_6" | "numpad_6" | "numpad6" => ComputerKey::Numpad6,
        "kp_7" | "numpad_7" | "numpad7" => ComputerKey::Numpad7,
        "kp_8" | "numpad_8" | "numpad8" => ComputerKey::Numpad8,
        "kp_9" | "numpad_9" | "numpad9" => ComputerKey::Numpad9,
        "kp_add" | "numpad_add" | "numpadadd" => ComputerKey::NumpadAdd,
        "kp_decimal" | "numpad_decimal" | "numpaddecimal" => ComputerKey::NumpadDecimal,
        "kp_divide" | "numpad_divide" | "numpaddivide" => ComputerKey::NumpadDivide,
        "kp_enter" | "numpad_enter" | "numpadenter" => ComputerKey::NumpadEnter,
        "kp_multiply" | "numpad_multiply" | "numpadmultiply" => ComputerKey::NumpadMultiply,
        "kp_subtract" | "numpad_subtract" | "numpadsubtract" => ComputerKey::NumpadSubtract,
        "pagedown" => ComputerKey::PageDown,
        "pageup" => ComputerKey::PageUp,
        "comma" => ComputerKey::Character(','),
        "less" => ComputerKey::Character('<'),
        "period" => ComputerKey::Character('.'),
        "greater" => ComputerKey::Character('>'),
        "slash" => ComputerKey::Character('/'),
        "question" => ComputerKey::Character('?'),
        "right" | "arrowright" => ComputerKey::Right,
        "shift" | "shift_l" | "shift_r" => ComputerKey::Shift,
        "space" => ComputerKey::Space,
        "tab" => ComputerKey::Tab,
        "up" | "arrowup" => ComputerKey::Up,
        _ => {
            let mut characters = value.chars();
            let Some(character) = characters.next() else {
                return Err("key must not be empty".to_owned());
            };
            if characters.next().is_some() {
                return Err("unsupported key name".to_owned());
            }
            ComputerKey::Character(character)
        }
    };
    Ok(key)
}

fn map_computer_application(application: ComputerApplication) -> ComputerApplicationState {
    ComputerApplicationState {
        id: application.id,
        display_name: application.display_name,
        last_used_date: application.last_used_date,
        use_count: application.use_count,
        is_running: application.is_running,
        window_count: application.windows.len(),
    }
}

fn map_computer_window(window: ComputerWindow) -> ComputerWindowState {
    ComputerWindowState {
        id: window.id,
        application: window.application,
        application_id: window.application_id,
        title: window.title,
        width: window.width,
        height: window.height,
        minimized: window.minimized,
        focused: window.focused,
    }
}

fn capture_label(capture: &ComputerCapture) -> String {
    format!(
        "Captured {}×{} ({} KiB)",
        capture.width,
        capture.height,
        capture.jpeg_bytes.div_ceil(1024)
    )
}

fn computer_state_description(
    window: &ComputerWindow,
    capture: Option<&ComputerCapture>,
    accessibility: Option<&ComputerAccessibilityState>,
) -> String {
    let screenshots = capture
        .map(|capture| {
            vec![json!({
                "id": capture.screenshot_id,
                "zIndex": 0,
                "originX": capture.window.x,
                "originY": capture.window.y,
                "width": capture.width,
                "height": capture.height
            })]
        })
        .unwrap_or_default();
    json!({
        "window": computer_window_value(window),
        "screenshots": screenshots,
        "accessibility": accessibility
    })
    .to_string()
}

fn search_fuzzy_files(
    app_server: &AppServerConnection,
    events: &Sender<Action>,
    runtime: &mut FuzzyFileSearchRuntime,
    session_id: String,
    roots: Vec<PathBuf>,
    query: String,
    start_session: bool,
) {
    if let Err(message) = validate_fuzzy_file_search_request(&session_id, &roots, &query) {
        emit(
            events,
            Action::FuzzyFileSearchFailed {
                session_id,
                query,
                message: message.to_owned(),
            },
        );
        return;
    }

    if runtime.support == FuzzyFileSearchSupport::Unsupported {
        run_legacy_fuzzy_file_search(app_server, events, runtime, session_id, roots, query);
        return;
    }

    let needs_start = start_session || runtime.session_id.as_deref() != Some(session_id.as_str());
    if needs_start {
        match start_fuzzy_file_search_session(app_server, &session_id, &roots) {
            Ok(_) => runtime.support = FuzzyFileSearchSupport::Supported,
            Err(error) if is_method_unsupported(&error) => {
                runtime.support = FuzzyFileSearchSupport::Unsupported;
                run_legacy_fuzzy_file_search(app_server, events, runtime, session_id, roots, query);
                return;
            }
            Err(error) => {
                emit_fuzzy_file_search_error(events, session_id, query, error);
                return;
            }
        }
    }
    runtime.session_id = Some(session_id.clone());
    runtime.roots.clone_from(&roots);

    match update_fuzzy_file_search_session(app_server, &session_id, &query) {
        Ok(_) => {
            runtime.support = FuzzyFileSearchSupport::Supported;
        }
        Err(error) if is_method_unsupported(&error) => {
            runtime.support = FuzzyFileSearchSupport::Unsupported;
            run_legacy_fuzzy_file_search(app_server, events, runtime, session_id, roots, query);
        }
        Err(error) if should_restart_fuzzy_file_search_session(&error) => {
            let restarted = start_fuzzy_file_search_session(app_server, &session_id, &roots)
                .and_then(|_| update_fuzzy_file_search_session(app_server, &session_id, &query));
            match restarted {
                Ok(_) => {
                    runtime.support = FuzzyFileSearchSupport::Supported;
                }
                Err(error) if is_method_unsupported(&error) => {
                    runtime.support = FuzzyFileSearchSupport::Unsupported;
                    run_legacy_fuzzy_file_search(
                        app_server, events, runtime, session_id, roots, query,
                    );
                }
                Err(error) => emit_fuzzy_file_search_error(events, session_id, query, error),
            }
        }
        Err(error) => emit_fuzzy_file_search_error(events, session_id, query, error),
    }
}

fn start_fuzzy_file_search_session(
    app_server: &AppServerConnection,
    session_id: &str,
    roots: &[PathBuf],
) -> Result<Value, AppServerError> {
    app_server.request(
        "fuzzyFileSearch/sessionStart",
        FuzzyFileSearchSessionStartParams {
            session_id: session_id.to_owned(),
            roots: roots.to_vec(),
        },
    )
}

fn update_fuzzy_file_search_session(
    app_server: &AppServerConnection,
    session_id: &str,
    query: &str,
) -> Result<Value, AppServerError> {
    app_server.request(
        "fuzzyFileSearch/sessionUpdate",
        FuzzyFileSearchSessionUpdateParams {
            session_id: session_id.to_owned(),
            query: query.to_owned(),
        },
    )
}

fn stop_fuzzy_file_search(
    app_server: &AppServerConnection,
    runtime: &mut FuzzyFileSearchRuntime,
    session_id: &str,
) {
    if runtime.session_id.as_deref() != Some(session_id) {
        return;
    }
    if runtime.support != FuzzyFileSearchSupport::Unsupported {
        let result: Result<Value, _> = app_server.request(
            "fuzzyFileSearch/sessionStop",
            FuzzyFileSearchSessionStopParams {
                session_id: session_id.to_owned(),
            },
        );
        if result.as_ref().is_err_and(is_method_unsupported) {
            runtime.support = FuzzyFileSearchSupport::Unsupported;
        }
    }
    runtime.clear_session();
}

fn run_legacy_fuzzy_file_search(
    app_server: &AppServerConnection,
    events: &Sender<Action>,
    runtime: &mut FuzzyFileSearchRuntime,
    session_id: String,
    roots: Vec<PathBuf>,
    query: String,
) {
    runtime.session_id = Some(session_id.clone());
    runtime.roots.clone_from(&roots);
    match app_server.request::<_, FuzzyFileSearchResponse>(
        "fuzzyFileSearch",
        FuzzyFileSearchParams {
            query: query.clone(),
            roots: roots.clone(),
            cancellation_token: Some(FUZZY_FILE_SEARCH_CANCELLATION_TOKEN.to_owned()),
        },
    ) {
        Ok(response) => {
            emit(
                events,
                Action::FuzzyFileSearchUpdated {
                    session_id: session_id.clone(),
                    query,
                    results: map_fuzzy_file_search_results(response.files, &roots),
                },
            );
            emit(events, Action::FuzzyFileSearchCompleted { session_id });
        }
        Err(error) => emit_fuzzy_file_search_error(events, session_id, query, error),
    }
}

fn emit_fuzzy_file_search_error(
    events: &Sender<Action>,
    session_id: String,
    query: String,
    error: AppServerError,
) {
    emit(
        events,
        Action::FuzzyFileSearchFailed {
            session_id,
            query,
            message: bounded(
                format!("Could not search workspace files: {error}"),
                MAX_STATUS_BYTES,
            ),
        },
    );
}

fn validate_fuzzy_file_search_request(
    session_id: &str,
    roots: &[PathBuf],
    query: &str,
) -> Result<(), &'static str> {
    if session_id.is_empty() || session_id.len() > 512 {
        return Err("The file-search session identifier is invalid.");
    }
    if query.is_empty() || query.len() > MAX_FUZZY_FILE_QUERY_BYTES {
        return Err("The file-search query is invalid.");
    }
    if roots.is_empty() || roots.len() > MAX_FUZZY_FILE_ROOTS {
        return Err("The file-search workspace roots are invalid.");
    }
    if roots
        .iter()
        .any(|root| !root.is_absolute() || root.as_os_str().len() > MAX_FUZZY_FILE_PATH_BYTES)
    {
        return Err("A file-search workspace root is invalid.");
    }
    Ok(())
}

fn is_method_unsupported(error: &AppServerError) -> bool {
    matches!(error, AppServerError::RequestFailed { code: -32601 })
}

fn should_restart_fuzzy_file_search_session(error: &AppServerError) -> bool {
    matches!(
        error,
        AppServerError::RequestFailed {
            code: -32602 | -32000
        }
    )
}

fn handle_fuzzy_file_search_event(
    event: &AppServerEvent,
    runtime: &FuzzyFileSearchRuntime,
    events: &Sender<Action>,
) -> bool {
    let AppServerEvent::Notification { method, params } = event else {
        return false;
    };
    match method.as_str() {
        "fuzzyFileSearch/sessionUpdated" => {
            let Ok(notification) =
                serde_json::from_value::<FuzzyFileSearchSessionUpdatedNotification>(params.clone())
            else {
                return true;
            };
            if runtime.session_id.as_deref() == Some(notification.session_id.as_str())
                && notification.query.len() <= MAX_FUZZY_FILE_QUERY_BYTES
            {
                emit(
                    events,
                    Action::FuzzyFileSearchUpdated {
                        session_id: notification.session_id,
                        query: notification.query,
                        results: map_fuzzy_file_search_results(notification.files, &runtime.roots),
                    },
                );
            }
            true
        }
        "fuzzyFileSearch/sessionCompleted" => {
            let Ok(notification) = serde_json::from_value::<
                FuzzyFileSearchSessionCompletedNotification,
            >(params.clone()) else {
                return true;
            };
            if runtime.session_id.as_deref() == Some(notification.session_id.as_str()) {
                emit(
                    events,
                    Action::FuzzyFileSearchCompleted {
                        session_id: notification.session_id,
                    },
                );
            }
            true
        }
        _ => false,
    }
}

fn map_fuzzy_file_search_results(
    files: Vec<ProtocolFuzzyFileResult>,
    roots: &[PathBuf],
) -> Vec<FuzzyFileResult> {
    let mut seen = HashSet::new();
    let mut results = Vec::with_capacity(files.len().min(MAX_FUZZY_FILE_RESULTS));
    for file in files
        .into_iter()
        .take(MAX_FUZZY_FILE_RESULTS.saturating_mul(4))
    {
        if results.len() == MAX_FUZZY_FILE_RESULTS {
            break;
        }
        if file.file_name.is_empty()
            || file.file_name.len() > MAX_ATTACHMENT_LABEL_BYTES
            || file.path.as_os_str().len() > MAX_FUZZY_FILE_PATH_BYTES
            || file.root.as_os_str().len() > MAX_FUZZY_FILE_PATH_BYTES
        {
            continue;
        }
        let Some(root) = roots
            .iter()
            .find(|root| fuzzy_file_paths_match(root, &file.root))
        else {
            continue;
        };
        if !valid_fuzzy_file_relative_path(&file.path) {
            continue;
        }
        let path = root.join(&file.path);
        if !seen.insert(path.clone()) {
            continue;
        }
        let detail = fuzzy_file_result_detail(root, &file.path, roots.len() > 1);
        results.push(FuzzyFileResult {
            name: file.file_name,
            path,
            detail,
            match_type: match file.match_type {
                FuzzyFileSearchMatchType::File => FuzzyFileMatchType::File,
                FuzzyFileSearchMatchType::Directory => FuzzyFileMatchType::Directory,
            },
        });
    }
    results
}

fn valid_fuzzy_file_relative_path(path: &Path) -> bool {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return false;
    }
    path.components().all(|component| match component {
        Component::Normal(value) => !FUZZY_FILE_SEARCH_EXCLUDED_COMPONENTS
            .iter()
            .any(|excluded| fuzzy_file_component_matches(value, excluded)),
        Component::CurDir => true,
        Component::ParentDir | Component::RootDir | Component::Prefix(_) => false,
    })
}

#[cfg(windows)]
fn fuzzy_file_component_matches(value: &std::ffi::OsStr, expected: &str) -> bool {
    value.to_string_lossy().eq_ignore_ascii_case(expected)
}

#[cfg(not(windows))]
fn fuzzy_file_component_matches(value: &std::ffi::OsStr, expected: &str) -> bool {
    value == expected
}

#[cfg(windows)]
fn fuzzy_file_paths_match(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

#[cfg(not(windows))]
fn fuzzy_file_paths_match(left: &Path, right: &Path) -> bool {
    left == right
}

fn fuzzy_file_result_detail(root: &Path, relative: &Path, include_root: bool) -> String {
    let mut parts = Vec::new();
    if include_root {
        parts.push(
            root.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| root.display().to_string()),
        );
    }
    if let Some(parent) = relative
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        parts.push(
            parent
                .components()
                .filter_map(|component| match component {
                    Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("/"),
        );
    }
    bounded(parts.join(" · "), MAX_ATTACHMENT_LABEL_BYTES)
}

fn browser_origin_auto_decision(
    request: &McpBrowserOriginElicitation,
    permissions: &BrowserPermissionsState,
) -> Option<BrowserOriginElicitationDecision> {
    match browser_permission_for_url(
        permissions,
        &request.origin,
        BrowserPermissionResource::Browse,
    ) {
        BrowserPermissionValue::Block => Some(BrowserOriginElicitationDecision::Deny),
        BrowserPermissionValue::Allow => Some(BrowserOriginElicitationDecision::AllowSite),
        BrowserPermissionValue::Default
            if permissions.approval_mode == BrowserApprovalMode::NeverAsk =>
        {
            Some(BrowserOriginElicitationDecision::AllowOnce)
        }
        BrowserPermissionValue::Default => None,
    }
}

fn browser_resource_auto_decision(
    request: &McpBrowserResourceElicitation,
    permissions: &BrowserPermissionsState,
) -> Option<BrowserResourceElicitationDecision> {
    if browser_permission_for_url(
        permissions,
        &request.origin,
        BrowserPermissionResource::Browse,
    ) == BrowserPermissionValue::Block
        || (request.resource == BrowserPermissionResource::FullCdp
            && !permissions.full_cdp_access_enabled)
    {
        return Some(BrowserResourceElicitationDecision::Deny);
    }
    match browser_permission_for_url(permissions, &request.origin, request.resource) {
        BrowserPermissionValue::Block => Some(BrowserResourceElicitationDecision::Deny),
        BrowserPermissionValue::Allow => Some(BrowserResourceElicitationDecision::AlwaysAllow),
        BrowserPermissionValue::Default => match request.resource {
            BrowserPermissionResource::Download
                if permissions.download_approval_mode == BrowserApprovalMode::NeverAsk =>
            {
                Some(BrowserResourceElicitationDecision::AllowOnce)
            }
            BrowserPermissionResource::Upload
                if permissions.upload_approval_mode == BrowserApprovalMode::NeverAsk =>
            {
                Some(BrowserResourceElicitationDecision::AllowOnce)
            }
            BrowserPermissionResource::Browse
            | BrowserPermissionResource::Download
            | BrowserPermissionResource::Upload
            | BrowserPermissionResource::FullCdp => None,
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_app_server_event(
    app_server: &AppServerConnection,
    event: AppServerEvent,
    events: &Sender<Action>,
    pending_approvals: &mut HashMap<String, PendingApproval>,
    computer_permissions: &mut HashMap<String, ComputerUsePermission>,
    computer_allowed_app_ids: &mut HashSet<String>,
    computer_accessibility: &mut ComputerUseAccessibilityClient,
    computer_url_policy: &mut ComputerUseUrlPolicy,
) -> bool {
    handle_app_server_event_with_browser_permissions(
        app_server,
        event,
        events,
        pending_approvals,
        computer_permissions,
        computer_allowed_app_ids,
        computer_accessibility,
        computer_url_policy,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn handle_app_server_event_with_browser_permissions(
    app_server: &AppServerConnection,
    event: AppServerEvent,
    events: &Sender<Action>,
    pending_approvals: &mut HashMap<String, PendingApproval>,
    computer_permissions: &mut HashMap<String, ComputerUsePermission>,
    computer_allowed_app_ids: &mut HashSet<String>,
    computer_accessibility: &mut ComputerUseAccessibilityClient,
    computer_url_policy: &mut ComputerUseUrlPolicy,
    computer_overlay: Option<&mut ComputerUseSystemOverlay>,
    browser_permissions: Option<&BrowserPermissionsState>,
) -> bool {
    match event {
        AppServerEvent::Notification { method, params } => {
            handle_notification(&method, params, events)
        }
        AppServerEvent::Request { id, method, params } => {
            match method.as_str() {
                "item/commandExecution/requestApproval"
                | "item/fileChange/requestApproval"
                | "item/permissions/requestApproval" => {
                    if pending_approvals.len() >= MAX_PENDING_APPROVALS {
                        let _ = app_server.respond_error(
                            &id,
                            -32000,
                            "the approval queue is full; retry after resolving another request",
                        );
                        emit(
                            events,
                            Action::SetStatus(
                                "Approval request rejected because the bounded queue is full."
                                    .to_owned(),
                            ),
                        );
                        return false;
                    }
                    let request_id = request_key(&id);
                    if pending_approvals.contains_key(&request_id) {
                        let _ = app_server.respond_error(&id, -32600, "duplicate approval request");
                        return false;
                    }
                    match map_app_server_approval(request_id.clone(), &method, params, id.clone()) {
                        Ok((request, pending)) => {
                            let auto_deny = matches!(
                                &request.context,
                                ApprovalContext::Permissions(context) if context.details.is_empty()
                            );
                            if auto_deny {
                                if let Err(error) = app_server.respond_success(
                                    &id,
                                    &PermissionsRequestApprovalResponse {
                                        permissions: PermissionProfile::default(),
                                        scope: PermissionGrantScope::Turn,
                                        strict_auto_review: None,
                                    },
                                ) {
                                    emit(
                                        events,
                                        Action::SetStatus(format!(
                                            "failed to auto-deny an empty permission request: {error}"
                                        )),
                                    );
                                }
                            } else {
                                pending_approvals.insert(request_id, pending);
                                emit(events, Action::ApprovalRequested(request));
                            }
                        }
                        Err(message) => {
                            let _ =
                                app_server.respond_error(&id, -32602, "invalid approval request");
                            emit(
                                events,
                                Action::SetStatus(format!(
                                    "Invalid approval request from app-server: {message}"
                                )),
                            );
                        }
                    }
                }
                "item/tool/requestUserInput" => {
                    if pending_approvals.len() >= MAX_PENDING_APPROVALS {
                        let _ = app_server.respond_error(
                            &id,
                            -32000,
                            "the client request queue is full; retry after resolving another request",
                        );
                        emit(
                            events,
                            Action::SetStatus(
                                "Structured input request rejected because the bounded queue is full."
                                    .to_owned(),
                            ),
                        );
                        return false;
                    }
                    let request_id = request_key(&id);
                    if pending_approvals.contains_key(&request_id) {
                        let _ = app_server.respond_error(
                            &id,
                            -32600,
                            "duplicate structured input request",
                        );
                        return false;
                    }
                    match map_user_input_request(request_id.clone(), params) {
                        Ok(request) => {
                            pending_approvals.insert(request_id, PendingApproval::UserInput { id });
                            emit(events, Action::UserInputRequested(request));
                        }
                        Err(()) => {
                            let _ = app_server.respond_error(
                                &id,
                                -32602,
                                "invalid structured input request",
                            );
                        }
                    }
                }
                "mcpServer/elicitation/request" => {
                    if pending_approvals.len() >= MAX_PENDING_APPROVALS {
                        let _ = app_server.respond_error(
                            &id,
                            -32000,
                            "the client request queue is full; retry after resolving another request",
                        );
                        emit(
                            events,
                            Action::SetStatus(
                                "MCP action request rejected because the bounded queue is full."
                                    .to_owned(),
                            ),
                        );
                        return false;
                    }
                    let request_id = request_key(&id);
                    if pending_approvals.contains_key(&request_id) {
                        let _ = app_server.respond_error(
                            &id,
                            -32600,
                            "duplicate MCP elicitation request",
                        );
                        return false;
                    }
                    match map_mcp_elicitation(request_id.clone(), params) {
                        Ok(request) => {
                            if let Some(permissions) = browser_permissions {
                                match &request {
                                    McpElicitation::BrowserOrigin(browser_request) => {
                                        if let Some(decision) = browser_origin_auto_decision(
                                            browser_request,
                                            permissions,
                                        ) {
                                            send_browser_origin_elicitation_response(
                                                app_server, &id, decision, events,
                                            );
                                            return false;
                                        }
                                    }
                                    McpElicitation::BrowserResource(browser_request) => {
                                        if let Some(decision) = browser_resource_auto_decision(
                                            browser_request,
                                            permissions,
                                        ) {
                                            send_browser_resource_elicitation_response(
                                                app_server, &id, decision, events,
                                            );
                                            return false;
                                        }
                                    }
                                    McpElicitation::Url(_) | McpElicitation::Form(_) => {}
                                }
                            }
                            pending_approvals
                                .insert(request_id, PendingApproval::McpElicitation { id });
                            emit(events, Action::McpElicitationRequested(request));
                        }
                        Err(McpElicitationMapError::Invalid) => {
                            let _ = app_server.respond_error(
                                &id,
                                -32602,
                                "invalid MCP elicitation request",
                            );
                        }
                    }
                }
                "currentTime/read" => {
                    let current_time_at = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_or(0, |duration| duration.as_secs());
                    let _ = app_server
                        .respond_success(&id, &json!({ "currentTimeAt": current_time_at }));
                }
                "item/tool/call" => {
                    handle_dynamic_tool_call(
                        app_server,
                        &id,
                        params,
                        events,
                        pending_approvals,
                        computer_permissions,
                        computer_allowed_app_ids,
                        computer_accessibility,
                        computer_url_policy,
                        computer_overlay,
                    );
                }
                _ => {
                    let _ = app_server.respond_error(&id, -32601, "unsupported client request");
                }
            }
            false
        }
        AppServerEvent::NotificationsDropped { count } => {
            emit(
                events,
                Action::SetStatus(format!("{count} app-server notifications were coalesced")),
            );
            false
        }
        AppServerEvent::Disconnected => {
            emit(events, Action::ConnectionLost);
            false
        }
    }
}

fn handle_notification(method: &str, params: Value, events: &Sender<Action>) -> bool {
    if method == "fs/changed" {
        return true;
    }
    match method {
        "remoteControl/status/changed" => {
            if let Ok(notification) =
                serde_json::from_value::<RemoteControlStatusChangedNotification>(params)
            {
                match map_remote_control_snapshot(notification.status, notification.environment_id)
                {
                    Ok((status, environment_id)) => emit(
                        events,
                        Action::RemoteControlStatusChanged {
                            status,
                            environment_id,
                        },
                    ),
                    Err(()) => emit(events, Action::RefreshRemoteControlStatus),
                }
            } else {
                emit(events, Action::RefreshRemoteControlStatus);
            }
        }
        "turn/started" => {
            if let (Some(task_id), Some(turn_id)) = (
                string_field(&params, "threadId"),
                params.get("turn").and_then(|turn| string_field(turn, "id")),
            ) {
                emit(events, Action::TurnStarted { task_id, turn_id });
            }
        }
        "turn/completed" => {
            if let (Some(task_id), Some(turn)) =
                (string_field(&params, "threadId"), params.get("turn"))
                && let Some(turn_id) = string_field(turn, "id")
            {
                if string_field(turn, "status").as_deref() == Some("interrupted") {
                    emit(events, Action::TurnInterrupted { task_id, turn_id });
                } else {
                    let failed = string_field(turn, "status").as_deref() == Some("failed");
                    emit(
                        events,
                        Action::TurnCompleted {
                            task_id,
                            turn_id,
                            failed,
                        },
                    );
                }
            }
        }
        "turn/diff/updated" => {
            if let Ok(notification) = serde_json::from_value::<TurnDiffUpdatedNotification>(params)
            {
                let truncated = notification.diff.len() > MAX_TURN_DIFF_BYTES;
                emit(
                    events,
                    Action::TurnDiffUpdated {
                        task_id: notification.thread_id,
                        turn_id: notification.turn_id,
                        diff: bounded(notification.diff, MAX_TURN_DIFF_BYTES),
                        truncated,
                    },
                );
            }
        }
        "item/started" | "item/completed" => {
            if let (Some(task_id), Some(turn_id), Some(item)) = (
                string_field(&params, "threadId"),
                string_field(&params, "turnId"),
                params.get("item").cloned(),
            ) && !is_hidden_timeline_item(&item)
            {
                emit(
                    events,
                    Action::UpsertTimelineItem {
                        task_id,
                        item: map_timeline_item(turn_id, item, method == "item/completed"),
                    },
                );
            }
        }
        "item/agentMessage/delta"
        | "item/plan/delta"
        | "item/reasoning/summaryTextDelta"
        | "item/reasoning/textDelta"
        | "item/commandExecution/outputDelta"
        | "item/fileChange/outputDelta" => {
            if let (Some(task_id), Some(turn_id), Some(item_id), Some(delta)) = (
                string_field(&params, "threadId"),
                string_field(&params, "turnId"),
                string_field(&params, "itemId"),
                string_field(&params, "delta"),
            ) {
                emit(
                    events,
                    Action::TimelineDelta {
                        task_id,
                        turn_id,
                        item_id,
                        kind: notification_kind(method),
                        delta,
                    },
                );
            }
        }
        "thread/goal/updated" => {
            if let Ok(notification) =
                serde_json::from_value::<ThreadGoalUpdatedNotification>(params)
            {
                emit(
                    events,
                    Action::GoalUpdated(map_thread_goal(notification.goal)),
                );
            }
        }
        "thread/goal/cleared" => {
            if let Ok(notification) =
                serde_json::from_value::<ThreadGoalClearedNotification>(params)
            {
                emit(
                    events,
                    Action::GoalCleared {
                        task_id: notification.thread_id,
                    },
                );
            }
        }
        "thread/status/changed" => {
            if let (Some(task_id), Some(status)) = (
                string_field(&params, "threadId"),
                params
                    .get("status")
                    .and_then(|status| string_field(status, "type")),
            ) && status == "idle"
            {
                emit(events, Action::MaybeContinueGoal { task_id });
            }
            emit(events, Action::RefreshTasks);
        }
        "thread/tokenUsage/updated" => {
            if let Ok(notification) =
                serde_json::from_value::<ThreadTokenUsageUpdatedNotification>(params)
            {
                emit(
                    events,
                    Action::ThreadTokenUsageUpdated {
                        task_id: notification.thread_id,
                        last_total_tokens: notification.token_usage.last.total_tokens,
                        model_context_window: notification.token_usage.model_context_window,
                    },
                );
            }
        }
        "model/verification" => {
            if let Ok(notification) =
                serde_json::from_value::<ModelVerificationNotification>(params)
                && notification
                    .verifications
                    .contains(&ModelVerification::TrustedAccessForCyber)
            {
                let turn_id = notification.turn_id;
                emit(
                    events,
                    Action::UpsertTimelineItem {
                        task_id: notification.thread_id,
                        item: TimelineItem {
                            id: format!("model-verification:{turn_id}:trusted-access-for-cyber"),
                            turn_id,
                            kind: TimelineKind::Warning,
                            text: TRUSTED_ACCESS_FOR_CYBER_WARNING.to_owned(),
                            detail: None,
                            process_id: None,
                            memory_citations: Vec::new(),
                            sources: vec![TimelineSource {
                                title: "Trusted Access for Cyber".to_owned(),
                                url: TRUSTED_ACCESS_FOR_CYBER_URL.to_owned(),
                            }],
                            attachments: Vec::new(),
                            output_artifacts: Vec::new(),
                            edit_supported: false,
                            completed: true,
                        },
                    },
                );
            }
        }
        "model/safetyBuffering/updated" => {
            if let Ok(notification) =
                serde_json::from_value::<ModelSafetyBufferingUpdatedNotification>(params)
            {
                emit(
                    events,
                    Action::SafetyBufferingUpdated {
                        task_id: notification.thread_id,
                        turn_id: notification.turn_id,
                        show_buffering_ui: notification.show_buffering_ui,
                        faster_model: notification.faster_model.and_then(|model| {
                            let model =
                                bounded(model.trim().to_owned(), MAX_ATTACHMENT_LABEL_BYTES);
                            (!model.is_empty()).then_some(model)
                        }),
                    },
                );
            }
        }
        "thread/started"
        | "thread/name/updated"
        | "thread/archived"
        | "thread/unarchived"
        | "thread/deleted" => emit(events, Action::RefreshTasks),
        "account/updated" | "account/rateLimits/updated" => {
            emit(events, Action::RefreshAccount);
        }
        "account/login/completed" => {
            if let Ok(notification) =
                serde_json::from_value::<AccountLoginCompletedNotification>(params)
            {
                emit(
                    events,
                    Action::AccountLoginCompleted {
                        login_id: notification.login_id.map(|login_id| bounded(login_id, 512)),
                        success: notification.success,
                    },
                );
            }
        }
        "skills/changed" => emit(events, Action::SkillsInvalidated),
        "app/list/updated" => {
            if let Ok(notification) = serde_json::from_value::<AppListUpdatedNotification>(params) {
                emit(events, Action::AppsLoaded(map_apps(notification.data)));
            }
        }
        "mcpServer/oauthLogin/completed" => {
            if let Ok(notification) =
                serde_json::from_value::<McpServerOauthLoginCompletedNotification>(params)
            {
                emit(
                    events,
                    Action::McpServerAuthenticationCompleted {
                        name: notification.name,
                        success: notification.success,
                        error: notification
                            .error
                            .map(|_| "MCP server authentication failed. Try again.".to_owned()),
                    },
                );
            }
        }
        "mcpServer/startupStatus/updated" => {
            if let Ok(notification) =
                serde_json::from_value::<McpServerStatusUpdatedNotification>(params)
            {
                emit(
                    events,
                    Action::McpServerStartupStatusUpdated {
                        name: notification.name,
                        status: map_mcp_startup_state(notification.status),
                        error: notification.error.map(|_| {
                            "MCP server could not start. Check its configuration and try again."
                                .to_owned()
                        }),
                        failure_reason: notification
                            .failure_reason
                            .map(map_mcp_startup_failure_reason),
                    },
                );
            }
        }
        "externalAgentConfig/import/progress" => {
            if let Ok(notification) =
                serde_json::from_value::<ExternalAgentConfigImportProgressNotification>(params)
            {
                emit(
                    events,
                    Action::ExternalImportProgress {
                        import_id: bounded(notification.import_id, MAX_IMPORT_FIELD_BYTES),
                        results: map_import_type_results(notification.item_type_results),
                    },
                );
            }
        }
        "externalAgentConfig/import/completed" => {
            if let Ok(notification) =
                serde_json::from_value::<ExternalAgentConfigImportCompletedNotification>(params)
            {
                emit(
                    events,
                    Action::ExternalImportCompleted {
                        import_id: bounded(notification.import_id, MAX_IMPORT_FIELD_BYTES),
                        results: map_import_type_results(notification.item_type_results),
                    },
                );
            }
        }
        "warning" | "guardianWarning" | "deprecationNotice" | "configWarning" => {
            if let Some(message) = string_field(&params, "message") {
                emit(
                    events,
                    Action::SetStatus(bounded(message, MAX_STATUS_BYTES)),
                );
            }
        }
        "error" => {
            let message = if params
                .get("willRetry")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                "Codex hit an error and is retrying.".to_owned()
            } else {
                "Codex couldn't complete the request. Try again, or check the account and connection settings."
                    .to_owned()
            };
            emit(
                events,
                Action::SetStatus(bounded(message, MAX_STATUS_BYTES)),
            );
        }
        _ => {}
    }
    false
}

fn refresh_git(generation: u64, cwd: &std::path::Path, events: &Sender<Action>) {
    match git_snapshot(cwd) {
        Ok(snapshot) => emit(
            events,
            Action::GitSnapshotLoaded {
                generation,
                git: Box::new(map_git_snapshot(snapshot)),
                error: None,
            },
        ),
        Err(GitError::InvalidRepository) => {
            emit(
                events,
                Action::GitSnapshotLoaded {
                    generation,
                    git: Box::default(),
                    error: None,
                },
            );
        }
        Err(error) => {
            emit(
                events,
                Action::GitSnapshotLoaded {
                    generation,
                    git: Box::default(),
                    error: Some(bounded(
                        format!("failed to inspect Git repository: {error}"),
                        MAX_STATUS_BYTES,
                    )),
                },
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn respond_to_approval(
    app_server: &AppServerConnection,
    request_id: String,
    decision: ApprovalDecision,
    events: &Sender<Action>,
    pending_approvals: &mut HashMap<String, PendingApproval>,
    computer_permissions: &mut HashMap<String, ComputerUsePermission>,
    computer_allowed_app_ids: &mut HashSet<String>,
    computer_accessibility: &mut ComputerUseAccessibilityClient,
    computer_url_policy: &mut ComputerUseUrlPolicy,
    mut computer_overlay: Option<&mut ComputerUseSystemOverlay>,
) {
    let Some(pending) = pending_approvals.remove(&request_id) else {
        return;
    };
    match pending {
        PendingApproval::Command {
            id,
            proposed_execpolicy_amendment,
            proposed_network_policy_amendment,
        } => {
            let response = match decision {
                ApprovalDecision::Accept => Some(CommandExecutionApprovalDecision::Value(
                    CommandExecutionApprovalDecisionValue::Accept,
                )),
                ApprovalDecision::Decline => Some(CommandExecutionApprovalDecision::Value(
                    CommandExecutionApprovalDecisionValue::Decline,
                )),
                ApprovalDecision::AcceptForSession => {
                    Some(CommandExecutionApprovalDecision::Value(
                        CommandExecutionApprovalDecisionValue::AcceptForSession,
                    ))
                }
                ApprovalDecision::AcceptWithExecpolicyAmendment(amendment)
                    if proposed_execpolicy_amendment.as_ref() == Some(&amendment) =>
                {
                    Some(
                        CommandExecutionApprovalDecision::AcceptWithExecpolicyAmendment {
                            accept_with_execpolicy_amendment: ExecpolicyAmendment {
                                execpolicy_amendment: amendment,
                            },
                        },
                    )
                }
                ApprovalDecision::ApplyNetworkPolicyAmendment(amendment)
                    if proposed_network_policy_amendment.as_ref() == Some(&amendment) =>
                {
                    Some(
                        CommandExecutionApprovalDecision::ApplyNetworkPolicyAmendment {
                            apply_network_policy_amendment: NetworkPolicyAmendmentDecision {
                                network_policy_amendment: protocol_network_policy_amendment(
                                    amendment,
                                ),
                            },
                        },
                    )
                }
                ApprovalDecision::AlwaysAllow
                | ApprovalDecision::AcceptWithExecpolicyAmendment(_)
                | ApprovalDecision::ApplyNetworkPolicyAmendment(_) => None,
            };
            let response = response.map_or_else(
                || {
                    app_server.respond_error(
                        &id,
                        -32602,
                        "approval decision did not match the command request",
                    )
                },
                |decision| {
                    app_server
                        .respond_success(&id, &CommandExecutionRequestApprovalResponse { decision })
                },
            );
            if let Err(error) = response {
                emit(
                    events,
                    Action::SetStatus(format!("failed to answer approval: {error}")),
                );
            }
        }
        PendingApproval::FileChange { id } => {
            let response = match decision {
                ApprovalDecision::Accept => Some(FileChangeApprovalDecision::Accept),
                ApprovalDecision::Decline => Some(FileChangeApprovalDecision::Decline),
                ApprovalDecision::AcceptForSession => {
                    Some(FileChangeApprovalDecision::AcceptForSession)
                }
                ApprovalDecision::AlwaysAllow
                | ApprovalDecision::AcceptWithExecpolicyAmendment(_)
                | ApprovalDecision::ApplyNetworkPolicyAmendment(_) => None,
            };
            let response = response.map_or_else(
                || {
                    app_server.respond_error(
                        &id,
                        -32602,
                        "approval decision did not match the file-change request",
                    )
                },
                |decision| {
                    app_server.respond_success(&id, &FileChangeRequestApprovalResponse { decision })
                },
            );
            if let Err(error) = response {
                emit(
                    events,
                    Action::SetStatus(format!("failed to answer approval: {error}")),
                );
            }
        }
        PendingApproval::Permissions { id, permissions } => {
            let (permissions, scope) = match decision {
                ApprovalDecision::Accept => (permissions, PermissionGrantScope::Turn),
                ApprovalDecision::AcceptForSession => (permissions, PermissionGrantScope::Session),
                ApprovalDecision::Decline => {
                    (PermissionProfile::default(), PermissionGrantScope::Turn)
                }
                ApprovalDecision::AlwaysAllow
                | ApprovalDecision::AcceptWithExecpolicyAmendment(_)
                | ApprovalDecision::ApplyNetworkPolicyAmendment(_) => {
                    let response = app_server.respond_error(
                        &id,
                        -32602,
                        "approval decision did not match the permission request",
                    );
                    if let Err(error) = response {
                        emit(
                            events,
                            Action::SetStatus(format!("failed to answer approval: {error}")),
                        );
                    }
                    return;
                }
            };
            if let Err(error) = app_server.respond_success(
                &id,
                &PermissionsRequestApprovalResponse {
                    permissions,
                    scope,
                    strict_auto_review: None,
                },
            ) {
                emit(
                    events,
                    Action::SetStatus(format!("failed to answer approval: {error}")),
                );
            }
        }
        PendingApproval::McpElicitation { id } => {
            pending_approvals.insert(request_id, PendingApproval::McpElicitation { id });
            emit(
                events,
                Action::SetStatus(
                    "The MCP action is waiting for its dedicated response controls.".to_owned(),
                ),
            );
        }
        PendingApproval::UserInput { id } => {
            pending_approvals.insert(request_id, PendingApproval::UserInput { id });
            emit(
                events,
                Action::SetStatus(
                    "The structured input request is waiting for its dedicated controls."
                        .to_owned(),
                ),
            );
        }
        PendingApproval::ComputerUse {
            id,
            params,
            window_id,
            application_id,
            application_name,
        } => {
            if decision == ApprovalDecision::Decline {
                respond_dynamic_tool_failure(
                    app_server,
                    &id,
                    &format!("Computer Use access to “{application_name}” was declined"),
                );
                return;
            }

            let permission_enabled = computer_permissions
                .get(&params.thread_id)
                .is_some_and(|permission| permission.enabled);
            if !permission_enabled {
                respond_dynamic_tool_failure(
                    app_server,
                    &id,
                    "Computer Use was disabled before approval",
                );
                return;
            }
            let inspected_window = match inspect_computer_window(&window_id) {
                Ok(window) => window,
                Err(_) => {
                    respond_dynamic_tool_failure(
                        app_server,
                        &id,
                        "the requested window is unavailable after approval",
                    );
                    return;
                }
            };
            let inspected_application_id =
                normalized_computer_app_id(&inspected_window.application_id);
            if inspected_application_id.as_deref() != Some(application_id.as_str()) {
                respond_dynamic_tool_failure(
                    app_server,
                    &id,
                    "the requested window changed owners before approval",
                );
                return;
            }
            if let Some(message) = forbidden_computer_target_message(
                &inspected_window.application_id,
                &inspected_window.application,
            ) {
                respond_dynamic_tool_failure(app_server, &id, &message);
                return;
            }
            if params.tool != "get_window_state"
                && computer_accessibility.user_input_requires_refresh(&window_id)
            {
                respond_dynamic_tool_failure(
                    app_server,
                    &id,
                    COMPUTER_USE_USER_INPUT_STALE_MESSAGE,
                );
                return;
            }
            if let Err(message) = computer_url_policy.enforce_and_block(
                app_server,
                computer_accessibility,
                &params,
                &inspected_window,
            ) {
                if let Some(overlay) = computer_overlay.as_deref_mut() {
                    let _ = overlay.complete_turn(&params.thread_id, &params.turn_id);
                }
                respond_dynamic_tool_failure(app_server, &id, message);
                return;
            }

            #[cfg(windows)]
            let always_allowed = if decision == ApprovalDecision::AlwaysAllow {
                persist_computer_use_allowed_app(
                    app_server,
                    &application_id,
                    events,
                    computer_allowed_app_ids,
                )
            } else {
                computer_use_policy_contains(computer_allowed_app_ids, &application_id)
            };
            #[cfg(not(windows))]
            let always_allowed = false;

            if let Some(permission) = computer_permissions.get_mut(&params.thread_id) {
                permission.authorized_application_id = Some(application_id.clone());
                permission.input_authorized = true;
            }
            emit(
                events,
                Action::ComputerUseAppAuthorized {
                    task_id: params.thread_id.clone(),
                    application_id,
                    always_allowed,
                },
            );
            complete_computer_tool_call(
                app_server,
                &id,
                &params,
                &inspected_window,
                events,
                computer_accessibility,
                computer_overlay,
            );
        }
        PendingApproval::ComputerUseLaunch {
            id,
            params,
            application_id,
            application_name,
        } => {
            if decision == ApprovalDecision::Decline {
                respond_dynamic_tool_failure(
                    app_server,
                    &id,
                    &format!("Computer Use launch of “{application_name}” was declined"),
                );
                return;
            }

            let permission_enabled = computer_permissions
                .get(&params.thread_id)
                .is_some_and(|permission| permission.enabled);
            if !permission_enabled {
                respond_dynamic_tool_failure(
                    app_server,
                    &id,
                    "Computer Use was disabled before launch approval",
                );
                return;
            }
            if let Some(message) =
                forbidden_computer_target_message(&application_id, &application_name)
            {
                respond_dynamic_tool_failure(app_server, &id, &message);
                return;
            }
            if let Err(error) = computer_accessibility.validate_app_launch(&application_id) {
                respond_dynamic_tool_failure(app_server, &id, &error.to_string());
                return;
            }

            #[cfg(windows)]
            if decision == ApprovalDecision::AlwaysAllow {
                persist_computer_use_allowed_app(
                    app_server,
                    &application_id,
                    events,
                    computer_allowed_app_ids,
                );
            }

            if let Err(message) = begin_computer_overlay(computer_overlay, &params, None, events) {
                respond_dynamic_tool_failure(app_server, &id, &message);
                return;
            }
            match computer_accessibility.launch_app(&application_id) {
                Ok(()) => {
                    let _ = app_server.respond_success(
                        &id,
                        &DynamicToolCallResponse {
                            content_items: vec![text_content(format!(
                                "Launched “{application_name}”."
                            ))],
                            success: true,
                        },
                    );
                }
                Err(error) => {
                    respond_dynamic_tool_failure(app_server, &id, &error.to_string());
                }
            }
        }
    }
}

fn respond_to_user_input(
    app_server: &AppServerConnection,
    request_id: String,
    answers: UserInputAnswers,
    events: &Sender<Action>,
    pending_approvals: &mut HashMap<String, PendingApproval>,
) {
    let response = match user_input_response(answers) {
        Ok(response) => response,
        Err(error) => {
            emit(events, Action::SetStatus(error));
            return;
        }
    };
    let Some(pending) = pending_approvals.remove(&request_id) else {
        return;
    };
    let PendingApproval::UserInput { id } = pending else {
        pending_approvals.insert(request_id, pending);
        emit(
            events,
            Action::SetStatus(
                "The selected client request is not a structured input request.".to_owned(),
            ),
        );
        return;
    };
    if let Err(error) = app_server.respond_success(&id, &response) {
        emit(
            events,
            Action::SetStatus(format!(
                "failed to answer structured input request: {error}"
            )),
        );
    }
}

fn user_input_response(answers: UserInputAnswers) -> Result<ToolRequestUserInputResponse, String> {
    if answers.answers.len() > MAX_USER_INPUT_QUESTIONS {
        return Err("Structured input response exceeds the question limit.".to_owned());
    }
    let mut response = BTreeMap::new();
    for answer in answers.answers {
        if answer.question_id.is_empty()
            || answer.question_id.len() > MAX_MCP_SERVER_FIELD_BYTES
            || answer.answers.len() != 1
            || answer
                .answers
                .iter()
                .any(|value| value.is_empty() || value.len() > MAX_USER_INPUT_VALUE_BYTES)
            || response.contains_key(&answer.question_id)
        {
            return Err("Structured input response contains an invalid answer.".to_owned());
        }
        response.insert(
            answer.question_id,
            ToolRequestUserInputAnswer {
                answers: answer.answers,
            },
        );
    }
    Ok(ToolRequestUserInputResponse { answers: response })
}

fn respond_to_mcp_elicitation(
    app_server: &AppServerConnection,
    request_id: String,
    decision: McpElicitationDecision,
    content: Option<McpElicitationContent>,
    events: &Sender<Action>,
    pending_approvals: &mut HashMap<String, PendingApproval>,
) {
    let content = if decision == McpElicitationDecision::Accept {
        match content
            .as_ref()
            .map(mcp_elicitation_content_json)
            .transpose()
        {
            Ok(content) => content,
            Err(error) => {
                emit(events, Action::SetStatus(error));
                return;
            }
        }
    } else {
        None
    };
    let Some(pending) = pending_approvals.remove(&request_id) else {
        return;
    };
    let PendingApproval::McpElicitation { id } = pending else {
        pending_approvals.insert(request_id, pending);
        emit(
            events,
            Action::SetStatus(
                "The selected client request is not an MCP action request.".to_owned(),
            ),
        );
        return;
    };
    let action = match decision {
        McpElicitationDecision::Accept => ProtocolMcpServerElicitationAction::Accept,
        McpElicitationDecision::Decline => ProtocolMcpServerElicitationAction::Decline,
        McpElicitationDecision::Cancel => ProtocolMcpServerElicitationAction::Cancel,
    };
    if let Err(error) = app_server.respond_success(
        &id,
        &McpServerElicitationRequestResponse {
            action,
            content,
            metadata: None,
        },
    ) {
        emit(
            events,
            Action::SetStatus(format!("failed to answer MCP action request: {error}")),
        );
    }
}

fn respond_to_browser_origin_elicitation(
    app_server: &AppServerConnection,
    request_id: String,
    decision: BrowserOriginElicitationDecision,
    events: &Sender<Action>,
    pending_approvals: &mut HashMap<String, PendingApproval>,
) {
    let Some(pending) = pending_approvals.remove(&request_id) else {
        return;
    };
    let PendingApproval::McpElicitation { id } = pending else {
        pending_approvals.insert(request_id, pending);
        emit(
            events,
            Action::SetStatus(
                "The selected client request is not a Browser website request.".to_owned(),
            ),
        );
        return;
    };
    send_browser_origin_elicitation_response(app_server, &id, decision, events);
}

fn send_browser_origin_elicitation_response(
    app_server: &AppServerConnection,
    id: &Value,
    decision: BrowserOriginElicitationDecision,
    events: &Sender<Action>,
) {
    let response = browser_origin_elicitation_response(decision);
    if let Err(error) = app_server.respond_success(id, &response) {
        emit(
            events,
            Action::SetStatus(format!("failed to answer Browser website request: {error}")),
        );
    }
}

fn browser_origin_elicitation_response(
    decision: BrowserOriginElicitationDecision,
) -> McpServerElicitationRequestResponse {
    let accepted = !matches!(decision, BrowserOriginElicitationDecision::Deny);
    let action = if accepted {
        ProtocolMcpServerElicitationAction::Accept
    } else {
        ProtocolMcpServerElicitationAction::Decline
    };
    let metadata = matches!(decision, BrowserOriginElicitationDecision::AllowSite)
        .then(|| json!({ "persist": "always" }));
    McpServerElicitationRequestResponse {
        action,
        content: accepted.then(|| json!({})),
        metadata,
    }
}

fn respond_to_browser_resource_elicitation(
    app_server: &AppServerConnection,
    request_id: String,
    decision: BrowserResourceElicitationDecision,
    events: &Sender<Action>,
    pending_approvals: &mut HashMap<String, PendingApproval>,
) {
    let Some(pending) = pending_approvals.remove(&request_id) else {
        return;
    };
    let PendingApproval::McpElicitation { id } = pending else {
        pending_approvals.insert(request_id, pending);
        emit(
            events,
            Action::SetStatus(
                "The selected client request is not a Browser permission request.".to_owned(),
            ),
        );
        return;
    };
    send_browser_resource_elicitation_response(app_server, &id, decision, events);
}

fn send_browser_resource_elicitation_response(
    app_server: &AppServerConnection,
    id: &Value,
    decision: BrowserResourceElicitationDecision,
    events: &Sender<Action>,
) {
    let response = browser_resource_elicitation_response(decision);
    if let Err(error) = app_server.respond_success(id, &response) {
        emit(
            events,
            Action::SetStatus(format!(
                "failed to answer Browser permission request: {error}"
            )),
        );
    }
}

fn browser_resource_elicitation_response(
    decision: BrowserResourceElicitationDecision,
) -> McpServerElicitationRequestResponse {
    let accepted = decision != BrowserResourceElicitationDecision::Deny;
    let metadata = match decision {
        BrowserResourceElicitationDecision::AllowConversation => {
            Some(json!({ "persist": "session" }))
        }
        BrowserResourceElicitationDecision::AlwaysAllow => Some(json!({ "persist": "always" })),
        BrowserResourceElicitationDecision::AllowOnce
        | BrowserResourceElicitationDecision::Deny => None,
    };
    McpServerElicitationRequestResponse {
        action: if accepted {
            ProtocolMcpServerElicitationAction::Accept
        } else {
            ProtocolMcpServerElicitationAction::Decline
        },
        content: accepted.then(|| json!({})),
        metadata,
    }
}

fn mcp_elicitation_content_json(content: &McpElicitationContent) -> Result<Value, String> {
    if content.fields.len() > MAX_MCP_FORM_FIELDS {
        return Err("MCP form response exceeds the field limit.".to_owned());
    }
    let mut object = serde_json::Map::with_capacity(content.fields.len());
    for (name, value) in &content.fields {
        if name.is_empty() || name.len() > MAX_MCP_SERVER_FIELD_BYTES || object.contains_key(name) {
            return Err("MCP form response contains an invalid field name.".to_owned());
        }
        let value = match value {
            McpElicitationValue::String(value) => {
                if value.len() > MAX_MCP_FORM_VALUE_BYTES {
                    return Err("MCP form response contains an oversized value.".to_owned());
                }
                Value::String(value.clone())
            }
            McpElicitationValue::Number(value) => {
                let number = value.parse::<serde_json::Number>().map_err(|_| {
                    "MCP form response contains an invalid numeric value.".to_owned()
                })?;
                Value::Number(number)
            }
            McpElicitationValue::Boolean(value) => Value::Bool(*value),
            McpElicitationValue::Strings(values) => {
                if values.len() > MAX_MCP_FORM_OPTIONS
                    || values
                        .iter()
                        .any(|value| value.len() > MAX_MCP_FORM_VALUE_BYTES)
                {
                    return Err("MCP form response contains an oversized selection.".to_owned());
                }
                Value::Array(values.iter().cloned().map(Value::String).collect())
            }
        };
        object.insert(name.clone(), value);
    }
    Ok(Value::Object(object))
}

fn map_thread_goal(goal: codex_protocol::ThreadGoal) -> CoreThreadGoal {
    CoreThreadGoal {
        task_id: goal.thread_id,
        objective: goal.objective,
        status: match goal.status {
            ProtocolThreadGoalStatus::Active => CoreThreadGoalStatus::Active,
            ProtocolThreadGoalStatus::Paused => CoreThreadGoalStatus::Paused,
            ProtocolThreadGoalStatus::Blocked => CoreThreadGoalStatus::Blocked,
            ProtocolThreadGoalStatus::UsageLimited => CoreThreadGoalStatus::UsageLimited,
            ProtocolThreadGoalStatus::BudgetLimited => CoreThreadGoalStatus::BudgetLimited,
            ProtocolThreadGoalStatus::Complete => CoreThreadGoalStatus::Complete,
        },
        tokens_used: goal.tokens_used,
        token_budget: goal.token_budget,
        time_used_seconds: goal.time_used_seconds,
        created_at: goal.created_at,
        updated_at: goal.updated_at,
    }
}

const fn core_approvals_reviewer(reviewer: ProtocolApprovalsReviewer) -> CoreApprovalsReviewer {
    match reviewer {
        ProtocolApprovalsReviewer::User => CoreApprovalsReviewer::User,
        ProtocolApprovalsReviewer::AutoReview => CoreApprovalsReviewer::AutoReview,
    }
}

const fn map_skill_scope(scope: ProtocolSkillScope) -> CoreSkillScope {
    match scope {
        ProtocolSkillScope::User => CoreSkillScope::User,
        ProtocolSkillScope::Repo => CoreSkillScope::Repo,
        ProtocolSkillScope::System => CoreSkillScope::System,
        ProtocolSkillScope::Admin => CoreSkillScope::Admin,
    }
}

const fn map_hook_event_name(event: ProtocolHookEventName) -> CoreHookEventName {
    match event {
        ProtocolHookEventName::PreToolUse => CoreHookEventName::PreToolUse,
        ProtocolHookEventName::PermissionRequest => CoreHookEventName::PermissionRequest,
        ProtocolHookEventName::PostToolUse => CoreHookEventName::PostToolUse,
        ProtocolHookEventName::PreCompact => CoreHookEventName::PreCompact,
        ProtocolHookEventName::PostCompact => CoreHookEventName::PostCompact,
        ProtocolHookEventName::SessionStart => CoreHookEventName::SessionStart,
        ProtocolHookEventName::SessionEnd => CoreHookEventName::SessionEnd,
        ProtocolHookEventName::UserPromptSubmit => CoreHookEventName::UserPromptSubmit,
        ProtocolHookEventName::SubagentStart => CoreHookEventName::SubagentStart,
        ProtocolHookEventName::SubagentStop => CoreHookEventName::SubagentStop,
        ProtocolHookEventName::Stop => CoreHookEventName::Stop,
    }
}

const fn map_hook_handler_type(handler: ProtocolHookHandlerType) -> CoreHookHandlerType {
    match handler {
        ProtocolHookHandlerType::Command => CoreHookHandlerType::Command,
        ProtocolHookHandlerType::Prompt => CoreHookHandlerType::Prompt,
        ProtocolHookHandlerType::Agent => CoreHookHandlerType::Agent,
    }
}

const fn map_hook_source(source: ProtocolHookSource) -> CoreHookSource {
    match source {
        ProtocolHookSource::User => CoreHookSource::User,
        ProtocolHookSource::Project => CoreHookSource::Project,
        ProtocolHookSource::Plugin => CoreHookSource::Plugin,
        ProtocolHookSource::SessionFlags => CoreHookSource::SessionFlags,
        ProtocolHookSource::System
        | ProtocolHookSource::Mdm
        | ProtocolHookSource::CloudRequirements
        | ProtocolHookSource::CloudManagedConfig
        | ProtocolHookSource::LegacyManagedConfigFile
        | ProtocolHookSource::LegacyManagedConfigMdm => CoreHookSource::Admin,
        ProtocolHookSource::Unknown => CoreHookSource::Unknown,
    }
}

const fn map_hook_trust_status(status: ProtocolHookTrustStatus) -> CoreHookTrustStatus {
    match status {
        ProtocolHookTrustStatus::Managed => CoreHookTrustStatus::Managed,
        ProtocolHookTrustStatus::Untrusted => CoreHookTrustStatus::Untrusted,
        ProtocolHookTrustStatus::Trusted => CoreHookTrustStatus::Trusted,
        ProtocolHookTrustStatus::Modified => CoreHookTrustStatus::Modified,
    }
}

fn hook_state_config_value(key: &str, enabled: Option<bool>, trusted_hash: Option<&str>) -> Value {
    let mut fields = serde_json::Map::new();
    if let Some(enabled) = enabled {
        fields.insert("enabled".to_owned(), Value::Bool(enabled));
    }
    if let Some(trusted_hash) = trusted_hash {
        fields.insert(
            "trusted_hash".to_owned(),
            Value::String(trusted_hash.to_owned()),
        );
    }
    let mut state = serde_json::Map::new();
    state.insert(key.to_owned(), Value::Object(fields));
    Value::Object(state)
}

fn map_plugin_detail(plugin_id: String, detail: codex_protocol::PluginDetail) -> PluginDetailView {
    let website_url = bounded_http_url(
        detail
            .summary
            .presentation
            .as_ref()
            .and_then(|presentation| presentation.website_url.clone()),
    );
    let privacy_policy_url = bounded_http_url(
        detail
            .summary
            .presentation
            .as_ref()
            .and_then(|presentation| presentation.privacy_policy_url.clone()),
    );
    let terms_of_service_url = bounded_http_url(
        detail
            .summary
            .presentation
            .as_ref()
            .and_then(|presentation| presentation.terms_of_service_url.clone()),
    );
    let description = detail
        .description
        .or_else(|| {
            detail
                .summary
                .presentation
                .as_ref()
                .and_then(|presentation| {
                    presentation
                        .long_description
                        .clone()
                        .or_else(|| presentation.short_description.clone())
                })
        })
        .unwrap_or_default();
    let capabilities = detail
        .summary
        .presentation
        .as_ref()
        .map(|presentation| presentation.capabilities.clone())
        .unwrap_or_default()
        .into_iter()
        .map(|capability| bounded(capability, MAX_STATUS_BYTES))
        .collect();
    let skills = detail
        .skills
        .into_iter()
        .map(|skill| {
            let display_name = skill
                .presentation
                .as_ref()
                .and_then(|presentation| presentation.display_name.clone())
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| skill.name.clone());
            PluginSkillDetail {
                name: bounded(skill.name, MAX_STATUS_BYTES),
                display_name: bounded(display_name, MAX_STATUS_BYTES),
                description: bounded(
                    skill
                        .short_description
                        .filter(|description| !description.trim().is_empty())
                        .unwrap_or(skill.description),
                    MAX_STATUS_BYTES,
                ),
                enabled: skill.enabled,
            }
        })
        .collect();
    let apps = detail
        .apps
        .into_iter()
        .map(|app| PluginDetailItem {
            name: bounded(app.name, MAX_STATUS_BYTES),
            description: bounded(
                app.description
                    .or(app.category)
                    .unwrap_or_else(|| "Included app".to_owned()),
                MAX_STATUS_BYTES,
            ),
        })
        .collect();
    let app_templates = detail
        .app_templates
        .into_iter()
        .map(|template| PluginDetailItem {
            name: bounded(template.name, MAX_STATUS_BYTES),
            description: bounded(
                template
                    .description
                    .or(template.category)
                    .or(template.reason)
                    .unwrap_or_else(|| "Included app template".to_owned()),
                MAX_STATUS_BYTES,
            ),
        })
        .collect();
    let hooks = detail
        .hooks
        .into_iter()
        .map(|hook| PluginDetailItem {
            name: bounded(hook.key, MAX_STATUS_BYTES),
            description: bounded(hook.event_name, MAX_STATUS_BYTES),
        })
        .collect();
    let mcp_servers = detail
        .mcp_servers
        .into_iter()
        .map(|server| bounded(server, MAX_STATUS_BYTES))
        .collect();
    let scheduled_tasks = detail
        .scheduled_tasks
        .unwrap_or_default()
        .into_iter()
        .map(|task| PluginScheduledTaskCard {
            name: bounded(task.name, MAX_STATUS_BYTES),
            prompt: bounded(task.prompt, MAX_STATUS_BYTES),
            schedule: map_plugin_schedule(task.schedule),
        })
        .collect();

    PluginDetailView {
        plugin_id,
        description: bounded(description, MAX_ITEM_TEXT_BYTES),
        capabilities,
        website_url,
        privacy_policy_url,
        terms_of_service_url,
        skills,
        apps,
        app_templates,
        hooks,
        mcp_servers,
        scheduled_tasks,
    }
}

fn map_app_detail(app: codex_protocol::ConnectorMetadata) -> AppDetailView {
    let tools = app
        .tool_summaries
        .unwrap_or_default()
        .into_iter()
        .take(codex_core::MAX_PLUGIN_DETAIL_ITEMS)
        .map(|tool| {
            let name = bounded(tool.name, MAX_SOURCE_TITLE_BYTES);
            AppToolCard {
                title: bounded(
                    tool.title.unwrap_or_else(|| name.clone()),
                    MAX_SOURCE_TITLE_BYTES,
                ),
                name,
                description: bounded(tool.description, MAX_ITEM_TEXT_BYTES),
            }
        })
        .collect();
    AppDetailView {
        app_id: bounded(app.id, MAX_SOURCE_TITLE_BYTES),
        name: bounded(app.name, MAX_SOURCE_TITLE_BYTES),
        description: bounded(app.description.unwrap_or_default(), MAX_ITEM_TEXT_BYTES),
        logo_url: bounded_http_url(app.icon_url),
        logo_url_dark: bounded_http_url(app.icon_url_dark),
        install_url: bounded_http_url(app.install_url),
        distribution_channel: app
            .distribution_channel
            .map(|channel| bounded(channel, MAX_SOURCE_TITLE_BYTES)),
        plugin_display_names: app
            .plugin_display_names
            .into_iter()
            .take(codex_core::MAX_PLUGIN_DETAIL_ITEMS)
            .map(|name| bounded(name, MAX_SOURCE_TITLE_BYTES))
            .collect(),
        tools,
    }
}

fn bounded_http_url(url: Option<String>) -> Option<String> {
    let url = url?;
    let url = url.trim();
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme.to_ascii_lowercase());
    matches!(scheme.as_deref(), Some("http" | "https"))
        .then(|| bounded(url.to_owned(), MAX_STATUS_BYTES))
}

fn map_user_input_request(request_id: String, params: Value) -> Result<UserInputRequest, ()> {
    let request: ToolRequestUserInputParams = serde_json::from_value(params).map_err(|_| ())?;
    if request_id.is_empty()
        || request_id.len() > MAX_MCP_SERVER_FIELD_BYTES
        || request.thread_id.trim().is_empty()
        || request.thread_id.len() > MAX_MCP_SERVER_FIELD_BYTES
        || request.turn_id.trim().is_empty()
        || request.turn_id.len() > MAX_MCP_SERVER_FIELD_BYTES
        || request.item_id.trim().is_empty()
        || request.item_id.len() > MAX_MCP_SERVER_FIELD_BYTES
        || request.questions.is_empty()
        || request.questions.len() > MAX_USER_INPUT_QUESTIONS
    {
        return Err(());
    }

    let mut question_ids = HashSet::new();
    let mut questions = Vec::with_capacity(request.questions.len());
    for question in request.questions {
        let id = question.id.trim();
        let header = question.header.trim();
        let prompt = question.question.trim();
        if id.is_empty()
            || id.len() > MAX_MCP_SERVER_FIELD_BYTES
            || header.is_empty()
            || header.len() > MAX_MCP_SERVER_FIELD_BYTES
            || prompt.is_empty()
            || prompt.len() > MAX_MCP_SERVER_FIELD_BYTES
            || !question_ids.insert(id.to_owned())
        {
            return Err(());
        }
        let options = question.options.unwrap_or_default();
        if options.len() > MAX_USER_INPUT_OPTIONS {
            return Err(());
        }
        let mut option_labels = HashSet::new();
        let mut mapped_options = Vec::with_capacity(options.len());
        for option in options {
            let label = option.label.trim();
            let description = option.description.trim();
            if label.is_empty()
                || label.len() > MAX_USER_INPUT_VALUE_BYTES
                || description.len() > MAX_MCP_SERVER_FIELD_BYTES
                || !option_labels.insert(label.to_owned())
            {
                return Err(());
            }
            mapped_options.push(CoreUserInputOption {
                label: label.to_owned(),
                description: description.to_owned(),
            });
        }
        questions.push(CoreUserInputQuestion {
            id: id.to_owned(),
            header: header.to_owned(),
            question: prompt.to_owned(),
            options: mapped_options,
            is_other: question.is_other,
            is_secret: question.is_secret,
        });
    }

    Ok(UserInputRequest {
        request_id,
        task_id: request.thread_id.trim().to_owned(),
        turn_id: request.turn_id.trim().to_owned(),
        item_id: request.item_id.trim().to_owned(),
        auto_resolution_ms: request.auto_resolution_ms,
        questions,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum McpElicitationMapError {
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BrowserResourceElicitationMetadata {
    origin: String,
    source_name: String,
    resource: BrowserPermissionResource,
    reason: Option<String>,
    persist_session: bool,
    persist_always: bool,
    elevated_risk: bool,
}

fn browser_elicitation_source_name(
    metadata: &serde_json::Map<String, Value>,
) -> Result<String, McpElicitationMapError> {
    let connector_id = metadata
        .get("connector_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    let connector_name = metadata
        .get("connector_name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    let connector_identity = format!("{connector_id} {connector_name}").to_ascii_lowercase();
    if connector_identity.contains("chrome") {
        Ok("Chrome".to_owned())
    } else if connector_identity.contains("browser") {
        Ok("Browser".to_owned())
    } else {
        Err(McpElicitationMapError::Invalid)
    }
}

fn browser_elicitation_persist_modes(metadata: &serde_json::Map<String, Value>) -> (bool, bool) {
    match metadata.get("persist") {
        Some(Value::String(value)) => (value == "session", value == "always"),
        Some(Value::Array(values)) => (
            values.iter().any(|value| value.as_str() == Some("session")),
            values.iter().any(|value| value.as_str() == Some("always")),
        ),
        _ => (false, false),
    }
}

fn browser_origin_elicitation_metadata(
    metadata: Option<&Value>,
) -> Result<Option<(String, Option<String>, String)>, McpElicitationMapError> {
    let Some(metadata) = metadata.and_then(Value::as_object) else {
        return Ok(None);
    };
    if metadata.get("codex_approval_kind").and_then(Value::as_str) != Some("mcp_tool_call")
        || metadata.get("tool_name").and_then(Value::as_str) != Some("access_browser_origin")
    {
        return Ok(None);
    }
    let (_, persists_always) = browser_elicitation_persist_modes(metadata);
    if !persists_always {
        return Err(McpElicitationMapError::Invalid);
    }
    let source_name = browser_elicitation_source_name(metadata)?;
    let tool_params = metadata.get("tool_params").and_then(Value::as_object);
    let origin = metadata
        .get("origin")
        .and_then(Value::as_str)
        .or_else(|| tool_params?.get("origin")?.as_str())
        .and_then(normalize_browser_origin)
        .ok_or(McpElicitationMapError::Invalid)?;
    if origin.len() > MAX_BROWSER_PERMISSION_ORIGIN_BYTES {
        return Err(McpElicitationMapError::Invalid);
    }
    let reason = metadata
        .get("reason")
        .and_then(Value::as_str)
        .or_else(|| tool_params?.get("reason")?.as_str())
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .map(|reason| bounded(reason.to_owned(), MAX_MCP_SERVER_FIELD_BYTES));
    Ok(Some((origin, reason, source_name)))
}

fn browser_resource_elicitation_metadata(
    metadata: Option<&Value>,
) -> Result<Option<BrowserResourceElicitationMetadata>, McpElicitationMapError> {
    let Some(metadata) = metadata.and_then(Value::as_object) else {
        return Ok(None);
    };
    if metadata.get("codex_approval_kind").and_then(Value::as_str) != Some("mcp_tool_call") {
        return Ok(None);
    }
    let tool_params = metadata.get("tool_params").and_then(Value::as_object);
    let tool_name = metadata.get("tool_name").and_then(Value::as_str);
    let (resource, asset_origin) = match tool_name {
        Some("download_browser_files") => (BrowserPermissionResource::Download, None),
        Some("upload_browser_files") => (BrowserPermissionResource::Upload, None),
        Some("access_browser_origin_with_raw_cdp") => {
            if metadata.get("riskLevel").and_then(Value::as_str) != Some("high")
                || metadata.get("full_cdp_access").and_then(Value::as_bool) != Some(true)
            {
                return Err(McpElicitationMapError::Invalid);
            }
            (BrowserPermissionResource::FullCdp, None)
        }
        None => {
            let Some(origins) = tool_params
                .and_then(|params| params.get("asset_origins"))
                .and_then(Value::as_array)
            else {
                return Ok(None);
            };
            let [origin] = origins.as_slice() else {
                return Err(McpElicitationMapError::Invalid);
            };
            (
                BrowserPermissionResource::Download,
                Some(origin.as_str().ok_or(McpElicitationMapError::Invalid)?),
            )
        }
        Some(_) => return Ok(None),
    };
    let source_name = browser_elicitation_source_name(metadata)?;
    let (persist_session, persist_always) = browser_elicitation_persist_modes(metadata);
    let expected_session = resource != BrowserPermissionResource::FullCdp;
    if !persist_always || persist_session != expected_session {
        return Err(McpElicitationMapError::Invalid);
    }
    let origin = asset_origin
        .or_else(|| metadata.get("origin").and_then(Value::as_str))
        .or_else(|| tool_params?.get("origin")?.as_str())
        .and_then(normalize_browser_origin)
        .ok_or(McpElicitationMapError::Invalid)?;
    if origin.len() > MAX_BROWSER_PERMISSION_ORIGIN_BYTES {
        return Err(McpElicitationMapError::Invalid);
    }
    let reason = metadata
        .get("reason")
        .and_then(Value::as_str)
        .or_else(|| tool_params?.get("reason")?.as_str())
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .map(|reason| bounded(reason.to_owned(), MAX_MCP_SERVER_FIELD_BYTES));
    Ok(Some(BrowserResourceElicitationMetadata {
        origin,
        source_name,
        resource,
        reason,
        persist_session,
        persist_always,
        elevated_risk: resource == BrowserPermissionResource::FullCdp,
    }))
}

fn map_mcp_elicitation(
    request_id: String,
    params: Value,
) -> Result<McpElicitation, McpElicitationMapError> {
    let unsupported_openai = unsupported_openai_form_elicitation(request_id.as_str(), &params);
    let params = match serde_json::from_value::<McpServerElicitationRequestParams>(params) {
        Ok(params) => params,
        Err(_) => {
            return unsupported_openai
                .map(McpElicitation::Form)
                .ok_or(McpElicitationMapError::Invalid);
        }
    };
    let request_id = request_id.trim();
    let task_id = params.thread_id.trim();
    if request_id.is_empty()
        || request_id.len() > MAX_MCP_SERVER_FIELD_BYTES
        || task_id.is_empty()
        || task_id.len() > MAX_MCP_SERVER_FIELD_BYTES
    {
        return Err(McpElicitationMapError::Invalid);
    }
    let turn_id = params
        .turn_id
        .map(|turn_id| bounded(turn_id.trim().to_owned(), MAX_MCP_SERVER_FIELD_BYTES))
        .filter(|turn_id| !turn_id.is_empty());
    let server_name = bounded(
        params.server_name.trim().to_owned(),
        MAX_MCP_SERVER_FIELD_BYTES,
    );
    let request_id = request_id.to_owned();
    let task_id = task_id.to_owned();

    match params.request {
        McpServerElicitationRequest::Url {
            elicitation_id,
            message,
            url,
            ..
        } => {
            let url = url.trim();
            if !is_supported_mcp_elicitation_url(url) {
                return Err(McpElicitationMapError::Invalid);
            }
            Ok(McpElicitation::Url(McpUrlElicitation {
                request_id,
                task_id,
                turn_id,
                server_name,
                elicitation_id: bounded(
                    elicitation_id.trim().to_owned(),
                    MAX_MCP_SERVER_FIELD_BYTES,
                ),
                message: bounded(message.trim().to_owned(), MAX_MCP_SERVER_FIELD_BYTES),
                url: url.to_owned(),
                link_opened: false,
            }))
        }
        McpServerElicitationRequest::Form {
            message,
            requested_schema,
            metadata,
        } => {
            if requested_schema.properties.len() > MAX_MCP_FORM_FIELDS {
                return Err(McpElicitationMapError::Invalid);
            }
            if requested_schema.properties.is_empty()
                && let Some((origin, reason, source_name)) =
                    browser_origin_elicitation_metadata(metadata.as_ref())?
            {
                return Ok(McpElicitation::BrowserOrigin(McpBrowserOriginElicitation {
                    request_id,
                    task_id,
                    turn_id,
                    server_name,
                    source_name,
                    origin,
                    reason,
                    message: bounded(message.trim().to_owned(), MAX_MCP_SERVER_FIELD_BYTES),
                }));
            }
            if requested_schema.properties.is_empty()
                && let Some(browser) = browser_resource_elicitation_metadata(metadata.as_ref())?
            {
                return Ok(McpElicitation::BrowserResource(
                    McpBrowserResourceElicitation {
                        request_id,
                        task_id,
                        turn_id,
                        server_name,
                        source_name: browser.source_name,
                        origin: browser.origin,
                        resource: browser.resource,
                        message: bounded(message.trim().to_owned(), MAX_MCP_SERVER_FIELD_BYTES),
                        reason: browser.reason,
                        persist_session: browser.persist_session,
                        persist_always: browser.persist_always,
                        elevated_risk: browser.elevated_risk,
                    },
                ));
            }
            let required = requested_schema
                .required
                .unwrap_or_default()
                .into_iter()
                .collect::<HashSet<_>>();
            let fields = requested_schema
                .properties
                .into_iter()
                .map(|(name, schema)| map_mcp_form_field(name, schema, &required))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(McpElicitation::Form(McpFormElicitation {
                request_id,
                task_id,
                turn_id,
                server_name,
                message: bounded(message.trim().to_owned(), MAX_MCP_SERVER_FIELD_BYTES),
                openai: false,
                unsupported_openai: false,
                fields,
            }))
        }
        McpServerElicitationRequest::OpenAiForm {
            message,
            requested_schema,
            ..
        } => {
            if requested_schema.properties.len() > MAX_MCP_FORM_FIELDS {
                return Ok(McpElicitation::Form(McpFormElicitation {
                    request_id,
                    task_id,
                    turn_id,
                    server_name,
                    message: bounded(message.trim().to_owned(), MAX_MCP_SERVER_FIELD_BYTES),
                    openai: true,
                    unsupported_openai: true,
                    fields: Vec::new(),
                }));
            }
            let required = requested_schema
                .required
                .unwrap_or_default()
                .into_iter()
                .collect::<HashSet<_>>();
            let fields = requested_schema
                .properties
                .into_iter()
                .map(|(name, schema)| match schema {
                    McpOpenAiElicitationFieldSchema::Primitive(schema) => {
                        map_mcp_form_field(name, schema, &required)
                    }
                    McpOpenAiElicitationFieldSchema::ImagePicker(schema) => {
                        map_openai_image_picker_field(name, schema, &required)
                    }
                })
                .collect::<Result<Vec<_>, _>>();
            let (fields, unsupported_openai) = match fields {
                Ok(fields) => (fields, false),
                Err(_) => (Vec::new(), true),
            };
            Ok(McpElicitation::Form(McpFormElicitation {
                request_id,
                task_id,
                turn_id,
                server_name,
                message: bounded(message.trim().to_owned(), MAX_MCP_SERVER_FIELD_BYTES),
                openai: true,
                unsupported_openai,
                fields,
            }))
        }
    }
}

fn unsupported_openai_form_elicitation(
    request_id: &str,
    params: &Value,
) -> Option<McpFormElicitation> {
    let request_id = request_id.trim();
    if params.get("mode").and_then(Value::as_str) != Some("openai/form")
        || request_id.is_empty()
        || request_id.len() > MAX_MCP_SERVER_FIELD_BYTES
    {
        return None;
    }
    let task_id = params.get("threadId")?.as_str()?.trim();
    let server_name = params.get("serverName")?.as_str()?.trim();
    let message = params.get("message")?.as_str()?.trim();
    if task_id.is_empty() || task_id.len() > MAX_MCP_SERVER_FIELD_BYTES {
        return None;
    }
    let turn_id = match params.get("turnId") {
        Some(Value::String(turn_id)) => Some(bounded(
            turn_id.trim().to_owned(),
            MAX_MCP_SERVER_FIELD_BYTES,
        ))
        .filter(|turn_id| !turn_id.is_empty()),
        Some(Value::Null) | None => None,
        Some(_) => return None,
    };
    Some(McpFormElicitation {
        request_id: request_id.to_owned(),
        task_id: task_id.to_owned(),
        turn_id,
        server_name: bounded(server_name.to_owned(), MAX_MCP_SERVER_FIELD_BYTES),
        message: bounded(message.to_owned(), MAX_MCP_SERVER_FIELD_BYTES),
        openai: true,
        unsupported_openai: true,
        fields: Vec::new(),
    })
}

fn is_supported_mcp_elicitation_url(url: &str) -> bool {
    url.len() <= MAX_MCP_SERVER_FIELD_BYTES
        && gpui::http_client::Uri::from_str(url)
            .ok()
            .is_some_and(|uri| {
                uri.scheme_str() == Some("https")
                    && uri.host().is_some()
                    && uri
                        .authority()
                        .is_none_or(|authority| !authority.as_str().contains('@'))
            })
}

fn map_mcp_form_field(
    name: String,
    schema: McpElicitationPrimitiveSchema,
    required: &HashSet<String>,
) -> Result<McpFormField, McpElicitationMapError> {
    let name = name.trim().to_owned();
    if name.is_empty() || name.len() > MAX_MCP_SERVER_FIELD_BYTES {
        return Err(McpElicitationMapError::Invalid);
    }
    let field_required = required.contains(&name);
    let (title, description, kind, default) = match schema {
        McpElicitationPrimitiveSchema::String {
            title,
            description,
            min_length,
            max_length,
            format,
            default,
            enum_values,
            enum_names,
            one_of,
        } => {
            if min_length.is_some_and(|value| value as usize > MAX_MCP_FORM_VALUE_BYTES) {
                return Err(McpElicitationMapError::Invalid);
            }
            let kind = if let Some(options) = one_of {
                McpFormFieldKind::SingleSelect {
                    options: map_titled_mcp_form_options(options)?,
                }
            } else if let Some(values) = enum_values {
                if values.len() > MAX_MCP_FORM_OPTIONS {
                    return Err(McpElicitationMapError::Invalid);
                }
                let names = enum_names.unwrap_or_default();
                let options = values
                    .into_iter()
                    .enumerate()
                    .map(|(index, value)| {
                        let label = names.get(index).cloned().unwrap_or_else(|| value.clone());
                        map_mcp_form_option(value, label)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                McpFormFieldKind::SingleSelect { options }
            } else {
                McpFormFieldKind::String {
                    min_length: min_length.map(|value| value as usize),
                    max_length: max_length
                        .map(|value| value as usize)
                        .map(|value| value.min(MAX_MCP_FORM_VALUE_BYTES)),
                    format: format.map(map_mcp_form_string_format),
                }
            };
            let default = default
                .map(|value| {
                    if value.len() > MAX_MCP_FORM_VALUE_BYTES {
                        Err(McpElicitationMapError::Invalid)
                    } else {
                        Ok(McpElicitationValue::String(value))
                    }
                })
                .transpose()?;
            (title, description, kind, default)
        }
        McpElicitationPrimitiveSchema::Array {
            title,
            description,
            min_items,
            max_items,
            items,
            default,
        } => {
            let options = match items {
                McpElicitationArrayItems::Untitled { values, .. } => values
                    .into_iter()
                    .map(|value| map_mcp_form_option(value.clone(), value))
                    .collect::<Result<Vec<_>, _>>()?,
                McpElicitationArrayItems::Titled { any_of } => map_titled_mcp_form_options(any_of)?,
            };
            if options.len() > MAX_MCP_FORM_OPTIONS
                || min_items.is_some_and(|value| value > MAX_MCP_FORM_OPTIONS as u64)
            {
                return Err(McpElicitationMapError::Invalid);
            }
            let default = default
                .map(|values| {
                    if values.len() > MAX_MCP_FORM_OPTIONS
                        || values
                            .iter()
                            .any(|value| value.len() > MAX_MCP_FORM_VALUE_BYTES)
                    {
                        Err(McpElicitationMapError::Invalid)
                    } else {
                        Ok(McpElicitationValue::Strings(values))
                    }
                })
                .transpose()?;
            (
                title,
                description,
                McpFormFieldKind::MultiSelect {
                    options,
                    min_items: min_items.map(|value| value as usize),
                    max_items: max_items
                        .and_then(|value| usize::try_from(value).ok())
                        .map(|value| value.min(MAX_MCP_FORM_OPTIONS)),
                },
                default,
            )
        }
        McpElicitationPrimitiveSchema::Boolean {
            title,
            description,
            default,
        } => (
            title,
            description,
            McpFormFieldKind::Boolean,
            default.map(McpElicitationValue::Boolean),
        ),
        McpElicitationPrimitiveSchema::Number {
            title,
            description,
            minimum,
            maximum,
            default,
        } => (
            title,
            description,
            McpFormFieldKind::Number {
                integer: false,
                minimum: minimum.map(|value| value.to_string()),
                maximum: maximum.map(|value| value.to_string()),
            },
            default.map(|value| McpElicitationValue::Number(value.to_string())),
        ),
        McpElicitationPrimitiveSchema::Integer {
            title,
            description,
            minimum,
            maximum,
            default,
        } => (
            title,
            description,
            McpFormFieldKind::Number {
                integer: true,
                minimum: minimum.map(|value| value.to_string()),
                maximum: maximum.map(|value| value.to_string()),
            },
            default.map(|value| McpElicitationValue::Number(value.to_string())),
        ),
    };
    Ok(McpFormField {
        title: bounded(
            title
                .unwrap_or_else(|| mcp_form_field_title(&name))
                .trim()
                .to_owned(),
            MAX_MCP_SERVER_FIELD_BYTES,
        ),
        description: description
            .map(|description| bounded(description.trim().to_owned(), MAX_MCP_SERVER_FIELD_BYTES))
            .filter(|description| !description.is_empty()),
        name,
        required: field_required,
        kind,
        default,
    })
}

fn map_openai_image_picker_field(
    name: String,
    schema: McpOpenAiImagePickerSchema,
    required: &HashSet<String>,
) -> Result<McpFormField, McpElicitationMapError> {
    let name = name.trim().to_owned();
    if name.is_empty()
        || name.len() > MAX_MCP_SERVER_FIELD_BYTES
        || schema.items.is_empty()
        || schema.items.len() > MAX_MCP_FORM_OPTIONS
    {
        return Err(McpElicitationMapError::Invalid);
    }

    let mut values = HashSet::new();
    let items = schema
        .items
        .into_iter()
        .map(|item| {
            if item.id.trim().is_empty()
                || item.id.len() > MAX_MCP_FORM_VALUE_BYTES
                || item.title.trim().is_empty()
                || item.title.len() > MAX_MCP_SERVER_FIELD_BYTES
                || !values.insert(item.id.clone())
                || !is_supported_mcp_image_data_url(&item.image)
            {
                return Err(McpElicitationMapError::Invalid);
            }
            Ok(McpFormImagePickerItem {
                value: item.id,
                title: item.title,
                image_data_url: Arc::from(item.image),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(McpFormField {
        title: bounded(
            schema.title.unwrap_or_else(|| mcp_form_field_title(&name)),
            MAX_MCP_SERVER_FIELD_BYTES,
        ),
        description: schema
            .description
            .map(|description| bounded(description, MAX_MCP_SERVER_FIELD_BYTES)),
        required: required.contains(&name),
        name,
        kind: McpFormFieldKind::ImagePicker { items },
        default: None,
    })
}

fn is_supported_mcp_image_data_url(value: &str) -> bool {
    if value.len() > MAX_MCP_FORM_IMAGE_DATA_URL_BYTES {
        return false;
    }
    let Some(value) = value.strip_prefix("data:image/") else {
        return false;
    };
    let Some((subtype, payload)) = value.split_once(";base64,") else {
        return false;
    };
    if subtype.is_empty()
        || !subtype
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'+' | b'-'))
        || payload.is_empty()
    {
        return false;
    }
    let padding = payload
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'=')
        .count();
    padding <= 2
        && payload.len() > padding
        && payload[..payload.len() - padding]
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/'))
}

fn map_mcp_form_option(
    value: String,
    label: String,
) -> Result<McpFormOption, McpElicitationMapError> {
    if value.is_empty()
        || value.len() > MAX_MCP_FORM_VALUE_BYTES
        || label.len() > MAX_MCP_SERVER_FIELD_BYTES
    {
        return Err(McpElicitationMapError::Invalid);
    }
    Ok(McpFormOption { value, label })
}

fn map_titled_mcp_form_options(
    options: Vec<codex_protocol::McpElicitationConstOption>,
) -> Result<Vec<McpFormOption>, McpElicitationMapError> {
    if options.len() > MAX_MCP_FORM_OPTIONS {
        return Err(McpElicitationMapError::Invalid);
    }
    options
        .into_iter()
        .map(|option| map_mcp_form_option(option.value, option.title))
        .collect()
}

const fn map_mcp_form_string_format(format: McpElicitationStringFormat) -> McpFormStringFormat {
    match format {
        McpElicitationStringFormat::Email => McpFormStringFormat::Email,
        McpElicitationStringFormat::Uri => McpFormStringFormat::Uri,
        McpElicitationStringFormat::Date => McpFormStringFormat::Date,
        McpElicitationStringFormat::DateTime => McpFormStringFormat::DateTime,
    }
}

fn mcp_form_field_title(name: &str) -> String {
    let mut title = String::with_capacity(name.len());
    let mut previous_lowercase = false;
    for character in name.chars() {
        if !character.is_alphanumeric() {
            if !title.ends_with(' ') && !title.is_empty() {
                title.push(' ');
            }
            previous_lowercase = false;
            continue;
        }
        if character.is_uppercase() && previous_lowercase && !title.ends_with(' ') {
            title.push(' ');
        }
        if title.is_empty() || title.ends_with(' ') {
            title.extend(character.to_uppercase());
        } else {
            title.push(character);
        }
        previous_lowercase = character.is_lowercase() || character.is_ascii_digit();
    }
    title
}

fn map_plugin_schedule(schedule: codex_protocol::PluginScheduledTaskSchedule) -> String {
    match schedule {
        codex_protocol::PluginScheduledTaskSchedule::Hourly {
            interval_hours,
            days,
        } => {
            let days = days
                .unwrap_or_default()
                .into_iter()
                .map(plugin_weekday_label)
                .collect::<Vec<_>>();
            if days.is_empty() {
                format!("Every {interval_hours} hour(s)")
            } else {
                format!("Every {interval_hours} hour(s) · {}", days.join(", "))
            }
        }
        codex_protocol::PluginScheduledTaskSchedule::Daily { time } => {
            format!("Daily at {time}")
        }
        codex_protocol::PluginScheduledTaskSchedule::Weekdays { time } => {
            format!("Weekdays at {time}")
        }
        codex_protocol::PluginScheduledTaskSchedule::Weekly { days, time } => format!(
            "Weekly on {} at {time}",
            days.into_iter()
                .map(plugin_weekday_label)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

const fn plugin_weekday_label(day: codex_protocol::PluginScheduledTaskWeekday) -> &'static str {
    match day {
        codex_protocol::PluginScheduledTaskWeekday::Mo => "Mon",
        codex_protocol::PluginScheduledTaskWeekday::Tu => "Tue",
        codex_protocol::PluginScheduledTaskWeekday::We => "Wed",
        codex_protocol::PluginScheduledTaskWeekday::Th => "Thu",
        codex_protocol::PluginScheduledTaskWeekday::Fr => "Fri",
        codex_protocol::PluginScheduledTaskWeekday::Sa => "Sat",
        codex_protocol::PluginScheduledTaskWeekday::Su => "Sun",
    }
}

const fn protocol_approvals_reviewer(reviewer: CoreApprovalsReviewer) -> ProtocolApprovalsReviewer {
    match reviewer {
        CoreApprovalsReviewer::User => ProtocolApprovalsReviewer::User,
        CoreApprovalsReviewer::AutoReview => ProtocolApprovalsReviewer::AutoReview,
    }
}

const fn map_goal_status_to_protocol(status: CoreThreadGoalStatus) -> ProtocolThreadGoalStatus {
    match status {
        CoreThreadGoalStatus::Active => ProtocolThreadGoalStatus::Active,
        CoreThreadGoalStatus::Paused => ProtocolThreadGoalStatus::Paused,
        CoreThreadGoalStatus::Blocked => ProtocolThreadGoalStatus::Blocked,
        CoreThreadGoalStatus::UsageLimited => ProtocolThreadGoalStatus::UsageLimited,
        CoreThreadGoalStatus::BudgetLimited => ProtocolThreadGoalStatus::BudgetLimited,
        CoreThreadGoalStatus::Complete => ProtocolThreadGoalStatus::Complete,
    }
}

fn map_turn_status(status: &str) -> Option<TaskRunStatus> {
    match status {
        "inProgress" => Some(TaskRunStatus::Running),
        "completed" => Some(TaskRunStatus::Completed),
        "interrupted" => Some(TaskRunStatus::Interrupted),
        "failed" => Some(TaskRunStatus::Failed),
        _ => None,
    }
}

fn review_mode_marker(item: &Value) -> Option<bool> {
    match string_field(item, "type").as_deref() {
        Some("enteredReviewMode") => Some(true),
        Some("exitedReviewMode") => Some(false),
        _ => None,
    }
}

fn newest_review_mode_from_items<'a>(items: impl IntoIterator<Item = &'a Value>) -> Option<bool> {
    items
        .into_iter()
        .take(HISTORY_PAGE_LIMIT as usize)
        .find_map(review_mode_marker)
}

fn load_active_turn_review_mode(
    app_server: &AppServerConnection,
    thread_id: &str,
    turn_id: &str,
) -> bool {
    let mut cursor = None;
    for _ in 0..MAX_REVIEW_MODE_PAGES {
        let Ok(page) = app_server.list_thread_items(ThreadItemsListParams {
            thread_id: thread_id.to_owned(),
            limit: HISTORY_PAGE_LIMIT,
            sort_direction: HistorySortDirection::Desc,
            turn_id: Some(turn_id.to_owned()),
            cursor,
        }) else {
            break;
        };
        if let Some(review_mode) =
            newest_review_mode_from_items(page.data.iter().map(|entry| &entry.item))
        {
            return review_mode;
        }
        let Some(next_cursor) = page.next_cursor else {
            return false;
        };
        cursor = Some(next_cursor);
    }
    true
}

fn map_account_profile(account: ProtocolAccount) -> AccountProfile {
    match account {
        ProtocolAccount::ApiKey => AccountProfile {
            kind: AccountKind::ApiKey,
            email: None,
            plan: None,
        },
        ProtocolAccount::ChatGpt { email, plan_type } => AccountProfile {
            kind: AccountKind::ChatGpt,
            email: email.map(|email| bounded(email, 512)),
            plan: Some(plan_type_label(plan_type).to_owned()),
        },
        ProtocolAccount::AmazonBedrock { .. } => AccountProfile {
            kind: AccountKind::AmazonBedrock,
            email: None,
            plan: None,
        },
    }
}

fn plan_type_label(plan_type: PlanType) -> &'static str {
    match plan_type {
        PlanType::Free => "Free",
        PlanType::Go => "Go",
        PlanType::Plus => "Plus",
        PlanType::Pro => "Pro",
        PlanType::Prolite => "Pro Lite",
        PlanType::Team => "Team",
        PlanType::SelfServeBusinessUsageBased => "Business (usage based)",
        PlanType::Business => "Business",
        PlanType::EnterpriseCbpUsageBased => "Enterprise (usage based)",
        PlanType::Enterprise => "Enterprise",
        PlanType::Edu => "Edu",
        PlanType::Unknown => "Unknown",
    }
}

fn map_usage_limit_window(window: codex_protocol::RateLimitWindow) -> UsageLimitWindow {
    let used_percent = if window.used_percent.is_finite() {
        window.used_percent.round().clamp(0.0, 100.0) as u8
    } else {
        0
    };
    UsageLimitWindow {
        used_percent,
        window_duration_mins: window.window_duration_mins.filter(|minutes| *minutes > 0),
        resets_at: window.resets_at.filter(|timestamp| *timestamp > 0),
    }
}

fn map_background_terminal(terminal: ThreadBackgroundTerminal) -> BackgroundTerminal {
    BackgroundTerminal {
        item_id: bounded(terminal.item_id, 512),
        process_id: bounded(terminal.process_id, 512),
        command: bounded(terminal.command, 16 * 1024),
        cwd: PathBuf::from(bounded(
            terminal.cwd.to_string_lossy().into_owned(),
            32 * 1024,
        )),
        os_pid: terminal.os_pid.filter(|pid| *pid > 0),
        cpu_percent: terminal
            .cpu_percent
            .filter(|percent| percent.is_finite() && *percent >= 0.0),
        rss_kb: terminal.rss_kb,
    }
}

fn map_task(thread: codex_protocol::ThreadSummary) -> TaskSummary {
    let title = thread
        .name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| {
            thread
                .preview
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("Untitled task")
                .trim()
                .to_owned()
        });
    let status_type = thread
        .status
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("idle");
    let waiting = thread
        .status
        .get("activeFlags")
        .and_then(Value::as_array)
        .is_some_and(|flags| {
            flags
                .iter()
                .any(|flag| flag.as_str() == Some("waitingOnApproval"))
        });
    TaskSummary {
        id: thread.id,
        title: bounded(title, 512),
        preview: bounded(thread.preview, 4 * 1024),
        cwd: thread.cwd,
        created_at: thread.created_at,
        updated_at: thread.recency_at.unwrap_or(thread.updated_at),
        parent_task_id: thread.parent_thread_id,
        forked_from_id: thread.forked_from_id,
        status: if waiting {
            TaskRunStatus::WaitingForApproval
        } else {
            match status_type {
                "active" => TaskRunStatus::Running,
                "systemError" => TaskRunStatus::Failed,
                _ => TaskRunStatus::Idle,
            }
        },
    }
}

fn map_timeline_item(turn_id: String, item: Value, completed: bool) -> TimelineItem {
    let item_type = string_field(&item, "type").unwrap_or_else(|| "notice".to_owned());
    let id = string_field(&item, "id").unwrap_or_else(|| format!("{turn_id}:{item_type}"));
    let completed = match item_type.as_str() {
        "collabAgentToolCall" | "imageGeneration" => {
            string_field(&item, "status").as_deref() != Some("inProgress")
        }
        _ => completed,
    };
    let process_id = (item_type == "commandExecution")
        .then(|| string_field(&item, "processId"))
        .flatten()
        .filter(|process_id| !process_id.trim().is_empty())
        .map(|process_id| bounded(process_id, 512));
    let memory_citations = if item_type == "agentMessage" {
        map_memory_citations(&item)
    } else {
        Vec::new()
    };
    let sources = if item_type == "webSearch" {
        map_web_search_sources(&item)
    } else {
        Vec::new()
    };
    let (attachments, edit_supported) = if item_type == "userMessage" {
        map_editable_message_attachments(&item)
    } else {
        (Vec::new(), false)
    };
    let output_artifacts = map_output_artifacts(&item_type, &item, completed);
    let (kind, text, detail) = match item_type.as_str() {
        "userMessage" => (
            TimelineKind::User,
            item.get("content")
                .and_then(Value::as_array)
                .map(|content| {
                    content
                        .iter()
                        .filter_map(|entry| {
                            string_field(entry, "text")
                                .or_else(|| string_field(entry, "path"))
                                .or_else(|| string_field(entry, "name"))
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default(),
            None,
        ),
        "agentMessage" => (
            TimelineKind::Agent,
            string_field(&item, "text").unwrap_or_default(),
            None,
        ),
        "plan" => (
            TimelineKind::Plan,
            string_field(&item, "text").unwrap_or_default(),
            None,
        ),
        "reasoning" => {
            let text = string_array(&item, "summary");
            let text = if text.is_empty() {
                string_array(&item, "content")
            } else {
                text
            };
            (TimelineKind::Reasoning, text, None)
        }
        "commandExecution" => {
            let (text, detail) = command_activity(&item, completed);
            (TimelineKind::Command, text, detail)
        }
        "fileChange" => {
            let (text, detail) = file_change_activity(&item);
            (TimelineKind::FileChange, text, detail)
        }
        "mcpToolCall" => {
            let server = string_field(&item, "server").unwrap_or_default();
            let tool = string_field(&item, "tool").unwrap_or_default();
            (TimelineKind::Tool, format!("{server} / {tool}"), None)
        }
        "dynamicToolCall" => {
            let namespace = string_field(&item, "namespace").unwrap_or_default();
            let tool = string_field(&item, "tool").unwrap_or_default();
            (TimelineKind::Tool, format!("{namespace} / {tool}"), None)
        }
        "webSearch" => {
            let (text, detail) = web_search_activity(&item);
            (TimelineKind::WebSearch, text, detail)
        }
        "collabAgentToolCall" => {
            let (text, detail) = collab_agent_activity(&item);
            (TimelineKind::Subagent, text, detail)
        }
        "subAgentActivity" => {
            let (text, detail) = subagent_activity(&item);
            (TimelineKind::Subagent, text, detail)
        }
        "imageView" => (
            TimelineKind::Image,
            "Viewed an image".to_owned(),
            string_field(&item, "path").and_then(nonempty_detail),
        ),
        "imageGeneration" => {
            let (text, detail) = image_generation_activity(&item, completed);
            (TimelineKind::Image, text, detail)
        }
        "enteredReviewMode" | "exitedReviewMode" => (
            TimelineKind::Notice,
            string_field(&item, "review").unwrap_or(item_type),
            None,
        ),
        "contextCompaction" => (
            TimelineKind::ContextCompaction,
            if completed {
                "Context compacted".to_owned()
            } else {
                "Compacting context".to_owned()
            },
            None,
        ),
        _ => (TimelineKind::Notice, item_type, None),
    };
    TimelineItem {
        id,
        turn_id,
        kind,
        text: bounded(text, MAX_ITEM_TEXT_BYTES),
        detail: detail.map(|detail| bounded(detail, MAX_ITEM_TEXT_BYTES)),
        process_id,
        memory_citations,
        sources,
        attachments,
        output_artifacts,
        edit_supported,
        completed,
    }
}

fn map_output_artifacts(item_type: &str, item: &Value, completed: bool) -> Vec<OutputArtifact> {
    const MAX_OUTPUT_ARTIFACTS_PER_ITEM: usize = 128;

    let mut paths = Vec::new();
    match item_type {
        "agentMessage" if completed => {
            if let Some(text) = string_field(item, "text") {
                paths.extend(output_directive_paths(&text));
            }
        }
        "fileChange" if completed => {
            paths.extend(
                item.get("changes")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|change| string_field(change, "path"))
                    .map(|path| (path, OutputArtifactKind::File)),
            );
        }
        "imageGeneration" => {
            let path = string_field(item, "savedPath")
                .or_else(|| string_field(item, "src"))
                .or_else(|| string_field(item, "path"));
            if let Some(path) = path {
                paths.push((path, OutputArtifactKind::GeneratedImage));
            }
        }
        _ => {}
    }

    let mut seen = HashSet::new();
    paths
        .into_iter()
        .filter_map(|(path, kind)| {
            let path = path.trim();
            if path.is_empty()
                || path.len() > codex_platform::MAX_ARTIFACT_PATH_BYTES
                || is_hidden_output_path(path)
                || !is_supported_artifact_path(PathBuf::from(path).as_path())
            {
                return None;
            }
            let path = PathBuf::from(path);
            let mut key = path.to_string_lossy().replace('\\', "/");
            if cfg!(windows) {
                key.make_ascii_lowercase();
            }
            seen.insert(key).then_some(OutputArtifact { path, kind })
        })
        .take(MAX_OUTPUT_ARTIFACTS_PER_ITEM)
        .collect()
}

fn output_directive_paths(text: &str) -> Vec<(String, OutputArtifactKind)> {
    const PREFIX: &str = "::codex-file-citation{";
    const MAX_DIRECTIVES: usize = 128;

    let mut paths = Vec::new();
    let mut cursor = 0;
    while paths.len() < MAX_DIRECTIVES {
        let Some(relative_start) = text[cursor..].find(PREFIX) else {
            break;
        };
        let attributes_start = cursor + relative_start + PREFIX.len();
        let Some(relative_end) = find_output_directive_end(&text[attributes_start..]) else {
            break;
        };
        let attributes_end = attributes_start + relative_end;
        let attributes = &text[attributes_start..attributes_end];
        if output_directive_attribute(attributes, "purpose").as_deref() == Some("output")
            && let Some(path) = output_directive_attribute(attributes, "path")
        {
            paths.push((path, OutputArtifactKind::File));
        }
        cursor = attributes_end + 1;
    }
    paths
}

fn find_output_directive_end(value: &str) -> Option<usize> {
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
        } else if character == '\\' && quote.is_some() {
            escaped = true;
        } else if quote == Some(character) {
            quote = None;
        } else if quote.is_none() && matches!(character, '"' | '\'') {
            quote = Some(character);
        } else if quote.is_none() && character == '}' {
            return Some(index);
        }
    }
    None
}

fn output_directive_attribute(attributes: &str, name: &str) -> Option<String> {
    let bytes = attributes.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let key_start = cursor;
        while cursor < bytes.len()
            && (bytes[cursor].is_ascii_alphanumeric()
                || matches!(bytes[cursor], b'_' | b'-' | b':'))
        {
            cursor += 1;
        }
        if key_start == cursor {
            cursor += 1;
            continue;
        }
        let key = &attributes[key_start..cursor];
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'=') {
            continue;
        }
        cursor += 1;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let (value, next_cursor) = parse_output_directive_attribute_value(attributes, cursor)?;
        cursor = next_cursor;
        if key == name {
            return Some(value);
        }
    }
    None
}

fn parse_output_directive_attribute_value(value: &str, start: usize) -> Option<(String, usize)> {
    let bytes = value.as_bytes();
    let quote = bytes
        .get(start)
        .copied()
        .filter(|byte| matches!(byte, b'"' | b'\''));
    if let Some(quote) = quote {
        let mut cursor = start + 1;
        let mut parsed = String::new();
        while cursor < bytes.len() {
            if bytes[cursor] == quote {
                return Some((parsed, cursor + 1));
            }
            if bytes[cursor] == b'\\' {
                cursor += 1;
                if cursor >= bytes.len() {
                    return None;
                }
                match bytes[cursor] {
                    b'n' => parsed.push('\n'),
                    b'r' => parsed.push('\r'),
                    b't' => parsed.push('\t'),
                    _ => {
                        let character = value[cursor..].chars().next()?;
                        parsed.push(character);
                        cursor += character.len_utf8();
                        continue;
                    }
                }
                cursor += 1;
                continue;
            }
            let character = value[cursor..].chars().next()?;
            parsed.push(character);
            cursor += character.len_utf8();
        }
        None
    } else {
        let mut cursor = start;
        while cursor < bytes.len() && !bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        (cursor > start).then(|| (value[start..cursor].to_owned(), cursor))
    }
}

fn is_hidden_output_path(path: &str) -> bool {
    path.replace('\\', "/")
        .split('/')
        .any(|segment| matches!(segment, "work" | ".codex_scratch"))
}

fn is_hidden_timeline_item(item: &Value) -> bool {
    match string_field(item, "type").as_deref() {
        Some("sleep") => true,
        Some("collabAgentToolCall") => string_field(item, "tool").as_deref() == Some("wait"),
        _ => false,
    }
}

fn collab_agent_activity(item: &Value) -> (String, Option<String>) {
    let action = string_field(item, "tool").unwrap_or_default();
    let status = string_field(item, "status").unwrap_or_else(|| "completed".to_owned());
    let receiver_ids = item
        .get("receiverThreadIds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|thread_id| !thread_id.trim().is_empty())
        .take(16)
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let prompt = string_field(item, "prompt")
        .filter(|prompt| !prompt.trim().is_empty())
        .map(|prompt| bounded(prompt, 8 * 1024));
    let header = collab_action_label(&action, &status, true);
    let summary = match receiver_ids.len() {
        0 => header.to_owned(),
        1 => format!("{header} 1 agent"),
        count => format!("{header} {count} agents"),
    };
    let mut rows = Vec::with_capacity(receiver_ids.len().saturating_add(1));
    for thread_id in &receiver_ids {
        let agent = stable_agent_display(thread_id);
        let state_suffix = collab_agent_state_suffix(item, thread_id);
        let action_label = collab_action_label(&action, &status, false);
        let row = match (action.as_str(), status.as_str(), prompt.as_deref()) {
            ("spawnAgent", "completed", Some(prompt)) => {
                format!("Created {agent} with the instructions: {prompt}")
            }
            ("sendInput", _, Some(prompt)) => {
                format!("{action_label} {agent}: {prompt}")
            }
            _ => format!("{action_label} {agent}{state_suffix}"),
        };
        rows.push(row);
    }
    if !matches!(
        (action.as_str(), status.as_str(), prompt.as_deref()),
        ("spawnAgent", "completed", Some(_)) | ("sendInput", _, Some(_))
    ) && let Some(prompt) = prompt
    {
        rows.push(format!("Input: {prompt}"));
    }
    (summary, nonempty_detail(rows.join("\n")))
}

fn collab_action_label(action: &str, status: &str, header: bool) -> &'static str {
    match (action, status, header) {
        ("spawnAgent", "inProgress", _) => "Creating",
        ("spawnAgent", "failed", true) => "Failed to create",
        ("spawnAgent", "failed", false) => "Failed creating",
        ("spawnAgent", _, _) => "Created",
        ("sendInput", "inProgress", _) => "Messaging",
        ("sendInput", "failed", true) => "Failed to message",
        ("sendInput", "failed", false) => "Failed messaging",
        ("sendInput", _, true) => "Messaged",
        ("sendInput", _, false) => "Messaged",
        ("resumeAgent", "inProgress", _) => "Resuming",
        ("resumeAgent", "failed", true) => "Failed to resume",
        ("resumeAgent", "failed", false) => "Failed resuming",
        ("resumeAgent", _, _) => "Resumed",
        ("closeAgent", "inProgress", _) => "Closing",
        ("closeAgent", "failed", true) => "Failed to close",
        ("closeAgent", "failed", false) => "Failed closing",
        ("closeAgent", _, _) => "Closed",
        (_, "inProgress", _) => "Working with",
        (_, "failed", _) => "Failed",
        _ => "Updated",
    }
}

fn collab_agent_state_suffix(item: &Value, thread_id: &str) -> String {
    let Some(state) = item
        .get("agentsStates")
        .and_then(Value::as_object)
        .and_then(|states| states.get(thread_id))
    else {
        return String::new();
    };
    let status = string_field(state, "status")
        .map(|status| match status.as_str() {
            "pendingInit" => "pending init",
            "running" => "running",
            "interrupted" => "interrupted",
            "shutdown" => "shutdown",
            "completed" => "completed",
            "errored" => "errored",
            "notFound" => "not found",
            _ => "unknown",
        })
        .unwrap_or("unknown");
    string_field(state, "message")
        .filter(|message| !message.trim().is_empty())
        .map_or_else(
            || format!(" ({status})"),
            |message| format!(" ({status}: {})", bounded(message, 512)),
        )
}

fn stable_agent_display(thread_id: &str) -> String {
    let prefix = thread_id.trim().chars().take(8).collect::<String>();
    if prefix.is_empty() {
        "Agent".to_owned()
    } else {
        format!("@agent-{prefix}")
    }
}

fn subagent_activity(item: &Value) -> (String, Option<String>) {
    let thread_id = string_field(item, "agentThreadId").unwrap_or_default();
    let display_name = string_field(item, "agentPath")
        .and_then(|path| subagent_display_name(&path))
        .unwrap_or_else(|| stable_agent_display(&thread_id));
    let text = match string_field(item, "kind").as_deref() {
        Some("started") => format!("{display_name} started working"),
        Some("interacted") => format!("{display_name} updated"),
        Some("interrupted") => format!("{display_name} interrupted"),
        _ => format!("{display_name} updated"),
    };
    (text, None)
}

fn subagent_display_name(agent_path: &str) -> Option<String> {
    let segment = agent_path
        .split('/')
        .map(str::trim)
        .rfind(|segment| !segment.is_empty() && *segment != "root")?;
    let normalized = segment
        .chars()
        .map(|character| {
            if matches!(character, '_' | '-') {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let mut characters = normalized.chars();
    let first = characters.next()?;
    Some(format!(
        "{}{}",
        first.to_uppercase(),
        characters.collect::<String>()
    ))
}

fn image_generation_activity(item: &Value, completed: bool) -> (String, Option<String>) {
    let mut details = Vec::new();
    if let Some(prompt) =
        string_field(item, "revisedPrompt").filter(|prompt| !prompt.trim().is_empty())
    {
        details.push(format!("Prompt: {}", bounded(prompt, 8 * 1024)));
    }
    if let Some(path) = string_field(item, "savedPath").filter(|path| !path.trim().is_empty()) {
        details.push(format!("Saved to {}", bounded(path, 4 * 1024)));
    }
    (
        if completed {
            "Generated image".to_owned()
        } else {
            "Generating image...".to_owned()
        },
        nonempty_detail(details.join("\n")),
    )
}

fn map_memory_citations(item: &Value) -> Vec<TimelineCitation> {
    item.get("memoryCitation")
        .and_then(|citation| citation.get("entries"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(MAX_MEMORY_CITATIONS)
        .filter_map(|entry| {
            let path = string_field(entry, "path")?;
            if path.trim().is_empty() {
                return None;
            }
            let line_start = positive_u32_field(entry, "lineStart").unwrap_or(1);
            let line_end = positive_u32_field(entry, "lineEnd")
                .unwrap_or(line_start)
                .max(line_start);
            Some(TimelineCitation {
                path: bounded(path, MAX_CITATION_FIELD_BYTES),
                line_start,
                line_end,
                note: bounded(
                    string_field(entry, "note").unwrap_or_default(),
                    MAX_CITATION_FIELD_BYTES,
                ),
            })
        })
        .collect()
}

fn web_search_activity(item: &Value) -> (String, Option<String>) {
    let action = item.get("action");
    let mut query = string_field(item, "query").unwrap_or_default();
    let mut detail = Vec::new();
    if let Some(action) = action {
        match string_field(action, "type").as_deref() {
            Some("search") => {
                let queries = action
                    .get("queries")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .take(16)
                    .map(|value| bounded(value.to_owned(), MAX_SOURCE_TITLE_BYTES))
                    .collect::<Vec<_>>();
                if query.trim().is_empty() {
                    query = string_field(action, "query")
                        .or_else(|| queries.first().cloned())
                        .unwrap_or_default();
                }
                if queries.len() > 1 {
                    detail.push(format!("Queries: {}", queries.join(", ")));
                }
            }
            Some("openPage") => detail.push("Opened a web page".to_owned()),
            Some("findInPage") => {
                let pattern = string_field(action, "pattern").unwrap_or_default();
                detail.push(if pattern.trim().is_empty() {
                    "Searched within a web page".to_owned()
                } else {
                    format!(
                        "Searched within a web page for {}",
                        bounded(pattern, MAX_SOURCE_TITLE_BYTES)
                    )
                });
            }
            _ => {}
        }
    }
    (query, nonempty_detail(detail.join("\n")))
}

fn map_web_search_sources(item: &Value) -> Vec<TimelineSource> {
    let mut sources = Vec::new();
    let mut seen = HashSet::new();
    if let Some(results) = item.get("results").and_then(Value::as_array) {
        for result in results.iter().take(MAX_WEB_SEARCH_SOURCES) {
            let Some(url) = web_result_field(result, &["url", "sourceUrl", "source_url", "link"])
                .and_then(bounded_external_url)
            else {
                continue;
            };
            if !seen.insert(url.clone()) {
                continue;
            }
            let title =
                web_result_field(result, &["title", "name", "displayTitle", "display_title"])
                    .filter(|title| !title.trim().is_empty())
                    .map(|title| bounded(title, MAX_SOURCE_TITLE_BYTES))
                    .unwrap_or_else(|| "Web source".to_owned());
            sources.push(TimelineSource { title, url });
        }
    }
    if sources.len() < MAX_WEB_SEARCH_SOURCES
        && let Some(url) = item
            .get("action")
            .and_then(|action| string_field(action, "url"))
            .and_then(bounded_external_url)
        && seen.insert(url.clone())
    {
        sources.push(TimelineSource {
            title: "Web page".to_owned(),
            url,
        });
    }
    sources
}

fn web_result_field(result: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| string_field(result, key))
        .or_else(|| {
            result
                .get("item")
                .and_then(|item| keys.iter().find_map(|key| string_field(item, key)))
        })
}

fn bounded_external_url(value: String) -> Option<String> {
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    (value.len() <= MAX_SOURCE_URL_BYTES
        && (lower.starts_with("https://") || lower.starts_with("http://")))
    .then(|| value.to_owned())
}

fn positive_u32_field(value: &Value, key: &str) -> Option<u32> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
}

fn command_activity(item: &Value, completed: bool) -> (String, Option<String>) {
    let command = string_field(item, "command").unwrap_or_default();
    let output = string_field(item, "aggregatedOutput").unwrap_or_default();
    let actions = item
        .get("commandActions")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let exploration = !actions.is_empty()
        && actions.iter().all(|action| {
            matches!(
                string_field(action, "type").as_deref(),
                Some("read" | "search" | "listFiles")
            )
        });

    if exploration {
        let read_count = actions
            .iter()
            .filter(|action| string_field(action, "type").as_deref() == Some("read"))
            .count();
        let search_count = actions
            .iter()
            .filter(|action| string_field(action, "type").as_deref() == Some("search"))
            .count();
        let list_count = actions
            .iter()
            .filter(|action| string_field(action, "type").as_deref() == Some("listFiles"))
            .count();
        let counts = [
            activity_count(read_count, "file", "files"),
            activity_count(search_count, "search", "searches"),
            activity_count(list_count, "list", "lists"),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        let verb = if completed { "Explored" } else { "Exploring" };
        let summary = if counts.is_empty() {
            verb.to_owned()
        } else {
            format!("{verb} {}", counts.join(", "))
        };
        let mut details = actions
            .iter()
            .filter_map(command_action_detail)
            .collect::<Vec<_>>();
        append_command_output(&mut details, &command, &output);
        details.push(command_execution_status(item, completed));
        return (summary, nonempty_detail(details.join("\n")));
    }

    let display_command = actions
        .iter()
        .rev()
        .filter_map(|action| string_field(action, "command"))
        .find(|command| !is_shell_wrapper(command))
        .or_else(|| (!is_shell_wrapper(&command)).then(|| command.clone()))
        .unwrap_or_default();
    let verb = if completed { "Ran" } else { "Running" };
    let summary = if display_command.trim().is_empty() {
        format!("{verb} command")
    } else {
        format!("{verb} {}", display_command.trim())
    };
    let mut details = Vec::new();
    append_command_output(&mut details, &command, &output);
    details.push(command_execution_status(item, completed));
    (summary, nonempty_detail(details.join("\n")))
}

fn command_action_detail(action: &Value) -> Option<String> {
    match string_field(action, "type").as_deref()? {
        "read" => string_field(action, "path")
            .or_else(|| string_field(action, "name"))
            .map(|target| format!("Read {target}")),
        "search" => {
            let query = string_field(action, "query").filter(|query| !query.trim().is_empty());
            let path = string_field(action, "path").filter(|path| !path.trim().is_empty());
            Some(match (query, path) {
                (Some(query), Some(path)) => format!("Searched for {query} in {path}"),
                (Some(query), None) => format!("Searched for {query}"),
                (None, Some(path)) => format!("Searched files in {path}"),
                (None, None) => "Searched for files".to_owned(),
            })
        }
        "listFiles" => Some(
            string_field(action, "path")
                .filter(|path| !path.trim().is_empty())
                .map_or_else(
                    || "Listed files".to_owned(),
                    |path| format!("Listed files in {path}"),
                ),
        ),
        _ => None,
    }
}

fn append_command_output(details: &mut Vec<String>, command: &str, output: &str) {
    if !command.trim().is_empty() {
        details.push(format!("$ {}", command.trim()));
    }
    if !output.trim().is_empty() {
        details.push(output.to_owned());
    }
}

fn command_execution_status(item: &Value, completed: bool) -> String {
    match string_field(item, "status").as_deref() {
        Some("declined") => "Declined".to_owned(),
        Some("failed") => item.get("exitCode").and_then(Value::as_i64).map_or_else(
            || "Failed".to_owned(),
            |exit_code| format!("Failed with exit code {exit_code}"),
        ),
        Some("inProgress") => "Running".to_owned(),
        _ => item.get("exitCode").and_then(Value::as_i64).map_or_else(
            || {
                if completed {
                    "Success".to_owned()
                } else {
                    "Running".to_owned()
                }
            },
            |exit_code| {
                if exit_code == 0 {
                    "Success".to_owned()
                } else {
                    format!("Failed with exit code {exit_code}")
                }
            },
        ),
    }
}

fn activity_count(count: usize, singular: &str, plural: &str) -> Option<String> {
    match count {
        0 => None,
        1 => Some(format!("1 {singular}")),
        _ => Some(format!("{count} {plural}")),
    }
}

fn is_shell_wrapper(command: &str) -> bool {
    let Some(program) = command.split_whitespace().next() else {
        return false;
    };
    let program = program
        .trim_matches(['"', '\''])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(program)
        .to_ascii_lowercase();
    matches!(
        program.as_str(),
        "bash"
            | "cmd"
            | "cmd.exe"
            | "fish"
            | "powershell"
            | "powershell.exe"
            | "pwsh"
            | "pwsh.exe"
            | "sh"
            | "zsh"
    )
}

fn file_change_activity(item: &Value) -> (String, Option<String>) {
    let changes = item
        .get("changes")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let mut summaries = Vec::with_capacity(changes.len());
    let mut details = Vec::with_capacity(changes.len());
    let mut verbs = Vec::with_capacity(changes.len());

    for change in changes {
        let Some(path) = string_field(change, "path").or_else(|| string_field(change, "filePath"))
        else {
            continue;
        };
        let diff = string_field(change, "diff").unwrap_or_default();
        let (additions, deletions) = diff_line_counts(&diff);
        let kind = change
            .get("kind")
            .and_then(|kind| string_field(kind, "type"))
            .or_else(|| string_field(change, "kind"))
            .unwrap_or_else(|| "update".to_owned());
        let verb = match kind.as_str() {
            "add" => "Created",
            "delete" => "Deleted",
            _ => "Edited",
        };
        let filename = path
            .rsplit(['/', '\\'])
            .next()
            .filter(|filename| !filename.is_empty())
            .unwrap_or(path.as_str());
        summaries.push(format!("{verb} {filename} +{additions} -{deletions}"));
        verbs.push(verb);
        if diff.trim().is_empty() {
            details.push(path);
        } else {
            details.push(format!("{path}\n{diff}"));
        }
    }

    let summary = match summaries.as_slice() {
        [] => "Edited files".to_owned(),
        [summary] => summary.clone(),
        summaries => {
            let verb = if verbs.iter().all(|verb| *verb == verbs[0]) {
                verbs[0]
            } else {
                "Changed"
            };
            format!("{verb} {} files", summaries.len())
        }
    };
    (summary, nonempty_detail(details.join("\n\n")))
}

fn diff_line_counts(diff: &str) -> (usize, usize) {
    let mut additions = 0;
    let mut deletions = 0;
    for line in diff.lines() {
        if line.starts_with('+') && !line.starts_with("+++") {
            additions += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            deletions += 1;
        }
    }
    (additions, deletions)
}

fn nonempty_detail(detail: String) -> Option<String> {
    (!detail.trim().is_empty()).then_some(detail)
}

fn map_editable_message_attachments(item: &Value) -> (Vec<ComposerAttachment>, bool) {
    let Some(content) = item.get("content").and_then(Value::as_array) else {
        return (Vec::new(), true);
    };
    let mut attachments = Vec::new();
    let mut supported = content.len() <= MAX_COMPOSER_ATTACHMENTS.saturating_add(1);
    for entry in content
        .iter()
        .take(MAX_COMPOSER_ATTACHMENTS.saturating_add(1))
    {
        let Some(input_type) = string_field(entry, "type") else {
            supported = false;
            continue;
        };
        let kind = match input_type.as_str() {
            "text" => continue,
            "localImage" => ComposerAttachmentKind::LocalImage,
            "mention" => ComposerAttachmentKind::Mention,
            "skill" => ComposerAttachmentKind::Skill,
            _ => {
                supported = false;
                continue;
            }
        };
        let Some(path) = string_field(entry, "path").map(PathBuf::from) else {
            supported = false;
            continue;
        };
        if !path.is_absolute() || attachments.len() >= MAX_COMPOSER_ATTACHMENTS {
            supported = false;
            continue;
        }
        let name = string_field(entry, "name")
            .or_else(|| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| path.display().to_string());
        attachments.push(ComposerAttachment {
            path,
            name: bounded(name, MAX_ATTACHMENT_LABEL_BYTES),
            kind,
        });
    }
    (attachments, supported)
}

fn notification_kind(method: &str) -> TimelineKind {
    match method {
        "item/agentMessage/delta" => TimelineKind::Agent,
        "item/plan/delta" => TimelineKind::Plan,
        "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => TimelineKind::Reasoning,
        "item/commandExecution/outputDelta" => TimelineKind::Command,
        "item/fileChange/outputDelta" => TimelineKind::FileChange,
        _ => TimelineKind::Notice,
    }
}

fn map_app_server_approval(
    request_id: String,
    method: &str,
    params: Value,
    id: Value,
) -> Result<(ApprovalRequest, PendingApproval), String> {
    match method {
        "item/commandExecution/requestApproval" => {
            let params = serde_json::from_value::<CommandExecutionRequestApprovalParams>(params)
                .map_err(|_| "command approval parameters did not match the stable schema")?;
            validate_command_approval(&params)?;

            let command = command_approval_display(&params)?;
            let network_approval_context = params
                .network_approval_context
                .as_ref()
                .map(core_network_approval_context);
            let proposed_network_policy_amendment = params
                .proposed_network_policy_amendments
                .as_deref()
                .unwrap_or_default()
                .iter()
                .find(|amendment| amendment.action == NetworkPolicyRuleAction::Allow)
                .map(core_network_policy_amendment);
            let proposed_execpolicy_amendment = params.proposed_execpolicy_amendment.clone();
            let detail = if command.trim().is_empty() {
                params
                    .reason
                    .clone()
                    .unwrap_or_else(|| "Codex wants to run a command.".to_owned())
            } else {
                command.clone()
            };
            let request = ApprovalRequest {
                request_id,
                task_id: params.thread_id,
                turn_id: Some(params.turn_id),
                kind: ApprovalKind::Command,
                title: "Allow ChatGPT to run this command?".to_owned(),
                detail,
                context: ApprovalContext::Command(CommandApprovalContext {
                    item_id: params.item_id,
                    command,
                    reason: params.reason,
                    network_approval_context,
                    proposed_execpolicy_amendment: proposed_execpolicy_amendment.clone(),
                    proposed_network_policy_amendment: proposed_network_policy_amendment.clone(),
                }),
            };
            Ok((
                request,
                PendingApproval::Command {
                    id,
                    proposed_execpolicy_amendment,
                    proposed_network_policy_amendment,
                },
            ))
        }
        "item/fileChange/requestApproval" => {
            let params = serde_json::from_value::<FileChangeRequestApprovalParams>(params)
                .map_err(|_| "file-change approval parameters did not match the stable schema")?;
            validate_approval_identity(
                &params.thread_id,
                &params.turn_id,
                &params.item_id,
                params.reason.as_deref(),
            )?;
            validate_optional_approval_field(params.grant_root.as_deref(), "grantRoot")?;
            let detail = params
                .reason
                .clone()
                .or_else(|| params.grant_root.clone())
                .unwrap_or_else(|| "Codex wants to change files.".to_owned());
            Ok((
                ApprovalRequest {
                    request_id,
                    task_id: params.thread_id,
                    turn_id: Some(params.turn_id),
                    kind: ApprovalKind::FileChange,
                    title: "Allow ChatGPT to edit the following files?".to_owned(),
                    detail,
                    context: ApprovalContext::FileChange(FileChangeApprovalContext {
                        item_id: params.item_id,
                        grant_root: params.grant_root,
                        reason: params.reason,
                    }),
                },
                PendingApproval::FileChange { id },
            ))
        }
        "item/permissions/requestApproval" => {
            let params = serde_json::from_value::<PermissionsRequestApprovalParams>(params)
                .map_err(|_| "permission approval parameters did not match the stable schema")?;
            validate_approval_identity(
                &params.thread_id,
                &params.turn_id,
                &params.item_id,
                params.reason.as_deref(),
            )?;
            validate_approval_field(&params.cwd, "cwd", MAX_APPROVAL_PATH_BYTES)?;
            validate_permission_profile(&params.permissions)?;
            let details = permission_request_details(&params.permissions);
            let title = permission_request_title(&details);
            let detail = params.reason.clone().unwrap_or_else(|| params.cwd.clone());
            Ok((
                ApprovalRequest {
                    request_id,
                    task_id: params.thread_id,
                    turn_id: Some(params.turn_id),
                    kind: ApprovalKind::Permissions,
                    title,
                    detail,
                    context: ApprovalContext::Permissions(PermissionsApprovalContext {
                        item_id: params.item_id,
                        cwd: params.cwd,
                        reason: params.reason,
                        details,
                    }),
                },
                PendingApproval::Permissions {
                    id,
                    permissions: params.permissions,
                },
            ))
        }
        _ => Err("unsupported approval method".to_owned()),
    }
}

fn validate_command_approval(params: &CommandExecutionRequestApprovalParams) -> Result<(), String> {
    validate_approval_identity(
        &params.thread_id,
        &params.turn_id,
        &params.item_id,
        params.reason.as_deref(),
    )?;
    validate_optional_approval_field(params.command.as_deref(), "command")?;
    validate_optional_approval_field(params.cwd.as_deref(), "cwd")?;
    validate_optional_approval_field(params.environment_id.as_deref(), "environmentId")?;
    validate_optional_approval_field(params.approval_id.as_deref(), "approvalId")?;
    if let Some(actions) = params.command_actions.as_deref() {
        if actions.len() > MAX_APPROVAL_LIST_ITEMS {
            return Err("commandActions exceeded the bounded item limit".to_owned());
        }
        for action in actions {
            validate_command_action(action)?;
        }
    }
    validate_approval_string_list(
        params.proposed_execpolicy_amendment.as_deref(),
        "proposedExecpolicyAmendment",
    )?;
    if let Some(context) = params.network_approval_context.as_ref() {
        validate_approval_field(
            &context.host,
            "networkApprovalContext.host",
            MAX_APPROVAL_PATH_BYTES,
        )?;
    }
    if let Some(amendments) = params.proposed_network_policy_amendments.as_deref() {
        if amendments.len() > MAX_APPROVAL_LIST_ITEMS {
            return Err(
                "proposedNetworkPolicyAmendments exceeded the bounded item limit".to_owned(),
            );
        }
        for amendment in amendments {
            validate_approval_field(
                &amendment.host,
                "proposedNetworkPolicyAmendments.host",
                MAX_APPROVAL_PATH_BYTES,
            )?;
        }
    }
    Ok(())
}

fn validate_command_action(action: &CommandAction) -> Result<(), String> {
    validate_approval_field(
        &action.command,
        "commandActions.command",
        MAX_APPROVAL_FIELD_BYTES,
    )?;
    validate_optional_approval_field(action.name.as_deref(), "commandActions.name")?;
    validate_optional_approval_field(action.path.as_deref(), "commandActions.path")?;
    validate_optional_approval_field(action.query.as_deref(), "commandActions.query")
}

fn validate_approval_identity(
    thread_id: &str,
    turn_id: &str,
    item_id: &str,
    reason: Option<&str>,
) -> Result<(), String> {
    validate_approval_field(thread_id, "threadId", MAX_APPROVAL_PATH_BYTES)?;
    validate_approval_field(turn_id, "turnId", MAX_APPROVAL_PATH_BYTES)?;
    validate_approval_field(item_id, "itemId", MAX_APPROVAL_PATH_BYTES)?;
    validate_optional_approval_field(reason, "reason")
}

fn validate_optional_approval_field(value: Option<&str>, field: &str) -> Result<(), String> {
    value.map_or(Ok(()), |value| {
        validate_approval_field(value, field, MAX_APPROVAL_FIELD_BYTES)
    })
}

fn validate_approval_field(value: &str, field: &str, limit: usize) -> Result<(), String> {
    if value.len() <= limit {
        Ok(())
    } else {
        Err(format!("{field} exceeded the bounded byte limit"))
    }
}

fn validate_approval_string_list(values: Option<&[String]>, field: &str) -> Result<(), String> {
    let Some(values) = values else {
        return Ok(());
    };
    if values.len() > MAX_APPROVAL_LIST_ITEMS {
        return Err(format!("{field} exceeded the bounded item limit"));
    }
    let mut total = 0_usize;
    for value in values {
        validate_approval_field(value, field, MAX_APPROVAL_FIELD_BYTES)?;
        total = total
            .checked_add(value.len())
            .and_then(|total| total.checked_add(1))
            .ok_or_else(|| format!("{field} exceeded the bounded byte limit"))?;
        if total > MAX_APPROVAL_FIELD_BYTES {
            return Err(format!("{field} exceeded the bounded byte limit"));
        }
    }
    Ok(())
}

fn command_approval_display(
    params: &CommandExecutionRequestApprovalParams,
) -> Result<String, String> {
    let commands = params
        .command_actions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|action| action.command.as_str())
        .collect::<Vec<_>>();
    if !commands.is_empty() {
        return join_bounded_approval_fields(&commands, " && ", "commandActions");
    }
    if let Some(command) = params.command.as_ref() {
        return Ok(command.clone());
    }
    if let Some(amendment) = params.proposed_execpolicy_amendment.as_deref()
        && !amendment.is_empty()
    {
        let quoted = amendment
            .iter()
            .map(|part| quote_execpolicy_part(part))
            .collect::<Vec<_>>();
        let quoted = quoted.iter().map(String::as_str).collect::<Vec<_>>();
        return join_bounded_approval_fields(&quoted, " ", "proposedExecpolicyAmendment");
    }
    Ok(String::new())
}

fn join_bounded_approval_fields(
    values: &[&str],
    separator: &str,
    field: &str,
) -> Result<String, String> {
    let total = values
        .iter()
        .try_fold(0_usize, |total, value| total.checked_add(value.len()))
        .and_then(|total| {
            total.checked_add(
                separator
                    .len()
                    .saturating_mul(values.len().saturating_sub(1)),
            )
        })
        .ok_or_else(|| format!("{field} exceeded the bounded byte limit"))?;
    if total > MAX_APPROVAL_FIELD_BYTES {
        return Err(format!("{field} exceeded the bounded byte limit"));
    }
    Ok(values.join(separator))
}

fn quote_execpolicy_part(value: &str) -> String {
    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "_@+=:,./-".contains(character))
    {
        return value.to_owned();
    }
    if !value.contains(['`', '$', '\\', '!']) && !value.contains('"') {
        return format!("\"{value}\"");
    }
    if value.is_empty() {
        return "''".to_owned();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn core_network_approval_context(
    context: &codex_protocol::NetworkApprovalContext,
) -> CoreNetworkApprovalContext {
    CoreNetworkApprovalContext {
        host: context.host.clone(),
        protocol: match context.protocol {
            NetworkApprovalProtocol::Http => CoreNetworkApprovalProtocol::Http,
            NetworkApprovalProtocol::Https => CoreNetworkApprovalProtocol::Https,
            NetworkApprovalProtocol::Socks5Tcp => CoreNetworkApprovalProtocol::Socks5Tcp,
            NetworkApprovalProtocol::Socks5Udp => CoreNetworkApprovalProtocol::Socks5Udp,
        },
    }
}

fn core_network_policy_amendment(amendment: &NetworkPolicyAmendment) -> CoreNetworkPolicyAmendment {
    CoreNetworkPolicyAmendment {
        action: match amendment.action {
            NetworkPolicyRuleAction::Allow => CoreNetworkPolicyAction::Allow,
            NetworkPolicyRuleAction::Deny => CoreNetworkPolicyAction::Deny,
        },
        host: amendment.host.clone(),
    }
}

fn protocol_network_policy_amendment(
    amendment: CoreNetworkPolicyAmendment,
) -> NetworkPolicyAmendment {
    NetworkPolicyAmendment {
        action: match amendment.action {
            CoreNetworkPolicyAction::Allow => NetworkPolicyRuleAction::Allow,
            CoreNetworkPolicyAction::Deny => NetworkPolicyRuleAction::Deny,
        },
        host: amendment.host,
    }
}

fn validate_permission_profile(profile: &PermissionProfile) -> Result<(), String> {
    let Some(file_system) = profile.file_system.as_ref() else {
        return Ok(());
    };
    validate_permission_path_list(file_system.read.as_deref(), "permissions.fileSystem.read")?;
    validate_permission_path_list(file_system.write.as_deref(), "permissions.fileSystem.write")?;
    if file_system.glob_scan_max_depth == Some(0) {
        return Err("permissions.fileSystem.globScanMaxDepth must be positive".to_owned());
    }
    if let Some(entries) = file_system.entries.as_deref() {
        if entries.len() > MAX_APPROVAL_LIST_ITEMS {
            return Err(
                "permissions.fileSystem.entries exceeded the bounded item limit".to_owned(),
            );
        }
        for entry in entries {
            validate_permission_path(&entry.path)?;
        }
    }
    Ok(())
}

fn validate_permission_path_list(values: Option<&[String]>, field: &str) -> Result<(), String> {
    let Some(values) = values else {
        return Ok(());
    };
    if values.len() > MAX_APPROVAL_LIST_ITEMS {
        return Err(format!("{field} exceeded the bounded item limit"));
    }
    for value in values {
        validate_approval_field(value, field, MAX_APPROVAL_PATH_BYTES)?;
    }
    Ok(())
}

fn validate_permission_path(path: &FileSystemPath) -> Result<(), String> {
    match path {
        FileSystemPath::Path { path } => validate_approval_field(
            path,
            "permissions.fileSystem.entries.path",
            MAX_APPROVAL_PATH_BYTES,
        ),
        FileSystemPath::GlobPattern { pattern } => validate_approval_field(
            pattern,
            "permissions.fileSystem.entries.pattern",
            MAX_APPROVAL_PATH_BYTES,
        ),
        FileSystemPath::Special { value } => match value {
            FileSystemSpecialPath::ProjectRoots { subpath } => validate_optional_approval_path(
                subpath.as_deref(),
                "permissions.fileSystem.entries.subpath",
            ),
            FileSystemSpecialPath::Unknown { path, subpath } => {
                validate_approval_field(
                    path,
                    "permissions.fileSystem.entries.path",
                    MAX_APPROVAL_PATH_BYTES,
                )?;
                validate_optional_approval_path(
                    subpath.as_deref(),
                    "permissions.fileSystem.entries.subpath",
                )
            }
            FileSystemSpecialPath::Root
            | FileSystemSpecialPath::Minimal
            | FileSystemSpecialPath::Tmpdir
            | FileSystemSpecialPath::SlashTmp => Ok(()),
        },
    }
}

fn validate_optional_approval_path(value: Option<&str>, field: &str) -> Result<(), String> {
    value.map_or(Ok(()), |value| {
        validate_approval_field(value, field, MAX_APPROVAL_PATH_BYTES)
    })
}

fn permission_request_details(profile: &PermissionProfile) -> Vec<PermissionRequestDetail> {
    let mut details = Vec::new();
    if profile.network.is_some() {
        details.push(PermissionRequestDetail::Network);
    }
    let Some(entries) = profile
        .file_system
        .as_ref()
        .and_then(|file_system| file_system.entries.as_deref())
    else {
        return details;
    };

    let mut read = Vec::new();
    let mut write = Vec::new();
    let mut read_seen = HashSet::new();
    let mut write_seen = HashSet::new();
    for entry in entries {
        let path = permission_path_display(&entry.path);
        match entry.access {
            FileSystemAccessMode::Read if read_seen.insert(path.clone()) => read.push(path),
            FileSystemAccessMode::Write if write_seen.insert(path.clone()) => write.push(path),
            FileSystemAccessMode::Read
            | FileSystemAccessMode::Write
            | FileSystemAccessMode::Deny => {}
        }
    }
    let read_write = read
        .iter()
        .filter(|path| write_seen.contains(path.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let read_only = read
        .into_iter()
        .filter(|path| !write_seen.contains(path.as_str()))
        .collect::<Vec<_>>();
    let write_only = write
        .into_iter()
        .filter(|path| !read_seen.contains(path.as_str()))
        .collect::<Vec<_>>();
    if !read_write.is_empty() {
        details.push(PermissionRequestDetail::FileSystem {
            access: PermissionFileSystemAccess::ReadWrite,
            paths: read_write,
        });
    }
    if !read_only.is_empty() {
        details.push(PermissionRequestDetail::FileSystem {
            access: PermissionFileSystemAccess::Read,
            paths: read_only,
        });
    }
    if !write_only.is_empty() {
        details.push(PermissionRequestDetail::FileSystem {
            access: PermissionFileSystemAccess::Write,
            paths: write_only,
        });
    }
    details
}

fn permission_path_display(path: &FileSystemPath) -> String {
    match path {
        FileSystemPath::Path { path } => path.clone(),
        FileSystemPath::GlobPattern { pattern } => pattern.clone(),
        FileSystemPath::Special { value } => match value {
            FileSystemSpecialPath::Root => "/".to_owned(),
            FileSystemSpecialPath::Minimal => ":minimal".to_owned(),
            FileSystemSpecialPath::ProjectRoots { subpath: None } => ":project_roots".to_owned(),
            FileSystemSpecialPath::ProjectRoots {
                subpath: Some(subpath),
            } => format!(":project_roots/{subpath}"),
            FileSystemSpecialPath::Tmpdir => ":tmpdir".to_owned(),
            FileSystemSpecialPath::SlashTmp => "/tmp".to_owned(),
            FileSystemSpecialPath::Unknown {
                path,
                subpath: None,
            } => path.clone(),
            FileSystemSpecialPath::Unknown {
                path,
                subpath: Some(subpath),
            } => format!("{path}/{subpath}"),
        },
    }
}

fn permission_request_title(details: &[PermissionRequestDetail]) -> String {
    if details.len() == 1 {
        return permission_detail_title(&details[0]);
    }
    let actions = details
        .iter()
        .map(permission_detail_action)
        .collect::<Vec<_>>();
    if actions.is_empty() {
        "Permissions".to_owned()
    } else {
        format!("Allow ChatGPT to {}?", english_conjunction(&actions))
    }
}

fn permission_detail_title(detail: &PermissionRequestDetail) -> String {
    match detail {
        PermissionRequestDetail::Network => "Allow ChatGPT to connect to the internet?".to_owned(),
        PermissionRequestDetail::FileSystem { access, paths } => {
            let paths = english_conjunction(paths);
            match access {
                PermissionFileSystemAccess::Read => {
                    format!("Allow ChatGPT to view the contents of {paths}?")
                }
                PermissionFileSystemAccess::Write => {
                    format!("Allow ChatGPT to edit the contents of {paths}?")
                }
                PermissionFileSystemAccess::ReadWrite => {
                    format!("Allow ChatGPT to view and edit the contents of {paths}?")
                }
            }
        }
    }
}

fn permission_detail_action(detail: &PermissionRequestDetail) -> String {
    match detail {
        PermissionRequestDetail::Network => "connect to the internet".to_owned(),
        PermissionRequestDetail::FileSystem { access, paths } => {
            let paths = english_conjunction(paths);
            match access {
                PermissionFileSystemAccess::Read => format!("view the contents of {paths}"),
                PermissionFileSystemAccess::Write => format!("edit the contents of {paths}"),
                PermissionFileSystemAccess::ReadWrite => {
                    format!("view and edit the contents of {paths}")
                }
            }
        }
    }
}

fn english_conjunction(values: &[String]) -> String {
    match values {
        [] => String::new(),
        [value] => value.clone(),
        [first, second] => format!("{first} and {second}"),
        values => {
            let last = &values[values.len() - 1];
            let rest = &values[..values.len() - 1];
            format!("{}, and {last}", rest.join(", "))
        }
    }
}

fn request_key(id: &Value) -> String {
    bounded(id.to_string(), 512)
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn string_array(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

const fn map_remote_control_status(
    status: RemoteControlConnectionStatus,
) -> RemoteControlRuntimeStatus {
    match status {
        RemoteControlConnectionStatus::Disabled => RemoteControlRuntimeStatus::Disabled,
        RemoteControlConnectionStatus::Connecting => RemoteControlRuntimeStatus::Connecting,
        RemoteControlConnectionStatus::Connected => RemoteControlRuntimeStatus::Connected,
        RemoteControlConnectionStatus::Errored => RemoteControlRuntimeStatus::Errored,
    }
}

fn bounded_remote_identifier(value: String, limit: usize) -> Option<String> {
    (!value.is_empty() && value.len() <= limit && !value.chars().any(char::is_control))
        .then_some(value)
}

fn map_remote_control_snapshot(
    status: RemoteControlConnectionStatus,
    value: Option<String>,
) -> Result<(RemoteControlRuntimeStatus, Option<String>), ()> {
    let status = map_remote_control_status(status);
    let environment_id = value
        .map(|value| bounded_remote_identifier(value, MAX_REMOTE_ENVIRONMENT_ID_BYTES).ok_or(()))
        .transpose()?;
    match (status, environment_id.is_some()) {
        (RemoteControlRuntimeStatus::Connected, true)
        | (RemoteControlRuntimeStatus::Disabled, false)
        | (RemoteControlRuntimeStatus::Connecting, _)
        | (RemoteControlRuntimeStatus::Errored, _) => {}
        _ => return Err(()),
    }
    Ok((status, environment_id))
}

fn map_remote_pairing(
    pairing_code: String,
    manual_pairing_code: Option<String>,
    environment_id: String,
    expires_at: i64,
) -> Result<RemotePairing, ()> {
    let pairing_code =
        bounded_remote_identifier(pairing_code, MAX_REMOTE_PAIRING_CODE_BYTES).ok_or(())?;
    let manual_pairing_code = manual_pairing_code
        .map(|code| bounded_remote_identifier(code, MAX_REMOTE_PAIRING_CODE_BYTES).ok_or(()))
        .transpose()?;
    let environment_id =
        bounded_remote_identifier(environment_id, MAX_REMOTE_ENVIRONMENT_ID_BYTES).ok_or(())?;
    Ok(RemotePairing {
        pairing_code,
        manual_pairing_code,
        environment_id,
        expires_at,
    })
}

fn remote_pairing_status_params(pairing: &RemotePairing) -> RemoteControlPairingStatusParams {
    // `pairingCode` is always returned by pairing/start. The core state deliberately
    // does not retain which UI path started pairing, so it is the stable canonical
    // choice; `manualPairingCode` remains a display-only fallback.
    RemoteControlPairingStatusParams {
        pairing_code: Some(pairing.pairing_code.clone()),
        manual_pairing_code: None,
    }
}

fn map_remote_devices_page(
    devices: Vec<RemoteControlClient>,
    next_cursor: Option<String>,
    limit: u32,
) -> Result<(Vec<RemoteDevice>, Option<String>), ()> {
    let page_limit = usize::try_from(limit).map_err(|_| ())?;
    if page_limit == 0 || devices.len() > page_limit {
        return Err(());
    }
    let mut seen = HashSet::new();
    let mut mapped = Vec::with_capacity(devices.len());
    for device in devices {
        let client_id =
            bounded_remote_identifier(device.client_id, MAX_REMOTE_DEVICE_ID_BYTES).ok_or(())?;
        let display_name = device
            .display_name
            .map(|name| {
                let name = name.trim().to_owned();
                (name.len() <= MAX_REMOTE_DEVICE_LABEL_BYTES && !name.chars().any(char::is_control))
                    .then_some(name)
                    .ok_or(())
            })
            .transpose()?
            .filter(|name| !name.is_empty());
        if seen.insert(client_id.clone()) {
            mapped.push(RemoteDevice {
                client_id,
                display_name,
            });
        }
    }
    let next_cursor = next_cursor
        .map(|cursor| bounded_remote_identifier(cursor, MAX_REMOTE_CURSOR_BYTES).ok_or(()))
        .transpose()?;
    Ok((mapped, next_cursor))
}

fn bounded(mut value: String, limit: usize) -> String {
    if value.len() <= limit {
        return value;
    }
    let boundary = value
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= limit)
        .last()
        .unwrap_or(0);
    value.truncate(boundary);
    value
}

fn push_bounded(output: &mut String, value: &str, limit: usize) {
    let remaining = limit.saturating_sub(output.len());
    if remaining == 0 {
        return;
    }
    output.push_str(&bounded(value.to_owned(), remaining));
}

fn emit(events: &Sender<Action>, action: Action) {
    let _ = events.send_timeout(action, UI_EVENT_TIMEOUT);
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::thread;
    use std::time::{Duration, Instant};

    use codex_core::{
        Action, AgentConfigScopeKind, AppearancePalette, AppearancePreferences, AppearanceTheme,
        AppearanceVariant, ApprovalContext, BrowserApprovalMode, BrowserDownloadPreferences,
        BrowserDownloadState, BrowserDownloadStatus, BrowserOriginElicitationDecision,
        BrowserPermissionResource, BrowserPermissionValue, BrowserPermissionsState,
        BrowserResourceElicitationDecision, BrowserSitePermission, ComposerAttachment,
        ComposerAttachmentKind, DiffMarkerStyle, FuzzyFileMatchType, GitPreferences, GitReviewMode,
        ImportItemType, KeyboardShortcutPreferences, MAX_MCP_SERVER_FIELD_BYTES,
        McpBrowserOriginElicitation, McpBrowserResourceElicitation, McpElicitation,
        McpElicitationContent, McpElicitationValue, McpFormElicitation, McpFormFieldKind,
        McpServerDraft, McpServerStartupFailureReason, McpServerStartupState, McpTransportKind,
        NetworkPolicyAction, OutputArtifactKind, PermissionFileSystemAccess,
        PermissionRequestDetail, Personality, PluginDirectoryTab, PrimaryWindowPlacement,
        PullRequestMergeMethod, ReducedMotionPreference, RemoteControlRuntimeStatus, RemotePairing,
        RetryableTurnSubmission, RetryableUserMessage, ReviewDelivery, TimelineItem, TimelineKind,
        TimelineSource, UserInputAnswer, UserInputAnswers,
    };
    use codex_platform::{AppServerEvent, ComputerApplication, ComputerKey};
    use codex_protocol::{
        AppInfo, AppToolSummary, ConfigReadResponse, ConnectorMetadata, FuzzyFileSearchMatchType,
        FuzzyFileSearchResult as ProtocolFuzzyFileResult,
        McpServerStatus as ProtocolMcpServerStatus, PluginListMarketplaceKind, RemoteControlClient,
        RemoteControlConnectionStatus, UserInput,
    };
    use crossbeam_channel::bounded;
    use serde_json::{Value, json};

    use super::{
        AppLogo, AppServerReconnectScheduler, BrowserPolicyTarget,
        COMPUTER_USE_USER_INPUT_STALE_MESSAGE, ComputerUseAccessibilityClient,
        ComputerUsePermission, GOAL_CONTINUATION_DELAY, GitRefreshDebouncer,
        GoalContinuationScheduler, MAX_ITEM_TEXT_BYTES, McpElicitationMapError, PendingApproval,
        PendingWorktreeRuntime, STABLE_OPT_OUT_NOTIFICATION_METHODS, TASK_SEARCH_DEBOUNCE,
        TRUSTED_ACCESS_FOR_CYBER_URL, TRUSTED_ACCESS_FOR_CYBER_WARNING, TaskSearchDebouncer,
        TerminalParserCallbacks, agent_configuration_snapshot, appearance_theme_key,
        bounded_remote_identifier, browser_origin_auto_decision,
        browser_origin_elicitation_response, browser_policy_target, browser_resource_auto_decision,
        browser_resource_elicitation_response, cancel_pending_worktree_runtime,
        combined_git_generation_prompt, combined_git_output_schema, commit_generation_prompt,
        commit_message_output_schema, composer_config_key, composer_inputs,
        computer_application_value, computer_tool_request_meta,
        computer_tool_requires_interruption_monitor, computer_use_allowed_app_ids,
        computer_use_allowed_app_ids_value, computer_use_app_authorized,
        computer_use_dynamic_tools, computer_window_argument, computer_window_schema,
        drag_coordinates, encode_appearance_preferences, encode_browser_download_preferences,
        encode_browser_permissions, encode_git_preferences, encode_keyboard_shortcut_preferences,
        encode_primary_window_placement, forbidden_computer_target_message, handle_notification,
        hook_state_config_value, index_app_logos, initialize_capabilities, is_hidden_timeline_item,
        linux_computer_use_dynamic_tools, map_app_detail, map_app_server_approval, map_apps,
        map_fuzzy_file_search_results, map_mcp_elicitation, map_mcp_resource_contents,
        map_mcp_runtime_catalog, map_remote_control_snapshot, map_remote_devices_page,
        map_timeline_item, map_user_input_request, mcp_elicitation_content_json,
        mcp_server_config_value, newest_review_mode_from_items, parse_appearance_preferences,
        parse_appearance_theme, parse_browser_download_preferences, parse_browser_permissions,
        parse_computer_key_chord, parse_generated_commit_message,
        parse_generated_commit_pull_request_messages, parse_generated_pull_request_message,
        parse_git_preferences, parse_keyboard_shortcut_preferences, parse_primary_window_placement,
        personalization_snapshot, plugin_directory_includes_marketplace,
        plugin_directory_marketplace_kinds, pull_request_generation_prompt,
        pull_request_output_schema, record_retryable_steer, remote_pairing_status_params,
        restored_browser_download, retryable_submission_inputs, run_computer_tool,
        safety_retry_fork_point, stored_browser_download, user_input_response,
    };

    #[cfg(any(windows, target_os = "linux"))]
    use super::computer_use_approval_detail;
    #[cfg(target_os = "linux")]
    use super::{
        computer_use_dynamic_tools_for_platform_with_available,
        computer_use_tool_supported_on_platform,
    };

    #[test]
    fn cancelling_worktree_runtime_joins_it_before_acknowledgement() {
        let cancellation = Arc::new(AtomicBool::new(false));
        let mut runtime = Some(PendingWorktreeRuntime {
            request_id: 7,
            cancellation: Arc::clone(&cancellation),
            thread: thread::spawn(|| {}),
        });

        cancel_pending_worktree_runtime(&mut runtime, 7);

        assert!(runtime.is_none());
        assert!(cancellation.load(Ordering::Acquire));
    }

    #[test]
    fn resumed_turn_uses_the_latest_review_mode_marker() {
        let entered = [json!({ "type": "enteredReviewMode" })];
        assert_eq!(
            newest_review_mode_from_items(entered.iter().rev()),
            Some(true)
        );
        let exited = [
            json!({ "type": "enteredReviewMode" }),
            json!({ "type": "exitedReviewMode" }),
        ];
        assert_eq!(
            newest_review_mode_from_items(exited.iter().rev()),
            Some(false)
        );
    }

    #[test]
    fn remote_pairing_status_uses_only_the_canonical_pairing_code() {
        let params = remote_pairing_status_params(&RemotePairing {
            pairing_code: "pairing-code".to_owned(),
            manual_pairing_code: Some("manual-code".to_owned()),
            environment_id: "environment-1".to_owned(),
            expires_at: 1_900_000_000,
        });

        assert_eq!(params.pairing_code.as_deref(), Some("pairing-code"));
        assert!(params.manual_pairing_code.is_none());
    }

    #[test]
    fn remote_status_notification_is_typed_and_drops_provider_identity() {
        let (events, receiver) = bounded(1);
        assert!(!handle_notification(
            "remoteControl/status/changed",
            json!({
                "status": "connected",
                "serverName": "private-hostname",
                "installationId": "private-installation-id",
                "environmentId": "environment-1"
            }),
            &events,
        ));

        assert!(matches!(
            receiver.try_recv(),
            Ok(Action::RemoteControlStatusChanged {
                status: RemoteControlRuntimeStatus::Connected,
                environment_id: Some(environment_id),
            }) if environment_id == "environment-1"
        ));
    }

    #[test]
    fn opaque_remote_identifiers_are_not_normalized() {
        assert_eq!(
            bounded_remote_identifier(" environment-1 ".to_owned(), 32),
            Some(" environment-1 ".to_owned())
        );
        assert!(bounded_remote_identifier("environment\n1".to_owned(), 32).is_none());
    }

    #[test]
    fn remote_environment_status_contract_preserves_opaque_values() {
        assert!(
            map_remote_control_snapshot(RemoteControlConnectionStatus::Connected, None).is_err()
        );
        assert!(matches!(
            map_remote_control_snapshot(RemoteControlConnectionStatus::Disabled, None),
            Ok((RemoteControlRuntimeStatus::Disabled, None))
        ));
        assert!(matches!(
            map_remote_control_snapshot(
                RemoteControlConnectionStatus::Connected,
                Some(" environment-1 ".to_owned()),
            ),
            Ok((RemoteControlRuntimeStatus::Connected, Some(environment_id)))
                if environment_id == " environment-1 "
        ));
        assert!(matches!(
            map_remote_control_snapshot(RemoteControlConnectionStatus::Connecting, None),
            Ok((RemoteControlRuntimeStatus::Connecting, None))
        ));
        assert!(matches!(
            map_remote_control_snapshot(
                RemoteControlConnectionStatus::Connecting,
                Some(" environment-1 ".to_owned()),
            ),
            Ok((RemoteControlRuntimeStatus::Connecting, Some(environment_id)))
                if environment_id == " environment-1 "
        ));
        assert!(matches!(
            map_remote_control_snapshot(
                RemoteControlConnectionStatus::Errored,
                Some(" environment-1 ".to_owned()),
            ),
            Ok((RemoteControlRuntimeStatus::Errored, Some(environment_id)))
                if environment_id == " environment-1 "
        ));
        assert!(
            map_remote_control_snapshot(
                RemoteControlConnectionStatus::Disabled,
                Some("environment-1".to_owned()),
            )
            .is_err()
        );
    }

    #[test]
    fn remote_device_page_is_bounded_before_the_action_channel() {
        let page = map_remote_devices_page(
            vec![RemoteControlClient {
                client_id: "client-1".to_owned(),
                display_name: Some("Phone".to_owned()),
                device_type: Some("mobile".to_owned()),
                device_model: Some("private-model".to_owned()),
                platform: Some("android".to_owned()),
                os_version: Some("1".to_owned()),
                app_version: Some("2".to_owned()),
                last_seen_at: Some(1),
            }],
            Some("next-1".to_owned()),
            1,
        );
        assert!(matches!(
            page,
            Ok((devices, Some(cursor)))
                if devices.len() == 1
                    && devices[0].client_id == "client-1"
                    && devices[0].display_name.as_deref() == Some("Phone")
                    && cursor == "next-1"
        ));

        assert!(
            map_remote_devices_page(
                vec![RemoteControlClient {
                    client_id: "x".repeat(257),
                    display_name: None,
                    device_type: None,
                    device_model: None,
                    platform: None,
                    os_version: None,
                    app_version: None,
                    last_seen_at: None,
                }],
                None,
                1,
            )
            .is_err()
        );
    }

    #[test]
    fn hook_state_writes_preserve_the_stable_nested_config_shape() {
        let key = "C:\\repo\\.codex\\hooks.json:pre_tool_use:0:0";
        assert_eq!(
            hook_state_config_value(key, Some(false), None),
            json!({
                (key): {
                    "enabled": false
                }
            })
        );
        assert_eq!(
            hook_state_config_value(key, None, Some("sha256:fixture")),
            json!({
                (key): {
                    "trusted_hash": "sha256:fixture"
                }
            })
        );
    }

    #[test]
    fn physical_escape_guard_is_scoped_to_computer_input_tools() {
        let event = AppServerEvent::Request {
            id: json!("request-1"),
            method: "item/tool/call".to_owned(),
            params: json!({
                "threadId": "thread-1",
                "turnId": "turn-1",
                "callId": "call-1",
                "namespace": "computer_use",
                "tool": "press_key",
                "arguments": {
                    "window": {"app": "fixture.exe", "id": 7},
                    "key": "Escape"
                }
            }),
        };
        let Some(request) = computer_tool_request_meta(&event) else {
            panic!("valid Computer Use request was not recognized");
        };
        assert_eq!(request.thread_id, "thread-1");
        assert_eq!(request.turn_id, "turn-1");
        assert_eq!(request.window_id.as_deref(), Some("7"));
        assert!(computer_tool_requires_interruption_monitor(&request.tool));
        assert!(!computer_tool_requires_interruption_monitor(
            "get_window_state"
        ));
        assert!(
            computer_tool_request_meta(&AppServerEvent::Request {
                id: json!("request-2"),
                method: "item/tool/call".to_owned(),
                params: json!({
                    "threadId": "thread-1",
                    "turnId": "turn-1",
                    "callId": "call-2",
                    "namespace": "other",
                    "tool": "press_key",
                    "arguments": {}
                }),
            })
            .is_none()
        );
    }

    #[test]
    fn physical_user_input_requires_a_fresh_window_state_before_actions() {
        let mut computer_accessibility = ComputerUseAccessibilityClient::new();
        computer_accessibility.mark_user_input("7");
        let (events, _event_rx) = bounded(1);

        assert_eq!(
            run_computer_tool(
                "press_key",
                &json!({
                    "window": {"app": "fixture.exe", "id": 7},
                    "key": "A"
                }),
                "thread-1",
                "7",
                &events,
                &mut computer_accessibility,
            ),
            Err(COMPUTER_USE_USER_INPUT_STALE_MESSAGE.to_owned())
        );
    }

    #[test]
    fn browser_url_policy_targets_match_the_stable_windows_contract() {
        for application_id in [
            "msedge.exe",
            r"programfiles_x64\Google\Chrome\Application\chrome.exe",
            "Brave.EXE",
            "opera",
            "iexplore.exe",
            "Mozilla.Firefox!firefox",
        ] {
            assert_eq!(
                browser_policy_target(application_id),
                BrowserPolicyTarget::Supported
            );
        }
        assert_eq!(
            browser_policy_target("browser.exe"),
            BrowserPolicyTarget::Unsupported
        );
        assert_eq!(
            browser_policy_target("code.exe"),
            BrowserPolicyTarget::NotBrowser
        );
    }

    #[test]
    fn composer_inputs_serialize_app_and_plugin_mentions_as_stable_markdown() {
        let skill_path = if cfg!(windows) {
            PathBuf::from(r"C:\skills\review\SKILL.md")
        } else {
            PathBuf::from("/skills/review/SKILL.md")
        };
        let input = composer_inputs(
            "Please continue".to_owned(),
            vec![
                ComposerAttachment {
                    path: PathBuf::from("app://calendar"),
                    name: "Calendar".to_owned(),
                    kind: ComposerAttachmentKind::App,
                },
                ComposerAttachment {
                    path: PathBuf::from("plugin://computer-use@openai"),
                    name: "Computer".to_owned(),
                    kind: ComposerAttachmentKind::Plugin,
                },
                ComposerAttachment {
                    path: skill_path.clone(),
                    name: "review".to_owned(),
                    kind: ComposerAttachmentKind::Skill,
                },
            ],
        );

        assert_eq!(input.len(), 2);
        assert!(matches!(
            &input[0],
            UserInput::Text {
                text,
                text_elements,
            } if text
                == "[@Calendar](app://calendar) [@Computer](plugin://computer-use@openai) Please continue"
                && text_elements.is_empty()
        ));
        assert!(matches!(
            &input[1],
            UserInput::Skill { name, path } if name == "review" && path == &skill_path
        ));
    }

    #[test]
    fn safety_retry_preserves_committed_steers_with_message_boundaries() {
        let input = retryable_submission_inputs(&[
            RetryableUserMessage {
                text: "Inspect the failure".to_owned(),
                attachments: Vec::new(),
            },
            RetryableUserMessage {
                text: "Focus on the Windows path".to_owned(),
                attachments: Vec::new(),
            },
        ]);

        assert_eq!(input.len(), 3);
        assert!(matches!(
            &input[0],
            UserInput::Text { text, .. } if text == "Inspect the failure"
        ));
        assert!(matches!(
            &input[1],
            UserInput::Text { text, .. } if text == "\n"
        ));
        assert!(matches!(
            &input[2],
            UserInput::Text { text, .. } if text == "Focus on the Windows path"
        ));
    }

    #[test]
    fn successful_steer_is_cached_before_the_reducer_round_trip() {
        let key = ("thread-1".to_owned(), "turn-1".to_owned());
        let mut retryable_turns = HashMap::from([(
            key.clone(),
            RetryableTurnSubmission {
                messages: vec![RetryableUserMessage {
                    text: "Inspect the failure".to_owned(),
                    attachments: Vec::new(),
                }],
                model: Some("gpt-5.6-sol".to_owned()),
                effort: Some("high".to_owned()),
                service_tier: None,
                permissions: None,
                approval_policy: None,
                approvals_reviewer: None,
                plan_mode: false,
                personality: Personality::Pragmatic,
            },
        )]);

        record_retryable_steer(
            &mut retryable_turns,
            &key.0,
            &key.1,
            &RetryableUserMessage {
                text: "Focus on Windows".to_owned(),
                attachments: Vec::new(),
            },
        );

        assert_eq!(
            retryable_turns[&key]
                .messages
                .iter()
                .map(|message| message.text.as_str())
                .collect::<Vec<_>>(),
            ["Inspect the failure", "Focus on Windows"]
        );
    }

    #[test]
    fn safety_retry_requires_the_interrupted_turn_to_remain_latest() {
        let valid = vec![
            json!({"id": "turn-2", "status": "interrupted"}),
            json!({"id": "turn-1", "status": "completed"}),
        ];
        assert!(safety_retry_fork_point(&valid, "turn-2").is_ok());
        assert!(safety_retry_fork_point(&valid, "turn-1").is_err());
        assert!(
            safety_retry_fork_point(&[json!({"id": "turn-2", "status": "inProgress"})], "turn-2")
                .is_err()
        );
        assert!(
            safety_retry_fork_point(
                &[
                    json!({"id": "turn-2", "status": "interrupted"}),
                    json!({"id": "turn-1", "status": "inProgress"}),
                ],
                "turn-2"
            )
            .is_err()
        );
    }

    #[test]
    fn appearance_theme_preference_round_trips_exact_values() {
        for theme in [
            AppearanceTheme::System,
            AppearanceTheme::Light,
            AppearanceTheme::Dark,
        ] {
            assert_eq!(
                parse_appearance_theme(appearance_theme_key(theme)),
                Some(theme)
            );
        }
        assert_eq!(parse_appearance_theme("auto"), None);
    }

    #[test]
    fn primary_window_placement_round_trips_as_one_bounded_versioned_value() {
        let Some(placement) = PrimaryWindowPlacement::new(-1_280, 42, 1_278, 789, true) else {
            panic!("valid window placement must construct");
        };
        let Ok(encoded) = encode_primary_window_placement(placement) else {
            panic!("window placement must serialize");
        };

        assert!(encoded.len() < 256);
        assert_eq!(parse_primary_window_placement(&encoded), Some(placement));

        let Ok(mut wrong_version) = serde_json::from_str::<Value>(&encoded) else {
            panic!("encoded window placement must be valid JSON");
        };
        wrong_version["version"] = json!(2);
        assert!(parse_primary_window_placement(&wrong_version.to_string()).is_none());

        let Ok(mut undersized) = serde_json::from_str::<Value>(&encoded) else {
            panic!("encoded window placement must be valid JSON");
        };
        undersized["width"] = json!(479);
        let Some(clamped) = parse_primary_window_placement(&undersized.to_string()) else {
            panic!("undersized window placement must be clamped");
        };
        assert_eq!(clamped.width(), 480);
        assert!(parse_primary_window_placement(&" ".repeat(1_025)).is_none());
    }

    #[test]
    fn appearance_preferences_round_trip_as_one_bounded_versioned_value() {
        let mut preferences = AppearancePreferences {
            use_pointer_cursors: true,
            reduced_motion: ReducedMotionPreference::On,
            diff_marker_style: DiffMarkerStyle::Symbols,
            ui_font_size: 16,
            code_font_size: 24,
            light: AppearancePalette::proof_light(),
            ..AppearancePreferences::default()
        };
        preferences.dark.ui_font = Some("Inter".to_owned());

        let Ok(encoded) = encode_appearance_preferences(&preferences) else {
            panic!("preferences must serialize");
        };
        assert!(encoded.len() < 4 * 1024);
        assert_eq!(
            parse_appearance_preferences(&encoded),
            Some(preferences.clone())
        );

        let Ok(mut wrong_version) = serde_json::from_str::<Value>(&encoded) else {
            panic!("encoded preferences must be valid JSON");
        };
        wrong_version["version"] = json!(2);
        assert!(
            parse_appearance_preferences(&wrong_version.to_string()).is_none(),
            "unknown preference schemas must not be guessed"
        );

        let Ok(mut invalid_theme) = serde_json::from_str::<Value>(&encoded) else {
            panic!("encoded preferences must be valid JSON");
        };
        invalid_theme["dark"]["codeThemeId"] = json!("not-a-theme");
        assert!(parse_appearance_preferences(&invalid_theme.to_string()).is_none());

        assert_eq!(
            preferences.palette(AppearanceVariant::Light).code_theme_id,
            "proof"
        );
    }

    #[test]
    fn git_preferences_round_trip_as_one_bounded_versioned_value() {
        let worktree_root = if cfg!(windows) {
            PathBuf::from(r"E:\codex-worktrees")
        } else {
            PathBuf::from("/tmp/codex-worktrees")
        };
        let preferences = GitPreferences {
            branch_prefix: "feature/".to_owned(),
            always_force_push: true,
            create_pull_request_as_draft: true,
            pull_request_merge_method: PullRequestMergeMethod::Squash,
            review_mode: GitReviewMode::LastTurnOnly,
            review_delivery: ReviewDelivery::Detached,
            commit_instructions: "Use an imperative subject.".to_owned(),
            pull_request_instructions: "Follow the repository template.".to_owned(),
            worktree_root: Some(worktree_root),
        };
        let Ok(encoded) = encode_git_preferences(&preferences) else {
            panic!("Git preferences must serialize");
        };
        assert!(encoded.len() < 4 * 1024);
        assert_eq!(parse_git_preferences(&encoded), Some(preferences));

        let Ok(mut wrong_version) = serde_json::from_str::<Value>(&encoded) else {
            panic!("encoded Git preferences must be valid JSON");
        };
        wrong_version["version"] = json!(2);
        assert!(parse_git_preferences(&wrong_version.to_string()).is_none());

        let Ok(mut invalid_merge_method) = serde_json::from_str::<Value>(&encoded) else {
            panic!("encoded Git preferences must be valid JSON");
        };
        invalid_merge_method["pullRequestMergeMethod"] = json!("rebase");
        assert!(parse_git_preferences(&invalid_merge_method.to_string()).is_none());

        let Ok(mut invalid_review_delivery) = serde_json::from_str::<Value>(&encoded) else {
            panic!("encoded Git preferences must be valid JSON");
        };
        invalid_review_delivery["reviewDelivery"] = json!("mail");
        assert!(parse_git_preferences(&invalid_review_delivery.to_string()).is_none());

        let Ok(mut legacy) = serde_json::from_str::<Value>(&encoded) else {
            panic!("encoded Git preferences must be valid JSON");
        };
        if let Some(object) = legacy.as_object_mut() {
            object.remove("reviewMode");
            object.remove("reviewDelivery");
        }
        assert_eq!(
            parse_git_preferences(&legacy.to_string())
                .map(|value| (value.review_mode, value.review_delivery)),
            Some((GitReviewMode::Full, ReviewDelivery::Inline))
        );
    }

    #[test]
    fn browser_download_preferences_round_trip_as_one_bounded_versioned_value() {
        let directory = if cfg!(windows) {
            PathBuf::from(r"C:\Users\tester\Downloads\Codex")
        } else {
            PathBuf::from("/home/tester/Downloads/Codex")
        };
        let preferences = BrowserDownloadPreferences {
            download_directory: Some(directory),
            prompt_for_user_downloads: true,
        };
        let Ok(encoded) = encode_browser_download_preferences(&preferences) else {
            panic!("Browser download preferences must serialize");
        };
        assert!(encoded.len() < 1_024);
        assert_eq!(
            parse_browser_download_preferences(&encoded),
            Some(preferences)
        );

        let Ok(mut wrong_version) = serde_json::from_str::<Value>(&encoded) else {
            panic!("encoded Browser download preferences must be valid JSON");
        };
        wrong_version["version"] = json!(2);
        assert!(parse_browser_download_preferences(&wrong_version.to_string()).is_none());

        let invalid_relative = json!({
            "version": 1,
            "downloadDirectory": "relative",
            "promptForUserDownloads": false,
        });
        assert_eq!(
            parse_browser_download_preferences(&invalid_relative.to_string()),
            Some(BrowserDownloadPreferences::default())
        );
    }

    #[test]
    fn browser_permissions_round_trip_as_one_bounded_versioned_value() {
        let permissions = BrowserPermissionsState {
            approval_mode: BrowserApprovalMode::NeverAsk,
            download_approval_mode: BrowserApprovalMode::AlwaysAsk,
            upload_approval_mode: BrowserApprovalMode::NeverAsk,
            full_cdp_access_enabled: true,
            sites: vec![BrowserSitePermission {
                origin: "https://example.com".to_owned(),
                browse: BrowserPermissionValue::Allow,
                download: BrowserPermissionValue::Block,
                upload: BrowserPermissionValue::Default,
                full_cdp: BrowserPermissionValue::Allow,
            }],
        };
        let Ok(encoded) = encode_browser_permissions(&permissions) else {
            panic!("Browser permissions must serialize");
        };
        assert!(encoded.len() < 1_024);
        assert_eq!(parse_browser_permissions(&encoded), Some(permissions));

        let Ok(mut wrong_version) = serde_json::from_str::<Value>(&encoded) else {
            panic!("encoded Browser permissions must be valid JSON");
        };
        wrong_version["version"] = json!(2);
        assert!(parse_browser_permissions(&wrong_version.to_string()).is_none());

        let Ok(mut invalid_value) = serde_json::from_str::<Value>(&encoded) else {
            panic!("encoded Browser permissions must be valid JSON");
        };
        invalid_value["sites"][0]["download"] = json!("prompt");
        assert!(parse_browser_permissions(&invalid_value.to_string()).is_none());
    }

    #[test]
    fn persisted_browser_download_omits_the_source_url_and_active_records() {
        let path = if cfg!(windows) {
            PathBuf::from(r"C:\Users\tester\Downloads\fixture.txt")
        } else {
            PathBuf::from("/home/tester/Downloads/fixture.txt")
        };
        let mut download = BrowserDownloadState {
            can_cancel: false,
            can_pause: false,
            can_resume: false,
            context_id: "chat".to_owned(),
            file_exists: true,
            filename: "fixture.txt".to_owned(),
            id: "download-1".to_owned(),
            path,
            received_bytes: 7,
            started_at_ms: 1,
            status: BrowserDownloadStatus::Complete,
            total_bytes: 7,
            updated_at_ms: 2,
            url: "https://example.com/private?token=do-not-store".to_owned(),
            user_initiated: true,
        };

        let Some(stored) = stored_browser_download(&download) else {
            panic!("terminal download must be persistable");
        };
        let restored = restored_browser_download(stored);
        assert!(restored.url.is_empty());
        assert_eq!(restored.id, download.id);
        assert_eq!(restored.status, BrowserDownloadStatus::Complete);

        download.status = BrowserDownloadStatus::Paused;
        assert!(stored_browser_download(&download).is_none());
    }

    #[test]
    fn keyboard_shortcut_preferences_round_trip_as_one_bounded_versioned_value() {
        let preferences = KeyboardShortcutPreferences {
            overrides: [
                (
                    "openCommandMenu".to_owned(),
                    vec!["Ctrl+Shift+K".to_owned(), "Ctrl+Space".to_owned()],
                ),
                ("toggleTerminal".to_owned(), Vec::new()),
            ]
            .into_iter()
            .collect(),
        };
        let Ok(encoded) = encode_keyboard_shortcut_preferences(&preferences) else {
            panic!("keyboard shortcut preferences must serialize");
        };
        assert!(encoded.len() < 2 * 1024);
        assert_eq!(
            parse_keyboard_shortcut_preferences(&encoded),
            Some(preferences.clone())
        );

        let Ok(mut wrong_version) = serde_json::from_str::<Value>(&encoded) else {
            panic!("encoded shortcut preferences must be valid JSON");
        };
        wrong_version["version"] = json!(2);
        assert!(parse_keyboard_shortcut_preferences(&wrong_version.to_string()).is_none());

        let Ok(mut too_many) = serde_json::from_str::<Value>(&encoded) else {
            panic!("encoded shortcut preferences must be valid JSON");
        };
        too_many["bindings"]["openCommandMenu"] =
            json!(["Ctrl+A", "Ctrl+B", "Ctrl+C", "Ctrl+D", "Ctrl+E"]);
        assert!(parse_keyboard_shortcut_preferences(&too_many.to_string()).is_none());
    }

    #[test]
    fn fuzzy_file_results_stay_inside_known_roots_and_skip_stable_exclusions() {
        let root = if cfg!(windows) {
            PathBuf::from(r"C:\repo")
        } else {
            PathBuf::from("/repo")
        };
        let wrong_root = if cfg!(windows) {
            PathBuf::from(r"C:\other")
        } else {
            PathBuf::from("/other")
        };
        let result =
            |file_name: &str,
             path: &str,
             result_root: PathBuf,
             match_type: FuzzyFileSearchMatchType| ProtocolFuzzyFileResult {
                file_name: file_name.to_owned(),
                indices: None,
                match_type,
                path: PathBuf::from(path),
                root: result_root,
                score: 10,
            };

        let mapped = map_fuzzy_file_search_results(
            vec![
                result(
                    "lib.rs",
                    "src/lib.rs",
                    root.clone(),
                    FuzzyFileSearchMatchType::File,
                ),
                result(
                    "src",
                    "src",
                    root.clone(),
                    FuzzyFileSearchMatchType::Directory,
                ),
                result(
                    "index.js",
                    "node_modules/pkg/index.js",
                    root.clone(),
                    FuzzyFileSearchMatchType::File,
                ),
                result(
                    "secret.txt",
                    "../secret.txt",
                    root.clone(),
                    FuzzyFileSearchMatchType::File,
                ),
                result(
                    "other.rs",
                    "other.rs",
                    wrong_root,
                    FuzzyFileSearchMatchType::File,
                ),
            ],
            std::slice::from_ref(&root),
        );

        assert_eq!(mapped.len(), 2);
        assert_eq!(mapped[0].path, root.join("src/lib.rs"));
        assert_eq!(mapped[0].detail, "src");
        assert_eq!(mapped[1].match_type, FuzzyFileMatchType::Directory);
    }

    #[test]
    fn commit_message_generation_uses_the_stable_prompt_and_response_bounds() {
        let context = "x".repeat(25_000);
        let prompt = commit_generation_prompt(&context, "Use a Conventional Commit subject.");
        let (_, bounded_context) = prompt.split_once("Diff context:\n").unwrap_or_default();

        assert_eq!(bounded_context.chars().count(), 20_000);
        assert!(prompt.contains("Make 0 tool calls."));
        assert!(prompt.contains("Custom commit instructions:\nUse a Conventional Commit subject."));
        assert_eq!(
            commit_message_output_schema()["properties"]["message"]["maxLength"],
            4_000
        );
        assert_eq!(
            parse_generated_commit_message(r#"{"message":"Add native commit flow"}"#).as_deref(),
            Some("Add native commit flow")
        );
        assert!(parse_generated_commit_message(r#"{"message":"Too short"}"#).is_some());
        assert!(parse_generated_commit_message(r#"{"message":"short"}"#).is_none());
        assert!(
            parse_generated_commit_message(
                r#"{"message":"Add native commit flow","unexpected":true}"#
            )
            .is_none()
        );
    }

    #[test]
    fn app_detail_mapping_keeps_bounded_http_metadata_and_tools() {
        let detail = map_app_detail(ConnectorMetadata {
            id: "connector_calendar".to_owned(),
            name: "Calendar".to_owned(),
            description: Some("Read and update events.".to_owned()),
            distribution_channel: Some("openai".to_owned()),
            icon_url: Some("file:///calendar.png".to_owned()),
            icon_url_dark: Some("https://example.com/calendar-dark.png".to_owned()),
            install_url: Some("javascript:alert(1)".to_owned()),
            plugin_display_names: vec!["Calendar plugin".to_owned()],
            tool_summaries: Some(vec![AppToolSummary {
                name: "list_events".to_owned(),
                title: None,
                description: "Lists events.".to_owned(),
            }]),
        });

        assert!(detail.logo_url.is_none());
        assert_eq!(
            detail.logo_url_dark.as_deref(),
            Some("https://example.com/calendar-dark.png")
        );
        assert!(detail.install_url.is_none());
        assert_eq!(detail.tools.len(), 1);
        assert_eq!(detail.tools[0].title, "list_events");
    }

    #[test]
    fn mcp_runtime_mapping_keeps_bounded_inspection_metadata() {
        let response = serde_json::from_value::<ProtocolMcpServerStatus>(json!({
            "name": "calendar",
            "authStatus": "oAuth",
            "serverInfo": {
                "name": "Calendar MCP",
                "version": "1.2.3",
                "title": "Calendar",
                "description": "x".repeat(MAX_MCP_SERVER_FIELD_BYTES + 1),
                "websiteUrl": "javascript:alert(1)"
            },
            "tools": {
                "list_events": {
                    "name": "list_events",
                    "title": "List events",
                    "description": "Lists calendar events.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "date": { "type": "string" }
                        }
                    }
                }
            },
            "resources": [{
                "name": "today",
                "uri": "calendar://today",
                "mimeType": "application/json",
                "size": 128
            }],
            "resourceTemplates": [{
                "name": "day",
                "uriTemplate": "calendar://day/{date}",
                "mimeType": "application/json"
            }]
        }));
        let Ok(status) = response else {
            panic!("MCP fixture must match the official schema");
        };
        let catalog = map_mcp_runtime_catalog(status);

        assert_eq!(catalog.tools.len(), 1);
        assert!(catalog.tools[0].input_schema.contains("\"date\""));
        assert_eq!(catalog.resources[0].uri, "calendar://today");
        assert_eq!(
            catalog.resource_templates[0].uri_template,
            "calendar://day/{date}"
        );
        assert_eq!(
            catalog
                .server_info
                .as_ref()
                .and_then(|info| info.website_url.as_deref()),
            None
        );
        assert_eq!(
            catalog
                .server_info
                .as_ref()
                .and_then(|info| info.description.as_ref())
                .map(String::len),
            Some(MAX_MCP_SERVER_FIELD_BYTES)
        );
        assert!(catalog.truncated);
    }

    #[test]
    fn mcp_resource_mapping_keeps_text_bounded_and_discards_raw_blobs() {
        let contents = map_mcp_resource_contents(vec![
            codex_protocol::McpResourceContent {
                uri: "calendar://today".to_owned(),
                mime_type: Some("application/json".to_owned()),
                text: Some("x".repeat(MAX_ITEM_TEXT_BYTES + 1)),
                blob: None,
            },
            codex_protocol::McpResourceContent {
                uri: "calendar://image".to_owned(),
                mime_type: Some("image/png".to_owned()),
                text: None,
                blob: Some("AA==".to_owned()),
            },
        ]);

        assert_eq!(
            contents[0].text.as_ref().map(String::len),
            Some(MAX_ITEM_TEXT_BYTES)
        );
        assert!(contents[0].truncated);
        assert_eq!(contents[1].blob_bytes, Some(1));
        assert!(contents[1].text.is_none());
    }

    #[test]
    fn structured_user_input_mapping_and_response_match_the_stable_contract() {
        let Ok(request) = map_user_input_request(
            "request-input-1".to_owned(),
            json!({
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "item-1",
                "questions": [{
                    "id": "scope",
                    "header": "Scope",
                    "question": "How broad should the change be?",
                    "options": [{
                        "label": "Focused (Recommended)",
                        "description": "Change only the affected path."
                    }],
                    "isOther": true,
                    "isSecret": false
                }],
                "autoResolutionMs": 1
            }),
        ) else {
            panic!("valid structured input request was rejected");
        };
        assert_eq!(request.task_id, "thread-1");
        assert_eq!(request.turn_id, "turn-1");
        assert_eq!(request.item_id, "item-1");
        assert_eq!(request.auto_resolution_ms, Some(1));
        assert_eq!(request.questions.len(), 1);
        assert_eq!(request.questions[0].id, "scope");
        assert_eq!(
            request.questions[0].options[0].label,
            "Focused (Recommended)"
        );
        assert!(request.questions[0].is_other);

        let Ok(response) = user_input_response(UserInputAnswers {
            answers: vec![UserInputAnswer {
                question_id: "scope".to_owned(),
                answers: vec!["Focused (Recommended)".to_owned()],
            }],
        }) else {
            panic!("valid structured input response was rejected");
        };
        let Ok(response) = serde_json::to_value(response) else {
            panic!("structured input response did not serialize");
        };
        assert_eq!(
            response,
            json!({
                "answers": {
                    "scope": {
                        "answers": ["Focused (Recommended)"]
                    }
                }
            })
        );

        let Ok(skipped) = user_input_response(UserInputAnswers::default()) else {
            panic!("empty structured input response was rejected");
        };
        let Ok(skipped) = serde_json::to_value(skipped) else {
            panic!("empty structured input response did not serialize");
        };
        assert_eq!(skipped, json!({"answers": {}}));
    }

    #[test]
    fn stable_approval_mapping_keeps_typed_scopes_and_permission_details() {
        let Ok((command, pending)) = map_app_server_approval(
            "approval-command-1".to_owned(),
            "item/commandExecution/requestApproval",
            json!({
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "command-1",
                "startedAtMs": 1,
                "command": "ignored fallback",
                "commandActions": [
                    {"type": "unknown", "command": "git status"},
                    {"type": "unknown", "command": "cargo test"}
                ],
                "networkApprovalContext": {
                    "host": "crates.io",
                    "protocol": "https"
                },
                "proposedExecpolicyAmendment": ["cargo", "test"],
                "proposedNetworkPolicyAmendments": [
                    {"action": "deny", "host": "crates.io"},
                    {"action": "allow", "host": "crates.io"}
                ]
            }),
            json!(1),
        ) else {
            panic!("valid command approval was rejected");
        };
        let ApprovalContext::Command(context) = command.context else {
            panic!("command approval did not retain its typed context");
        };
        assert_eq!(context.command, "git status && cargo test");
        assert_eq!(
            context
                .network_approval_context
                .as_ref()
                .map(|network| network.host.as_str()),
            Some("crates.io")
        );
        assert!(matches!(
            context.proposed_network_policy_amendment,
            Some(amendment)
                if amendment.action == NetworkPolicyAction::Allow
                    && amendment.host == "crates.io"
        ));
        assert!(matches!(
            pending,
            PendingApproval::Command {
                proposed_execpolicy_amendment: Some(amendment),
                proposed_network_policy_amendment: Some(_),
                ..
            } if amendment == ["cargo", "test"]
        ));

        let Ok((permissions, pending)) = map_app_server_approval(
            "approval-permission-1".to_owned(),
            "item/permissions/requestApproval",
            json!({
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "permission-1",
                "startedAtMs": 1,
                "cwd": "C:\\work",
                "reason": "Read generated documentation.",
                "permissions": {
                    "network": {"enabled": true},
                    "fileSystem": {
                        "entries": [
                            {
                                "access": "read",
                                "path": {"type": "path", "path": "C:\\work\\docs"}
                            },
                            {
                                "access": "write",
                                "path": {"type": "path", "path": "C:\\work\\docs"}
                            }
                        ]
                    }
                }
            }),
            json!(2),
        ) else {
            panic!("valid permission approval was rejected");
        };
        let ApprovalContext::Permissions(context) = permissions.context else {
            panic!("permission approval did not retain its typed context");
        };
        assert!(matches!(
            context.details.as_slice(),
            [
                PermissionRequestDetail::Network,
                PermissionRequestDetail::FileSystem {
                    access: PermissionFileSystemAccess::ReadWrite,
                    paths
                }
            ] if paths.as_slice() == ["C:\\work\\docs"]
        ));
        assert!(matches!(
            pending,
            PendingApproval::Permissions { permissions, .. }
                if permissions.network.is_some() && permissions.file_system.is_some()
        ));

        let Ok((file_change, pending)) = map_app_server_approval(
            "approval-file-1".to_owned(),
            "item/fileChange/requestApproval",
            json!({
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "patch-1",
                "startedAtMs": 1,
                "grantRoot": "C:\\work",
                "reason": "Update the focused files."
            }),
            json!(3),
        ) else {
            panic!("valid file-change approval was rejected");
        };
        assert!(matches!(
            file_change.context,
            ApprovalContext::FileChange(context)
                if context.item_id == "patch-1"
                    && context.grant_root.as_deref() == Some("C:\\work")
        ));
        assert!(matches!(pending, PendingApproval::FileChange { .. }));
    }

    #[test]
    fn mcp_elicitation_mapping_supports_bounded_forms_and_rejects_unsafe_links() {
        let request = map_mcp_elicitation(
            "request-1".to_owned(),
            json!({
                "serverName": "calendar",
                "threadId": "thread-1",
                "turnId": "turn-1",
                "mode": "url",
                "elicitationId": "elicitation-1",
                "message": "x".repeat(MAX_MCP_SERVER_FIELD_BYTES + 1),
                "url": "https://example.com/connect"
            }),
        );
        let request = match request {
            Ok(McpElicitation::Url(request)) => request,
            Ok(McpElicitation::Form(_)) => panic!("URL elicitation mapped to a form"),
            Ok(McpElicitation::BrowserOrigin(_)) => {
                panic!("URL elicitation mapped to a Browser origin request")
            }
            Ok(McpElicitation::BrowserResource(_)) => {
                panic!("URL elicitation mapped to a Browser resource request")
            }
            Err(error) => panic!("valid URL elicitation was rejected: {error:?}"),
        };
        assert_eq!(request.request_id, "request-1");
        assert_eq!(request.server_name, "calendar");
        assert_eq!(request.message.len(), MAX_MCP_SERVER_FIELD_BYTES);
        assert_eq!(request.url, "https://example.com/connect");
        assert!(!request.link_opened);

        assert_eq!(
            map_mcp_elicitation(
                "request-2".to_owned(),
                json!({
                    "serverName": "calendar",
                    "threadId": "thread-1",
                    "mode": "url",
                    "elicitationId": "elicitation-2",
                    "message": "Connect",
                    "url": "javascript:alert(1)"
                }),
            ),
            Err(McpElicitationMapError::Invalid)
        );
        assert!(matches!(
            map_mcp_elicitation(
                "request-3".to_owned(),
                json!({
                    "serverName": "calendar",
                    "threadId": "thread-1",
                    "mode": "form",
                    "message": "Choose a calendar and date",
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
                                "type": "boolean"
                            }
                        },
                        "required": ["calendar", "date"]
                    }
                }),
            ),
            Ok(McpElicitation::Form(McpFormElicitation {
                fields,
                ..
            })) if fields.len() == 3
                && fields.iter().any(|field| {
                    field.name == "calendar"
                        && field.required
                        && matches!(
                            &field.kind,
                            McpFormFieldKind::SingleSelect { options }
                                if options.len() == 2
                        )
                        && field.default
                            == Some(McpElicitationValue::String("work".to_owned()))
                })
        ));
        assert!(matches!(
            map_mcp_elicitation(
                "request-image-picker".to_owned(),
                json!({
                    "serverName": "templates",
                    "threadId": "thread-1",
                    "mode": "openai/form",
                    "message": "Choose a template",
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
                }),
            ),
            Ok(McpElicitation::Form(McpFormElicitation {
                openai: true,
                fields,
                ..
            })) if matches!(
                fields.as_slice(),
                [field] if field.required
                    && matches!(
                        &field.kind,
                        McpFormFieldKind::ImagePicker { items }
                            if items.len() == 1
                                && items[0].value == "clean"
                    )
            )
        ));
        assert!(matches!(
            map_mcp_elicitation(
                "request-invalid-image-picker".to_owned(),
                json!({
                    "serverName": "templates",
                    "threadId": "thread-1",
                    "mode": "openai/form",
                    "message": "Choose a template",
                    "requestedSchema": {
                        "type": "object",
                        "properties": {
                            "template": {
                                "type": "openai/imagePicker",
                                "items": [
                                    {
                                        "id": "clean",
                                        "title": "Clean",
                                        "image": "https://example.com/template.png"
                                    }
                                ]
                            }
                        }
                    }
                }),
            ),
            Ok(McpElicitation::Form(McpFormElicitation {
                openai: true,
                unsupported_openai: true,
                fields,
                ..
            })) if fields.is_empty()
        ));
        assert_eq!(
            map_mcp_elicitation(
                "request-4".to_owned(),
                json!({
                    "serverName": "calendar",
                    "threadId": "thread-1",
                    "mode": "url",
                    "elicitationId": "elicitation-4",
                    "message": "Connect",
                    "url": "http://example.com/connect"
                }),
            ),
            Err(McpElicitationMapError::Invalid)
        );

        let content = McpElicitationContent {
            fields: vec![
                (
                    "calendar".to_owned(),
                    McpElicitationValue::String("work".to_owned()),
                ),
                ("notify".to_owned(), McpElicitationValue::Boolean(true)),
            ],
        };
        assert_eq!(
            mcp_elicitation_content_json(&content),
            Ok(json!({"calendar": "work", "notify": true}))
        );
    }

    #[test]
    fn browser_origin_elicitation_maps_the_official_metadata_contract() {
        let request = map_mcp_elicitation(
            "browser-request-1".to_owned(),
            json!({
                "serverName": "node_repl",
                "threadId": "thread-1",
                "turnId": "turn-1",
                "mode": "form",
                "message": "Allow Browser to access this site?",
                "requestedSchema": {
                    "type": "object",
                    "properties": {}
                },
                "_meta": {
                    "codex_approval_kind": "mcp_tool_call",
                    "connector_id": "browser-use",
                    "tool_name": "access_browser_origin",
                    "persist": ["always"],
                    "origin": "https://Example.com:443/private",
                    "reason": "Continue on this website"
                }
            }),
        );
        assert!(matches!(
            request,
            Ok(McpElicitation::BrowserOrigin(McpBrowserOriginElicitation {
                request_id,
                task_id,
                turn_id: Some(turn_id),
                server_name,
                source_name,
                origin,
                reason: Some(reason),
                ..
            })) if request_id == "browser-request-1"
                && task_id == "thread-1"
                && turn_id == "turn-1"
                && server_name == "node_repl"
                && source_name == "Browser"
                && origin == "https://example.com"
                && reason == "Continue on this website"
        ));

        assert_eq!(
            map_mcp_elicitation(
                "browser-request-unsafe".to_owned(),
                json!({
                    "serverName": "node_repl",
                    "threadId": "thread-1",
                    "mode": "form",
                    "message": "Allow Browser to access this site?",
                    "requestedSchema": {
                        "type": "object",
                        "properties": {}
                    },
                    "_meta": {
                        "codex_approval_kind": "mcp_tool_call",
                        "connector_id": "browser-use",
                        "tool_name": "access_browser_origin",
                        "persist": "always",
                        "origin": "https://user:secret@example.com/private"
                    }
                }),
            ),
            Err(McpElicitationMapError::Invalid)
        );
    }

    #[test]
    fn browser_origin_rules_and_replies_match_the_official_scope_choices() {
        let request = McpBrowserOriginElicitation {
            request_id: "browser-request-1".to_owned(),
            task_id: "thread-1".to_owned(),
            turn_id: Some("turn-1".to_owned()),
            server_name: "node_repl".to_owned(),
            source_name: "Browser".to_owned(),
            origin: "https://example.com".to_owned(),
            reason: None,
            message: "Allow Browser to access this site?".to_owned(),
        };
        let mut permissions = BrowserPermissionsState::default();
        assert_eq!(browser_origin_auto_decision(&request, &permissions), None);

        permissions.sites.push(BrowserSitePermission {
            origin: request.origin.clone(),
            browse: BrowserPermissionValue::Allow,
            download: BrowserPermissionValue::Default,
            upload: BrowserPermissionValue::Default,
            full_cdp: BrowserPermissionValue::Default,
        });
        assert_eq!(
            browser_origin_auto_decision(&request, &permissions),
            Some(BrowserOriginElicitationDecision::AllowSite)
        );
        permissions.sites[0].browse = BrowserPermissionValue::Block;
        assert_eq!(
            browser_origin_auto_decision(&request, &permissions),
            Some(BrowserOriginElicitationDecision::Deny)
        );
        permissions.sites.clear();
        permissions.approval_mode = BrowserApprovalMode::NeverAsk;
        assert_eq!(
            browser_origin_auto_decision(&request, &permissions),
            Some(BrowserOriginElicitationDecision::AllowOnce)
        );

        let Ok(deny_response) = serde_json::to_value(browser_origin_elicitation_response(
            BrowserOriginElicitationDecision::Deny,
        )) else {
            panic!("Browser deny response must serialize");
        };
        assert_eq!(deny_response, json!({"action": "decline"}));

        let Ok(once_response) = serde_json::to_value(browser_origin_elicitation_response(
            BrowserOriginElicitationDecision::AllowOnce,
        )) else {
            panic!("Browser one-time response must serialize");
        };
        assert_eq!(once_response, json!({"action": "accept", "content": {}}));

        let Ok(site_response) = serde_json::to_value(browser_origin_elicitation_response(
            BrowserOriginElicitationDecision::AllowSite,
        )) else {
            panic!("Browser site response must serialize");
        };
        assert_eq!(
            site_response,
            json!({
                "action": "accept",
                "content": {},
                "_meta": {"persist": "always"}
            })
        );
    }

    #[test]
    fn browser_resource_elicitations_map_the_official_metadata_contracts() {
        let download = map_mcp_elicitation(
            "browser-download".to_owned(),
            json!({
                "serverName": "node_repl",
                "threadId": "thread-1",
                "turnId": "turn-1",
                "mode": "form",
                "message": "Allow download from https://example.com?",
                "requestedSchema": {
                    "type": "object",
                    "properties": {}
                },
                "_meta": {
                    "codex_approval_kind": "mcp_tool_call",
                    "connector_id": "browser-use",
                    "connector_name": "Browser",
                    "persist": ["session", "always"],
                    "tool_name": "download_browser_files",
                    "tool_title": "Download browser files",
                    "tool_params": {
                        "origin": "https://Example.com:443/file"
                    },
                    "file_transfer": "download",
                    "origin": "https://Example.com:443/file"
                }
            }),
        );
        assert!(matches!(
            download,
            Ok(McpElicitation::BrowserResource(
                McpBrowserResourceElicitation {
                    request_id,
                    task_id,
                    turn_id: Some(turn_id),
                    source_name,
                    origin,
                    resource: BrowserPermissionResource::Download,
                    persist_session: true,
                    persist_always: true,
                    elevated_risk: false,
                    ..
                }
            )) if request_id == "browser-download"
                && task_id == "thread-1"
                && turn_id == "turn-1"
                && source_name == "Browser"
                && origin == "https://example.com"
        ));

        let raw_cdp = map_mcp_elicitation(
            "browser-raw-cdp".to_owned(),
            json!({
                "serverName": "node_repl",
                "threadId": "thread-1",
                "mode": "form",
                "message": "Allow Browser to use full CDP access on https://example.com",
                "requestedSchema": {
                    "type": "object",
                    "properties": {}
                },
                "_meta": {
                    "codex_approval_kind": "mcp_tool_call",
                    "connector_id": "browser-use",
                    "connector_name": "Browser",
                    "persist": "always",
                    "riskLevel": "high",
                    "tool_name": "access_browser_origin_with_raw_cdp",
                    "tool_title": "Use raw CDP on browser origin",
                    "tool_params": {
                        "origin": "https://example.com/private"
                    },
                    "tool_params_display": [],
                    "full_cdp_access": true,
                    "origin": "https://example.com/private"
                }
            }),
        );
        assert!(matches!(
            raw_cdp,
            Ok(McpElicitation::BrowserResource(
                McpBrowserResourceElicitation {
                    origin,
                    resource: BrowserPermissionResource::FullCdp,
                    persist_session: false,
                    persist_always: true,
                    elevated_risk: true,
                    ..
                }
            )) if origin == "https://example.com"
        ));

        let page_asset = map_mcp_elicitation(
            "browser-page-asset".to_owned(),
            json!({
                "serverName": "node_repl",
                "threadId": "thread-1",
                "mode": "form",
                "message": "Allow download from https://cdn.example.com?",
                "requestedSchema": {
                    "type": "object",
                    "properties": {}
                },
                "_meta": {
                    "codex_approval_kind": "mcp_tool_call",
                    "connector_id": "browser-use",
                    "connector_name": "Browser",
                    "persist": ["session", "always"],
                    "tool_params": {
                        "asset_origins": ["https://cdn.example.com/image.png"]
                    }
                }
            }),
        );
        assert!(matches!(
            page_asset,
            Ok(McpElicitation::BrowserResource(
                McpBrowserResourceElicitation {
                    origin,
                    resource: BrowserPermissionResource::Download,
                    ..
                }
            )) if origin == "https://cdn.example.com"
        ));

        assert_eq!(
            map_mcp_elicitation(
                "browser-unsafe-raw-cdp".to_owned(),
                json!({
                    "serverName": "node_repl",
                    "threadId": "thread-1",
                    "mode": "form",
                    "message": "Allow Browser to use full CDP access?",
                    "requestedSchema": {
                        "type": "object",
                        "properties": {}
                    },
                    "_meta": {
                        "codex_approval_kind": "mcp_tool_call",
                        "connector_id": "browser-use",
                        "persist": "always",
                        "tool_name": "access_browser_origin_with_raw_cdp",
                        "origin": "https://user:secret@example.com"
                    }
                }),
            ),
            Err(McpElicitationMapError::Invalid)
        );
    }

    #[test]
    fn browser_resource_rules_and_replies_match_the_official_scope_choices() {
        let request = McpBrowserResourceElicitation {
            request_id: "browser-download".to_owned(),
            task_id: "thread-1".to_owned(),
            turn_id: Some("turn-1".to_owned()),
            server_name: "node_repl".to_owned(),
            source_name: "Browser".to_owned(),
            origin: "https://example.com".to_owned(),
            resource: BrowserPermissionResource::Download,
            message: "Allow download from https://example.com?".to_owned(),
            reason: None,
            persist_session: true,
            persist_always: true,
            elevated_risk: false,
        };
        let mut permissions = BrowserPermissionsState::default();
        assert_eq!(browser_resource_auto_decision(&request, &permissions), None);

        permissions.download_approval_mode = BrowserApprovalMode::NeverAsk;
        assert_eq!(
            browser_resource_auto_decision(&request, &permissions),
            Some(BrowserResourceElicitationDecision::AllowOnce)
        );
        permissions.sites.push(BrowserSitePermission {
            origin: request.origin.clone(),
            browse: BrowserPermissionValue::Default,
            download: BrowserPermissionValue::Allow,
            upload: BrowserPermissionValue::Block,
            full_cdp: BrowserPermissionValue::Default,
        });
        assert_eq!(
            browser_resource_auto_decision(&request, &permissions),
            Some(BrowserResourceElicitationDecision::AlwaysAllow)
        );
        permissions.sites[0].browse = BrowserPermissionValue::Block;
        assert_eq!(
            browser_resource_auto_decision(&request, &permissions),
            Some(BrowserResourceElicitationDecision::Deny)
        );

        let mut raw_cdp = request.clone();
        raw_cdp.resource = BrowserPermissionResource::FullCdp;
        raw_cdp.persist_session = false;
        raw_cdp.elevated_risk = true;
        permissions.sites.clear();
        assert_eq!(
            browser_resource_auto_decision(&raw_cdp, &permissions),
            Some(BrowserResourceElicitationDecision::Deny)
        );
        permissions.full_cdp_access_enabled = true;
        assert_eq!(browser_resource_auto_decision(&raw_cdp, &permissions), None);

        let Ok(session_response) = serde_json::to_value(browser_resource_elicitation_response(
            BrowserResourceElicitationDecision::AllowConversation,
        )) else {
            panic!("Browser conversation response must serialize");
        };
        assert_eq!(
            session_response,
            json!({
                "action": "accept",
                "content": {},
                "_meta": {"persist": "session"}
            })
        );

        let Ok(always_response) = serde_json::to_value(browser_resource_elicitation_response(
            BrowserResourceElicitationDecision::AlwaysAllow,
        )) else {
            panic!("Browser persistent response must serialize");
        };
        assert_eq!(
            always_response,
            json!({
                "action": "accept",
                "content": {},
                "_meta": {"persist": "always"}
            })
        );

        let Ok(deny_response) = serde_json::to_value(browser_resource_elicitation_response(
            BrowserResourceElicitationDecision::Deny,
        )) else {
            panic!("Browser deny response must serialize");
        };
        assert_eq!(deny_response, json!({"action": "decline"}));
    }

    #[test]
    fn pull_request_generation_uses_the_stable_structured_contract() {
        let context = "x".repeat(35_000);
        let prompt = pull_request_generation_prompt(&context, "Follow the pull request template.");
        let (_, bounded_context) = prompt.split_once("Context:\n").unwrap_or_default();
        assert_eq!(bounded_context.chars().count(), 30_000);
        assert!(prompt.contains("Include a Summary section and a Testing section."));
        assert!(prompt.contains("Pull request instructions:\nFollow the pull request template."));
        assert_eq!(
            pull_request_output_schema()["properties"]["title"]["maxLength"],
            120
        );
        assert_eq!(
            pull_request_output_schema()["properties"]["body"]["maxLength"],
            30_000
        );
        let generated = parse_generated_pull_request_message(
            r###"{"title":"Add native PR flow","body":"## Summary\n- Add PR workflow"}"###,
        );
        assert_eq!(
            generated.as_ref().map(|details| details.title.as_str()),
            Some("Add native PR flow")
        );

        let combined = combined_git_generation_prompt("commit and PR context");
        assert!(combined.contains("generate one git commit message plus one pull request"));
        assert_eq!(
            combined_git_output_schema()["required"],
            json!(["message", "title", "body"])
        );
        let generated = parse_generated_commit_pull_request_messages(
            r###"{"message":"Add native PR flow","title":"Add native PR flow","body":"## Summary\n- Add PR workflow"}"###,
        );
        assert_eq!(
            generated
                .as_ref()
                .and_then(|details| details.commit_message.as_deref()),
            Some("Add native PR flow")
        );
    }

    #[test]
    fn terminal_parser_keeps_the_bounded_shell_title() {
        let mut parser =
            vt100::Parser::new_with_callbacks(24, 80, 10, TerminalParserCallbacks::default());
        parser.process(b"\x1b]2;PowerShell\x07");

        assert_eq!(parser.callbacks().title, "PowerShell");
    }

    #[test]
    fn initialize_capabilities_match_the_stable_desktop_contract() {
        let capabilities = initialize_capabilities();
        let expected_methods = STABLE_OPT_OUT_NOTIFICATION_METHODS
            .iter()
            .map(|method| (*method).to_owned())
            .collect::<Vec<_>>();

        assert!(capabilities.experimental_api);
        assert!(!capabilities.request_attestation);
        assert_eq!(capabilities.mcp_server_openai_form_elicitation, Some(true));
        assert_eq!(
            capabilities.opt_out_notification_methods,
            Some(expected_methods)
        );
    }

    #[test]
    fn account_notifications_refresh_the_usage_snapshot() {
        let (events, actions) = bounded(1);

        assert!(!handle_notification(
            "account/rateLimits/updated",
            json!({}),
            &events
        ));
        assert!(matches!(actions.try_recv(), Ok(Action::RefreshAccount)));
    }

    #[test]
    fn external_import_completion_notification_reaches_the_reducer() {
        let (events, actions) = bounded(1);

        assert!(!handle_notification(
            "externalAgentConfig/import/completed",
            json!({
                "importId": "import-1",
                "itemTypeResults": [{
                    "itemType": "SESSIONS",
                    "successes": [{
                        "itemType": "SESSIONS",
                        "target": "thread-1"
                    }],
                    "failures": []
                }]
            }),
            &events
        ));
        assert!(matches!(
            actions.try_recv(),
            Ok(Action::ExternalImportCompleted {
                import_id,
                results,
            }) if import_id == "import-1"
                && results.len() == 1
                && results[0].item_type == ImportItemType::Sessions
                && results[0].successes.len() == 1
        ));
    }

    #[test]
    fn thread_token_usage_notification_keeps_the_last_turn_context_snapshot() {
        let (events, actions) = bounded(1);

        assert!(!handle_notification(
            "thread/tokenUsage/updated",
            json!({
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
            }),
            &events
        ));
        assert!(matches!(
            actions.try_recv(),
            Ok(Action::ThreadTokenUsageUpdated {
                task_id,
                last_total_tokens: 125,
                model_context_window: Some(1000),
            }) if task_id == "thread-1"
        ));
    }

    #[test]
    fn safety_buffering_notification_keeps_only_bounded_retry_metadata() {
        let (events, actions) = bounded(1);

        assert!(!handle_notification(
            "model/safetyBuffering/updated",
            json!({
                "threadId": "thread-1",
                "turnId": "turn-1",
                "model": "gpt-5.6-sol",
                "useCases": ["provider-only-use-case"],
                "reasons": ["provider-only-reason"],
                "showBufferingUi": true,
                "fasterModel": "x".repeat(700)
            }),
            &events
        ));
        assert!(matches!(
            actions.try_recv(),
            Ok(Action::SafetyBufferingUpdated {
                task_id,
                turn_id,
                show_buffering_ui: true,
                faster_model: Some(faster_model),
            }) if task_id == "thread-1"
                && turn_id == "turn-1"
                && faster_model.len() == 512
        ));
    }

    #[test]
    fn model_verification_notification_surfaces_only_the_stable_cyber_warning() {
        let (events, actions) = bounded(1);

        assert!(!handle_notification(
            "model/verification",
            json!({
                "threadId": "thread-1",
                "turnId": "turn-7",
                "verifications": ["trustedAccessForCyber"]
            }),
            &events
        ));
        assert!(matches!(
            actions.try_recv(),
            Ok(Action::UpsertTimelineItem {
                task_id,
                item: TimelineItem {
                    id,
                    turn_id,
                    kind: TimelineKind::Warning,
                    text,
                    sources,
                    completed: true,
                    ..
                },
            }) if task_id == "thread-1"
                && id == "model-verification:turn-7:trusted-access-for-cyber"
                && turn_id == "turn-7"
                && text == TRUSTED_ACCESS_FOR_CYBER_WARNING
                && sources == [TimelineSource {
                    title: "Trusted Access for Cyber".to_owned(),
                    url: TRUSTED_ACCESS_FOR_CYBER_URL.to_owned(),
                }]
        ));

        assert!(!handle_notification(
            "model/verification",
            json!({
                "threadId": "thread-1",
                "turnId": "turn-8",
                "verifications": []
            }),
            &events
        ));
        assert!(actions.try_recv().is_err());
    }

    #[test]
    fn turn_diff_notification_uses_the_typed_bounded_last_turn_action() {
        let (events, actions) = bounded(1);

        assert!(!handle_notification(
            "turn/diff/updated",
            json!({
                "threadId": "thread-1",
                "turnId": "turn-7",
                "diff": "diff --git a/a.txt b/a.txt\n@@ -0,0 +1 @@\n+native"
            }),
            &events
        ));
        assert!(matches!(
            actions.try_recv(),
            Ok(Action::TurnDiffUpdated {
                task_id,
                turn_id,
                diff,
                truncated: false,
            }) if task_id == "thread-1"
                && turn_id == "turn-7"
                && diff.ends_with("+native")
        ));
    }

    #[test]
    fn account_login_completion_uses_typed_bounded_state_without_provider_error() {
        let (events, actions) = bounded(1);

        assert!(!handle_notification(
            "account/login/completed",
            json!({
                "loginId": "x".repeat(700),
                "success": false,
                "error": "provider payload must not reach the UI"
            }),
            &events
        ));
        assert!(matches!(
            actions.try_recv(),
            Ok(Action::AccountLoginCompleted {
                login_id: Some(login_id),
                success: false,
            }) if login_id.len() == 512
        ));
    }

    #[test]
    fn turn_errors_do_not_expose_provider_payloads() {
        let (events, actions) = bounded(1);

        assert!(!handle_notification(
            "error",
            json!({
                "willRetry": false,
                "error": {
                    "message": "provider payload must not reach the UI",
                    "additionalDetails": "request metadata must stay private"
                }
            }),
            &events
        ));
        assert!(matches!(
            actions.try_recv(),
            Ok(Action::SetStatus(message))
                if message
                    == "Codex couldn't complete the request. Try again, or check the account and connection settings."
        ));
    }

    #[test]
    fn editable_user_messages_preserve_supported_local_attachments() {
        #[cfg(windows)]
        let (image_path, mention_path, skill_path) = (
            r"C:\repo\design.png",
            r"C:\repo\README.md",
            r"C:\skills\imagegen\SKILL.md",
        );
        #[cfg(not(windows))]
        let (image_path, mention_path, skill_path) = (
            "/repo/design.png",
            "/repo/README.md",
            "/skills/imagegen/SKILL.md",
        );
        let item = map_timeline_item(
            "turn-1".to_owned(),
            json!({
                "type": "userMessage",
                "id": "message-1",
                "content": [
                    {"type": "text", "text": "Update the design"},
                    {"type": "localImage", "path": image_path},
                    {"type": "mention", "name": "README.md", "path": mention_path},
                    {"type": "skill", "name": "imagegen", "path": skill_path}
                ]
            }),
            true,
        );

        assert_eq!(item.kind, TimelineKind::User);
        assert!(item.edit_supported);
        assert_eq!(item.attachments.len(), 3);
        assert_eq!(
            item.attachments
                .iter()
                .map(|attachment| attachment.kind)
                .collect::<Vec<_>>(),
            [
                ComposerAttachmentKind::LocalImage,
                ComposerAttachmentKind::Mention,
                ComposerAttachmentKind::Skill,
            ]
        );

        let external = map_timeline_item(
            "turn-2".to_owned(),
            json!({
                "type": "userMessage",
                "id": "message-2",
                "content": [{"type": "image", "url": "https://example.com/design.png"}]
            }),
            true,
        );
        assert!(!external.edit_supported);
    }

    #[test]
    fn context_compaction_items_use_the_stable_labels() {
        let running = map_timeline_item(
            "turn-compact".to_owned(),
            json!({"type": "contextCompaction", "id": "compact-1"}),
            false,
        );
        assert_eq!(running.kind, TimelineKind::ContextCompaction);
        assert_eq!(running.text, "Compacting context");

        let completed = map_timeline_item(
            "turn-compact".to_owned(),
            json!({"type": "contextCompaction", "id": "compact-1"}),
            true,
        );
        assert_eq!(completed.kind, TimelineKind::ContextCompaction);
        assert_eq!(completed.text, "Context compacted");
    }

    #[test]
    fn command_and_patch_activity_use_the_stable_compact_summaries() {
        let command = map_timeline_item(
            "turn-activity".to_owned(),
            json!({
                "type": "commandExecution",
                "id": "command-1",
                "command": "pwsh -Command inspect",
                "commandActions": [
                    {"type": "read", "command": "cat README.md", "name": "README.md", "path": "README.md"},
                    {"type": "search", "command": "rg TODO", "query": "TODO", "path": null},
                    {"type": "listFiles", "command": "rg --files", "path": null}
                ],
                "aggregatedOutput": "README.md"
            }),
            true,
        );
        assert_eq!(command.text, "Explored 1 file, 1 search, 1 list");
        let Some(detail) = command.detail.as_deref() else {
            panic!("exploration should preserve bounded command detail");
        };
        assert!(detail.contains("Read README.md"));
        assert!(detail.contains("Searched for TODO"));
        assert!(detail.contains("Listed files"));

        let patch = map_timeline_item(
            "turn-activity".to_owned(),
            json!({
                "type": "fileChange",
                "id": "patch-1",
                "changes": [{
                    "path": "app/page.tsx",
                    "kind": {"type": "update"},
                    "diff": "@@ -1 +1 @@\n-old\n+new"
                }],
                "status": "completed"
            }),
            true,
        );
        assert_eq!(patch.text, "Edited page.tsx +1 -1");
        assert!(patch.detail.as_deref().is_some_and(|detail| {
            detail.contains("app/page.tsx") && detail.contains("@@ -1 +1 @@")
        }));
    }

    #[test]
    fn web_sources_and_memory_citations_keep_only_bounded_structured_fields() {
        let search = map_timeline_item(
            "turn-search".to_owned(),
            json!({
                "type": "webSearch",
                "id": "search-1",
                "query": "native Rust UI",
                "action": {"type": "search", "queries": ["native Rust UI", "GPUI Markdown"]},
                "results": [
                    {"title": "GPUI Component", "url": "https://example.test/gpui", "providerPayload": {"secret": "ignored"}},
                    {"title": "Duplicate", "url": "https://example.test/gpui"},
                    {"title": "Local", "url": "file:///C:/private.txt"}
                ]
            }),
            true,
        );
        assert_eq!(search.kind, TimelineKind::WebSearch);
        assert_eq!(search.text, "native Rust UI");
        assert_eq!(search.sources.len(), 1);
        assert_eq!(search.sources[0].title, "GPUI Component");
        assert_eq!(search.sources[0].url, "https://example.test/gpui");
        assert!(
            search
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("GPUI Markdown"))
        );

        let response = map_timeline_item(
            "turn-search".to_owned(),
            json!({
                "type": "agentMessage",
                "id": "answer-1",
                "text": "Answer",
                "memoryCitation": {
                    "entries": [{
                        "path": "memory/project.md",
                        "lineStart": 4,
                        "lineEnd": 8,
                        "note": "Project convention"
                    }]
                }
            }),
            true,
        );
        assert_eq!(response.memory_citations.len(), 1);
        assert_eq!(response.memory_citations[0].line_start, 4);
        assert_eq!(response.memory_citations[0].line_end, 8);
        assert_eq!(response.memory_citations[0].note, "Project convention");
    }

    #[test]
    fn specialized_activity_matches_stable_subagent_image_and_background_contracts() {
        let collab = map_timeline_item(
            "turn-agents".to_owned(),
            json!({
                "type": "collabAgentToolCall",
                "id": "collab-1",
                "tool": "spawnAgent",
                "status": "completed",
                "receiverThreadIds": ["019f9347-ea06-7541-bf27-83ff94790120"],
                "prompt": "Inspect the native renderer.",
                "agentsStates": {
                    "019f9347-ea06-7541-bf27-83ff94790120": {
                        "status": "running",
                        "message": "Reviewing UI"
                    }
                }
            }),
            true,
        );
        assert_eq!(collab.kind, TimelineKind::Subagent);
        assert_eq!(collab.text, "Created 1 agent");
        assert_eq!(
            collab.detail.as_deref(),
            Some("Created @agent-019f9347 with the instructions: Inspect the native renderer.")
        );

        let activity = map_timeline_item(
            "turn-agents".to_owned(),
            json!({
                "type": "subAgentActivity",
                "id": "activity-1",
                "kind": "started",
                "agentThreadId": "019f9347-ea06-7541-bf27-83ff94790120",
                "agentPath": "/root/ui_worker"
            }),
            true,
        );
        assert_eq!(activity.kind, TimelineKind::Subagent);
        assert_eq!(activity.text, "Ui worker started working");

        let image = map_timeline_item(
            "turn-image".to_owned(),
            json!({
                "type": "imageGeneration",
                "id": "image-1",
                "status": "completed",
                "revisedPrompt": "A native Rust window",
                "result": "raw-provider-payload-must-not-render",
                "savedPath": "artifacts/native-window.png"
            }),
            true,
        );
        assert_eq!(image.kind, TimelineKind::Image);
        assert_eq!(image.text, "Generated image");
        assert!(
            image
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("artifacts/native-window.png"))
        );
        assert!(
            !image
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("raw-provider-payload"))
        );

        let command = map_timeline_item(
            "turn-command".to_owned(),
            json!({
                "type": "commandExecution",
                "id": "command-1",
                "command": "cargo watch",
                "processId": "process-7",
                "status": "inProgress",
                "commandActions": []
            }),
            false,
        );
        assert_eq!(command.process_id.as_deref(), Some("process-7"));

        assert!(is_hidden_timeline_item(&json!({
            "type": "collabAgentToolCall",
            "tool": "wait"
        })));
        assert!(is_hidden_timeline_item(&json!({"type": "sleep"})));
        assert!(!is_hidden_timeline_item(&json!({
            "type": "collabAgentToolCall",
            "tool": "sendInput"
        })));
    }

    #[test]
    fn output_artifacts_follow_stable_directives_edits_and_generated_images() {
        let response = map_timeline_item(
            "turn-output".to_owned(),
            json!({
                "type": "agentMessage",
                "id": "answer-output",
                "text": concat!(
                    "Done.\n",
                    "::codex-file-citation{path=\"reports/final.md\" purpose=\"output\"}\n",
                    "::codex-file-citation{path=\"reports/final.md\" purpose=\"output\"}\n",
                    "::codex-file-citation{path=\"work/private.csv\" purpose=\"output\"}\n",
                    "::codex-file-citation{path=\"reports/run.exe\" purpose=\"output\"}\n",
                    "::codex-file-citation{path=\"src/lib.rs\" line_range_start=\"1\" line_range_end=\"2\"}"
                )
            }),
            true,
        );
        assert_eq!(response.output_artifacts.len(), 1);
        assert_eq!(
            response.output_artifacts[0].path,
            PathBuf::from("reports/final.md")
        );
        assert_eq!(response.output_artifacts[0].kind, OutputArtifactKind::File);

        let file_change = map_timeline_item(
            "turn-output".to_owned(),
            json!({
                "type": "fileChange",
                "id": "change-output",
                "changes": [
                    {"path": "reports/data.xlsx"},
                    {"path": "src/lib.rs"}
                ]
            }),
            true,
        );
        assert_eq!(file_change.output_artifacts.len(), 1);
        assert_eq!(
            file_change.output_artifacts[0].path,
            PathBuf::from("reports/data.xlsx")
        );

        let image = map_timeline_item(
            "turn-output".to_owned(),
            json!({
                "type": "imageGeneration",
                "id": "generated-output",
                "status": "inProgress",
                "savedPath": "reports/cover.png"
            }),
            false,
        );
        assert_eq!(image.output_artifacts.len(), 1);
        assert_eq!(
            image.output_artifacts[0].kind,
            OutputArtifactKind::GeneratedImage
        );
    }

    #[test]
    fn plugin_directory_tabs_use_the_stable_marketplace_contract() {
        assert_eq!(
            plugin_directory_marketplace_kinds(PluginDirectoryTab::CuratedByOpenAi),
            None
        );
        assert_eq!(
            plugin_directory_marketplace_kinds(PluginDirectoryTab::SharedWithYou),
            Some(vec![PluginListMarketplaceKind::SharedWithMe])
        );
        assert_eq!(
            plugin_directory_marketplace_kinds(PluginDirectoryTab::CreatedByMe),
            Some(vec![PluginListMarketplaceKind::CreatedByMeRemote])
        );
        assert_eq!(
            plugin_directory_marketplace_kinds(PluginDirectoryTab::Workspace),
            Some(vec![PluginListMarketplaceKind::WorkspaceDirectory])
        );
        assert_eq!(
            plugin_directory_marketplace_kinds(PluginDirectoryTab::Local),
            Some(vec![PluginListMarketplaceKind::Local])
        );
        assert!(plugin_directory_includes_marketplace(
            PluginDirectoryTab::CuratedByOpenAi,
            "openai-primary-runtime"
        ));
        assert!(!plugin_directory_includes_marketplace(
            PluginDirectoryTab::CuratedByOpenAi,
            "personal-marketplace"
        ));
    }

    #[test]
    fn app_logos_enrich_only_unambiguous_plugin_names() {
        let mut logos = HashMap::new();
        let mut ambiguous = HashSet::new();
        let first = AppLogo {
            light: Some("https://example.com/alpha.png".to_owned()),
            dark: None,
        };
        index_app_logos(
            &mut logos,
            &mut ambiguous,
            [" Alpha ".to_owned()],
            first.clone(),
        );
        assert_eq!(logos.get("alpha"), Some(&first));

        index_app_logos(
            &mut logos,
            &mut ambiguous,
            ["ALPHA".to_owned()],
            AppLogo {
                light: Some("https://example.com/other.png".to_owned()),
                dark: None,
            },
        );
        assert!(!logos.contains_key("alpha"));
        assert!(ambiguous.contains("alpha"));
    }

    #[test]
    fn app_list_updates_keep_connectable_and_connected_apps() {
        let apps = map_apps(vec![
            AppInfo {
                id: "connected".to_owned(),
                name: "Connected".to_owned(),
                description: Some("Ready to use".to_owned()),
                logo_url: None,
                logo_url_dark: None,
                install_url: None,
                is_accessible: true,
                is_enabled: false,
                plugin_display_names: Vec::new(),
            },
            AppInfo {
                id: "discoverable".to_owned(),
                name: "Discoverable".to_owned(),
                description: None,
                logo_url: None,
                logo_url_dark: None,
                install_url: None,
                is_accessible: false,
                is_enabled: true,
                plugin_display_names: Vec::new(),
            },
        ]);
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].id, "connected");
        assert!(apps[0].is_accessible);
        assert!(!apps[0].enabled);
        assert_eq!(apps[1].id, "discoverable");
        assert!(!apps[1].is_accessible);

        let (sender, receiver) = bounded(1);
        assert!(!handle_notification(
            "app/list/updated",
            json!({
                "data": [{
                    "id": "connected",
                    "name": "Connected",
                    "isAccessible": true,
                    "isEnabled": true
                }]
            }),
            &sender,
        ));
        let Ok(action) = receiver.try_recv() else {
            panic!("expected apps update action");
        };
        match action {
            Action::AppsLoaded(apps) => {
                assert_eq!(apps.len(), 1);
                assert_eq!(apps[0].id, "connected");
                assert!(apps[0].enabled);
            }
            _ => panic!("expected apps update"),
        }
    }

    #[test]
    fn computer_use_policy_uses_the_highest_precedence_enabled_layer() {
        let config = serde_json::from_value::<ConfigReadResponse>(json!({
            "config": {},
            "origins": {},
            "layers": [
                {
                    "config": {
                        "computer_use": {
                            "windows": {
                                "always_allowed_app_ids": {
                                    "blocked.exe": true
                                }
                            }
                        }
                    },
                    "disabledReason": "admin disabled"
                },
                {
                    "config": {
                        "computer_use": {
                            "windows": {
                                "always_allowed_app_ids": {
                                    " MSPaint.EXE ": true,
                                    "mspaint.exe": true,
                                    "NOTEPAD.EXE": true,
                                    "disabled.exe": false
                                }
                            }
                        }
                    }
                },
                {
                    "config": {
                        "computer_use": {
                            "windows": {
                                "always_allowed_app_ids": ["lower-priority.exe"]
                            }
                        }
                    }
                }
            ]
        }));
        let Ok(config) = config else {
            panic!("expected a valid config response fixture");
        };

        assert_eq!(
            computer_use_allowed_app_ids(&config),
            ["mspaint.exe", "notepad.exe"]
        );
    }

    #[test]
    fn personalization_snapshot_follows_stable_defaults_and_legacy_fallbacks() {
        let config = serde_json::from_value::<ConfigReadResponse>(json!({
            "config": {
                "model_personality": "pragmatic",
                "features": {
                    "memories": true
                },
                "memories": {
                    "no_memories_if_mcp_or_web_search": true
                }
            },
            "origins": {}
        }));
        let Ok(config) = config else {
            panic!("expected a valid personalization config fixture");
        };
        let snapshot = personalization_snapshot(&config);
        assert_eq!(snapshot.personality, Personality::Pragmatic);
        assert!(snapshot.memory_available);
        assert!(snapshot.generate_memories);
        assert!(snapshot.use_memories);
        assert!(snapshot.memories_enabled);
        assert!(!snapshot.allow_memory_generation_from_tool_assisted_chats);

        let current = serde_json::from_value::<ConfigReadResponse>(json!({
            "config": {
                "personality": "friendly",
                "model_personality": "pragmatic",
                "memories": {
                    "generate_memories": false,
                    "use_memories": true,
                    "disable_on_external_context": false,
                    "no_memories_if_mcp_or_web_search": true
                }
            },
            "origins": {}
        }));
        let Ok(current) = current else {
            panic!("expected a valid personalization config fixture");
        };
        let snapshot = personalization_snapshot(&current);
        assert_eq!(snapshot.personality, Personality::Friendly);
        assert!(!snapshot.memory_available);
        assert!(!snapshot.generate_memories);
        assert!(snapshot.use_memories);
        assert!(!snapshot.memories_enabled);
        assert!(snapshot.allow_memory_generation_from_tool_assisted_chats);
    }

    #[test]
    fn agent_configuration_snapshot_preserves_scopes_restrictions_and_managed_origins() {
        let root = if cfg!(windows) {
            PathBuf::from(r"C:\isolated\repo")
        } else {
            PathBuf::from("/isolated/repo")
        };
        let dot_codex = root.join(".codex");
        let user_config = root.join("user-config.toml");
        let managed_config = root.join("managed-config.toml");
        let config = serde_json::from_value::<ConfigReadResponse>(json!({
            "config": {
                "approval_policy": "on-request",
                "sandbox_mode": "workspace-write",
                "sandbox_workspace_write": {
                    "network_access": true
                }
            },
            "origins": {
                "sandbox_mode": {
                    "name": {
                        "type": "system",
                        "file": managed_config
                    },
                    "version": "sha256:managed"
                }
            },
            "layers": [
                {
                    "name": {
                        "type": "project",
                        "dotCodexFolder": dot_codex
                    },
                    "version": "sha256:project",
                    "config": {
                        "sandbox_mode": "workspace-write",
                        "sandbox_workspace_write": {
                            "network_access": false
                        }
                    }
                },
                {
                    "name": {
                        "type": "user",
                        "file": user_config
                    },
                    "version": "sha256:user",
                    "config": {
                        "approval_policy": "untrusted"
                    }
                },
                {
                    "name": {
                        "type": "system",
                        "file": managed_config
                    },
                    "version": "sha256:managed",
                    "config": {
                        "sandbox_mode": "workspace-write"
                    }
                }
            ]
        }));
        let requirements =
            serde_json::from_value::<codex_protocol::ConfigRequirementsReadResponse>(json!({
                "requirements": {
                    "allowedApprovalPolicies": ["on-request", "never"],
                    "allowedSandboxModes": ["read-only", "workspace-write"]
                }
            }));
        let (Ok(config), Ok(requirements)) = (config, requirements) else {
            panic!("expected valid configuration fixtures");
        };

        let snapshot = agent_configuration_snapshot(&config, requirements.requirements.as_ref());
        assert_eq!(snapshot.scopes.len(), 3);
        assert_eq!(snapshot.scopes[0].kind, AgentConfigScopeKind::Project);
        assert_eq!(snapshot.scopes[0].label, "repo");
        assert_eq!(
            snapshot.scopes[0].sandbox_mode.as_deref(),
            Some("workspace-write")
        );
        assert_eq!(snapshot.scopes[0].network_access, Some(false));
        assert_eq!(snapshot.scopes[1].kind, AgentConfigScopeKind::User);
        assert_eq!(
            snapshot.scopes[1].approval_policy.as_deref(),
            Some("untrusted")
        );
        assert_eq!(snapshot.scopes[2].kind, AgentConfigScopeKind::Managed);
        assert_eq!(snapshot.effective_approval_policy, "on-request");
        assert_eq!(snapshot.effective_sandbox_mode, "workspace-write");
        assert!(snapshot.effective_network_access);
        assert_eq!(snapshot.allowed_approval_policies, ["on-request", "never"]);
        assert_eq!(
            snapshot.allowed_sandbox_modes,
            ["read-only", "workspace-write"]
        );
        assert!(!snapshot.approval_managed);
        assert!(snapshot.sandbox_managed);
        assert!(!snapshot.network_managed);
    }

    #[test]
    fn computer_use_policy_accepts_documented_array_and_writes_stable_table() {
        let config = serde_json::from_value::<ConfigReadResponse>(json!({
            "config": {},
            "origins": {},
            "layers": [{
                "config": {
                    "computer_use": {
                        "windows": {
                            "always_allowed_app_ids": [" MSPaint.EXE ", "mspaint.exe"]
                        }
                    }
                }
            }]
        }));
        let Ok(config) = config else {
            panic!("expected a valid config response fixture");
        };

        assert_eq!(computer_use_allowed_app_ids(&config), ["mspaint.exe"]);
        assert_eq!(
            computer_use_allowed_app_ids_value(&[
                " MSPaint.EXE ".to_owned(),
                "NOTEPAD.EXE".to_owned()
            ]),
            json!({
                "mspaint.exe": true,
                "notepad.exe": true
            })
        );
    }

    #[test]
    fn computer_use_authorization_is_scoped_to_the_real_app_identifier() {
        let permission = ComputerUsePermission {
            enabled: true,
            authorized_application_id: Some(r"process:c:\windows\system32\mspaint.exe".to_owned()),
            input_authorized: true,
        };
        let always_allowed = HashSet::from(["notepad.exe".to_owned()]);

        assert!(computer_use_app_authorized(
            &permission,
            &always_allowed,
            r"process:c:\windows\system32\mspaint.exe"
        ));
        assert!(computer_use_app_authorized(
            &permission,
            &always_allowed,
            r"process:c:\windows\system32\notepad.exe"
        ));
        assert!(!computer_use_app_authorized(
            &permission,
            &always_allowed,
            r"process:c:\windows\system32\calc.exe"
        ));
        assert!(!computer_use_app_authorized(
            &permission,
            &HashSet::from([r"c:\tools\notepad.exe".to_owned()]),
            r"process:c:\windows\system32\notepad.exe"
        ));
    }

    #[test]
    fn computer_use_app_usage_matches_the_stable_wire_shape() {
        let application = ComputerApplication {
            id: "apple.itunes".to_owned(),
            display_name: Some("iTunes".to_owned()),
            last_used_date: Some("2024-01-02".to_owned()),
            use_count: Some(17),
            is_running: false,
            windows: Vec::new(),
        };

        assert_eq!(
            computer_application_value(&application),
            json!({
                "id": "apple.itunes",
                "displayName": "iTunes",
                "lastUsedDate": "2024-01-02",
                "useCount": 17,
                "isRunning": false,
                "windows": []
            })
        );
    }

    #[test]
    fn computer_use_product_policy_cannot_be_bypassed_by_user_approval() {
        assert_eq!(
            forbidden_computer_target_message(
                r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
                "Windows PowerShell"
            ),
            Some(
                "Computer Use cannot operate on “Windows PowerShell”; product policy blocks this app"
                    .to_owned()
            )
        );
        assert_eq!(
            forbidden_computer_target_message(r"C:\Windows\System32\mspaint.exe", "Paint"),
            None
        );
    }

    #[test]
    fn computer_use_drag_matches_the_official_coordinate_contract() {
        let Ok(tools) = serde_json::to_value(computer_use_dynamic_tools()) else {
            panic!("Computer Use tools must serialize");
        };
        let drag = tools
            .pointer("/0/tools")
            .and_then(Value::as_array)
            .and_then(|tools| {
                tools
                    .iter()
                    .find(|tool| tool.get("name") == Some(&json!("drag")))
            });
        assert_eq!(
            drag.and_then(|tool| tool.pointer("/inputSchema/required")),
            Some(&json!(["from_x", "from_y", "to_x", "to_y", "window"]))
        );
        assert_eq!(
            drag.and_then(|tool| tool.pointer("/inputSchema/properties/from_x/type")),
            Some(&json!("number"))
        );
        assert_eq!(
            drag_coordinates(&json!({
                "from_x": 10.4,
                "from_y": 20.5,
                "to_x": -1.5,
                "to_y": -1.6
            })),
            Ok((10, 21, -1, -2))
        );
    }

    #[test]
    fn computer_use_tools_match_the_official_action_names_and_key_chords() {
        let Ok(tools) = serde_json::to_value(computer_use_dynamic_tools()) else {
            panic!("Computer Use tools must serialize");
        };
        let names = tools
            .pointer("/0/tools")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            [
                "list_windows",
                "get_window",
                "list_apps",
                "launch_app",
                "get_window_state",
                "click",
                "press_key",
                "type_text",
                "scroll",
                "set_value",
                "drag",
                "perform_secondary_action",
                "activate_window"
            ]
        );
        let get_window_state = tools
            .pointer("/0/tools")
            .and_then(Value::as_array)
            .and_then(|tools| {
                tools
                    .iter()
                    .find(|tool| tool.get("name") == Some(&json!("get_window_state")))
            });
        assert_eq!(
            get_window_state.and_then(|tool| tool.pointer("/inputSchema/required")),
            Some(&json!(["window"]))
        );
        let expected_window_schema = computer_window_schema();
        assert_eq!(
            get_window_state.and_then(|tool| tool.pointer("/inputSchema/properties/window")),
            Some(&expected_window_schema)
        );
        let type_text = tools
            .pointer("/0/tools")
            .and_then(Value::as_array)
            .and_then(|tools| {
                tools
                    .iter()
                    .find(|tool| tool.get("name") == Some(&json!("type_text")))
            });
        assert_eq!(
            type_text.and_then(|tool| tool.pointer("/inputSchema/properties/text/minLength")),
            None
        );
        assert_eq!(
            computer_window_argument(&json!({
                "window": {
                    "app": r"process:C:\Windows\System32\notepad.exe",
                    "id": 42,
                    "title": "Notes"
                }
            })),
            Ok((
                "42".to_owned(),
                r"process:C:\Windows\System32\notepad.exe".to_owned()
            ))
        );
        assert_eq!(
            parse_computer_key_chord("Control_L+Shift_L+period"),
            Ok((
                ComputerKey::Character('.'),
                vec![ComputerKey::Control, ComputerKey::Shift]
            ))
        );
        assert_eq!(
            parse_computer_key_chord("KP_0"),
            Ok((ComputerKey::Numpad0, Vec::new()))
        );
        assert_eq!(
            parse_computer_key_chord("Numpad_Add"),
            Ok((ComputerKey::NumpadAdd, Vec::new()))
        );
        assert_eq!(
            parse_computer_key_chord("KP_Enter"),
            Ok((ComputerKey::NumpadEnter, Vec::new()))
        );
        assert!(parse_computer_key_chord("Windows+R").is_err());
    }

    #[test]
    fn linux_computer_use_schema_is_screenshot_observation_only() {
        let Ok(tools) = serde_json::to_value(linux_computer_use_dynamic_tools()) else {
            panic!("Linux Computer Use tools must serialize");
        };
        let Some(tools) = tools.pointer("/0/tools").and_then(Value::as_array) else {
            panic!("Linux Computer Use namespace must contain tools");
        };
        let names = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "list_windows",
                "get_window",
                "list_apps",
                "get_window_state"
            ]
        );

        let Some(state) = tools
            .iter()
            .find(|tool| tool.get("name") == Some(&json!("get_window_state")))
        else {
            panic!("Linux state tool must exist");
        };
        assert_eq!(
            state.pointer("/inputSchema/required"),
            Some(&json!(["include_screenshot", "window"]))
        );
        assert_eq!(
            state.pointer("/inputSchema/properties/include_text"),
            Some(&json!({"enum": [false], "default": false}))
        );
        assert_eq!(
            state.pointer("/inputSchema/properties/include_screenshot"),
            Some(&json!({"enum": [true]}))
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_computer_use_selection_and_router_are_read_only() {
        assert!(computer_use_dynamic_tools_for_platform_with_available(true).is_some());
        assert!(computer_use_dynamic_tools_for_platform_with_available(false).is_none());
        for tool in [
            "list_windows",
            "get_window",
            "list_apps",
            "get_window_state",
        ] {
            assert!(computer_use_tool_supported_on_platform(tool));
        }
        for tool in [
            "launch_app",
            "click",
            "press_key",
            "type_text",
            "scroll",
            "set_value",
            "drag",
            "perform_secondary_action",
            "activate_window",
        ] {
            assert!(!computer_use_tool_supported_on_platform(tool));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_window_state_rejects_text_and_non_screenshot_requests() {
        let (events, _event_rx) = bounded(1);
        let mut computer_accessibility = ComputerUseAccessibilityClient::new();
        assert_eq!(
            run_computer_tool(
                "get_window_state",
                &json!({"include_text": true, "include_screenshot": true}),
                "thread-1",
                "7",
                &events,
                &mut computer_accessibility,
            ),
            Err("Linux Computer Use observation does not support include_text".to_owned())
        );
        assert_eq!(
            run_computer_tool(
                "get_window_state",
                &json!({"include_screenshot": false}),
                "thread-1",
                "7",
                &events,
                &mut computer_accessibility,
            ),
            Err("Linux Computer Use observation requires include_screenshot=true".to_owned())
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_computer_use_approval_copy_remains_unchanged() {
        assert_eq!(
            computer_use_approval_detail("Paint"),
            "ChatGPT can see and control “Paint” on your computer. Allow once for this task or always allow this app."
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_computer_use_approval_is_task_only_observation() {
        assert_eq!(
            computer_use_approval_detail("Terminal"),
            "ChatGPT can observe “Terminal” on your computer. Allow once for this task."
        );
    }

    #[test]
    fn mcp_oauth_completion_uses_the_typed_notification() {
        let (sender, receiver) = bounded(1);
        assert!(!handle_notification(
            "mcpServer/oauthLogin/completed",
            json!({
                "name": "calendar",
                "threadId": null,
                "success": false,
                "error": "provider token=secret"
            }),
            &sender,
        ));
        let Ok(action) = receiver.try_recv() else {
            panic!("expected MCP authentication action");
        };
        assert!(matches!(
            action,
            Action::McpServerAuthenticationCompleted {
                name,
                success: false,
                error: Some(error),
            } if name == "calendar"
                && error == "MCP server authentication failed. Try again."
        ));
    }

    #[test]
    fn mcp_editor_and_startup_notifications_match_the_stable_contract() {
        let value = mcp_server_config_value(
            None,
            &McpServerDraft {
                name: "remote".to_owned(),
                transport: McpTransportKind::StreamableHttp,
                command: String::new(),
                args: Vec::new(),
                env: Vec::new(),
                env_vars: Vec::new(),
                cwd: String::new(),
                url: "https://mcp.example.com/mcp".to_owned(),
                bearer_token_env_var: "MCP_TOKEN".to_owned(),
                http_headers: vec![("X-Team".to_owned(), "Codex".to_owned())],
                env_http_headers: vec![("Authorization".to_owned(), "AUTH_HEADER".to_owned())],
            },
        );
        assert_eq!(
            value,
            json!({
                "enabled": true,
                "url": "https://mcp.example.com/mcp",
                "bearer_token_env_var": "MCP_TOKEN",
                "http_headers": { "X-Team": "Codex" },
                "env_http_headers": { "Authorization": "AUTH_HEADER" }
            })
        );

        let (sender, receiver) = bounded(1);
        assert!(!handle_notification(
            "mcpServer/startupStatus/updated",
            json!({
                "threadId": null,
                "name": "remote",
                "status": "failed",
                "error": "OAuth token expired: secret=do-not-expose",
                "failureReason": "reauthenticationRequired"
            }),
            &sender,
        ));
        let Ok(action) = receiver.try_recv() else {
            panic!("expected MCP startup action");
        };
        assert!(matches!(
            action,
            Action::McpServerStartupStatusUpdated {
                name,
                status: McpServerStartupState::Failed,
                error: Some(error),
                failure_reason: Some(
                    McpServerStartupFailureReason::ReauthenticationRequired
                ),
            } if name == "remote"
                && error == "MCP server could not start. Check its configuration and try again."
        ));
    }

    #[test]
    fn composer_default_keys_follow_the_active_config_profile() {
        assert_eq!(composer_config_key(None, "model"), "model");
        assert_eq!(
            composer_config_key(Some("work"), "model_reasoning_effort"),
            "profiles.work.model_reasoning_effort"
        );
        assert_eq!(
            composer_config_key(Some("work"), "service_tier"),
            "profiles.work.service_tier"
        );
    }

    #[test]
    fn git_refreshes_are_coalesced_until_the_debounce_expires() {
        let mut debouncer = GitRefreshDebouncer::default();
        let start = Instant::now();
        let delay = Duration::from_millis(300);

        debouncer.schedule(1, PathBuf::from("first"), start, delay);
        debouncer.schedule(
            2,
            PathBuf::from("latest"),
            start + Duration::from_millis(100),
            delay,
        );

        assert_eq!(debouncer.take_due(start + Duration::from_millis(399)), None);
        assert_eq!(
            debouncer.take_due(start + Duration::from_millis(400)),
            Some((2, PathBuf::from("latest")))
        );
        assert_eq!(debouncer.take_due(start + Duration::from_millis(500)), None);
    }

    #[test]
    fn app_server_reconnect_uses_bounded_exponential_backoff_and_deduplicates() {
        let mut scheduler = AppServerReconnectScheduler::default();
        let start = Instant::now();

        for (attempt, delay_secs) in [(1, 1), (2, 2), (3, 4), (4, 8), (5, 16), (6, 20), (7, 20)] {
            let delay = Duration::from_secs(delay_secs);
            assert_eq!(scheduler.schedule(start), Some((attempt, delay)));
            assert_eq!(scheduler.schedule(start), None);
            assert_eq!(
                scheduler.take_due(start + delay - Duration::from_millis(1)),
                None
            );
            assert_eq!(scheduler.take_due(start + delay), Some(attempt));
        }

        scheduler.reset();
        assert_eq!(scheduler.schedule(start), Some((1, Duration::from_secs(1))));
    }

    #[test]
    fn task_search_keeps_only_the_latest_query_until_the_debounce_expires() {
        let mut debouncer = TaskSearchDebouncer::default();
        let start = Instant::now();

        debouncer.schedule(1, "native".to_owned(), start, TASK_SEARCH_DEBOUNCE);
        debouncer.schedule(
            2,
            "native ui".to_owned(),
            start + Duration::from_millis(40),
            TASK_SEARCH_DEBOUNCE,
        );

        assert_eq!(debouncer.take_due(start + Duration::from_millis(139)), None);
        assert_eq!(
            debouncer.take_due(start + Duration::from_millis(140)),
            Some((2, "native ui".to_owned()))
        );
        assert_eq!(debouncer.take_due(start + Duration::from_millis(240)), None);
    }

    #[test]
    fn goal_continuations_keep_the_first_exact_delay_and_deduplicate() {
        let mut scheduler = GoalContinuationScheduler::default();
        let start = Instant::now();

        assert!(
            scheduler
                .schedule("thread-1".to_owned(), start, GOAL_CONTINUATION_DELAY)
                .is_ok()
        );
        assert!(
            scheduler
                .schedule(
                    "thread-1".to_owned(),
                    start + Duration::from_millis(100),
                    GOAL_CONTINUATION_DELAY
                )
                .is_ok()
        );

        assert!(
            scheduler
                .take_due(start + Duration::from_millis(249))
                .is_empty()
        );
        assert_eq!(
            scheduler.take_due(start + Duration::from_millis(250)),
            ["thread-1".to_owned()]
        );
        assert!(
            scheduler
                .take_due(start + Duration::from_secs(1))
                .is_empty()
        );
    }
}
