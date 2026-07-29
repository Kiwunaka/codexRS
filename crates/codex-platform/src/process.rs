use std::error::Error;
use std::fmt;
use std::io::{self, Read};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use win32job::{ExtendedLimitInfo, Job};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const WAIT_TICK: Duration = Duration::from_millis(10);

#[derive(Debug)]
pub(crate) struct BoundedOutput {
    pub stdout: Vec<u8>,
    pub stdout_truncated: bool,
}

#[derive(Debug)]
pub enum ProcessError {
    Spawn(io::Error),
    Pipe(&'static str),
    Reader(io::Error),
    ReaderPanicked,
    Wait(io::Error),
    Cancelled,
    TimedOut,
    Exit {
        status: ExitStatus,
        stderr: String,
    },
    #[cfg(windows)]
    Job(win32job::JobError),
}

impl fmt::Display for ProcessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(_) => formatter.write_str("could not start process"),
            Self::Pipe(name) => write!(formatter, "process did not expose {name}"),
            Self::Reader(_) => formatter.write_str("could not read process output"),
            Self::ReaderPanicked => formatter.write_str("process output reader panicked"),
            Self::Wait(_) => formatter.write_str("could not wait for process"),
            Self::Cancelled => formatter.write_str("process was cancelled"),
            Self::TimedOut => formatter.write_str("process timed out"),
            Self::Exit { status, stderr } => {
                let code = status
                    .code()
                    .map_or_else(|| "signal".to_owned(), |code| code.to_string());
                if stderr.trim().is_empty() {
                    write!(formatter, "process exited with {code}")
                } else {
                    write!(formatter, "process exited with {code}: {}", stderr.trim())
                }
            }
            #[cfg(windows)]
            Self::Job(_) => formatter.write_str("could not supervise process with a Job Object"),
        }
    }
}

impl Error for ProcessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Spawn(error) | Self::Reader(error) | Self::Wait(error) => Some(error),
            #[cfg(windows)]
            Self::Job(error) => Some(error),
            Self::Pipe(_)
            | Self::ReaderPanicked
            | Self::Cancelled
            | Self::TimedOut
            | Self::Exit { .. } => None,
        }
    }
}

pub(crate) fn run_bounded(
    command: &mut Command,
    stdout_limit: usize,
    stderr_limit: usize,
    timeout: Duration,
) -> Result<BoundedOutput, ProcessError> {
    run_bounded_inner(command, stdout_limit, stderr_limit, timeout, None)
}

pub(crate) fn run_bounded_cancelable(
    command: &mut Command,
    stdout_limit: usize,
    stderr_limit: usize,
    timeout: Duration,
    cancellation: &AtomicBool,
) -> Result<BoundedOutput, ProcessError> {
    run_bounded_inner(
        command,
        stdout_limit,
        stderr_limit,
        timeout,
        Some(cancellation),
    )
}

fn run_bounded_inner(
    command: &mut Command,
    stdout_limit: usize,
    stderr_limit: usize,
    timeout: Duration,
    cancellation: Option<&AtomicBool>,
) -> Result<BoundedOutput, ProcessError> {
    if cancellation.is_some_and(|cancellation| cancellation.load(Ordering::Acquire)) {
        return Err(ProcessError::Cancelled);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    #[cfg(windows)]
    let job = {
        let mut limits = ExtendedLimitInfo::new();
        limits.limit_kill_on_job_close();
        Job::create_with_limit_info(&limits).map_err(ProcessError::Job)?
    };

    let mut child = command.spawn().map_err(ProcessError::Spawn)?;
    #[cfg(windows)]
    if let Err(error) = job.assign_process(child.as_raw_handle() as isize) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(ProcessError::Job(error));
    }

    let stdout = child.stdout.take().ok_or(ProcessError::Pipe("stdout"))?;
    let stderr = child.stderr.take().ok_or(ProcessError::Pipe("stderr"))?;
    let stdout_reader = thread::Builder::new()
        .name("codex-platform-stdout".to_owned())
        .spawn(move || read_bounded(stdout, stdout_limit))
        .map_err(ProcessError::Reader)?;
    let stderr_reader = thread::Builder::new()
        .name("codex-platform-stderr".to_owned())
        .spawn(move || read_bounded(stderr, stderr_limit))
        .map_err(ProcessError::Reader)?;

    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(ProcessError::Wait)? {
            break status;
        }
        if cancellation.is_some_and(|cancellation| cancellation.load(Ordering::Acquire)) {
            #[cfg(windows)]
            drop(job);
            #[cfg(not(windows))]
            {
                let _ = child.kill();
            }
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(ProcessError::Cancelled);
        }
        if Instant::now() >= deadline {
            #[cfg(windows)]
            drop(job);
            #[cfg(not(windows))]
            {
                let _ = child.kill();
            }
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(ProcessError::TimedOut);
        }
        thread::sleep(WAIT_TICK);
    };

    #[cfg(windows)]
    drop(job);
    let (stdout, stdout_truncated) = stdout_reader
        .join()
        .map_err(|_| ProcessError::ReaderPanicked)??;
    let (stderr, _) = stderr_reader
        .join()
        .map_err(|_| ProcessError::ReaderPanicked)??;
    if !status.success() {
        return Err(ProcessError::Exit {
            status,
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        });
    }
    Ok(BoundedOutput {
        stdout,
        stdout_truncated,
    })
}

fn read_bounded(mut reader: impl Read, limit: usize) -> Result<(Vec<u8>, bool), ProcessError> {
    let mut output = Vec::with_capacity(limit.min(64 * 1024));
    let mut truncated = false;
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(ProcessError::Reader)?;
        if read == 0 {
            return Ok((output, truncated));
        }
        let remaining = limit.saturating_sub(output.len());
        let keep = read.min(remaining);
        output.extend_from_slice(&buffer[..keep]);
        truncated |= keep < read;
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::process::Command;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    use super::{ProcessError, read_bounded, run_bounded_cancelable};

    #[test]
    fn bounded_reader_drains_but_keeps_only_the_budget() -> Result<(), super::ProcessError> {
        let input = Cursor::new(vec![b'x'; 32]);
        let (output, truncated) = read_bounded(input, 7)?;

        assert_eq!(output, vec![b'x'; 7]);
        assert!(truncated);
        Ok(())
    }

    #[test]
    fn cancellation_terminates_a_supervised_process() {
        let cancellation = Arc::new(AtomicBool::new(false));
        let cancellation_signal = Arc::clone(&cancellation);
        let signal = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            cancellation_signal.store(true, Ordering::Release);
        });
        let mut command = slow_command();
        let started = Instant::now();

        let result = run_bounded_cancelable(
            &mut command,
            1_024,
            1_024,
            Duration::from_secs(10),
            &cancellation,
        );

        assert!(signal.join().is_ok(), "cancellation signal should finish");
        assert!(matches!(result, Err(ProcessError::Cancelled)));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(windows)]
    fn slow_command() -> Command {
        let mut command = Command::new("powershell.exe");
        command.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Start-Sleep -Seconds 5",
        ]);
        command
    }

    #[cfg(not(windows))]
    fn slow_command() -> Command {
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 5"]);
        command
    }
}
