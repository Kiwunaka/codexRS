use std::{
    env,
    error::Error,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
    process,
};

const LINUX_DESKTOP_ENTRY_FILE_NAME: &str = "com.codexrs.CodexRS.desktop";

const MAX_LINUX_DESKTOP_PATH_BYTES: usize = 8 * 1024;
const MAX_LINUX_DESKTOP_ENTRY_BYTES: usize = 16 * 1024;
const MAX_TEMPORARY_ENTRY_ATTEMPTS: u32 = 16;

#[derive(Debug)]
pub enum LinuxDesktopEntryError {
    MissingHome,
    InvalidDataDirectory(&'static str),
    InvalidExecutablePath,
    TemporaryNameExhausted,
    EntryAlreadyExists {
        path: PathBuf,
    },
    Io {
        action: &'static str,
        source: io::Error,
    },
}

impl fmt::Display for LinuxDesktopEntryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHome => formatter.write_str("HOME is required when XDG_DATA_HOME is unset"),
            Self::InvalidDataDirectory(variable) => {
                write!(formatter, "{variable} must be a bounded absolute path")
            }
            Self::InvalidExecutablePath => formatter.write_str(
                "the current executable path must be a bounded UTF-8 absolute path without controls or field codes",
            ),
            Self::TemporaryNameExhausted => {
                formatter.write_str("could not reserve a temporary desktop entry path")
            }
            Self::EntryAlreadyExists { path } => write!(
                formatter,
                "desktop entry already exists at {}; it was not inspected or changed; remove it manually, then rerun the command",
                path.display()
            ),
            Self::Io { action, source } => write!(formatter, "could not {action}: {source}"),
        }
    }
}

impl Error for LinuxDesktopEntryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::MissingHome
            | Self::InvalidDataDirectory(_)
            | Self::InvalidExecutablePath
            | Self::TemporaryNameExhausted
            | Self::EntryAlreadyExists { .. } => None,
        }
    }
}

pub fn install_linux_desktop_entry() -> Result<PathBuf, LinuxDesktopEntryError> {
    let applications_directory = desktop_applications_directory(
        env::var_os("XDG_DATA_HOME").as_deref().map(Path::new),
        env::var_os("HOME").as_deref().map(Path::new),
    )?;
    let executable = env::current_exe().map_err(|source| LinuxDesktopEntryError::Io {
        action: "resolve the current executable path",
        source,
    })?;

    install_linux_desktop_entry_at(&applications_directory, &executable)
}

fn desktop_applications_directory(
    xdg_data_home: Option<&Path>,
    home: Option<&Path>,
) -> Result<PathBuf, LinuxDesktopEntryError> {
    let data_directory = match xdg_data_home {
        Some(path) => {
            validate_data_directory(path, "XDG_DATA_HOME")?;
            path.to_path_buf()
        }
        None => {
            let home = home.ok_or(LinuxDesktopEntryError::MissingHome)?;
            validate_data_directory(home, "HOME")?;
            home.join(".local").join("share")
        }
    };
    let applications_directory = data_directory.join("applications");
    validate_data_directory(&applications_directory, "XDG_DATA_HOME")?;
    Ok(applications_directory)
}

fn validate_data_directory(
    path: &Path,
    variable: &'static str,
) -> Result<(), LinuxDesktopEntryError> {
    if !path.is_absolute()
        || path.as_os_str().is_empty()
        || path.as_os_str().len() > MAX_LINUX_DESKTOP_PATH_BYTES
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(LinuxDesktopEntryError::InvalidDataDirectory(variable));
    }
    Ok(())
}

fn install_linux_desktop_entry_at(
    applications_directory: &Path,
    executable: &Path,
) -> Result<PathBuf, LinuxDesktopEntryError> {
    let contents = render_desktop_entry(executable)?;
    fs::create_dir_all(applications_directory).map_err(|source| LinuxDesktopEntryError::Io {
        action: "create the desktop entry directory",
        source,
    })?;

    let destination = applications_directory.join(LINUX_DESKTOP_ENTRY_FILE_NAME);
    let (mut temporary, temporary_path) = create_temporary_entry(applications_directory)?;
    if let Err(error) = temporary
        .write_all(&contents)
        .and_then(|_| temporary.sync_all())
    {
        drop(temporary);
        let _ = fs::remove_file(&temporary_path);
        return Err(LinuxDesktopEntryError::Io {
            action: "write the temporary desktop entry",
            source: error,
        });
    }
    drop(temporary);

    let result = publish_temporary_entry(&temporary_path, &destination);
    let cleanup = fs::remove_file(&temporary_path);
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(destination),
        (Ok(()), Err(source)) => Err(LinuxDesktopEntryError::Io {
            action: "remove the temporary desktop entry",
            source,
        }),
        (Err(error), _) => Err(error),
    }
}

fn render_desktop_entry(executable: &Path) -> Result<Vec<u8>, LinuxDesktopEntryError> {
    if !executable.is_absolute() || executable.as_os_str().len() > MAX_LINUX_DESKTOP_PATH_BYTES {
        return Err(LinuxDesktopEntryError::InvalidExecutablePath);
    }
    let executable = executable
        .to_str()
        .filter(|path| {
            !path.is_empty() && !path.contains('%') && !path.chars().any(char::is_control)
        })
        .ok_or(LinuxDesktopEntryError::InvalidExecutablePath)?;

    let mut escaped = String::with_capacity(executable.len());
    for character in executable.chars() {
        if character == '\\' {
            escaped.push_str(r"\\\\");
        } else {
            if matches!(character, '"' | '`' | '$') {
                escaped.push('\\');
            }
            escaped.push(character);
        }
    }
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=codexRS\nComment=Native Codex desktop client\nExec=\"{escaped}\"\nTerminal=false\nCategories=Development;\n"
    );
    if entry.len() > MAX_LINUX_DESKTOP_ENTRY_BYTES {
        return Err(LinuxDesktopEntryError::InvalidExecutablePath);
    }
    Ok(entry.into_bytes())
}

fn create_temporary_entry(directory: &Path) -> Result<(File, PathBuf), LinuxDesktopEntryError> {
    for sequence in 0..MAX_TEMPORARY_ENTRY_ATTEMPTS {
        let path = temporary_entry_path(directory, sequence);
        match OpenOptions::new().create_new(true).write(true).open(&path) {
            Ok(file) => return Ok((file, path)),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(LinuxDesktopEntryError::Io {
                    action: "create a temporary desktop entry",
                    source,
                });
            }
        }
    }
    Err(LinuxDesktopEntryError::TemporaryNameExhausted)
}

fn temporary_entry_path(directory: &Path, sequence: u32) -> PathBuf {
    directory.join(format!(
        ".{LINUX_DESKTOP_ENTRY_FILE_NAME}.{}-{sequence}.tmp",
        process::id()
    ))
}

fn publish_temporary_entry(
    temporary_path: &Path,
    destination: &Path,
) -> Result<(), LinuxDesktopEntryError> {
    fs::hard_link(temporary_path, destination).map_err(|source| {
        if source.kind() == io::ErrorKind::AlreadyExists {
            LinuxDesktopEntryError::EntryAlreadyExists {
                path: destination.to_path_buf(),
            }
        } else {
            LinuxDesktopEntryError::Io {
                action: "publish the desktop entry",
                source,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::OsString,
        fs,
        io::Read,
        os::unix::fs::symlink,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock is after the Unix epoch")
                .as_nanos();
            let path =
                env::temp_dir().join(format!("codexrs-desktop-entry-{}-{nanos}", process::id()));
            fs::create_dir(&path).expect("create test directory");
            Self { path }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn read_test_file(path: &Path) -> Vec<u8> {
        let mut file = fs::File::open(path).expect("open test file");
        let mut bytes = Vec::new();
        std::io::Read::by_ref(&mut file)
            .take((MAX_LINUX_DESKTOP_ENTRY_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .expect("read test file");
        assert!(bytes.len() <= MAX_LINUX_DESKTOP_ENTRY_BYTES);
        bytes
    }

    #[test]
    fn desktop_applications_directory_requires_absolute_xdg_or_home() {
        let directory = desktop_applications_directory(Some(Path::new("/tmp/data")), None)
            .expect("resolve XDG data directory");
        assert_eq!(directory, PathBuf::from("/tmp/data/applications"));

        let directory = desktop_applications_directory(None, Some(Path::new("/tmp/home")))
            .expect("resolve HOME data directory");
        assert_eq!(
            directory,
            PathBuf::from("/tmp/home/.local/share/applications")
        );

        assert!(matches!(
            desktop_applications_directory(Some(Path::new("relative")), None),
            Err(LinuxDesktopEntryError::InvalidDataDirectory(
                "XDG_DATA_HOME"
            ))
        ));
        assert!(matches!(
            desktop_applications_directory(None, Some(Path::new("relative"))),
            Err(LinuxDesktopEntryError::InvalidDataDirectory("HOME"))
        ));
    }

    #[test]
    fn desktop_entry_escapes_exec_without_shell_or_field_codes() {
        let entry = render_desktop_entry(Path::new(r#"/opt/Codex RS/$`"\codexrs"#))
            .expect("render desktop entry");
        let entry = String::from_utf8(entry).expect("desktop entry is UTF-8");

        assert!(entry.contains("Exec=\"/opt/Codex RS/\\$\\`\\\"\\\\\\\\codexrs\"\n"));
        assert!(!entry.contains("sh -c"));
        assert!(!entry.contains('%'));
        assert!(matches!(
            render_desktop_entry(Path::new("/opt/codex%rs")),
            Err(LinuxDesktopEntryError::InvalidExecutablePath)
        ));
        assert!(matches!(
            render_desktop_entry(Path::new("/opt/codex\nrs")),
            Err(LinuxDesktopEntryError::InvalidExecutablePath)
        ));
    }

    #[test]
    fn install_creates_entry_and_cleans_temporary_file() {
        let directory = TestDirectory::new();
        let applications = directory.path.join("applications");
        let installed = install_linux_desktop_entry_at(&applications, Path::new("/opt/codexrs"))
            .expect("install desktop entry");

        assert_eq!(installed, applications.join(LINUX_DESKTOP_ENTRY_FILE_NAME));
        let contents =
            String::from_utf8(read_test_file(&installed)).expect("desktop entry is UTF-8");
        assert!(contents.contains("Exec=\"/opt/codexrs\""));
        let names = fs::read_dir(&applications)
            .expect("read applications directory")
            .map(|entry| entry.expect("read directory entry").file_name())
            .collect::<Vec<_>>();
        assert_eq!(names, vec![OsString::from(LINUX_DESKTOP_ENTRY_FILE_NAME)]);
    }

    #[test]
    fn install_never_changes_an_existing_regular_file() {
        let directory = TestDirectory::new();
        let applications = directory.path.join("applications");
        fs::create_dir_all(&applications).expect("create applications directory");
        let destination = applications.join(LINUX_DESKTOP_ENTRY_FILE_NAME);
        fs::write(&destination, b"foreign entry").expect("write foreign entry");

        assert!(matches!(
            install_linux_desktop_entry_at(&applications, Path::new("/opt/codexrs")),
            Err(LinuxDesktopEntryError::EntryAlreadyExists { .. })
        ));
        assert_eq!(read_test_file(&destination), b"foreign entry");
        let names = fs::read_dir(&applications)
            .expect("read applications directory")
            .map(|entry| entry.expect("read directory entry").file_name())
            .collect::<Vec<_>>();
        assert_eq!(names, vec![OsString::from(LINUX_DESKTOP_ENTRY_FILE_NAME)]);
    }

    #[test]
    fn install_never_changes_an_existing_symlink() {
        let directory = TestDirectory::new();
        let applications = directory.path.join("applications");
        fs::create_dir_all(&applications).expect("create applications directory");
        let target = directory.path.join("foreign.desktop");
        fs::write(&target, b"foreign entry").expect("write symlink target");
        let destination = applications.join(LINUX_DESKTOP_ENTRY_FILE_NAME);
        symlink(&target, &destination).expect("create destination symlink");

        assert!(matches!(
            install_linux_desktop_entry_at(&applications, Path::new("/opt/codexrs")),
            Err(LinuxDesktopEntryError::EntryAlreadyExists { .. })
        ));
        assert_eq!(read_test_file(&target), b"foreign entry");
        assert!(
            fs::symlink_metadata(&destination)
                .expect("read destination metadata")
                .file_type()
                .is_symlink()
        );
    }
}
