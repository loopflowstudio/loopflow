use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use clap::{CommandFactory, Parser};
use loopflow::engine::target::{resolve_definition, DefinitionKind, Target};
use loopflow::engine::{compile_flow, load_flow, ConcreteStep};
use loopflow::lf::navigation::normalize_args;
use loopflow::lf::{Cli, Commands, FlowCommand, SkillCommand};
use tempfile::TempDir;

fn fixture() -> TempDir {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
    fs::create_dir_all(repo.path().join(".lf/flows")).unwrap();
    for name in ["solo", "paired", "rename", "land", "release-run"] {
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

fn json_entries(repo: &Path, home: &Path, args: &[&str]) -> Vec<serde_json::Value> {
    serde_json::from_slice(&success(run(repo, home, args))).unwrap()
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
        vec![vec!["help", "wt", "create"], vec!["wt", "create", "--help"]],
        vec![vec!["sync", "--help"], vec!["help", "sync"]],
        vec![vec!["help", "pr", "land"], vec!["pr", "land", "--help"]],
        vec![
            vec!["help", "account", "route"],
            vec!["account", "route", "--help"],
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
    let unknown = run(
        repo.path(),
        home.path(),
        &["help", "account", "absent-child"],
    );
    let unknown_flag = run(
        repo.path(),
        home.path(),
        &["account", "absent-child", "--help"],
    );
    assert_eq!(unknown.status.code(), Some(2));
    assert_eq!(unknown_flag.status.code(), Some(2));
    assert_eq!(unknown.stderr, unknown_flag.stderr);
    let collision = success(run(repo.path(), home.path(), &["help", "release-run"]));
    let collision = String::from_utf8(collision).unwrap();
    assert!(collision.contains("flow (.lf/flows/release-run.yaml)"));
    assert!(collision.contains("lf skill release-run"));
    assert!(collision.contains("flow wins untyped lookup"));
    let skill = success(run(repo.path(), home.path(), &["help", "skill", "paired"]));
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

    let help = success(run(repo.path(), home.path(), &["help", "skill", "paired"]));
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
fn typed_help_inspects_reserved_definitions_without_launching() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    // The removed verb no longer lists anything or falls back to the list Flow.
    let retired = run(repo.path(), home.path(), &["skill", "list"]);
    assert_eq!(retired.status.code(), Some(2));
    assert!(retired.stdout.is_empty());
    assert!(String::from_utf8_lossy(&retired.stderr).contains("skill not found: list"));
    let db = rusqlite::Connection::open(home.path().join(".lf/loopflow.db")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM agent_sessions", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );

    fs::write(
        repo.path().join(".lf/skills/list.md"),
        "Reserved skill body.",
    )
    .unwrap();

    for args in [
        vec!["lf", "skill", "list"],
        vec!["lf", "skill", "--", "list"],
    ] {
        let args = normalize_args(args.into_iter().map(str::to_string).collect()).unwrap();
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(matches!(cli.command,
            Some(Commands::Skill { cmd: SkillCommand::External(args) }) if args == ["list"]));
    }
    let skill = success(run(
        repo.path(),
        home.path(),
        &["help", "skill", "--", "list"],
    ));
    let skill = String::from_utf8(skill).unwrap();
    assert!(skill.contains("Reserved skill body."), "{skill}");
    assert!(skill.contains("lf skill list"), "{skill}");
    for owner in ["flow", "run"] {
        let flow = success(run(
            repo.path(),
            home.path(),
            &["help", owner, "--", "list"],
        ));
        let flow = String::from_utf8(flow).unwrap();
        assert!(flow.contains("flow (.lf/flows/list.yaml)"), "{flow}");
    }
    for args in [["help", "skill", "list"], ["skill", "list", "--help"]] {
        assert_eq!(
            success(run(repo.path(), home.path(), &args)),
            skill.as_bytes()
        );
    }
    assert_eq!(
        success(run(repo.path(), home.path(), &["help", "--", "task"])),
        success(run(repo.path(), home.path(), &["help", "task"]))
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM agent_sessions", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0,
        "inspection must not start an agent Session"
    );
}

#[test]
fn removed_options_and_aliases_report_usage_errors_without_effects() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    for args in [
        &["--as", "wave:exports"][..],
        &["task", "create", "--run"][..],
        &["--mode", "invalid"][..],
        &["--no-diff"][..],
        &["--diff-files"][..],
        &["--no-diff-files"][..],
        &["--no-chrome"][..],
        &["--diff", "invalid"][..],
        &["--chrome", "invalid"][..],
        &["task", "changes", "INF-123", "--json"][..],
        &["task", "diff", "INF-123", "src.rs", "--files"],
        &["task", "diff", "INF-123", "--files", "--draft"],
        &["wt", "list", "--full"],
        &["wave", "status", "--no-sync"],
        &["wt", "list", "--format", "json"],
        &["account", "status"],
        &["account", "--verify"],
        &["account", "route", "show"],
        &["account", "route", "--default", "--repo", "a/b"],
        &[
            "account",
            "route",
            "set",
            "claude",
            "a",
            "--default",
            "--repo",
            "a/b",
        ],
        &[
            "account",
            "--cached",
            "connect",
            "codex",
            "work@example.com",
        ],
        &["account", "route", "--json", "set", "codex", "work@"],
        &["wt", "rm", "unused"],
        &["-M", "unused", "run", "solo"],
        &["-C", "run", "solo"],
    ] {
        let result = run(repo.path(), home.path(), args);
        assert_eq!(result.status.code(), Some(2), "{args:?}");
        assert!(result.stdout.is_empty(), "{args:?}");
        assert!(!result.stderr.is_empty(), "{args:?}");
    }
    assert!(
        !home.path().join(".lf").exists(),
        "invalid input wrote state"
    );
}

#[test]
fn list_preserves_kinds_overrides_sources_and_reserved_invocations() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    let entries = json_entries(repo.path(), home.path(), &["list", "--json"]);
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
fn skill_catalog_preserves_namespace_discovery() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".lf/skills/team/nested")).unwrap();
    for name in ["team/review", "team/nested/check"] {
        fs::write(
            repo.path().join(format!(".lf/skills/{name}.md")),
            format!("Skill {name} body."),
        )
        .unwrap();
    }

    let catalog = json_entries(repo.path(), home.path(), &["list", "skill", "--json"]);
    let namespace = catalog.iter().find(|row| row["name"] == "team").unwrap();
    assert_eq!(namespace["kind"], "namespace");
    assert_eq!(namespace["invocation"], "lf list skill team");
    assert!(!catalog.iter().any(|row| row["kind"] == "flow"));
    let scoped = json_entries(
        repo.path(),
        home.path(),
        &["list", "skill", "team", "--json"],
    );
    let all = json_entries(repo.path(), home.path(), &["list", "--json"]);
    let expected: Vec<_> = all
        .into_iter()
        .filter(|row| row["kind"] == "skill" && row["name"].as_str().unwrap().starts_with("team/"))
        .collect();
    assert_eq!(scoped, expected);
    assert_eq!(scoped.len(), 2);
    let text = success(run(repo.path(), home.path(), &["list", "skill", "team"]));
    let text = String::from_utf8(text).unwrap();
    for row in &scoped {
        assert!(text.contains(row["name"].as_str().unwrap()));
        assert!(text.contains(row["invocation"].as_str().unwrap()));
    }

    assert!(!home.path().join(".lf").exists());
}

#[test]
fn ambiguous_commands_never_fall_back_to_installed_definitions() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    for args in [&["rename"][..], &["rename", "--help"], &["help", "rename"]] {
        let result = run(repo.path(), home.path(), args);
        assert_eq!(result.status.code(), Some(2));
        let message = String::from_utf8_lossy(&result.stderr);
        assert!(message.contains("lf session rename"), "{message}");
        assert!(message.contains("lf wave rename"), "{message}");
    }
    let selected = success(run(repo.path(), home.path(), &["run", "rename", "--help"]));
    assert!(String::from_utf8_lossy(&selected).contains("Skill rename body."));
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
    let ConcreteStep::Command(captured) = &restored else {
        panic!("expected saved command");
    };
    let canonical = normalize_args(captured.item.argv()).unwrap();
    assert_eq!(canonical, step.item.argv());
    assert_eq!(serde_json::to_value(restored).unwrap(), saved);

    let legacy = serde_json::json!({"Command": {
        "item": {"command": "task", "args": ["pr", "land", "--local"]},
        "sources": ["commands"]
    }});
    let restored: ConcreteStep = serde_json::from_value(legacy).unwrap();
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
        normalized(&["lf", "account", "rou"]),
        ["lf", "account", "route"]
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
        ["lf", "machine", "ssh", "somewhere", "show", "--help"]
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
fn help_preserves_location_without_promoting_query_filters() {
    for (args, task) in [
        (
            vec!["lf", "--task", "LOO-123", "skill", "debug", "--help"],
            Some("LOO-123"),
        ),
        (
            vec!["lf", "monitor", "list", "--task", "LOO-123", "--help"],
            None,
        ),
    ] {
        let cli = Cli::try_parse_from(
            normalize_args(args.into_iter().map(String::from).collect()).unwrap(),
        )
        .unwrap();
        assert_eq!(cli.task.as_deref(), task);
        assert!(matches!(cli.command, Some(Commands::Help { .. })));
    }
}

#[test]
fn transitive_lookup_prefers_exact_names_and_derives_unique_prefixes() {
    let tree = clap::Command::new("lf")
        .subcommand(
            clap::Command::new("task")
                .subcommand(clap::Command::new("pr").subcommand(clap::Command::new("land"))),
        )
        .subcommand(clap::Command::new("repo").subcommand(clap::Command::new("pr")))
        .subcommand(clap::Command::new("monitor"))
        .subcommand(clap::Command::new("landing"))
        .subcommand(clap::Command::new("__internal").hide(true))
        .subcommand(clap::Command::new("home").subcommand(clap::Command::new("id")));
    let resolve = |name| loopflow::lf::navigation::resolve_child(&tree, name, &[]);
    assert_eq!(resolve("land").unwrap().unwrap(), ["task", "pr", "land"]);
    assert!(resolve("pr").is_err());
    assert_eq!(resolve("mon").unwrap().unwrap(), ["monitor"]);
    assert_eq!(resolve("__internal").unwrap().unwrap(), ["__internal"]);
    assert!(resolve("__int").unwrap().is_none());
    assert!(resolve("p").is_err());
    let collision = tree.clone().subcommand(clap::Command::new("money"));
    assert!(loopflow::lf::navigation::resolve_child(&collision, "mon", &[]).is_err());
    assert_eq!(resolve("id").unwrap().unwrap(), ["machine", "id"]);
}

#[test]
fn git_commands_keep_their_root_ownership() {
    let tree = loopflow::lf::navigation::command_tree();
    for name in ["pr", "wt", "sync", "commit"] {
        assert!(tree.find_subcommand(name).is_some());
        assert!(tree
            .find_subcommand("task")
            .unwrap()
            .find_subcommand(name)
            .is_none());
    }
}

#[test]
fn account_has_one_owner_without_predecessor_aliases() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    for retired in ["auth", "identity", "id"] {
        let output = run(repo.path(), home.path(), &["help", retired, "status"]);
        assert_eq!(output.status.code(), Some(2), "{retired}");
        assert!(output.stdout.is_empty());
    }
    let help = String::from_utf8(success(run(
        repo.path(),
        home.path(),
        &["account", "--help"],
    )))
    .unwrap();
    assert!(help.contains("lf account"));
    assert!(help.contains("--json"));
    assert!(!home.path().join(".lf").exists());
}

#[test]
fn repository_commands_have_one_owner_and_derived_shorthand() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    let tree = Cli::command();
    for (leaf, path) in [
        ("release", vec!["repo", "release"]),
        ("tokens", vec!["repo", "tokens"]),
        ("ci", vec!["repo", "ci"]),
    ] {
        assert!(tree.find_subcommand(leaf).is_none());
        let mut canonical = path.clone();
        canonical.push("--help");
        let help = success(run(repo.path(), home.path(), &canonical));
        assert_eq!(
            success(run(repo.path(), home.path(), &[leaf, "--help"])),
            help
        );
        assert!(String::from_utf8_lossy(&help).contains(&format!("lf {}", path.join(" "))));
        let command = tree
            .find_subcommand("repo")
            .unwrap()
            .find_subcommand(leaf)
            .unwrap();
        assert_eq!(command.get_all_aliases().count(), 0);
    }
    assert!(!home.path().join(".lf").exists());
}

#[test]
fn command_tree_has_no_registered_aliases() {
    fn check(command: &clap::Command) {
        assert_eq!(
            command.get_all_aliases().count(),
            0,
            "{}",
            command.get_name()
        );
        for arg in command.get_arguments() {
            assert!(
                arg.get_all_aliases().unwrap_or_default().is_empty(),
                "{arg}"
            );
            assert!(
                arg.get_all_short_aliases().unwrap_or_default().is_empty(),
                "{arg}"
            );
        }
        for child in command.get_subcommands() {
            check(child);
        }
    }
    check(&loopflow::lf::navigation::command_tree());
}

#[test]
fn flow_help_validates_expansion_and_review_boundaries_without_effects() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    let flow = repo.path().join(".lf/flows/inspection.yaml");
    fs::write(
        &flow,
        "- step:\n    name: solo\n    id: review\n    human: true\n",
    )
    .unwrap();
    let output = success(run(
        repo.path(),
        home.path(),
        &["help", "flow", "inspection"],
    ));
    assert!(String::from_utf8_lossy(&output).contains("Review steps: review"));
    for (definition, expected) in [
        ("- flow: absent-flow\n", "absent-flow"),
        ("- step:\n    name: solo\n    human: true\n", "stable id"),
        ("- step:\n    name: solo\n    id: review\n    human: true\n- step:\n    name: solo\n    id: review\n    human: true\n", "not unique"),
    ] {
        fs::write(&flow, definition).unwrap();
        let output = run(repo.path(), home.path(), &["help", "flow", "inspection"]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(expected), "{error}");
    }
    for args in [vec!["machine", "user", "name"], vec!["pr", "status"]] {
        let output = run(repo.path(), home.path(), &args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    fs::write(
        repo.path().join(".lf/flows/validate.yaml"),
        "- step: solo\n",
    )
    .unwrap();
    let output = success(run(repo.path(), home.path(), &["help", "flow", "validate"]));
    assert!(String::from_utf8_lossy(&output).contains("solo"));
    assert!(!home.path().join(".lf").exists());
}

#[test]
fn authored_wave_catalog_needs_no_registry_and_keeps_empty_goals() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join("wave/parent/child")).unwrap();
    std::fs::write(
        repo.path().join("wave/parent/GOAL.md"),
        "# Parent\n\nKeep exports reliable.\n",
    )
    .unwrap();
    std::fs::write(repo.path().join("wave/parent/child/GOAL.md"), "").unwrap();
    let rows = json_entries(repo.path(), home.path(), &["list", "wave", "--json"]);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["name"], "parent");
    assert_eq!(rows[1]["name"], "parent/child");
    assert!(rows[1]["description"]
        .as_str()
        .unwrap()
        .contains("Empty goal"));
    assert!(!home.path().join(".lf").exists());
}
