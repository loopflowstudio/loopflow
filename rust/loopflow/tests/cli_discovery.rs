use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use clap::{CommandFactory, Parser};
use loopflow::definition::{resolve_definition, DefinitionKind, Target};
use loopflow::graph::compile_graph;
use loopflow::graph::load_graph;
use loopflow::graph::ConcreteStep;
use loopflow::lf::navigation::normalize_args;
use loopflow::lf::{Cli, Commands, GraphCommand, SkillCommand};
use tempfile::TempDir;

fn fixture() -> TempDir {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
    fs::create_dir_all(repo.path().join(".lf/graphs")).unwrap();
    for name in ["solo", "paired", "rename", "land", "release-run"] {
        fs::write(
            repo.path().join(format!(".lf/skills/{name}.md")),
            format!("Skill {name} body."),
        )
        .unwrap();
    }
    for name in ["paired", "only-graph", "release-run", "list"] {
        fs::write(
            repo.path().join(format!(".lf/graphs/{name}.yaml")),
            "- step: solo\n",
        )
        .unwrap();
    }
    repo
}

#[test]
fn global_agent_help_and_no_agent_diagnostic() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    let output = run(repo.path(), home.path(), &["help", "-a", "claude"]);
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert_eq!(help.matches("--agent").count(), 1);
    assert!(help.contains("overrides agents in Tasks, Graphs, skills and child commands"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("does not start an agent"));
    assert!(!home.path().join(".lf").exists());
}

#[test]
fn config_user_reports_the_forwarded_participant() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("HOME", home.path())
        .env("LF_HOME", home.path().join(".lf"))
        .env("LF_USER_NAME", "Dustin")
        .env("PATH", home.path().join("no-tools"))
        .current_dir(repo.path())
        .args(["config", "user"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8(success(output)).unwrap(), "Dustin\n");
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
    assert!(output.stderr.is_empty(), "{output:?}");
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
            vec!["skill", "debug", "--help"],
        ],
        vec![
            vec!["help", "graph", "list"],
            vec!["graph", "list", "--help"],
        ],
        vec![vec!["help", "wt", "create"], vec!["wt", "create", "--help"]],
        vec![vec!["sync", "--help"], vec!["help", "sync"]],
        vec![vec!["help", "pr", "land"], vec!["pr", "land", "--help"]],
        vec![vec!["self", "install", "--help"], vec!["install", "--help"]],
        vec![
            vec!["self", "sync-skills", "--help"],
            vec!["help", "self", "sync-skills"],
        ],
        vec![
            vec!["self", "install", "preflight", "--help"],
            vec!["install", "preflight", "--help"],
        ],
        vec![
            vec!["self", "install", "recover-switch", "--help"],
            vec!["install", "recover-switch", "--help"],
        ],
        vec![
            vec!["self", "install", "advance-switch", "--help"],
            vec!["install", "advance-switch", "--help"],
        ],
        vec![
            vec!["help", "account", "route"],
            vec!["account", "route", "--help"],
        ],
        vec![
            vec!["help", "paired"],
            vec!["paired", "--help"],
            vec!["graph", "paired", "-a", "unused", "--help"],
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
    assert!(collision.contains("graph (.lf/graphs/release-run.yaml)"));
    assert!(collision.contains("lf skill release-run"));
    assert!(collision.contains("graph wins untyped lookup"));
    let skill = success(run(repo.path(), home.path(), &["help", "skill", "paired"]));
    assert!(String::from_utf8_lossy(&skill).contains("Skill paired body."));
    let overview = success(run(repo.path(), home.path(), &["--help"]));
    let overview = String::from_utf8(overview).unwrap();
    assert!(overview.starts_with("Usage: lf "));
    assert!(overview.contains("--repo") && overview.contains("--machine"));
    assert!(
        !home.path().join(".lf").exists(),
        "inspection wrote runtime state"
    );
}

#[test]
fn retired_launch_flags_are_rejected() {
    for args in [
        vec!["lf", "--max-turns", "2", "skill", "debug"],
        vec!["lf", "--no-loopflow", "skill", "debug"],
        vec!["lf", "--steers-after", "42", "graph", "proof"],
    ] {
        assert!(Cli::try_parse_from(args).is_err());
    }
}

#[test]
fn removed_run_dispatch_reports_owners_without_launching() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    for args in [["run", "paired"], ["run", "--help"], ["help", "run"]] {
        let result = run(repo.path(), home.path(), &args);
        assert_eq!(result.status.code(), Some(2));
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(error.contains("ambiguous command 'run'"), "{error}");
        assert!(error.contains("lf task run"), "{error}");
        assert!(result.stdout.is_empty());
    }
    let overview = String::from_utf8(success(run(repo.path(), home.path(), &["help"]))).unwrap();
    assert!(overview.contains("lf graph <name>"), "{overview}");
    assert!(!overview.contains("lf run "), "{overview}");
    assert!(!home.path().join(".lf").exists());
}

#[test]
fn skill_help_does_not_offer_untyped_execution_when_the_graph_is_invalid() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    fs::write(repo.path().join(".lf/graphs/paired.yaml"), "- [invalid\n").unwrap();

    let help = success(run(repo.path(), home.path(), &["help", "skill", "paired"]));
    let help = String::from_utf8(help).unwrap();
    assert!(help.contains("lf skill paired [message]"), "{help}");
    assert!(
        help.contains("Untyped lookup fails: invalid graph"),
        "{help}"
    );
    assert!(!help.contains("lf run paired"), "{help}");
    assert!(!help.contains("lf paired [message]"), "{help}");
    let untyped = run(repo.path(), home.path(), &["paired", "--help"]);
    assert_eq!(untyped.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&untyped.stderr).contains("invalid graph"));
}

#[test]
fn typed_help_inspects_reserved_definitions_without_launching() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    // The removed verb no longer lists anything or falls back to the list Graph.
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
    let graph = success(run(
        repo.path(),
        home.path(),
        &["help", "graph", "--", "list"],
    ));
    let graph = String::from_utf8(graph).unwrap();
    assert!(graph.contains("graph (.lf/graphs/list.yaml)"), "{graph}");
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
        &["-M", "unused", "skill", "solo"],
        &["-C", "skill", "solo"],
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
    assert_eq!(pair[0]["kind"], "graph");
    assert_eq!(pair[1]["kind"], "skill");
    assert_eq!(pair[1]["source"], ".lf/skills/release-run.md");
    let reserved = entries
        .iter()
        .find(|row| row["kind"] == "graph" && row["name"] == "list")
        .unwrap();
    assert_eq!(reserved["invocation"], "lf graph -- list");
    assert!(!entries
        .iter()
        .any(|row| row["name"] == "solo" && row["kind"] == "graph"));
}

#[test]
fn skill_catalog_lists_literal_names_from_nested_sources() {
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
    assert!(catalog
        .iter()
        .any(|row| row["name"] == "team/review" && row["kind"] == "skill"));
    assert!(catalog.iter().any(|row| row["name"] == "team/nested/check"));
    assert!(!catalog.iter().any(|row| row["kind"] == "namespace"));
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
    let selected = success(run(
        repo.path(),
        home.path(),
        &["skill", "rename", "--help"],
    ));
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
    for name in ["paired", "only-graph"] {
        assert!(matches!(
            resolve_definition(repo.path(), name, None).unwrap(),
            Target::Graph(_)
        ));
        let expanded = compile_graph(&load_graph(name, repo.path()).unwrap(), repo.path()).unwrap();
        assert!(matches!(&expanded[0], ConcreteStep::Skill(step) if step.skill.name == "solo"));
    }
    assert!(matches!(
        resolve_definition(repo.path(), "paired", Some(DefinitionKind::Skill)).unwrap(),
        Target::Skill(_)
    ));
    assert!(resolve_definition(repo.path(), "solo", Some(DefinitionKind::Graph)).is_err());
    fs::write(
        repo.path().join(".lf/graphs/outer.yaml"),
        "- paired\n- graph: paired\n- interactive:\n    name: paired\n    id: review\n",
    )
    .unwrap();
    let expanded = compile_graph(&load_graph("outer", repo.path()).unwrap(), repo.path()).unwrap();
    assert_eq!(expanded.len(), 3);
    assert!(matches!(&expanded[0], ConcreteStep::Skill(step) if step.skill.name == "solo"));
    assert!(matches!(&expanded[1], ConcreteStep::Skill(step) if step.skill.name == "solo"));
    assert!(
        matches!(&expanded[2], ConcreteStep::Skill(step) if step.skill.name == "paired" && step.interactive)
    );
    fs::write(repo.path().join(".lf/graphs/paired.yaml"), "malformed: [").unwrap();
    assert!(resolve_definition(repo.path(), "paired", None)
        .unwrap_err()
        .to_string()
        .contains("invalid graph"));
    assert!(load_graph("paired", repo.path()).is_err());
    assert!(matches!(
        resolve_definition(repo.path(), "paired", Some(DefinitionKind::Skill)).unwrap(),
        Target::Skill(_)
    ));
}

#[test]
fn command_targets_compose_and_captured_operations_remain_readable() {
    let repo = fixture();
    fs::write(
        repo.path().join(".lf/graphs/commands.yaml"),
        "- cmd: pr land --local\n",
    )
    .unwrap();
    let graph = load_graph("commands", repo.path()).unwrap();
    let steps = compile_graph(&graph, repo.path()).unwrap();
    let ConcreteStep::Command(step) = &steps[0] else {
        panic!("expected command")
    };
    assert_eq!(step.item.argv(), ["lf", "pr", "land", "--local"]);
    let adapted = Target::Command(step.item.clone()).into_graph();
    assert_eq!(adapted.items, graph.items);

    let saved = serde_json::json!({"Command": {
        "item": {"command": "pr", "args": ["land", "--local"]},
        "sources": ["commands"]
    }});
    let restored: ConcreteStep = serde_json::from_value(saved.clone()).unwrap();
    let ConcreteStep::Command(captured) = &restored else {
        panic!("expected saved command");
    };
    let canonical = normalize_args(captured.item.argv()).unwrap();
    assert_eq!(canonical, ["lf", "ops", "pr", "land", "--local"]);
    assert_eq!(serde_json::to_value(restored).unwrap(), saved);

    let legacy = serde_json::json!({"Command": {
        "item": {"command": "task", "args": ["pr", "land", "--local"]},
        "sources": ["commands"]
    }});
    let restored: ConcreteStep = serde_json::from_value(legacy).unwrap();
    assert_eq!(serde_json::to_value(restored).unwrap(), saved);

    fs::write(
        repo.path().join(".lf/graphs/commands.yaml"),
        "- op: pr land\n",
    )
    .unwrap();
    assert!(load_graph("commands", repo.path())
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
        ["lf", "ops", "account", "route"]
    );
    assert_eq!(
        normalized(&["lf", "land", "--wait-and-fix"]),
        ["lf", "ops", "pr", "land", "--wait-and-fix"]
    );
    assert_eq!(
        normalized(&["lf", "task", "comment", "status"]),
        ["lf", "task", "comment", "status"]
    );
    assert_eq!(
        normalized(&["lf", "graph", "land", "--", "--help"]),
        ["lf", "graph", "land", "--", "--help"]
    );
    let parsed = Cli::try_parse_from(normalized(&["lf", "graph", "--", "list"])).unwrap();
    assert!(
        matches!(parsed.command, Some(Commands::Graph { cmd: GraphCommand::External(args) }) if args == ["list"])
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
            vec!["lf", "history", "list", "--task", "LOO-123", "--help"],
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
fn open_is_one_root_command_and_keeps_pr_open_explicit() {
    let composed = Cli::try_parse_from([
        "lf",
        "--task",
        "LOO-427",
        "open",
        "--session",
        "session-exact",
        "--diff",
        "--json",
    ])
    .unwrap();
    assert!(
        matches!(&composed.command, Some(Commands::Open(args)) if args.session.as_deref() == Some("session-exact") && args.diff && args.json)
    );
    assert!(Cli::command()
        .find_subcommand("desktop")
        .unwrap()
        .find_subcommand("open")
        .is_none());
    let saved: loopflow::graph::Command =
        serde_json::from_value(serde_json::json!({"command": "open", "args": []})).unwrap();
    assert_eq!(saved.clone().current().argv(), ["lf", "open"]);
    assert!(Cli::try_parse_from(["lf", "ops", "pr", "open", "--help"])
        .is_err_and(|error| { error.kind() == clap::error::ErrorKind::DisplayHelp }));
}

#[test]
fn desktop_state_uses_the_retained_reader_without_an_inspect_alias() {
    use loopflow::lf::DesktopCommand;

    for args in [
        vec!["lf", "desktop", "state", "--json"],
        vec!["lf", "desktop", "sta", "--json"],
    ] {
        let args = normalize_args(args.into_iter().map(str::to_owned).collect()).unwrap();
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Desktop {
                cmd: DesktopCommand::State { json: true }
            })
        ));
    }
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    let help = success(run(
        repo.path(),
        home.path(),
        &["desktop", "state", "--help"],
    ));
    assert_eq!(
        help,
        success(run(repo.path(), home.path(), &["help", "desktop", "state"]))
    );
    let retired = run(repo.path(), home.path(), &["desktop", "inspect", "--json"]);
    assert_eq!(retired.status.code(), Some(2));
    assert!(retired.stdout.is_empty());
    assert!(!home.path().join(".lf").exists());
}

#[test]
fn history_owns_recorded_reads_and_replay_without_changing_live_monitoring() {
    let parse = |args: &[&str]| {
        Cli::try_parse_from(normalize_args(args.iter().map(|s| s.to_string()).collect()).unwrap())
            .unwrap()
    };
    let cli = parse(&[
        "lf", "history", "--task", "LOO-123", "--since", "24h", "--json",
    ]);
    assert!(
        cli.task.is_none(),
        "history filters must not launch Task work"
    );
    let Some(Commands::History { feed, cmd: None }) = cli.command else {
        panic!("expected the default history feed");
    };
    assert_eq!(feed.task.as_deref(), Some("LOO-123"));
    assert_eq!(feed.since, "24h");
    assert!(feed.json);

    for verb in ["list", "show", "usage", "replay"] {
        let mut args = vec!["lf", "history", verb];
        if matches!(verb, "show" | "replay") {
            args.push("exact-capture");
        }
        assert!(matches!(
            parse(&args).command,
            Some(Commands::History { cmd: Some(_), .. })
        ));
        assert!(Cli::try_parse_from(["lf", "monitor", verb]).is_err());
    }
    for verb in ["usage", "replay"] {
        let mut args = vec!["lf", verb];
        if verb == "replay" {
            args.push("exact-capture");
        }
        assert!(matches!(
            parse(&args).command,
            Some(Commands::History { cmd: Some(_), .. })
        ));
    }
    for verb in ["ps", "top"] {
        assert!(matches!(
            parse(&["lf", verb]).command,
            Some(Commands::Monitor { .. })
        ));
    }
    assert!(Cli::try_parse_from(["lf", "monitor", "activity"]).is_err());
    assert!(Cli::try_parse_from(["lf", "monitor", "active"]).is_err());
    assert!(Cli::command().find_subcommand("replay").is_none());
}

#[test]
fn history_help_is_passive_and_saved_commands_keep_their_evidence_selectors() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    for verb in ["list", "show", "usage", "replay"] {
        let help = success(run(repo.path(), home.path(), &["history", verb, "--help"]));
        assert_eq!(
            help,
            success(run(repo.path(), home.path(), &["help", "history", verb]))
        );
    }
    let retired = run(repo.path(), home.path(), &["help", "activity"]);
    assert_eq!(retired.status.code(), Some(2));
    assert!(!home.path().join(".lf").exists());

    for (command, args, expected) in [
        (
            "monitor",
            vec!["show", "session-id", "--input", "capture-id"],
            vec!["show", "session-id", "--input", "capture-id"],
        ),
        (
            "monitor",
            vec!["activity", "--since", "24h"],
            vec!["--since", "24h"],
        ),
        (
            "activity",
            vec!["--task", "LOO-123"],
            vec!["--task", "LOO-123"],
        ),
        ("replay", vec!["capture-id"], vec!["replay", "capture-id"]),
        ("usage", vec!["--days", "0"], vec!["usage", "--days", "0"]),
    ] {
        let saved = serde_json::json!({"command": command, "args": args});
        let restored: loopflow::graph::Command = serde_json::from_value(saved).unwrap();
        assert_eq!(restored.command, "history");
        assert_eq!(restored.args, expected);
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
        .subcommand(clap::Command::new("machine").subcommand(clap::Command::new("id")));
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
fn git_commands_belong_to_ops_not_task() {
    let tree = loopflow::lf::navigation::command_tree();
    for name in ["pr", "wt", "sync", "commit"] {
        assert!(tree
            .find_subcommand("ops")
            .unwrap()
            .find_subcommand(name)
            .is_some());
        if name == "sync" {
            continue;
        } // Task planning sync is separate from Git sync.
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
fn moved_commands_have_one_owner_and_keep_their_short_names() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    let tree = Cli::command();
    assert!(tree.find_subcommand("repo").is_none());
    for (short, path) in [
        (vec!["land"], vec!["ops", "pr", "land"]),
        (vec!["pr", "land"], vec!["ops", "pr", "land"]),
        (vec!["commit"], vec!["ops", "commit"]),
        (vec!["sync"], vec!["ops", "sync"]),
        (vec!["wt", "create"], vec!["ops", "wt", "create"]),
        (vec!["account"], vec!["ops", "account"]),
        (vec!["machine", "list"], vec!["ops", "machine", "list"]),
        (vec!["pool"], vec!["ops", "machine", "pool"]),
        (vec!["release", "run"], vec!["ops", "release", "run"]),
        (vec!["ci"], vec!["ops", "ci"]),
        (vec!["tokens"], vec!["ops", "tokens"]),
        (vec!["repository"], vec!["config", "repository"]),
        (vec!["planning", "key"], vec!["config", "planning", "key"]),
        (vec!["reteam"], vec!["wave", "reteam"]),
    ] {
        assert!(tree.find_subcommand(short[0]).is_none(), "{short:?}");
        let help = success(run(
            repo.path(),
            home.path(),
            &[path.clone(), vec!["--help"]].concat(),
        ));
        assert_eq!(
            success(run(
                repo.path(),
                home.path(),
                &[short.clone(), vec!["--help"]].concat()
            )),
            help,
            "{short:?}"
        );
        assert!(String::from_utf8_lossy(&help).contains(&format!("lf {}", path.join(" "))));
    }
    assert!(!home.path().join(".lf").exists());
}

#[test]
fn installation_configuration_and_app_commands_have_distinct_owners() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    for (short, owner) in [("install", "self"), ("doctor", "self"), ("user", "config")] {
        let help = success(run(repo.path(), home.path(), &[owner, short, "--help"]));
        assert_eq!(
            success(run(repo.path(), home.path(), &[short, "--help"])),
            help
        );
        assert!(String::from_utf8_lossy(&help).contains(&format!("lf {owner} {short}")));
    }
    let tree = Cli::command();
    let mut machine_commands: Vec<_> = tree
        .find_subcommand("ops")
        .unwrap()
        .find_subcommand("machine")
        .unwrap()
        .get_subcommands()
        .map(|command| command.get_name())
        .collect();
    machine_commands.sort();
    assert_eq!(
        machine_commands,
        [
            "add",
            "connect",
            "credentials",
            "end",
            "id",
            "list",
            "ondemand",
            "pool",
            "public",
            "remove",
            "rename",
            "status"
        ]
    );
    let help =
        String::from_utf8(success(run(repo.path(), home.path(), &["help", "--all"]))).unwrap();
    for path in ["self install", "self doctor", "config user", "open"] {
        assert!(help.contains(path), "{help}");
    }
    assert!(!help.contains("screenshot"));
    assert!(!help.contains("machine desktop"));
    success(run(repo.path(), home.path(), &["help", "open"]));
    assert!(
        !run(repo.path(), home.path(), &["desktop", "open", "--help"])
            .status
            .success()
    );
    // Work opening must not change the explicit PR operation.
    success(run(repo.path(), home.path(), &["pr", "open", "--help"]));
    assert!(!home.path().join(".lf").exists());
}

#[test]
fn retired_installation_and_capture_paths_do_not_resolve() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    for path in [
        vec!["installation", "install"],
        vec!["machine", "install"],
        vec!["machine", "doctor"],
        vec!["machine", "user"],
        vec!["machine", "desktop"],
        vec!["machine", "screenshot"],
        vec!["self", "screenshot"],
        vec!["screenshot"],
        vec!["__screenshot-supervisor"],
    ] {
        let mut args = vec!["help"];
        args.extend(&path);
        let output = run(repo.path(), home.path(), &args);
        assert_eq!(output.status.code(), Some(2), "{path:?}: {output:?}");
        assert!(output.stdout.is_empty());
        if Cli::command().find_subcommand(path[0]).is_some() {
            let mut args = vec!["lf"];
            args.extend(path);
            assert!(Cli::try_parse_from(args).is_err());
        }
    }
    assert!(!home.path().join(".lf").exists());
}

#[test]
fn command_tree_has_no_command_aliases() {
    fn check(command: &clap::Command) {
        assert_eq!(
            command.get_all_aliases().count(),
            0,
            "{}",
            command.get_name()
        );
        for arg in command.get_arguments() {
            // Loaded 0.14.14 cron jobs pass `--flow`; LOO-574 removes the alias.
            if command.get_name() == "run"
                && arg.get_long() == Some("graph")
                && arg.get_all_aliases() == Some(vec!["flow"])
            {
                continue;
            }
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
fn graph_help_validates_expansion_and_review_boundaries_without_effects() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    let graph = repo.path().join(".lf/graphs/inspection.yaml");
    fs::write(&graph, "- interactive:\n    name: solo\n    id: review\n").unwrap();
    let output = success(run(
        repo.path(),
        home.path(),
        &["help", "graph", "inspection"],
    ));
    assert!(String::from_utf8_lossy(&output).contains("solo (interactive)"));
    for (definition, expected) in [
        ("- graph: absent-graph\n", "absent-graph"),
        ("- step:\n    name: solo\n    human: true\n", "retired"),
        ("- interactive:\n    name: solo\n    id: review\n- interactive:\n    name: solo\n    id: review\n", "not unique"),
    ] {
        fs::write(&graph, definition).unwrap();
        let output = run(repo.path(), home.path(), &["help", "graph", "inspection"]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(expected), "{error}");
    }
    for args in [vec!["config", "user", "name"], vec!["pr", "status"]] {
        let output = run(repo.path(), home.path(), &args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    fs::write(
        repo.path().join(".lf/graphs/validate.yaml"),
        "- step: solo\n",
    )
    .unwrap();
    let output = success(run(
        repo.path(),
        home.path(),
        &["help", "graph", "validate"],
    ));
    assert!(String::from_utf8_lossy(&output).contains("solo"));
    assert!(!home.path().join(".lf").exists());
}

#[test]
fn wave_catalog_reads_imported_definitions_and_keeps_empty_goals() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join(".lf/wave/parent/child")).unwrap();
    std::fs::write(
        repo.path().join(".lf/wave/parent/GOAL.md"),
        "# Parent\n\nKeep exports reliable.\n",
    )
    .unwrap();
    std::fs::write(repo.path().join(".lf/wave/parent/child/GOAL.md"), "").unwrap();
    assert!(json_entries(repo.path(), home.path(), &["list", "wave", "--json"]).is_empty());
    assert!(!home.path().join(".lf").exists());
    let store =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join(".lf/loopflow.db")).unwrap();
    store
        .ensure_wave(
            repo.path().canonicalize().unwrap().to_str().unwrap(),
            "parent/child",
        )
        .unwrap();
    let rows = json_entries(repo.path(), home.path(), &["list", "wave", "--json"]);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["name"], "parent");
    assert_eq!(rows[1]["name"], "parent/child");
    assert!(rows[1]["description"]
        .as_str()
        .unwrap()
        .contains("Empty goal"));
}

#[test]
fn remote_selection_preserves_command_arguments_and_literal_boundaries() {
    use loopflow::lf::navigation::machine_invocation;
    let args = [
        "lf",
        "--account",
        "personal@",
        "--machine=mini",
        "--forward-agent",
        "--task",
        "LOO-123",
        "implement",
        "--",
        "--machine",
        "literal",
    ]
    .map(String::from);
    let (cli, command) = machine_invocation(&args).unwrap().unwrap();
    assert_eq!(cli.machine.as_deref(), Some("mini"));
    assert!(cli.account.is_empty());
    assert!(cli.forward_agent);
    assert_eq!(
        command,
        [
            "--account",
            "personal@",
            "--task",
            "LOO-123",
            "implement",
            "--",
            "--machine",
            "literal"
        ]
    );
    for args in [
        vec!["lf", "commit", "-m", "--machine"],
        vec!["lf", "--docs", "--machine", "status"],
        vec!["lf", "skill", "--", "--machine", "literal"],
    ] {
        assert!(
            machine_invocation(&args.iter().map(|s| (*s).to_string()).collect::<Vec<_>>())
                .unwrap()
                .is_none()
        );
    }
    assert!(machine_invocation(&["lf", "--machine"].map(String::from)).is_err());
}

#[test]
fn native_skill_help_and_graph_capture_keep_the_selected_source_and_declarations() {
    let repo = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let native = repo.path().join(".claude/skills/audit/SKILL.md");
    fs::create_dir_all(native.parent().unwrap()).unwrap();
    fs::write(&native, "---\ndescription: Audit a project\nagent: Explore\ncontext: fork\nallowed-tools: Read\n---\nAudit $ARGUMENTS using [rules](rules.md).\n").unwrap();
    fs::write(
        native.parent().unwrap().join("rules.md"),
        "Preserve these rules.",
    )
    .unwrap();
    let other = repo.path().join(".agents/skills/audit/SKILL.md");
    fs::create_dir_all(other.parent().unwrap()).unwrap();
    fs::write(&other, "Other source").unwrap();

    let help =
        String::from_utf8(success(run(repo.path(), home.path(), &["help", "audit"]))).unwrap();
    assert!(help.contains(".claude/skills/audit/SKILL.md"), "{help}");
    assert!(help.contains("Audit $ARGUMENTS"), "{help}");
    let entries = json_entries(repo.path(), home.path(), &["list", "skill", "--json"]);
    let audit = entries
        .iter()
        .find(|entry| entry["name"] == "audit")
        .unwrap();
    assert_eq!(audit["source"], ".claude/skills/audit/SKILL.md");
    assert_eq!(audit["description"], "Audit a project");
    assert!(!entries.iter().any(|entry| entry["name"] == "audit/rules"));

    fs::create_dir_all(repo.path().join(".lf/graphs")).unwrap();
    fs::write(repo.path().join(".lf/graphs/check.yaml"), "- audit\n").unwrap();
    let graph = load_graph("check", repo.path()).unwrap();
    let captured = serde_json::to_string(&graph).unwrap();
    fs::remove_file(&native).unwrap();
    let retained = serde_json::from_str(&captured).unwrap();
    let compiled = compile_graph(&retained, repo.path()).unwrap();
    let ConcreteStep::Skill(step) = &compiled[0] else {
        panic!("expected skill")
    };
    assert!(
        step.skill.agent.is_none(),
        "Claude subagent must not select an lf harness"
    );
    let source = step.skill.source.as_ref().unwrap();
    assert_eq!(source.path, native);
    assert!(source
        .frontmatter
        .as_deref()
        .unwrap()
        .contains("context: fork"));
    assert_eq!(
        step.skill.content.as_deref(),
        Some("Audit $ARGUMENTS using [rules](rules.md).\n")
    );
}

#[test]
fn desktop_verbs_take_pane_paths_without_public_lifetime_tokens() {
    use loopflow::lf::commands::desktop::DesktopKey;
    use loopflow::lf::DesktopCommand;
    let literal = "héλ🙂 \\n \"$(literal)\"";
    let cli = Cli::try_parse_from([
        "lf",
        "--repo",
        "project",
        "--task",
        "LOO-427",
        "desktop",
        "send",
        "-p",
        "2/shell:1",
        literal,
    ])
    .unwrap();
    assert_eq!(cli.task.as_deref(), Some("LOO-427"));
    assert!(matches!(cli.command, Some(Commands::Desktop {
        cmd: DesktopCommand::Send { text: Some(text), target }
    }) if text == literal && target.pane.as_deref() == Some("2/shell:1")));
    let cli = Cli::try_parse_from(["lf", "desktop", "send-key", "enter"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Commands::Desktop {
            cmd: DesktopCommand::SendKey {
                key: DesktopKey::Enter,
                ..
            }
        })
    ));
    // One shell line, as typed; no `--`.
    let cli = Cli::try_parse_from(["lf", "desktop", "run", "cargo test -p loopflow"]).unwrap();
    assert!(matches!(cli.command, Some(Commands::Desktop {
        cmd: DesktopCommand::Run { line: Some(line), .. }
    }) if line == "cargo test -p loopflow"));
    let cli = Cli::try_parse_from(["lf", "desktop", "goto", "2", "--json"]).unwrap();
    assert!(matches!(cli.command, Some(Commands::Desktop {
        cmd: DesktopCommand::Goto { index: 2, target }
    }) if target.json));
    let cli = Cli::try_parse_from([
        "lf",
        "desktop",
        "do",
        "section",
        "-p",
        "board",
        "section=done",
    ])
    .unwrap();
    assert!(matches!(cli.command, Some(Commands::Desktop {
        cmd: DesktopCommand::Do { action, args, target }
    }) if action == "section" && args == ["section=done"] && target.pane.as_deref() == Some("board")));
    for removed in [
        &["desktop", "text", "hello"][..],
        &["desktop", "read", "--region", "screen"],
        &["desktop", "restore"],
        &["desktop", "zoom"],
        &["desktop", "zoom", "-b", "shell", "--on"],
        &["desktop", "zoom", "-t", "2", "--on"],
        &["desktop", "back", "-p", "shell"],
        &["desktop", "send", "--surface", "removed", "text"],
    ] {
        assert!(
            Cli::try_parse_from(std::iter::once("lf").chain(removed.iter().copied())).is_err(),
            "{removed:?}"
        );
    }
    assert!(Cli::try_parse_from(["lf", "desktop", "send-key", "arbitrary-bytes"]).is_err());
    assert!(Cli::try_parse_from(["lf", "desktop", "zoom", "--on", "--off"]).is_err());
    assert!(
        matches!(Cli::try_parse_from(["lf", "desktop", "move", "-p", "browser", "--destination", "session", "--axis", "vertical"]).unwrap().command,
        Some(Commands::Desktop { cmd: DesktopCommand::Move { destination, .. } }) if destination == "session")
    );
}

#[test]
fn portable_help_describes_exact_kind_selection() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".lf/graphs/team")).unwrap();
    fs::write(repo.path().join(".lf/graphs/team/check.yaml"), "- solo\n").unwrap();
    fs::write(
        repo.path().join(".lf/skills/team-check.md"),
        "Explicit skill body",
    )
    .unwrap();
    let help =
        |args: &[&str]| String::from_utf8(success(run(repo.path(), home.path(), args))).unwrap();
    let graph = help(&["help", "team/check"]);
    assert!(graph.contains("team/check — graph"));
    assert!(graph.contains("team-check"));
    let skill = help(&["help", "team-check"]);
    assert!(skill.contains("team-check — skill"));
    assert!(!skill.contains("graph wins untyped lookup"));
    assert!(!skill.contains("Untyped lookup selects"));
    fs::write(repo.path().join(".lf/graphs/team-check.yaml"), "- solo\n").unwrap();
    assert!(help(&["help", "team-check"]).contains("team-check — graph"));
    assert!(help(&["help", "skill", "team-check"])
        .contains("Untyped lookup selects the same-named graph"));
}

#[test]
fn structured_help_describes_pane_creation_and_read_results_without_desktop() {
    use loopflow::lf::navigation::describe_command;
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    let output = success(run(
        repo.path(),
        home.path(),
        &["help", "desktop", "run", "--json"],
    ));
    let shell: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(
        shell,
        describe_command(&["desktop".into(), "run".into()], false).unwrap()
    );
    assert!(!home.path().join(".lf").exists());
    let definition = &shell["definition"];
    let args = definition["arguments"].as_array().unwrap();
    assert!(!args.iter().any(|arg| arg["name"] == "axis"));
    assert!(definition["result_schema"]["properties"]["tab"].is_object());
    assert!(definition["result_schema"]["properties"]["target"].is_object());
    for name in ["pane", "window", "line"] {
        assert!(
            args.iter()
                .any(|arg| arg["name"] == name && arg["required"] == false),
            "{name}"
        );
    }
    assert!(!args
        .iter()
        .any(|arg| arg["name"] == "task" || arg["name"] == "block" || arg["name"] == "target"));
    assert_eq!(
        definition["result_schema"]["properties"]["destination"]["$ref"],
        "#/$defs/DesktopDestination"
    );
    let view = describe_command(&["desktop".into(), "view".into()], false).unwrap();
    let schema = &view["definition"]["result_schema"];
    assert_eq!(schema["anyOf"].as_array().unwrap().len(), 2);
    assert!(schema["$defs"]["ViewPlan"].is_object());
    assert!(schema["$defs"]["DesktopOpening"].is_object());
    let read = describe_command(&["desktop".into(), "capture".into()], false).unwrap();
    let region = read["definition"]["arguments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|arg| arg["name"] == "region")
        .unwrap();
    assert_eq!(
        region["values"],
        serde_json::json!(["screen", "scrollback", "selection"])
    );
    assert!(read["definition"]["result_schema"]["$defs"]["DesktopTextResult"].is_object());
    assert!(describe_command(&["desktop".into(), "split".into()], false).is_err());
    let all = describe_command(&["desktop".into()], true).unwrap();
    assert!(all["definition"]["commands"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|command| {
            // Desktop's Work reads print its presentation types, which carry no schema.
            ![
                "lf desktop rename",
                "lf desktop wait",
                "lf desktop plan",
                "lf desktop done",
                "lf desktop wave",
                "lf desktop sessions",
                "lf desktop task",
            ]
            .contains(&command["command"].as_str().unwrap())
        })
        .all(|command| command["result_schema"].is_object()));
}

#[test]
fn desktop_query_plan_reads_backlog_without_execution_or_desktop() {
    let repo = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    assert!(Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(repo.path())
        .status()
        .unwrap()
        .success());
    let database = home.path().join("loopflow.db");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime
        .block_on(loopflow::store::open_ephemeral_store(
            &loopflow::store::StorageConfig::sqlite(database.clone()),
        ))
        .unwrap();
    let conn = rusqlite::Connection::open(&database).unwrap();
    conn.execute_batch(include_str!("fixtures/saved_selection_work.sql"))
        .unwrap();
    let path = repo.path().canonicalize().unwrap().display().to_string();
    conn.execute("UPDATE repository_plans SET repo=?1", [&path])
        .unwrap();
    conn.execute("UPDATE waves SET repo=?1", [&path]).unwrap();
    // Resumed, FIX-1 joins FIX-2 in Working, so Working Tasks has two tiles to arrange.
    conn.execute(
        "UPDATE tasks SET paused=0 WHERE issue_identifier='FIX-1'",
        [],
    )
    .unwrap();
    let counts = || -> (i64, i64, i64) {
        conn.query_row("SELECT (SELECT count(*) FROM agent_sessions), (SELECT count(*) FROM task_ownership), (SELECT count(*) FROM processes)", [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).unwrap()
    };
    let before = counts();
    let command = |args: &[&str]| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        command
            .env_clear()
            .env("HOME", home.path())
            .env("LF_HOME", home.path())
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env("PATH", "/usr/bin:/bin")
            .current_dir(repo.path())
            .args(args);
        command
    };
    let invoke = |args: &[&str]| command(args).output().unwrap();
    let output = invoke(&["desktop", "view", "state:backlog", "--plan", "--json"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let plan: loopflow::work::selection::ViewPlan = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(plan.contents.len(), 1);
    assert_eq!(plan.contents[0].title, "Backlog");
    assert!(plan.contents[0].primary_session.is_none());
    assert!(plan.contents[0].argv.is_none());
    assert_eq!(counts(), before);
    let missing = invoke(&["desktop", "view", "query:missing", "--plan"]);
    assert_eq!(
        missing.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&missing.stderr)
    );
    let invalid = invoke(&["desktop", "view", "repo:other", "--plan"]);
    assert_eq!(
        invalid.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&invalid.stderr)
    );
    assert_eq!(counts(), before);
    let saved = invoke(&["desktop", "query", "query:backlog", "state:backlog"]);
    assert!(
        saved.status.success(),
        "{}",
        String::from_utf8_lossy(&saved.stderr)
    );
    let saved: loopflow::work::selection::Dashboard =
        serde_json::from_slice(&saved.stdout).unwrap();
    let after_save = counts();
    assert_eq!((after_save.0, after_save.1), (before.0, before.1));
    let reopened = invoke(&["desktop", "view", "query:backlog", "--plan"]);
    assert!(
        reopened.status.success(),
        "{}",
        String::from_utf8_lossy(&reopened.stderr)
    );
    let reopened: loopflow::work::selection::ViewPlan =
        serde_json::from_slice(&reopened.stdout).unwrap();
    assert_eq!(reopened.saved_revision, Some(saved.revision));
    assert_eq!(reopened.arrangement, saved.arrangement);
    assert_eq!(counts(), after_save);
    let task_id =
        loopflow::durable::TaskId::parse("task_33333333333333333333333333333333").unwrap();
    let revision = runtime
        .block_on(store.get_task(&task_id))
        .unwrap()
        .unwrap()
        .revision
        .to_string();
    let renamed = invoke(&[
        "desktop",
        "rename",
        "task:task_33333333333333333333333333333333",
        "Later task",
        "--revision",
        &revision,
    ]);
    assert!(
        renamed.status.success(),
        "{}",
        String::from_utf8_lossy(&renamed.stderr)
    );
    assert_eq!(
        runtime
            .block_on(store.get_task(&task_id))
            .unwrap()
            .unwrap()
            .title,
        "Later task"
    );
    let stale_name = invoke(&[
        "desktop",
        "rename",
        "task:task_33333333333333333333333333333333",
        "stale",
        "--revision",
        &revision,
    ]);
    assert!(!stale_name.status.success());
    assert_eq!(
        runtime
            .block_on(store.get_task(&task_id))
            .unwrap()
            .unwrap()
            .title,
        "Later task"
    );
    let pinned = invoke(&["desktop", "pin", "backlog", "--revision", "1"]);
    assert!(
        pinned.status.success(),
        "{}",
        String::from_utf8_lossy(&pinned.stderr)
    );
    let named = invoke(&[
        "desktop",
        "rename",
        "query:backlog",
        "Later",
        "--revision",
        "2",
    ]);
    assert!(
        named.status.success(),
        "{}",
        String::from_utf8_lossy(&named.stderr)
    );
    let removed = invoke(&["desktop", "bind", "repository", "backlog"]);
    assert_eq!(removed.status.code(), Some(2));
    let before_reads = counts();
    let defaults = invoke(&["desktop", "view", "--plan"]);
    let defaults: loopflow::work::selection::ViewPlan =
        serde_json::from_slice(&defaults.stdout).unwrap();
    assert!(defaults.dashboard_id.is_none());
    // Both Waves and the two idle Tasks; the unstarted one isn't Working.
    assert_eq!(defaults.contents.len(), 4);
    let plan = invoke(&["desktop", "view", "query:backlog", "--plan"]);
    let plan: loopflow::work::selection::ViewPlan = serde_json::from_slice(&plan.stdout).unwrap();
    assert_eq!(plan.name.as_deref(), Some("Later"));
    assert!(plan.pinned);
    assert_eq!(plan.contents.len(), 1);
    let layout = invoke(&["desktop", "layout", "repository"]);
    assert!(layout.status.success());
    assert_eq!(String::from_utf8_lossy(&layout.stdout).trim(), "null");
    assert_eq!(counts(), before_reads);
    let stale = invoke(&["desktop", "unpin", "backlog", "--revision", "1"]);
    assert!(!stale.status.success());
    let plan = invoke(&["desktop", "view", "query:backlog", "--plan"]);
    let plan: loopflow::work::selection::ViewPlan = serde_json::from_slice(&plan.stdout).unwrap();
    assert!(plan.pinned);
    assert_eq!(plan.saved_revision, Some(3));
    let invoke_input = |args: &[&str], input: &str| {
        use std::io::Write;
        let mut child = command(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    };
    // Tile verbs aimed at a saved record's tile edit it without Desktop.
    let tile = format!("query:backlog/task:{}", plan.arrangement.order[0].work.id);
    let hidden = invoke(&["desktop", "hide", "-p", &tile]);
    assert!(
        hidden.status.success(),
        "{}",
        String::from_utf8_lossy(&hidden.stderr)
    );
    let arranged: loopflow::work::selection::Dashboard =
        serde_json::from_slice(&hidden.stdout).unwrap();
    assert!(arranged.arrangement.project().visible.is_empty());
    assert_eq!(arranged.arrangement.order.len(), 1);
    // Hiding never changes whether the order is automatic.
    assert_eq!(arranged.arrangement.mode, plan.arrangement.mode);
    // A hidden tile is no longer shown, so hiding it again names nothing.
    let again = invoke(&["desktop", "hide", "-p", &tile]);
    assert_eq!(again.status.code(), Some(3));
    let order = defaults.resolution.task_ids();
    // Working Tasks keeps only the shared order: nothing to size.
    let sized = invoke(&[
        "desktop",
        "resize",
        "-p",
        &format!("working-tasks/task:{}", order[0]),
        "--toward",
        &format!("task:{}", order[1]),
        "--ratio",
        "0.6",
    ]);
    assert_eq!(sized.status.code(), Some(1));
    let moved = invoke(&[
        "desktop",
        "move",
        "-p",
        &format!("working-tasks/task:{}", order[order.len() - 1]),
        "--destination",
        &format!("task:{}", order[0]),
    ]);
    assert!(
        moved.status.success(),
        "{}",
        String::from_utf8_lossy(&moved.stderr)
    );
    let moved: Vec<String> = serde_json::from_slice(&moved.stdout).unwrap();
    assert_eq!(moved[0], order[order.len() - 1]);
    let reopened = invoke(&["desktop", "view", "query:backlog", "--plan"]);
    let reopened: loopflow::work::selection::ViewPlan =
        serde_json::from_slice(&reopened.stdout).unwrap();
    assert_eq!(reopened.arrangement, arranged.arrangement);
    assert_eq!(reopened.saved_revision, Some(arranged.revision));
    let revision = arranged.revision.to_string();
    let slots = invoke_input(
        &[
            "desktop",
            "layout",
            "query:backlog",
            "--save",
            "--revision",
            &revision,
        ],
        "null",
    );
    assert!(
        slots.status.success(),
        "{}",
        String::from_utf8_lossy(&slots.stderr)
    );
    let read = invoke(&["desktop", "layout", "query:backlog"]);
    let read: loopflow::work::selection::Dashboard = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!((read.slots, read.revision), (None, arranged.revision + 1));
    let mut layout: loopflow::work::selection::WorkPaneLayout =
        serde_json::from_str(include_str!("../../../tests/fixtures/dto/pane_layout.json")).unwrap();
    layout.revision = 0;
    // The fixture's conversation is not recorded here; the session pane shows the start state.
    if let loopflow::work::selection::PaneTree::Split { first, .. } = &mut layout.tree {
        **first = loopflow::work::selection::PaneTree::Pane {
            id: "pane-0".into(),
            content: loopflow::work::selection::PaneIntent::Session {
                session_id: None,
                location: None,
            },
        };
    }
    layout.focused = None;
    layout.zoomed = None;
    layout.hidden.clear();
    let input = serde_json::to_string(&layout).unwrap();
    let saved = invoke_input(
        &[
            "desktop",
            "layout",
            "task:task_22222222222222222222222222222222",
            "--save",
        ],
        &input,
    );
    assert!(
        saved.status.success(),
        "{}",
        String::from_utf8_lossy(&saved.stderr)
    );
    let saved: loopflow::work::selection::WorkPaneLayout =
        serde_json::from_slice(&saved.stdout).unwrap();
    assert_eq!(saved.drafts, layout.drafts);
    assert_eq!(saved.revision, 1);
    assert!(!invoke_input(
        &[
            "desktop",
            "layout",
            "task:task_22222222222222222222222222222222",
            "--save"
        ],
        &input
    )
    .status
    .success());
    let before_read = counts();
    let reopened = invoke(&[
        "desktop",
        "layout",
        "task:task_22222222222222222222222222222222",
    ]);
    let reopened: loopflow::work::selection::WorkPaneLayout =
        serde_json::from_slice(&reopened.stdout).unwrap();
    assert_eq!(reopened, saved);
    assert_eq!(counts(), before_read);
    assert_eq!((counts().0, counts().1), (before.0, before.1));
    assert_eq!(
        conn.query_row("SELECT count(*) FROM dashboards", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}
