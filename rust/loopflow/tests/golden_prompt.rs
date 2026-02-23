use std::fs;
use std::path::{Path, PathBuf};

use loopflow::engine::{
    default_gather_sources, format_prompt, gather_context, GatherContextOpts, PromptFormatMode,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GoldenCase {
    name: String,
    repo: String,
    step: Option<String>,
    run_mode: Option<String>,
    directions: Vec<String>,
    lfdocs: bool,
    diff_files: bool,
    diff: bool,
    clipboard: bool,
    area: Option<String>,
    wave: Option<String>,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn load_cases() -> Vec<PathBuf> {
    let root = repo_root().join("tests/goldens");
    let mut cases = Vec::new();
    let entries = fs::read_dir(&root).expect("read goldens directory");
    for entry in entries {
        let entry = entry.expect("read golden entry");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("yaml") {
            cases.push(path);
        }
    }
    cases.sort();
    cases
}

fn normalize_prompt(prompt: &str, repo: &Path) -> String {
    prompt
        .replace("\r\n", "\n")
        .replace(repo.to_string_lossy().as_ref(), "<REPO>")
        .trim()
        .to_string()
}

#[test]
fn golden_prompts_match_python() {
    let root = repo_root();
    for case_path in load_cases() {
        let yaml = fs::read_to_string(&case_path).expect("read golden yaml");
        let case: GoldenCase = serde_yaml_ng::from_str(&yaml).expect("parse golden yaml");

        let repo = root.join(&case.repo);
        let opts = GatherContextOpts {
            repo_root: repo.clone(),
            step: case.step.clone(),
            message: None,
            run_mode: case.run_mode.clone(),
            directions: case.directions.clone(),
            files: Vec::new(),
            sources: default_gather_sources(
                case.lfdocs,
                case.diff_files || case.diff,
                case.clipboard,
                case.area.as_deref(),
                case.wave.as_deref(),
            ),
            area: case.area.clone(),
            wave: case.wave.clone(),
        };

        let components = gather_context(&opts).expect("gather context");
        let prompt = format_prompt(PromptFormatMode::Full, &components);
        let actual = normalize_prompt(&prompt, &repo);

        let expected_path = case_path.with_extension("md");
        let expected = fs::read_to_string(&expected_path).unwrap_or_else(|_| {
            panic!(
                "missing golden file {} (run tests/goldens/update_goldens.py)",
                expected_path.display()
            )
        });
        let expected = normalize_prompt(&expected, &repo);

        if actual != expected {
            let actual_path = case_path.with_extension("actual.md");
            fs::write(&actual_path, prompt).expect("write actual prompt");
            panic!(
                "golden prompt mismatch for {} (wrote {})",
                case.name,
                actual_path.display()
            );
        }
    }
}
