mod support;
use std::{
    io::Write,
    process::{Command, Stdio},
};
use support::*;
fn invoke(
    sb: &Sandbox,
    repo: &TestRepo<'_>,
    host: &str,
    event: &str,
    payload: &str,
) -> std::process::Output {
    let mut command = Command::new(reporinse_bin());
    command
        .current_dir(&repo.dir)
        .args(["hook", host, event])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = sb.isolate(&mut command).spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}
#[test]
fn cli_blocks_configured_actions_and_does_not_execute_commands() {
    let sb = Sandbox::new();
    let repo = sb.repo("agent protocols");
    repo.write(
        ".reporinse.toml",
        "version=1\n[rules.\"agent.command-guard\"]\nmode=\"block\"\ndeny=[\"*never-run-this*\"]\n",
    );
    for (host, event, tool) in [
        ("claude", "PreToolUse", "Bash"),
        ("codex", "PreToolUse", "Bash"),
        ("gemini", "BeforeTool", "run_shell_command"),
        ("cursor", "beforeShellExecution", "Shell"),
    ] {
        let payload=serde_json::json!({"hook_event_name":event,"cwd":repo.dir,"tool_name":tool,"tool_input":{"command":"never-run-this"},"command":"never-run-this"}).to_string();
        let output = invoke(&sb, &repo, host, event, &payload);
        assert_code(&output, 0);
        let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            v["decision"] == "deny"
                || v["permission"] == "deny"
                || v["hookSpecificOutput"]["permissionDecision"] == "deny"
        );
        let malformed = invoke(&sb, &repo, host, event, "{");
        assert_code(&malformed, 0);
        if host == "cursor" {
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&malformed.stdout).unwrap()["permission"],
                "allow"
            );
        }
    }
}
#[test]
fn installed_handlers_execute_with_paths_containing_spaces_and_unicode() {
    let sb = Sandbox::new();
    let repo = sb.repo("installed ñá");
    let copied_dir = sb.path().join("bin ñ space's $x");
    std::fs::create_dir(&copied_dir).unwrap();
    let copied_binary = copied_dir.join(if cfg!(windows) {
        "reporinse.exe"
    } else {
        "reporinse"
    });
    std::fs::copy(reporinse_bin(), &copied_binary).unwrap();
    support::make_executable(&copied_binary);
    repo.write(
        ".reporinse.toml",
        "version=1\n[rules.\"agent.command-guard\"]\nmode=\"block\"\ndeny=[\"*blocked*\"]\n",
    );
    for (host, event, tool) in [
        ("claude", "PreToolUse", "Bash"),
        ("codex", "PreToolUse", "Bash"),
        ("gemini", "BeforeTool", "run_shell_command"),
        ("cursor", "beforeShellExecution", "Shell"),
    ] {
        assert_code(
            &sb.reporinse_with(
                &copied_binary,
                &repo.dir,
                &["install", "--provider", host, "--all", "--yes"],
            ),
            0,
        );
        let filename = if matches!(host, "claude" | "gemini") {
            "settings.json"
        } else {
            "hooks.json"
        };
        let value: serde_json::Value = serde_json::from_slice(
            &std::fs::read(repo.path(&format!(".{host}/{filename}"))).unwrap(),
        )
        .unwrap();
        let entry = &value["hooks"][event][0];
        let handler = if host == "cursor" {
            entry
        } else {
            &entry["hooks"][0]
        };
        let mut command = if host == "claude" {
            let mut c = Command::new(handler["command"].as_str().unwrap());
            for arg in handler["args"].as_array().unwrap() {
                c.arg(arg.as_str().unwrap());
            }
            c
        } else if cfg!(windows) {
            let mut c = Command::new("cmd.exe");
            c.args([
                "/D",
                "/S",
                "/C",
                handler
                    .get("commandWindows")
                    .unwrap_or(&handler["command"])
                    .as_str()
                    .unwrap(),
            ]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", handler["command"].as_str().unwrap()]);
            c
        };
        command
            .current_dir(&repo.dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let payload=serde_json::json!({"hook_event_name":event,"cwd":repo.dir,"tool_name":tool,"tool_input":{"command":"blocked"},"command":"blocked"}).to_string();
        let mut child = sb.isolate(&mut command).spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(payload.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert_code(&output, 0);
        let v: serde_json::Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|_| panic!("{host}: {} / {}", stdout(&output), stderr(&output)));
        assert!(
            v["decision"] == "deny"
                || v["permission"] == "deny"
                || v["hookSpecificOutput"]["permissionDecision"] == "deny",
            "{host}: {v}"
        );
    }
}
