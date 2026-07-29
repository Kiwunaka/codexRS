use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use winsafe::prelude::{
    kernel_Hkey as _, ole_IUnknown as _, oleaut_IPropertyStore as _, oleaut_Variant as _,
    shell_IEnumShellItems as _, shell_IShellItem as _, shell_IShellItem2 as _,
};
use winsafe::{self as win, co};

use crate::computer_use::{
    PROGRAM_FILES_X64_FOLDER_ID, PROGRAM_FILES_X86_FOLDER_ID, SYSTEM_X64_FOLDER_ID,
    SYSTEM_X86_FOLDER_ID, windows_process_application_id,
};
use crate::{ComputerApplication, MAX_COMPUTER_APPLICATIONS};

const MAX_CATALOG_SOURCE_ENTRIES: usize = 2_048;
const MAX_PACKAGE_DIRECTORIES: usize = 1_024;
const MAX_SHORTCUT_FILES: usize = 512;
const MAX_EXECUTION_ALIASES: usize = 256;
const MAX_START_MENU_DIRECTORIES: usize = 256;
const MAX_START_MENU_DEPTH: usize = 8;
const MAX_SHORTCUT_BYTES: u64 = 1024 * 1024;
const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_MANIFEST_NODES: u32 = 20_000;
const MAX_APPLICATIONS_PER_PACKAGE: usize = 64;
const MAX_APPLICATION_ID_BYTES: usize = 512;
const MAX_DISPLAY_NAME_BYTES: usize = 512;
const MAX_WINDOWS_PATH_BYTES: usize = 32 * 1024;
const MAX_USER_ASSIST_SUBKEYS: usize = 32;
const MAX_USER_ASSIST_SIGNALS: usize = 4_096;
const MAX_USER_ASSIST_VALUE_BYTES: usize = 4 * 1024;
const MAX_USER_ASSIST_SUBKEY_NAME_UNITS: u32 = 128;
const MAX_USER_ASSIST_VALUE_NAME_UNITS: u32 = 32 * 1024;
const MAX_USAGE_KEYS_PER_APP: usize = 16;
const USER_ASSIST_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const USER_ASSIST_ROOT: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\UserAssist";
const USER_PROGRAMS_FOLDER_ID: &str = "{A77F5D77-2E2B-44C3-A6A2-ABA601054A51}";
const COMMON_PROGRAMS_FOLDER_ID: &str = "{0139D44E-6AFE-49F2-8690-3DAFCAE6FFB8}";
type UserAssistCache = Mutex<Option<(Instant, HashMap<String, UserAssistSignal>)>>;
static USER_ASSIST_CACHE: OnceLock<UserAssistCache> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub(crate) enum ComputerLaunchTarget {
    Aumid(String),
    Shortcut(PathBuf),
    Executable(PathBuf),
}

pub(crate) struct ComputerApplicationCatalog {
    pub applications: Vec<ComputerApplication>,
    pub launch_targets: HashMap<String, ComputerLaunchTarget>,
}

#[derive(Debug)]
struct CatalogEntry {
    application: ComputerApplication,
    launch_target: ComputerLaunchTarget,
    priority: u8,
    usage_keys: Vec<String>,
    last_used_filetime: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UserAssistSignal {
    last_used_filetime: u64,
    last_used_date: String,
    use_count: u32,
}

fn catalog_entry(
    application: ComputerApplication,
    launch_target: ComputerLaunchTarget,
    priority: u8,
) -> CatalogEntry {
    let mut usage_keys = Vec::with_capacity(2);
    push_usage_key(&mut usage_keys, &application.id);
    match &launch_target {
        ComputerLaunchTarget::Aumid(identifier) => push_usage_key(&mut usage_keys, identifier),
        ComputerLaunchTarget::Shortcut(path) => {
            push_usage_key(&mut usage_keys, &path.to_string_lossy());
        }
        ComputerLaunchTarget::Executable(path) => {
            push_usage_key(&mut usage_keys, &path.to_string_lossy());
            push_executable_process_key(&mut usage_keys, &path.to_string_lossy());
        }
    }
    CatalogEntry {
        application,
        launch_target,
        priority,
        usage_keys,
        last_used_filetime: None,
    }
}

pub(crate) fn discover_windows_computer_apps() -> ComputerApplicationCatalog {
    let mut entries = BTreeMap::<String, CatalogEntry>::new();
    discover_apps_folder(&mut entries);
    discover_packaged_apps(&mut entries);
    discover_execution_aliases(&mut entries);
    discover_start_menu_shortcuts(&mut entries);
    apply_user_assist_signals(&mut entries);

    let mut entries = entries.into_iter().collect::<Vec<_>>();
    sort_catalog_entries(&mut entries);
    entries.truncate(MAX_COMPUTER_APPLICATIONS);

    let mut applications = Vec::with_capacity(entries.len());
    let mut launch_targets = HashMap::with_capacity(entries.len());
    for (id, entry) in entries {
        launch_targets.insert(id, entry.launch_target);
        applications.push(entry.application);
    }
    ComputerApplicationCatalog {
        applications,
        launch_targets,
    }
}

fn sort_catalog_entries(entries: &mut [(String, CatalogEntry)]) {
    entries.sort_by(|(_, left), (_, right)| {
        right
            .last_used_filetime
            .cmp(&left.last_used_filetime)
            .then_with(|| {
                left.application
                    .display_name
                    .as_deref()
                    .unwrap_or(&left.application.id)
                    .to_lowercase()
                    .cmp(
                        &right
                            .application
                            .display_name
                            .as_deref()
                            .unwrap_or(&right.application.id)
                            .to_lowercase(),
                    )
            })
            .then_with(|| right.application.use_count.cmp(&left.application.use_count))
            .then_with(|| left.application.id.cmp(&right.application.id))
    });
}

fn discover_apps_folder(entries: &mut BTreeMap<String, CatalogEntry>) {
    let Ok(folder) = win::SHCreateItemFromParsingName::<win::IShellItem>(
        "shell:AppsFolder",
        None::<&win::IBindCtx>,
    ) else {
        return;
    };
    let Ok(items) =
        folder.BindToHandler::<win::IEnumShellItems>(None::<&win::IBindCtx>, &co::BHID::EnumItems)
    else {
        return;
    };

    for _ in 0..MAX_CATALOG_SOURCE_ENTRIES {
        if entries.len() >= MAX_CATALOG_SOURCE_ENTRIES {
            return;
        }
        let item = match items.Next() {
            Ok(Some(item)) => item,
            Ok(None) | Err(_) => return,
        };
        let display_name = shell_item_name(&item, co::SIGDN::NORMALDISPLAY, MAX_DISPLAY_NAME_BYTES)
            .and_then(|value| display_name_candidate(&value));
        let application_id = shell_item_app_user_model_id(&item);
        let parsing_name = shell_item_name(
            &item,
            co::SIGDN::PARENTRELATIVEPARSING,
            MAX_WINDOWS_PATH_BYTES,
        )
        .or_else(|| {
            shell_item_name(
                &item,
                co::SIGDN::DESKTOPABSOLUTEPARSING,
                MAX_WINDOWS_PATH_BYTES,
            )
        });
        let file_system_path =
            shell_item_name(&item, co::SIGDN::FILESYSPATH, MAX_WINDOWS_PATH_BYTES);
        let target_parsing_path = shell_item_link_target_parsing_path(&item);
        let launch_path = target_parsing_path
            .as_deref()
            .filter(|value| apps_folder_launch_path(value).is_some())
            .or(file_system_path.as_deref());
        if let Some(entry) = application_id
            .as_deref()
            .or(parsing_name.as_deref())
            .and_then(|name| application_from_apps_folder_item(name, display_name, launch_path))
        {
            insert_entry(entries, entry);
        }
    }
}

fn shell_item_app_user_model_id(item: &win::IShellItem) -> Option<String> {
    const APP_USER_MODEL_ID: win::PROPERTYKEY = win::PROPERTYKEY {
        fmtid: win::GUID::new("9f4c2855-9f79-4b39-a8d0-e1d42de1d5f3"),
        pid: 5,
    };

    shell_item_lpwstr_property(item, &APP_USER_MODEL_ID, MAX_APPLICATION_ID_BYTES)
}

fn shell_item_link_target_parsing_path(item: &win::IShellItem) -> Option<String> {
    const LINK_TARGET_PARSING_PATH: win::PROPERTYKEY = win::PROPERTYKEY {
        fmtid: win::GUID::new("b9b4b3fc-2b51-4a42-b5d8-324146afcf25"),
        pid: 2,
    };

    shell_item_lpwstr_property(item, &LINK_TARGET_PARSING_PATH, MAX_WINDOWS_PATH_BYTES)
}

fn shell_item_lpwstr_property(
    item: &win::IShellItem,
    key: &win::PROPERTYKEY,
    max_bytes: usize,
) -> Option<String> {
    let item = item.QueryInterface::<win::IShellItem2>().ok()?;
    let properties = item.GetPropertyStore(co::GPS::DEFAULT).ok()?;
    let value = properties.GetValue(key).ok()?;
    bounded_lpwstr_property(&value, max_bytes)
}

fn bounded_lpwstr_property(value: &win::PROPVARIANT, max_bytes: usize) -> Option<String> {
    if value.vt() == co::VT::BSTR {
        let value = value.bstr()?;
        return (value.len() <= max_bytes).then_some(value);
    }
    if value.vt() != co::VT::LPWSTR {
        return None;
    }
    let pointer = usize::from_ne_bytes(
        value
            .raw()
            .get(..std::mem::size_of::<usize>())?
            .try_into()
            .ok()?,
    ) as *const u16;
    if pointer.is_null() {
        return None;
    }
    let mut units = Vec::with_capacity(max_bytes.min(128));
    for index in 0..=max_bytes {
        // The PROPVARIANT owns a null-terminated LPWSTR for its lifetime.
        // Read one valid code unit at a time through winsafe's safe copy API,
        // stopping at the terminator and never scanning an unbounded string.
        let unit = win::WString::from_wchars_count(pointer.wrapping_add(index), 1)
            .as_slice()
            .first()
            .copied()?;
        if unit == 0 {
            let value = String::from_utf16(&units).ok()?;
            return (value.len() <= max_bytes).then_some(value);
        }
        if index == max_bytes {
            return None;
        }
        units.push(unit);
    }
    None
}

fn shell_item_name(item: &win::IShellItem, kind: co::SIGDN, max_bytes: usize) -> Option<String> {
    let value = item.GetDisplayName(kind).ok()?;
    (value.len() <= max_bytes).then_some(value)
}

fn application_from_apps_folder_item(
    parsing_name: &str,
    display_name: Option<String>,
    file_system_path: Option<&str>,
) -> Option<CatalogEntry> {
    let parsing_name = parsing_name.trim();
    if parsing_name.len() > MAX_WINDOWS_PATH_BYTES || parsing_name.chars().any(char::is_control) {
        return None;
    }
    let application_id =
        strip_ascii_prefix(parsing_name, r"shell:AppsFolder\").unwrap_or(parsing_name);
    if valid_apps_folder_id(application_id) {
        let application_id = application_id.to_owned();
        let mut entry = catalog_entry(
            empty_application(application_id.clone(), display_name),
            ComputerLaunchTarget::Aumid(application_id),
            4,
        );
        add_apps_folder_file_system_alias(&mut entry, file_system_path);
        return Some(entry);
    }

    if let Some(resolved) = resolve_user_assist_known_folder(application_id) {
        let path = apps_folder_launch_path(&resolved)?;
        let metadata = std::fs::symlink_metadata(path).ok()?;
        if !metadata.is_file() && !metadata.file_type().is_symlink() {
            return None;
        }
        let mut entry = catalog_entry(
            empty_application(application_id.to_owned(), display_name),
            ComputerLaunchTarget::Executable(path.to_path_buf()),
            4,
        );
        add_apps_folder_file_system_alias(&mut entry, file_system_path);
        return Some(entry);
    }

    let path = apps_folder_launch_path(application_id)
        .or_else(|| file_system_path.and_then(apps_folder_launch_path))?;
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() && !metadata.file_type().is_symlink() {
        return None;
    }
    let application_id = windows_process_application_id(path);
    if application_id.is_empty() {
        return None;
    }
    let mut entry = catalog_entry(
        empty_application(application_id, display_name),
        ComputerLaunchTarget::Executable(path.to_path_buf()),
        4,
    );
    add_apps_folder_file_system_alias(&mut entry, file_system_path);
    Some(entry)
}

fn apps_folder_launch_path(value: &str) -> Option<&Path> {
    let path = Path::new(value.trim());
    (path.is_absolute() && (has_extension(path, "exe") || has_extension(path, "chm")))
        .then_some(path)
}

fn add_apps_folder_file_system_alias(entry: &mut CatalogEntry, value: Option<&str>) {
    let Some(value) = value else {
        return;
    };
    let path = Path::new(value.trim());
    if path.is_absolute() && has_extension(path, "exe") {
        push_usage_key(&mut entry.usage_keys, &path.to_string_lossy());
        push_executable_process_key(&mut entry.usage_keys, &path.to_string_lossy());
    }
}

fn valid_apps_folder_id(value: &str) -> bool {
    valid_aumid(value)
        || value
            .get(.."Microsoft.AutoGenerated.".len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("Microsoft.AutoGenerated."))
            && value
                .get("Microsoft.AutoGenerated.".len()..)
                .is_some_and(valid_registry_guid_name)
        || value.len() <= MAX_APPLICATION_ID_BYTES
            && value.contains('.')
            && value.as_bytes()[0].is_ascii_alphabetic()
            && value.split('.').all(|part| {
                !part.is_empty()
                    && part
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            })
}

fn apply_user_assist_signals(entries: &mut BTreeMap<String, CatalogEntry>) {
    let signals = discover_user_assist_signals();
    for entry in entries.values_mut() {
        let signal = catalog_usage_keys(entry)
            .into_iter()
            .filter_map(|key| signals.get(&key))
            .max_by(|left, right| {
                left.last_used_filetime
                    .cmp(&right.last_used_filetime)
                    .then_with(|| left.use_count.cmp(&right.use_count))
            })
            .cloned();
        if let Some(signal) = signal {
            entry.last_used_filetime = Some(signal.last_used_filetime);
            entry.application.last_used_date = Some(signal.last_used_date);
            entry.application.use_count = Some(signal.use_count);
        }
    }
}

fn discover_user_assist_signals() -> HashMap<String, UserAssistSignal> {
    let cache = USER_ASSIST_CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(cache) = cache.lock()
        && let Some((cached_at, signals)) = cache.as_ref()
        && cached_at.elapsed() < USER_ASSIST_CACHE_TTL
    {
        return signals.clone();
    }

    let signals = scan_user_assist_signals();
    if let Ok(mut cache) = cache.lock() {
        *cache = Some((Instant::now(), signals.clone()));
    }
    signals
}

fn scan_user_assist_signals() -> HashMap<String, UserAssistSignal> {
    let mut signals = HashMap::new();
    let Ok(root) = win::HKEY::CURRENT_USER.RegOpenKeyEx(
        Some(USER_ASSIST_ROOT),
        co::REG_OPTION::default(),
        co::KEY::READ,
    ) else {
        return signals;
    };
    let mut subkey_count = 0_u32;
    let mut max_subkey_name_units = 0_u32;
    if root
        .RegQueryInfoKey(
            None,
            Some(&mut subkey_count),
            Some(&mut max_subkey_name_units),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .is_err()
        || max_subkey_name_units > MAX_USER_ASSIST_SUBKEY_NAME_UNITS
    {
        return signals;
    }
    let Ok(subkeys) = root.RegEnumKeyEx() else {
        return signals;
    };

    let mut scanned_values = 0_usize;
    for subkey in subkeys.take(
        usize::try_from(subkey_count)
            .unwrap_or(MAX_USER_ASSIST_SUBKEYS)
            .min(MAX_USER_ASSIST_SUBKEYS),
    ) {
        let Ok(subkey) = subkey else {
            break;
        };
        if !valid_registry_guid_name(&subkey) {
            continue;
        }
        let count_path = format!("{subkey}\\Count");
        let Ok(count_key) =
            root.RegOpenKeyEx(Some(&count_path), co::REG_OPTION::default(), co::KEY::READ)
        else {
            continue;
        };
        let mut value_count = 0_u32;
        let mut max_value_name_units = 0_u32;
        let mut max_value_bytes = 0_u32;
        if count_key
            .RegQueryInfoKey(
                None,
                None,
                None,
                None,
                Some(&mut value_count),
                Some(&mut max_value_name_units),
                Some(&mut max_value_bytes),
                None,
                None,
            )
            .is_err()
            || max_value_name_units > MAX_USER_ASSIST_VALUE_NAME_UNITS
            || max_value_bytes as usize > MAX_USER_ASSIST_VALUE_BYTES
        {
            continue;
        }
        let Ok(values) = count_key.RegEnumValue() else {
            continue;
        };
        let remaining = MAX_USER_ASSIST_SIGNALS.saturating_sub(scanned_values);
        for value in values.take(
            usize::try_from(value_count)
                .unwrap_or(remaining)
                .min(remaining),
        ) {
            scanned_values += 1;
            let Ok((encoded_name, value_type)) = value else {
                break;
            };
            if value_type != co::REG::BINARY
                || encoded_name.is_empty()
                || encoded_name.len() > MAX_WINDOWS_PATH_BYTES
                || encoded_name.chars().any(char::is_control)
            {
                continue;
            }
            let Ok(win::RegistryValue::Binary(data)) =
                count_key.RegQueryValueEx(Some(&encoded_name))
            else {
                continue;
            };
            if data.len() > MAX_USER_ASSIST_VALUE_BYTES {
                continue;
            }
            let Some(signal) = parse_user_assist_signal(&data) else {
                continue;
            };
            let decoded_name = rot13_ascii(&encoded_name);
            for key in user_assist_usage_keys(&decoded_name) {
                insert_user_assist_signal(&mut signals, key, signal.clone());
            }
        }
        if scanned_values >= MAX_USER_ASSIST_SIGNALS {
            break;
        }
    }
    signals
}

fn parse_user_assist_signal(data: &[u8]) -> Option<UserAssistSignal> {
    let use_count = read_le_u32(data, 4)?;
    let filetime_offset = if data.len() >= 72 {
        60
    } else if data.len() == 16 {
        8
    } else {
        return None;
    };
    let filetime = read_le_u64(data, filetime_offset)?;
    Some(UserAssistSignal {
        last_used_filetime: filetime,
        last_used_date: filetime_date(filetime)?,
        use_count,
    })
}

fn filetime_date(value: u64) -> Option<String> {
    if value == 0 {
        return None;
    }
    let file_time = win::FILETIME {
        dwLowDateTime: value as u32,
        dwHighDateTime: (value >> 32) as u32,
    };
    let mut system_time = win::SYSTEMTIME::default();
    win::FileTimeToSystemTime(&file_time, &mut system_time).ok()?;
    (system_time.wYear >= 1601
        && (1..=12).contains(&system_time.wMonth)
        && (1..=31).contains(&system_time.wDay))
    .then(|| {
        format!(
            "{:04}-{:02}-{:02}",
            system_time.wYear, system_time.wMonth, system_time.wDay
        )
    })
}

fn read_le_u32(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn read_le_u64(data: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        data.get(offset..offset.checked_add(8)?)?.try_into().ok()?,
    ))
}

fn rot13_ascii(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            'a'..='m' | 'A'..='M' => char::from(character as u8 + 13),
            'n'..='z' | 'N'..='Z' => char::from(character as u8 - 13),
            _ => character,
        })
        .collect()
}

fn user_assist_usage_keys(identifier: &str) -> Vec<String> {
    let identifier = strip_user_assist_kind(identifier);
    let mut keys = Vec::with_capacity(4);
    push_usage_key(&mut keys, identifier);
    push_executable_process_key(&mut keys, identifier);
    if let Some(resolved) = resolve_user_assist_known_folder(identifier) {
        push_usage_key(&mut keys, &resolved);
        push_executable_process_key(&mut keys, &resolved);
    }
    keys
}

fn strip_user_assist_kind(value: &str) -> &str {
    if value
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("UEME_"))
    {
        value
            .split_once(':')
            .map(|(_, identifier)| identifier)
            .unwrap_or(value)
    } else {
        value
    }
}

fn resolve_user_assist_known_folder(value: &str) -> Option<String> {
    let prefix = value.get(..38)?;
    let suffix = value.get(38..)?.trim_start_matches(['\\', '/']);
    if suffix.is_empty() {
        return None;
    }
    let base = if prefix.eq_ignore_ascii_case(PROGRAM_FILES_X64_FOLDER_ID) {
        std::env::var_os("ProgramW6432")
            .or_else(|| std::env::var_os("ProgramFiles"))
            .map(PathBuf::from)
    } else if prefix.eq_ignore_ascii_case(PROGRAM_FILES_X86_FOLDER_ID) {
        std::env::var_os("ProgramFiles(x86)").map(PathBuf::from)
    } else if prefix.eq_ignore_ascii_case(SYSTEM_X64_FOLDER_ID) {
        std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .map(|path| path.join("System32"))
    } else if prefix.eq_ignore_ascii_case(SYSTEM_X86_FOLDER_ID) {
        std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .map(|path| path.join("SysWOW64"))
    } else if prefix.eq_ignore_ascii_case(USER_PROGRAMS_FOLDER_ID) {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join(r"Microsoft\Windows\Start Menu\Programs"))
    } else if prefix.eq_ignore_ascii_case(COMMON_PROGRAMS_FOLDER_ID) {
        std::env::var_os("ProgramData")
            .map(PathBuf::from)
            .map(|path| path.join(r"Microsoft\Windows\Start Menu\Programs"))
    } else {
        None
    }?;
    let resolved = base.join(suffix);
    (resolved.to_string_lossy().len() <= MAX_WINDOWS_PATH_BYTES)
        .then(|| resolved.to_string_lossy().into_owned())
}

fn catalog_usage_keys(entry: &CatalogEntry) -> Vec<String> {
    entry.usage_keys.clone()
}

fn push_usage_key(keys: &mut Vec<String>, value: &str) {
    if keys.len() >= MAX_USAGE_KEYS_PER_APP {
        return;
    }
    let Some(key) = normalize_usage_key(value) else {
        return;
    };
    if !keys.contains(&key) {
        keys.push(key);
    }
}

fn push_executable_process_key(keys: &mut Vec<String>, value: &str) {
    if keys.len() >= MAX_USAGE_KEYS_PER_APP {
        return;
    }
    let value = value.trim();
    let value = strip_ascii_prefix(value, "process:").unwrap_or(value);
    let value = strip_ascii_prefix(value, r"\\?\").unwrap_or(value);
    let Some(file_name) = value.rsplit(['\\', '/']).next() else {
        return;
    };
    if file_name.is_empty()
        || file_name.len() > MAX_APPLICATION_ID_BYTES
        || file_name.chars().any(char::is_control)
        || !file_name
            .get(file_name.len().saturating_sub(4)..)
            .is_some_and(|extension| extension.eq_ignore_ascii_case(".exe"))
    {
        return;
    }
    let key = format!("executable-name:{}", file_name.to_lowercase());
    if !keys.contains(&key) {
        keys.push(key);
    }
}

fn normalize_usage_key(value: &str) -> Option<String> {
    let value = value.trim();
    let value = strip_ascii_prefix(value, "process:").unwrap_or(value);
    let value = strip_ascii_prefix(value, r"shell:AppsFolder\").unwrap_or(value);
    let value = strip_ascii_prefix(value, r"\\?\").unwrap_or(value);
    if value.is_empty()
        || value.len() > MAX_WINDOWS_PATH_BYTES
        || value.chars().any(char::is_control)
    {
        return None;
    }
    Some(value.replace('/', r"\").to_lowercase())
}

fn insert_user_assist_signal(
    signals: &mut HashMap<String, UserAssistSignal>,
    key: String,
    signal: UserAssistSignal,
) {
    match signals.get_mut(&key) {
        Some(existing)
            if signal.last_used_filetime > existing.last_used_filetime
                || signal.last_used_filetime == existing.last_used_filetime
                    && signal.use_count > existing.use_count =>
        {
            *existing = signal;
        }
        Some(_) => {}
        None => {
            signals.insert(key, signal);
        }
    }
}

fn valid_registry_guid_name(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 38
        && bytes.first() == Some(&b'{')
        && bytes.last() == Some(&b'}')
        && bytes[1..37].iter().enumerate().all(|(index, byte)| {
            matches!(index, 8 | 13 | 18 | 23) && *byte == b'-'
                || !matches!(index, 8 | 13 | 18 | 23) && byte.is_ascii_hexdigit()
        })
}

pub(crate) fn explicit_executable_launch_target(app_id: &str) -> Option<ComputerLaunchTarget> {
    let value = app_id.trim();
    if value.is_empty()
        || value.len() > MAX_WINDOWS_PATH_BYTES
        || value.chars().any(char::is_control)
    {
        return None;
    }
    let value = strip_ascii_prefix(value, "process:").unwrap_or(value);
    let path = expand_leading_environment_variable(Path::new(value))?;
    if !path.is_absolute() || !has_extension(&path, "exe") {
        return None;
    }
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    if !metadata.is_file() && !metadata.file_type().is_symlink() {
        return None;
    }
    Some(ComputerLaunchTarget::Executable(path))
}

fn discover_start_menu_shortcuts(entries: &mut BTreeMap<String, CatalogEntry>) {
    let roots = [
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join(r"Microsoft\Windows\Start Menu\Programs")),
        std::env::var_os("ProgramData")
            .map(PathBuf::from)
            .map(|path| path.join(r"Microsoft\Windows\Start Menu\Programs")),
    ];
    let mut shortcut_count = 0_usize;
    let mut directory_count = 0_usize;
    for root in roots.into_iter().flatten() {
        let mut pending = VecDeque::from([(root, 0_usize)]);
        while let Some((directory, depth)) = pending.pop_front() {
            if directory_count >= MAX_START_MENU_DIRECTORIES
                || shortcut_count >= MAX_SHORTCUT_FILES
                || entries.len() >= MAX_CATALOG_SOURCE_ENTRIES
            {
                return;
            }
            directory_count += 1;
            let Ok(read_dir) = std::fs::read_dir(directory) else {
                continue;
            };
            for entry in read_dir.take(MAX_CATALOG_SOURCE_ENTRIES) {
                let Ok(entry) = entry else {
                    continue;
                };
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if file_type.is_symlink() {
                    continue;
                }
                if file_type.is_dir() && depth < MAX_START_MENU_DEPTH {
                    pending.push_back((entry.path(), depth + 1));
                    continue;
                }
                let path = entry.path();
                if !file_type.is_file() || !has_extension(&path, "lnk") {
                    continue;
                }
                shortcut_count += 1;
                if let Some(discovered) = application_from_shortcut(&path) {
                    insert_entry(entries, discovered);
                }
                if shortcut_count >= MAX_SHORTCUT_FILES
                    || entries.len() >= MAX_CATALOG_SOURCE_ENTRIES
                {
                    return;
                }
            }
        }
    }
}

fn application_from_shortcut(path: &Path) -> Option<CatalogEntry> {
    let metadata = path.metadata().ok()?;
    if metadata.len() == 0 || metadata.len() > MAX_SHORTCUT_BYTES {
        return None;
    }
    let shortcut = lnks::Shortcut::load(path).ok()?;
    let display_name = path
        .file_stem()
        .map(|name| bounded_text(name.to_string_lossy().into_owned(), MAX_DISPLAY_NAME_BYTES))
        .filter(|name| !name.is_empty());

    if let Some(application_id) = shortcut_aumid(&shortcut) {
        return Some(catalog_entry(
            empty_application(application_id.clone(), display_name),
            ComputerLaunchTarget::Shortcut(path.to_path_buf()),
            3,
        ));
    }

    let target_path = expand_leading_environment_variable(shortcut.target_path.as_deref()?)?;
    if !target_path.is_absolute() || !has_extension(&target_path, "exe") {
        return None;
    }
    let application_id = windows_process_application_id(&target_path);
    if application_id.is_empty() {
        return None;
    }
    let mut entry = catalog_entry(
        empty_application(application_id, display_name),
        ComputerLaunchTarget::Shortcut(path.to_path_buf()),
        3,
    );
    push_usage_key(&mut entry.usage_keys, &target_path.to_string_lossy());
    push_executable_process_key(&mut entry.usage_keys, &target_path.to_string_lossy());
    Some(entry)
}

fn shortcut_aumid(shortcut: &lnks::Shortcut) -> Option<String> {
    let target_name = shortcut
        .target_path
        .as_deref()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().to_lowercase())?;
    if target_name != "explorer.exe" {
        return None;
    }
    let arguments = shortcut.arguments.as_deref()?.trim().trim_matches('"');
    let application_id = strip_ascii_prefix(arguments, r"shell:AppsFolder\")?.trim();
    valid_aumid(application_id).then(|| application_id.to_owned())
}

fn discover_execution_aliases(entries: &mut BTreeMap<String, CatalogEntry>) {
    let Some(root) = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|path| path.join(r"Microsoft\WindowsApps"))
    else {
        return;
    };
    let Ok(read_dir) = std::fs::read_dir(root) else {
        return;
    };
    for entry in read_dir.take(MAX_EXECUTION_ALIASES) {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if !has_extension(&path, "exe") {
            continue;
        }
        let application_id = windows_process_application_id(&path);
        if application_id.is_empty() {
            continue;
        }
        let display_name = path
            .file_stem()
            .map(|name| bounded_text(name.to_string_lossy().into_owned(), MAX_DISPLAY_NAME_BYTES))
            .filter(|name| !name.is_empty());
        insert_entry(
            entries,
            catalog_entry(
                empty_application(application_id, display_name),
                ComputerLaunchTarget::Executable(path),
                2,
            ),
        );
        if entries.len() >= MAX_CATALOG_SOURCE_ENTRIES {
            return;
        }
    }
}

fn discover_packaged_apps(entries: &mut BTreeMap<String, CatalogEntry>) {
    let Some(root) = std::env::var_os("ProgramW6432")
        .or_else(|| std::env::var_os("ProgramFiles"))
        .map(PathBuf::from)
        .map(|path| path.join("WindowsApps"))
    else {
        return;
    };
    let Ok(read_dir) = std::fs::read_dir(root) else {
        return;
    };
    let mut package_directories = read_dir
        .take(MAX_PACKAGE_DIRECTORIES)
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(|file_type| file_type.is_dir() && !file_type.is_symlink())
                .map(|_| entry.path())
        })
        .collect::<Vec<_>>();
    package_directories.sort_by(|left, right| right.file_name().cmp(&left.file_name()));

    for package_directory in package_directories {
        let Some(package_full_name) = package_directory
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        let manifest_path = package_directory.join("AppxManifest.xml");
        let Some(manifest) = read_bounded_utf8_file(&manifest_path, MAX_MANIFEST_BYTES) else {
            continue;
        };
        for (application_id, display_name) in
            packaged_applications_from_manifest(&manifest, &package_full_name)
        {
            insert_entry(
                entries,
                catalog_entry(
                    empty_application(application_id.clone(), Some(display_name)),
                    ComputerLaunchTarget::Aumid(application_id),
                    1,
                ),
            );
            if entries.len() >= MAX_CATALOG_SOURCE_ENTRIES {
                return;
            }
        }
    }
}

fn packaged_applications_from_manifest(
    manifest: &str,
    package_full_name: &str,
) -> Vec<(String, String)> {
    let Ok(document) = roxmltree::Document::parse_with_options(
        manifest,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_MANIFEST_NODES,
        },
    ) else {
        return Vec::new();
    };
    let Some(identity) = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "Identity")
    else {
        return Vec::new();
    };
    let Some(identity_name) = identity.attribute("Name").filter(|name| {
        !name.is_empty()
            && name.len() <= MAX_APPLICATION_ID_BYTES
            && !name.chars().any(char::is_control)
    }) else {
        return Vec::new();
    };
    let package_prefix = format!("{identity_name}_");
    if !package_full_name
        .get(..package_prefix.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(&package_prefix))
    {
        return Vec::new();
    }
    let Some(publisher_id) = package_full_name
        .rsplit('_')
        .next()
        .filter(|id| valid_publisher_id(id))
    else {
        return Vec::new();
    };
    let package_display_name = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "Properties")
        .and_then(|properties| {
            properties
                .children()
                .find(|node| node.is_element() && node.tag_name().name() == "DisplayName")
        })
        .and_then(|node| node.text())
        .and_then(display_name_candidate);

    document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "Application")
        .take(MAX_APPLICATIONS_PER_PACKAGE)
        .filter_map(|application| {
            let application_name = application.attribute("Id")?.trim();
            if application_name.is_empty()
                || application_name.len() > MAX_APPLICATION_ID_BYTES
                || application_name.contains('!')
                || application_name.chars().any(char::is_control)
            {
                return None;
            }
            let application_id = format!("{identity_name}_{publisher_id}!{application_name}");
            if !valid_aumid(&application_id) {
                return None;
            }
            let display_name = application
                .descendants()
                .find(|node| {
                    node.is_element()
                        && matches!(node.tag_name().name(), "VisualElements" | "DefaultTile")
                })
                .and_then(|node| node.attribute("DisplayName"))
                .and_then(display_name_candidate)
                .or_else(|| {
                    application
                        .attribute("DisplayName")
                        .and_then(display_name_candidate)
                })
                .or_else(|| package_display_name.clone())
                .unwrap_or_else(|| friendly_identity_name(identity_name, application_name));
            Some((application_id, display_name))
        })
        .collect()
}

fn insert_entry(entries: &mut BTreeMap<String, CatalogEntry>, mut entry: CatalogEntry) {
    let id = entry.application.id.to_lowercase();
    if id.is_empty() || id.len() > MAX_APPLICATION_ID_BYTES {
        return;
    }
    match entries.get_mut(&id) {
        Some(existing) if existing.priority > entry.priority => {
            merge_usage_keys(&mut existing.usage_keys, entry.usage_keys);
        }
        Some(existing) if existing.priority == entry.priority => {
            if existing.application.display_name.is_none() {
                existing.application.display_name = entry.application.display_name;
            }
            merge_usage_keys(&mut existing.usage_keys, entry.usage_keys);
        }
        Some(existing) => {
            merge_usage_keys(
                &mut entry.usage_keys,
                std::mem::take(&mut existing.usage_keys),
            );
            *existing = entry;
        }
        None => {
            entries.insert(id, entry);
        }
    }
}

fn merge_usage_keys(target: &mut Vec<String>, source: Vec<String>) {
    for key in source {
        if target.len() >= MAX_USAGE_KEYS_PER_APP {
            return;
        }
        if !target.contains(&key) {
            target.push(key);
        }
    }
}

fn empty_application(id: String, display_name: Option<String>) -> ComputerApplication {
    ComputerApplication {
        id,
        display_name,
        last_used_date: None,
        use_count: None,
        is_running: false,
        windows: Vec::new(),
    }
}

fn read_bounded_utf8_file(path: &Path, limit: usize) -> Option<String> {
    let mut bytes = Vec::with_capacity(limit.min(16 * 1024));
    File::open(path)
        .ok()?
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > limit {
        return None;
    }
    String::from_utf8(bytes).ok()
}

fn display_name_candidate(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > MAX_DISPLAY_NAME_BYTES
        || value.to_ascii_lowercase().contains("ms-resource:")
        || value.starts_with("@{")
        || value.chars().any(char::is_control)
    {
        return None;
    }
    Some(value.to_owned())
}

fn friendly_identity_name(identity_name: &str, application_name: &str) -> String {
    let identity = identity_name
        .rsplit('.')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(identity_name);
    let value = if application_name.eq_ignore_ascii_case("app") {
        identity.to_owned()
    } else {
        format!("{identity} · {application_name}")
    };
    bounded_text(value, MAX_DISPLAY_NAME_BYTES)
}

fn valid_aumid(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.contains('!')
        && !value.chars().any(|character| {
            character.is_control()
                || character.is_whitespace()
                || matches!(character, '"' | '\'' | '/' | '\\')
        })
}

fn valid_publisher_id(value: &str) -> bool {
    value.len() == 13
        && value.bytes().all(|byte| {
            let byte = byte.to_ascii_lowercase();
            byte.is_ascii_digit()
                || byte.is_ascii_lowercase() && !matches!(byte, b'i' | b'l' | b'o' | b'u')
        })
}

fn expand_leading_environment_variable(path: &Path) -> Option<PathBuf> {
    let value = path.to_string_lossy();
    if !value.starts_with('%') {
        return (value.len() <= MAX_WINDOWS_PATH_BYTES).then(|| path.to_path_buf());
    }
    let end = value.get(1..)?.find('%')?.checked_add(1)?;
    let variable = value.get(1..end)?;
    if variable.is_empty()
        || variable.len() > 64
        || !variable
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return None;
    }
    let base = std::env::var_os(variable).map(PathBuf::from)?;
    let suffix = value.get(end + 1..)?.trim_start_matches(['\\', '/']);
    let expanded = if suffix.is_empty() {
        base
    } else {
        base.join(suffix)
    };
    (expanded.to_string_lossy().len() <= MAX_WINDOWS_PATH_BYTES).then_some(expanded)
}

fn has_extension(path: &Path, expected: &str) -> bool {
    path.extension()
        .is_some_and(|extension| extension.to_string_lossy().eq_ignore_ascii_case(expected))
}

fn strip_ascii_prefix<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    value
        .get(..prefix.len())
        .filter(|candidate| candidate.eq_ignore_ascii_case(prefix))
        .and_then(|_| value.get(prefix.len()..))
}

fn bounded_text(mut value: String, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value;
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value.truncate(boundary);
    value
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::{
        COMMON_PROGRAMS_FOLDER_ID, ComputerLaunchTarget, MAX_MANIFEST_BYTES,
        USER_PROGRAMS_FOLDER_ID, application_from_apps_folder_item, catalog_entry,
        display_name_candidate, empty_application, insert_entry, normalize_usage_key,
        packaged_applications_from_manifest, parse_user_assist_signal, read_bounded_utf8_file,
        resolve_user_assist_known_folder, rot13_ascii, sort_catalog_entries, strip_ascii_prefix,
        user_assist_usage_keys, valid_apps_folder_id, valid_aumid, valid_registry_guid_name,
    };

    #[test]
    fn packaged_manifest_produces_canonical_aumids_and_safe_display_names() {
        let manifest = r#"<?xml version="1.0" encoding="utf-8"?>
            <Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
                     xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10">
              <Identity Name="OpenAI.Codex" Publisher="CN=fixture" Version="1.0.0.0" />
              <Properties><DisplayName>ms-resource:PackageName</DisplayName></Properties>
              <Applications>
                <Application Id="App" Executable="app\Codex.exe">
                  <uap:VisualElements DisplayName="Codex" />
                </Application>
                <Application Id="Tools" Executable="tools\Codex.exe" />
              </Applications>
            </Package>"#;

        assert_eq!(
            packaged_applications_from_manifest(
                manifest,
                "OpenAI.Codex_1.0.0.0_x64__2p2nqsd0c76g0"
            ),
            [
                (
                    "OpenAI.Codex_2p2nqsd0c76g0!App".to_owned(),
                    "Codex".to_owned()
                ),
                (
                    "OpenAI.Codex_2p2nqsd0c76g0!Tools".to_owned(),
                    "Codex · Tools".to_owned()
                )
            ]
        );
    }

    #[test]
    fn app_identifiers_and_resource_names_are_validated() {
        assert!(valid_aumid("openai.codex_2p2nqsd0c76g0!app"));
        assert!(!valid_aumid(r"shell:AppsFolder\OpenAI.Codex!App"));
        assert!(valid_apps_folder_id("ai.opencode.desktop"));
        assert!(valid_apps_folder_id("Apple.iTunes"));
        assert!(valid_apps_folder_id(
            "Microsoft.AutoGenerated.{01234567-89AB-CDEF-0123-456789ABCDEF}"
        ));
        assert!(!valid_apps_folder_id("1invalid.application"));
        assert!(display_name_candidate("ms-resource:Name").is_none());
        assert_eq!(
            strip_ascii_prefix("PROCESS:C:\\App.exe", "process:"),
            Some("C:\\App.exe")
        );
    }

    #[test]
    fn apps_folder_items_keep_shell_ids_and_accept_observed_shell_targets()
    -> Result<(), Box<dyn std::error::Error>> {
        let Some(entry) = application_from_apps_folder_item(
            "Apple.iTunes",
            Some("iTunes".to_owned()),
            Some(r"C:\Program Files\iTunes\iTunes.exe"),
        ) else {
            panic!("valid AppsFolder identifier was rejected");
        };
        assert_eq!(entry.application.id, "Apple.iTunes");
        assert_eq!(entry.application.display_name.as_deref(), Some("iTunes"));
        assert_eq!(
            entry.launch_target,
            ComputerLaunchTarget::Aumid("Apple.iTunes".to_owned())
        );
        assert!(
            entry
                .usage_keys
                .contains(&r"c:\program files\itunes\itunes.exe".to_owned())
        );
        assert!(
            entry
                .usage_keys
                .contains(&"executable-name:itunes.exe".to_owned())
        );
        assert!(
            application_from_apps_folder_item(
                r"C:\Users\fixture\Downloads\archive.7z",
                Some("archive".to_owned()),
                None,
            )
            .is_none()
        );

        let fixture =
            std::env::temp_dir().join(format!("codexrs-computer-apps-{}.chm", std::process::id()));
        std::fs::write(&fixture, b"fixture")?;
        let fallback = application_from_apps_folder_item(
            "::{opaque-shell-item}",
            Some("Help".to_owned()),
            Some(&fixture.to_string_lossy()),
        );
        let _ = std::fs::remove_file(&fixture);
        let Some(fallback) = fallback else {
            panic!("filesystem-backed AppsFolder item was rejected");
        };
        assert_eq!(
            fallback.launch_target,
            ComputerLaunchTarget::Executable(fixture.clone())
        );
        assert_eq!(fallback.application.id, fixture.to_string_lossy());
        Ok(())
    }

    #[test]
    fn higher_priority_apps_folder_entries_keep_shortcut_usage_aliases() {
        let mut entries = BTreeMap::new();
        insert_entry(
            &mut entries,
            catalog_entry(
                empty_application("Contoso.App".to_owned(), Some("Contoso".to_owned())),
                ComputerLaunchTarget::Aumid("Contoso.App".to_owned()),
                4,
            ),
        );
        insert_entry(
            &mut entries,
            catalog_entry(
                empty_application("Contoso.App".to_owned(), Some("Contoso".to_owned())),
                ComputerLaunchTarget::Shortcut(PathBuf::from(
                    r"C:\Users\fixture\Start Menu\Contoso.lnk",
                )),
                3,
            ),
        );

        let Some(entry) = entries.get("contoso.app") else {
            panic!("catalog entry missing");
        };
        assert_eq!(
            entry.launch_target,
            ComputerLaunchTarget::Aumid("Contoso.App".to_owned())
        );
        assert!(entry.usage_keys.contains(&"contoso.app".to_owned()));
        assert!(
            entry
                .usage_keys
                .contains(&r"c:\users\fixture\start menu\contoso.lnk".to_owned())
        );
    }

    #[test]
    fn executable_process_keys_correlate_different_observed_paths() {
        let entry = catalog_entry(
            empty_application(
                r"{7C5A40EF-A0FB-4BFC-874A-C0F2E0B9FA8E}\Fixture\App.exe".to_owned(),
                Some("Fixture".to_owned()),
            ),
            ComputerLaunchTarget::Executable(PathBuf::from(
                r"C:\Program Files (x86)\Fixture\App.exe",
            )),
            4,
        );
        let signal_keys = user_assist_usage_keys(r"D:\Portable\Fixture\App.exe");

        assert!(
            entry
                .usage_keys
                .contains(&"executable-name:app.exe".to_owned())
        );
        assert!(signal_keys.iter().any(|key| entry.usage_keys.contains(key)));
        assert_eq!(user_assist_usage_keys("Contoso.App"), ["contoso.app"]);
    }

    #[test]
    fn user_assist_records_match_the_modern_windows_layout() {
        let mut record = [0_u8; 72];
        record[4..8].copy_from_slice(&17_u32.to_le_bytes());
        record[60..68].copy_from_slice(&133_486_272_000_000_000_u64.to_le_bytes());

        let Some(signal) = parse_user_assist_signal(&record) else {
            panic!("valid UserAssist record was rejected");
        };
        assert_eq!(signal.last_used_filetime, 133_486_272_000_000_000);
        assert_eq!(signal.last_used_date, "2024-01-02");
        assert_eq!(signal.use_count, 17);
        assert!(parse_user_assist_signal(&record[..68]).is_none());
        assert_eq!(rot13_ascii(r"P:\Hfref\кириллица"), r"C:\Users\кириллица");
        assert_eq!(
            normalize_usage_key(r"PROCESS:\\?\C:/Program Files/App/app.exe").as_deref(),
            Some(r"c:\program files\app\app.exe")
        );
        if let Some(app_data) = std::env::var_os("APPDATA") {
            assert_eq!(
                resolve_user_assist_known_folder(&format!(
                    r"{USER_PROGRAMS_FOLDER_ID}\Fixture\App.lnk"
                )),
                Some(
                    PathBuf::from(app_data)
                        .join(r"Microsoft\Windows\Start Menu\Programs\Fixture\App.lnk")
                        .to_string_lossy()
                        .into_owned()
                )
            );
        }
        if let Some(program_data) = std::env::var_os("ProgramData") {
            assert_eq!(
                resolve_user_assist_known_folder(&format!(
                    r"{COMMON_PROGRAMS_FOLDER_ID}\Fixture\App.lnk"
                )),
                Some(
                    PathBuf::from(program_data)
                        .join(r"Microsoft\Windows\Start Menu\Programs\Fixture\App.lnk")
                        .to_string_lossy()
                        .into_owned()
                )
            );
        }
        assert!(valid_registry_guid_name(
            "{CEBFF5CD-ACE2-4F4F-9178-9926F41749EA}"
        ));
        assert!(!valid_registry_guid_name("Settings"));
    }

    #[test]
    fn catalog_ranking_uses_full_user_assist_time_before_display_name() {
        let mut older = catalog_entry(
            empty_application("older.app".to_owned(), Some("A".to_owned())),
            ComputerLaunchTarget::Aumid("older.app".to_owned()),
            4,
        );
        older.last_used_filetime = Some(10);
        older.application.last_used_date = Some("2026-07-24".to_owned());
        older.application.use_count = Some(100);

        let mut newer = catalog_entry(
            empty_application("newer.app".to_owned(), Some("Z".to_owned())),
            ComputerLaunchTarget::Aumid("newer.app".to_owned()),
            4,
        );
        newer.last_used_filetime = Some(11);
        newer.application.last_used_date = Some("2026-07-24".to_owned());
        newer.application.use_count = Some(0);

        let mut entries = vec![
            ("older.app".to_owned(), older),
            ("newer.app".to_owned(), newer),
        ];
        sort_catalog_entries(&mut entries);

        assert_eq!(entries[0].0, "newer.app");
    }

    #[test]
    fn bounded_manifest_reader_rejects_oversized_input() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = std::env::temp_dir().join(format!(
            "codexrs-computer-apps-manifest-{}.xml",
            std::process::id()
        ));
        std::fs::write(&fixture, vec![b'x'; MAX_MANIFEST_BYTES + 1])?;
        let result = read_bounded_utf8_file(&fixture, MAX_MANIFEST_BYTES);
        let _ = std::fs::remove_file(fixture);
        assert!(result.is_none());
        Ok(())
    }
}
