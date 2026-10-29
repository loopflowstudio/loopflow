//! W2-241: the complete CLI Wave-resolution matrix for reads and mutations.
//!
//! Every Wave-scoped `lf` command — reads and mutations — runs across seven
//! ambient environments in one table-driven harness. All commands in the same
//! environment classify the same way. A completeness guard walks the clap tree
//! and fails CI when a new `--wave`-bearing command is not registered.

use crate::test_linear_input::{InventoryFixture, SeedLinearSqlite};
use std::collections::HashSet;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use clap::{ArgAction, CommandFactory};
use loopflow::id::WaveId;
use loopflow::lf::Cli;
use loopflow::store::sqlite::SqliteStore;
use loopflow::work::wave::Wave;

// ─── Command registry ───────────────────────────────────────────────────

/// How a command accepts its explicit `--wave` override.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WaveForm {
    /// `--wave <name>` flag on the subcommand.
    Flag,
    /// `<name>` positional on the subcommand.
    Positional,
}

struct Cmd {
    id: &'static str,
    /// Subcommand path for the completeness guard (e.g. `["wave", "connect"]`).
    path: &'static [&'static str],
    /// Full args after `lf` (subcommand path + extra flags/values).
    base_args: &'static [&'static str],
    wave_form: WaveForm,
    /// With no context, read all Waves instead of requiring one.
    global_default: bool,
}

/// Commands that resolve an ambient wave without a `wave` arg on the
/// subcommand itself — they inherit the top-level `--wave` or read
/// `LF_WAVE_ID` directly. The completeness guard checks
/// these exist as real clap leaves but does not discover them via the
/// `wave`-arg walk.
const AMBIENT_ONLY: &[&[&str]] = &[];

/// Task identity selects its owning Wave; an optional Wave only checks that
/// ownership. Ambient Wave selection must not redirect an explicit Task.
const ISSUE_OWNED: &[&[&str]] = &[&["task", "edit"], &["task", "comment"]];

/// Authored inputs do not select an ambient execution Wave: context reads
/// directories.
const AUTHORED_CONTEXT: &[&[&str]] = &[&["context"]];

/// Session entry uses an explicit Work target or checkout discovery, never an
/// ambient Wave. `session_launch_contract` covers both forms and their effects.
const SESSION_TARGETS: &[&[&str]] = &[&["session", "resume"], &["session", "new"]];

/// Membership changes name an explicit destination and preserve execution context.
/// `lf set` and `lf unset` take only the typed root `--wave`, else apply to the
/// checkout's repository; `wave_setup_tests::settings_scope_by_the_typed_wave_only`
/// covers them.
const MEMBERSHIP_TARGETS: &[&[&str]] = &[&["task", "move"]];

/// Commands whose optional `--wave` filters recorded results instead of
/// selecting ambient Wave context. These must not inherit `LF_WAVE_ID`.
/// Typed historical filters may resolve an explicit name to its stored ID.
const FILTER_ONLY: &[&[&str]] = &[
    &["task", "list"],
    &["desktop", "done"],
    &["history"],
    &["ops", "ci"],
    &["history", "list"],
    &["history", "search"],
    &["history", "usage"],
];

/// Rotation names its Wave or takes `--all`; it never resolves ambient context.
const EXPLICIT_OR_ALL: &[&[&str]] = &[&["wave", "new-chapter"]];

/// Commands that require a Wave on the command line and therefore never
/// resolve ambient context.
const EXPLICIT_WAVE_ONLY: &[&[&str]] = &[&["wave", "rename"], &["discord", "serve"]];

const COMMANDS: &[Cmd] = &[
    // ── Reads ────────────────────────────────────────────────────────────
    Cmd {
        id: "wave status",
        path: &["wave", "status"],
        base_args: &["wave", "status", "--json"],
        wave_form: WaveForm::Positional,
        global_default: false,
    },
    Cmd {
        id: "wave show",
        path: &["wave", "show"],
        base_args: &["wave", "show", "--json"],
        wave_form: WaveForm::Positional,
        global_default: true,
    },
    Cmd {
        id: "desktop wave",
        path: &["desktop", "wave"],
        base_args: &["desktop", "wave", "--json"],
        wave_form: WaveForm::Positional,
        global_default: false,
    },
    Cmd {
        id: "desktop plan",
        path: &["desktop", "plan"],
        base_args: &["desktop", "plan", "--json"],
        wave_form: WaveForm::Positional,
        global_default: true,
    },
    Cmd {
        id: "wave edit-plan",
        path: &["wave", "edit-plan"],
        base_args: &["wave", "edit-plan", "--summary", "Fixture"],
        wave_form: WaveForm::Positional,
        global_default: false,
    },
    Cmd {
        id: "wave graph show",
        path: &["wave", "graph", "show"],
        base_args: &["wave", "graph", "show", "--json"],
        wave_form: WaveForm::Positional,
        global_default: false,
    },
    // ── Mutations ────────────────────────────────────────────────────────
    Cmd {
        id: "wave connect",
        path: &["wave", "connect"],
        base_args: &["wave", "connect"],
        wave_form: WaveForm::Positional,
        global_default: false,
    },
    Cmd {
        id: "task create",
        path: &["task", "create"],
        base_args: &["task", "create", "--title", "Fixture task"],
        wave_form: WaveForm::Flag,
        global_default: false,
    },
    Cmd {
        id: "wave update-plan",
        path: &["wave", "update-plan"],
        base_args: &["wave", "update-plan", "--plan", "plan.json"],
        wave_form: WaveForm::Flag,
        global_default: false,
    },
];

// ─── Environments ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// Wave was resolved (command succeeded or failed downstream of resolution).
    Resolved,
    /// Resolver returned `StaleIdentity` — a UUID the registry has no row for.
    StaleIdentity,
    /// Resolver returned `NoContext` — no `--wave` and no `LF_WAVE_ID`.
    NoContext,
    /// Resolver rejected an explicit name absent from the registry.
    UnknownExplicit,
}

struct Env {
    id: &'static str,
    /// `LF_WAVE_ID` value (None = unset).
    wave_id: Option<String>,
    /// Explicit `--wave` value to pass (None = don't pass).
    explicit_wave: Option<String>,
    /// Expected outcome for standard commands in this environment.
    default_expected: Outcome,
}

fn make_envs(product_uuid: &str, stale_uuid: &str) -> Vec<Env> {
    vec![
        Env {
            id: "registered-uuid",
            wave_id: Some(product_uuid.to_string()),
            explicit_wave: None,
            default_expected: Outcome::Resolved,
        },
        Env {
            id: "registered-name",
            wave_id: Some("product".to_string()),
            explicit_wave: None,
            default_expected: Outcome::Resolved,
        },
        Env {
            id: "explicit-override",
            wave_id: Some(stale_uuid.to_string()),
            explicit_wave: Some("product".to_string()),
            default_expected: Outcome::Resolved,
        },
        Env {
            id: "stale-uuid",
            wave_id: Some(stale_uuid.to_string()),
            explicit_wave: None,
            default_expected: Outcome::StaleIdentity,
        },
        Env {
            id: "stale-name",
            wave_id: Some("ghost".to_string()),
            explicit_wave: None,
            default_expected: Outcome::StaleIdentity,
        },
        Env {
            id: "explicit-unknown",
            wave_id: None,
            explicit_wave: Some("unknown-explicit".to_string()),
            default_expected: Outcome::UnknownExplicit,
        },
        Env {
            id: "absent",
            wave_id: None,
            explicit_wave: None,
            default_expected: Outcome::NoContext,
        },
    ]
}

/// Expected outcome for a specific command × environment cell, accounting for
/// documented special cases.
fn expected_outcome(cmd: &Cmd, env: &Env) -> Outcome {
    // Creation ignores ambient context and may register an explicit destination.
    if cmd.id == "task create" {
        return Outcome::Resolved;
    }
    // Connection may register the selected Wave.
    if env.id == "explicit-unknown" && cmd.id == "wave connect" {
        return Outcome::Resolved;
    }

    if env.id == "absent" {
        if cmd.global_default {
            return Outcome::Resolved;
        }
        return Outcome::NoContext;
    }
    env.default_expected
}

// ─── Classification ─────────────────────────────────────────────────────

fn classify(output: &std::process::Output) -> Outcome {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stderr}{stdout}");
    let resolution_text = combined
        .lines()
        .filter(|line| !line.contains("ambient wave identity failed validation;"))
        .collect::<Vec<_>>()
        .join("\n");

    if !output.status.success() {
        if resolution_text.contains("is not registered on this machine") {
            return Outcome::UnknownExplicit;
        }
        if resolution_text.contains("owning Wave") && resolution_text.contains("is not registered")
        {
            return Outcome::StaleIdentity;
        }
        if resolution_text.contains("stale") {
            return Outcome::StaleIdentity;
        }
        if resolution_text.contains("determine wave")
            || resolution_text.contains("no wave")
            || resolution_text.contains("pass --wave")
            || resolution_text.contains("pass a wave")
            || resolution_text.contains("no wave given")
        {
            return Outcome::NoContext;
        }
        // Non-resolution error: the wave was resolved, the command failed
        // downstream (no Linear token, no registry row, git check, etc.).
        return Outcome::Resolved;
    }

    Outcome::Resolved
}

// ─── Helpers ────────────────────────────────────────────────────────────

fn open_test_store(path: &Path) -> SqliteStore {
    SqliteStore::new(path).expect("open store")
}

/// Seed a machine home with one registered wave ("product"), a PM snapshot,
/// a wave directory with MEMORY.md, and a git repo on a clean `main` branch.
fn seed(home: &Path, repo: &Path) -> Wave {
    std::fs::create_dir_all(home).expect("home");
    std::fs::create_dir_all(repo).expect("repo");

    // Resolution may reach chat connection. Keep process startup outside this
    // matrix; wave_start_tests owns the real daemon path and its cleanup.
    let bin = home.join("bin");
    std::fs::create_dir_all(&bin).expect("test bin");
    for name in ["tmux", "codex", "claude", "opencode"] {
        let executable = bin.join(name);
        std::fs::write(&executable, "#!/bin/sh\nexit 79\n").expect("fake executable");
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755))
            .expect("fake executable permissions");
    }

    // Task start requires a clean repository before reaching Wave resolution.
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .expect("git")
    };
    if !repo.join(".git").exists() {
        git(&["init", "-b", "main"]);
        git(&["config", "user.email", "test@loopflow.test"]);
        git(&["config", "user.name", "Matrix Test"]);
    }

    let store = open_test_store(&home.join("loopflow.db"));
    let wave = Wave::new(
        WaveId::new(),
        "product".to_string(),
        repo.display().to_string(),
    );
    store.create_wave(&wave).expect("register wave");
    store
        .seed_inventory(&InventoryFixture {
            wave_id: wave.id().clone(),
            provider: "linear".to_string(),
            initiative: "initiative-1".to_string(),
            synced_at: chrono::Utc::now().timestamp(),
            snapshot: crate::test_linear_input::Inventory {
                projects: vec![],
                items: vec![],
            },
        })
        .expect("seed pm snapshot");

    // Commit everything so the repo is clean.
    git(&["add", "."]);
    std::fs::write(
        repo.join("plan.json"),
        serde_json::to_vec(&serde_json::json!({"metric_targets":[], "krs":[]})).unwrap(),
    )
    .unwrap();
    git(&["add", "plan.json"]);

    let commit = git(&["commit", "-m", "seed", "--allow-empty"]);
    assert!(
        commit.status.success() || commit.status.code() == Some(1),
        "git commit failed: {}",
        String::from_utf8_lossy(&commit.stderr)
    );

    wave
}

/// Build the full CLI args for a command × environment cell.
fn build_args(cmd: &Cmd, env: &Env) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    let explicit = env.explicit_wave.as_deref();

    args.extend(cmd.base_args.iter().map(|s| s.to_string()));
    if let Some(w) = explicit {
        match cmd.wave_form {
            WaveForm::Flag => {
                args.push("--wave".to_string());
                args.push(w.to_string());
            }
            WaveForm::Positional => {
                args.push(w.to_string());
            }
        }
    }
    args
}

/// Run `lf` with the given home, repo, command, and environment. Returns the
/// process output (stdout, stderr, exit code). The enclosing proof phase owns its timeout.
fn run_lf(home: &Path, repo: &Path, cmd: &Cmd, env: &Env) -> std::process::Output {
    let args = build_args(cmd, env);

    let mut command = Command::new(loopflow_test_support::lf_binary());
    command
        .args(&args)
        .current_dir(repo)
        .env("LF_HOME", home)
        .env(
            "PATH",
            format!(
                "{}:{}",
                home.join("bin").display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("HOME", home)
        .env_remove("LF_TRACE_ID")
        .env_remove("LF_WAVE_ID");

    if let Some(id) = &env.wave_id {
        command.env("LF_WAVE_ID", id);
    }

    command.output().expect("lf runs")
}

// ─── Matrix test ────────────────────────────────────────────────────────

/// Every Wave-scoped command × every environment. The expected outcome is
/// shared per environment (with documented special cases for roadmap). A
/// divergence — a command that silently drops a stale UUID or
/// invents its own resolution rule — fails a cell.
#[test]
fn matrix_every_command_every_environment() {
    let mut failures = Vec::new();
    let mut total = 0usize;
    let tmp = tempfile::tempdir().unwrap();
    let cell = tmp.path().join("cell");
    let pristine = tmp.path().join("pristine");
    let home = cell.join("home");
    let repo = cell.join("repo");
    let wave = seed(&home, &repo);
    // Seed once, after closing the store, then restore independent bytes for
    // each command. Keep the original paths: the registry stores absolute paths.
    // Sharing live state would let Task creation provision a later cell's Wave.
    std::fs::rename(&cell, &pristine).unwrap();
    for index in 0..make_envs("", "").len() {
        for cmd in COMMANDS {
            assert!(Command::new("cp")
                .arg("-R")
                .arg(&pristine)
                .arg(&cell)
                .status()
                .unwrap()
                .success());
            let env = make_envs(wave.id().as_str(), WaveId::new().as_str()).remove(index);
            total += 1;
            let output = run_lf(&home, &repo, cmd, &env);
            std::fs::remove_dir_all(&cell).unwrap();
            let outcome = classify(&output);
            let expected = expected_outcome(cmd, &env);
            if outcome != expected {
                failures.push(format!(
                    "  `{}` in `{}` → {:?} (expected {:?})\n    exit: {}\n    stdout: {}\n    stderr: {}",
                    cmd.id, env.id, outcome, expected, output.status,
                    String::from_utf8_lossy(&output.stdout).trim(),
                    String::from_utf8_lossy(&output.stderr).trim(),
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "matrix had {} failure(s) out of {} cells:\n{}",
            failures.len(),
            total,
            failures.join("\n")
        );
    }
}

// ─── Completeness guard ─────────────────────────────────────────────────

/// Recursively walk the clap command tree and collect every command path that
/// has a non-required, non-Vec `wave` arg (by `long == "wave"` or `id ==
/// "wave"`). Required `wave: String` (always-explicit) and `wave: Vec<String>`
/// (filter, not target) are excluded.
fn collect_wave_arg_commands(
    cmd: &clap::Command,
    path: &mut Vec<String>,
    found: &mut Vec<Vec<String>>,
) {
    for sub in cmd.get_subcommands() {
        path.push(sub.get_name().to_string());

        let has_optional_wave = sub.get_arguments().any(|arg| {
            let is_wave = arg.get_long() == Some("wave") || arg.get_id() == "wave";
            let is_required = arg.is_required_set();
            let is_vec = matches!(arg.get_action(), ArgAction::Append);
            is_wave && !is_required && !is_vec
        });
        if has_optional_wave {
            found.push(path.clone());
        }

        collect_wave_arg_commands(sub, path, found);
        path.pop();
    }
}

/// Navigate the clap tree by subcommand names. Returns the leaf command if
/// found.
fn find_clap_command<'a>(root: &'a clap::Command, path: &[&str]) -> Option<&'a clap::Command> {
    let mut current = root;
    for name in path {
        current = current.find_subcommand(name)?;
    }
    Some(current)
}

/// The registry is complete: every `wave`-bearing clap leaf is classified as
/// a resolver, filter, Task owner or authored context, every cron leaf has exactly one
/// Wave-context classification, every ambient/explicit-only command exists as
/// a real clap leaf, and every registry entry maps to a real clap leaf. Adding
/// a new `--wave`-bearing command without classifying it fails CI; removing a
/// command leaves a stale entry that also fails.
#[test]
fn registry_is_complete() {
    let root = Cli::command();

    // 1. Discover all wave-arg commands in the clap tree.
    let mut found = Vec::new();
    collect_wave_arg_commands(&root, &mut Vec::new(), &mut found);
    found.sort();
    found.dedup();

    // 2. Build the registry path set from COMMANDS.
    let registry_paths: HashSet<Vec<String>> = COMMANDS
        .iter()
        .map(|c| c.path.iter().map(|s| s.to_string()).collect())
        .collect();
    let filter_paths: HashSet<Vec<String>> = FILTER_ONLY
        .iter()
        .chain(ISSUE_OWNED)
        .chain(AUTHORED_CONTEXT)
        .chain(SESSION_TARGETS)
        .chain(MEMBERSHIP_TARGETS)
        .chain(EXPLICIT_OR_ALL)
        .map(|path| path.iter().map(|s| s.to_string()).collect())
        .collect();

    // 3. Every wave-arg clap command must be classified.
    for path in &found {
        assert!(
            registry_paths.contains(path) || filter_paths.contains(path),
            "clap command {:?} has an optional `wave` arg but is not classified — \
             classify its Wave selection in the command registry",
            path
        );
    }

    // 4. Every ambient-only, filter-only, and explicit-only command must exist
    //    as a real clap leaf. Explicit-only commands must require `wave`.
    for path in AMBIENT_ONLY
        .iter()
        .chain(FILTER_ONLY)
        .chain(ISSUE_OWNED)
        .chain(AUTHORED_CONTEXT)
        .chain(SESSION_TARGETS)
        .chain(MEMBERSHIP_TARGETS)
        .chain(EXPLICIT_OR_ALL)
    {
        assert!(
            find_clap_command(&root, path).is_some(),
            "classified command {:?} does not exist in the clap tree",
            path
        );
    }
    for path in EXPLICIT_WAVE_ONLY {
        let command = find_clap_command(&root, path).unwrap_or_else(|| {
            panic!("classified command {path:?} does not exist in the clap tree")
        });
        assert!(
            command.get_arguments().any(|arg| {
                (arg.get_long() == Some("wave") || arg.get_id() == "wave")
                    && arg.is_required_set()
                    && !matches!(arg.get_action(), ArgAction::Append)
            }),
            "explicit-only command {path:?} must require one `wave` argument"
        );
    }

    // 5. Every registry entry must map to a real clap command (no stale
    //    entries). Ambient-only commands are checked above.
    let ambient_set: HashSet<Vec<String>> = AMBIENT_ONLY
        .iter()
        .map(|p| p.iter().map(|s| s.to_string()).collect())
        .collect();

    for cmd in COMMANDS {
        let path: Vec<&str> = cmd.path.to_vec();
        if ambient_set.contains(&cmd.path.iter().map(|s| s.to_string()).collect::<Vec<_>>()) {
            continue;
        }
        assert!(
            find_clap_command(&root, &path).is_some(),
            "registry entry `{}` ({:?}) does not map to a real clap command",
            cmd.id,
            cmd.path
        );
    }
}
