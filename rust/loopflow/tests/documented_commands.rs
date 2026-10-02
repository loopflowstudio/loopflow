//! Literal command ownership is checked against the live Clap tree, never a copied catalog.
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

fn literal_path_error(args: &[String], repo: &Path) -> Option<String> {
    let tree = loopflow::lf::navigation::command_tree();
    let mut command = &tree;
    for name in args.iter().skip(1) {
        if name.starts_with('-') || name.contains('<') || name.contains('$') {
            break;
        }
        if let Some(child) = command.find_subcommand(name) {
            command = child;
        } else if command.get_name() == "lf" {
            // Authored names and illustrative placeholders are valid at root;
            // an actual descendant command needs its literal owner.
            fn descendant(command: &clap::Command, name: &str) -> bool {
                command
                    .get_subcommands()
                    .any(|child| child.get_name() == name || descendant(child, name))
            }
            if descendant(&tree, name)
                && loopflow::engine::target::resolve_definition(repo, name, None).is_err()
            {
                return Some(format!("{name} requires its command owner"));
            }
            break;
        } else if command.get_subcommands().next().is_some()
            && command.get_positionals().next().is_none()
            && !command.is_allow_external_subcommands_set()
        {
            return Some(format!(
                "{} has no immediate child {name}",
                command.get_name()
            ));
        } else {
            break;
        }
    }
    None
}

#[test]
fn documented_commands_use_literal_paths() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = vec![repo.join("README.md"), repo.join("AGENTS.md")];
    for directory in ["docs", "rust/loopflow/src/engine/builtins", "skills"] {
        markdown_files(&repo.join(directory), &mut files);
    }
    let mut failures = Vec::new();
    let mut checked = 0;
    for path in files {
        for (line, example) in examples(&fs::read_to_string(&path).unwrap()) {
            let mut args = words(example);
            if args.get(1).is_some_and(|word| word == "help") {
                args.remove(1);
            }
            if let Some(error) = literal_path_error(&args, &repo) {
                failures.push(format!("{}:{line}: {example}: {error}", path.display()));
            }
            checked += 1;
        }
    }
    assert!(checked > 200);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn wrapped_inline_commands_retain_literal_paths() {
    let text = "Read `lf\npr land` or `lf help\npr land`.\n";
    let extracted = examples(text);
    assert_eq!(extracted, [(1, "lf\npr land"), (2, "lf help\npr land")]);
    for (_, example) in extracted {
        let args = words(example);
        assert_eq!(normalize_args(args.clone()), args);
    }
}
