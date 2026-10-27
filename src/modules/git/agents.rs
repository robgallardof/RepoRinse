//! Known AI agent identities used by `git.ai-attribution`.
//!
//! Precision beats recall: a trailer is attributed to an agent only when its address is an address no human
//! uses, or its display name *is* an agent name (optionally followed by a model/version suffix). A name that
//! merely starts with an agent word ("Claude Dupont") is a person. The full decision table lives in
//! `docs/rules/git.ai-attribution.md`; keep both in sync.

/// Display names that identify an agent on their own. Lower case, words separated by one space.
pub const AGENT_NAMES: &[&str] = &[
    "aider",
    "amazon q",
    "amazon q developer",
    "chatgpt",
    "chatgpt codex connector",
    "claude code",
    "codex",
    "codex cli",
    "copilot",
    "copilot swe agent",
    "cursor",
    "cursor agent",
    "deepseek",
    "deepseek harness",
    "opencode",
    "devin ai",
    "devin ai integration",
    "gemini cli",
    "gemini code assist",
    "github copilot",
    "google labs jules",
    "openai",
    "openai codex",
    "openhands",
    "openhands agent",
    "windsurf",
];

/// Agent names that are also ordinary human first names. Alone, they only count with an agent address or
/// no address at all; "Claude <claude@dupont.fr>" is kept.
pub const AMBIGUOUS_NAMES: &[&str] = &["claude", "devin", "gemini", "jules"];

/// Exact addresses written by agents (lower case).
pub const AGENT_EMAILS: &[&str] = &[
    "noreply@anthropic.com",
    "no-reply@anthropic.com",
    "noreply@openai.com",
    "codex@openai.com",
    "copilot@github.com",
    "cursoragent@cursor.com",
    "noreply@cursor.com",
    "noreply@aider.chat",
    "aider@aider.chat",
];

/// Domains whose `noreply`/`no-reply`/`bot` mailboxes are agent addresses.
const AGENT_VENDOR_DOMAINS: &[&str] = &[
    "anthropic.com",
    "openai.com",
    "cursor.com",
    "cursor.sh",
    "aider.chat",
    "cognition.ai",
];

/// Words of a GitHub bot login (`<login>[bot]@users.noreply.github.com`) that mark an AI agent app.
const AGENT_BOT_LOGIN_WORDS: &[&str] = &[
    "claude",
    "copilot",
    "codex",
    "openai",
    "chatgpt",
    "gemini",
    "cursor",
    "devin",
    "jules",
    "aider",
    "openhands",
    "anthropic",
];

/// Words allowed after an agent name without turning it into a person ("Claude Opus 4.5", "aider (gpt-4o)").
const MODEL_WORDS: &[&str] = &[
    "agent",
    "ai",
    "anthropic",
    "assistant",
    "bot",
    "chatgpt",
    "claude",
    "cli",
    "code",
    "codex",
    "exp",
    "experimental",
    "flash",
    "gemini",
    "github",
    "google",
    "gpt",
    "haiku",
    "high",
    "instant",
    "latest",
    "lite",
    "low",
    "max",
    "medium",
    "mini",
    "model",
    "nano",
    "openai",
    "opus",
    "preview",
    "pro",
    "sonnet",
    "thinking",
    "turbo",
    "ultra",
];

/// How a co-author display name relates to the agent lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameMatch {
    /// Not an agent name.
    None,
    /// An agent name, alone or followed only by model/version words.
    Agent,
    /// A bare agent name that is also a human first name ("Claude", "Devin").
    Ambiguous,
    /// An agent word followed by something else ("Claude Dupont"): a person.
    HumanLike,
}

/// Lower case, `-`/`_`/`(`/`)`/`/` turned into spaces, `[bot]` suffix noted, whitespace collapsed.
fn tokens(name: &str) -> (Vec<String>, bool) {
    let mut lower = name.trim().to_lowercase();
    let bot = lower.ends_with("[bot]");
    if bot {
        lower.truncate(lower.len() - "[bot]".len());
    }
    let cleaned: String = lower
        .chars()
        .map(|c| match c {
            '-' | '_' | '(' | ')' | '/' | ',' => ' ',
            c => c,
        })
        .collect();
    (
        cleaned.split_whitespace().map(str::to_string).collect(),
        bot,
    )
}

fn is_model_word(token: &str) -> bool {
    MODEL_WORDS.contains(&token) || token.chars().any(|c| c.is_ascii_digit())
}

/// Classifies a display name. `extra` are user-configured agent names (treated as unambiguous).
pub fn classify_name(name: &str, extra: &[String]) -> NameMatch {
    let (words, bot) = tokens(name);
    if words.is_empty() {
        return NameMatch::None;
    }
    let candidates = AGENT_NAMES
        .iter()
        .map(|n| (n.to_string(), false))
        .chain(AMBIGUOUS_NAMES.iter().map(|n| (n.to_string(), true)))
        .chain(extra.iter().map(|n| (tokens(n).0.join(" "), false)));

    // Longest matching agent name wins ("claude code" before "claude").
    let mut best: Option<(usize, bool)> = None;
    for (candidate, ambiguous) in candidates {
        let parts: Vec<&str> = candidate.split(' ').filter(|p| !p.is_empty()).collect();
        if parts.is_empty() || parts.len() > words.len() {
            continue;
        }
        if parts.iter().zip(&words).all(|(p, w)| p == w)
            && best.is_none_or(|(len, _)| parts.len() > len)
        {
            best = Some((parts.len(), ambiguous));
        }
    }
    let Some((len, ambiguous)) = best else {
        return NameMatch::None;
    };
    let rest = &words[len..];
    if rest.is_empty() {
        if ambiguous && !bot {
            NameMatch::Ambiguous
        } else {
            NameMatch::Agent
        }
    } else if rest.iter().all(|w| is_model_word(w)) {
        NameMatch::Agent
    } else {
        NameMatch::HumanLike
    }
}

/// True when `email` is an address only agents use. `extra` holds configured addresses or `@domain` suffixes.
pub fn is_agent_email(email: &str, extra: &[String]) -> bool {
    let email = email.trim().to_lowercase();
    let Some((local, domain)) = email.rsplit_once('@') else {
        return false;
    };
    if AGENT_EMAILS.contains(&email.as_str()) {
        return true;
    }
    if AGENT_VENDOR_DOMAINS.contains(&domain) && matches!(local, "noreply" | "no-reply" | "bot") {
        return true;
    }
    if domain == "users.noreply.github.com" {
        // `<id>+<login>@users.noreply.github.com`; humans have the same shape, so only bot apps with an
        // agent word and the Copilot coding agent account count.
        let login = local.rsplit_once('+').map_or(local, |(_, l)| l);
        if login == "copilot" {
            return true;
        }
        if let Some(stem) = login.strip_suffix("[bot]") {
            return stem
                .split(['-', '_'])
                .any(|w| AGENT_BOT_LOGIN_WORDS.contains(&w));
        }
        return false;
    }
    extra.iter().any(|e| {
        let e = e.trim().to_lowercase();
        match e.strip_prefix('@') {
            Some(d) => domain == d,
            None => email == e,
        }
    })
}

/// Why a co-author is attributed to an agent, or `None` to keep it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoAuthorDecision {
    Remove(&'static str),
    Keep,
    /// Kept on purpose: a bare human-looking name ("Claude") with a personal address.
    KeepAmbiguous,
}

/// The decision table of `docs/rules/git.ai-attribution.md`.
pub fn decide(
    name: &str,
    email: Option<&str>,
    extra_names: &[String],
    extra_emails: &[String],
) -> CoAuthorDecision {
    if email.is_some_and(|e| is_agent_email(e, extra_emails)) {
        return CoAuthorDecision::Remove("known agent address");
    }
    match classify_name(name, extra_names) {
        NameMatch::Agent => CoAuthorDecision::Remove("known agent name"),
        NameMatch::Ambiguous if email.is_none_or(|e| e.trim().is_empty()) => {
            CoAuthorDecision::Remove("agent name without an address")
        }
        NameMatch::Ambiguous => CoAuthorDecision::KeepAmbiguous,
        NameMatch::HumanLike | NameMatch::None => CoAuthorDecision::Keep,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(name: &str, email: Option<&str>) -> CoAuthorDecision {
        decide(name, email, &[], &[])
    }

    #[test]
    fn agent_names_with_model_suffixes() {
        for name in [
            "Claude Code",
            "Claude Opus 4.5",
            "Claude Sonnet 4",
            "claude-3-7-sonnet",
            "Claude 3.5 Haiku",
            "ChatGPT",
            "OpenAI Codex",
            "GitHub Copilot",
            "Copilot",
            "Gemini CLI",
            "Gemini 2.5 Pro",
            "Cursor Agent",
            "aider (gpt-4o)",
            "Devin AI",
            "claude[bot]",
        ] {
            assert_eq!(classify_name(name, &[]), NameMatch::Agent, "{name}");
        }
    }

    #[test]
    fn people_are_not_agents() {
        assert_eq!(classify_name("Claude Dupont", &[]), NameMatch::HumanLike);
        assert_eq!(classify_name("Devin Booker", &[]), NameMatch::HumanLike);
        assert_eq!(classify_name("Jean-Claude Van Damme", &[]), NameMatch::None);
        assert_eq!(classify_name("Ana", &[]), NameMatch::None);
        assert_eq!(classify_name("Claude", &[]), NameMatch::Ambiguous);
    }

    #[test]
    fn decision_table() {
        use CoAuthorDecision::*;
        assert_eq!(
            d("Claude", Some("noreply@anthropic.com")),
            Remove("known agent address")
        );
        assert_eq!(d("Claude", None), Remove("agent name without an address"));
        assert_eq!(d("Claude", Some("claude@dupont.fr")), KeepAmbiguous);
        assert_eq!(d("Claude Dupont", Some("claude@dupont.fr")), Keep);
        assert_eq!(
            d("Claude Dupont", Some("noreply@anthropic.com")),
            Remove("known agent address")
        );
        assert_eq!(
            d("Claude Opus 4.5", Some("claude@dupont.fr")),
            Remove("known agent name")
        );
        assert_eq!(
            d("Copilot", Some("copilot@github.com")),
            Remove("known agent address")
        );
        assert_eq!(
            d(
                "Copilot",
                Some("198982749+Copilot@users.noreply.github.com")
            ),
            Remove("known agent address")
        );
        assert_eq!(
            d("x", Some("41898282+claude[bot]@users.noreply.github.com")),
            Remove("known agent address")
        );
        assert_eq!(
            d(
                "dependabot[bot]",
                Some("49699333+dependabot[bot]@users.noreply.github.com")
            ),
            Keep
        );
        assert_eq!(
            d("Claude", Some("1234+claude@users.noreply.github.com")),
            KeepAmbiguous
        );
        assert_eq!(d("Jane", Some("jane@anthropic.com")), Keep);
        assert_eq!(d("Ana", Some("ana@example.com")), Keep);
    }

    #[test]
    fn configured_identities() {
        let names = vec!["Acme Bot".to_string()];
        let emails = vec!["@agents.acme.dev".to_string(), "x@y.z".to_string()];
        assert_eq!(classify_name("acme-bot", &names), NameMatch::Agent);
        assert!(is_agent_email("build@AGENTS.acme.dev", &emails));
        assert!(is_agent_email("x@y.z", &emails));
        assert!(!is_agent_email("x@acme.dev", &emails));
    }
}
