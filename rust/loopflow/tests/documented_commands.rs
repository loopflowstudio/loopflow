//! Command ambiguity is checked against the live Clap tree, never a copied catalog.
use std::fs;
use std::path::{Path, PathBuf};

use loopflow::lf::navigation::normalize_args;
use regex::Regex;

fn markdown_files(path: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            markdown_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "md") {
            files.push(path);
        }
    }
}

// Only tokenize example text. Never execute it, expand a variable, or launch a provider.
fn words(example: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut escaped = false;
    for ch in example.chars() {
        if escaped {
            word.push(ch);
            escaped = false;
        } else if ch == '\\' && quote != Some('\'') {
            escaped = true;
        } else if quote == Some(ch) {
            quote = None;
        } else if quote.is_none() && matches!(ch, '\'' | '"') {
            quote = Some(ch);
        } else if quote.is_none() && ch == '#' {
            break;
        } else if quote.is_none() && ch.is_whitespace() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
        } else {
            word.push(ch);
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

fn examples(text: &str) -> Vec<(usize, &str)> {
    let invocation = Regex::new(r"(?m)(?:^|[\s`])(?P<command>lf\s+)").unwrap();
    invocation
        .captures_iter(text)
        .filter_map(|capture| {
            let command = capture.name("command").unwrap();
            let start = command.start();
            let inline = text[..start].ends_with('`');
            if !inline && command.as_str().contains('\n') {
                return None;
            }
            let example = if inline {
                text[start..].split('`').next().unwrap()
            } else {
                text[start..].split(['`', '\n']).next().unwrap()
            };
            Some((
                text[..start].bytes().filter(|byte| *byte == b'\n').count() + 1,
                example,
            ))
        })
        .collect()
}

#[test]
fn documented_invocations_have_no_ambiguous_command_shorthand() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = vec![repo.join("README.md"), repo.join("AGENTS.md")];
    for directory in ["docs", "rust/loopflow/src/builtins", "skills"] {
        markdown_files(&repo.join(directory), &mut files);
    }
    files.sort();
    let mut failures = Vec::new();
    let mut checked = 0;
    for path in files {
        let text = fs::read_to_string(&path).unwrap();
        for (line_number, example) in examples(&text) {
            let mut args = words(example);
            // Help resolves its path in the inspection layer after normalization.
            if args.get(1).is_some_and(|word| word == "help") {
                args.remove(1);
            }
            let result = normalize_args(args);
            // The reference deliberately demonstrates ambiguity and its error.
            let expect_ambiguous = example.contains("# lf-doc: ambiguous");
            if result.is_err() != expect_ambiguous {
                failures.push(format!(
                    "{}:{}: {example}\n{}",
                    path.strip_prefix(&repo).unwrap().display(),
                    line_number,
                    result
                        .err()
                        .map(|error| error.to_string())
                        .unwrap_or_else(|| {
                            "expected the documented ambiguity, but command now resolves".into()
                        })
                ));
            }
            checked += 1;
        }
    }
    assert!(
        checked > 200,
        "documentation scanner found only {checked} examples"
    );
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn another_owner_makes_a_documented_shortcut_fail() {
    use loopflow::lf::navigation::resolve_child;
    let tree = clap::Command::new("lf").subcommand(
        clap::Command::new("task")
            .subcommand(clap::Command::new("pr").subcommand(clap::Command::new("land"))),
    );
    assert_eq!(
        resolve_child(&tree, "land", &[]).unwrap().unwrap(),
        ["task", "pr", "land"]
    );
    let changed =
        tree.subcommand(clap::Command::new("another").subcommand(clap::Command::new("land")));
    assert!(resolve_child(&changed, "land", &[]).is_err());
    assert_eq!(
        resolve_child(
            changed.find_subcommand("task").unwrap(),
            "land",
            &["task".into()]
        )
        .unwrap()
        .unwrap(),
        ["pr", "land"]
    );
}

#[test]
fn wrapped_inline_commands_are_checked_for_ambiguity() {
    let text = "Read `lf\nstatus` or `lf help\nstatus`.\n\n```sh\nlf land --help\n```\n";
    let extracted = examples(text);
    assert_eq!(
        extracted,
        [
            (1, "lf\nstatus"),
            (2, "lf help\nstatus"),
            (6, "lf land --help")
        ]
    );
    for (_, example) in &extracted[..2] {
        let mut args = words(example);
        if args[1] == "help" {
            args.remove(1);
        }
        assert!(normalize_args(args).is_err(), "{example}");
    }
}
