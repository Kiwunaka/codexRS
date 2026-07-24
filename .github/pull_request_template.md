## What changed

Describe the observable behavior changed and the requirement or failure that
made it necessary.

## Verification

- [ ] `python scripts/check_dependency_policy.py`
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] Windows impact checked
- [ ] Linux impact checked

Add focused test results, screenshots, or smoke-test notes as appropriate.

## Safety

- [ ] No credentials, live `.codex` data, raw provider payloads, or proprietary
      upstream assets are included.
- [ ] New external input, output, queues, and subprocesses are explicitly
      bounded.
- [ ] The change does not add a browser runtime or bypass the official
      app-server boundary.
