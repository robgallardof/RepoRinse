# Docker (optional)

RepoRinse is a single native binary; Docker is only a convenience for CI runners or machines without Rust.
The image contains the `reporinse` binary, `git` and nothing else, runs as UID 10001 and needs no network
(RepoRinse never uses one at runtime).

```sh
docker build -t reporinse:local .
```

## What runs in a container

| Command | In Docker | Notes |
|---|---|---|
| `check --files …`, `check --staged`, `check --message …` | Yes | Mount the repository at `/repo` |
| `fix --files …`, `fix --staged`, `fix --message …` | Yes | Needs a writable mount; on Linux pass your UID so ownership is kept |
| `modules`, `rule`, `config`, `status`, `doctor` | Yes | Read-only |
| `install` / `uninstall` (Git hooks, agent integrations) | **No** | A hook or agent config written from a container would reference `/usr/local/bin/reporinse`, which does not exist on the host. Install natively (README options A/B) |

```sh
docker run --rm --network none -v "$PWD:/repo" reporinse:local check --staged
docker run --rm --network none -v "$PWD:/repo" reporinse:local --json check --files src/main.rs
docker run --rm --network none --user "$(id -u):$(id -g)" -v "$PWD:/repo" reporinse:local fix --message .git/COMMIT_EDITMSG
```

PowerShell: replace `$PWD` with `${PWD}` and omit `--user` (Docker Desktop maps ownership itself).

Inside the image, `/repo` is listed in the system Git config as a `safe.directory`, because a bind-mounted repository
usually belongs to another UID. Nothing is written to the host's Git configuration.

## Compose

`compose.yaml` defines a one-shot service with a read-only container filesystem, no capabilities and
`network_mode: none`:

```sh
REPORINSE_REPO=/path/to/repo REPORINSE_USER="$(id -u):$(id -g)" docker compose run --rm reporinse check --staged
```

| Variable | Default | Purpose |
|---|---|---|
| `REPORINSE_REPO` | `.` | Repository mounted at `/repo` |
| `REPORINSE_USER` | `10001:10001` | Runtime UID:GID |

## In CI

```yaml
- run: docker build -t reporinse:local .
- run: docker run --rm --network none -v "$PWD:/repo" reporinse:local check --files $(git diff --name-only origin/main...)
```

Exit codes are the same as the native binary (see README → Exit codes).

## Verify

```sh
python scripts/test_docker.py
```

Builds the image and, against a throw-away Git repository with an isolated `HOME`, checks: non-root user,
`check --files` JSON, `check --staged` on a mount owned by another UID, `fix --message` writing back to the host
file, and the Compose service with networking disabled.

### Verification — 2026-10-26

Executed on Windows with Docker Desktop Linux containers: image build, UID 10001,
JSON file findings, staged Git inspection, message fix on a temporary mount and
Compose with networking disabled. All passed. `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings` and `cargo test` also passed.
Host hook/agent installation was NOT EXECUTED in this container verification;
native integration tests exercised isolated temporary installations instead.
