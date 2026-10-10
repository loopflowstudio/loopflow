//! Measure authored context against size targets without changing launch input.

use std::collections::BTreeMap;
use std::path::Path;

use crate::engine::config::Config;
use crate::engine::error::CoreError;
use crate::engine::prompt::{count_tokens, DocumentSource, PromptComponents};

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
}

impl BudgetKey {
    pub const ALL: [Self; 4] = [
        Self::MemoryTokens,
        Self::MemoryBytes,
        Self::ScratchTokens,
        Self::ScratchBytes,
    ];

    pub fn default_limit(self) -> usize {
        match self {
            Self::MemoryTokens => 16_000,
            // The Task's own notes fit; replays without older scratch reached the
            // same outcomes, reading files from disk (performance/context-ablation.md).
            Self::ScratchTokens => 12_000,
            Self::MemoryBytes => 128 * 1024,
            Self::ScratchBytes => 96 * 1024,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::MemoryTokens => "memory_tokens",
            Self::MemoryBytes => "memory_bytes",
            Self::ScratchTokens => "scratch_tokens",
            Self::ScratchBytes => "scratch_bytes",
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
    pub fn render(&self) -> String {
        let mut lines = vec!["Context size targets (cl100k_base tokens; UTF-8 bytes). Query after edits: `lf context`. Targets do not truncate input or refuse launches.".into()];
        for (key, limit) in &self.budgets.0 {
            lines.push(format!(
                "{}: {} ({})",
                key.name(),
                limit.value,
                limit.source
            ));
        }
        for usage in &self.usage {
            lines.push(format!(
                "{}: {}/{} tokens, {}/{} bytes; over by {} tokens, {} bytes",
                usage.source,
                usage.original_tokens,
                usage.token_limit,
                usage.original_bytes,
                usage.byte_limit,
                usage.original_tokens.saturating_sub(usage.token_limit),
                usage.original_bytes.saturating_sub(usage.byte_limit)
            ));
        }
        lines.join("\n")
    }
}

pub(crate) fn measure_context(
    components: &PromptComponents,
    budgets: ContextBudgets,
) -> ContextBudgetReport {
    let mut usage = Vec::new();
    let mut measure = |source: &str, text: &str, tokens, bytes| {
        usage.push(ContextUsage {
            source: source.into(),
            original_tokens: tokens_in(text),
            original_bytes: text.len(),
            submitted_tokens: tokens_in(text),
            submitted_bytes: text.len(),
            token_limit: budgets.limit(tokens),
            byte_limit: budgets.limit(bytes),
        });
    };
    let memories: Vec<_> = components
        .docs
        .iter()
        .filter(|doc| {
            doc.source == DocumentSource::RepoMemory
                || (doc.source == DocumentSource::Wave && doc.path.ends_with("/MEMORY.md"))
        })
        .collect();
    if !memories.is_empty() {
        let paths = memories
            .iter()
            .map(|doc| doc.path.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let text = memories
            .iter()
            .map(|doc| doc.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        measure(
            &paths,
            &text,
            BudgetKey::MemoryTokens,
            BudgetKey::MemoryBytes,
        );
    }
    let scratch = components
        .docs
        .iter()
        .filter(|doc| doc.source == DocumentSource::Scratch)
        .map(|doc| doc.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    measure(
        "scratch/",
        &scratch,
        BudgetKey::ScratchTokens,
        BudgetKey::ScratchBytes,
    );
    ContextBudgetReport { budgets, usage }
}

#[cfg(test)]
mod tests {
    use super::{measure_context, BudgetKey, ContextBudgets};
    use crate::engine::config::Config;
    use crate::engine::prompt::{Document, DocumentSource, PromptComponents};

    #[test]
    fn context_targets_measure_without_changing_sources() {
        let repo = tempfile::tempdir().unwrap();
        let config = Config {
            context_budgets: [(BudgetKey::ScratchBytes, 1)].into(),
            ..Default::default()
        };
        let components = PromptComponents {
            docs: vec![Document {
                path: "scratch/plan.md".into(),
                content: "😀 exact source\r\n".repeat(1000),
                source: DocumentSource::Scratch,
            }],
            ..Default::default()
        };
        let budgets = ContextBudgets::resolve(&config, repo.path(), None).unwrap();
        let report = measure_context(&components, budgets);
        assert_eq!(
            report.usage[0].original_bytes,
            components.docs[0].content.len()
        );
        assert_eq!(
            report.usage[0].submitted_bytes,
            report.usage[0].original_bytes
        );
        assert!(report.render().contains("over by"));
        assert!(!repo.path().join(".lf").exists());
    }
}
