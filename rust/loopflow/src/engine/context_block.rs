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

/// Read current scratch and SQLite-owned Wave/ancestor documents on every call.
/// `skill_file` contains the saved active skill, not a newly resolved definition.
/// `references` point to complete saved briefs, steers, clipboard or summaries.
/// They are listed, not preloaded. This function never changes source documents.
pub fn build_context_block(
    repo_root: &Path,
    wave: Option<&str>,
    moment: ContextMoment,
    skill_file: Option<&Path>,
    references: &[PathBuf],
) -> Result<ContextBlock, CoreError> {
    let repo_root = fs::canonicalize(repo_root)?;
    let mut documents = gather_documents(&GatherSpec {
        repo_root: repo_root.clone(),
        wave: wave.map(str::to_owned),
        ..Default::default()
    })?;
    let branch = crate::engine::git::current_branch(&repo_root)
        .ok()
        .flatten();
    order_documents(&mut documents, wave, branch.as_deref());
    render_block(&repo_root, wave, moment, skill_file, references, documents)
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

    use serde_json::Value;
    use tempfile::tempdir;

    use super::{
        build_context_block, order_documents, render_block, ContextMoment, HOOK_CONTEXT_BYTES,
    };
    use crate::engine::prompt::{render_reference, Document, DocumentSource};

    fn document(path: &str, content: &str, source: DocumentSource) -> Document {
        Document {
            path: path.into(),
            content: content.into(),
            source,
        }
    }

    #[test]
    fn context_block_reads_current_files_without_cutting_large_unicode_documents() {
        let repo = tempdir().unwrap();
        fs::create_dir(repo.path().join("scratch")).unwrap();
        let large = format!("WHOLE_START{}WHOLE_END", "🦀".repeat(50_000));
        fs::write(repo.path().join("scratch/large.md"), &large).unwrap();
        fs::write(repo.path().join("scratch/small.md"), "fresh one").unwrap();
        let first =
            build_context_block(repo.path(), None, ContextMoment::Start, None, &[]).unwrap();
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
        let compact =
            build_context_block(repo.path(), None, ContextMoment::Compact, None, &[]).unwrap();
        assert!(compact.text.contains("fresh two"));
        assert!(!compact.text.contains("fresh one"));
    }

    #[test]
    fn context_block_reports_unreadable_text_instead_of_claiming_a_complete_listing() {
        let repo = tempdir().unwrap();
        fs::create_dir(repo.path().join("scratch")).unwrap();
        fs::write(repo.path().join("scratch/invalid.md"), [0xff, 0xfe]).unwrap();
        let error =
            build_context_block(repo.path(), None, ContextMoment::Start, None, &[]).unwrap_err();
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
        let block = build_context_block(
            repo.path(),
            None,
            ContextMoment::Compact,
            Some(&skill_path),
            &[],
        )
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
        let block = build_context_block(
            repo.path(),
            None,
            ContextMoment::Compact,
            Some(&skill_path),
            &references,
        )
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
            let block =
                build_context_block(repo.path(), None, moment, Some(&skill_path), &[]).unwrap();
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
