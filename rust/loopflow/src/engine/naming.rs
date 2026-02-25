use crate::engine::config::BranchNameConfig;
use crate::engine::error::GitError;
use chrono::Local;
use rand::prelude::IndexedRandom;
use rand::Rng;
use regex::Regex;
use std::path::Path;
use std::process::Command;

const MAGICAL: &[&str] = &[
    "aurora", "cascade", "crystal", "drift", "echo", "ember", "fern", "flume", "frost", "glade",
    "grove", "haze", "ivy", "jade", "luna", "mist", "nova", "opal", "petal", "prism", "rain",
    "ripple", "sage", "shade", "spark", "star", "stone", "storm", "tide", "vale", "wave", "wisp",
    "wren", "zephyr",
];

const MUSICAL: &[&str] = &[
    "allegro", "aria", "ballad", "cadence", "canon", "chord", "coda", "duet", "forte", "fugue",
    "harmony", "hymn", "lilt", "lyric", "melody", "motif", "opus", "prelude", "refrain", "rondo",
    "sonata", "tempo", "trill", "tune", "verse", "waltz",
];

/// Parsed components of a branch name according to the naming schema.
#[derive(Debug, Clone, PartialEq)]
pub struct BranchNameParts {
    pub user: Option<String>,
    pub name: String,
    pub timestamp: Option<String>,
    pub words: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum BranchSchemaToken {
    Literal(String),
    Placeholder(String),
}

fn parse_schema_tokens(schema: &str) -> Option<Vec<BranchSchemaToken>> {
    let mut tokens = Vec::new();
    let mut remaining = schema;

    while let Some(open_index) = remaining.find('{') {
        let (literal, after_literal) = remaining.split_at(open_index);
        if !literal.is_empty() {
            tokens.push(BranchSchemaToken::Literal(literal.to_string()));
        }

        let after_open = &after_literal[1..];
        let close_index = after_open.find('}')?;
        let (placeholder, after_placeholder) = after_open.split_at(close_index);
        if placeholder.is_empty() {
            return None;
        }
        tokens.push(BranchSchemaToken::Placeholder(placeholder.to_string()));
        remaining = &after_placeholder[1..];
    }

    if !remaining.is_empty() {
        tokens.push(BranchSchemaToken::Literal(remaining.to_string()));
    }

    tokens
        .iter()
        .any(|token| matches!(token, BranchSchemaToken::Placeholder(_)))
        .then_some(tokens)
}

fn placeholder_pattern(name: &str) -> &'static str {
    match name {
        "user" => "[a-z0-9_-]+",
        "name" => "[a-z0-9._-]+",
        "timestamp" | "ts" => "\\d{8}_\\d{4}",
        "date" => "\\d{8}",
        "words" => "[a-z]+-[a-z]+",
        _ => "[a-z0-9._-]+",
    }
}

fn build_schema_regex(tokens: &[BranchSchemaToken]) -> Option<(Regex, Vec<String>)> {
    let mut pattern = String::from("^");
    let mut placeholders = Vec::new();

    for token in tokens {
        match token {
            BranchSchemaToken::Literal(literal) => pattern.push_str(&regex::escape(literal)),
            BranchSchemaToken::Placeholder(name) => {
                pattern.push('(');
                pattern.push_str(placeholder_pattern(name));
                pattern.push(')');
                placeholders.push(name.clone());
            }
        }
    }
    pattern.push('$');

    let regex = Regex::new(&pattern).ok()?;
    Some((regex, placeholders))
}

fn strip_trailing_optional_segment(tokens: &[BranchSchemaToken]) -> Option<Vec<BranchSchemaToken>> {
    let mut trimmed = tokens.to_vec();
    let trailing = match trimmed.last() {
        Some(BranchSchemaToken::Placeholder(name)) => name.as_str(),
        _ => return None,
    };

    if trailing != "words" {
        return None;
    }

    trimmed.pop();
    if matches!(trimmed.last(), Some(BranchSchemaToken::Literal(_))) {
        trimmed.pop();
    }
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed)
}

fn schema_variants(tokens: &[BranchSchemaToken]) -> Vec<Vec<BranchSchemaToken>> {
    let mut variants = vec![tokens.to_vec()];
    let mut current = tokens.to_vec();

    while let Some(next) = strip_trailing_optional_segment(&current) {
        variants.push(next.clone());
        current = next;
    }

    variants
}

fn captures_to_branch_parts(
    captures: &regex::Captures<'_>,
    placeholders: &[String],
) -> Option<BranchNameParts> {
    let mut user = None;
    let mut name = None;
    let mut timestamp = None;
    let mut words = None;

    for (index, placeholder) in placeholders.iter().enumerate() {
        let value = captures.get(index + 1)?.as_str().to_string();
        match placeholder.as_str() {
            "user" => user = Some(value),
            "name" => name = Some(value),
            "timestamp" | "ts" | "date" => timestamp = Some(value),
            "words" => words = Some(value),
            _ => {}
        }
    }

    Some(BranchNameParts {
        user,
        name: name?,
        timestamp,
        words,
    })
}

/// Reverse-parse a branch name using the configured schema.
/// Returns None if the branch doesn't match the schema pattern.
pub fn parse_branch_name(
    branch: &str,
    config: Option<&BranchNameConfig>,
) -> Option<BranchNameParts> {
    let default = BranchNameConfig::default();
    let config = config.unwrap_or(&default);
    let tokens = parse_schema_tokens(config.schema_.as_str())?;

    schema_variants(&tokens).into_iter().find_map(|variant| {
        let (regex, placeholders) = build_schema_regex(&variant)?;
        let captures = regex.captures(branch)?;
        captures_to_branch_parts(&captures, &placeholders)
    })
}

/// Extract the wave name ({name} component) from a branch name.
pub fn wave_name(branch: &str, config: Option<&BranchNameConfig>) -> Option<String> {
    parse_branch_name(branch, config).map(|parts| parts.name)
}

pub fn sanitize_for_branch(value: &str) -> String {
    let mut out = String::new();
    let mut last_was_dash = false;
    for ch in value.chars() {
        let lowered = ch.to_ascii_lowercase();
        let keep =
            lowered.is_ascii_alphanumeric() || lowered == '-' || lowered == '_' || lowered == '.';
        if keep {
            out.push(lowered);
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    let mut collapsed = String::new();
    let mut prev_dash = false;
    for ch in trimmed.chars() {
        if ch == '-' {
            if !prev_dash {
                collapsed.push(ch);
            }
            prev_dash = true;
        } else {
            collapsed.push(ch);
            prev_dash = false;
        }
    }
    if collapsed.is_empty() {
        "user".to_string()
    } else {
        collapsed
    }
}

fn git_username(repo: &Path) -> Result<String, GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["config", "user.name"])
        .output()?;
    if output.status.success() {
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !name.is_empty() {
            return Ok(sanitize_for_branch(&name));
        }
    }
    let fallback = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    Ok(sanitize_for_branch(&fallback))
}

fn generate_word_pair_with_rng<R: Rng + ?Sized>(rng: &mut R) -> String {
    let magical = MAGICAL.choose(rng).copied().unwrap_or("wisp");
    let musical = MUSICAL.choose(rng).copied().unwrap_or("forte");
    format!("{magical}-{musical}")
}

pub fn generate_word_pair() -> String {
    let mut rng = rand::rng();
    generate_word_pair_with_rng(&mut rng)
}

fn generate_timestamp() -> String {
    Local::now().format("%Y%m%d_%H%M").to_string()
}

fn generate_date() -> String {
    Local::now().format("%Y%m%d").to_string()
}

pub fn format_branch_name(
    short_name: &str,
    config: Option<&BranchNameConfig>,
    repo: &Path,
) -> Result<String, GitError> {
    let default = BranchNameConfig::default();
    let config = config.unwrap_or(&default);

    let schema = config.schema_.as_str();
    if schema == "{name}" {
        return Ok(sanitize_for_branch(short_name));
    }

    let user = git_username(repo)?;
    let name = sanitize_for_branch(short_name);
    let ts = generate_timestamp();
    let date = generate_date();
    let words = generate_word_pair();

    let mut result = schema.replace("{name}", &name);
    result = result.replace("{user}", &user);
    result = result.replace("{timestamp}", &ts);
    result = result.replace("{ts}", &ts);
    result = result.replace("{date}", &date);
    result = result.replace("{words}", &words);

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use loopflow_test_support::TestRepo;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn sanitize_for_branch_cleans_input() {
        let cleaned = sanitize_for_branch("Jack Heart!!!");
        assert_eq!(cleaned, "jack-heart");
    }

    #[test]
    fn sanitize_removes_special_chars() {
        let cleaned = sanitize_for_branch("feat/my thing!");
        assert_eq!(cleaned, "feat-my-thing");
    }

    #[test]
    fn sanitize_collapses_hyphens() {
        let cleaned = sanitize_for_branch("a---b");
        assert_eq!(cleaned, "a-b");
    }

    #[test]
    fn sanitize_trims_leading_trailing() {
        let cleaned = sanitize_for_branch("-foo-");
        assert_eq!(cleaned, "foo");
    }

    #[test]
    fn parse_branch_name_parses_default_schema_with_words() {
        let parts = parse_branch_name("jack-heart.mobile.20260225_1122.aurora-fugue", None)
            .expect("parse branch");
        assert_eq!(parts.user.as_deref(), Some("jack-heart"));
        assert_eq!(parts.name, "mobile");
        assert_eq!(parts.timestamp.as_deref(), Some("20260225_1122"));
        assert_eq!(parts.words.as_deref(), Some("aurora-fugue"));
    }

    #[test]
    fn parse_branch_name_allows_missing_words_for_default_schema() {
        let parts = parse_branch_name("jack-heart.mobile.20260225_1122", None).expect("parse");
        assert_eq!(parts.user.as_deref(), Some("jack-heart"));
        assert_eq!(parts.name, "mobile");
        assert_eq!(parts.timestamp.as_deref(), Some("20260225_1122"));
        assert_eq!(parts.words, None);
    }

    #[test]
    fn parse_branch_name_supports_dots_in_name() {
        let parts =
            parse_branch_name("jack-heart.mobile.feature.20260225_1122", None).expect("parse");
        assert_eq!(parts.name, "mobile.feature");
        assert_eq!(parts.timestamp.as_deref(), Some("20260225_1122"));
    }

    #[test]
    fn parse_branch_name_returns_none_for_non_matching_branch() {
        assert_eq!(parse_branch_name("mobile", None), None);
    }

    #[test]
    fn parse_branch_name_supports_custom_schema() {
        let config = BranchNameConfig {
            schema_: "{name}".to_string(),
        };
        let parts = parse_branch_name("mobile", Some(&config)).expect("parse");
        assert_eq!(parts.user, None);
        assert_eq!(parts.name, "mobile");
        assert_eq!(parts.timestamp, None);
        assert_eq!(parts.words, None);
    }

    #[test]
    fn wave_name_extracts_wave_component() {
        let name = wave_name("jack-heart.mobile.20260225_1122", None).expect("wave name");
        assert_eq!(name, "mobile");
    }

    #[test]
    fn wave_name_returns_none_when_parse_fails() {
        assert_eq!(wave_name("mobile", None), None);
    }

    #[test]
    fn format_branch_name_uses_default_schema_without_config() {
        let repo = TestRepo::new();
        let name = format_branch_name("feature", None, repo.path()).expect("format");
        assert!(name.contains("feature"), "should include the short name");
        assert!(
            name.contains('.'),
            "should use the default schema with separators"
        );
        assert_ne!(name, "feature", "should not pass through raw name");
    }

    #[test]
    fn format_branch_name_with_schema() {
        let repo = TestRepo::new();
        let config = BranchNameConfig {
            schema_: "{user}/{words}".to_string(),
        };
        let name = format_branch_name("feature", Some(&config), repo.path()).expect("format");
        assert!(name.starts_with("jack/"));
        let suffix = name.trim_start_matches("jack/");
        assert!(suffix.contains('-'));
    }

    #[test]
    fn format_branch_name_with_timestamp() {
        let repo = TestRepo::new();
        let config = BranchNameConfig {
            schema_: "{ts}".to_string(),
        };
        let name = format_branch_name("feature", Some(&config), repo.path()).expect("format");
        let parts: Vec<&str> = name.split('_').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].len(), 8);
        assert_eq!(parts[1].len(), 4);
    }

    #[test]
    fn format_branch_name_with_date() {
        let repo = TestRepo::new();
        let config = BranchNameConfig {
            schema_: "{date}".to_string(),
        };
        let name = format_branch_name("feature", Some(&config), repo.path()).expect("format");
        assert_eq!(name.len(), 8);
        assert!(name.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn format_branch_name_with_name() {
        let repo = TestRepo::new();
        let config = BranchNameConfig {
            schema_: "{name}".to_string(),
        };
        let name = format_branch_name("My Feature!", Some(&config), repo.path()).expect("format");
        assert_eq!(name, "my-feature");
    }

    #[test]
    fn generate_word_pair_is_deterministic_with_rng() {
        let mut rng = StdRng::seed_from_u64(42);
        let pair = generate_word_pair_with_rng(&mut rng);
        assert!(!pair.is_empty());
        assert!(pair.contains('-'));
    }

    #[test]
    fn word_pairs_are_two_words() {
        let mut rng = StdRng::seed_from_u64(7);
        let pair = generate_word_pair_with_rng(&mut rng);
        let parts: Vec<&str> = pair.split('-').collect();
        assert_eq!(parts.len(), 2);
        assert!(parts.iter().all(|p| !p.is_empty()));
    }

    #[test]
    fn word_pairs_vary_with_different_seeds() {
        let mut rng_a = StdRng::seed_from_u64(1);
        let mut rng_b = StdRng::seed_from_u64(2);
        let a = generate_word_pair_with_rng(&mut rng_a);
        let b = generate_word_pair_with_rng(&mut rng_b);
        assert_ne!(a, b);
    }
}
