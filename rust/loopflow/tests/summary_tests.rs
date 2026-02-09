use std::fs;
use std::path::Path;

use loopflow::engine::config::SummaryConfig;
use loopflow::engine::prompt::{
    compute_source_hash, count_area_tokens, is_summary_fresh, load_summaries,
    parse_summary_frontmatter, write_summary,
};
use tempfile::TempDir;

fn init_repo(dir: &Path) {
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .expect("git init");
}

fn write_source_file(repo: &Path, rel_path: &str, content: &str) {
    let full = repo.join(rel_path);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, content).unwrap();
}

fn write_summary_file(repo: &Path, area_path: &str, source_hash: &str, body: &str) {
    let summaries_dir = repo.join(".lf/summaries");
    fs::create_dir_all(&summaries_dir).unwrap();

    let normalized = area_path.trim_end_matches('/');
    let hash = sha2_hex(normalized);
    let content = format!(
        "---\npath: {}\nsource_hash: {}\ntokens: 100\ngenerated_at: 2026-01-01T00:00:00Z\nmodel: gemini\n---\n{}",
        area_path, source_hash, body
    );
    fs::write(summaries_dir.join(format!("{}.md", hash)), content).unwrap();
}

fn sha2_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

// =============================================================================
// load_summaries
// =============================================================================

#[test]
fn load_summaries_returns_empty_when_no_dir() {
    let temp = TempDir::new().unwrap();
    let summaries = vec![SummaryConfig {
        path: "src/".to_string(),
        tokens: Some(5000),
        model: "gemini".to_string(),
    }];
    let result = load_summaries(temp.path(), &summaries);
    assert!(result.is_empty());
}

#[test]
fn load_summaries_returns_empty_when_no_configs() {
    let temp = TempDir::new().unwrap();
    let result = load_summaries(temp.path(), &[]);
    assert!(result.is_empty());
}

#[test]
fn load_summaries_returns_documents_from_cache() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    write_summary_file(
        repo,
        "src/engine/",
        "abc123",
        "# Engine Summary\nCore types.",
    );

    let summaries = vec![SummaryConfig {
        path: "src/engine/".to_string(),
        tokens: Some(5000),
        model: "gemini".to_string(),
    }];

    let result = load_summaries(repo, &summaries);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].path, "src/engine/");
    assert_eq!(result[0].category, "summaries");
    assert!(result[0].content.contains("Engine Summary"));
    // Frontmatter should be stripped
    assert!(!result[0].content.contains("source_hash"));
}

#[test]
fn load_summaries_skips_missing_files() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    // Create summaries dir but only for one area
    write_summary_file(repo, "src/api/", "abc", "# API Summary");

    let summaries = vec![
        SummaryConfig {
            path: "src/api/".to_string(),
            tokens: Some(5000),
            model: "gemini".to_string(),
        },
        SummaryConfig {
            path: "src/missing/".to_string(),
            tokens: Some(5000),
            model: "gemini".to_string(),
        },
    ];

    let result = load_summaries(repo, &summaries);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].path, "src/api/");
}

// =============================================================================
// parse_summary_frontmatter
// =============================================================================

#[test]
fn parse_frontmatter_extracts_metadata() {
    let content = "---\npath: src/engine/\nsource_hash: abc123\ntokens: 4200\n---\n# Summary";
    let meta = parse_summary_frontmatter(content);
    assert_eq!(meta.get("path").unwrap(), "src/engine/");
    assert_eq!(meta.get("source_hash").unwrap(), "abc123");
    assert_eq!(meta.get("tokens").unwrap(), "4200");
}

#[test]
fn parse_frontmatter_returns_empty_for_no_frontmatter() {
    let content = "# Just a heading\nSome text.";
    let meta = parse_summary_frontmatter(content);
    assert!(meta.is_empty());
}

// =============================================================================
// compute_source_hash
// =============================================================================

#[test]
fn compute_source_hash_deterministic() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    write_source_file(repo, "src/main.rs", "fn main() {}");
    write_source_file(repo, "src/lib.rs", "pub mod engine;");

    let hash1 = compute_source_hash(repo, "src/").unwrap();
    let hash2 = compute_source_hash(repo, "src/").unwrap();
    assert_eq!(hash1, hash2);
    assert!(!hash1.is_empty());
}

#[test]
fn compute_source_hash_changes_on_content_change() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    write_source_file(repo, "src/main.rs", "fn main() {}");
    let hash1 = compute_source_hash(repo, "src/").unwrap();

    write_source_file(repo, "src/main.rs", "fn main() { println!(\"hello\"); }");
    let hash2 = compute_source_hash(repo, "src/").unwrap();

    assert_ne!(hash1, hash2);
}

#[test]
fn compute_source_hash_empty_for_missing_path() {
    let temp = TempDir::new().unwrap();
    let hash = compute_source_hash(temp.path(), "nonexistent/").unwrap();
    assert!(hash.is_empty());
}

// =============================================================================
// is_summary_fresh
// =============================================================================

#[test]
fn is_summary_fresh_returns_false_when_no_cache() {
    let temp = TempDir::new().unwrap();
    assert!(!is_summary_fresh(temp.path(), "src/").unwrap());
}

#[test]
fn is_summary_fresh_returns_true_when_hash_matches() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    write_source_file(repo, "src/main.rs", "fn main() {}");
    let hash = compute_source_hash(repo, "src/").unwrap();
    write_summary_file(repo, "src/", &hash, "# Summary");

    assert!(is_summary_fresh(repo, "src/").unwrap());
}

#[test]
fn is_summary_fresh_returns_false_when_hash_mismatches() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    write_source_file(repo, "src/main.rs", "fn main() {}");
    write_summary_file(repo, "src/", "stale_hash", "# Summary");

    assert!(!is_summary_fresh(repo, "src/").unwrap());
}

// =============================================================================
// count_area_tokens
// =============================================================================

#[test]
fn count_area_tokens_returns_zero_for_missing_path() {
    let temp = TempDir::new().unwrap();
    let tokens = count_area_tokens(temp.path(), "nonexistent/").unwrap();
    assert_eq!(tokens, 0);
}

#[test]
fn count_area_tokens_counts_file_content() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);

    write_source_file(
        repo,
        "src/main.rs",
        "fn main() { println!(\"hello world\"); }",
    );
    let tokens = count_area_tokens(repo, "src/").unwrap();
    assert!(tokens > 0);
}

// =============================================================================
// write_summary
// =============================================================================

#[test]
fn write_summary_creates_file_with_frontmatter() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    write_summary(
        repo,
        "src/engine/",
        "abc123",
        500,
        "gemini",
        "# Engine\nCore types.",
    )
    .unwrap();

    let hash = sha2_hex("src/engine");
    let path = repo.join(".lf/summaries").join(format!("{}.md", hash));
    assert!(path.exists());

    let content = fs::read_to_string(&path).unwrap();
    assert!(content.starts_with("---"));
    assert!(content.contains("path: src/engine/"));
    assert!(content.contains("source_hash: abc123"));
    assert!(content.contains("tokens: 500"));
    assert!(content.contains("model: gemini"));
    assert!(content.contains("# Engine\nCore types."));
}

// =============================================================================
// Integration: summaries appear in formatted prompt
// =============================================================================

#[test]
fn summaries_appear_in_formatted_prompt() {
    use loopflow::engine::prompt::Document;
    use loopflow::engine::{format_prompt, PromptComponents};

    let components = PromptComponents {
        summaries: vec![Document {
            path: "src/engine/".to_string(),
            content: "# Engine Summary\nCore prompt assembly.".to_string(),
            category: "summaries".to_string(),
        }],
        ..Default::default()
    };

    let prompt = format_prompt(&components);
    assert!(prompt.contains("<lf:summaries>"));
    assert!(prompt.contains("<lf:summary path=\"src/engine/\">"));
    assert!(prompt.contains("Engine Summary"));
    assert!(prompt.contains("</lf:summary>"));
    assert!(prompt.contains("</lf:summaries>"));
}
