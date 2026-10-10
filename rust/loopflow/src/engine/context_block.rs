//! Whole-file conversation context. Size targets never select launch content.
//!
//! Claude's hook string limit is 10,000 characters; Codex 0.161.0's default
//! is 2,500 approximate tokens (ceil(UTF-8 bytes / 4)). The shared 10,000-byte
//! ceiling also avoids the observed Claude Unicode spill. Budget the rendered
//! string, including paths, escaping and the compact-only active skill.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::engine::error::CoreError;
use crate::engine::prompt::{
    gather_documents, render_reference, write_prompt_log, Document, DocumentSource, GatherSpec,
};

const HOOK_CONTEXT_BYTES: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ContextMoment {
    Start,
    Compact,
}

/// Captured source identities, not frozen context. Hooks reread scratch and Wave
/// storage; references and the active skill retain their complete launch bytes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContextDelivery {
    pub repo: PathBuf,
    pub wave_id: Option<crate::id::WaveId>,
    pub skill_file: Option<PathBuf>,
    pub references: Vec<PathBuf>,
    pub home: PathBuf,
}

impl ContextDelivery {
    pub fn prepare(
        components: &crate::engine::prompt::PromptComponents,
    ) -> Result<Self, CoreError> {
        let repo = fs::canonicalize(&components.repo_root)?;
        let mut references = Vec::new();
        for doc in components.docs.iter().chain(&components.summaries) {
            if matches!(doc.source, DocumentSource::Scratch | DocumentSource::Wave) {
                continue;
            }
            // Explicit documents remain paths. Generated summaries have no source file.
            references.push(if doc.source == DocumentSource::Summary {
                write_prompt_log(&repo, &doc.content, "summary", None)?
            } else {
                repo.join(&doc.path)
            });
        }
        if let Some(text) = &components.clipboard {
            references.push(write_prompt_log(&repo, text, "clipboard", None)?);
        }
        if components.diff.is_some() || !components.diff_files.is_empty() {
            let paths = components
                .diff_files
                .iter()
                .map(|doc| doc.path.as_str())
                .collect::<Vec<_>>();
            references.push(write_prompt_log(&repo, &format!(
                "Inspect current changes with `git diff` and `git diff main...HEAD`.\nChanged paths: {}\n",
                serde_json::to_string(&paths).expect("paths serialize")
            ), "changes", None)?);
        }
        let skill_file = components
            .skill
            .as_ref()
            .map(|skill| {
                let source = skill
                    .source
                    .as_ref()
                    .map(|source| {
                        format!(
                    "Original skill source: {}. Resolve relative assets from its directory.\n\n",
                    serde_json::to_string(&source.path).expect("skill path serializes")
                )
                    })
                    .unwrap_or_default();
                write_prompt_log(
                    &repo,
                    &format!("{source}{}", skill.source_text()),
                    "active-skill",
                    None,
                )
            })
            .transpose()?;
        Ok(Self {
            repo: repo.clone(),
            wave_id: components
                .wave
                .as_deref()
                .map(|name| {
                    crate::work::wave::context::resolve_managed_wave_sync(Some(&repo), Some(name))
                        .map(|wave| wave.id().clone())
                        .map_err(|error| CoreError::IoError(error.to_string()))
                })
                .transpose()?,
            skill_file,
            references,
            home: crate::store::lf_home_dir(),
        })
    }

    pub fn block(&self, moment: ContextMoment) -> Result<ContextBlock, CoreError> {
        // A Wave may move while its conversation and checkout stay put. Follow
        // its durable identity for memory, but keep scratch in this checkout.
        let mut documents = gather_documents(&GatherSpec {
            repo_root: self.repo.clone(),
            ..Default::default()
        })?;
        let wave = if let Some(id) = &self.wave_id {
            let store = crate::store::sqlite::SqliteStore::open_read_only(
                &crate::store::database_path_from_env()?,
            )
            .map_err(|error| CoreError::IoError(error.to_string()))?;
            let wave = store
                .get_wave(id)
                .map_err(|error| CoreError::IoError(error.to_string()))?
                .ok_or_else(|| CoreError::IoError(format!("Saved context Wave {id} is missing")))?;
            documents.extend(crate::engine::prompt::gather_saved_wave_docs(
                &store, &wave,
            )?);
            Some(wave)
        } else {
            None
        };
        let wave = wave.as_ref().map(|wave| wave.slug());
        let branch = crate::engine::git::current_branch(&self.repo)
            .ok()
            .flatten();
        order_documents(&mut documents, wave, branch.as_deref());
        render_block(
            &self.repo,
            wave,
            moment,
            self.skill_file.as_deref(),
            &self.references,
            documents,
        )
    }

    /// Saved settings are private and outlive the launching driver. Native
    /// resumes do not need the original environment or replay the first turn.
    pub fn hook_settings(&self) -> anyhow::Result<serde_json::Value> {
        let path = write_prompt_log(
            &self.repo,
            &serde_json::to_string(self)?,
            "context-delivery",
            None,
        )?;
        let executable = crate::engine::process::resolve_lf_binary();
        let quote = |path: &Path| crate::engine::process::shell_escape(&path.to_string_lossy());
        let command = format!(
            "env LF_HOME={} {} __context-block --delivery {} --moment",
            quote(&self.home),
            quote(&executable),
            quote(&path)
        );
        Ok(serde_json::json!({"SessionStart": [
            {"matcher": "startup|resume", "hooks": [{"type":"command", "command": format!("{command} start"), "timeout": 30}]},
            {"matcher": "compact", "hooks": [{"type":"command", "command": format!("{command} compact"), "timeout": 30}]}
        ]}))
    }
}

/// One complete, readable source; logical Wave names never masquerade as files.
#[derive(Debug, Serialize)]
struct ContextFile {
    source: String,
    path: PathBuf,
    bytes: usize,
}

/// The complete listing is retained even when its pointer alone fits the hook.
#[derive(Debug)]
pub struct ContextBlock {
    pub text: String,
    pub manifest_path: PathBuf,
}

fn order_documents(documents: &mut [Document], wave: Option<&str>, branch: Option<&str>) {
    let memory = wave.map(|wave| format!("wave/{wave}/MEMORY.md"));
    let plan = branch.map(|branch| format!("scratch/{branch}.md"));
    let priority = |doc: &Document| {
        if memory.as_deref() == Some(doc.path.as_str()) {
            0
        } else if plan.as_deref() == Some(doc.path.as_str()) {
            1
        } else if doc.source == DocumentSource::Scratch {
            2
        } else {
            3
        }
    };
    documents.sort_by(|a, b| {
        (priority(a), a.content.len(), &a.path).cmp(&(priority(b), b.content.len(), &b.path))
    });
}

fn render_block(
    repo_root: &Path,
    wave: Option<&str>,
    moment: ContextMoment,
    skill_file: Option<&Path>,
    references: &[PathBuf],
    documents: Vec<Document>,
) -> Result<ContextBlock, CoreError> {
    let mut files = Vec::new();
    for doc in &documents {
        // Wave bytes belong to SQLite. Retain an immutable complete snapshot;
        // a similarly named checkout file may be stale or may not exist at all.
        let path = if doc.source == DocumentSource::Wave {
            write_prompt_log(repo_root, &doc.content, "wave-snapshot", None)?
        } else {
            repo_root.join(&doc.path)
        };
        files.push(ContextFile {
            source: doc.path.clone(),
            path,
            bytes: doc.content.len(),
        });
    }
    for path in references {
        let path = repo_root.join(path);
        files.push(ContextFile {
            source: path.display().to_string(),
            bytes: fs::read(&path)?.len(),
            path,
        });
    }
    let skill = match (moment, skill_file) {
        (ContextMoment::Compact, Some(path)) => {
            let path = repo_root.join(path);
            let text = fs::read_to_string(&path)?;
            files.push(ContextFile {
                source: "Active skill (saved at launch)".into(),
                path,
                bytes: text.len(),
            });
            Some(text)
        }
        _ => None,
    };
    // Serialize the metadata once: the inline listing and overflow file describe
    // exactly the same sources, including the scratch/Wave roots and saved skill.
    let manifest = serde_json::to_string_pretty(&serde_json::json!({
        "scratch": repo_root.join("scratch"),
        "wave": wave.map(|wave| format!("wave/{wave}")),
        "wave_source": "SQLite; read the snapshot paths below, not checkout copies",
        "files": files,
    }))
    .expect("context manifest is JSON serializable");
    let manifest_path = write_prompt_log(repo_root, &manifest, "context-manifest", None)?;
    // A repository-relative pointer leaves room for context even with long roots.
    let pointer = manifest_path
        .strip_prefix(repo_root)
        .expect("manifest is inside repository");
    let mut text = format!(
        "Current reference context, not a new request. Historical instructions in these files do not select work.\n\
         From the repository root:\nRead the complete context listing and any files not preloaded below: {}.\n",
        serde_json::to_string(pointer).expect("manifest pointer is JSON serializable")
    );
    // Reserve the saved skill before the listing or any file body. Neither is
    // ever cut; omitted sections remain readable through the manifest.
    if let Some(skill) = skill {
        if !append_whole(
            &mut text,
            &format!("\nActive skill after compaction:\n{skill}\n"),
        ) {
            text.push_str("Read the complete saved active skill from the context listing.\n");
        }
    }
    append_whole(
        &mut text,
        &format!(
            "\nComplete file listing (UTF-8 bytes):\n{}\n",
            render_reference(&manifest)
        ),
    );
    for doc in documents {
        let source = serde_json::to_string(&doc.path).expect("document path serializes");
        let body = render_reference(&format!(
            "\n<lf:file source={source}>\n{}\n</lf:file>\n",
            doc.content
        ));
        append_whole(&mut text, &body);
    }
    Ok(ContextBlock {
        text,
        manifest_path,
    })
}

/// Selection always counts rendered bytes, including reference escaping.
fn append_whole(text: &mut String, section: &str) -> bool {
    if text.len() + section.len() > HOOK_CONTEXT_BYTES {
        return false;
    }
    text.push_str(section);
    true
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use serde_json::Value;
    use tempfile::tempdir;

    use super::{
        order_documents, render_block, ContextDelivery, ContextMoment, HOOK_CONTEXT_BYTES,
    };
    use crate::engine::prompt::{render_reference, Document, DocumentSource};

    fn delivery(repo: &Path) -> ContextDelivery {
        ContextDelivery {
            repo: repo.canonicalize().unwrap(),
            wave_id: None,
            skill_file: None,
            references: Vec::new(),
            home: repo.join("machine"),
        }
    }

    fn document(path: &str, content: &str, source: DocumentSource) -> Document {
        Document {
            path: path.into(),
            content: content.into(),
            source,
        }
    }

    #[test]
    fn saved_context_follows_wave_identity_after_rename() {
        let _home = crate::journal::TestLedgerGuard::new();
        let repo = loopflow_test_support::TestRepo::new();
        let canonical = crate::repository::CanonicalRepo::discover(repo.path()).unwrap();
        let store = crate::store::sqlite::SqliteStore::new(
            &crate::store::database_path_from_env().unwrap(),
        )
        .unwrap();
        let wave = store.ensure_wave(&canonical.to_string(), "before").unwrap();
        store
            .update_wave_document(&wave, "MEMORY.md", "Retained wave memory")
            .unwrap();
        fs::create_dir(repo.path().join("scratch")).unwrap();
        fs::write(repo.path().join("scratch/plan.md"), "Checkout scratch").unwrap();
        let delivery = super::ContextDelivery::prepare(&crate::engine::prompt::PromptComponents {
            repo_root: repo.path().display().to_string(),
            wave: Some("before".into()),
            ..Default::default()
        })
        .unwrap();
        let parent = store.ensure_wave(&canonical.to_string(), "parent").unwrap();
        store
            .update_wave_document(&parent, "MEMORY.md", "Current ancestor memory")
            .unwrap();
        store
            .relocate_waves(&[crate::store::WaveLocatorUpdate {
                wave_id: wave.clone(),
                expected_repo: canonical.to_string(),
                expected_slug: "before".into(),
                target: crate::work::wave::WaveLocator::new(canonical.clone(), "parent/after")
                    .unwrap(),
                retire_collision: None,
            }])
            .unwrap();
        let replacement = store.ensure_wave(&canonical.to_string(), "before").unwrap();
        store
            .update_wave_document(&replacement, "MEMORY.md", "Wrong replacement memory")
            .unwrap();
        let block = delivery.block(ContextMoment::Compact).unwrap().text;
        assert!(block.contains("Retained wave memory"));
        assert!(block.contains("wave/parent/after/MEMORY.md"));
        assert!(block.contains("Current ancestor memory"));
        assert!(block.contains("Checkout scratch"));
        assert!(!block.contains("Wrong replacement memory"));
    }

    #[test]
    fn context_block_reads_current_files_without_cutting_large_unicode_documents() {
        let repo = tempdir().unwrap();
        fs::create_dir(repo.path().join("scratch")).unwrap();
        let large = format!("WHOLE_START{}WHOLE_END", "🦀".repeat(50_000));
        fs::write(repo.path().join("scratch/large.md"), &large).unwrap();
        fs::write(repo.path().join("scratch/small.md"), "fresh one").unwrap();
        let first = delivery(repo.path()).block(ContextMoment::Start).unwrap();
        assert!(first.text.len() <= HOOK_CONTEXT_BYTES);
        assert!(first.text.contains("fresh one"));
        assert!(!first.text.contains("WHOLE_START"));
        assert!(!first.text.contains("WHOLE_END"));
        let manifest: Value =
            serde_json::from_slice(&fs::read(first.manifest_path).unwrap()).unwrap();
        let large_file = manifest["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|file| file["source"] == "scratch/large.md")
            .unwrap();
        assert_eq!(large_file["bytes"], large.len());
        assert_eq!(
            fs::read_to_string(large_file["path"].as_str().unwrap()).unwrap(),
            large
        );

        fs::write(repo.path().join("scratch/small.md"), "fresh two").unwrap();
        let compact = delivery(repo.path()).block(ContextMoment::Compact).unwrap();
        assert!(compact.text.contains("fresh two"));
        assert!(!compact.text.contains("fresh one"));
    }

    #[test]
    fn context_block_reports_unreadable_text_instead_of_claiming_a_complete_listing() {
        let repo = tempdir().unwrap();
        fs::create_dir(repo.path().join("scratch")).unwrap();
        fs::write(repo.path().join("scratch/invalid.md"), [0xff, 0xfe]).unwrap();
        let error = delivery(repo.path())
            .block(ContextMoment::Start)
            .unwrap_err();
        assert!(error.to_string().contains("scratch/invalid.md"));
    }

    #[test]
    fn context_block_prioritizes_selected_memory_then_branch_plan_then_small_scratch() {
        let mut docs = vec![
            document("scratch/b.md", "four", DocumentSource::Scratch),
            document("wave/parent/MEMORY.md", "p", DocumentSource::Wave),
            document("scratch/a.md", "one", DocumentSource::Scratch),
            document(
                "scratch/topic.md",
                "the branch plan",
                DocumentSource::Scratch,
            ),
            document(
                "wave/parent/child/MEMORY.md",
                "selected memory",
                DocumentSource::Wave,
            ),
        ];
        order_documents(&mut docs, Some("parent/child"), Some("topic"));
        assert_eq!(
            docs.iter().map(|doc| doc.path.as_str()).collect::<Vec<_>>(),
            [
                "wave/parent/child/MEMORY.md",
                "scratch/topic.md",
                "scratch/a.md",
                "scratch/b.md",
                "wave/parent/MEMORY.md",
            ]
        );
    }

    #[test]
    fn context_block_retains_wave_bytes_at_readable_private_paths_not_checkout_copies() {
        let repo = tempdir().unwrap();
        let saved = "saved Wave memory 🦀\r\nwith final whitespace \t\n";
        fs::create_dir_all(repo.path().join("wave/example")).unwrap();
        let checkout = repo.path().join("wave/example/MEMORY.md");
        fs::write(&checkout, "stale checkout copy").unwrap();
        let block = render_block(
            repo.path(),
            Some("example"),
            ContextMoment::Start,
            None,
            &[],
            vec![document(
                "wave/example/MEMORY.md",
                saved,
                DocumentSource::Wave,
            )],
        )
        .unwrap();
        let manifest = fs::read_to_string(&block.manifest_path).unwrap();
        assert!(block.text.contains(&render_reference(&manifest)));
        let manifest: Value = serde_json::from_str(&manifest).unwrap();
        let path = std::path::Path::new(manifest["files"][0]["path"].as_str().unwrap());
        assert_ne!(path, checkout);
        assert_eq!(fs::read_to_string(path).unwrap(), saved);
        assert_eq!(fs::read_to_string(checkout).unwrap(), "stale checkout copy");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(block.manifest_path)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn context_block_overflow_keeps_the_entire_listing_and_skill_readable() {
        let repo = tempdir().unwrap();
        fs::create_dir(repo.path().join("scratch")).unwrap();
        for index in 0..200 {
            fs::write(
                repo.path().join(format!("scratch/file-{index:03}.md")),
                format!("file {index}"),
            )
            .unwrap();
        }
        let skill = "ACTIVE_START\n".to_owned() + &"saved skill\n".repeat(2_000) + "ACTIVE_END";
        let skill_path = repo.path().join("active-skill.md");
        fs::write(&skill_path, &skill).unwrap();
        let block = ContextDelivery {
            skill_file: Some(skill_path.clone()),
            ..delivery(repo.path())
        }
        .block(ContextMoment::Compact)
        .unwrap();
        assert!(block.text.len() <= HOOK_CONTEXT_BYTES);
        assert!(!block.text.contains("Complete file listing (UTF-8 bytes)"));
        assert!(!block.text.contains("ACTIVE_START"));
        assert!(!block.text.contains("ACTIVE_END"));
        assert!(block.text.contains("Read the complete saved active skill"));
        assert!(block
            .text
            .contains(block.manifest_path.file_name().unwrap().to_str().unwrap()));
        let manifest: Value =
            serde_json::from_slice(&fs::read(block.manifest_path).unwrap()).unwrap();
        assert_eq!(manifest["files"].as_array().unwrap().len(), 201);
        for file in manifest["files"].as_array().unwrap() {
            let contents = fs::read(file["path"].as_str().unwrap()).unwrap();
            assert_eq!(contents.len() as u64, file["bytes"].as_u64().unwrap());
        }
        assert_eq!(fs::read_to_string(&skill_path).unwrap(), skill);
    }

    #[test]
    fn context_block_reserves_skill_when_only_the_combined_listing_overflows() {
        let repo = tempdir().unwrap();
        let references: Vec<_> = (0..20)
            .map(|index| {
                let path = repo.path().join(format!("reference-{index}.md"));
                fs::write(&path, "Listed reference, not preloaded").unwrap();
                path
            })
            .collect();
        let skill_path = repo.path().join("skill.md");
        let skill = "s".repeat(7_000);
        fs::write(&skill_path, &skill).unwrap();
        let block = ContextDelivery {
            skill_file: Some(skill_path),
            references,
            ..delivery(repo.path())
        }
        .block(ContextMoment::Compact)
        .unwrap();
        let manifest = fs::read_to_string(block.manifest_path).unwrap();
        assert!(manifest.len() < HOOK_CONTEXT_BYTES);
        assert!(manifest.len() + skill.len() > HOOK_CONTEXT_BYTES);
        assert!(block.text.len() <= HOOK_CONTEXT_BYTES);
        assert!(block.text.contains(&skill));
        assert!(!block.text.contains("Complete file listing (UTF-8 bytes)"));
        assert!(!block.text.contains("Listed reference, not preloaded"));
        let manifest: Value = serde_json::from_str(&manifest).unwrap();
        assert_eq!(manifest["files"].as_array().unwrap().len(), 21);
    }

    #[test]
    fn context_block_reintroduces_saved_skill_only_after_compaction() {
        let repo = tempdir().unwrap();
        let skill_path = repo.path().join("active.md");
        let skill = "Use $native exactly.\nKeep final whitespace. \t\n";
        fs::write(&skill_path, skill).unwrap();
        for (moment, included) in [
            (ContextMoment::Start, false),
            (ContextMoment::Compact, true),
        ] {
            let block = ContextDelivery {
                skill_file: Some(skill_path.clone()),
                ..delivery(repo.path())
            }
            .block(moment)
            .unwrap();
            assert_eq!(block.text.contains(skill), included);
            assert!(block.text.len() <= HOOK_CONTEXT_BYTES);
        }
    }

    #[test]
    fn context_block_counts_reference_escaping_before_selecting_whole_files() {
        let repo = tempdir().unwrap();
        let source = "$native ".repeat(1_000);
        let block = render_block(
            repo.path(),
            None,
            ContextMoment::Start,
            None,
            &[],
            vec![
                document("scratch/dollars.md", &source, DocumentSource::Scratch),
                document(
                    "scratch/small.md",
                    "Read $native as reference, not a skill.",
                    DocumentSource::Scratch,
                ),
            ],
        )
        .unwrap();
        assert!(source.len() < HOOK_CONTEXT_BYTES);
        assert!(render_reference(&source).len() > HOOK_CONTEXT_BYTES);
        assert!(!block.text.contains("&#36;native &#36;native"));
        assert!(block
            .text
            .contains("Read &#36;native as reference, not a skill."));
        assert!(block.text.len() <= HOOK_CONTEXT_BYTES);
    }
}
