use std::error::Error;
#[cfg(any(target_os = "linux", test))]
use std::ffi::OsStr;
use std::fmt;
#[cfg(windows)]
use std::fs::File;
#[cfg(windows)]
use std::io::Read;
#[cfg(windows)]
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, RgbaImage};
use serde::{Deserialize, Serialize};

#[cfg(any(windows, target_os = "linux"))]
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
#[cfg(any(windows, target_os = "linux"))]
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
#[cfg(any(windows, target_os = "linux"))]
use xcap::Window;

pub const MAX_COMPUTER_WINDOWS: usize = 100;
pub const MAX_COMPUTER_APPLICATIONS: usize = 40;
pub const MAX_COMPUTER_TEXT_BYTES: usize = 16 * 1024;
pub const MAX_COMPUTER_CAPTURE_BYTES: usize = 3 * 1024 * 1024;

/// Returns whether the current platform can expose the Computer Use tool namespace.
///
/// Linux observation is intentionally limited to X11/XWayland, where an X11
/// display is available. Pure Wayland requires the separate portal path.
#[must_use]
pub fn computer_use_platform_available() -> bool {
    #[cfg(windows)]
    {
        true
    }

    #[cfg(target_os = "linux")]
    {
        x11_display_available(std::env::var_os("DISPLAY").as_deref())
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        false
    }
}

/// Pure X11/XWayland availability predicate used by the Linux platform gate.
#[must_use]
#[cfg(any(target_os = "linux", test))]
fn x11_display_available(display: Option<&OsStr>) -> bool {
    display.is_some_and(|display| !display.is_empty())
}

const MAX_WINDOW_TEXT_BYTES: usize = 512;
#[cfg(windows)]
const MAX_APPX_MANIFEST_BYTES: usize = 1024 * 1024;
#[cfg(windows)]
const MAX_APPX_MANIFEST_NODES: u32 = 20_000;
#[cfg(windows)]
pub(crate) const PROGRAM_FILES_X64_FOLDER_ID: &str = "{6D809377-6AF0-444B-8957-A3773F02200E}";
#[cfg(windows)]
pub(crate) const PROGRAM_FILES_X86_FOLDER_ID: &str = "{7C5A40EF-A0FB-4BFC-874A-C0F2E0B9FA8E}";
#[cfg(windows)]
pub(crate) const SYSTEM_X64_FOLDER_ID: &str = "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}";
#[cfg(windows)]
pub(crate) const SYSTEM_X86_FOLDER_ID: &str = "{D65231B0-B2F1-4857-A4CE-A8E7C6EA7D27}";
const MAX_SOURCE_PIXELS: u64 = 16_777_216;
const MAX_CAPTURE_WIDTH: u32 = 1_600;
const MAX_CAPTURE_HEIGHT: u32 = 1_200;
const JPEG_QUALITY: u8 = 78;
const MAX_SCROLL_DELTA: i32 = 10_000;
static CAPTURE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

const FORBIDDEN_COMPUTER_TARGET_KEYS: &[&str] = &[
    // Codex and terminal surfaces can bypass the command-execution policy.
    "codex",
    "codexalpha",
    "codexbeta",
    "codexnightly",
    "codexrs",
    "codexcommandprompt",
    "chatgpt",
    "chatgptalpha",
    "chatgptbeta",
    "chatgptnightly",
    "alacritty",
    "cmd",
    "cmder",
    "conemu",
    "conemu64",
    "conemu64c",
    "conemuc",
    "conhost",
    "fluentterminal",
    "gitbash",
    "hyper",
    "kitty",
    "mintty",
    "mobaxterm",
    "openconsole",
    "powershell",
    "powershellise",
    "putty",
    "puttytel",
    "pwsh",
    "tabby",
    "terminal",
    "terminus",
    "termius",
    "teraterm",
    "ttermpro",
    "uninstall",
    "warp",
    "wezterm",
    "weztermgui",
    "windowspowershell",
    "windowsterminal",
    "wsl",
    "wt",
    // Authentication, password, and identity stores.
    "1password",
    "1passwordbrowsersupport",
    "bitwarden",
    "dashlane",
    "enpass",
    "enpasspasswordmanager",
    "icloudpasswords",
    "icloudpasswords",
    "identities",
    "keepass",
    "keepasspasswordsafe",
    "keepassxc",
    "keeperpasswordmanager",
    "keeperpasswordmanager",
    "lastpass",
    "nordpass",
    "protonpass",
    "roboform",
    // Security products and Windows security/authentication surfaces.
    "a2start",
    "adaware",
    "adawareantivirus",
    "applicationframehost",
    "avg",
    "avgantivirus",
    "avgantivirusfree",
    "avginternetsecurity",
    "avgui",
    "avgnt",
    "avastantivirus",
    "avastfreeantivirus",
    "avastone",
    "avastpremiumsecurity",
    "avastui",
    "avktray",
    "avira",
    "aviraantivirus",
    "avirafreeantivirus",
    "aviraoesystray",
    "avirasystray",
    "avp",
    "avpui",
    "backgroundtaskhost",
    "bdagent",
    "bitdefender",
    "bitdefenderagent",
    "bitdefenderantivirusfree",
    "bitdefendersecuritycenter",
    "bitdefendertotalsecurity",
    "ciscistray",
    "clamwin",
    "clamwinantivirus",
    "comodoantivirus",
    "comodofirewall",
    "comodointernetsecurity",
    "crowdstrikefalconsensor",
    "crowdstrikewindowssensor",
    "csfalconcontainer",
    "csfalconservice",
    "ctfmon",
    "drweb",
    "drwebsecurityspace",
    "dwservice",
    "egui",
    "emsisoftantimalware",
    "emsisoftsecuritycenter",
    "eset",
    "esetgui",
    "esetinternetsecurity",
    "esetmaingui",
    "esetnod32antivirus",
    "esetsecurity",
    "fsecure",
    "fsui32",
    "gdataantivirus",
    "gdatasecuritycenter",
    "gdatasecuritysoftware",
    "gdatatotalprotection",
    "gdsc",
    "inputapp",
    "kaspersky",
    "kasperskyantivirus",
    "kasperskyfree",
    "kasperskyinternetsecurity",
    "kasperskysecuritycloud",
    "kasperskytotalsecurity",
    "lockapp",
    "malwarebytes",
    "malwarebytesantimalware",
    "mbam",
    "mbamgui",
    "mcafee",
    "mcafeelivesafe",
    "mcafeesecurity",
    "mcafeesecurityscanplus",
    "mcafeetotalprotection",
    "mcafeeuicontainer",
    "mcuicnt",
    "microsoftdefender",
    "microsoftdefenderantivirus",
    "norton360",
    "nortonav",
    "nortonavirus",
    "nortonsecurity",
    "nortonsecurity",
    "pandadome",
    "pandasecurity",
    "pccntmon",
    "psuaconsole",
    "runtimebroker",
    "seccenter",
    "sechealthui",
    "securityhealthsystray",
    "shellexperiencehost",
    "sihost",
    "sophoshome",
    "sophosui",
    "tabtip",
    "textinputhost",
    "totalav",
    "totalavultimateantivirus",
    "totalavultimateantivirususerinterface",
    "trendmicro",
    "trendmicrointernetsecurity",
    "trendmicromaximumsecurity",
    "uiwinmgr",
    "uistub",
    "webrootsecureanywhere",
    "windowssecurity",
    "wrsa",
    "zatray",
    "zlclient",
    "zonealarm",
    "zonealarmsecurity",
];

pub fn computer_use_target_is_forbidden(application_id: &str, application_name: &str) -> bool {
    if application_id.len() > MAX_WINDOW_TEXT_BYTES
        || application_name.len() > MAX_WINDOW_TEXT_BYTES
    {
        return true;
    }

    let mut keys = Vec::with_capacity(20);
    push_computer_policy_key(&mut keys, application_id);
    push_computer_policy_key(&mut keys, application_name);
    if let Some(file_name) = application_id.rsplit(['\\', '/']).next() {
        let stem = file_name
            .get(..file_name.len().saturating_sub(4))
            .filter(|_| {
                file_name
                    .get(file_name.len().saturating_sub(4)..)
                    .is_some_and(|suffix| suffix.eq_ignore_ascii_case(".exe"))
            })
            .unwrap_or(file_name);
        push_computer_policy_key(&mut keys, stem);
    }
    for component in application_id
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|component| !component.is_empty())
        .take(16)
    {
        push_computer_policy_key(&mut keys, component);
    }

    keys.iter()
        .any(|key| FORBIDDEN_COMPUTER_TARGET_KEYS.contains(&key.as_str()))
}

fn push_computer_policy_key(keys: &mut Vec<String>, value: &str) {
    if keys.len() >= 20 {
        return;
    }
    let key = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(128)
        .map(|character| character.to_ascii_lowercase())
        .collect::<String>();
    if !key.is_empty() && !keys.contains(&key) {
        keys.push(key);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputerWindow {
    pub id: String,
    pub process_id: u32,
    pub application: String,
    pub application_id: String,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub minimized: bool,
    pub focused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputerApplication {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_count: Option<u32>,
    #[serde(default)]
    pub is_running: bool,
    #[serde(default)]
    pub windows: Vec<ComputerWindow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputerCapture {
    pub window: ComputerWindow,
    pub screenshot_id: String,
    pub width: u32,
    pub height: u32,
    pub jpeg_bytes: usize,
    pub image_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComputerButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputerKey {
    Alt,
    Backspace,
    Control,
    Delete,
    Down,
    End,
    Enter,
    Escape,
    Home,
    Left,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadAdd,
    NumpadDecimal,
    NumpadDivide,
    NumpadEnter,
    NumpadMultiply,
    NumpadSubtract,
    PageDown,
    PageUp,
    Right,
    Shift,
    Space,
    Tab,
    Up,
    Character(char),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputerUseError {
    Unsupported,
    Enumerate,
    WindowNotFound,
    WindowUnavailable,
    WindowNotFocused,
    CoordinateOutsideWindow,
    InvalidInput,
    Capture,
    CaptureTooLarge,
    Encode,
    Input,
}

impl fmt::Display for ComputerUseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unsupported => "Computer Use is not supported on this platform",
            Self::Enumerate => "could not enumerate desktop windows",
            Self::WindowNotFound => "the selected window is no longer available",
            Self::WindowUnavailable => "the selected window is minimized or has no visible area",
            Self::WindowNotFocused => "the selected window must be focused before keyboard input",
            Self::CoordinateOutsideWindow => {
                "the requested coordinate is outside the selected window"
            }
            Self::InvalidInput => "the Computer Use input is invalid or exceeds its limit",
            Self::Capture => "could not capture the selected window",
            Self::CaptureTooLarge => "the selected window exceeds the bounded capture limit",
            Self::Encode => "could not encode the bounded window capture",
            Self::Input => "the operating system rejected the input event",
        })
    }
}

impl Error for ComputerUseError {}

#[cfg(any(windows, target_os = "linux"))]
pub fn list_computer_windows() -> Result<Vec<ComputerWindow>, ComputerUseError> {
    let source_windows = Window::all()
        .map_err(|_| ComputerUseError::Enumerate)?
        .into_iter()
        .take(MAX_COMPUTER_WINDOWS + 1)
        .collect::<Vec<_>>();
    let process_ids = source_windows
        .iter()
        .filter_map(|window| window.pid().ok())
        .filter(|process_id| *process_id != std::process::id())
        .map(Pid::from_u32)
        .collect::<Vec<_>>();
    let processes = load_processes(&process_ids);
    let mut windows = source_windows
        .iter()
        .filter_map(|window| map_window_with_processes(window, &processes).ok())
        .filter(|window| window.process_id != std::process::id())
        .filter(|window| !window.application_id.is_empty())
        .filter(|window| !window.title.is_empty() && window.width > 0 && window.height > 0)
        .collect::<Vec<_>>();
    windows.sort_by(|left, right| {
        right
            .focused
            .cmp(&left.focused)
            .then_with(|| left.application.cmp(&right.application))
            .then_with(|| left.title.cmp(&right.title))
    });
    windows.truncate(MAX_COMPUTER_WINDOWS);
    Ok(windows)
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn list_computer_windows() -> Result<Vec<ComputerWindow>, ComputerUseError> {
    Err(ComputerUseError::Unsupported)
}

#[cfg(any(windows, target_os = "linux"))]
pub fn inspect_computer_window(id: &str) -> Result<ComputerWindow, ComputerUseError> {
    let window = find_window(id)?;
    map_window(&window)
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn inspect_computer_window(_id: &str) -> Result<ComputerWindow, ComputerUseError> {
    Err(ComputerUseError::Unsupported)
}

#[cfg(any(windows, target_os = "linux"))]
pub fn capture_computer_window(id: &str) -> Result<ComputerCapture, ComputerUseError> {
    let window = find_window(id)?;
    let metadata = map_window(&window)?;
    ensure_available(&metadata)?;
    let source_pixels = u64::from(metadata.width) * u64::from(metadata.height);
    if source_pixels > MAX_SOURCE_PIXELS {
        return Err(ComputerUseError::CaptureTooLarge);
    }

    let image = window
        .capture_image()
        .map_err(|_| ComputerUseError::Capture)?;
    let (width, height) = bounded_dimensions(image.width(), image.height());
    let image = if image.width() == width && image.height() == height {
        image
    } else {
        image::imageops::resize(&image, width, height, FilterType::Triangle)
    };
    encode_capture(metadata, image)
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn capture_computer_window(_id: &str) -> Result<ComputerCapture, ComputerUseError> {
    Err(ComputerUseError::Unsupported)
}

#[cfg(any(windows, target_os = "linux"))]
pub fn move_over_computer_window(id: &str, x: i32, y: i32) -> Result<(), ComputerUseError> {
    let window = inspect_computer_window(id)?;
    let (screen_x, screen_y) = relative_to_screen(&window, x, y)?;
    let mut enigo = input_connection()?;
    enigo
        .move_mouse(screen_x, screen_y, Coordinate::Abs)
        .map_err(|_| ComputerUseError::Input)
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn move_over_computer_window(_id: &str, _x: i32, _y: i32) -> Result<(), ComputerUseError> {
    Err(ComputerUseError::Unsupported)
}

#[cfg(any(windows, target_os = "linux"))]
pub fn click_computer_window(
    id: &str,
    x: i32,
    y: i32,
    button: ComputerButton,
    clicks: u8,
) -> Result<(), ComputerUseError> {
    if !(1..=3).contains(&clicks) {
        return Err(ComputerUseError::InvalidInput);
    }
    let window = inspect_computer_window(id)?;
    let (screen_x, screen_y) = relative_to_screen(&window, x, y)?;
    let mut enigo = input_connection()?;
    enigo
        .move_mouse(screen_x, screen_y, Coordinate::Abs)
        .map_err(|_| ComputerUseError::Input)?;
    let button = match button {
        ComputerButton::Left => Button::Left,
        ComputerButton::Right => Button::Right,
        ComputerButton::Middle => Button::Middle,
    };
    for _ in 0..clicks {
        enigo
            .button(button, Direction::Click)
            .map_err(|_| ComputerUseError::Input)?;
    }
    Ok(())
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn click_computer_window(
    _id: &str,
    _x: i32,
    _y: i32,
    _button: ComputerButton,
    _clicks: u8,
) -> Result<(), ComputerUseError> {
    Err(ComputerUseError::Unsupported)
}

#[cfg(any(windows, target_os = "linux"))]
pub fn drag_computer_window(
    id: &str,
    from_x: i32,
    from_y: i32,
    to_x: i32,
    to_y: i32,
) -> Result<(), ComputerUseError> {
    let window = inspect_computer_window(id)?;
    let (from_screen_x, from_screen_y) = relative_to_screen(&window, from_x, from_y)?;
    let (to_screen_x, to_screen_y) = relative_to_screen(&window, to_x, to_y)?;
    let mut enigo = input_connection()?;
    enigo
        .move_mouse(from_screen_x, from_screen_y, Coordinate::Abs)
        .map_err(|_| ComputerUseError::Input)?;
    enigo
        .button(Button::Left, Direction::Press)
        .map_err(|_| ComputerUseError::Input)?;
    let move_result = enigo.move_mouse(to_screen_x, to_screen_y, Coordinate::Abs);
    let release_result = enigo.button(Button::Left, Direction::Release);
    move_result
        .and(release_result)
        .map_err(|_| ComputerUseError::Input)
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn drag_computer_window(
    _id: &str,
    _from_x: i32,
    _from_y: i32,
    _to_x: i32,
    _to_y: i32,
) -> Result<(), ComputerUseError> {
    Err(ComputerUseError::Unsupported)
}

#[cfg(any(windows, target_os = "linux"))]
pub fn scroll_computer_window(
    id: &str,
    x: i32,
    y: i32,
    delta_x: i32,
    delta_y: i32,
) -> Result<(), ComputerUseError> {
    if delta_x.abs() > MAX_SCROLL_DELTA
        || delta_y.abs() > MAX_SCROLL_DELTA
        || (delta_x == 0 && delta_y == 0)
    {
        return Err(ComputerUseError::InvalidInput);
    }
    let window = inspect_computer_window(id)?;
    let (screen_x, screen_y) = relative_to_screen(&window, x, y)?;
    let mut enigo = input_connection()?;
    enigo
        .move_mouse(screen_x, screen_y, Coordinate::Abs)
        .map_err(|_| ComputerUseError::Input)?;
    if delta_x != 0 {
        enigo
            .scroll(delta_x, Axis::Horizontal)
            .map_err(|_| ComputerUseError::Input)?;
    }
    if delta_y != 0 {
        enigo
            .scroll(delta_y, Axis::Vertical)
            .map_err(|_| ComputerUseError::Input)?;
    }
    Ok(())
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn scroll_computer_window(
    _id: &str,
    _x: i32,
    _y: i32,
    _delta_x: i32,
    _delta_y: i32,
) -> Result<(), ComputerUseError> {
    Err(ComputerUseError::Unsupported)
}

#[cfg(any(windows, target_os = "linux"))]
pub fn type_into_computer_window(id: &str, text: &str) -> Result<(), ComputerUseError> {
    if text.len() > MAX_COMPUTER_TEXT_BYTES {
        return Err(ComputerUseError::InvalidInput);
    }
    require_focused(id)?;
    if text.is_empty() {
        return Ok(());
    }
    input_connection()?
        .text(text)
        .map_err(|_| ComputerUseError::Input)
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn type_into_computer_window(_id: &str, _text: &str) -> Result<(), ComputerUseError> {
    Err(ComputerUseError::Unsupported)
}

#[cfg(any(windows, target_os = "linux"))]
pub fn press_computer_key(
    id: &str,
    key: ComputerKey,
    modifiers: &[ComputerKey],
) -> Result<(), ComputerUseError> {
    if modifiers.len() > 4
        || modifiers.iter().any(|modifier| {
            !matches!(
                modifier,
                ComputerKey::Alt | ComputerKey::Control | ComputerKey::Shift
            )
        })
    {
        return Err(ComputerUseError::InvalidInput);
    }
    require_focused(id)?;
    let mut enigo = input_connection()?;
    for modifier in modifiers {
        enigo
            .key(map_key(*modifier), Direction::Press)
            .map_err(|_| ComputerUseError::Input)?;
    }
    #[cfg(windows)]
    let result = if key == ComputerKey::NumpadEnter {
        enigo
            .raw(0x1c | enigo::EXT, Direction::Click)
            .map_err(|_| ComputerUseError::Input)
    } else {
        enigo
            .key(map_key(key), Direction::Click)
            .map_err(|_| ComputerUseError::Input)
    };
    #[cfg(target_os = "linux")]
    let result = enigo
        .key(map_key(key), Direction::Click)
        .map_err(|_| ComputerUseError::Input);
    for modifier in modifiers.iter().rev() {
        let _ = enigo.key(map_key(*modifier), Direction::Release);
    }
    result
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn press_computer_key(
    _id: &str,
    _key: ComputerKey,
    _modifiers: &[ComputerKey],
) -> Result<(), ComputerUseError> {
    Err(ComputerUseError::Unsupported)
}

fn bounded_dimensions(width: u32, height: u32) -> (u32, u32) {
    if width == 0 || height == 0 {
        return (width, height);
    }
    let width_scale = f64::from(MAX_CAPTURE_WIDTH) / f64::from(width);
    let height_scale = f64::from(MAX_CAPTURE_HEIGHT) / f64::from(height);
    let scale = width_scale.min(height_scale).min(1.0);
    (
        (f64::from(width) * scale).round().max(1.0) as u32,
        (f64::from(height) * scale).round().max(1.0) as u32,
    )
}

fn encode_capture(
    window: ComputerWindow,
    image: RgbaImage,
) -> Result<ComputerCapture, ComputerUseError> {
    let width = image.width();
    let height = image.height();
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY)
        .encode_image(&DynamicImage::ImageRgba8(image))
        .map_err(|_| ComputerUseError::Encode)?;
    if jpeg.len() > MAX_COMPUTER_CAPTURE_BYTES {
        return Err(ComputerUseError::CaptureTooLarge);
    }
    let jpeg_bytes = jpeg.len();
    let image_url = format!("data:image/jpeg;base64,{}", STANDARD.encode(jpeg));
    let screenshot_id = format!(
        "screenshot-{}",
        CAPTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    Ok(ComputerCapture {
        window,
        screenshot_id,
        width,
        height,
        jpeg_bytes,
        image_url,
    })
}

fn relative_to_screen(
    window: &ComputerWindow,
    x: i32,
    y: i32,
) -> Result<(i32, i32), ComputerUseError> {
    ensure_available(window)?;
    let width = i32::try_from(window.width).map_err(|_| ComputerUseError::InvalidInput)?;
    let height = i32::try_from(window.height).map_err(|_| ComputerUseError::InvalidInput)?;
    if x < 0 || y < 0 || x >= width || y >= height {
        return Err(ComputerUseError::CoordinateOutsideWindow);
    }
    let screen_x = window
        .x
        .checked_add(x)
        .ok_or(ComputerUseError::InvalidInput)?;
    let screen_y = window
        .y
        .checked_add(y)
        .ok_or(ComputerUseError::InvalidInput)?;
    Ok((screen_x, screen_y))
}

fn ensure_available(window: &ComputerWindow) -> Result<(), ComputerUseError> {
    if window.minimized || window.width == 0 || window.height == 0 {
        Err(ComputerUseError::WindowUnavailable)
    } else {
        Ok(())
    }
}

#[cfg(any(windows, target_os = "linux"))]
fn require_focused(id: &str) -> Result<(), ComputerUseError> {
    let window = inspect_computer_window(id)?;
    ensure_available(&window)?;
    if window.focused {
        Ok(())
    } else {
        Err(ComputerUseError::WindowNotFocused)
    }
}

#[cfg(any(windows, target_os = "linux"))]
fn input_connection() -> Result<Enigo, ComputerUseError> {
    Enigo::new(&Settings::default()).map_err(|_| ComputerUseError::Input)
}

#[cfg(any(windows, target_os = "linux"))]
fn find_window(id: &str) -> Result<Window, ComputerUseError> {
    let id = id
        .parse::<u32>()
        .map_err(|_| ComputerUseError::InvalidInput)?;
    Window::all()
        .map_err(|_| ComputerUseError::Enumerate)?
        .into_iter()
        .find(|window| {
            window.id().ok() == Some(id)
                && window
                    .pid()
                    .ok()
                    .is_some_and(|pid| pid != std::process::id())
        })
        .ok_or(ComputerUseError::WindowNotFound)
}

#[cfg(any(windows, target_os = "linux"))]
fn map_window(window: &Window) -> Result<ComputerWindow, ComputerUseError> {
    let process_id = window.pid().unwrap_or_default();
    let process_ids = [Pid::from_u32(process_id)];
    let processes = load_processes(&process_ids);
    map_window_with_processes(window, &processes)
}

#[cfg(any(windows, target_os = "linux"))]
fn map_window_with_processes(
    window: &Window,
    processes: &System,
) -> Result<ComputerWindow, ComputerUseError> {
    let process_id = window.pid().unwrap_or_default();
    let application = bounded_text(window.app_name().unwrap_or_default(), MAX_WINDOW_TEXT_BYTES);
    Ok(ComputerWindow {
        id: window
            .id()
            .map_err(|_| ComputerUseError::WindowNotFound)?
            .to_string(),
        process_id,
        application_id: process_application_id(processes, process_id, &application),
        application,
        title: bounded_text(window.title().unwrap_or_default(), MAX_WINDOW_TEXT_BYTES),
        x: window
            .x()
            .map_err(|_| ComputerUseError::WindowUnavailable)?,
        y: window
            .y()
            .map_err(|_| ComputerUseError::WindowUnavailable)?,
        width: window
            .width()
            .map_err(|_| ComputerUseError::WindowUnavailable)?,
        height: window
            .height()
            .map_err(|_| ComputerUseError::WindowUnavailable)?,
        minimized: window.is_minimized().unwrap_or(false),
        focused: window.is_focused().unwrap_or(false),
    })
}

#[cfg(any(windows, target_os = "linux"))]
fn load_processes(process_ids: &[Pid]) -> System {
    let mut processes = System::new();
    if !process_ids.is_empty() {
        processes.refresh_processes_specifics(
            ProcessesToUpdate::Some(process_ids),
            ProcessRefreshKind::new().with_exe(UpdateKind::Always),
        );
    }
    processes
}

#[cfg(windows)]
fn process_application_id(processes: &System, process_id: u32, _application: &str) -> String {
    let Some(executable_path) = processes
        .process(Pid::from_u32(process_id))
        .and_then(|process| process.exe())
    else {
        return String::new();
    };
    windows_process_application_id(executable_path)
}

#[cfg(windows)]
pub(crate) fn windows_process_application_id(executable_path: &Path) -> String {
    const SHARED_HOST_EXECUTABLES: &[&str] = &[
        "applicationframehost.exe",
        "dllhost.exe",
        "mmc.exe",
        "rundll32.exe",
    ];

    let executable_name = executable_path
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if executable_name.is_empty() || SHARED_HOST_EXECUTABLES.contains(&executable_name.as_str()) {
        return String::new();
    }
    if let Some(application_id) = windows_packaged_application_id(executable_path) {
        return application_id;
    }
    if let Some(application_id) = windows_known_folder_application_id(executable_path) {
        return application_id;
    }
    let application_id = executable_path.to_string_lossy();
    let application_id = application_id
        .strip_prefix(r"\\?\")
        .unwrap_or(&application_id);
    if !executable_path.is_absolute()
        || application_id.is_empty()
        || application_id.len() > MAX_WINDOW_TEXT_BYTES
        || application_id.chars().any(char::is_control)
    {
        return String::new();
    }
    application_id.to_owned()
}

#[cfg(windows)]
fn windows_known_folder_application_id(executable_path: &Path) -> Option<String> {
    let mut roots = Vec::with_capacity(4);
    if let Some(path) = std::env::var_os("ProgramW6432")
        .or_else(|| std::env::var_os("ProgramFiles"))
        .map(PathBuf::from)
    {
        roots.push((PROGRAM_FILES_X64_FOLDER_ID, path));
    }
    if let Some(path) = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from) {
        roots.push((PROGRAM_FILES_X86_FOLDER_ID, path));
    }
    if let Some(path) = std::env::var_os("SystemRoot").map(PathBuf::from) {
        roots.push((SYSTEM_X64_FOLDER_ID, path.join("System32")));
        roots.push((SYSTEM_X86_FOLDER_ID, path.join("SysWOW64")));
    }
    known_folder_application_id(executable_path, &roots)
}

#[cfg(windows)]
fn known_folder_application_id(
    executable_path: &Path,
    roots: &[(&str, PathBuf)],
) -> Option<String> {
    roots.iter().find_map(|(folder_id, root)| {
        let relative = strip_windows_path_prefix(executable_path, root)?;
        if !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        {
            return None;
        }
        let relative = relative.to_string_lossy().replace('/', r"\");
        let application_id = format!("{folder_id}\\{relative}");
        (application_id.len() <= MAX_WINDOW_TEXT_BYTES
            && !application_id.chars().any(char::is_control))
        .then_some(application_id)
    })
}

#[cfg(windows)]
fn windows_packaged_application_id(executable_path: &Path) -> Option<String> {
    let program_files = std::env::var_os("ProgramW6432")
        .or_else(|| std::env::var_os("ProgramFiles"))
        .map(PathBuf::from)?;
    let windows_apps = program_files.join("WindowsApps");
    let relative_path = strip_windows_path_prefix(executable_path, &windows_apps)?;
    let mut relative_components = relative_path.components();
    let package_full_name = match relative_components.next()? {
        Component::Normal(component) => component.to_string_lossy().into_owned(),
        _ => return None,
    };
    let executable_relative_path = relative_components.collect::<PathBuf>();
    if executable_relative_path.as_os_str().is_empty() {
        return None;
    }

    let manifest_path = windows_apps
        .join(&package_full_name)
        .join("AppxManifest.xml");
    let mut manifest_bytes = Vec::with_capacity(16 * 1024);
    File::open(manifest_path)
        .ok()?
        .take((MAX_APPX_MANIFEST_BYTES + 1) as u64)
        .read_to_end(&mut manifest_bytes)
        .ok()?;
    if manifest_bytes.len() > MAX_APPX_MANIFEST_BYTES {
        return None;
    }
    let manifest = std::str::from_utf8(&manifest_bytes).ok()?;
    packaged_application_id_from_manifest(
        manifest,
        &package_full_name,
        &executable_relative_path.to_string_lossy(),
    )
}

#[cfg(windows)]
fn strip_windows_path_prefix(path: &Path, base: &Path) -> Option<PathBuf> {
    fn component_eq(left: Component<'_>, right: Component<'_>) -> bool {
        fn display(component: Component<'_>) -> String {
            component
                .as_os_str()
                .to_string_lossy()
                .trim_start_matches(r"\\?\")
                .to_lowercase()
        }
        display(left) == display(right)
    }

    let mut path_components = path.components();
    for base_component in base.components() {
        if !component_eq(path_components.next()?, base_component) {
            return None;
        }
    }
    let relative = path_components.collect::<PathBuf>();
    (!relative.as_os_str().is_empty()).then_some(relative)
}

#[cfg(windows)]
fn packaged_application_id_from_manifest(
    manifest: &str,
    package_full_name: &str,
    executable_relative_path: &str,
) -> Option<String> {
    let document = roxmltree::Document::parse_with_options(
        manifest,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_APPX_MANIFEST_NODES,
        },
    )
    .ok()?;
    let identity_name = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "Identity")?
        .attribute("Name")?;
    let package_name_prefix = format!("{identity_name}_");
    if !package_full_name
        .get(..package_name_prefix.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(&package_name_prefix))
    {
        return None;
    }
    let publisher_id = package_full_name.rsplit('_').next()?;
    if !valid_publisher_id(publisher_id) {
        return None;
    }

    let target_executable = normalized_windows_relative_path(executable_relative_path)?;
    let mut matching_application_ids = document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "Application")
        .filter_map(|application| {
            let executable =
                normalized_windows_relative_path(application.attribute("Executable")?)?;
            executable
                .eq_ignore_ascii_case(&target_executable)
                .then(|| application.attribute("Id"))
                .flatten()
        })
        .take(2);
    let application_id = matching_application_ids.next()?;
    if matching_application_ids.next().is_some()
        || application_id.is_empty()
        || application_id.contains('!')
    {
        return None;
    }
    let application_user_model_id = format!("{identity_name}_{publisher_id}!{application_id}");
    (!application_user_model_id.is_empty()
        && application_user_model_id.len() <= 128
        && !application_user_model_id.chars().any(|character| {
            character.is_control()
                || character.is_whitespace()
                || matches!(character, '"' | '\'' | '/' | '\\')
        }))
    .then_some(application_user_model_id)
}

#[cfg(windows)]
fn normalized_windows_relative_path(value: &str) -> Option<String> {
    let mut components = Vec::new();
    for component in Path::new(value.trim()).components() {
        match component {
            Component::Normal(component) => {
                components.push(component.to_string_lossy().to_lowercase())
            }
            Component::CurDir => {}
            Component::ParentDir | Component::Prefix(_) | Component::RootDir => return None,
        }
    }
    (!components.is_empty()).then(|| components.join(r"\"))
}

#[cfg(windows)]
fn valid_publisher_id(value: &str) -> bool {
    value.len() == 13
        && value.bytes().all(|byte| {
            let byte = byte.to_ascii_lowercase();
            byte.is_ascii_digit()
                || byte.is_ascii_lowercase() && !matches!(byte, b'i' | b'l' | b'o' | b'u')
        })
}

#[cfg(target_os = "linux")]
fn process_application_id(processes: &System, process_id: u32, application: &str) -> String {
    let executable_name = processes
        .process(Pid::from_u32(process_id))
        .and_then(|process| process.exe())
        .and_then(|path| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    normalize_application_id(if executable_name.is_empty() {
        application
    } else {
        &executable_name
    })
}

#[cfg(any(target_os = "linux", test))]
fn normalize_application_id(value: &str) -> String {
    let value = value.trim().to_lowercase();
    if value.len() > MAX_WINDOW_TEXT_BYTES {
        String::new()
    } else {
        value
    }
}

fn bounded_text(mut value: String, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
    value
}

#[cfg(any(windows, target_os = "linux"))]
fn map_key(key: ComputerKey) -> Key {
    match key {
        ComputerKey::Alt => Key::Alt,
        ComputerKey::Backspace => Key::Backspace,
        ComputerKey::Control => Key::Control,
        ComputerKey::Delete => Key::Delete,
        ComputerKey::Down => Key::DownArrow,
        ComputerKey::End => Key::End,
        ComputerKey::Enter => Key::Return,
        ComputerKey::Escape => Key::Escape,
        ComputerKey::Home => Key::Home,
        ComputerKey::Left => Key::LeftArrow,
        ComputerKey::Numpad0 => Key::Numpad0,
        ComputerKey::Numpad1 => Key::Numpad1,
        ComputerKey::Numpad2 => Key::Numpad2,
        ComputerKey::Numpad3 => Key::Numpad3,
        ComputerKey::Numpad4 => Key::Numpad4,
        ComputerKey::Numpad5 => Key::Numpad5,
        ComputerKey::Numpad6 => Key::Numpad6,
        ComputerKey::Numpad7 => Key::Numpad7,
        ComputerKey::Numpad8 => Key::Numpad8,
        ComputerKey::Numpad9 => Key::Numpad9,
        ComputerKey::NumpadAdd => Key::Add,
        ComputerKey::NumpadDecimal => Key::Decimal,
        ComputerKey::NumpadDivide => Key::Divide,
        ComputerKey::NumpadEnter => Key::Return,
        ComputerKey::NumpadMultiply => Key::Multiply,
        ComputerKey::NumpadSubtract => Key::Subtract,
        ComputerKey::PageDown => Key::PageDown,
        ComputerKey::PageUp => Key::PageUp,
        ComputerKey::Right => Key::RightArrow,
        ComputerKey::Shift => Key::Shift,
        ComputerKey::Space => Key::Space,
        ComputerKey::Tab => Key::Tab,
        ComputerKey::Up => Key::UpArrow,
        ComputerKey::Character(character) => Key::Unicode(character),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ComputerUseError, ComputerWindow, MAX_CAPTURE_HEIGHT, MAX_CAPTURE_WIDTH,
        bounded_dimensions, bounded_text, computer_use_target_is_forbidden,
        normalize_application_id, relative_to_screen, x11_display_available,
    };
    #[cfg(windows)]
    use super::{
        PROGRAM_FILES_X64_FOLDER_ID, known_folder_application_id,
        packaged_application_id_from_manifest, windows_process_application_id,
    };
    #[cfg(windows)]
    use std::path::{Path, PathBuf};

    fn window() -> ComputerWindow {
        ComputerWindow {
            id: "7".to_owned(),
            process_id: 1,
            application: "test".to_owned(),
            application_id: "test.exe".to_owned(),
            title: "fixture".to_owned(),
            x: -100,
            y: 50,
            width: 800,
            height: 600,
            minimized: false,
            focused: true,
        }
    }

    #[test]
    fn capture_dimensions_preserve_aspect_ratio_within_bounds() {
        assert_eq!(bounded_dimensions(3_840, 2_160), (1_600, 900));
        assert_eq!(bounded_dimensions(800, 1_600), (600, 1_200));
        assert_eq!(
            bounded_dimensions(MAX_CAPTURE_WIDTH, MAX_CAPTURE_HEIGHT),
            (MAX_CAPTURE_WIDTH, MAX_CAPTURE_HEIGHT)
        );
    }

    #[test]
    fn coordinates_are_relative_to_the_selected_window() {
        assert_eq!(relative_to_screen(&window(), 25, 30), Ok((-75, 80)));
        assert_eq!(
            relative_to_screen(&window(), 800, 30),
            Err(ComputerUseError::CoordinateOutsideWindow)
        );
    }

    #[test]
    fn bounded_text_does_not_split_utf8() {
        assert_eq!(bounded_text("abЯ".to_owned(), 3), "ab");
    }

    #[test]
    fn application_ids_are_stable_and_case_insensitive() {
        assert_eq!(normalize_application_id("  MSPaint.EXE "), "mspaint.exe");
        assert!(normalize_application_id(&"x".repeat(513)).is_empty());
    }

    #[test]
    fn x11_display_gate_requires_a_nonempty_display_name() {
        assert!(!x11_display_available(None));
        assert!(!x11_display_available(Some(std::ffi::OsStr::new(""))));
        assert!(x11_display_available(Some(std::ffi::OsStr::new(":0"))));
    }

    #[test]
    fn product_policy_blocks_sensitive_targets_without_substring_false_positives() {
        assert!(computer_use_target_is_forbidden(
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
            "Windows PowerShell"
        ));
        assert!(computer_use_target_is_forbidden(
            "OpenAI.CodexBeta_2p2nqsd0c76g0!App",
            "ChatGPT (Beta)"
        ));
        assert!(computer_use_target_is_forbidden(
            "Bitwarden.Desktop",
            "Bitwarden"
        ));
        assert!(!computer_use_target_is_forbidden(
            r"C:\Windows\System32\mspaint.exe",
            "Paint"
        ));
        assert!(!computer_use_target_is_forbidden(
            "com.example.terminalvelocity",
            "Terminal Velocity"
        ));
        assert!(computer_use_target_is_forbidden(&"x".repeat(513), ""));
    }

    #[cfg(windows)]
    #[test]
    fn windows_process_ids_use_stable_absolute_paths_without_a_legacy_prefix() {
        assert_eq!(
            windows_process_application_id(Path::new(r"C:\Tools\Editor\Editor.EXE")),
            r"C:\Tools\Editor\Editor.EXE"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_known_folder_ids_match_the_stable_shape() {
        assert_eq!(
            known_folder_application_id(
                Path::new(r"C:\Program Files\Editor\Editor.EXE"),
                &[(
                    PROGRAM_FILES_X64_FOLDER_ID,
                    PathBuf::from(r"C:\Program Files")
                )]
            )
            .as_deref(),
            Some(r"{6D809377-6AF0-444B-8957-A3773F02200E}\Editor\Editor.EXE")
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_shared_hosts_do_not_become_persistent_app_ids() {
        assert!(
            windows_process_application_id(Path::new(
                r"C:\Windows\System32\ApplicationFrameHost.exe"
            ))
            .is_empty()
        );
    }

    #[cfg(windows)]
    #[test]
    fn packaged_windows_executable_resolves_to_its_manifest_aumid() {
        let manifest = r#"<?xml version="1.0" encoding="utf-8"?>
            <Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10">
              <Identity Name="OpenAI.CodexBeta" ProcessorArchitecture="x64"
                        Version="26.715.3651.0"
                        Publisher="CN=50BDFD77-8903-4850-9FFE-6E8522F64D5B" />
              <Applications>
                <Application Id="App" Executable="app/ChatGPT (Beta).exe"
                             EntryPoint="Windows.FullTrustApplication" />
              </Applications>
            </Package>"#;

        assert_eq!(
            packaged_application_id_from_manifest(
                manifest,
                "OpenAI.CodexBeta_26.715.3651.0_x64__2p2nqsd0c76g0",
                r"app\ChatGPT (Beta).exe"
            )
            .as_deref(),
            Some("OpenAI.CodexBeta_2p2nqsd0c76g0!App")
        );
        assert!(
            packaged_application_id_from_manifest(
                manifest,
                "OpenAI.CodexBeta_26.715.3651.0_x64__2p2nqsd0c76g0",
                r"app\other.exe"
            )
            .is_none()
        );
    }
}
