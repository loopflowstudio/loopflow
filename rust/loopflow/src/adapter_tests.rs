//! Contracts between typed operations and the CLI/presentation adapters.
use loopflow_execution::ops::context::ContextFact;
use loopflow_execution::ops::task::TaskProcessOptions;
use loopflow_execution::test_support::human_session::SessionHome;
use std::fs;
#[test]
fn no_raw_leaves_raw_conversations_out_of_the_list() {
    use clap::Parser;
    let lists_raw = |argv: &[&str]| {
        let cli = crate::lf::Cli::try_parse_from(argv).unwrap();
        let Some(crate::lf::Commands::Session {
            cmd: crate::lf::SessionCommand::List(args),
        }) = cli.command
        else {
            panic!("not a session list");
        };
        let filter =
            crate::lf::commands::session::list_filter(&args, std::path::Path::new("/")).unwrap();
        filter.raw
    };
    assert!(lists_raw(&["lf", "session", "list", "--all"]));
    assert!(lists_raw(&["lf", "session", "list", "--all", "--headless"]));
    assert!(lists_raw(&[
        "lf",
        "session",
        "list",
        "--all",
        "--available"
    ]));
    assert!(!lists_raw(&["lf", "session", "list", "--all", "--no-raw"]));
}

#[test]
fn previews_without_a_readable_registry_are_unavailable_and_create_nothing() {
    let _lock = loopflow_execution::journal::test_env_lock();
    let home = SessionHome::new();
    let database = home.path().join("loopflow.db");
    let options = TaskProcessOptions::default();
    for corrupt in [false, true] {
        if corrupt {
            fs::write(&database, "not a database").unwrap();
        }
        let run =
            crate::lf::commands::context::explain_task_run(Some("UNKNOWN-1"), &options, false)
                .unwrap();
        assert!(run.action.is_none());
        assert!(matches!(
            run.resolution.task,
            ContextFact::Unavailable { .. }
        ));
        assert!(!run.unavailable.is_empty());
        let checkout =
            crate::lf::commands::context::explain_task_checkout(Some("UNKNOWN-1"), &options)
                .unwrap();
        assert!(checkout.action.is_none());
        match corrupt {
            true => assert_eq!(fs::read(&database).unwrap(), b"not a database"),
            false => assert!(!database.exists()),
        }
    }
}

// ---- Task checkout: reuse, restore or prepare ----

#[test]
fn task_sync_has_no_independent_resolve_flag() {
    use clap::Parser;
    assert!(
        crate::lf::Cli::try_parse_from(["lf", "task", "sync", "FIX-7", "--resolve", "local"])
            .is_err()
    );
}

#[cfg(test)]
mod operation_spellings {
    use clap::Parser;

    use crate::lf::{Cli, Commands, TaskCommand};
    use loopflow_execution::engine::{checkout, graph, pr, repo, task};

    #[test]
    fn every_child_spelling_parses() {
        let parse = |args: Vec<String>| {
            Cli::try_parse_from(std::iter::once("lf").chain(args.iter().map(String::as_str)))
                .and_then(Cli::checked)
                .unwrap_or_else(|error| panic!("{args:?}: {error}"))
        };
        assert!(matches!(
            parse(task::spell_auto_on("LOO-1")).command,
            Some(Commands::Task {
                cmd: crate::lf::TaskCommand::Auto {
                    cmd: crate::lf::AutoCommand::On { json: true, .. }
                }
            })
        ));
        for populated in [false, true] {
            let optional = |value| populated.then_some(value);
            let cli = parse(task::spell_run(
                "LOO-1",
                optional("implement"),
                optional("code"),
                optional("codex"),
                populated,
            ));
            assert!(matches!(
                cli.command,
                Some(Commands::Task {
                    cmd: TaskCommand::Run { json: true, .. }
                })
            ));
            for noun in ["graph", "skill"] {
                let cli = parse(graph::spell(
                    noun,
                    "example",
                    optional("Keep it small"),
                    optional("codex"),
                ));
                assert!(cli.batch);
            }
            parse(repo::spell_release(optional("minor"), optional("lf")));
            parse(pr::spell_land(&pr::LandInput {
                strict: populated,
                local: populated,
                wait_and_fix: populated,
                worktree: optional("checkout").map(str::to_owned),
                message: optional("Commit message").map(str::to_owned),
                title: optional("PR title").map(str::to_owned),
                body: optional("PR body").map(str::to_owned),
                agent: optional("codex").map(str::to_owned),
            }));
            parse(checkout::spell_sync(&checkout::SyncInput {
                onto: optional("main"),
                manual: populated,
                preview: false,
                continue_sync: false,
                abort: false,
                adopt: false,
            }));
        }
    }
}

#[cfg(test)]
mod graph_instructions {
    use crate::lf::{navigation::normalize_args, Cli, Commands, SkillCommand};
    use clap::Parser;
    use loopflow_execution::graph::instructions::render_graph_instructions;
    use loopflow_execution::graph::{compile_graph, load_authored_graph};
    use std::fs;
    use tempfile::TempDir;

    fn recipe(repo: &std::path::Path, name: &str) -> String {
        let graph = load_authored_graph(name, repo).unwrap();
        let steps = compile_graph(&graph, repo).unwrap();
        render_graph_instructions(name, &steps, &mut |skill| {
            Ok(std::path::PathBuf::from("/captures").join(format!("{}.json", skill.name)))
        })
        .unwrap()
    }

    #[test]
    fn chat_recipe_runs_work_and_keeps_reviews_and_nested_decisions_here() {
        let repo = TempDir::new().unwrap();
        fs::create_dir_all(repo.path().join(".lf/graphs")).unwrap();
        fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
        fs::write(
            repo.path().join(".lf/skills/work.md"),
            "Work body must not be bundled.",
        )
        .unwrap();
        fs::write(
            repo.path().join(".lf/graphs/repair.yaml"),
            "- loop:\n    - work\n    - loop-or-next\n",
        )
        .unwrap();
        fs::write(
            repo.path().join(".lf/graphs/chat.yaml"),
            r#"
- loop:
    - step: {name: work, id: first}
    - cmd: commit -m don't-expand-$HOME;echo-unsafe
    - xor:
        paths:
          repair:
            graph: repair
            description: Repair the findings
          skip:
            description: Nothing to repair
    - loop-or-next
- interactive: {name: demo, id: review}
"#,
        )
        .unwrap();
        let plan = recipe(repo.path(), "chat");
        assert!(plan.contains("1. Run `lf -b --skill-input /captures/work.json skill -- work`."));
        assert!(plan.contains("repeat steps 1 through 3"));
        assert!(plan.contains("repeat steps 3[repair].1 through 3[repair].1"));
        assert!(plan.contains("reassess at step 3[repair].2"));
        assert!(plan.contains("rejoin after step 3"));
        assert!(plan.contains("5. Execute the skill `demo` in this conversation."));
        assert!(plan.contains("Wait for the participant's response"));
        assert!(plan.contains("Repair the findings"));
        assert!(plan.contains("A deferred check alone is not a blocker"));
        assert!(!plan.contains("Work body must not be bundled"));
        assert!(!plan.contains("JSON"));
        assert!(!plan.contains("Resolved skill bodies"));
        let command = plan
            .split("```sh\n")
            .nth(1)
            .unwrap()
            .split("\n```")
            .next()
            .unwrap();
        assert_eq!(
            shlex::split(command).unwrap(),
            vec!["lf", "commit", "-m", "don't-expand-$HOME;echo-unsafe"]
        );
    }

    #[test]
    fn chat_recipe_preserves_overridden_loop_and_router_skills() {
        let repo = TempDir::new().unwrap();
        fs::create_dir_all(repo.path().join(".lf/graphs")).unwrap();
        fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
        for name in ["loop-or-next", "xor-route"] {
            fs::write(
                repo.path().join(format!(".lf/skills/{name}.md")),
                "Custom criteria",
            )
            .unwrap();
        }
        fs::write(repo.path().join(".lf/graphs/chat.yaml"), "- loop:\n    - implement\n    - loop-or-next\n- xor:\n    paths:\n      done:\n        description: Finished\n").unwrap();
        let plan = recipe(repo.path(), "chat");
        for name in ["loop-or-next", "xor-route"] {
            assert!(plan.contains(&format!("Execute the skill `{name}` in this conversation.")));
            assert!(plan.contains(
                &repo
                    .path()
                    .join(format!(".lf/skills/{name}.md"))
                    .display()
                    .to_string()
            ));
        }
        assert!(plan.contains("iterate repeats steps 1 through 1"));
        assert!(!plan.contains("If the last pass made meaningful progress"));
    }

    #[test]
    fn chat_recipe_uses_explicit_skills_for_graph_and_command_collisions() {
        let repo = TempDir::new().unwrap();
        fs::create_dir_all(repo.path().join(".lf/graphs")).unwrap();
        fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
        fs::write(repo.path().join(".lf/skills/sync.md"), "Skill, not command").unwrap();
        fs::write(repo.path().join(".lf/skills/ship.md"), "Skill, not Graph").unwrap();
        fs::write(
            repo.path().join(".lf/skills/defaulted.md"),
            "---\nagent: claude\n---\nUse the configured preference.",
        )
        .unwrap();
        fs::write(repo.path().join(".lf/graphs/ship.yaml"), "- implement\n").unwrap();
        fs::write(
            repo.path().join(".lf/graphs/chat.yaml"),
            "- step: sync\n- step: ship\n- step: {name: implement, agent: codex:mini}\n- defaulted\n",
        )
        .unwrap();
        let plan = recipe(repo.path(), "chat");
        assert!(plan.contains("Run `lf -b --skill-input /captures/sync.json skill -- sync`"));
        assert!(plan.contains("Run `lf -b --skill-input /captures/ship.json skill -- ship`"));
        assert!(
            plan.contains("Run `lf -b --skill-input /captures/implement.json skill -- implement`")
        );
        assert!(
            plan.contains("Run `lf -b --skill-input /captures/defaulted.json skill -- defaulted`")
        );
    }

    #[test]
    fn chat_recipe_preserves_shell_characters_and_cli_syntax_in_skill_names() {
        let repo = TempDir::new().unwrap();
        fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
        fs::create_dir_all(repo.path().join(".lf/graphs")).unwrap();
        for name in ["--help", "don't-expand-$HOME;echo-unsafe"] {
            fs::write(repo.path().join(format!(".lf/skills/{name}.md")), "Work").unwrap();
            fs::write(
                repo.path().join(".lf/graphs/chat.yaml"),
                format!("- step: {}\n", serde_json::to_string(name).unwrap()),
            )
            .unwrap();
            let plan = recipe(repo.path(), "chat");
            let command = plan
                .split("1. Run `")
                .nth(1)
                .unwrap()
                .split('`')
                .next()
                .unwrap();
            let argv = shlex::split(command).unwrap();
            let cli = Cli::try_parse_from(normalize_args(argv).unwrap()).unwrap();
            assert!(cli.batch);
            match cli.command.unwrap() {
                Commands::Skill {
                    cmd: SkillCommand::External(args),
                }
                | Commands::External(args) => assert_eq!(args, [name]),
                command => panic!("expected skill invocation, got {command:?}"),
            }
        }
    }
}

#[test]
fn configuration_write_rejects_a_scope_outside_its_destination() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/configuration_destinations.json"
    ))
    .unwrap();
    for destination in fixture.as_array().unwrap() {
        let mut request = serde_json::json!({"kind":"config.set", "destination":destination,
                "key":"agent", "value":"codex"});
        assert!(
            serde_json::from_value::<crate::lf::commands::write::Write>(request.clone()).is_ok()
        );
        request["scope"] = "global".into();
        assert!(serde_json::from_value::<crate::lf::commands::write::Write>(request).is_err());
    }
}

#[test]
fn the_card_for_a_raw_conversation_carries_its_turn() {
    use crate::lf::commands::screens::{SessionCard, SessionLiveness};
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/session_card.json"
    ))
    .unwrap();
    let mut value = fixture["session"].clone();
    value["id"] = "provider-id".into();
    value["raw"] = true.into();
    value["state"] = "active".into();
    value["turn"]["state"] = "running".into();
    value["turn"]["ended_at"] = serde_json::Value::Null;
    let session: loopflow_execution::ops::human_session::SessionRecord =
        serde_json::from_value(value).unwrap();
    let card = SessionCard::from(session);
    assert!(card.session.raw);
    assert_eq!(card.session.id.as_str(), "provider-id");
    assert_eq!(
        card.session.turn.unwrap().state,
        loopflow_harness::transcript::live::TurnState::Running
    );
    assert_eq!(card.liveness, SessionLiveness::Live);
    assert_eq!(card.control_holder, None);
    assert_eq!(card.last_line, None);
}
