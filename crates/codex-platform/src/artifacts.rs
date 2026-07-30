use std::error::Error;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

pub const MAX_ARTIFACT_PATH_BYTES: usize = 8 * 1024;
pub const MAX_ARTIFACT_PREVIEW_BYTES: u64 = 40 * 1024 * 1024;
pub const MAX_ARTIFACT_TEXT_BYTES: usize = 512 * 1024;

const SUPPORTED_EXTENSIONS: &[&str] = &[
    "avif", "csv", "doc", "docx", "gif", "jpeg", "jpg", "md", "mdx", "pdf", "png", "ppt", "pptx",
    "tsv", "webp", "xls", "xlsm", "xlsx",
];
const IMAGE_EXTENSIONS: &[&str] = &["jpeg", "jpg", "png", "webp"];
const TEXT_EXTENSIONS: &[&str] = &["csv", "md", "mdx", "tsv"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactFileKind {
    Text,
    Image,
    TooLarge,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactFilePreview {
    pub path: PathBuf,
    pub file_name: String,
    pub extension: String,
    pub size_bytes: u64,
    pub kind: ArtifactFileKind,
    pub text: Option<String>,
    pub truncated: bool,
}

#[derive(Debug)]
pub enum ArtifactError {
    InvalidWorkspace,
    InvalidPath,
    PathOutsideWorkspace,
    NotAFile,
    UnsupportedExtension,
    FileChanged,
    InvalidDestination,
    Io(io::Error),
    Launch(io::Error),
    Save(io::Error),
}

impl fmt::Display for ArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidWorkspace => formatter.write_str("the chat workspace is unavailable"),
            Self::InvalidPath => formatter.write_str("the output path is invalid"),
            Self::PathOutsideWorkspace => {
                formatter.write_str("the output is outside the chat workspace")
            }
            Self::NotAFile => formatter.write_str("the output file no longer exists"),
            Self::UnsupportedExtension => {
                formatter.write_str("this output type is not supported by the stable viewer")
            }
            Self::FileChanged => formatter.write_str("the output changed while it was being read"),
            Self::InvalidDestination => {
                formatter.write_str("the selected download destination is invalid")
            }
            Self::Io(_) => formatter.write_str("the output file could not be read"),
            Self::Launch(_) => formatter.write_str("the file manager could not be opened"),
            Self::Save(_) => formatter.write_str("the output file could not be saved"),
        }
    }
}

impl Error for ArtifactError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) | Self::Launch(error) | Self::Save(error) => Some(error),
            Self::InvalidWorkspace
            | Self::InvalidPath
            | Self::PathOutsideWorkspace
            | Self::NotAFile
            | Self::UnsupportedExtension
            | Self::FileChanged
            | Self::InvalidDestination => None,
        }
    }
}

#[must_use]
pub fn is_supported_artifact_path(path: &Path) -> bool {
    extension(path).is_some_and(|extension| SUPPORTED_EXTENSIONS.contains(&extension.as_str()))
}

pub fn inspect_artifact(
    workspace: &Path,
    requested_path: &Path,
) -> Result<ArtifactFilePreview, ArtifactError> {
    let path = resolve_artifact_path(workspace, requested_path)?;
    let metadata = path.metadata().map_err(ArtifactError::Io)?;
    if !metadata.is_file() {
        return Err(ArtifactError::NotAFile);
    }
    let extension = extension(&path).ok_or(ArtifactError::UnsupportedExtension)?;
    if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
        return Err(ArtifactError::UnsupportedExtension);
    }
    let size_bytes = metadata.len();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or(ArtifactError::InvalidPath)?
        .to_owned();
    if size_bytes > MAX_ARTIFACT_PREVIEW_BYTES {
        return Ok(ArtifactFilePreview {
            path,
            file_name,
            extension,
            size_bytes,
            kind: ArtifactFileKind::TooLarge,
            text: None,
            truncated: false,
        });
    }
    if TEXT_EXTENSIONS.contains(&extension.as_str()) {
        let (text, truncated) = read_bounded_text(&path)?;
        return Ok(ArtifactFilePreview {
            path,
            file_name,
            extension,
            size_bytes,
            kind: ArtifactFileKind::Text,
            text: Some(text),
            truncated,
        });
    }
    let kind = if IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        ArtifactFileKind::Image
    } else {
        ArtifactFileKind::Unsupported
    };
    Ok(ArtifactFilePreview {
        path,
        file_name,
        extension,
        size_bytes,
        kind,
        text: None,
        truncated: false,
    })
}

pub fn inspect_workspace_file(
    workspace: &Path,
    requested_path: &Path,
) -> Result<ArtifactFilePreview, ArtifactError> {
    let path = resolve_artifact_path(workspace, requested_path)?;
    let metadata = path.metadata().map_err(ArtifactError::Io)?;
    if !metadata.is_file() {
        return Err(ArtifactError::NotAFile);
    }
    let extension = extension(&path).unwrap_or_else(|| "file".to_owned());
    let size_bytes = metadata.len();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or(ArtifactError::InvalidPath)?
        .to_owned();
    if size_bytes > MAX_ARTIFACT_PREVIEW_BYTES {
        return Ok(ArtifactFilePreview {
            path,
            file_name,
            extension,
            size_bytes,
            kind: ArtifactFileKind::TooLarge,
            text: None,
            truncated: false,
        });
    }
    if IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        return Ok(ArtifactFilePreview {
            path,
            file_name,
            extension,
            size_bytes,
            kind: ArtifactFileKind::Image,
            text: None,
            truncated: false,
        });
    }
    let (text, truncated) = read_bounded_workspace_text(&path)?;
    Ok(ArtifactFilePreview {
        path,
        file_name,
        extension,
        size_bytes,
        kind: if text.is_some() {
            ArtifactFileKind::Text
        } else {
            ArtifactFileKind::Unsupported
        },
        text,
        truncated,
    })
}

pub fn read_artifact_image(
    workspace: &Path,
    requested_path: &Path,
) -> Result<Vec<u8>, ArtifactError> {
    let path = resolve_artifact_path(workspace, requested_path)?;
    let extension = extension(&path).ok_or(ArtifactError::UnsupportedExtension)?;
    if !IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        return Err(ArtifactError::UnsupportedExtension);
    }
    let metadata = path.metadata().map_err(ArtifactError::Io)?;
    if !metadata.is_file() {
        return Err(ArtifactError::NotAFile);
    }
    if metadata.len() > MAX_ARTIFACT_PREVIEW_BYTES {
        return Err(ArtifactError::FileChanged);
    }
    let mut file = File::open(path).map_err(ArtifactError::Io)?;
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len())
            .map_or(MAX_ARTIFACT_PREVIEW_BYTES as usize, |size| size)
            .min(MAX_ARTIFACT_PREVIEW_BYTES as usize),
    );
    file.by_ref()
        .take(MAX_ARTIFACT_PREVIEW_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(ArtifactError::Io)?;
    if bytes.len() as u64 > MAX_ARTIFACT_PREVIEW_BYTES {
        return Err(ArtifactError::FileChanged);
    }
    Ok(bytes)
}

pub fn reveal_artifact(workspace: &Path, requested_path: &Path) -> Result<(), ArtifactError> {
    open_workspace_path(workspace, requested_path)
}

pub fn save_artifact_copy(
    workspace: &Path,
    requested_path: &Path,
    destination: &Path,
) -> Result<(), ArtifactError> {
    let source = resolve_artifact_path(workspace, requested_path)?;
    let source_metadata = source.metadata().map_err(ArtifactError::Io)?;
    if !source_metadata.is_file() {
        return Err(ArtifactError::NotAFile);
    }
    if destination.as_os_str().is_empty()
        || !destination.is_absolute()
        || destination.to_string_lossy().len() > MAX_ARTIFACT_PATH_BYTES
        || destination.file_name().is_none()
    {
        return Err(ArtifactError::InvalidDestination);
    }
    let destination_parent = destination
        .parent()
        .filter(|parent| parent.is_dir())
        .ok_or(ArtifactError::InvalidDestination)?;
    if !destination_parent.is_absolute() {
        return Err(ArtifactError::InvalidDestination);
    }
    if destination.exists()
        && destination
            .canonicalize()
            .is_ok_and(|existing| existing == source)
    {
        return Err(ArtifactError::InvalidDestination);
    }
    fs::copy(source, destination)
        .map(|_| ())
        .map_err(ArtifactError::Save)
}

pub fn open_workspace_path(workspace: &Path, requested_path: &Path) -> Result<(), ArtifactError> {
    let path = resolve_artifact_path(workspace, requested_path)?;
    let metadata = path.metadata().map_err(ArtifactError::Io)?;
    if !metadata.is_file() && !metadata.is_dir() {
        return Err(ArtifactError::NotAFile);
    }
    let child = open_path_command(&path, metadata.is_dir())
        .spawn()
        .map_err(ArtifactError::Launch)?;
    reap_child(child);
    Ok(())
}

fn resolve_artifact_path(
    workspace: &Path,
    requested_path: &Path,
) -> Result<PathBuf, ArtifactError> {
    if workspace.as_os_str().is_empty()
        || requested_path.as_os_str().is_empty()
        || requested_path.to_string_lossy().len() > MAX_ARTIFACT_PATH_BYTES
    {
        return Err(ArtifactError::InvalidPath);
    }
    let workspace = workspace
        .canonicalize()
        .map_err(|_| ArtifactError::InvalidWorkspace)?;
    if !workspace.is_dir() {
        return Err(ArtifactError::InvalidWorkspace);
    }
    let candidate = if requested_path.is_absolute() {
        requested_path.to_path_buf()
    } else {
        workspace.join(requested_path)
    };
    let path = candidate
        .canonicalize()
        .map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => ArtifactError::NotAFile,
            _ => ArtifactError::Io(error),
        })?;
    if !path.starts_with(&workspace) {
        return Err(ArtifactError::PathOutsideWorkspace);
    }
    Ok(path)
}

fn read_bounded_text(path: &Path) -> Result<(String, bool), ArtifactError> {
    let mut file = File::open(path).map_err(ArtifactError::Io)?;
    let mut bytes = Vec::with_capacity(MAX_ARTIFACT_TEXT_BYTES.min(64 * 1024));
    file.by_ref()
        .take((MAX_ARTIFACT_TEXT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(ArtifactError::Io)?;
    let truncated = bytes.len() > MAX_ARTIFACT_TEXT_BYTES;
    bytes.truncate(MAX_ARTIFACT_TEXT_BYTES);
    Ok((String::from_utf8_lossy(&bytes).into_owned(), truncated))
}

fn read_bounded_workspace_text(path: &Path) -> Result<(Option<String>, bool), ArtifactError> {
    let mut file = File::open(path).map_err(ArtifactError::Io)?;
    let mut bytes = Vec::with_capacity(MAX_ARTIFACT_TEXT_BYTES.min(64 * 1024));
    file.by_ref()
        .take((MAX_ARTIFACT_TEXT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(ArtifactError::Io)?;
    let truncated = bytes.len() > MAX_ARTIFACT_TEXT_BYTES;
    bytes.truncate(MAX_ARTIFACT_TEXT_BYTES);
    if bytes.contains(&0) {
        return Ok((None, truncated));
    }
    Ok((
        Some(String::from_utf8_lossy(&bytes).into_owned()),
        truncated,
    ))
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::trim)
        .filter(|extension| !extension.is_empty())
        .map(str::to_ascii_lowercase)
}

#[cfg(windows)]
fn open_path_command(path: &Path, is_directory: bool) -> Command {
    let mut command = Command::new("explorer.exe");
    if is_directory {
        command.arg(path);
    } else {
        command.arg("/select,").arg(path);
    }
    command
}

#[cfg(target_os = "linux")]
fn open_path_command(path: &Path, is_directory: bool) -> Command {
    let mut command = Command::new("xdg-open");
    command.arg(if is_directory {
        path
    } else {
        path.parent().unwrap_or(path)
    });
    command
}

#[cfg(not(any(windows, target_os = "linux")))]
fn open_path_command(_path: &Path, _is_directory: bool) -> Command {
    Command::new("false")
}

fn reap_child(mut child: Child) {
    std::thread::spawn(move || {
        let _ = child.wait();
    });
}

#[cfg(test)]
mod tests {
    use super::{
        ArtifactError, ArtifactFileKind, MAX_ARTIFACT_TEXT_BYTES, inspect_artifact,
        inspect_workspace_file, is_supported_artifact_path, save_artifact_copy,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn supported_extensions_match_the_stable_output_contract() {
        for path in [
            "output.csv",
            "output.docx",
            "output.jpg",
            "output.md",
            "output.pdf",
            "output.pptx",
            "output.xlsx",
        ] {
            assert!(is_supported_artifact_path(PathBuf::from(path).as_path()));
        }
        assert!(!is_supported_artifact_path(
            PathBuf::from("output.exe").as_path()
        ));
    }

    #[test]
    fn text_preview_is_bounded_and_cannot_escape_the_workspace()
    -> Result<(), Box<dyn std::error::Error>> {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let sandbox =
            std::env::temp_dir().join(format!("codexrs-artifact-{}-{unique}", std::process::id()));
        let root = sandbox.join("workspace");
        fs::create_dir_all(&root)?;
        let output = root.join("output.md");
        let outside = sandbox.join("outside.md");
        fs::write(&output, vec![b'x'; MAX_ARTIFACT_TEXT_BYTES + 64])?;
        fs::write(&outside, b"outside")?;
        let source = root.join("main.rs");
        let binary = root.join("binary.dat");
        fs::write(&source, b"fn main() {}\n")?;
        fs::write(&binary, b"binary\0payload")?;

        let preview = inspect_artifact(&root, PathBuf::from("output.md").as_path())?;
        assert_eq!(preview.kind, ArtifactFileKind::Text);
        assert!(preview.truncated);
        assert_eq!(
            preview.text.as_deref().map(str::len),
            Some(MAX_ARTIFACT_TEXT_BYTES)
        );
        assert!(matches!(
            inspect_artifact(&root, &outside),
            Err(ArtifactError::PathOutsideWorkspace)
        ));
        let source_preview = inspect_workspace_file(&root, PathBuf::from("main.rs").as_path())?;
        assert_eq!(source_preview.kind, ArtifactFileKind::Text);
        assert_eq!(source_preview.text.as_deref(), Some("fn main() {}\n"));
        assert_eq!(
            inspect_workspace_file(&root, PathBuf::from("binary.dat").as_path())?.kind,
            ArtifactFileKind::Unsupported
        );

        fs::remove_file(output)?;
        fs::remove_file(outside)?;
        fs::remove_file(source)?;
        fs::remove_file(binary)?;
        fs::remove_dir(root)?;
        fs::remove_dir(sandbox)?;
        Ok(())
    }

    #[test]
    fn artifact_copy_is_workspace_confined() -> Result<(), Box<dyn std::error::Error>> {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let sandbox =
            std::env::temp_dir().join(format!("codexrs-download-{}-{unique}", std::process::id()));
        let root = sandbox.join("workspace");
        fs::create_dir_all(&root)?;
        let source = root.join("generated.png");
        let outside = sandbox.join("outside.png");
        let destination = sandbox.join("Codex Image.png");
        fs::write(&source, b"generated image")?;
        fs::write(&outside, b"outside")?;

        save_artifact_copy(
            &root,
            PathBuf::from("generated.png").as_path(),
            &destination,
        )?;
        assert_eq!(fs::read(&destination)?, b"generated image");
        assert!(matches!(
            save_artifact_copy(&root, &outside, &sandbox.join("blocked.png")),
            Err(ArtifactError::PathOutsideWorkspace)
        ));
        assert!(matches!(
            save_artifact_copy(&root, &source, &source),
            Err(ArtifactError::InvalidDestination)
        ));

        fs::remove_file(source)?;
        fs::remove_file(outside)?;
        fs::remove_file(destination)?;
        fs::remove_dir(root)?;
        fs::remove_dir(sandbox)?;
        Ok(())
    }
}
