use std::fs;
use std::path::{Path, PathBuf};

use loopflow::engine::{
    format_prompt, gather_context, GatherContextOpts, PromptFormatMode, Surface,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GoldenCase {
    name: String,
    repo: String,
    skill: Option<String>,
    surface: Option<Surface>,
    directions: Vec<String>,
    docs: Vec<String>,
    diff_files: bool,
    diff: bool,
    clipboard: bool,
    #[serde(default)]
    no_loopflow: bool,
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
    // Goldens are hermetic fixture renders: a run inside a managed wave
    // test process (workers run this suite) must not leak ambient Wave context
    // into them. Safe to set here — this binary runs exactly one test.
    std::env::remove_var("LF_WAVE_ID");

    let root = repo_root();
    for case_path in load_cases() {
        let yaml = fs::read_to_string(&case_path).expect("read golden yaml");
        let case: GoldenCase = serde_yaml_ng::from_str(&yaml).expect("parse golden yaml");

        let repo = root.join(&case.repo);
        let wave_memory = case
            .wave
            .as_deref()
            .and_then(|wave| loopflow::work::wave::context::gather_wave_memory(&repo, wave));
        let opts = GatherContextOpts {
            repo_root: repo.clone(),
            skill: case.skill.clone(),
            message: None,
            operate: !case.no_loopflow,
            surface: case.surface.unwrap_or_default(),
            directions: case.directions.clone(),
            docs: case.docs.clone(),
            files: Vec::new(),
            include_diff: case.diff,
            include_diff_files: case.diff_files,
            include_clipboard: case.clipboard,
            wave: case.wave.clone(),
            wave_memory,
            related_repos: Vec::new(),
        };

        let gathered = gather_context(&opts).expect("gather context");
        let prompt = format_prompt(PromptFormatMode::Full, gathered.components()).into_string();
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
