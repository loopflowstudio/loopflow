//! Bound implicit launch context while retaining the complete source on disk.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::engine::config::Config;
use crate::engine::error::CoreError;
use crate::engine::prompt::{count_tokens, Document, DocumentSource, PromptComponents};
use crate::trace::{ContextAssetKind, ContextDecision, ContextDecisionKind, ContextScope};

/// Keys in the existing `context_budgets` configuration block.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BudgetKey {
    MemoryTokens,
    MemoryBytes,
    ScratchTokens,
    ScratchBytes,
    GoalTokens,
    GoalBytes,
    InputTokens,
    InputBytes,
}

impl BudgetKey {
    pub const ALL: [Self; 8] = [
        Self::MemoryTokens,
        Self::MemoryBytes,
        Self::ScratchTokens,
        Self::ScratchBytes,
        Self::GoalTokens,
        Self::GoalBytes,
        Self::InputTokens,
        Self::InputBytes,
    ];

    pub fn default_limit(self) -> usize {
        match self {
            Self::MemoryTokens | Self::ScratchTokens | Self::GoalTokens => 16_000,
            Self::InputTokens => 64_000,
            Self::MemoryBytes | Self::ScratchBytes | Self::GoalBytes => 128 * 1024,
            Self::InputBytes => 512 * 1024,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::MemoryTokens => "memory_tokens",
            Self::MemoryBytes => "memory_bytes",
            Self::ScratchTokens => "scratch_tokens",
            Self::ScratchBytes => "scratch_bytes",
            Self::GoalTokens => "goal_tokens",
            Self::GoalBytes => "goal_bytes",
            Self::InputTokens => "input_tokens",
            Self::InputBytes => "input_bytes",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BudgetLimit {
    pub value: usize,
    pub source: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ContextBudgets(BTreeMap<BudgetKey, BudgetLimit>);

impl ContextBudgets {
    pub fn resolve(config: &Config, repo: &Path, wave: Option<&str>) -> Result<Self, CoreError> {
        let mut limits: BTreeMap<_, _> = BudgetKey::ALL
            .into_iter()
            .map(|key| {
                let value = config
                    .context_budgets
                    .get(&key)
                    .copied()
                    .unwrap_or(key.default_limit());
                let source = config
                    .context_budget_sources
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| {
                        if config.context_budgets.contains_key(&key) {
                            "provided config"
                        } else {
                            "default"
                        }
                        .into()
                    });
                (key, BudgetLimit { value, source })
            })
            .collect();
        if let Some(wave) = wave {
            let wave_config = crate::work::wave::config::try_read_wave_config(repo, wave)
                .map_err(|err| CoreError::ExecutionFailed(err.to_string()))?;
            if let Some(config) = wave_config {
                for (key, value) in config.context_budgets {
                    limits.insert(
                        key,
                        BudgetLimit {
                            value,
                            source: repo
                                .join(format!("wave/{wave}/GOAL.md"))
                                .display()
                                .to_string(),
                        },
                    );
                }
            }
        }
        for (key, limit) in &limits {
            if limit.value == 0 {
                return Err(CoreError::ExecutionFailed(format!(
                    "context_budgets.{} must be positive ({})",
                    key.name(),
                    limit.source
                )));
            }
        }
        Ok(Self(limits))
    }

    pub fn limit(&self, key: BudgetKey) -> usize {
        self.0[&key].value
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ContextUsage {
    pub source: String,
    pub original_tokens: usize,
    pub original_bytes: usize,
    pub submitted_tokens: usize,
    pub submitted_bytes: usize,
    pub token_limit: usize,
    pub byte_limit: usize,
}

fn tokens_in(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        count_tokens(text)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ContextBudgetReport {
    pub budgets: ContextBudgets,
    pub usage: Vec<ContextUsage>,
}

impl ContextBudgetReport {
    fn bound_source(
        &mut self,
        source: &str,
        original: &str,
        tokens: BudgetKey,
        bytes: BudgetKey,
        repo_root: &Path,
    ) -> Result<String, CoreError> {
        let bounded = bound_source(
            original,
            self.budgets.limit(tokens),
            self.budgets.limit(bytes),
            repo_root,
        )?;
        self.usage.push(ContextUsage {
            source: source.into(),
            original_tokens: tokens_in(original),
            original_bytes: original.len(),
            submitted_tokens: tokens_in(&bounded),
            submitted_bytes: bounded.len(),
            token_limit: self.budgets.limit(tokens),
            byte_limit: self.budgets.limit(bytes),
        });
        Ok(bounded)
    }

    pub(crate) fn measure_input(
        &mut self,
        original_system: &str,
        original_task: &str,
        system: &str,
        task: &str,
    ) {
        self.usage
            .retain(|usage| usage.source != "total assembled input");
        self.usage.push(ContextUsage {
            source: "total assembled input".into(),
            original_tokens: tokens_in(original_system) + tokens_in(original_task),
            original_bytes: original_system.len() + original_task.len(),
            submitted_tokens: tokens_in(system) + tokens_in(task),
            submitted_bytes: system.len() + task.len(),
            token_limit: self.budgets.limit(BudgetKey::InputTokens),
            byte_limit: self.budgets.limit(BudgetKey::InputBytes),
        });
    }

    pub fn render(&self) -> String {
        let mut lines = vec!["Context budgets (cl100k_base tokens; UTF-8 bytes). Query again after edits: `lf context`.".into()];
        for (key, limit) in &self.budgets.0 {
            lines.push(format!(
                "{}: {} ({})",
                key.name(),
                limit.value,
                limit.source
            ));
        }
        for usage in &self.usage {
            lines.push(format!("{}: original {}/{} tokens, {}/{} bytes; submitted {} tokens, {} bytes; over by {} tokens, {} bytes{}",
                usage.source, usage.original_tokens, usage.token_limit, usage.original_bytes, usage.byte_limit,
                usage.submitted_tokens, usage.submitted_bytes,
                usage.original_tokens.saturating_sub(usage.token_limit), usage.original_bytes.saturating_sub(usage.byte_limit),
                if usage.source != "total assembled input" && usage.original_bytes != usage.submitted_bytes { "; excerpt only: read the complete source before curating" } else { "" }));
        }
        if let Some(input) = self
            .usage
            .iter()
            .find(|usage| usage.source == "total assembled input")
        {
            lines.push(format!(
                "Total submitted input over by {} tokens, {} bytes.",
                input.submitted_tokens.saturating_sub(input.token_limit),
                input.submitted_bytes.saturating_sub(input.byte_limit)
            ));
        }
        lines.push("The next memory- or scratch-writing step must bring over-budget sources just under these limits. Curate memory gradually: retire the largest stale sections to git history first, preserving uncommitted evidence before removal; stop once it fits instead of rewriting toward a small target. Preserve live decisions, unresolved work, attribution and evidence limits. Merge duplicate scratch notes and remove obsolete stacked-parent notes. Re-query after writing; never raise a limit just to hide overflow.".into());
        lines.join("\n")
    }
}

fn bound_source(
    text: &str,
    tokens: usize,
    bytes: usize,
    repo_root: &Path,
) -> Result<String, CoreError> {
    if text.len() <= bytes && count_tokens(text) <= tokens {
        return Ok(text.to_string());
    }
    let source = preserve_source(text, repo_root)?;
    let notice = format!(
        "\n\n[Context budget: excerpt only. Original: {}/{} tokens, {}/{} bytes. Full text: {}. Read the relevant omitted sections before acting; this excerpt is not the complete instruction.]\n\n",
        count_tokens(text), tokens, text.len(), bytes, source.display(),
    );
    // Include both ends: definitions usually lead, recent direction usually trails.
    let mut keep = bytes.saturating_sub(notice.len()).min(text.len()) / 2;
    loop {
        let mut head = keep;
        while !text.is_char_boundary(head) {
            head -= 1;
        }
        let mut tail = text.len() - keep;
        while !text.is_char_boundary(tail) {
            tail += 1;
        }
        let result = format!("{}{notice}{}", &text[..head], &text[tail..]);
        if result.len() <= bytes && count_tokens(&result) <= tokens {
            return Ok(result);
        }
        if keep == 0 {
            return Err(CoreError::ExecutionFailed(format!("context budget {tokens} tokens / {bytes} bytes cannot fit the source pointer to {}; increase the limit", source.display())));
        }
        keep = keep * 3 / 4;
    }
}

pub(crate) fn bound_context(
    components: &mut PromptComponents,
    budgets: ContextBudgets,
) -> Result<ContextBudgetReport, CoreError> {
    let mut report = ContextBudgetReport {
        budgets,
        usage: Vec::new(),
    };
    let repo_root = Path::new(&components.repo_root);
    let (memory_path, memory_text) = components
        .wave_memory
        .as_ref()
        .map(|memory| (memory.path.as_str(), memory.content.as_str()))
        .unwrap_or(("Wave memory (absent)", ""));
    // Preserve the exact gathered text, including any ancestor Wave memories.
    let bounded = report.bound_source(
        memory_path,
        memory_text,
        BudgetKey::MemoryTokens,
        BudgetKey::MemoryBytes,
        repo_root,
    )?;
    if let Some(memory) = &mut components.wave_memory {
        record_reduction(
            &mut components.budget_decisions,
            &memory.content,
            &bounded,
            ContextAssetKind::Memory,
            ContextScope::Wave,
            &memory.path,
        );
        memory.content = bounded;
    }
    let scratch_docs: Vec<_> = components
        .docs
        .iter()
        .filter(|doc| doc.source == DocumentSource::Scratch)
        .collect();
    let scratch_index = scratch_docs
        .iter()
        .map(|doc| doc.path.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let scratch = scratch_docs
        .iter()
        .map(|doc| format!("# {}\n\n{}", doc.path, doc.content))
        .collect::<Vec<_>>()
        .join("\n\n");
    let scratch = if scratch_docs.is_empty() {
        String::new()
    } else {
        format!("{scratch_index}\n\n{scratch}")
    };
    // Bound the collection, including its index: per-file notices alone
    // can exhaust a budget when a checkout has hundreds of scratch files.
    let bounded = report.bound_source(
        "scratch/",
        &scratch,
        BudgetKey::ScratchTokens,
        BudgetKey::ScratchBytes,
        repo_root,
    )?;
    if bounded != scratch {
        record_reduction(
            &mut components.budget_decisions,
            &scratch,
            &bounded,
            ContextAssetKind::Scratch,
            ContextScope::Repo,
            "scratch/",
        );
        let first = components
            .docs
            .iter()
            .position(|doc| doc.source == DocumentSource::Scratch)
            .expect("nonempty scratch exceeded its budget");
        components
            .docs
            .retain(|doc| doc.source != DocumentSource::Scratch);
        components.docs.insert(
            first,
            Document {
                path: "scratch/".into(),
                content: bounded,
                source: DocumentSource::Scratch,
            },
        );
    }
    let goal_source = if components.message.is_some() {
        "goal (launch message)"
    } else {
        "goal (no launch message)"
    };
    let bounded = report.bound_source(
        goal_source,
        components.message.as_deref().unwrap_or_default(),
        BudgetKey::GoalTokens,
        BudgetKey::GoalBytes,
        repo_root,
    )?;
    if let Some(message) = &mut components.message {
        record_reduction(
            &mut components.budget_decisions,
            message,
            &bounded,
            ContextAssetKind::UserMessage,
            ContextScope::User,
            "launch message",
        );
        *message = bounded;
    }
    Ok(report)
}

fn record_reduction(
    decisions: &mut Vec<ContextDecision>,
    original: &str,
    bounded: &str,
    kind: ContextAssetKind,
    scope: ContextScope,
    source: &str,
) {
    if original == bounded {
        return;
    }
    decisions.push(ContextDecision {
        position: decisions.len() as u32,
        kind,
        scope,
        label: source.to_string(),
        source_path: (kind != ContextAssetKind::UserMessage).then(|| source.to_string()),
        decision: ContextDecisionKind::Truncated,
        reason: "launch budget; excerpt names the complete source".into(),
        original_bytes: Some(original.len() as u64),
        original_tokens: Some(count_tokens(original) as u64),
        asset_position: None,
    });
}

pub(crate) fn bound_message(
    message: &str,
    repo_root: &Path,
    budgets: &ContextBudgets,
) -> Result<String, CoreError> {
    bound_source(
        message,
        budgets.limit(BudgetKey::GoalTokens),
        budgets.limit(BudgetKey::GoalBytes),
        repo_root,
    )
}

fn preserve_source(text: &str, repo_root: &Path) -> Result<PathBuf, CoreError> {
    let digest = format!("{:x}", Sha256::digest(text.as_bytes()));
    let path = repo_root
        .join(".lf/tmp/context")
        .join(format!("{digest}.md"));
    std::fs::create_dir_all(path.parent().expect("context file has a parent"))?;
    std::fs::write(&path, text.as_bytes())?;
    Ok(path)
}

pub(crate) fn check_input(
    system: &str,
    task: &str,
    budgets: &ContextBudgets,
) -> Result<(), CoreError> {
    let input_tokens = budgets.limit(BudgetKey::InputTokens);
    let input_bytes = budgets.limit(BudgetKey::InputBytes);
    let bytes = system.len() + task.len();
    let tokens = tokens_in(system) + tokens_in(task);
    if bytes > input_bytes || tokens > input_tokens {
        return Err(CoreError::ExecutionFailed(format!(
            "launch context exceeds the input budget: {tokens}/{input_tokens} tokens, \
             {bytes}/{input_bytes} bytes. Reduce explicit docs, skill instructions, \
             clipboard or diff context; full memory, scratch and goal sources remain on disk"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{bound_context, check_input, BudgetKey, ContextBudgets};
    use crate::engine::config::{load_config, Config};
    use crate::engine::prompt::{Document, DocumentSource, PromptComponents};

    #[test]
    fn default_memory_budget_keeps_large_memories_and_excerpts_outliers() {
        let repo = tempfile::tempdir().unwrap();
        let budgets = ContextBudgets::resolve(&Config::default(), repo.path(), None).unwrap();
        for (words, excerpted) in [(14_000, false), (16_000, false), (16_001, true)] {
            let memory = " word".repeat(words);
            let mut components = PromptComponents {
                repo_root: repo.path().display().to_string(),
                wave_memory: Some(Document {
                    path: "wave/build/MEMORY.md".into(),
                    content: memory.clone(),
                    source: DocumentSource::WaveMemory,
                }),
                ..Default::default()
            };
            let report = bound_context(&mut components, budgets.clone()).unwrap();
            let submitted = &components.wave_memory.as_ref().unwrap().content;
            assert_eq!(submitted != &memory, excerpted);
            assert_eq!(report.usage[0].token_limit, 16_000);
            assert_eq!(report.usage[0].byte_limit, 128 * 1024);
            assert!(report.usage[0].submitted_tokens <= 16_000);
            assert_eq!(components.budget_decisions.len(), usize::from(excerpted));
        }
        assert!(check_input("", &" word".repeat(64_000), &budgets).is_ok());
        assert!(check_input("", &" word".repeat(64_001), &budgets).is_err());
        assert_eq!(budgets.limit(BudgetKey::InputBytes), 512 * 1024);
    }

    #[test]
    fn budget_overrides_merge_per_field_with_winning_sources() {
        let home = crate::journal::TestLedgerGuard::new();
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir_all(repo.path().join(".lf")).unwrap();
        fs::create_dir_all(repo.path().join("wave/build")).unwrap();
        fs::write(home.home().join("config.yaml"), "context_budgets:\n  memory_tokens: 4000\n  scratch_tokens: 9000\n  input_bytes: 900000\n").unwrap();
        let repo_config = repo.path().join(".lf/config.yaml");
        fs::write(
            &repo_config,
            "context_budgets:\n  memory_tokens: 5000\n  goal_tokens: 7000\n",
        )
        .unwrap();
        let wave_config = repo.path().join("wave/build/GOAL.md");
        fs::write(
            &wave_config,
            "---\ncontext_budgets:\n  memory_tokens: 6000\n---\nBuild.\n",
        )
        .unwrap();
        let config = load_config(Some(repo.path())).unwrap().unwrap();
        let budgets = ContextBudgets::resolve(&config, repo.path(), Some("build")).unwrap();
        for (key, value, source) in [
            (
                BudgetKey::MemoryTokens,
                6000,
                wave_config.display().to_string(),
            ),
            (
                BudgetKey::ScratchTokens,
                9000,
                home.home().join("config.yaml").display().to_string(),
            ),
            (
                BudgetKey::GoalTokens,
                7000,
                repo_config.display().to_string(),
            ),
            (BudgetKey::InputTokens, 64000, "default".into()),
        ] {
            assert_eq!(budgets.limit(key), value);
            assert_eq!(budgets.0[&key].source, source);
        }
        assert_eq!(budgets.limit(BudgetKey::InputBytes), 900000);
    }

    #[test]
    fn source_feedback_preserves_original_usage_and_clears_after_curation() {
        let repo = tempfile::tempdir().unwrap();
        let config = Config {
            context_budgets: [
                (BudgetKey::MemoryTokens, 400),
                (BudgetKey::ScratchBytes, 1800),
            ]
            .into(),
            ..Default::default()
        };
        let budgets = ContextBudgets::resolve(&config, repo.path(), None).unwrap();
        let mut components = PromptComponents {
            repo_root: repo.path().display().to_string(),
            wave_memory: Some(Document {
                path: "wave/build/MEMORY.md".into(),
                content: "Live decision and evidence. ".repeat(400),
                source: DocumentSource::WaveMemory,
            }),
            docs: vec![Document {
                path: "scratch/plan.md".into(),
                content: "Remain unresolved. ".repeat(600),
                source: DocumentSource::Scratch,
            }],
            ..Default::default()
        };
        let report = bound_context(&mut components, budgets.clone()).unwrap();
        assert!(report.usage[0].original_tokens > 400);
        assert!(report.usage[0].submitted_tokens <= 400);
        assert!(report.usage[1].original_bytes > 1800);
        assert!(report.usage[1].submitted_bytes <= 1800);
        assert!(report
            .render()
            .contains("next memory- or scratch-writing step"));
        assert_eq!(components.budget_decisions.len(), 2);
        assert_eq!(
            fs::read_dir(repo.path().join(".lf/tmp/context"))
                .unwrap()
                .count(),
            2
        );

        components.wave_memory.as_mut().unwrap().content =
            "Live decision retained; old evidence in git.".into();
        components.docs[0].content = "Unresolved work retained.".into();
        let report = bound_context(&mut components, budgets).unwrap();
        assert!(report
            .usage
            .iter()
            .all(|usage| usage.original_tokens <= usage.token_limit
                && usage.original_bytes <= usage.byte_limit));
        assert!(!report.render().contains("excerpt only"));
    }

    #[test]
    fn configured_input_limit_replaces_the_default() {
        let repo = tempfile::tempdir().unwrap();
        let config = Config {
            context_budgets: [(BudgetKey::InputTokens, 100)].into(),
            ..Default::default()
        };
        let budgets = ContextBudgets::resolve(&config, repo.path(), None).unwrap();
        assert!(check_input("small", "message", &budgets).is_ok());
        assert!(check_input("", &" word".repeat(101), &budgets)
            .unwrap_err()
            .to_string()
            .contains("/100 tokens"));
        let config = Config {
            context_budgets: [(BudgetKey::InputBytes, 0)].into(),
            ..Default::default()
        };
        assert!(ContextBudgets::resolve(&config, repo.path(), None)
            .unwrap_err()
            .to_string()
            .contains("input_bytes must be positive"));
    }
}
