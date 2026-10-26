//! POSIX `sh` hook shims. They are tiny dispatchers: run the hook that was there before (if any), then
//! `reporinse hook git <name>`. Always LF line endings so Git for Windows' `sh` runs them.

use crate::core::integration::InstallScope;

/// First line after the shebang; identifies files RepoRinse wrote even without a manifest.
pub const MARKER: &str = "# reporinse-managed-hook";

/// Suffix of a foreign hook RepoRinse moved aside and runs first.
pub const CHAINED_SUFFIX: &str = ".reporinse-chained";

/// Environment variable set while a RepoRinse shim runs; nested shims skip the RepoRinse step (they still run
/// their chained hooks) so a hook chain never runs the same rules twice or recurses.
pub const GUARD_ENV: &str = "REPORINSE_HOOK_ACTIVE";

/// Hooks RepoRinse knows how to serve.
pub const COMMIT_MSG: &str = "commit-msg";
pub const PRE_COMMIT: &str = "pre-commit";

/// Inputs of one rendered shim.
#[derive(Debug, Clone)]
pub struct ShimSpec<'a> {
    pub hook: &'a str,
    pub scope: InstallScope,
    /// Absolute path of the binary recorded at install time (forward slashes).
    pub binary: &'a str,
    /// Modules the hook runs (`--only`).
    pub modules: &'a [String],
}

/// Single-quotes `s` for `sh`.
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

pub fn is_managed(content: &[u8]) -> bool {
    content
        .split(|b| *b == b'\n')
        .take(3)
        .any(|line| line.starts_with(MARKER.as_bytes()))
}

pub fn render(spec: &ShimSpec<'_>) -> String {
    let scope = match spec.scope {
        InstallScope::Repo => "repo",
        InstallScope::User => "user",
    };
    let mut s = String::new();
    s.push_str("#!/bin/sh\n");
    s.push_str(&format!("{MARKER} v1 hook={} scope={scope}\n", spec.hook));
    s.push_str("# Written by `reporinse install`. `reporinse uninstall` removes it and restores any hook it chained.\n");
    s.push_str("# Local edits are reported as drift. Docs: docs/integrations/git.md\n");
    s.push_str(&format!("hook_name={}\n", sh_quote(spec.hook)));
    s.push_str(&format!("reporinse_bin={}\n", sh_quote(spec.binary)));
    s.push_str(&format!(
        "reporinse_only={}\n",
        sh_quote(&spec.modules.join(","))
    ));
    s.push_str(
        r#"already_active=${REPORINSE_HOOK_ACTIVE:-}
REPORINSE_HOOK_ACTIVE=1
export REPORINSE_HOOK_ACTIVE

# 1. The hook that was here before RepoRinse runs first, with the same arguments and stdin.
#    Its failure stops the commit and RepoRinse does not touch anything afterwards.
chained="$(dirname -- "$0")/$hook_name.reporinse-chained"
if [ -f "$chained" ] && [ -x "$chained" ]; then
    # Plain sh/bash hooks are run by their shell with $0 set to this hook's path, because tools such as
    # Husky find their own files from "$0". Anything else is executed directly.
    first_line=
    IFS= read -r first_line < "$chained" || true
    case "$first_line" in
        '#!/bin/sh' | '#!/usr/bin/env sh') runner=sh ;;
        '#!/bin/bash' | '#!/usr/bin/env bash') runner=bash ;;
        *) runner= ;;
    esac
    if [ -n "$runner" ]; then
        "$runner" -c 'c=$1; shift; . "$c"' "$0" "$chained" "$@" || exit $?
    else
        "$chained" "$@" || exit $?
    fi
fi
"#,
    );
    if spec.scope == InstallScope::User {
        s.push_str(
            r#"
# 2. A global core.hooksPath hides the repository's own hooks from Git, so run .git/hooks/<name> here.
if git_common=$(git rev-parse --git-common-dir 2>/dev/null); then
    repo_hook="$git_common/hooks/$hook_name"
    if [ -f "$repo_hook" ] && [ -x "$repo_hook" ]; then
        "$repo_hook" "$@" || exit $?
    fi
fi
"#,
        );
    }
    s.push_str(
        r#"
# An outer RepoRinse shim already runs the rules for this hook.
[ -n "$already_active" ] && exit 0

if [ -x "$reporinse_bin" ]; then
    bin=$reporinse_bin
elif command -v reporinse >/dev/null 2>&1; then
    bin=reporinse
else
    # Fail open: cleanup rules must never make commits impossible.
    echo "reporinse: binary not found at '$reporinse_bin' nor on PATH; $hook_name checks skipped." >&2
    echo "reporinse: reinstall it, or remove this hook with 'reporinse uninstall --provider git'." >&2
    exit 0
fi
exec "$bin" hook --only "$reporinse_only" git "$hook_name" "$@"
"#,
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_and_markers() {
        assert_eq!(sh_quote("it's"), r"'it'\''s'");
        let modules = vec!["git.ai-attribution".to_string()];
        let text = render(&ShimSpec {
            hook: COMMIT_MSG,
            scope: InstallScope::Repo,
            binary: "C:/Program Files/ñ dir/reporinse.exe",
            modules: &modules,
        });
        assert!(text.starts_with("#!/bin/sh\n# reporinse-managed-hook v1"));
        assert!(!text.contains('\r'));
        assert!(text.contains("reporinse_bin='C:/Program Files/ñ dir/reporinse.exe'"));
        assert!(text.contains(CHAINED_SUFFIX));
        assert!(text.contains(GUARD_ENV));
        assert!(!text.contains("git_common"));
        assert!(is_managed(text.as_bytes()));
        assert!(!is_managed(b"#!/bin/sh\necho hi\n"));
    }

    #[test]
    fn user_scope_runs_repo_hooks() {
        let text = render(&ShimSpec {
            hook: PRE_COMMIT,
            scope: InstallScope::User,
            binary: "/usr/bin/reporinse",
            modules: &[],
        });
        assert!(text.contains("git rev-parse --git-common-dir"));
    }
}
