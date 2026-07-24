# Known stable failures

These failures are acceptance-test inputs, not implementation details to copy.

| Failure | Local evidence | Required behavior |
| --- | --- | --- |
| Windows multi-root white screen | `path.posix.relative` receives drive paths and reaches a missing `process.cwd()` | Use native path components; opening multiple roots cannot crash |
| Unbounded JSONL line | Observed line: 594,127,437 bytes | Reject or quarantine oversized records without allocating the full line |
| Startup history scan | History reached roughly 9 GB with many files over 100 MB | Query metadata only and page message bodies |
| Git process storm | Public reports and local investigation show repeated `git.exe` spawning | Debounce, coalesce, cache, and cap Git concurrency |
| Process cleanup storm | Repeated `taskkill.exe`, `conhost.exe`, and WMI activity | Supervise one process tree and perform one bounded shutdown |
| Unbounded logging | Repeated writes and growth of `logs_2.sqlite` | Batch, rotate, retain, and expose disk budgets |

## Initial budgets

- Protocol frame: 16 MiB.
- Stored inline event: 8 MiB.
- Default history page: 100 events.
- Maximum history page: 500 events.
- Diagnostic log budget: 64 MiB.
- Parallel Git subprocesses: 1 until measurements prove otherwise.
