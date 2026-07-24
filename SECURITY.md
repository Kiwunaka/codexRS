# Security policy

## Supported versions

| Version | Security updates |
| --- | --- |
| `main` | Yes |
| Latest pre-release | Yes |
| Older pre-releases | No |

Until the first stable release, fixes land on `main` and the next candidate.

## Reporting a vulnerability

Do not open a public issue. Use
[GitHub private vulnerability reporting](https://github.com/Kiwunaka/codexRS/security/advisories/new)
and include:

- the affected commit or release;
- platform and official Codex CLI version;
- the trust boundary involved;
- minimal reproduction steps using isolated fixtures;
- impact and any known mitigations.

Remove credentials, tokens, private keys, user history, screenshots, and raw
provider payloads. A maintainer will acknowledge the report as capacity allows,
confirm scope, and coordinate disclosure after a fix is available.

## In scope

- app-server framing, request routing, and approval handling;
- process-tree supervision and shutdown;
- direct access to live Codex-owned files;
- Computer Use window selection, capture, and input authorization;
- codexRS-owned SQLite state;
- Git and terminal command boundaries;
- release artifacts and dependency-policy bypasses.

## Out of scope

- vulnerabilities in the separately installed official Codex CLI or OpenAI
  service;
- unsupported modified builds;
- issues that require exposing third-party accounts, credentials, or data;
- social engineering and denial-of-service testing against public services.

Upstream Codex issues should follow the
[OpenAI Codex security policy](https://github.com/openai/codex/security/policy).
