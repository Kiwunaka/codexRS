fn main() {
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
}
