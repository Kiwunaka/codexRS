## What changed

<!-- Describe the user-visible or contract-level change. -->

## Why

<!-- Link the concrete requirement, failure, or risk that requires this change. -->

## Verification

- [ ] `cargo.exe fmt --all --check`
- [ ] `cargo.exe clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo.exe test --workspace`

## Safety

- [ ] No credentials, live `.codex` data, databases, logs, extracted upstream
      bundles, executables, or OpenAI assets are included.
