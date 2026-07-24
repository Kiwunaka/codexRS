use std::error::Error;
use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, RgbaImage};

#[cfg(any(windows, target_os = "linux"))]
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
#[cfg(any(windows, target_os = "linux"))]
use xcap::Window;

pub const MAX_COMPUTER_WINDOWS: usize = 100;
pub const MAX_COMPUTER_TEXT_BYTES: usize = 16 * 1024;
pub const MAX_COMPUTER_CAPTURE_BYTES: usize = 3 * 1024 * 1024;

const MAX_WINDOW_TEXT_BYTES: usize = 512;
const MAX_SOURCE_PIXELS: u64 = 16_777_216;
const MAX_CAPTURE_WIDTH: u32 = 1_600;
const MAX_CAPTURE_HEIGHT: u32 = 1_200;
const JPEG_QUALITY: u8 = 78;
const MAX_SCROLL_DELTA: i32 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputerWindow {
    pub id: String,
    pub process_id: u32,
    pub application: String,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub minimized: bool,
    pub focused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputerCapture {
    pub window: ComputerWindow,
    pub width: u32,
    pub height: u32,
    pub jpeg_bytes: usize,
    pub image_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    Meta,
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
    let mut windows = Window::all()
        .map_err(|_| ComputerUseError::Enumerate)?
        .into_iter()
        .filter_map(|window| map_window(&window).ok())
        .filter(|window| !window.title.is_empty() && window.width > 0 && window.height > 0)
        .take(MAX_COMPUTER_WINDOWS + 1)
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
    if !(1..=2).contains(&clicks) {
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
    if text.is_empty() || text.len() > MAX_COMPUTER_TEXT_BYTES {
        return Err(ComputerUseError::InvalidInput);
    }
    require_focused(id)?;
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
                ComputerKey::Alt | ComputerKey::Control | ComputerKey::Meta | ComputerKey::Shift
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
    Ok(ComputerCapture {
        window,
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
        .find(|window| window.id().ok() == Some(id))
        .ok_or(ComputerUseError::WindowNotFound)
}

#[cfg(any(windows, target_os = "linux"))]
fn map_window(window: &Window) -> Result<ComputerWindow, ComputerUseError> {
    Ok(ComputerWindow {
        id: window
            .id()
            .map_err(|_| ComputerUseError::WindowNotFound)?
            .to_string(),
        process_id: window.pid().unwrap_or_default(),
        application: bounded_text(window.app_name().unwrap_or_default(), MAX_WINDOW_TEXT_BYTES),
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
        ComputerKey::Meta => Key::Meta,
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
        bounded_dimensions, bounded_text, relative_to_screen,
    };

    fn window() -> ComputerWindow {
        ComputerWindow {
            id: "7".to_owned(),
            process_id: 1,
            application: "test".to_owned(),
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
}
