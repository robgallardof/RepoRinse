use crate::core::{
    model::*,
    rule::{Rule, RuleContext},
};
pub struct ChangeRule(pub bool);
static SCOPE: RuleMetadata = RuleMetadata {
    id: "git.diff-scope",
    version: "1.0.0",
    category: "git",
    summary: "Report change sets exceeding configured file or line limits.",
    risk: RiskLevel::Low,
    priority: Priority::P1,
    availability: Availability::Implemented,
    modes: &[RuleMode::Warn, RuleMode::Block],
    default_mode: RuleMode::Warn,
    targets: &[TargetKind::ChangeSet],
    providers: &["git", "cli"],
};
static DEPENDENCIES: RuleMetadata = RuleMetadata {
    id: "git.dependency-watch",
    summary: "Report changed dependency manifests and lockfiles for review.",
    ..SCOPE
};
impl Rule for ChangeRule {
    fn metadata(&self) -> &RuleMetadata {
        if self.0 { &SCOPE } else { &DEPENDENCIES }
    }
    fn validate_options(&self, options: &toml::Table) -> Vec<String> {
        options
            .iter()
            .filter_map(|(k, v)| {
                if !self.0 || !matches!(k.as_str(), "max_files" | "max_lines") {
                    Some(format!("unknown option '{k}'"))
                } else if v.as_integer().is_none_or(|i| i < 1) {
                    Some(format!("{k} must be a positive integer"))
                } else {
                    None
                }
            })
            .collect()
    }
    fn analyze(&self, ctx: &RuleContext<'_>) -> RuleResult {
        let mut result = RuleResult::default();
        let Some(changes) = ctx.changes else {
            return result;
        };
        if ctx.settings.mode == RuleMode::Off {
            return result;
        }
        if self.0 {
            let lines = changes.paths.iter().fold(0usize, |n, p| {
                n.saturating_add(p.added_lines)
                    .saturating_add(p.deleted_lines)
            });
            if changes.paths.len() > ctx.settings.int("max_files").unwrap_or(25) as usize
                || lines > ctx.settings.int("max_lines").unwrap_or(500) as usize
            {
                result.findings.push(Finding::new(
                    SCOPE.id,
                    ctx.settings.severity(),
                    format!(
                        "Large change set: {} files, {lines} changed lines; review scope",
                        changes.paths.len()
                    ),
                ));
            }
        } else {
            for p in &changes.paths {
                let name = p.path.rsplit('/').next().unwrap_or("");
                if matches!(
                    name,
                    "Cargo.toml"
                        | "Cargo.lock"
                        | "package.json"
                        | "package-lock.json"
                        | "yarn.lock"
                        | "pnpm-lock.yaml"
                        | "pyproject.toml"
                        | "requirements.txt"
                        | "poetry.lock"
                        | "uv.lock"
                        | "go.mod"
                        | "go.sum"
                        | "Gemfile"
                        | "Gemfile.lock"
                        | "composer.json"
                        | "composer.lock"
                ) || name.ends_with(".csproj")
                {
                    result.findings.push(
                        Finding::new(
                            DEPENDENCIES.id,
                            ctx.settings.severity(),
                            "Dependency manifest or lockfile changed; review intent and versions",
                        )
                        .at(&p.path, None, None),
                    );
                }
            }
        }
        result
    }
}
