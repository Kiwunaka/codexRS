# Architecture

## Reference boundary

Stable `26.721.3996.0` is an executable specification. We inspect it to recover
user-visible behavior, app-server messages, and compatibility requirements.
Extracted JavaScript and OpenAI assets are not production dependencies.

## Workspace

```text
codex-app
├── codex-core       domain types and state transitions
├── codex-protocol   bounded app-server transport and message schema
├── codex-storage    SQLite persistence, paging, retention, migration
└── codex-platform   paths, Git, processes, Windows integration
```

The UI must depend on these boundaries. Storage, Git, subprocesses, and network
work must never execute on the UI thread.

## Non-negotiable invariants

1. Every frame, event, queue, and log has a configurable upper bound.
2. Session lists load metadata only; messages load in bounded pages.
3. Large tool output is stored as a separate blob or truncated preview.
4. Storage has one writer and explicit schema migrations.
5. Git refreshes are debounced, coalesced, cancellable, and concurrency-limited.
6. Child processes belong to one supervisor and shut down once.
7. Windows paths stay as `Path`/`PathBuf`; they are never passed through POSIX
   path semantics.
8. Existing Codex history is imported from a snapshot and never mutated in
   place.

## Planned milestones

1. Capture and replay the stable app-server handshake.
2. Build a read-only session browser against a copied data fixture.
3. Select and validate the native UI renderer.
4. Implement conversation streaming and tool approvals.
5. Add Git, terminal, MCP, plugins, and production migrations.
