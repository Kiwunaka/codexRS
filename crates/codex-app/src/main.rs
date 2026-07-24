use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;
use std::process::ExitCode;

use codex_platform::{
    AppServerClient, AppServerConfig, AppServerError, CodexHome, CodexHomeKind,
    DEFAULT_THREAD_PAGE_LIMIT, MAX_THREAD_PAGE_LIMIT, resolve_codex_binary,
};
use codex_protocol::ClientInfo;

mod backend;
mod ui;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("codex-rs: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), CliError> {
    let mut args = env::args_os();
    let _program = args.next();
    match args.next() {
        None => {
            ui::run();
            Ok(())
        }
        Some(command) if command == "info" => {
            print_bootstrap();
            Ok(())
        }
        Some(command) if command == "probe" => run_probe(args),
        Some(command) if command == "--help" || command == "-h" => {
            print_help();
            Ok(())
        }
        Some(_) => Err(CliError::UnknownCommand),
    }
}

fn run_probe(args: impl Iterator<Item = OsString>) -> Result<(), CliError> {
    let mut codex_binary = resolve_codex_binary(None);
    let mut codex_home = None;
    let mut limit = DEFAULT_THREAD_PAGE_LIMIT;
    let mut args = args;

    while let Some(option) = args.next() {
        match option.to_str() {
            Some("--codex-bin") => {
                codex_binary =
                    PathBuf::from(args.next().ok_or(CliError::MissingValue("--codex-bin"))?);
            }
            Some("--codex-home") => {
                codex_home = Some(PathBuf::from(
                    args.next().ok_or(CliError::MissingValue("--codex-home"))?,
                ));
            }
            Some("--limit") => {
                let value = args.next().ok_or(CliError::MissingValue("--limit"))?;
                limit = value
                    .to_str()
                    .and_then(|value| value.parse::<u32>().ok())
                    .ok_or(CliError::InvalidLimit)?;
            }
            Some("--help" | "-h") => {
                print_probe_help();
                return Ok(());
            }
            _ => return Err(CliError::UnknownOption),
        }
    }

    if !(1..=MAX_THREAD_PAGE_LIMIT).contains(&limit) {
        return Err(CliError::InvalidLimit);
    }

    let home = CodexHome::resolve(codex_home)?;
    let home_kind = home.kind();
    let mut client = AppServerClient::spawn(AppServerConfig::new(codex_binary, home))?;
    let _initialize = client.initialize(ClientInfo {
        name: "codex-rs".to_owned(),
        title: Some("codexRS".to_owned()),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    })?;
    let page = client.list_threads_state_db_only(limit)?;
    client.shutdown()?;

    let home_label = match home_kind {
        CodexHomeKind::Default => "default",
        CodexHomeKind::Configured => "configured",
    };
    println!("app-server: initialized");
    println!("codex-home: {home_label} (verified)");
    println!(
        "threads: {} (more: {})",
        page.data.len(),
        if page.next_cursor.is_some() {
            "yes"
        } else {
            "no"
        }
    );
    Ok(())
}

fn print_bootstrap() {
    let reference = codex_core::stable_reference();
    let runtime_policy = codex_platform::RuntimePolicy::default();

    println!("codex-rs bootstrap");
    println!(
        "reference: {} {} {}",
        reference.package_name, reference.package_version, reference.architecture
    );
    println!("runtime: {}", reference.runtime);
    println!(
        "limits: protocol={} MiB, inline-event={} MiB, git-processes={}",
        codex_protocol::DEFAULT_MAX_FRAME_BYTES / (1024 * 1024),
        codex_storage::MAX_INLINE_EVENT_BYTES / (1024 * 1024),
        runtime_policy.max_parallel_git_processes
    );
    println!("run `codexrs --help` for app-server probe options");
}

fn print_help() {
    println!("Usage:");
    println!("  codexrs              open the native desktop client");
    println!("  codexrs info         print bounded runtime information");
    println!("  codexrs probe [OPTIONS]");
    println!();
    println!("The probe uses the default ~/.codex unless --codex-home or CODEX_HOME is set.");
    println!("Set CODEX_RS_CODEX_BIN when codex.exe is not available through PATH.");
}

fn print_probe_help() {
    println!("Usage: codexrs probe [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --codex-bin <PATH>   codex executable (or CODEX_RS_CODEX_BIN)");
    println!("  --codex-home <PATH>  override CODEX_HOME; defaults to ~/.codex");
    println!("  --limit <1..=100>    metadata page size; defaults to 20");
}

#[derive(Debug)]
enum CliError {
    UnknownCommand,
    UnknownOption,
    MissingValue(&'static str),
    InvalidLimit,
    AppServer(AppServerError),
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCommand => formatter.write_str("unknown command; use --help"),
            Self::UnknownOption => formatter.write_str("unknown probe option; use probe --help"),
            Self::MissingValue(option) => write!(formatter, "missing value for {option}"),
            Self::InvalidLimit => {
                formatter.write_str("limit must be an integer from 1 through 100")
            }
            Self::AppServer(error) => error.fmt(formatter),
        }
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AppServer(error) => Some(error),
            Self::UnknownCommand
            | Self::UnknownOption
            | Self::MissingValue(_)
            | Self::InvalidLimit => None,
        }
    }
}

impl From<AppServerError> for CliError {
    fn from(error: AppServerError) -> Self {
        Self::AppServer(error)
    }
}
