"""Real test of the optional Docker image and Compose service (standard library only).

Builds the image and runs it against a throw-away Git repository: `check --files`
(JSON findings), `check --staged` (Git inside the container on a bind mount owned by
another UID), `fix --message` (writes back to the mounted file) and the Compose
service with networking disabled. Host `git` runs with an isolated HOME and no
global/system config. Temporary files and containers are removed on exit.

    python scripts/test_docker.py
"""

import json
import os
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
IMAGE = "reporinse:local"
TRAILER = "Co-Authored-By: Claude <noreply@anthropic.com>"


def run(*args, env=None, check=True, cwd=None):
    out = subprocess.run(args, env=env, cwd=cwd, text=True, capture_output=True, timeout=900, encoding="utf-8")
    if check and out.returncode:
        raise RuntimeError(f"failed {' '.join(map(str, args))}:\n{out.stdout}\n{out.stderr}")
    return out


def main():
    run("docker", "build", "-t", IMAGE, str(ROOT))
    print("ok  build")
    uid = run("docker", "run", "--rm", "--entrypoint", "id", IMAGE, "-u").stdout.strip()
    assert uid == "10001", uid
    print("ok  runs as non-root (uid 10001)")

    with tempfile.TemporaryDirectory(prefix="reporinse-docker-") as tmp:
        home = Path(tmp) / "home"
        repo = Path(tmp) / "repo"
        home.mkdir()
        repo.mkdir()
        git_env = dict(os.environ, HOME=str(home), XDG_CONFIG_HOME=str(home), GIT_CONFIG_GLOBAL=str(home / "none"),
                       GIT_CONFIG_NOSYSTEM="1", GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@example.invalid",
                       GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@example.invalid")
        run("git", "init", "-q", cwd=repo, env=git_env)
        (repo / "a.txt").write_bytes(b"hello   \nworld\n")
        (repo / "msg.txt").write_bytes(f"feat: add parser\n\n{TRAILER}\n".encode())
        run("git", "add", "a.txt", cwd=repo, env=git_env)
        for path in (repo, *repo.rglob("*")):
            os.chmod(path, 0o777 if path.is_dir() else 0o666)
        user = f"{os.getuid()}:{os.getgid()}" if hasattr(os, "getuid") else "10001:10001"
        base = ["docker", "run", "--rm", "--user", user, "--network", "none", "-v", f"{repo}:/repo", IMAGE]

        out = run(*base, "--json", "check", "--files", "a.txt").stdout
        rules = {f["rule_id"] for f in json.loads(out)["findings"]}
        assert "text.whitespace" in rules, out
        print("ok  check --files: findings as JSON")

        out = run(*base, "--json", "check", "--staged").stdout
        assert any(f["path"] == "a.txt" for f in json.loads(out)["findings"]), out
        print("ok  check --staged: git works on a mount owned by another uid")

        run(*base, "fix", "--message", "msg.txt")
        fixed = (repo / "msg.txt").read_text(encoding="utf-8")
        assert TRAILER not in fixed and fixed.startswith("feat: add parser"), fixed
        print("ok  fix --message: AI trailer removed in the mounted file")

        env = dict(os.environ, REPORINSE_REPO=str(repo), REPORINSE_USER=user)
        compose = ["docker", "compose", "-p", "reporinse-smoke", "-f", str(ROOT / "compose.yaml")]
        out = run(*compose, "run", "--rm", "reporinse", "check", "--files", "a.txt", "--format", "plain", env=env).stdout
        assert "text.whitespace" in out, out
        print("ok  compose run: read-only container, no network")
        run(*compose, "down", "--remove-orphans", env=env, check=False)
    print("all ok")


if __name__ == "__main__":
    main()
