# Security policy

Report vulnerabilities privately through the repository's GitHub Security Advisories when enabled,
or contact the maintainer through the contact information on their GitHub profile. Do not post secrets
or a working exploit in a public issue. This project does not claim a guaranteed response time.

RepoRinse is local and deterministic. It never executes command strings from agent payloads.
Agent guards match explicit glob policies; they are not a shell parser, sandbox or complete enforcement boundary.
Hooks may be bypassed by the host, disabled by users, or fail open. Keep normal approval and sandbox controls.

Fixes refuse symlinks/reparse points, oversized/unsupported files, stale source and overlapping edit plans.
Staged fixes lock the index, refuse partial staging, use a temporary index, and attempt worktree rollback on failure.
There is no filesystem lock against ordinary concurrent editors: do not edit files while fixing them.
Agent settings have manifests, stale-file checks and a lock; uninstall refuses drift unless explicitly forced.

See [threat model](docs/security/threat-model.md) and [verification report](docs/development/verification.md)
for tested coverage and limitations. No source scanning tool can prove an arbitrary shell command is safe.
