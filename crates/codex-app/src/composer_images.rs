//! Native composer image input; attachment transport remains in the existing reducer.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use codex_core::MAX_COMPOSER_ATTACHMENTS;
use gpui::{ClipboardEntry, ClipboardItem, Image, ImageFormat};

const MAX_CLIPBOARD_IMAGE_BYTES: usize = 20 * 1024 * 1024;

pub(crate) fn clipboard_image(item: &ClipboardItem) -> Option<&Image> {
    item.entries().iter().find_map(|entry| match entry {
        ClipboardEntry::Image(image) => Some(image),
        ClipboardEntry::String(_) => None,
    })
}

pub(crate) fn store_clipboard_image(directory: &Path, image: &Image) -> Result<PathBuf, String> {
    if image.bytes().is_empty() || image.bytes().len() > MAX_CLIPBOARD_IMAGE_BYTES {
        return Err("Clipboard images must contain data and be at most 20 MiB.".to_owned());
    }
    let extension = match image.format() {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Gif => "gif",
        ImageFormat::Webp => "webp",
        _ => return Err("Paste a PNG, JPG, GIF, or WEBP image.".to_owned()),
    };
    std::fs::create_dir_all(directory)
        .map_err(|error| format!("Could not create the pasted image directory: {error}"))?;
    let directory = directory
        .canonicalize()
        .map_err(|error| format!("Could not resolve the pasted image directory: {error}"))?;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = directory.join(format!(
        "pasted-image-{}-{stamp}-{}.{extension}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| format!("Could not save the pasted image: {error}"))?;
    file.write_all(image.bytes())
        .map_err(|error| format!("Could not save the pasted image: {error}"))?;
    Ok(path)
}

pub(crate) fn dropped_images(paths: &[PathBuf], available: usize) -> Vec<PathBuf> {
    paths
        .iter()
        .take(MAX_COMPOSER_ATTACHMENTS)
        .filter(|path| path.is_absolute() && is_stable_composer_photo(path) && path.is_file())
        .take(available.min(MAX_COMPOSER_ATTACHMENTS))
        .cloned()
        .collect()
}

pub(crate) fn is_stable_composer_photo(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp"
            )
        })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use codex_core::{Action, AppState, ComposerAttachmentKind, reduce};
    use gpui::ImageFormat;

    use super::*;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "codexrs-composer-images-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            assert!(std::fs::create_dir(&path).is_ok());
            Self(path)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let Ok(path) = self.0.canonicalize() else {
                return;
            };
            let Ok(temporary) = std::env::temp_dir().canonicalize() else {
                return;
            };
            if path.starts_with(temporary)
                && path.file_name().is_some_and(|name| {
                    name.to_string_lossy()
                        .starts_with("codexrs-composer-images-")
                })
            {
                let _ = std::fs::remove_dir_all(path);
            }
        }
    }

    fn png() -> Image {
        let mut bytes = Cursor::new(Vec::new());
        assert!(
            image::DynamicImage::new_rgba8(2, 2)
                .write_to(&mut bytes, image::ImageFormat::Png)
                .is_ok()
        );
        Image::from_bytes(ImageFormat::Png, bytes.into_inner())
    }

    #[test]
    fn clipboard_image_attaches_without_replacing_text_or_sending() {
        assert!(clipboard_image(&ClipboardItem::new_string("keep this text".to_owned())).is_none());
        let fixture = Fixture::new();
        let image = png();
        let item = ClipboardItem::new_image(&image);
        let extracted = clipboard_image(&item);
        assert!(
            extracted.is_some(),
            "The copied image must reach the attachment path"
        );
        let path = store_clipboard_image(&fixture.0, &image)
            .unwrap_or_else(|error| panic!("Could not store clipboard image: {error}"));
        assert!(path.is_absolute() && path.is_file());
        assert_eq!(std::fs::read(&path).ok().as_deref(), Some(image.bytes()));
        let mut state = AppState {
            composer: "Explique esta imagem".to_owned(),
            ..AppState::default()
        };
        assert!(
            reduce(
                &mut state,
                Action::AddComposerAttachments(vec![path.clone()])
            )
            .is_empty()
        );
        assert_eq!(state.composer, "Explique esta imagem");
        assert_eq!(state.composer_attachments.len(), 1);
        assert_eq!(
            state.composer_attachments[0].kind,
            ComposerAttachmentKind::LocalImage
        );
        assert!(reduce(&mut state, Action::RemoveComposerAttachment(0)).is_empty());
        assert!(state.composer_attachments.is_empty());
        assert!(
            path.is_file(),
            "Removing a draft must not invalidate a stored image"
        );
    }

    #[test]
    fn drop_accepts_existing_images_and_respects_available_slots() {
        let fixture = Fixture::new();
        let image = fixture.0.join("screen.PNG");
        let text = fixture.0.join("notes.txt");
        let directory = fixture.0.join("folder.png");
        assert!(std::fs::write(&image, png().bytes()).is_ok());
        assert!(std::fs::write(&text, "ordinary text").is_ok());
        assert!(std::fs::create_dir(&directory).is_ok());
        let paths = vec![
            text,
            directory,
            fixture.0.join("missing.png"),
            image.clone(),
        ];
        assert_eq!(dropped_images(&paths, 1), vec![image]);
        assert!(dropped_images(&paths, 0).is_empty());
    }

    #[test]
    fn invalid_clipboard_image_does_not_create_a_file() {
        let fixture = Fixture::new();
        let directory = fixture.0.join("pasted-images");
        assert!(store_clipboard_image(&directory, &Image::empty()).is_err());
        let oversized = Image::from_bytes(ImageFormat::Png, vec![0; MAX_CLIPBOARD_IMAGE_BYTES + 1]);
        assert!(store_clipboard_image(&directory, &oversized).is_err());
        assert!(!directory.exists());
    }
}
