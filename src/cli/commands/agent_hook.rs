use super::hook::HookArgs;
use crate::{
    cli::output::Output,
    core::{
        config, engine,
        integration::{Adapter, HookDecision},
        model::EventStage,
    },
    integrations::agent_adapter::{self, HostAdapter},
};
use std::{io::Read, path::PathBuf};
pub fn run(args: &HookArgs, out: &mut Output) -> i32 {
    let provider = args.provider.to_lowercase();
    let mut neutral = agent_adapter::neutral(&provider, &args.event);
    let result = (|| -> Result<_, String> {
        if std::env::var("REPORINSE_HOOK_DEPTH")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
            >= 1
        {
            return Ok(neutral.clone());
        }
        let mut payload = String::new();
        std::io::stdin()
            .take(1024 * 1024 + 1)
            .read_to_string(&mut payload)
            .map_err(|_| "cannot read UTF-8 hook input")?;
        let adapter =
            crate::integrations::find_adapter(&provider).ok_or("unsupported hook provider")?;
        let event = adapter.parse(&payload)?;
        neutral = agent_adapter::neutral(&event.provider, &event.original_event);
        if event.provider == provider && event.original_event != args.event {
            return Err("hook event does not match entry point".into());
        }
        let cwd = event
            .working_directory
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or(std::env::current_dir().map_err(|_| "cannot resolve working directory")?);
        let top = crate::integrations::git::command::Git::at(&cwd)
            .env("REPORINSE_HOOK_DEPTH", "1")
            .text(&["rev-parse", "--show-toplevel"])
            .map(PathBuf::from)
            .unwrap_or(cwd);
        let registry = crate::modules::registry();
        for id in &args.only {
            if !registry.contains(id) {
                return Err("unknown module in hook; reinstall integration".into());
            }
        }
        let config = config::load(&registry, Some(&top), &config::CliOverrides::default())
            .map_err(|_| "invalid configuration; run reporinse config validate")?;
        if !config
            .providers
            .get(&event.provider)
            .copied()
            .unwrap_or(true)
        {
            return Ok(neutral.clone());
        }
        let report = engine::run_event(&registry, &config, &event, &args.only);
        let decision = if report.blocked() && event.stage == EventStage::Before {
            HookDecision::Block
        } else if !report.findings.is_empty() {
            HookDecision::Warn
        } else {
            HookDecision::Allow
        };
        Ok(HostAdapter(if event.provider == "cursor" {
            "cursor"
        } else {
            adapter.provider()
        })
        .render(&event, decision, &report.findings))
    })();
    let response = match result {
        Ok(response) => response,
        Err(_) => {
            out.raw_err("reporinse: hook skipped because input or configuration was invalid; run config validate and doctor.\n");
            neutral
        }
    };
    out.raw(response.stdout);
    out.raw_err(response.stderr);
    response.exit_code
}
