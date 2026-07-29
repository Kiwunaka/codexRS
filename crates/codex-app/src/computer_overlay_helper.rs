use std::process::ExitCode;

fn main() -> ExitCode {
    match codex_platform::run_computer_use_overlay_helper() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("codex-computer-use-overlay: {error}");
            ExitCode::FAILURE
        }
    }
}
