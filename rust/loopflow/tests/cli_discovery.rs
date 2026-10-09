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
        vec![vec!["help", "flow", "list"], vec!["flow", "list", "--help"]],
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
            vec!["flow", "paired", "-a", "unused", "--help"],
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
fn retired_launch_flags_are_not_forwarded_and_steer_cursor_is_preserved() {
    for args in [
        vec!["lf", "--max-turns", "2", "skill", "debug"],
        vec!["lf", "--no-loopflow", "skill", "debug"],
    ] {
        assert!(Cli::try_parse_from(args).is_err());
    }
    let cli = Cli::try_parse_from(["lf", "--steers-after", "42", "flow", "proof"]).unwrap();
    let child_args = std::iter::once("lf".to_owned())
        .chain(cli.process_options().step_args())
        .chain(["skill".to_owned(), "debug".to_owned()]);
    let child = Cli::try_parse_from(child_args).unwrap();
    assert_eq!(child.steers_after, Some(42));
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
    assert!(overview.contains("lf flow <name>"), "{overview}");
    assert!(!overview.contains("lf run "), "{overview}");
    assert!(!home.path().join(".lf").exists());
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
    let untyped = run(repo.path(), home.path(), &["paired", "--help"]);
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
    let flow = success(run(
        repo.path(),
        home.path(),
        &["help", "flow", "--", "list"],
    ));
    let flow = String::from_utf8(flow).unwrap();
    assert!(flow.contains("flow (.lf/flows/list.yaml)"), "{flow}");
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
        normalized(&["lf", "flow", "land", "--", "--help"]),
        ["lf", "flow", "land", "--", "--help"]
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
fn desktop_open_has_one_owner_and_ambiguous_shorthand_has_no_effects() {
    use loopflow::lf::DesktopCommand;

    let cli = Cli::try_parse_from(["lf", "desktop", "open"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Commands::Desktop {
            cmd: DesktopCommand::Open {
                session: None,
                diff: false,
                json: false
            }
        })
    ));
    let composed = Cli::try_parse_from([
        "lf",
        "--task",
        "LOO-427",
        "desktop",
        "open",
        "--session",
        "session-exact",
        "--diff",
        "--json",
    ])
    .unwrap();
    assert!(
        matches!(composed.command, Some(Commands::Desktop { cmd: DesktopCommand::Open { session: Some(ref id), diff: true, json: true } }) if id == "session-exact")
    );
    assert!(Cli::command().find_subcommand("open").is_none());
    let saved: loopflow::engine::flow::Command =
        serde_json::from_value(serde_json::json!({"command": "open", "args": []})).unwrap();
    assert_eq!(saved.argv(), ["lf", "desktop", "open"]);
    assert_eq!(saved.clone().current(), saved);

    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    let output = run(repo.path(), home.path(), &["open"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    for owner in ["desktop", "pr"] {
        assert!(error.contains(&format!("lf {owner} open")), "{error}");
    }
    assert!(!error.contains("lf session open"));
    assert!(!home.path().join(".lf").exists());
}

#[test]
fn desktop_list_uses_the_retained_reader_without_an_inspect_alias() {
    use loopflow::lf::DesktopCommand;

    for args in [
        vec!["lf", "desktop", "list", "--json"],
        vec!["lf", "desktop", "li", "--json"],
    ] {
        let args = normalize_args(args.into_iter().map(str::to_owned).collect()).unwrap();
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Desktop {
                cmd: DesktopCommand::List { json: true }
            })
        ));
    }
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    let help = success(run(
        repo.path(),
        home.path(),
        &["desktop", "list", "--help"],
    ));
    assert_eq!(
        help,
        success(run(repo.path(), home.path(), &["help", "desktop", "list"]))
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
    for verb in ["ps", "top", "active"] {
        assert!(matches!(
            parse(&["lf", verb]).command,
            Some(Commands::Monitor { .. })
        ));
    }
    assert!(Cli::try_parse_from(["lf", "monitor", "activity"]).is_err());
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
        let restored: loopflow::engine::flow::Command = serde_json::from_value(saved).unwrap();
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
fn git_commands_keep_their_root_ownership() {
    let tree = loopflow::lf::navigation::command_tree();
    for name in ["pr", "wt", "sync", "commit"] {
        assert!(tree.find_subcommand(name).is_some());
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
            "id",
            "list",
            "remove",
            "rename",
            "status"
        ]
    );
    let help =
        String::from_utf8(success(run(repo.path(), home.path(), &["help", "--all"]))).unwrap();
    for path in ["self install", "self doctor", "config user", "desktop open"] {
        assert!(help.contains(path), "{help}");
    }
    assert!(!help.contains("screenshot"));
    assert!(!help.contains("machine desktop"));
    success(run(
        repo.path(),
        home.path(),
        &["desktop", "open", "--help"],
    ));
    // Desktop opening must not change the explicit PR operation.
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
        vec!["desktop"],
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
    for args in [vec!["config", "user", "name"], vec!["pr", "status"]] {
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
fn wave_catalog_reads_imported_definitions_and_keeps_empty_goals() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join("wave/parent/child")).unwrap();
    std::fs::write(
        repo.path().join("wave/parent/GOAL.md"),
        "# Parent\n\nKeep exports reliable.\n",
    )
    .unwrap();
    std::fs::write(repo.path().join("wave/parent/child/GOAL.md"), "").unwrap();
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
fn native_skill_help_and_flow_capture_keep_the_selected_source_and_declarations() {
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

    fs::create_dir_all(repo.path().join(".lf/flows")).unwrap();
    fs::write(repo.path().join(".lf/flows/check.yaml"), "- audit\n").unwrap();
    let flow = load_flow("check", repo.path()).unwrap();
    let captured = serde_json::to_string(&flow).unwrap();
    fs::remove_file(&native).unwrap();
    let retained = serde_json::from_str(&captured).unwrap();
    let compiled = compile_flow(&retained, repo.path()).unwrap();
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
fn desktop_input_keeps_text_separate_from_keys_and_requires_surface_identity() {
    use loopflow::lf::commands::desktop::DesktopKey;
    use loopflow::lf::DesktopCommand;
    let literal = "héλ🙂 \\n \"$(literal)\"";
    let args = [
        "lf",
        "desktop",
        "text",
        "--target",
        "{}",
        "--surface",
        "surface",
        "--",
        literal,
    ]
    .map(str::to_string)
    .to_vec();
    let cli = Cli::try_parse_from(normalize_args(args).unwrap()).unwrap();
    assert!(matches!(cli.command, Some(Commands::Desktop {
        cmd: DesktopCommand::Text { text, surface, .. }
    }) if text == literal && surface == "surface"));
    let cli = Cli::try_parse_from([
        "lf",
        "desktop",
        "key",
        "--target",
        "{}",
        "--surface",
        "surface",
        "enter",
    ])
    .unwrap();
    assert!(matches!(
        cli.command,
        Some(Commands::Desktop {
            cmd: DesktopCommand::Key {
                key: DesktopKey::Enter,
                ..
            }
        })
    ));
    for operation in ["text", "key"] {
        assert!(
            Cli::try_parse_from(["lf", "desktop", operation, "--target", "{}", "enter"]).is_err()
        );
    }
    assert!(Cli::try_parse_from([
        "lf",
        "desktop",
        "key",
        "--target",
        "{}",
        "--surface",
        "surface",
        "arbitrary-bytes"
    ])
    .is_err());
}

#[test]
fn portable_help_describes_exact_kind_selection() {
    let repo = fixture();
    let home = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".lf/flows/team")).unwrap();
    fs::write(repo.path().join(".lf/flows/team/check.yaml"), "- solo\n").unwrap();
    fs::write(
        repo.path().join(".lf/skills/team-check.md"),
        "Explicit skill body",
    )
    .unwrap();
    let help =
        |args: &[&str]| String::from_utf8(success(run(repo.path(), home.path(), args))).unwrap();
    let flow = help(&["help", "team/check"]);
    assert!(flow.contains("team/check — flow"));
    assert!(flow.contains("team-check"));
    let skill = help(&["help", "team-check"]);
    assert!(skill.contains("team-check — skill"));
    assert!(!skill.contains("flow wins untyped lookup"));
    assert!(!skill.contains("Untyped lookup selects"));
    fs::write(repo.path().join(".lf/flows/team-check.yaml"), "- solo\n").unwrap();
    assert!(help(&["help", "team-check"]).contains("team-check — flow"));
    assert!(help(&["help", "skill", "team-check"])
        .contains("Untyped lookup selects the same-named flow"));
}
