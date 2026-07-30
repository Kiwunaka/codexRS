use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde::Deserialize;

use crate::process::{BoundedOutput, ProcessError, run_bounded};

const MAX_GH_STDOUT_BYTES: usize = 64 * 1024;
const MAX_GH_SEARCH_STDOUT_BYTES: usize = 512 * 1024;
const MAX_GH_DETAIL_STDOUT_BYTES: usize = 256 * 1024;
const MAX_GH_DIFF_STDOUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_GH_TIMELINE_STDOUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_GH_STDERR_BYTES: usize = 16 * 1024;
const MAX_GH_URL_BYTES: usize = 8 * 1024;
const MAX_GH_TITLE_CHARS: usize = 120;
const MAX_GH_BODY_CHARS: usize = 30_000;
const MAX_GH_ACTIVITY_BODY_CHARS: usize = 16 * 1024;
const MAX_GH_SEARCH_CHARS: usize = 256;
const MAX_GH_CURSOR_BYTES: usize = 1_024;
const MAX_GH_SEARCH_PAGE_SIZE: usize = 100;
const MAX_GH_TIMELINE_ITEMS: usize = 1_000;
const MAX_GH_TIMELINE_PAGES: usize = 10;
const GH_VERSION_TIMEOUT: Duration = Duration::from_secs(1);
const GH_AUTH_TIMEOUT: Duration = Duration::from_secs(10);
const GH_OPERATION_TIMEOUT: Duration = Duration::from_secs(15);
const GH_SEARCH_TIMEOUT: Duration = Duration::from_secs(30);
const GITHUB_HOSTNAME: &str = "github.com";
const PULL_REQUEST_SEARCH_QUERY: &str = r#"
query($searchQuery: String!, $first: Int!, $after: String) {
  search(query: $searchQuery, type: ISSUE, first: $first, after: $after) {
    issueCount
    nodes {
      __typename
      ... on PullRequest {
        additions
        author { avatarUrl(size: 48) login }
        baseRefName
        createdAt
        deletions
        headRefName
        id
        isDraft
        mergeStateStatus
        mergeable
        number
        repository { name owner { login } }
        state
        statusCheckRollup {
          contexts(first: 100) {
            totalCount
            nodes {
              __typename
              ... on CheckRun { conclusion name startedAt status }
              ... on StatusContext { context createdAt state }
            }
          }
        }
        title
        updatedAt
        url
      }
    }
    pageInfo { endCursor hasNextPage }
  }
}
"#;
const PULL_REQUEST_DETAIL_FIELDS: &str = "additions,author,baseRefName,body,createdAt,\
comments,deletions,headRefName,headRefOid,isDraft,mergedAt,mergedBy,mergeStateStatus,mergeable,\
number,reviews,reviewDecision,state,statusCheckRollup,title,updatedAt,url";
const PULL_REQUEST_REVIEW_ACTIVITY_QUERY: &str = r#"
query($owner:String!,$repo:String!,$number:Int!,$after:String){
  repository(owner:$owner,name:$repo){
    pullRequest(number:$number){
      comments(first:100){
        nodes{author{login avatarUrl(size:48)} body createdAt id url}
        pageInfo{hasNextPage}
      }
      latestReviews(first:100){
        nodes{author{login} state comments(first:1){totalCount}}
        pageInfo{hasNextPage}
      }
      reviewThreads(first:100,after:$after){
        nodes{
          id
          comments(first:100){
            nodes{author{login avatarUrl(size:48)} body createdAt id url}
            pageInfo{hasNextPage}
          }
          diffSide
          isResolved
          line
          originalLine
          originalStartLine
          path
          startDiffSide
          startLine
        }
        pageInfo{hasNextPage endCursor}
      }
    }
  }
}
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHubCliAvailability {
    Available,
    Missing,
    AuthenticationRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GitHubPullRequestRelationship {
    All,
    Authored,
    #[default]
    ReviewRequested,
    Reviewed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GitHubPullRequestLifecycle {
    All,
    #[default]
    Open,
    Merged,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHubPullRequestState {
    Open,
    Closed,
    Merged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GitHubCiStatus {
    #[default]
    None,
    Pending,
    Passing,
    Failing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubUser {
    pub login: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubPullRequestIdentity {
    pub hostname: String,
    pub owner: String,
    pub repository: String,
    pub number: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubPullRequestSummary {
    pub identity: GitHubPullRequestIdentity,
    pub node_id: String,
    pub title: String,
    pub url: String,
    pub state: GitHubPullRequestState,
    pub is_draft: bool,
    pub author_login: Option<String>,
    pub base_branch: String,
    pub head_branch: String,
    pub additions: u64,
    pub deletions: u64,
    pub created_at: String,
    pub updated_at: String,
    pub ci_status: GitHubCiStatus,
    pub is_author: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GitHubPullRequestSearchFilters {
    pub relationship: GitHubPullRequestRelationship,
    pub lifecycle: GitHubPullRequestLifecycle,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubPullRequestSearchPage {
    pub account: GitHubUser,
    pub items: Vec<GitHubPullRequestSummary>,
    pub total_count: u64,
    pub next_cursor: Option<String>,
    pub has_next_page: bool,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubPullRequestDetail {
    pub summary: GitHubPullRequestSummary,
    pub body: String,
    pub head_revision: String,
    pub review_decision: Option<String>,
    pub mergeable: Option<String>,
    pub merge_state_status: Option<String>,
    pub checks: Vec<GitHubPullRequestCheck>,
    pub activity: Vec<GitHubPullRequestActivity>,
    pub checks_partial: bool,
    pub activity_partial: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHubPullRequestActivityKind {
    Event,
    Comment,
    Review,
    ReviewComment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubPullRequestActivity {
    pub id: String,
    pub kind: GitHubPullRequestActivityKind,
    pub actor_login: Option<String>,
    pub body: String,
    pub created_at: String,
    pub event: Option<String>,
    pub url: Option<String>,
    pub path: Option<String>,
    pub line: Option<u64>,
    pub start_line: Option<u64>,
    pub review_thread_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHubCheckStatus {
    Pending,
    Passing,
    Failing,
    Neutral,
    Skipped,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHubPullRequestReviewEvent {
    Approve,
    Comment,
    RequestChanges,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHubPullRequestReviewState {
    Draft,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHubPullRequestMergeMethod {
    Merge,
    Squash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubPullRequestCheck {
    pub name: String,
    pub workflow: Option<String>,
    pub status: GitHubCheckStatus,
    pub description: Option<String>,
    pub link: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubPullRequestDiff {
    pub head_revision: String,
    pub unified_diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubPullRequest {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub base_branch: String,
    pub head_branch: String,
    pub is_draft: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubPullRequestStatus {
    pub availability: GitHubCliAvailability,
    pub pull_request: Option<GitHubPullRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubCreatePullRequest {
    pub head_branch: String,
    pub base_branch: Option<String>,
    pub is_draft: bool,
    pub open_in_browser: bool,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubCreatedPullRequest {
    pub number: Option<u64>,
    pub url: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug)]
pub enum GitHubError {
    CliMissing,
    AuthenticationRequired,
    Process(ProcessError),
    InvalidInput,
    InvalidOutput,
    UserFailed(ProcessError),
    SearchFailed(ProcessError),
    DetailFailed(ProcessError),
    DiffFailed(ProcessError),
    HeadRevisionFailed(ProcessError),
    HeadChanged,
    DiffTooLarge,
    CommentFailed(ProcessError),
    ReviewFailed(ProcessError),
    UpdateFailed(ProcessError),
    MergeFailed(ProcessError),
    CreateFailed(ProcessError),
    OpenFailed(ProcessError),
}

impl fmt::Display for GitHubError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CliMissing => formatter.write_str("GitHub CLI (gh) is not installed"),
            Self::AuthenticationRequired => {
                formatter.write_str("GitHub CLI (gh) is not authenticated.")
            }
            Self::Process(error) => error.fmt(formatter),
            Self::InvalidInput => formatter.write_str("pull request input is invalid"),
            Self::InvalidOutput => {
                formatter.write_str("GitHub CLI returned malformed pull request output")
            }
            Self::UserFailed(error) => write!(formatter, "Failed to load GitHub user: {error}"),
            Self::SearchFailed(error) => {
                write!(formatter, "Failed to search pull requests: {error}")
            }
            Self::DetailFailed(error) => {
                write!(formatter, "Failed to load pull request details: {error}")
            }
            Self::DiffFailed(error) => {
                write!(formatter, "Failed to load pull request diff: {error}")
            }
            Self::HeadRevisionFailed(error) => {
                write!(
                    formatter,
                    "Failed to verify pull request head commit: {error}"
                )
            }
            Self::HeadChanged => {
                formatter.write_str("Pull request head changed during diff acquisition")
            }
            Self::DiffTooLarge => {
                formatter.write_str("Pull request diff exceeds the 8 MiB display limit")
            }
            Self::CommentFailed(error) => {
                write!(formatter, "Failed to post pull request comment: {error}")
            }
            Self::ReviewFailed(error) => {
                write!(formatter, "Failed to submit pull request review: {error}")
            }
            Self::UpdateFailed(error) => {
                write!(formatter, "Failed to update pull request: {error}")
            }
            Self::MergeFailed(error) => {
                write!(formatter, "Failed to merge pull request: {error}")
            }
            Self::CreateFailed(error) => {
                write!(formatter, "Failed to create pull request: {error}")
            }
            Self::OpenFailed(error) => {
                write!(formatter, "Failed to open pull request in browser: {error}")
            }
        }
    }
}

impl Error for GitHubError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Process(error)
            | Self::UserFailed(error)
            | Self::SearchFailed(error)
            | Self::DetailFailed(error)
            | Self::DiffFailed(error)
            | Self::HeadRevisionFailed(error)
            | Self::CommentFailed(error)
            | Self::ReviewFailed(error)
            | Self::UpdateFailed(error)
            | Self::MergeFailed(error)
            | Self::CreateFailed(error)
            | Self::OpenFailed(error) => Some(error),
            Self::CliMissing
            | Self::AuthenticationRequired
            | Self::InvalidInput
            | Self::InvalidOutput
            | Self::HeadChanged
            | Self::DiffTooLarge => None,
        }
    }
}

pub fn search_pull_requests(
    root: &Path,
    filters: &GitHubPullRequestSearchFilters,
    cursor: Option<&str>,
    page_size: usize,
) -> Result<GitHubPullRequestSearchPage, GitHubError> {
    require_available(root)?;
    if filters.text.chars().count() > MAX_GH_SEARCH_CHARS
        || !(1..=MAX_GH_SEARCH_PAGE_SIZE).contains(&page_size)
        || cursor.is_some_and(|cursor| {
            cursor.is_empty()
                || cursor.len() > MAX_GH_CURSOR_BYTES
                || cursor.chars().any(char::is_control)
        })
    {
        return Err(GitHubError::InvalidInput);
    }
    let account = current_user_unchecked(root)?;
    let search_query = pull_request_search_query(filters);
    let mut args = vec![
        "api".to_owned(),
        "graphql".to_owned(),
        "-f".to_owned(),
        format!("query={PULL_REQUEST_SEARCH_QUERY}"),
        "-f".to_owned(),
        format!("searchQuery={search_query}"),
        "-F".to_owned(),
        format!("first={page_size}"),
        "--hostname".to_owned(),
        GITHUB_HOSTNAME.to_owned(),
    ];
    if let Some(cursor) = cursor {
        args.extend(["-f".to_owned(), format!("after={cursor}")]);
    }
    let string_args = args.iter().map(String::as_str).collect::<Vec<_>>();
    let output = gh_output_with_limit(
        root,
        &string_args,
        MAX_GH_SEARCH_STDOUT_BYTES,
        GH_SEARCH_TIMEOUT,
    )
    .map_err(|error| match error {
        GitHubError::Process(error) => GitHubError::SearchFailed(error),
        error => error,
    })?;
    let response = serde_json::from_slice::<PullRequestSearchResponse>(&output.stdout)
        .map_err(|_| GitHubError::InvalidOutput)?;
    let search = response.data.search;
    let mut items = Vec::with_capacity(search.nodes.len());
    for record in search.nodes.into_iter().flatten() {
        if record.kind != "PullRequest" {
            continue;
        }
        items.push(record.try_into_summary(&account.login)?);
    }
    let next_cursor = search
        .page_info
        .end_cursor
        .filter(|next| search.page_info.has_next_page && Some(next.as_str()) != cursor);
    let has_next_page = next_cursor.is_some() && !items.is_empty();
    Ok(GitHubPullRequestSearchPage {
        account,
        total_count: search.issue_count,
        truncated: search.issue_count > 1_000 || search.page_info.has_next_page && !has_next_page,
        items,
        next_cursor: has_next_page.then_some(next_cursor).flatten(),
        has_next_page,
    })
}

pub fn pull_request_detail(
    root: &Path,
    identity: &GitHubPullRequestIdentity,
    account_login: &str,
) -> Result<GitHubPullRequestDetail, GitHubError> {
    require_available(root)?;
    if !valid_identity(identity) || account_login.is_empty() || account_login.len() > 256 {
        return Err(GitHubError::InvalidInput);
    }
    let repository = format!("{}/{}", identity.owner, identity.repository);
    let args = [
        "pr".to_owned(),
        "view".to_owned(),
        identity.number.to_string(),
        "--json".to_owned(),
        PULL_REQUEST_DETAIL_FIELDS.to_owned(),
        "--repo".to_owned(),
        repository,
    ];
    let string_args = args.iter().map(String::as_str).collect::<Vec<_>>();
    let output = gh_output_with_limit(
        root,
        &string_args,
        MAX_GH_DETAIL_STDOUT_BYTES,
        GH_OPERATION_TIMEOUT,
    )
    .map_err(|error| match error {
        GitHubError::Process(error) => GitHubError::DetailFailed(error),
        error => error,
    })?;
    let record = serde_json::from_slice::<PullRequestDetailRecord>(&output.stdout)
        .map_err(|_| GitHubError::InvalidOutput)?;
    let mut detail = record.try_into_detail(identity, account_login)?;
    match pull_request_checks_unchecked(root, identity) {
        Ok(checks) => detail.checks = checks,
        Err(_) => detail.checks_partial = true,
    }
    match pull_request_review_activity_unchecked(root, identity) {
        Ok((activity, partial)) => {
            let mut ids = detail
                .activity
                .iter()
                .map(|item| item.id.clone())
                .collect::<HashSet<_>>();
            detail.activity.extend(
                activity
                    .into_iter()
                    .filter(|item| ids.insert(item.id.clone())),
            );
            detail
                .activity
                .sort_by(|left, right| left.created_at.cmp(&right.created_at));
            detail.activity.truncate(MAX_GH_TIMELINE_ITEMS);
            detail.activity_partial = partial;
        }
        Err(_) => detail.activity_partial = true,
    }
    Ok(detail)
}

pub fn pull_request_diff(
    root: &Path,
    identity: &GitHubPullRequestIdentity,
) -> Result<GitHubPullRequestDiff, GitHubError> {
    require_available(root)?;
    if !valid_identity(identity) {
        return Err(GitHubError::InvalidInput);
    }
    let repository = format!("{}/{}", identity.owner, identity.repository);
    let number = identity.number.to_string();
    for _ in 0..2 {
        let before = pull_request_head_revision(root, &number, &repository)?;
        let args = [
            "pr",
            "diff",
            number.as_str(),
            "--patch",
            "--repo",
            repository.as_str(),
        ];
        let output =
            gh_output_with_limit(root, &args, MAX_GH_DIFF_STDOUT_BYTES, GH_OPERATION_TIMEOUT)
                .map_err(|error| match error {
                    GitHubError::Process(error) => GitHubError::DiffFailed(error),
                    error => error,
                })?;
        if output.stdout_truncated {
            return Err(GitHubError::DiffTooLarge);
        }
        let unified_diff =
            String::from_utf8(output.stdout).map_err(|_| GitHubError::InvalidOutput)?;
        let after = pull_request_head_revision(root, &number, &repository)?;
        if before == after {
            return Ok(GitHubPullRequestDiff {
                head_revision: before,
                unified_diff,
            });
        }
    }
    Err(GitHubError::HeadChanged)
}

pub fn post_pull_request_comment(
    root: &Path,
    identity: &GitHubPullRequestIdentity,
    body: &str,
) -> Result<(), GitHubError> {
    require_available(root)?;
    let body = body.trim();
    if !valid_identity(identity)
        || body.is_empty()
        || body.chars().count() > MAX_GH_BODY_CHARS
        || body.contains('\0')
    {
        return Err(GitHubError::InvalidInput);
    }
    let args = pull_request_comment_args(identity, body);
    let string_args = args.iter().map(String::as_str).collect::<Vec<_>>();
    gh_output_with_limit(
        root,
        &string_args,
        MAX_GH_STDOUT_BYTES,
        GH_OPERATION_TIMEOUT,
    )
    .map_err(|error| match error {
        GitHubError::Process(error) => GitHubError::CommentFailed(error),
        error => error,
    })?;
    Ok(())
}

pub fn submit_pull_request_review(
    root: &Path,
    identity: &GitHubPullRequestIdentity,
    expected_head_revision: &str,
    event: GitHubPullRequestReviewEvent,
    body: &str,
) -> Result<(), GitHubError> {
    require_available(root)?;
    let expected_head_revision = expected_head_revision.trim();
    let body = body.trim();
    if !valid_identity(identity)
        || !valid_head_revision(expected_head_revision)
        || body.chars().count() > MAX_GH_BODY_CHARS
        || body.contains('\0')
        || (event != GitHubPullRequestReviewEvent::Approve && body.is_empty())
    {
        return Err(GitHubError::InvalidInput);
    }
    verify_pull_request_head(root, identity, expected_head_revision)?;
    let args = pull_request_review_args(identity, expected_head_revision, event, body);
    let string_args = args.iter().map(String::as_str).collect::<Vec<_>>();
    gh_output_with_limit(
        root,
        &string_args,
        MAX_GH_STDOUT_BYTES,
        GH_OPERATION_TIMEOUT,
    )
    .map_err(|error| match error {
        GitHubError::Process(error) => GitHubError::ReviewFailed(error),
        error => error,
    })?;
    Ok(())
}

pub fn set_pull_request_review_state(
    root: &Path,
    identity: &GitHubPullRequestIdentity,
    expected_head_revision: &str,
    state: GitHubPullRequestReviewState,
) -> Result<(), GitHubError> {
    require_available(root)?;
    let expected_head_revision = expected_head_revision.trim();
    if !valid_identity(identity) || !valid_head_revision(expected_head_revision) {
        return Err(GitHubError::InvalidInput);
    }
    verify_pull_request_head(root, identity, expected_head_revision)?;
    let args = pull_request_review_state_args(identity, state);
    let string_args = args.iter().map(String::as_str).collect::<Vec<_>>();
    gh_output_with_limit(
        root,
        &string_args,
        MAX_GH_STDOUT_BYTES,
        GH_OPERATION_TIMEOUT,
    )
    .map_err(|error| match error {
        GitHubError::Process(error) => GitHubError::UpdateFailed(error),
        error => error,
    })?;
    Ok(())
}

pub fn update_pull_request_title(
    root: &Path,
    identity: &GitHubPullRequestIdentity,
    expected_head_revision: &str,
    title: &str,
) -> Result<(), GitHubError> {
    require_available(root)?;
    let expected_head_revision = expected_head_revision.trim();
    let title = title.trim();
    if !valid_identity(identity)
        || !valid_head_revision(expected_head_revision)
        || title.is_empty()
        || title.chars().count() > MAX_GH_TITLE_CHARS
        || title.chars().any(char::is_control)
    {
        return Err(GitHubError::InvalidInput);
    }
    verify_pull_request_head(root, identity, expected_head_revision)?;
    let args = pull_request_title_args(identity, title);
    let string_args = args.iter().map(String::as_str).collect::<Vec<_>>();
    gh_output_with_limit(
        root,
        &string_args,
        MAX_GH_STDOUT_BYTES,
        GH_OPERATION_TIMEOUT,
    )
    .map_err(|error| match error {
        GitHubError::Process(error) => GitHubError::UpdateFailed(error),
        error => error,
    })?;
    Ok(())
}

pub fn update_pull_request_body(
    root: &Path,
    identity: &GitHubPullRequestIdentity,
    expected_head_revision: &str,
    body: &str,
) -> Result<(), GitHubError> {
    require_available(root)?;
    let expected_head_revision = expected_head_revision.trim();
    if !valid_identity(identity)
        || !valid_head_revision(expected_head_revision)
        || body.chars().count() > MAX_GH_BODY_CHARS
        || body.contains('\0')
    {
        return Err(GitHubError::InvalidInput);
    }
    verify_pull_request_head(root, identity, expected_head_revision)?;
    let args = pull_request_body_args(identity, body);
    let string_args = args.iter().map(String::as_str).collect::<Vec<_>>();
    gh_output_with_limit(
        root,
        &string_args,
        MAX_GH_STDOUT_BYTES,
        GH_OPERATION_TIMEOUT,
    )
    .map_err(|error| match error {
        GitHubError::Process(error) => GitHubError::UpdateFailed(error),
        error => error,
    })?;
    Ok(())
}

pub fn merge_pull_request(
    root: &Path,
    identity: &GitHubPullRequestIdentity,
    expected_head_revision: &str,
    method: GitHubPullRequestMergeMethod,
) -> Result<(), GitHubError> {
    require_available(root)?;
    let expected_head_revision = expected_head_revision.trim();
    if !valid_identity(identity) || !valid_head_revision(expected_head_revision) {
        return Err(GitHubError::InvalidInput);
    }
    let args = pull_request_merge_args(identity, expected_head_revision, method);
    let string_args = args.iter().map(String::as_str).collect::<Vec<_>>();
    gh_output_with_limit(
        root,
        &string_args,
        MAX_GH_STDOUT_BYTES,
        GH_OPERATION_TIMEOUT,
    )
    .map_err(|error| match error {
        GitHubError::Process(error) => GitHubError::MergeFailed(error),
        error => error,
    })?;
    Ok(())
}

fn pull_request_comment_args(identity: &GitHubPullRequestIdentity, body: &str) -> Vec<String> {
    vec![
        "pr".to_owned(),
        "comment".to_owned(),
        identity.number.to_string(),
        "--body".to_owned(),
        body.to_owned(),
        "--repo".to_owned(),
        format!("{}/{}", identity.owner, identity.repository),
    ]
}

fn pull_request_review_args(
    identity: &GitHubPullRequestIdentity,
    expected_head_revision: &str,
    event: GitHubPullRequestReviewEvent,
    body: &str,
) -> Vec<String> {
    let event = match event {
        GitHubPullRequestReviewEvent::Approve => "APPROVE",
        GitHubPullRequestReviewEvent::Comment => "COMMENT",
        GitHubPullRequestReviewEvent::RequestChanges => "REQUEST_CHANGES",
    };
    let mut args = vec![
        "api".to_owned(),
        format!(
            "repos/{}/{}/pulls/{}/reviews",
            identity.owner, identity.repository, identity.number
        ),
        "--method".to_owned(),
        "POST".to_owned(),
        "-f".to_owned(),
        format!("commit_id={expected_head_revision}"),
        "-f".to_owned(),
        format!("event={event}"),
    ];
    if !body.is_empty() {
        args.extend(["-f".to_owned(), format!("body={body}")]);
    }
    args.extend(["--hostname".to_owned(), identity.hostname.clone()]);
    args
}

fn pull_request_review_state_args(
    identity: &GitHubPullRequestIdentity,
    state: GitHubPullRequestReviewState,
) -> Vec<String> {
    let mut args = vec![
        "pr".to_owned(),
        "ready".to_owned(),
        identity.number.to_string(),
    ];
    if state == GitHubPullRequestReviewState::Draft {
        args.push("--undo".to_owned());
    }
    args.extend([
        "--repo".to_owned(),
        format!("{}/{}", identity.owner, identity.repository),
    ]);
    args
}

fn pull_request_title_args(identity: &GitHubPullRequestIdentity, title: &str) -> Vec<String> {
    vec![
        "pr".to_owned(),
        "edit".to_owned(),
        identity.number.to_string(),
        "--title".to_owned(),
        title.to_owned(),
        "--repo".to_owned(),
        format!("{}/{}", identity.owner, identity.repository),
    ]
}

fn pull_request_body_args(identity: &GitHubPullRequestIdentity, body: &str) -> Vec<String> {
    vec![
        "pr".to_owned(),
        "edit".to_owned(),
        identity.number.to_string(),
        "--body".to_owned(),
        body.to_owned(),
        "--repo".to_owned(),
        format!("{}/{}", identity.owner, identity.repository),
    ]
}

fn pull_request_merge_args(
    identity: &GitHubPullRequestIdentity,
    expected_head_revision: &str,
    method: GitHubPullRequestMergeMethod,
) -> Vec<String> {
    vec![
        "pr".to_owned(),
        "merge".to_owned(),
        identity.number.to_string(),
        match method {
            GitHubPullRequestMergeMethod::Merge => "--merge",
            GitHubPullRequestMergeMethod::Squash => "--squash",
        }
        .to_owned(),
        "--match-head-commit".to_owned(),
        expected_head_revision.to_owned(),
        "--repo".to_owned(),
        format!("{}/{}", identity.owner, identity.repository),
    ]
}

fn verify_pull_request_head(
    root: &Path,
    identity: &GitHubPullRequestIdentity,
    expected_head_revision: &str,
) -> Result<(), GitHubError> {
    let repository = format!("{}/{}", identity.owner, identity.repository);
    let number = identity.number.to_string();
    let actual_head_revision = pull_request_head_revision(root, &number, &repository)?;
    if actual_head_revision != expected_head_revision {
        return Err(GitHubError::HeadChanged);
    }
    Ok(())
}

fn pull_request_head_revision(
    root: &Path,
    number: &str,
    repository: &str,
) -> Result<String, GitHubError> {
    let args = [
        "pr",
        "view",
        number,
        "--json",
        "headRefOid",
        "--jq",
        ".headRefOid",
        "--repo",
        repository,
    ];
    let output = gh_output_with_limit(root, &args, 257, Duration::from_secs(5)).map_err(
        |error| match error {
            GitHubError::Process(error) => GitHubError::HeadRevisionFailed(error),
            error => error,
        },
    )?;
    if output.stdout_truncated {
        return Err(GitHubError::InvalidOutput);
    }
    let revision = String::from_utf8(output.stdout)
        .map_err(|_| GitHubError::InvalidOutput)?
        .trim()
        .to_owned();
    if revision.is_empty() || revision.len() > 256 || revision.chars().any(char::is_control) {
        return Err(GitHubError::InvalidOutput);
    }
    Ok(revision)
}

fn pull_request_checks_unchecked(
    root: &Path,
    identity: &GitHubPullRequestIdentity,
) -> Result<Vec<GitHubPullRequestCheck>, GitHubError> {
    const FULL_FIELDS: &str =
        "bucket,completedAt,description,event,link,name,startedAt,state,workflow";
    const FALLBACK_FIELDS: &str = "bucket,description,link,name,workflow";
    let repository = format!("{}/{}", identity.owner, identity.repository);
    let number = identity.number.to_string();
    let load = |fields: &str| {
        let args = [
            "pr",
            "checks",
            number.as_str(),
            "--json",
            fields,
            "--repo",
            repository.as_str(),
        ];
        gh_output_with_limit(
            root,
            &args,
            MAX_GH_DETAIL_STDOUT_BYTES,
            Duration::from_secs(5),
        )
        .map_err(|error| match error {
            GitHubError::Process(error) => GitHubError::DetailFailed(error),
            error => error,
        })
    };
    let output = match load(FULL_FIELDS) {
        Ok(output) => output,
        Err(error) if github_error_contains(&error, &["unknown json field", "unknown field"]) => {
            load(FALLBACK_FIELDS)?
        }
        Err(error) if github_error_contains(&error, &["no checks reported", "no checks found"]) => {
            return Ok(Vec::new());
        }
        Err(error) => return Err(error),
    };
    let records = serde_json::from_slice::<Vec<PullRequestCheckRecord>>(&output.stdout)
        .map_err(|_| GitHubError::InvalidOutput)?;
    let mut checks = HashMap::<String, (usize, GitHubPullRequestCheck)>::new();
    for (index, record) in records.into_iter().enumerate() {
        let Some(check) = record.try_into_check() else {
            continue;
        };
        let key = if check.started_at.is_some() {
            format!(
                "{}\u{0}{}",
                check.name,
                check.workflow.as_deref().unwrap_or_default()
            )
        } else {
            format!("entry:{index}")
        };
        let replace = checks.get(&key).is_none_or(|(_, current)| {
            check.started_at.as_deref().unwrap_or_default()
                > current.started_at.as_deref().unwrap_or_default()
        });
        if replace {
            checks.insert(key, (index, check));
        }
    }
    let mut checks = checks.into_values().collect::<Vec<_>>();
    checks.sort_by_key(|(index, _)| *index);
    Ok(checks.into_iter().map(|(_, check)| check).collect())
}

fn github_error_contains(error: &GitHubError, needles: &[&str]) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    needles.iter().any(|needle| message.contains(needle))
}

fn pull_request_review_activity_unchecked(
    root: &Path,
    identity: &GitHubPullRequestIdentity,
) -> Result<(Vec<GitHubPullRequestActivity>, bool), GitHubError> {
    let mut cursor = None::<String>;
    let mut activity = Vec::new();
    let mut partial = false;
    for page in 0..MAX_GH_TIMELINE_PAGES {
        let mut args = vec![
            "api".to_owned(),
            "graphql".to_owned(),
            "-f".to_owned(),
            format!("query={PULL_REQUEST_REVIEW_ACTIVITY_QUERY}"),
            "-F".to_owned(),
            format!("owner={}", identity.owner),
            "-F".to_owned(),
            format!("repo={}", identity.repository),
            "-F".to_owned(),
            format!("number={}", identity.number),
        ];
        if let Some(cursor) = cursor.as_ref() {
            args.extend(["-F".to_owned(), format!("after={cursor}")]);
        }
        args.extend(["--hostname".to_owned(), identity.hostname.clone()]);
        let string_args = args.iter().map(String::as_str).collect::<Vec<_>>();
        let output = gh_output_with_limit(
            root,
            &string_args,
            MAX_GH_TIMELINE_STDOUT_BYTES,
            Duration::from_secs(5),
        )
        .map_err(|error| match error {
            GitHubError::Process(error) => GitHubError::DetailFailed(error),
            error => error,
        })?;
        let response = serde_json::from_slice::<ReviewActivityResponse>(&output.stdout)
            .map_err(|_| GitHubError::InvalidOutput)?;
        let Some(pull_request) = response.data.repository.pull_request else {
            return Ok((activity, partial));
        };
        if page == 0 {
            partial |= pull_request.comments.page_info.has_next_page
                || pull_request.latest_reviews.page_info.has_next_page;
            for comment in pull_request.comments.nodes.into_iter().flatten() {
                if let Some(comment) = comment.into_activity(
                    GitHubPullRequestActivityKind::Comment,
                    "comment",
                    None,
                    None,
                    None,
                    None,
                ) {
                    activity.push(comment);
                }
            }
        }
        let thread_page = pull_request.review_threads;
        for thread in thread_page.nodes.into_iter().flatten() {
            partial |= thread.comments.page_info.has_next_page;
            let line = thread
                .line
                .or(thread.original_line)
                .or(thread.start_line)
                .or(thread.original_start_line);
            let start_line = if thread.line.is_none() && thread.original_line.is_some() {
                thread.original_start_line.or(thread.start_line).or(line)
            } else {
                thread.start_line.or(thread.original_start_line).or(line)
            };
            let path = thread.path.filter(|path| !path.trim().is_empty());
            for comment in thread.comments.nodes.into_iter().flatten() {
                if let Some(comment) = comment.into_activity(
                    GitHubPullRequestActivityKind::ReviewComment,
                    "review-comment",
                    path.clone(),
                    line,
                    start_line,
                    thread.id.clone(),
                ) {
                    activity.push(comment);
                }
            }
        }
        if activity.len() >= MAX_GH_TIMELINE_ITEMS {
            activity.truncate(MAX_GH_TIMELINE_ITEMS);
            return Ok((activity, true));
        }
        if !thread_page.page_info.has_next_page {
            return Ok((activity, partial));
        }
        let next = thread_page.page_info.end_cursor.filter(|next| {
            !next.is_empty()
                && next.len() <= MAX_GH_CURSOR_BYTES
                && Some(next.as_str()) != cursor.as_deref()
        });
        let Some(next) = next else {
            return Ok((activity, true));
        };
        cursor = Some(next);
    }
    Ok((activity, true))
}

pub fn pull_request_status(
    root: &Path,
    head_branch: &str,
) -> Result<GitHubPullRequestStatus, GitHubError> {
    let availability = cli_availability(root)?;
    if availability != GitHubCliAvailability::Available {
        return Ok(GitHubPullRequestStatus {
            availability,
            pull_request: None,
        });
    }
    let args = [
        "pr",
        "list",
        "--head",
        head_branch,
        "--state",
        "open",
        "--limit",
        "1",
        "--json",
        "url,state,isDraft,number,baseRefName,headRefName,title",
    ];
    let output = gh_output(root, &args, GH_OPERATION_TIMEOUT)?;
    let records = serde_json::from_slice::<Vec<PullRequestRecord>>(&output.stdout)
        .map_err(|_| GitHubError::InvalidOutput)?;
    let pull_request = records
        .into_iter()
        .next()
        .map(TryInto::try_into)
        .transpose()?;
    Ok(GitHubPullRequestStatus {
        availability,
        pull_request,
    })
}

pub fn create_pull_request(
    root: &Path,
    request: &GitHubCreatePullRequest,
) -> Result<GitHubCreatedPullRequest, GitHubError> {
    match cli_availability(root)? {
        GitHubCliAvailability::Available => {}
        GitHubCliAvailability::Missing => return Err(GitHubError::CliMissing),
        GitHubCliAvailability::AuthenticationRequired => {
            return Err(GitHubError::AuthenticationRequired);
        }
    }
    if !valid_branch(&request.head_branch)
        || request
            .base_branch
            .as_deref()
            .is_some_and(|branch| !valid_branch(branch))
        || request.title.contains('\0')
        || request.body.contains('\0')
        || request.title.chars().count() > MAX_GH_TITLE_CHARS
        || request.body.chars().count() > MAX_GH_BODY_CHARS
    {
        return Err(GitHubError::InvalidInput);
    }

    let title = nonempty(&request.title)
        .map(str::to_owned)
        .or_else(|| {
            git_log(root, "%s")
                .ok()
                .and_then(|value| nonempty(&value).map(str::to_owned))
        })
        .unwrap_or_else(|| request.head_branch.clone());
    let body = nonempty(&request.body)
        .map(str::to_owned)
        .or_else(|| git_log(root, "%b").ok())
        .unwrap_or_default();
    let args = create_pull_request_args(request, &title, &body);
    let string_args = args.iter().map(String::as_str).collect::<Vec<_>>();
    let mut command = gh_command(root, &string_args);
    if request.open_in_browser {
        command.env("GH_BROWSER", "echo");
    }
    let output = run_bounded(
        &mut command,
        MAX_GH_STDOUT_BYTES,
        MAX_GH_STDERR_BYTES,
        GH_OPERATION_TIMEOUT,
    )
    .map_err(|error| {
        if request.open_in_browser {
            GitHubError::OpenFailed(error)
        } else {
            GitHubError::CreateFailed(error)
        }
    })?;
    let output = String::from_utf8(output.stdout).map_err(|_| GitHubError::InvalidOutput)?;
    let url = first_url(&output).ok_or(GitHubError::InvalidOutput)?;
    Ok(GitHubCreatedPullRequest {
        number: pull_request_number(&url),
        url,
        title,
        body,
    })
}

pub fn cli_availability(root: &Path) -> Result<GitHubCliAvailability, GitHubError> {
    match gh_output(root, &["--version"], GH_VERSION_TIMEOUT) {
        Ok(_) => {}
        Err(GitHubError::Process(ProcessError::Spawn(error)))
            if error.kind() == std::io::ErrorKind::NotFound =>
        {
            return Ok(GitHubCliAvailability::Missing);
        }
        Err(GitHubError::Process(ProcessError::Exit { .. })) => {
            return Ok(GitHubCliAvailability::Missing);
        }
        Err(error) => return Err(error),
    }
    match gh_output(root, &["auth", "status", "--active"], GH_AUTH_TIMEOUT) {
        Ok(_) => Ok(GitHubCliAvailability::Available),
        Err(GitHubError::Process(ProcessError::Exit { stderr, .. }))
            if stderr.contains("unknown flag") && stderr.contains("--active") =>
        {
            match gh_output(root, &["auth", "status"], GH_AUTH_TIMEOUT) {
                Ok(_) => Ok(GitHubCliAvailability::Available),
                Err(GitHubError::Process(ProcessError::Exit { .. })) => {
                    Ok(GitHubCliAvailability::AuthenticationRequired)
                }
                Err(error) => Err(error),
            }
        }
        Err(GitHubError::Process(ProcessError::Exit { .. })) => {
            Ok(GitHubCliAvailability::AuthenticationRequired)
        }
        Err(error) => Err(error),
    }
}

fn require_available(root: &Path) -> Result<(), GitHubError> {
    match cli_availability(root)? {
        GitHubCliAvailability::Available => Ok(()),
        GitHubCliAvailability::Missing => Err(GitHubError::CliMissing),
        GitHubCliAvailability::AuthenticationRequired => Err(GitHubError::AuthenticationRequired),
    }
}

fn current_user_unchecked(root: &Path) -> Result<GitHubUser, GitHubError> {
    let output = gh_output_with_limit(
        root,
        &["api", "user", "--hostname", GITHUB_HOSTNAME],
        MAX_GH_STDOUT_BYTES,
        Duration::from_secs(5),
    )
    .map_err(|error| match error {
        GitHubError::Process(error) => GitHubError::UserFailed(error),
        error => error,
    })?;
    let user = serde_json::from_slice::<UserRecord>(&output.stdout)
        .map_err(|_| GitHubError::InvalidOutput)?;
    if !valid_login(&user.login) {
        return Err(GitHubError::InvalidOutput);
    }
    Ok(GitHubUser {
        login: user.login,
        avatar_url: user.avatar_url.filter(|url| valid_https_url(url)),
    })
}

fn pull_request_search_query(filters: &GitHubPullRequestSearchFilters) -> String {
    let mut terms = vec!["is:pr".to_owned()];
    match filters.relationship {
        GitHubPullRequestRelationship::All => {}
        GitHubPullRequestRelationship::Authored => terms.push("author:@me".to_owned()),
        GitHubPullRequestRelationship::ReviewRequested => {
            terms.push("review-requested:@me".to_owned());
        }
        GitHubPullRequestRelationship::Reviewed => terms.push("reviewed-by:@me".to_owned()),
    }
    match filters.lifecycle {
        GitHubPullRequestLifecycle::All => {}
        GitHubPullRequestLifecycle::Open => terms.push("is:open".to_owned()),
        GitHubPullRequestLifecycle::Merged => terms.push("is:merged".to_owned()),
        GitHubPullRequestLifecycle::Closed => {
            terms.extend(["is:closed".to_owned(), "is:unmerged".to_owned()]);
        }
    }
    let text = filters
        .text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if !text.is_empty() {
        terms.push(format!(
            "\"{}\"",
            text.replace('\\', "\\\\").replace('"', "\\\"")
        ));
    }
    terms.push("sort:updated-desc".to_owned());
    terms.join(" ")
}

fn create_pull_request_args(
    request: &GitHubCreatePullRequest,
    title: &str,
    body: &str,
) -> Vec<String> {
    let mut args = vec!["pr".to_owned(), "create".to_owned()];
    if request.open_in_browser {
        args.push("--web".to_owned());
    }
    args.extend([
        "--head".to_owned(),
        request.head_branch.clone(),
        "--title".to_owned(),
        title.to_owned(),
    ]);
    if let Some(base_branch) = request.base_branch.as_deref().and_then(nonempty) {
        args.extend(["--base".to_owned(), base_branch.to_owned()]);
    }
    if request.is_draft && !request.open_in_browser {
        args.push("--draft".to_owned());
    }
    args.extend(["--body".to_owned(), body.to_owned()]);
    args
}

fn gh_output(root: &Path, args: &[&str], timeout: Duration) -> Result<BoundedOutput, GitHubError> {
    gh_output_with_limit(root, args, MAX_GH_STDOUT_BYTES, timeout)
}

fn gh_output_with_limit(
    root: &Path,
    args: &[&str],
    stdout_limit: usize,
    timeout: Duration,
) -> Result<BoundedOutput, GitHubError> {
    let mut command = gh_command(root, args);
    run_bounded(&mut command, stdout_limit, MAX_GH_STDERR_BYTES, timeout)
        .map_err(GitHubError::Process)
}

fn gh_command(root: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("gh");
    command.current_dir(root).args(args);
    command
}

fn git_log(root: &Path, format: &str) -> Result<String, GitHubError> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(root)
        .arg("log")
        .arg("-1")
        .arg(format!("--pretty={format}"));
    let output = run_bounded(
        &mut command,
        MAX_GH_BODY_CHARS.saturating_add(1),
        MAX_GH_STDERR_BYTES,
        GH_OPERATION_TIMEOUT,
    )
    .map_err(GitHubError::Process)?;
    String::from_utf8(output.stdout).map_err(|_| GitHubError::InvalidOutput)
}

fn nonempty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn valid_branch(branch: &str) -> bool {
    !branch.is_empty()
        && branch.len() <= 1_024
        && !branch.starts_with('-')
        && !branch.chars().any(char::is_control)
}

fn valid_login(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
}

fn valid_repository_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

fn valid_identity(identity: &GitHubPullRequestIdentity) -> bool {
    identity.hostname.eq_ignore_ascii_case(GITHUB_HOSTNAME)
        && valid_repository_component(&identity.owner)
        && valid_repository_component(&identity.repository)
        && identity.number > 0
}

fn valid_head_revision(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

fn valid_https_url(value: &str) -> bool {
    value.len() <= MAX_GH_URL_BYTES
        && value.starts_with("https://")
        && !value.chars().any(char::is_control)
        && !value.chars().any(char::is_whitespace)
}

fn validate_pull_request_url(
    value: &str,
    identity: &GitHubPullRequestIdentity,
) -> Result<(), GitHubError> {
    if !valid_https_url(value) {
        return Err(GitHubError::InvalidOutput);
    }
    let rest = value
        .strip_prefix("https://")
        .ok_or(GitHubError::InvalidOutput)?;
    let (hostname, path) = rest.split_once('/').ok_or(GitHubError::InvalidOutput)?;
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let parts = path.split('/').collect::<Vec<_>>();
    if !hostname.eq_ignore_ascii_case(&identity.hostname)
        || parts.len() != 4
        || !parts[0].eq_ignore_ascii_case(&identity.owner)
        || !parts[1].eq_ignore_ascii_case(&identity.repository)
        || parts[2] != "pull"
        || parts[3].parse::<u64>().ok() != Some(identity.number)
    {
        return Err(GitHubError::InvalidOutput);
    }
    Ok(())
}

fn bounded_chars(value: String, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value;
    }
    value.chars().take(limit).collect()
}

fn first_url(output: &str) -> Option<String> {
    output
        .split_whitespace()
        .map(|value| {
            value.trim_matches(|character: char| {
                matches!(
                    character,
                    '"' | '\'' | '(' | ')' | '[' | ']' | '<' | '>' | ','
                )
            })
        })
        .find(|value| {
            value.len() <= MAX_GH_URL_BYTES
                && (value.starts_with("https://") || value.starts_with("http://"))
        })
        .map(str::to_owned)
}

fn pull_request_number(url: &str) -> Option<u64> {
    url.split("/pull/")
        .nth(1)?
        .split(['/', '?', '#'])
        .next()?
        .parse()
        .ok()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequestRecord {
    number: u64,
    title: String,
    url: String,
    base_ref_name: String,
    head_ref_name: String,
    is_draft: bool,
}

#[derive(Debug, Deserialize)]
struct UserRecord {
    login: String,
    avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PullRequestSearchResponse {
    data: PullRequestSearchData,
}

#[derive(Debug, Deserialize)]
struct PullRequestSearchData {
    search: PullRequestSearchConnection,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequestSearchConnection {
    issue_count: u64,
    nodes: Vec<Option<PullRequestSearchRecord>>,
    page_info: PullRequestPageInfo,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PullRequestPageInfo {
    end_cursor: Option<String>,
    has_next_page: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequestSearchRecord {
    #[serde(rename = "__typename")]
    kind: String,
    additions: u64,
    author: Option<GitHubActorRecord>,
    base_ref_name: String,
    created_at: String,
    deletions: u64,
    head_ref_name: String,
    id: String,
    is_draft: bool,
    number: u64,
    repository: GitHubRepositoryRecord,
    state: String,
    status_check_rollup: Option<StatusCheckRollupRecord>,
    title: String,
    updated_at: String,
    url: String,
}

impl PullRequestSearchRecord {
    fn try_into_summary(
        self,
        account_login: &str,
    ) -> Result<GitHubPullRequestSummary, GitHubError> {
        let identity = GitHubPullRequestIdentity {
            hostname: GITHUB_HOSTNAME.to_owned(),
            owner: self.repository.owner.login,
            repository: self.repository.name,
            number: self.number,
        };
        if !valid_identity(&identity)
            || self.id.is_empty()
            || self.id.len() > 1_024
            || self.title.chars().count() > 1_024
        {
            return Err(GitHubError::InvalidOutput);
        }
        validate_pull_request_url(&self.url, &identity)?;
        let author_login = self.author.map(|author| author.login);
        Ok(GitHubPullRequestSummary {
            identity,
            node_id: self.id,
            title: bounded_chars(self.title, 1_024),
            url: self.url,
            state: pull_request_state(&self.state)?,
            is_draft: self.is_draft,
            is_author: author_login
                .as_deref()
                .is_some_and(|author| author.eq_ignore_ascii_case(account_login)),
            author_login,
            base_branch: bounded_chars(self.base_ref_name, 1_024),
            head_branch: bounded_chars(self.head_ref_name, 1_024),
            additions: self.additions,
            deletions: self.deletions,
            created_at: bounded_chars(self.created_at, 128),
            updated_at: bounded_chars(self.updated_at, 128),
            ci_status: ci_status(self.status_check_rollup.as_ref()),
        })
    }
}

#[derive(Debug, Deserialize, Clone)]
struct GitHubActorRecord {
    login: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRepositoryRecord {
    name: String,
    owner: GitHubRepositoryOwnerRecord,
}

#[derive(Debug, Deserialize)]
struct GitHubRepositoryOwnerRecord {
    login: String,
}

#[derive(Debug, Deserialize)]
struct StatusCheckRollupRecord {
    contexts: StatusCheckContextsRecord,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusCheckContextsRecord {
    total_count: u64,
    nodes: Vec<Option<StatusCheckRecord>>,
}

#[derive(Debug, Deserialize)]
struct StatusCheckRecord {
    #[serde(rename = "__typename")]
    kind: Option<String>,
    conclusion: Option<String>,
    state: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequestCheckRecord {
    bucket: Option<String>,
    completed_at: Option<String>,
    description: Option<String>,
    link: Option<String>,
    name: Option<String>,
    started_at: Option<String>,
    state: Option<String>,
    workflow: Option<String>,
}

impl PullRequestCheckRecord {
    fn try_into_check(self) -> Option<GitHubPullRequestCheck> {
        let name = self.name?.trim().to_owned();
        if name.is_empty() || name.chars().count() > 1_024 {
            return None;
        }
        let status = if self
            .state
            .as_deref()
            .is_some_and(|state| state.eq_ignore_ascii_case("neutral"))
        {
            GitHubCheckStatus::Neutral
        } else {
            match self
                .bucket
                .as_deref()
                .unwrap_or_default()
                .to_ascii_uppercase()
                .as_str()
            {
                "FAIL" | "CANCEL" => GitHubCheckStatus::Failing,
                "PENDING" => GitHubCheckStatus::Pending,
                "PASS" => GitHubCheckStatus::Passing,
                "SKIP" | "SKIPPING" => GitHubCheckStatus::Skipped,
                _ => GitHubCheckStatus::Unknown,
            }
        };
        Some(GitHubPullRequestCheck {
            name,
            workflow: self
                .workflow
                .filter(|value| !value.trim().is_empty())
                .map(|value| bounded_chars(value, 1_024)),
            status,
            description: self
                .description
                .filter(|value| !value.trim().is_empty())
                .map(|value| bounded_chars(value, 4_096)),
            link: self.link.filter(|value| valid_https_url(value)),
            started_at: self
                .started_at
                .filter(|value| !value.trim().is_empty())
                .map(|value| bounded_chars(value, 128)),
            completed_at: self
                .completed_at
                .filter(|value| !value.trim().is_empty())
                .map(|value| bounded_chars(value, 128)),
        })
    }
}

#[derive(Debug, Deserialize)]
struct ReviewActivityResponse {
    data: ReviewActivityData,
}

#[derive(Debug, Deserialize)]
struct ReviewActivityData {
    repository: ReviewActivityRepository,
}

#[derive(Debug, Deserialize)]
struct ReviewActivityRepository {
    #[serde(rename = "pullRequest")]
    pull_request: Option<ReviewActivityPullRequest>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ReviewActivityPullRequest {
    #[serde(default)]
    comments: TimelineCommentConnection,
    #[serde(default)]
    latest_reviews: ReviewSummaryConnection,
    #[serde(default)]
    review_threads: ReviewThreadConnection,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TimelineCommentConnection {
    #[serde(default)]
    nodes: Vec<Option<TimelineCommentRecord>>,
    #[serde(default)]
    page_info: PullRequestPageInfo,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ReviewSummaryConnection {
    #[serde(default)]
    page_info: PullRequestPageInfo,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ReviewThreadConnection {
    #[serde(default)]
    nodes: Vec<Option<ReviewThreadRecord>>,
    #[serde(default)]
    page_info: PullRequestPageInfo,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewThreadRecord {
    id: Option<String>,
    #[serde(default)]
    comments: TimelineCommentConnection,
    line: Option<u64>,
    original_line: Option<u64>,
    original_start_line: Option<u64>,
    path: Option<String>,
    start_line: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TimelineCommentRecord {
    author: Option<GitHubActorRecord>,
    body: Option<String>,
    created_at: Option<String>,
    id: Option<String>,
    url: Option<String>,
}

impl TimelineCommentRecord {
    fn into_activity(
        self,
        kind: GitHubPullRequestActivityKind,
        id_prefix: &str,
        path: Option<String>,
        line: Option<u64>,
        start_line: Option<u64>,
        review_thread_id: Option<String>,
    ) -> Option<GitHubPullRequestActivity> {
        let body = self.body?.trim().to_owned();
        let created_at = self.created_at?.trim().to_owned();
        let raw_id = self
            .id
            .filter(|id| !id.trim().is_empty())
            .or_else(|| self.url.clone())?;
        if body.is_empty() || created_at.is_empty() {
            return None;
        }
        Some(GitHubPullRequestActivity {
            id: format!("{id_prefix}:{}", bounded_chars(raw_id, 1_024)),
            kind,
            actor_login: self.author.map(|author| author.login),
            body: bounded_chars(body, MAX_GH_ACTIVITY_BODY_CHARS),
            created_at: bounded_chars(created_at, 128),
            event: None,
            url: self.url.filter(|url| valid_https_url(url)),
            path: path.map(|path| bounded_chars(path, 1_024)),
            line,
            start_line,
            review_thread_id: review_thread_id.map(|id| bounded_chars(id, 1_024)),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequestReviewRecord {
    author: Option<GitHubActorRecord>,
    body: Option<String>,
    created_at: Option<String>,
    id: Option<String>,
    state: Option<String>,
    submitted_at: Option<String>,
    url: Option<String>,
    comments: Option<Vec<TimelineCommentRecord>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequestDetailRecord {
    additions: Option<u64>,
    author: Option<GitHubActorRecord>,
    base_ref_name: Option<String>,
    body: Option<String>,
    comments: Option<Vec<TimelineCommentRecord>>,
    created_at: Option<String>,
    deletions: Option<u64>,
    head_ref_name: Option<String>,
    head_ref_oid: Option<String>,
    is_draft: Option<bool>,
    merged_at: Option<String>,
    merged_by: Option<GitHubActorRecord>,
    merge_state_status: Option<String>,
    mergeable: Option<String>,
    number: Option<u64>,
    review_decision: Option<String>,
    reviews: Option<Vec<PullRequestReviewRecord>>,
    state: Option<String>,
    status_check_rollup: Option<Vec<Option<StatusCheckRecord>>>,
    title: Option<String>,
    updated_at: Option<String>,
    url: Option<String>,
}

impl PullRequestDetailRecord {
    fn baseline_activity(&self) -> Vec<GitHubPullRequestActivity> {
        let mut activity = Vec::new();
        if let Some(created_at) = self
            .created_at
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            activity.push(GitHubPullRequestActivity {
                id: format!("opened:{}", self.number.unwrap_or_default()),
                kind: GitHubPullRequestActivityKind::Event,
                actor_login: self.author.as_ref().map(|author| author.login.clone()),
                body: String::new(),
                created_at: bounded_chars(created_at.to_owned(), 128),
                event: Some("opened".to_owned()),
                url: self.url.clone().filter(|url| valid_https_url(url)),
                path: None,
                line: None,
                start_line: None,
                review_thread_id: None,
            });
        }
        for comment in self.comments.iter().flatten().cloned() {
            if let Some(comment) = comment.into_activity(
                GitHubPullRequestActivityKind::Comment,
                "comment",
                None,
                None,
                None,
                None,
            ) {
                activity.push(comment);
            }
        }
        for (review_index, review) in self.reviews.iter().flatten().enumerate() {
            let created_at = review
                .submitted_at
                .as_deref()
                .or(review.created_at.as_deref())
                .filter(|value| !value.trim().is_empty());
            let raw_id = review
                .id
                .as_deref()
                .or(review.url.as_deref())
                .map(str::to_owned)
                .unwrap_or_else(|| review_index.to_string());
            let actor_login = review.author.as_ref().map(|author| author.login.clone());
            if let (Some(created_at), Some(state)) = (created_at, review.state.as_deref()) {
                let event = match state.to_ascii_uppercase().as_str() {
                    "APPROVED" => Some("approved"),
                    "CHANGES_REQUESTED" => Some("changes_requested"),
                    _ => None,
                };
                if let Some(event) = event {
                    activity.push(GitHubPullRequestActivity {
                        id: format!("review-event:{raw_id}:{event}"),
                        kind: GitHubPullRequestActivityKind::Event,
                        actor_login: actor_login.clone(),
                        body: String::new(),
                        created_at: bounded_chars(created_at.to_owned(), 128),
                        event: Some(event.to_owned()),
                        url: review.url.clone().filter(|url| valid_https_url(url)),
                        path: None,
                        line: None,
                        start_line: None,
                        review_thread_id: None,
                    });
                }
            }
            if let (Some(created_at), Some(body)) = (
                created_at,
                review
                    .body
                    .as_deref()
                    .filter(|body| !body.trim().is_empty()),
            ) {
                activity.push(GitHubPullRequestActivity {
                    id: format!("review:{raw_id}"),
                    kind: GitHubPullRequestActivityKind::Review,
                    actor_login: actor_login.clone(),
                    body: bounded_chars(body.trim().to_owned(), MAX_GH_ACTIVITY_BODY_CHARS),
                    created_at: bounded_chars(created_at.to_owned(), 128),
                    event: None,
                    url: review.url.clone().filter(|url| valid_https_url(url)),
                    path: None,
                    line: None,
                    start_line: None,
                    review_thread_id: None,
                });
            }
            for comment in review.comments.iter().flatten().cloned() {
                if let Some(comment) = comment.into_activity(
                    GitHubPullRequestActivityKind::ReviewComment,
                    "review-comment",
                    None,
                    None,
                    None,
                    None,
                ) {
                    activity.push(comment);
                }
            }
        }
        if let Some(merged_at) = self
            .merged_at
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            activity.push(GitHubPullRequestActivity {
                id: format!("merged:{}", self.number.unwrap_or_default()),
                kind: GitHubPullRequestActivityKind::Event,
                actor_login: self
                    .merged_by
                    .as_ref()
                    .map(|merged_by| merged_by.login.clone()),
                body: String::new(),
                created_at: bounded_chars(merged_at.to_owned(), 128),
                event: Some("merged".to_owned()),
                url: self.url.clone().filter(|url| valid_https_url(url)),
                path: None,
                line: None,
                start_line: None,
                review_thread_id: None,
            });
        }
        activity.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        activity.truncate(MAX_GH_TIMELINE_ITEMS);
        activity
    }

    fn try_into_detail(
        self,
        expected: &GitHubPullRequestIdentity,
        account_login: &str,
    ) -> Result<GitHubPullRequestDetail, GitHubError> {
        let activity = self.baseline_activity();
        if self.number != Some(expected.number) {
            return Err(GitHubError::InvalidOutput);
        }
        let url = self.url.ok_or(GitHubError::InvalidOutput)?;
        validate_pull_request_url(&url, expected)?;
        let author_login = self.author.map(|author| author.login);
        let rollup = self
            .status_check_rollup
            .map(|nodes| StatusCheckRollupRecord {
                contexts: StatusCheckContextsRecord {
                    total_count: nodes.len() as u64,
                    nodes,
                },
            });
        let summary = GitHubPullRequestSummary {
            identity: expected.clone(),
            node_id: format!(
                "{}/{}/{}#{}",
                expected.hostname, expected.owner, expected.repository, expected.number
            ),
            title: bounded_chars(self.title.unwrap_or_default(), 1_024),
            url,
            state: pull_request_state(self.state.as_deref().unwrap_or_default())?,
            is_draft: self.is_draft.unwrap_or(false),
            is_author: author_login
                .as_deref()
                .is_some_and(|author| author.eq_ignore_ascii_case(account_login)),
            author_login,
            base_branch: bounded_chars(self.base_ref_name.unwrap_or_default(), 1_024),
            head_branch: bounded_chars(self.head_ref_name.unwrap_or_default(), 1_024),
            additions: self.additions.unwrap_or_default(),
            deletions: self.deletions.unwrap_or_default(),
            created_at: bounded_chars(self.created_at.unwrap_or_default(), 128),
            updated_at: bounded_chars(self.updated_at.unwrap_or_default(), 128),
            ci_status: ci_status(rollup.as_ref()),
        };
        let head_revision = self.head_ref_oid.unwrap_or_default();
        if head_revision.is_empty()
            || head_revision.len() > 256
            || head_revision.chars().any(char::is_control)
        {
            return Err(GitHubError::InvalidOutput);
        }
        Ok(GitHubPullRequestDetail {
            summary,
            body: bounded_chars(self.body.unwrap_or_default(), MAX_GH_BODY_CHARS),
            head_revision,
            review_decision: self
                .review_decision
                .map(|decision| bounded_chars(decision, 64)),
            mergeable: self.mergeable.map(|value| bounded_chars(value, 64)),
            merge_state_status: self
                .merge_state_status
                .map(|value| bounded_chars(value, 64)),
            checks: Vec::new(),
            activity,
            checks_partial: false,
            activity_partial: false,
        })
    }
}

fn pull_request_state(value: &str) -> Result<GitHubPullRequestState, GitHubError> {
    match value.to_ascii_uppercase().as_str() {
        "OPEN" => Ok(GitHubPullRequestState::Open),
        "CLOSED" => Ok(GitHubPullRequestState::Closed),
        "MERGED" => Ok(GitHubPullRequestState::Merged),
        _ => Err(GitHubError::InvalidOutput),
    }
}

fn ci_status(rollup: Option<&StatusCheckRollupRecord>) -> GitHubCiStatus {
    let Some(rollup) = rollup else {
        return GitHubCiStatus::None;
    };
    let mut passing = false;
    let mut pending = rollup.contexts.total_count > rollup.contexts.nodes.len() as u64;
    for check in rollup.contexts.nodes.iter().flatten() {
        let value = check
            .conclusion
            .as_deref()
            .or(check.state.as_deref())
            .or(check.status.as_deref())
            .unwrap_or_default()
            .to_ascii_uppercase();
        if matches!(
            value.as_str(),
            "FAILURE"
                | "ERROR"
                | "FAIL"
                | "CANCELLED"
                | "CANCELED"
                | "TIMED_OUT"
                | "ACTION_REQUIRED"
        ) {
            return GitHubCiStatus::Failing;
        }
        if matches!(
            value.as_str(),
            "PENDING" | "QUEUED" | "IN_PROGRESS" | "EXPECTED" | ""
        ) {
            pending = true;
        } else {
            passing = true;
        }
        let _ = &check.kind;
    }
    if pending {
        GitHubCiStatus::Pending
    } else if passing {
        GitHubCiStatus::Passing
    } else {
        GitHubCiStatus::None
    }
}

impl TryFrom<PullRequestRecord> for GitHubPullRequest {
    type Error = GitHubError;

    fn try_from(record: PullRequestRecord) -> Result<Self, Self::Error> {
        if first_url(&record.url).as_deref() != Some(record.url.as_str()) {
            return Err(GitHubError::InvalidOutput);
        }
        Ok(Self {
            number: record.number,
            title: record.title,
            url: record.url,
            base_branch: record.base_ref_name,
            head_branch: record.head_ref_name,
            is_draft: record.is_draft,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GitHubCiStatus, GitHubCreatePullRequest, GitHubPullRequestIdentity,
        GitHubPullRequestLifecycle, GitHubPullRequestMergeMethod, GitHubPullRequestRelationship,
        GitHubPullRequestReviewEvent, GitHubPullRequestReviewState, GitHubPullRequestSearchFilters,
        PullRequestSearchResponse, create_pull_request_args, first_url, pull_request_body_args,
        pull_request_comment_args, pull_request_merge_args, pull_request_number,
        pull_request_review_args, pull_request_review_state_args, pull_request_search_query,
        pull_request_title_args,
    };

    #[test]
    fn create_arguments_match_stable_draft_and_browser_contracts() {
        let mut request = GitHubCreatePullRequest {
            head_branch: "feature/native-pr".to_owned(),
            base_branch: Some("main".to_owned()),
            is_draft: true,
            open_in_browser: false,
            title: String::new(),
            body: String::new(),
        };
        assert_eq!(
            create_pull_request_args(&request, "Native PR", "## Summary"),
            [
                "pr",
                "create",
                "--head",
                "feature/native-pr",
                "--title",
                "Native PR",
                "--base",
                "main",
                "--draft",
                "--body",
                "## Summary"
            ]
        );

        request.open_in_browser = true;
        assert_eq!(
            create_pull_request_args(&request, "Native PR", "## Summary"),
            [
                "pr",
                "create",
                "--web",
                "--head",
                "feature/native-pr",
                "--title",
                "Native PR",
                "--base",
                "main",
                "--body",
                "## Summary"
            ]
        );
    }

    #[test]
    fn parses_bounded_pull_request_urls_and_numbers() {
        assert_eq!(
            first_url("Created https://github.com/Kiwunaka/codexRS/pull/42\n")
                .as_deref()
                .and_then(pull_request_number),
            Some(42)
        );
        assert!(first_url("file:///tmp/not-supported").is_none());
    }

    #[test]
    fn mutation_arguments_match_the_stable_guarded_contracts() {
        let identity = GitHubPullRequestIdentity {
            hostname: "github.com".to_owned(),
            owner: "Kiwunaka".to_owned(),
            repository: "codexRS".to_owned(),
            number: 42,
        };
        assert_eq!(
            pull_request_comment_args(&identity, "Ship it"),
            [
                "pr",
                "comment",
                "42",
                "--body",
                "Ship it",
                "--repo",
                "Kiwunaka/codexRS"
            ]
        );
        assert_eq!(
            pull_request_review_args(
                &identity,
                "e3296b1",
                GitHubPullRequestReviewEvent::RequestChanges,
                "Please keep the bound."
            ),
            [
                "api",
                "repos/Kiwunaka/codexRS/pulls/42/reviews",
                "--method",
                "POST",
                "-f",
                "commit_id=e3296b1",
                "-f",
                "event=REQUEST_CHANGES",
                "-f",
                "body=Please keep the bound.",
                "--hostname",
                "github.com"
            ]
        );
        assert_eq!(
            pull_request_merge_args(&identity, "e3296b1", GitHubPullRequestMergeMethod::Squash),
            [
                "pr",
                "merge",
                "42",
                "--squash",
                "--match-head-commit",
                "e3296b1",
                "--repo",
                "Kiwunaka/codexRS"
            ]
        );
        assert_eq!(
            pull_request_review_state_args(&identity, GitHubPullRequestReviewState::Draft),
            ["pr", "ready", "42", "--undo", "--repo", "Kiwunaka/codexRS"]
        );
        assert_eq!(
            pull_request_review_state_args(&identity, GitHubPullRequestReviewState::Ready),
            ["pr", "ready", "42", "--repo", "Kiwunaka/codexRS"]
        );
        assert_eq!(
            pull_request_title_args(&identity, "Keep this bounded"),
            [
                "pr",
                "edit",
                "42",
                "--title",
                "Keep this bounded",
                "--repo",
                "Kiwunaka/codexRS"
            ]
        );
        assert_eq!(
            pull_request_body_args(&identity, "Native body"),
            [
                "pr",
                "edit",
                "42",
                "--body",
                "Native body",
                "--repo",
                "Kiwunaka/codexRS"
            ]
        );
    }

    #[test]
    fn search_query_matches_the_stable_relationship_and_lifecycle_contract() {
        assert_eq!(
            pull_request_search_query(&GitHubPullRequestSearchFilters {
                relationship: GitHubPullRequestRelationship::Reviewed,
                lifecycle: GitHubPullRequestLifecycle::Closed,
                text: "native \"review\"".to_owned(),
            }),
            "is:pr reviewed-by:@me is:closed is:unmerged \"native \\\"review\\\"\" \
             sort:updated-desc"
        );
    }

    #[test]
    fn search_response_keeps_identity_stats_and_check_state() {
        let response: PullRequestSearchResponse = serde_json::from_str(
            r#"{"data":{"search":{"issueCount":1,"nodes":[{"__typename":"PullRequest","additions":12,"author":{"login":"Kiwunaka"},"baseRefName":"main","createdAt":"2026-07-25T00:00:00Z","deletions":3,"headRefName":"feature/native-pr","id":"PR_kwDOfixture","isDraft":true,"mergeStateStatus":"CLEAN","mergeable":"MERGEABLE","number":42,"repository":{"name":"codexRS","owner":{"login":"Kiwunaka"}},"state":"OPEN","statusCheckRollup":{"contexts":{"totalCount":1,"nodes":[{"__typename":"CheckRun","conclusion":"SUCCESS","status":"COMPLETED"}]}},"title":"Add native pull request workflow","updatedAt":"2026-07-25T01:00:00Z","url":"https://github.com/Kiwunaka/codexRS/pull/42"}],"pageInfo":{"endCursor":null,"hasNextPage":false}}}}"#,
        )
        .unwrap_or_else(|error| panic!("fixture should parse: {error}"));
        let summary = response
            .data
            .search
            .nodes
            .into_iter()
            .flatten()
            .next()
            .and_then(|record| record.try_into_summary("kiwunaka").ok());
        assert_eq!(
            summary.as_ref().map(|summary| summary.identity.number),
            Some(42)
        );
        assert_eq!(
            summary.as_ref().map(|summary| summary.ci_status),
            Some(GitHubCiStatus::Passing)
        );
        assert_eq!(
            summary.as_ref().map(|summary| summary.is_author),
            Some(true)
        );
    }
}
