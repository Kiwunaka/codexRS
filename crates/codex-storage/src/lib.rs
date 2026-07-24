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

const SCHEMA_VERSION: i64 = 1;

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
            | Self::WorkspacePathTooLarge => None,
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
             PRAGMA user_version = 1;",
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

    use super::{
        DEFAULT_HISTORY_PAGE_SIZE, MAX_HISTORY_PAGE_SIZE, MAX_INLINE_EVENT_BYTES, Store,
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
}
