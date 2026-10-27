//! Host wire protocols. No command text is ever executed here.
use crate::core::{
    integration::{Adapter, AdapterResponse, HookDecision},
    model::*,
};
use serde_json::{Value, json};

pub struct HostAdapter(pub &'static str);
pub fn supported(provider: &str, event: &str) -> bool {
    match provider {
        "claude" | "codex" | "opencode" | "deepseek" => matches!(
            event,
            "PreToolUse" | "PostToolUse" | "SessionStart" | "Stop"
        ),
        "gemini" => matches!(
            event,
            "BeforeTool" | "AfterTool" | "SessionStart" | "SessionEnd"
        ),
        "cursor" => matches!(
            event,
            "beforeShellExecution"
                | "preToolUse"
                | "beforeReadFile"
                | "afterFileEdit"
                | "sessionStart"
                | "stop"
        ),
        _ => false,
    }
}
fn string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}
impl Adapter for HostAdapter {
    fn provider(&self) -> &'static str {
        self.0
    }
    fn parse(&self, payload: &str) -> Result<NormalizedEvent, String> {
        if payload.len() > 1024 * 1024 {
            return Err("hook input exceeds 1 MiB".into());
        }
        let v: Value = serde_json::from_str(payload).map_err(|_| "invalid hook JSON")?;
        if !v.is_object() {
            return Err("hook input must be an object".into());
        }
        let provider = if self.0 == "claude" && v.get("cursor_version").is_some() {
            "cursor"
        } else {
            self.0
        };
        let event = string(&v, "hook_event_name").ok_or("missing hook_event_name")?;
        if !supported(provider, &event) {
            return Err("unsupported hook event".into());
        }
        let input = v.get("tool_input").unwrap_or(&Value::Null);
        let tool = string(&v, "tool_name").unwrap_or_default();
        let stage = match event.as_str() {
            "PreToolUse"
            | "BeforeTool"
            | "beforeShellExecution"
            | "preToolUse"
            | "beforeReadFile" => EventStage::Before,
            "PostToolUse" | "AfterTool" | "afterFileEdit" => EventStage::After,
            _ => EventStage::Lifecycle,
        };
        let mut action = match tool.as_str() {
            "Bash" | "PowerShell" | "Shell" | "run_shell_command" => EventAction::ShellCommand,
            "Write" | "Edit" | "write_file" | "replace" | "apply_patch" => EventAction::FileWrite,
            "Read" | "read_file" => EventAction::FileRead,
            _ => EventAction::Unknown,
        };
        match event.as_str() {
            "beforeShellExecution" => action = EventAction::ShellCommand,
            "beforeReadFile" => action = EventAction::FileRead,
            "afterFileEdit" => action = EventAction::FileWrite,
            "SessionStart" | "sessionStart" => action = EventAction::SessionStart,
            "Stop" | "stop" | "SessionEnd" => action = EventAction::SessionStop,
            _ => (),
        }
        let command = if action == EventAction::ShellCommand {
            string(input, "command").or_else(|| string(&v, "command"))
        } else {
            None
        };
        let paths = string(input, "file_path")
            .or_else(|| string(&v, "file_path"))
            .into_iter()
            .collect();
        // apply_patch paths and Cursor Write input are intentionally not guessed from undocumented formats.
        let cwd = string(input, "dir_path")
            .or_else(|| string(input, "working_directory"))
            .or_else(|| string(&v, "cwd"))
            .or_else(|| {
                v.get("workspace_roots")?
                    .as_array()?
                    .first()?
                    .as_str()
                    .map(str::to_owned)
            });
        Ok(NormalizedEvent {
            provider: provider.into(),
            original_event: event,
            stage,
            action,
            working_directory: cwd,
            paths,
            command,
            session_id: string(&v, "session_id").or_else(|| string(&v, "conversation_id")),
            tool_name: if tool.is_empty() { None } else { Some(tool) },
        })
    }
    fn render(
        &self,
        event: &NormalizedEvent,
        decision: HookDecision,
        findings: &[Finding],
    ) -> AdapterResponse {
        let block = decision == HookDecision::Block && event.stage == EventStage::Before;
        // Deliberately omit source, paths, command text and configuration patterns from host messages.
        let mut message = findings
            .iter()
            .take(20)
            .map(|f| {
                format!(
                    "{}: {}",
                    f.rule_id,
                    if f.severity == Severity::Error {
                        "configured policy matched"
                    } else {
                        "review recommended"
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        if findings.len() > 20 {
            message.push_str(&format!("\n… and {} more", findings.len() - 20));
        }
        message = message.chars().take(1800).collect();
        let warning = decision != HookDecision::Allow && !message.is_empty();
        let provider = event.provider.as_str();
        let payload = match provider {
            "claude" | "codex" | "opencode" | "deepseek" if block => Some(
                json!({"hookSpecificOutput":{"hookEventName":event.original_event,"permissionDecision":"deny","permissionDecisionReason":message}}),
            ),
            "claude" | "codex" | "opencode" | "deepseek" if warning => {
                Some(json!({"systemMessage":message}))
            }
            "claude" | "codex" | "opencode" | "deepseek" => None,
            "gemini" if block => {
                Some(json!({"decision":"deny","reason":message,"systemMessage":message}))
            }
            "gemini" if warning => Some(json!({"systemMessage":message})),
            "gemini" => Some(json!({})),
            "cursor" if event.stage == EventStage::Before => {
                let mut v = json!({"permission":if block {"deny"} else {"allow"}});
                if warning {
                    v["user_message"] = json!(message);
                    if event.original_event != "beforeReadFile" {
                        v["agent_message"] = json!(message);
                    }
                }
                Some(v)
            }
            "cursor" => Some(json!({})),
            _ => None,
        };
        AdapterResponse {
            exit_code: 0,
            stdout: payload.map(|v| format!("{v}\n")).unwrap_or_default(),
            stderr: if provider == "cursor" && event.stage != EventStage::Before && warning {
                format!("{message}\n")
            } else {
                String::new()
            },
        }
    }
}

pub fn neutral(provider: &str, event: &str) -> AdapterResponse {
    AdapterResponse {
        exit_code: 0,
        stdout: if provider == "cursor"
            && supported(provider, event)
            && matches!(
                event,
                "beforeShellExecution" | "preToolUse" | "beforeReadFile"
            ) {
            "{\"permission\":\"allow\"}\n".into()
        } else if matches!(provider, "gemini" | "cursor") {
            "{}\n".into()
        } else {
            String::new()
        },
        stderr: String::new(),
    }
}
