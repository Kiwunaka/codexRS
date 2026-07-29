use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread::{self, ThreadId};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, params};

pub const MAX_INLINE_EVENT_BYTES: usize = 8 * 1024 * 1024;
pub const DEFAULT_HISTORY_PAGE_SIZE: usize = 100;
pub const MAX_HISTORY_PAGE_SIZE: usize = 500;
pub const DIAGNOSTIC_LOG_BUDGET_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_PREFERENCE_KEY_BYTES: usize = 128;
pub const MAX_PREFERENCE_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_WORKSPACE_PATH_BYTES: usize = 64 * 1024;
pub const MAX_BROWSER_DOWNLOAD_RECORDS: usize = 200;

const SCHEMA_VERSION: i64 = 2;
const MAX_BROWSER_DOWNLOAD_ID_BYTES: usize = 256;
const MAX_BROWSER_DOWNLOAD_CONTEXT_BYTES: usize = 256;
const MAX_BROWSER_DOWNLOAD_FILENAME_BYTES: usize = 512;
const MAX_BROWSER_DOWNLOAD_PATH_BYTES: usize = 4 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventTooLarge {
    pub actual: usize,
    pub limit: usize,
}

impl fmt::Display for EventTooLarge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "inline event is {} bytes; the limit is {} bytes",
            self.actual, self.limit
        )
    }
}

impl Error for EventTooLarge {}

#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
    Sql(rusqlite::Error),
    WrongThread,
    UnsupportedSchema(i64),
    PreferenceKeyTooLarge,
    PreferenceValueTooLarge,
    WorkspacePathTooLarge,
    BrowserDownloadInvalid,
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Sql(error) => error.fmt(formatter),
            Self::WrongThread => formatter.write_str("storage is owned by another thread"),
            Self::UnsupportedSchema(version) => {
                write!(
                    formatter,
                    "storage schema version {version} is newer than this build"
                )
            }
            Self::PreferenceKeyTooLarge => formatter.write_str("preference key exceeds 128 bytes"),
            Self::PreferenceValueTooLarge => formatter.write_str("preference value exceeds 64 KiB"),
            Self::WorkspacePathTooLarge => {
                formatter.write_str("workspace path exceeds the 64 KiB storage limit")
            }
            Self::BrowserDownloadInvalid => {
                formatter.write_str("browser download record is invalid")
            }
        }
    }
}

impl Error for StoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Sql(error) => Some(error),
            Self::WrongThread
            | Self::UnsupportedSchema(_)
            | Self::PreferenceKeyTooLarge
            | Self::PreferenceValueTooLarge
            | Self::WorkspacePathTooLarge
            | Self::BrowserDownloadInvalid => None,
        }
    }
}

impl From<std::io::Error> for StoreError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sql(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentWorkspace {
    pub path: PathBuf,
    pub last_opened_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserDownloadRecordStatus {
    Failed,
    Canceled,
    Complete,
}

impl BrowserDownloadRecordStatus {
    const fn as_i64(self) -> i64 {
        match self {
            Self::Failed => 0,
            Self::Canceled => 1,
            Self::Complete => 2,
        }
    }

    const fn from_i64(value: i64) -> Option<Self> {
        match value {
            0 => Some(Self::Failed),
            1 => Some(Self::Canceled),
            2 => Some(Self::Complete),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredBrowserDownload {
    pub context_id: String,
    pub filename: String,
    pub id: String,
    pub path: PathBuf,
    pub received_bytes: u64,
    pub started_at_ms: u64,
    pub status: BrowserDownloadRecordStatus,
    pub total_bytes: u64,
    pub updated_at_ms: u64,
    pub user_initiated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_offset: Option<usize>,
}

pub struct Store {
    connection: Connection,
    owner: ThreadId,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        Self::from_connection(connection)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut connection: Connection) -> Result<Self, StoreError> {
        connection.busy_timeout(Duration::from_secs(2))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        migrate(&mut connection)?;
        Ok(Self {
            connection,
            owner: thread::current().id(),
        })
    }

    pub fn preference(&self, key: &str) -> Result<Option<String>, StoreError> {
        self.ensure_owner()?;
        validate_preference(key, "")?;
        self.connection
            .query_row(
                "SELECT value FROM ui_preferences WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::from)
    }

    pub fn set_preference(
        &mut self,
        key: &str,
        value: &str,
        updated_at: i64,
    ) -> Result<(), StoreError> {
        self.ensure_owner()?;
        validate_preference(key, value)?;
        self.connection.execute(
            "INSERT INTO ui_preferences(key, value, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE
             SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, updated_at],
        )?;
        Ok(())
    }

    pub fn remember_workspace(
        &mut self,
        path: &Path,
        last_opened_at: i64,
    ) -> Result<(), StoreError> {
        self.ensure_owner()?;
        let encoded = encode_path(path);
        if encoded.len() > MAX_WORKSPACE_PATH_BYTES {
            return Err(StoreError::WorkspacePathTooLarge);
        }
        self.connection.execute(
            "INSERT INTO recent_workspaces(path, last_opened_at)
             VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE
             SET last_opened_at = excluded.last_opened_at",
            params![encoded, last_opened_at],
        )?;
        Ok(())
    }

    pub fn recent_workspaces(
        &self,
        requested_limit: usize,
        offset: usize,
    ) -> Result<Page<RecentWorkspace>, StoreError> {
        self.ensure_owner()?;
        let limit = bounded_history_page_size(requested_limit);
        let sql_limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let sql_offset = i64::try_from(offset).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare_cached(
            "SELECT path, last_opened_at
             FROM recent_workspaces
             ORDER BY last_opened_at DESC, path ASC
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = statement.query_map(params![sql_limit, sql_offset], |row| {
            let encoded: Vec<u8> = row.get(0)?;
            let last_opened_at = row.get(1)?;
            Ok((encoded, last_opened_at))
        })?;
        let mut items = Vec::with_capacity(limit);
        for row in rows {
            let (encoded, last_opened_at) = row?;
            items.push(RecentWorkspace {
                path: decode_path(encoded),
                last_opened_at,
            });
        }
        let next_offset = (items.len() == limit).then(|| offset.saturating_add(items.len()));
        Ok(Page { items, next_offset })
    }

    pub fn upsert_browser_download(
        &mut self,
        download: &StoredBrowserDownload,
    ) -> Result<(), StoreError> {
        self.ensure_owner()?;
        validate_browser_download(download)?;
        let path = encode_path(&download.path);
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO browser_downloads(
                id, context_id, filename, path, received_bytes, started_at_ms,
                status, total_bytes, updated_at_ms, user_initiated
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET
                context_id = excluded.context_id,
                filename = excluded.filename,
                path = excluded.path,
                received_bytes = excluded.received_bytes,
                started_at_ms = excluded.started_at_ms,
                status = excluded.status,
                total_bytes = excluded.total_bytes,
                updated_at_ms = excluded.updated_at_ms,
                user_initiated = excluded.user_initiated",
            params![
                download.id,
                download.context_id,
                download.filename,
                path,
                sqlite_u64(download.received_bytes),
                sqlite_u64(download.started_at_ms),
                download.status.as_i64(),
                sqlite_u64(download.total_bytes),
                sqlite_u64(download.updated_at_ms),
                i64::from(download.user_initiated),
            ],
        )?;
        transaction.execute(
            "DELETE FROM browser_downloads
             WHERE id NOT IN (
                SELECT id FROM browser_downloads
                ORDER BY updated_at_ms DESC, id ASC
                LIMIT ?1
             )",
            [i64::try_from(MAX_BROWSER_DOWNLOAD_RECORDS).unwrap_or(i64::MAX)],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn remove_browser_download(&mut self, id: &str) -> Result<(), StoreError> {
        self.ensure_owner()?;
        if id.is_empty()
            || id.len() > MAX_BROWSER_DOWNLOAD_ID_BYTES
            || id.chars().any(char::is_control)
        {
            return Err(StoreError::BrowserDownloadInvalid);
        }
        self.connection
            .execute("DELETE FROM browser_downloads WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn browser_downloads(
        &self,
        requested_limit: usize,
        offset: usize,
    ) -> Result<Page<StoredBrowserDownload>, StoreError> {
        self.ensure_owner()?;
        let limit = requested_limit.clamp(1, MAX_BROWSER_DOWNLOAD_RECORDS);
        let mut statement = self.connection.prepare_cached(
            "SELECT id, context_id, filename, path, received_bytes, started_at_ms,
                    status, total_bytes, updated_at_ms, user_initiated
             FROM browser_downloads
             ORDER BY updated_at_ms DESC, id ASC
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = statement.query_map(
            params![
                i64::try_from(limit).unwrap_or(i64::MAX),
                i64::try_from(offset).unwrap_or(i64::MAX)
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                ))
            },
        )?;
        let mut items = Vec::with_capacity(limit);
        for row in rows {
            let (
                id,
                context_id,
                filename,
                path,
                received_bytes,
                started_at_ms,
                status,
                total_bytes,
                updated_at_ms,
                user_initiated,
            ) = row?;
            let Some(status) = BrowserDownloadRecordStatus::from_i64(status) else {
                return Err(StoreError::BrowserDownloadInvalid);
            };
            items.push(StoredBrowserDownload {
                context_id,
                filename,
                id,
                path: decode_path(path),
                received_bytes: received_bytes.max(0) as u64,
                started_at_ms: started_at_ms.max(0) as u64,
                status,
                total_bytes: total_bytes.max(0) as u64,
                updated_at_ms: updated_at_ms.max(0) as u64,
                user_initiated: user_initiated != 0,
            });
        }
        let next_offset = (items.len() == limit).then(|| offset.saturating_add(items.len()));
        Ok(Page { items, next_offset })
    }

    fn ensure_owner(&self) -> Result<(), StoreError> {
        if thread::current().id() == self.owner {
            Ok(())
        } else {
            Err(StoreError::WrongThread)
        }
    }
}

fn migrate(connection: &mut Connection) -> Result<(), StoreError> {
    let version = connection.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?;
    if version > SCHEMA_VERSION {
        return Err(StoreError::UnsupportedSchema(version));
    }
    if version == 0 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "CREATE TABLE ui_preferences (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL,
                updated_at INTEGER NOT NULL
             ) STRICT;
             CREATE TABLE recent_workspaces (
                path BLOB PRIMARY KEY NOT NULL,
                last_opened_at INTEGER NOT NULL
             ) STRICT;
             CREATE TABLE browser_downloads (
                id TEXT PRIMARY KEY NOT NULL,
                context_id TEXT NOT NULL,
                filename TEXT NOT NULL,
                path BLOB NOT NULL,
                received_bytes INTEGER NOT NULL CHECK(received_bytes >= 0),
                started_at_ms INTEGER NOT NULL CHECK(started_at_ms >= 0),
                status INTEGER NOT NULL CHECK(status IN (0, 1, 2)),
                total_bytes INTEGER NOT NULL CHECK(total_bytes >= 0),
                updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms >= 0),
                user_initiated INTEGER NOT NULL CHECK(user_initiated IN (0, 1))
             ) STRICT;
             CREATE INDEX browser_downloads_updated
             ON browser_downloads(updated_at_ms DESC, id ASC);
             PRAGMA user_version = 2;",
        )?;
        transaction.commit()?;
    }
    if version == 1 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "CREATE TABLE browser_downloads (
                id TEXT PRIMARY KEY NOT NULL,
                context_id TEXT NOT NULL,
                filename TEXT NOT NULL,
                path BLOB NOT NULL,
                received_bytes INTEGER NOT NULL CHECK(received_bytes >= 0),
                started_at_ms INTEGER NOT NULL CHECK(started_at_ms >= 0),
                status INTEGER NOT NULL CHECK(status IN (0, 1, 2)),
                total_bytes INTEGER NOT NULL CHECK(total_bytes >= 0),
                updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms >= 0),
                user_initiated INTEGER NOT NULL CHECK(user_initiated IN (0, 1))
             ) STRICT;
             CREATE INDEX browser_downloads_updated
             ON browser_downloads(updated_at_ms DESC, id ASC);
             PRAGMA user_version = 2;",
        )?;
        transaction.commit()?;
    }
    Ok(())
}

fn validate_preference(key: &str, value: &str) -> Result<(), StoreError> {
    if key.is_empty() || key.len() > MAX_PREFERENCE_KEY_BYTES {
        return Err(StoreError::PreferenceKeyTooLarge);
    }
    if value.len() > MAX_PREFERENCE_VALUE_BYTES {
        return Err(StoreError::PreferenceValueTooLarge);
    }
    Ok(())
}

fn validate_browser_download(download: &StoredBrowserDownload) -> Result<(), StoreError> {
    let path = download.path.to_string_lossy();
    if download.id.is_empty()
        || download.id.len() > MAX_BROWSER_DOWNLOAD_ID_BYTES
        || download.id.chars().any(char::is_control)
        || download.context_id.len() > MAX_BROWSER_DOWNLOAD_CONTEXT_BYTES
        || download.context_id.chars().any(char::is_control)
        || download.filename.is_empty()
        || download.filename.len() > MAX_BROWSER_DOWNLOAD_FILENAME_BYTES
        || download.filename.chars().any(char::is_control)
        || !download.path.is_absolute()
        || path.is_empty()
        || path.len() > MAX_BROWSER_DOWNLOAD_PATH_BYTES
        || path.contains('\0')
    {
        return Err(StoreError::BrowserDownloadInvalid);
    }
    Ok(())
}

fn sqlite_u64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(windows)]
fn encode_path(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(windows)]
fn decode_path(bytes: Vec<u8>) -> PathBuf {
    use std::os::windows::ffi::OsStringExt;

    let wide = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    PathBuf::from(OsString::from_wide(&wide))
}

#[cfg(unix)]
fn encode_path(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().to_vec()
}

#[cfg(unix)]
fn decode_path(bytes: Vec<u8>) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;

    PathBuf::from(OsString::from_vec(bytes))
}

pub fn validate_inline_event_size(actual: usize) -> Result<(), EventTooLarge> {
    if actual <= MAX_INLINE_EVENT_BYTES {
        Ok(())
    } else {
        Err(EventTooLarge {
            actual,
            limit: MAX_INLINE_EVENT_BYTES,
        })
    }
}

#[must_use]
pub fn bounded_history_page_size(requested: usize) -> usize {
    requested.clamp(1, MAX_HISTORY_PAGE_SIZE)
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::path::Path;

    use rusqlite::Connection;

    use super::{
        BrowserDownloadRecordStatus, DEFAULT_HISTORY_PAGE_SIZE, MAX_BROWSER_DOWNLOAD_RECORDS,
        MAX_HISTORY_PAGE_SIZE, MAX_INLINE_EVENT_BYTES, Store, StoredBrowserDownload,
        bounded_history_page_size, validate_inline_event_size,
    };

    #[test]
    fn rejects_the_observed_594_mb_failure_without_allocating_it() {
        let error = match validate_inline_event_size(594_127_437) {
            Err(error) => error,
            Ok(()) => panic!("the observed oversized event was accepted"),
        };
        assert_eq!(error.limit, MAX_INLINE_EVENT_BYTES);
        assert_eq!(error.actual, 594_127_437);
    }

    #[test]
    fn accepts_the_inline_limit_exactly() {
        assert!(validate_inline_event_size(MAX_INLINE_EVENT_BYTES).is_ok());
    }

    #[test]
    fn page_sizes_are_always_bounded() {
        assert_eq!(
            bounded_history_page_size(DEFAULT_HISTORY_PAGE_SIZE),
            DEFAULT_HISTORY_PAGE_SIZE
        );
        assert_eq!(bounded_history_page_size(0), 1);
        assert_eq!(bounded_history_page_size(usize::MAX), MAX_HISTORY_PAGE_SIZE);
    }

    #[test]
    fn preferences_and_native_paths_round_trip() -> Result<(), Box<dyn Error>> {
        let mut store = Store::open_in_memory()?;
        store.set_preference("route", "repository", 11)?;
        store.remember_workspace(Path::new(r"C:\workspace\кириллица"), 12)?;

        assert_eq!(store.preference("route")?.as_deref(), Some("repository"));
        let page = store.recent_workspaces(10, 0)?;
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].path, Path::new(r"C:\workspace\кириллица"));
        assert_eq!(page.next_offset, None);
        Ok(())
    }

    #[test]
    fn recent_workspace_pages_are_ordered_and_bounded() -> Result<(), Box<dyn Error>> {
        let mut store = Store::open_in_memory()?;
        for (index, path) in ["one", "two", "three"].into_iter().enumerate() {
            store.remember_workspace(Path::new(path), i64::try_from(index).unwrap_or_default())?;
        }

        let first = store.recent_workspaces(2, 0)?;
        assert_eq!(first.items[0].path, Path::new("three"));
        assert_eq!(first.items[1].path, Path::new("two"));
        assert_eq!(first.next_offset, Some(2));

        let second = store.recent_workspaces(2, 2)?;
        assert_eq!(second.items[0].path, Path::new("one"));
        assert_eq!(second.next_offset, None);
        Ok(())
    }

    #[test]
    fn browser_download_history_is_bounded_ordered_and_removable() -> Result<(), Box<dyn Error>> {
        let mut store = Store::open_in_memory()?;
        let base = if cfg!(windows) {
            Path::new(r"C:\Downloads")
        } else {
            Path::new("/tmp/downloads")
        };
        for index in 0..=MAX_BROWSER_DOWNLOAD_RECORDS {
            let id = format!("download-{index:03}");
            store.upsert_browser_download(&StoredBrowserDownload {
                context_id: "chat".to_owned(),
                filename: format!("{id}.txt"),
                id,
                path: base.join(format!("файл-{index:03}.txt")),
                received_bytes: index as u64,
                started_at_ms: index as u64,
                status: BrowserDownloadRecordStatus::Complete,
                total_bytes: index as u64,
                updated_at_ms: index as u64,
                user_initiated: true,
            })?;
        }

        let first = store.browser_downloads(100, 0)?;
        assert_eq!(first.items.len(), 100);
        assert_eq!(first.items[0].id, "download-200");
        assert_eq!(first.next_offset, Some(100));
        let second = store.browser_downloads(100, 100)?;
        assert_eq!(second.items.len(), 100);
        assert_eq!(
            second.items.last().map(|item| item.id.as_str()),
            Some("download-001")
        );
        assert_eq!(second.next_offset, Some(200));
        assert!(store.browser_downloads(100, 200)?.items.is_empty());

        store.remove_browser_download("download-200")?;
        assert_eq!(
            store
                .browser_downloads(1, 0)?
                .items
                .first()
                .map(|item| item.id.as_str()),
            Some("download-199")
        );
        Ok(())
    }

    #[test]
    fn version_one_storage_migrates_browser_download_history() -> Result<(), Box<dyn Error>> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(
            "CREATE TABLE ui_preferences (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL,
                updated_at INTEGER NOT NULL
             ) STRICT;
             CREATE TABLE recent_workspaces (
                path BLOB PRIMARY KEY NOT NULL,
                last_opened_at INTEGER NOT NULL
             ) STRICT;
             PRAGMA user_version = 1;",
        )?;
        let store = Store::from_connection(connection)?;
        assert!(store.browser_downloads(10, 0)?.items.is_empty());
        Ok(())
    }
}
