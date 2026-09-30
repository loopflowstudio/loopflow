use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use clap::Parser;
use loopflow::engine::target::{resolve_definition, DefinitionKind, Target};
use loopflow::engine::{compile_flow, load_flow, ConcreteStep};
use loopflow::lf::navigation::normalize_args;
use loopflow::lf::{Cli, Commands, FlowCommand};
use tempfile::TempDir;

fn fixture() -> TempDir {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
    fs::create_dir_all(repo.path().join(".lf/flows")).unwrap();
    for name in ["solo", "paired", "complete", "land", "release-run"] {
        fs::write(
            repo.path().join(format!(".lf/skills/{name}.md")),
            format!("Skill {name} body."),
        )
        .unwrap();
    }
    for name in ["paired", "only-flow", "release-run", "list"] {
        fs::write(
            repo.path().join(format!(".lf/flows/{name}.yaml")),
            "- step: solo\n",
        )
        .unwrap();
    }
    repo
}

fn run(repo: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("HOME", home)
        .env("LF_HOME", home.join(".lf"))
        .env("LF_DB_PATH", home.join(".lf/store.db"))
        .env("NO_COLOR", "1")
        // Inspection must work without git, providers, npx, or credentials.
        .env("PATH", home.join("no-tools"))
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap()
}

fn success(output: Output) -> Vec<u8> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[test]
fn inspection_is_identical_across_spellings_and_has_no_launch_side_effects() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    for forms in [
        vec![
            vec!["help", "debug"],
            vec!["debug", "--help"],
            vec!["run", "debug", "--help"],
        ],
        vec![vec!["help", "flow", "list"], vec!["flow", "list", "--help"]],
        vec![
            vec!["help", "land"],
            vec!["land", "--help"],
            vec!["pr", "land", "--help"],
        ],
        vec![
            vec!["help", "auth", "show"],
            vec!["auth", "route", "show", "--help"],
        ],
        vec![
            vec!["help", "paired"],
            vec!["paired", "--help"],
            vec!["run", "paired", "-m", "unused", "--help"],
        ],
    ] {
        let expected = success(run(repo.path(), home.path(), &forms[0]));
        for form in &forms[1..] {
            assert_eq!(
                success(run(repo.path(), home.path(), form)),
                expected,
                "{form:?}"
            );
        }
    }
    let unknown = run(repo.path(), home.path(), &["help", "auth", "absent-child"]);
    let unknown_flag = run(
        repo.path(),
        home.path(),
        &["auth", "absent-child", "--help"],
    );
    assert_eq!(unknown.status.code(), Some(2));
    assert_eq!(unknown_flag.status.code(), Some(2));
    assert_eq!(unknown.stderr, unknown_flag.stderr);
    let collision = success(run(repo.path(), home.path(), &["help", "release-run"]));
    let collision = String::from_utf8(collision).unwrap();
    assert!(collision.contains("flow (.lf/flows/release-run.yaml)"));
    assert!(collision.contains("lf skill release-run"));
    assert!(collision.contains("flow wins untyped lookup"));
    let skill = success(run(repo.path(), home.path(), &["skill", "show", "paired"]));
    assert!(String::from_utf8_lossy(&skill).contains("Skill paired body."));
    let uncached = success(run(
        repo.path(),
        home.path(),
        &["run", "npx/no-such-cached-skill", "--help"],
    ));
    assert!(String::from_utf8_lossy(&uncached).contains("not cached locally"));
    let overview = success(run(repo.path(), home.path(), &["--help"]));
    let overview = String::from_utf8(overview).unwrap();
    assert!(overview.starts_with("Usage: lf "));
    assert!(overview.lines().count() <= 25, "{overview}");
    assert!(
        !home.path().join(".lf").exists(),
        "inspection wrote runtime state"
    );
}

#[test]
fn skill_help_does_not_offer_untyped_execution_when_the_flow_is_invalid() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    fs::write(repo.path().join(".lf/flows/paired.yaml"), "- [invalid\n").unwrap();

    let help = success(run(repo.path(), home.path(), &["skill", "show", "paired"]));
    let help = String::from_utf8(help).unwrap();
    assert!(help.contains("lf skill paired [message]"), "{help}");
    assert!(
        help.contains("Untyped lookup fails: invalid flow"),
        "{help}"
    );
    assert!(!help.contains("lf run paired"), "{help}");
    assert!(!help.contains("lf paired [message]"), "{help}");
    let untyped = run(repo.path(), home.path(), &["run", "paired", "--help"]);
    assert_eq!(untyped.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&untyped.stderr).contains("invalid flow"));
}

#[test]
fn list_preserves_kinds_overrides_sources_and_reserved_invocations() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    let output = success(run(repo.path(), home.path(), &["list", "--json"]));
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&output).unwrap();
    let pair: Vec<_> = entries
        .iter()
        .filter(|row| row["name"] == "release-run")
        .collect();
    assert_eq!(pair.len(), 2);
    assert_eq!(pair[0]["kind"], "flow");
    assert_eq!(pair[1]["kind"], "skill");
    assert_eq!(pair[1]["source"], ".lf/skills/release-run.md");
    let reserved = entries
        .iter()
        .find(|row| row["kind"] == "flow" && row["name"] == "list")
        .unwrap();
    assert_eq!(reserved["invocation"], "lf flow -- list");
    assert!(!entries
        .iter()
        .any(|row| row["name"] == "solo" && row["kind"] == "flow"));
}

#[test]
fn ambiguous_commands_never_fall_back_to_installed_definitions() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    for args in [
        &["complete"][..],
        &["complete", "--help"],
        &["help", "complete"],
    ] {
        let result = run(repo.path(), home.path(), args);
        assert_eq!(result.status.code(), Some(2));
        let message = String::from_utf8_lossy(&result.stderr);
        assert!(message.contains("lf session complete"), "{message}");
        assert!(message.contains("lf task complete"), "{message}");
    }
    let selected = success(run(
        repo.path(),
        home.path(),
        &["run", "complete", "--help"],
    ));
    assert!(String::from_utf8_lossy(&selected).contains("Skill complete body."));
    assert!(!home.path().join(".lf").exists());
}

#[test]
fn dual_kind_clients_share_selection_and_explicit_skill_steps_keep_review_metadata() {
    let repo = fixture();
    assert!(matches!(
        resolve_definition(repo.path(), "solo", None).unwrap(),
        Target::Skill(_)
    ));
    for name in ["paired", "only-flow"] {
        assert!(matches!(
            resolve_definition(repo.path(), name, None).unwrap(),
            Target::Flow(_)
        ));
        let expanded = compile_flow(&load_flow(name, repo.path()).unwrap(), repo.path()).unwrap();
        assert!(matches!(&expanded[0], ConcreteStep::Skill(step) if step.skill.name == "solo"));
    }
    assert!(matches!(
        resolve_definition(repo.path(), "paired", Some(DefinitionKind::Skill)).unwrap(),
        Target::Skill(_)
    ));
    assert!(resolve_definition(repo.path(), "solo", Some(DefinitionKind::Flow)).is_err());
    fs::write(
        repo.path().join(".lf/flows/outer.yaml"),
        "- paired\n- flow: paired\n- step:\n    name: paired\n    id: review\n    human: true\n",
    )
    .unwrap();
    let expanded = compile_flow(&load_flow("outer", repo.path()).unwrap(), repo.path()).unwrap();
    assert_eq!(expanded.len(), 3);
    assert!(matches!(&expanded[0], ConcreteStep::Skill(step) if step.skill.name == "solo"));
    assert!(matches!(&expanded[1], ConcreteStep::Skill(step) if step.skill.name == "solo"));
    assert!(
        matches!(&expanded[2], ConcreteStep::Skill(step) if step.skill.name == "paired" && step.human)
    );
    fs::write(repo.path().join(".lf/flows/paired.yaml"), "malformed: [").unwrap();
    assert!(resolve_definition(repo.path(), "paired", None)
        .unwrap_err()
        .to_string()
        .contains("invalid flow"));
    assert!(load_flow("paired", repo.path()).is_err());
    assert!(matches!(
        resolve_definition(repo.path(), "paired", Some(DefinitionKind::Skill)).unwrap(),
        Target::Skill(_)
    ));
}

#[test]
fn command_targets_compose_and_captured_operations_remain_readable() {
    let repo = fixture();
    fs::write(
        repo.path().join(".lf/flows/commands.yaml"),
        "- cmd: pr land --local\n",
    )
    .unwrap();
    let flow = load_flow("commands", repo.path()).unwrap();
    let steps = compile_flow(&flow, repo.path()).unwrap();
    let ConcreteStep::Command(step) = &steps[0] else {
        panic!("expected command")
    };
    assert_eq!(step.item.argv(), ["lf", "pr", "land", "--local"]);
    let adapted = Target::Command(step.item.clone()).into_flow();
    assert_eq!(adapted.items, flow.items);

    let saved = serde_json::json!({"Command": {
        "item": {"command": "pr", "args": ["land", "--local"]},
        "sources": ["commands"]
    }});
    let restored: ConcreteStep = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(restored, steps[0]);
    assert_eq!(serde_json::to_value(restored).unwrap(), saved);

    fs::write(
        repo.path().join(".lf/flows/commands.yaml"),
        "- op: pr land\n",
    )
    .unwrap();
    assert!(load_flow("commands", repo.path())
        .unwrap_err()
        .to_string()
        .contains("cmd"));
}

#[test]
fn shorthand_stops_at_leaf_and_passthrough_boundaries() {
    let normalized =
        |args: &[&str]| normalize_args(args.iter().map(|arg| arg.to_string()).collect()).unwrap();
    assert_eq!(
        normalized(&["lf", "auth", "show"]),
        ["lf", "auth", "route", "show"]
    );
    assert_eq!(
        normalized(&["lf", "land", "--next", "show"]),
        ["lf", "pr", "land", "--next", "show"]
    );
    assert_eq!(
        normalized(&["lf", "task", "comment", "status"]),
        ["lf", "task", "comment", "status"]
    );
    assert_eq!(
        normalized(&["lf", "ssh", "somewhere", "show", "--help"]),
        ["lf", "ssh", "somewhere", "show", "--help"]
    );
    assert_eq!(
        normalized(&["lf", "run", "land", "--", "--help"]),
        ["lf", "run", "land", "--", "--help"]
    );
    let parsed = Cli::try_parse_from(normalized(&["lf", "flow", "--", "list"])).unwrap();
    assert!(
        matches!(parsed.command, Some(Commands::Flow { cmd: FlowCommand::External(args) }) if args == ["list"])
    );
}

#[test]
fn transitive_lookup_counts_canonical_targets_and_respects_exact_aliases() {
    let tree = clap::Command::new("lf")
        .subcommand(
            clap::Command::new("task").subcommand(
                clap::Command::new("pr")
                    .visible_alias("pull-request")
                    .subcommand(clap::Command::new("land")),
            ),
        )
        .subcommand(clap::Command::new("repo").subcommand(clap::Command::new("pr")))
        .subcommand(clap::Command::new("identity").visible_alias("id"))
        .subcommand(clap::Command::new("home").subcommand(clap::Command::new("id")));
    let resolve = |name| loopflow::lf::navigation::resolve_child(&tree, name, &[]);
    assert_eq!(resolve("land").unwrap().unwrap(), ["task", "pr", "land"]);
    assert!(resolve("pr").is_err());
    assert_eq!(resolve("pull-request").unwrap().unwrap(), ["task", "pr"]);
    assert_eq!(resolve("id").unwrap().unwrap(), ["identity"]);
}
