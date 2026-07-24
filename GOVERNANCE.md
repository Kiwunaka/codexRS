# Governance

codexRS uses lightweight maintainer-led governance.

## Roles

- Contributors submit issues, discussions, documentation, tests, and code.
- Maintainers triage work, review pull requests, protect release boundaries,
  and publish releases.
- The repository owner appoints or removes maintainers based on sustained,
  constructive contributions and project needs.

## Decisions

Routine changes are decided through pull-request review. Changes to the
app-server contract, trust boundary, storage schema, platform support,
dependencies, licensing, or release policy begin with a public issue or
discussion.

Maintainers seek practical consensus. When consensus is not reached, the
repository owner makes the final call and records the decision in the issue or
pull request.

## Releases

Maintainers may publish a candidate only after the documented release gates
pass. Stable releases also require supported-platform smoke tests and no known
release-blocking security issue.

No role grants access to contributor credentials, private data, or production
systems.
