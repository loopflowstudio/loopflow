//! Attribute the context submitted to a provider turn.

use crate::prompt::PromptComponents;

pub fn attributed_context(
    components: &PromptComponents,
    system_prompt: &str,
    task_prompt: &str,
    deduplication_decisions: &[crate::trace::ContextDecision],
    request: Option<&str>,
) -> crate::trace::PreparedTurnContext {
    use crate::prompt::DocumentSource;
    use crate::trace::{
        ContextAssetKind as Kind, ContextAssetSpec, ContextChannel, ContextDecision,
        ContextDecisionKind, ContextScope as Scope,
    };

    let mut specs = Vec::new();
    let mut push = |content: &str,
                    kind: Kind,
                    scope: Scope,
                    label: String,
                    source_path: Option<String>,
                    included_by: &str| {
        if content.is_empty() {
            return;
        }
        // Attribute the submitted reference bytes, while components retain the
        // authored source. Otherwise escaped references become anonymous assembly.
        let content = match included_by {
            "wave" | "docs" | "diff_files" | "diff" | "summary" | "clipboard" => {
                crate::prompt::escape_reference(content)
            }
            "message" => crate::prompt::render_message(content),
            "steers" => crate::prompt::render_reference(content),
            _ => content.to_string(),
        };
        for channel in [ContextChannel::System, ContextChannel::Task]
            .into_iter()
            .filter(|channel| match channel {
                ContextChannel::System => system_prompt.contains(&content),
                ContextChannel::Task => task_prompt.contains(&content),
            })
        {
            specs.push(ContextAssetSpec {
                channel,
                kind,
                scope,
                label: label.clone(),
                source_path: source_path.clone(),
                included_by: included_by.to_string(),
                content: content.to_string(),
                match_all_occurrences: !matches!(
                    included_by,
                    "message" | "steers" | "vendor_skill"
                ),
            });
        }
    };

    if components.operate {
        let source_path = std::path::Path::new(&components.repo_root)
            .join("rust/loopflow-definitions/src/builtins/LOOPFLOW.md");
        push(
            &crate::prompt::loopflow_section(),
            Kind::OperatingInstructions,
            Scope::Global,
            "LOOPFLOW.md".to_string(),
            source_path
                .is_file()
                .then(|| source_path.to_string_lossy().to_string()),
            "operate",
        );
    }
    if let Some(guidance) = tagged_block(
        system_prompt,
        "<lf:structured_replies>",
        "</lf:structured_replies>",
    ) {
        push(
            guidance,
            Kind::ProviderInstructions,
            Scope::Provider,
            "structured reply contract".to_string(),
            None,
            "provider_invocation",
        );
    }
    push(
        components.surface.instructions(),
        Kind::SurfaceInstructions,
        Scope::Global,
        format!("{:?} surface", components.surface),
        None,
        "surface",
    );
    if let Some(wave) = &components.wave {
        let open = format!("<lf:wave name=\"{wave}\">");
        let goal = tagged_block(system_prompt, &open, "</lf:wave>")
            .or_else(|| tagged_block(task_prompt, &open, "</lf:wave>"))
            .unwrap_or(open.as_str());
        push(goal, Kind::Goal, Scope::Wave, wave.clone(), None, "wave");
    }
    for document in &components.docs {
        let kind = if document.source == DocumentSource::Scratch {
            Kind::Scratch
        } else if document.source == DocumentSource::RepoMemory
            || (document.source == DocumentSource::Wave && document.path.ends_with("/MEMORY.md"))
        {
            Kind::Memory
        } else if document.path.ends_with("AGENTS.md") || document.path.ends_with("CLAUDE.md") {
            Kind::RepoInstructions
        } else {
            Kind::Document
        };
        let content = if document.source == DocumentSource::Docs {
            crate::prompt::format_document(document)
        } else {
            document.content.clone()
        };
        push(
            &content,
            kind,
            if document.source == DocumentSource::Wave {
                Scope::Wave
            } else {
                Scope::Repo
            },
            document.path.clone(),
            Some(document.path.clone()),
            if document.source == DocumentSource::Wave {
                "wave"
            } else {
                "docs"
            },
        );
    }
    for document in &components.diff_files {
        push(
            &document.content,
            Kind::Diff,
            Scope::Repo,
            document.path.clone(),
            Some(document.path.clone()),
            "diff_files",
        );
    }
    if let Some(change) = &components.change {
        push(
            &change.prompt_section(),
            Kind::Diff,
            Scope::Repo,
            "branch diff".to_string(),
            None,
            "diff",
        );
    }
    for summary in &components.summaries {
        push(
            &summary.content,
            Kind::Summary,
            Scope::Task,
            summary.path.clone(),
            Some(summary.path.clone()),
            "summary",
        );
    }
    if let Some(clipboard) = &components.clipboard {
        push(
            clipboard,
            Kind::Clipboard,
            Scope::User,
            "clipboard".to_string(),
            None,
            "clipboard",
        );
    }
    if let Some(skill) = &components.skill {
        if let Some(content) = &skill.content {
            push(
                content,
                Kind::SkillInstructions,
                Scope::Step,
                skill.name.clone(),
                skill
                    .source
                    .as_ref()
                    .map(|source| source.path.to_string_lossy().to_string()),
                "skill",
            );
        } else {
            push(
                &skill.name,
                Kind::SkillInstructions,
                Scope::Step,
                skill.name.clone(),
                None,
                "vendor_skill",
            );
        }
    }
    // Bound launches append the caller's request to inherited Work context.
    // Attribute it separately before the enclosing Goal claims the same bytes.
    if let Some(request) = request {
        push(
            request,
            Kind::UserMessage,
            Scope::User,
            "user message".into(),
            None,
            "message",
        );
    }
    if let Some(message) = &components.message {
        let (kind, scope) = components
            .message_context
            .unwrap_or((Kind::UserMessage, Scope::User));
        push(
            message,
            kind,
            scope,
            if kind == Kind::UserMessage {
                "user message".to_string()
            } else {
                "inherited launch goal".to_string()
            },
            None,
            "message",
        );
    }

    let mut decisions = deduplication_decisions.to_vec();
    if matches!(&components.change, Some(change) if change.patch.is_none()) {
        decisions.push(ContextDecision {
            position: decisions.len() as u32,
            kind: Kind::Diff,
            scope: Scope::Repo,
            label: "branch diff".to_string(),
            source_path: None,
            decision: ContextDecisionKind::StatOnly,
            reason: "the patch is too large to inline".to_string(),
            original_bytes: None,
            original_tokens: None,
            asset_position: None,
        });
    }

    let mut context = crate::trace::PreparedTurnContext::from_attributed_prompts(
        system_prompt,
        task_prompt,
        specs,
        decisions,
    );
    for decision in &mut context.decisions {
        if decision.decision != ContextDecisionKind::Included {
            continue;
        }
        match decision.kind {
            Kind::OperatingInstructions if decision.label == "LOOPFLOW.md" => {
                decision.reason = "default launcher guidance".into();
            }
            Kind::SkillInstructions => {
                decision.reason = format!("selected skill: {}", decision.label);
            }
            Kind::SurfaceInstructions => {
                decision.reason = format!("{:?} conversation surface", components.surface);
            }
            Kind::RepoInstructions => {
                decision.reason = "repository guide included in this input".into();
            }
            _ => {}
        }
    }
    let mut omitted = |label: &str, decision, reason: &str| {
        context.decisions.push(ContextDecision {
            position: context.decisions.len() as u32,
            kind: Kind::OperatingInstructions,
            scope: Scope::Global,
            label: label.into(),
            source_path: None,
            decision,
            reason: reason.into(),
            original_bytes: None,
            original_tokens: None,
            asset_position: None,
        });
    };
    if !components.operate {
        omitted(
            "LOOPFLOW.md",
            ContextDecisionKind::Excluded,
            "automatic guidance disabled for this input",
        );
    }
    context
}

fn tagged_block<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = text.find(open)?;
    let end = text[start..].find(close)? + start + close.len();
    Some(&text[start..end])
}

#[cfg(test)]
mod tests {
    use super::attributed_context;
    use crate::prompt::change::{Change, ChangeRange};
    use crate::prompt::{Document, DocumentSource, PromptComponents};
    use crate::trace::{ContextAssetKind, ContextScope};
    #[test]
    fn attributed_context_does_not_claim_reference_bodies_in_first_turn() {
        let components = PromptComponents {
            docs: vec![
                Document {
                    path: "scratch/intent.md".into(),
                    content: "> $kickoff".into(),
                    source: DocumentSource::Scratch,
                },
                Document {
                    path: ".lf/wave/product/MEMORY.md".into(),
                    content: "Earlier $design".into(),
                    source: DocumentSource::Wave,
                },
            ],
            message: Some("Build it.\n<lf:steers>Jack wrote $kickoff.</lf:steers>".into()),
            ..Default::default()
        };
        let turn = crate::prompt::format_first_turn(&components);
        let prepared = attributed_context(&components, "", &turn, &[], None);
        let task = &prepared.task;
        assert_eq!(task.text, turn);
        assert!(turn.contains("Build it."));
        assert!(turn.contains("Jack wrote &#36;kickoff."));
        assert!(!turn.contains("Earlier"));
        assert!(!task.assets.iter().any(|asset| matches!(
            asset.kind,
            ContextAssetKind::Scratch | ContextAssetKind::Memory
        )));
        assert!(task.assets.iter().any(|asset| {
            asset.kind == ContextAssetKind::UserMessage
                && turn[asset.byte_start as usize..asset.byte_end as usize].contains("Build it.")
        }));
    }

    #[test]
    fn attributed_context_keeps_nested_message_and_repeated_channel_sources() {
        let components = PromptComponents {
            docs: vec![Document {
                path: ".lf/wave/product/MEMORY.md".to_string(),
                content: "MEMORY".to_string(),
                source: DocumentSource::Wave,
            }],
            message: Some("outer MEMORY remainder".to_string()),
            message_context: Some((ContextAssetKind::Goal, ContextScope::Step)),
            ..PromptComponents::default()
        };

        let prepared =
            attributed_context(&components, "MEMORY", "outer MEMORY remainder", &[], None);

        assert_eq!(
            prepared.system.unwrap().assets[0].kind,
            ContextAssetKind::Memory
        );
        assert_eq!(
            prepared
                .task
                .assets
                .iter()
                .filter(|asset| asset.kind == ContextAssetKind::Memory)
                .count(),
            1
        );
        assert_eq!(
            prepared
                .task
                .assets
                .iter()
                .filter(|asset| asset.kind == ContextAssetKind::Goal)
                .count(),
            2
        );
        assert!(prepared
            .task
            .assets
            .iter()
            .all(|asset| asset.kind != ContextAssetKind::Assembly));
    }

    #[test]
    fn the_branch_change_in_the_first_turn_is_a_diff_asset() {
        let components = PromptComponents {
            message: Some("What does this branch change?".into()),
            change: Some(Change {
                range: ChangeRange::Uncommitted,
                files: Vec::new(),
                patch: Some("+let cost = \"$5\";\n".into()),
                patch_bytes: 20,
            }),
            ..Default::default()
        };
        let turn = crate::prompt::format_first_turn(&components);
        let prepared = attributed_context(&components, "", &turn, &[], None);
        let diff = prepared
            .task
            .assets
            .iter()
            .find(|asset| asset.kind == ContextAssetKind::Diff)
            .expect("the change section is attributed");
        assert!(turn[diff.byte_start as usize..diff.byte_end as usize].contains("&#36;5"));
    }
}
