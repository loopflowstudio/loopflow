//! W2-241: the complete CLI Wave-resolution matrix for reads and mutations.
//!
//! Every Wave-scoped `lf` command — reads and mutations — runs across seven
//! ambient environments in one table-driven harness. All commands in the same
//! environment classify the same way. A completeness guard walks the clap tree
//! and fails CI when a new `--wave`-bearing command is not registered.
//!
//! The remaining divergences W2-151 left behind (`lf home probe`, `lf roadmap`,
//! `lf project start`, `lf project promote`) were
//! fixed on main before this test shipped. The matrix is the proof they stay
//! fixed: any command that silently drops a stale UUID or invents its own
//! resolution rule fails a cell.

use std::collections::HashSet;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

use clap::{ArgAction, CommandFactory};
use loopflow::id::WaveId;
use loopflow::lf::Cli;
use loopflow::store::sqlite::SqliteStore;
use loopflow::store::PmSnapshotRow;
use loopflow::wave::Wave;

// ─── Command registry ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Read,
    Mutation,
}

/// How a command accepts its explicit `--wave` override.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WaveForm {
    /// `--wave <name>` flag on the subcommand.
    Flag,
    /// `<name>` positional on the subcommand.
    Positional,
    /// `WaveTargetArgs` flattened: `--wave <name>`.
    Target,
}

#[derive(Debug, Clone, Copy, Default)]
struct Special {
    /// `NoContext` → proceed globally (list all waves, sync all waves) — exit
    /// 0 or downstream error, not a resolution error. Roadmap and `pm status`
    /// behave this way.
    global_default: bool,
    /// `NoContext` → exit 0 "dropped" (publish-to-no-subscriber). `chat post`
    /// drops silently instead of erroring.
    silent_drop: bool,
    /// Pipes this text to stdin (for commands whose `trailing_var_arg` would
    /// swallow `--wave` if text were on the command line).
    stdin: Option<&'static str>,
}

impl Special {
    const NONE: Self = Self {
        global_default: false,
        silent_drop: false,
        stdin: None,
    };
}

struct Cmd {
    id: &'static str,
    /// Subcommand path for the completeness guard (e.g. `["pm", "show"]`).
    path: &'static [&'static str],
    /// Full args after `lf` (subcommand path + extra flags/values).
    base_args: &'static [&'static str],
    wave_form: WaveForm,
    kind: Kind,
    special: Special,
}

/// Commands that resolve an ambient wave without a `wave` arg on the
/// subcommand itself — they inherit the top-level `--wave` or read
/// `LF_WAVE_ID` directly. The completeness guard checks
/// these exist as real clap leaves but does not discover them via the
/// `wave`-arg walk.
const AMBIENT_ONLY: &[&[&str]] = &[];

/// Commands whose optional `--wave` narrows a machine-wide result instead of
/// selecting ambient Wave context. These must not inherit `LF_WAVE_ID` or
/// reject names absent from the registry.
const FILTER_ONLY: &[&[&str]] = &[&["activity"], &["ci"], &["cron", "list"], &["runs"]];

/// Commands that require a Wave on the command line and therefore never
/// resolve ambient context. Cron keeps these explicit because scheduled host
/// operations must name the installed Wave whose authority they validate.
const EXPLICIT_WAVE_ONLY: &[&[&str]] = &[
    &["cron", "preflight"],
    &["cron", "sync"],
    &["cron", "run"],
    &["cron", "history"],
    &["cron", "trigger"],
    &["cron", "remove"],
];

const COMMANDS: &[Cmd] = &[
    // ── Reads ────────────────────────────────────────────────────────────
    Cmd {
        id: "status",
        path: &["status"],
        base_args: &["status", "--json"],
        wave_form: WaveForm::Positional,
        kind: Kind::Read,
        special: Special::NONE,
    },
    Cmd {
        id: "roadmap",
        path: &["roadmap"],
        base_args: &["roadmap", "--json"],
        wave_form: WaveForm::Flag,
        kind: Kind::Read,
        special: Special {
            global_default: true,
            ..Special::NONE
        },
    },
    Cmd {
        id: "pm show",
        path: &["pm", "show"],
        base_args: &["pm", "show", "--no-sync", "--json"],
        wave_form: WaveForm::Flag,
        kind: Kind::Read,
        special: Special::NONE,
    },
    Cmd {
        id: "pm status",
        path: &["pm", "status"],
        base_args: &["pm", "status"],
        wave_form: WaveForm::Flag,
        kind: Kind::Read,
        special: Special {
            global_default: true,
            ..Special::NONE
        },
    },
    Cmd {
        id: "chat history",
        path: &["chat"],
        base_args: &["chat", "--history", "--json"],
        wave_form: WaveForm::Target,
        kind: Kind::Read,
        special: Special::NONE,
    },
    Cmd {
        id: "home probe",
        path: &["home", "probe"],
        base_args: &["home", "probe", "--json"],
        wave_form: WaveForm::Positional,
        kind: Kind::Read,
        special: Special::NONE,
    },
    Cmd {
        id: "pm sync plan",
        path: &["pm", "sync"],
        base_args: &["pm", "sync", "--plan"],
        wave_form: WaveForm::Flag,
        kind: Kind::Read,
        special: Special {
            global_default: true,
            ..Special::NONE
        },
    },
    // ── Mutations ────────────────────────────────────────────────────────
    // `chat post` uses stdin for text: its `trailing_var_arg` would swallow
    // `--wave` if text were on the command line.
    Cmd {
        id: "chat post",
        path: &["chat"],
        base_args: &["chat"],
        wave_form: WaveForm::Target,
        kind: Kind::Mutation,
        special: Special {
            silent_drop: true,
            stdin: Some("matrix-test-message\n"),
            ..Special::NONE
        },
    },
    Cmd {
        id: "pm init",
        path: &["pm", "init"],
        base_args: &["pm", "init"],
        wave_form: WaveForm::Flag,
        kind: Kind::Mutation,
        special: Special::NONE,
    },
    Cmd {
        id: "pm sync",
        path: &["pm", "sync"],
        base_args: &["pm", "sync"],
        wave_form: WaveForm::Flag,
        kind: Kind::Mutation,
        special: Special {
            global_default: true,
            ..Special::NONE
        },
    },
    Cmd {
        id: "pm rename",
        path: &["pm", "rename"],
        base_args: &["pm", "rename", "--title", "Renamed"],
        wave_form: WaveForm::Flag,
        kind: Kind::Mutation,
        special: Special::NONE,
    },
    Cmd {
        id: "pm task create",
        path: &["pm", "task", "create"],
        base_args: &["pm", "task", "create", "--project", "test", "--title", "T"],
        wave_form: WaveForm::Flag,
        kind: Kind::Mutation,
        special: Special::NONE,
    },
    Cmd {
        id: "pm task update",
        path: &["pm", "task", "update"],
        base_args: &["pm", "task", "update", "--id", "W2-999"],
        wave_form: WaveForm::Flag,
        kind: Kind::Mutation,
        special: Special::NONE,
    },
    Cmd {
        id: "pm task done",
        path: &["pm", "task", "done"],
        base_args: &["pm", "task", "done", "--id", "W2-999"],
        wave_form: WaveForm::Flag,
        kind: Kind::Mutation,
        special: Special::NONE,
    },
    Cmd {
        id: "pm task move",
        path: &["pm", "task", "move"],
        base_args: &["pm", "task", "move", "--id", "W2-999", "--project", "test"],
        wave_form: WaveForm::Flag,
        kind: Kind::Mutation,
        special: Special::NONE,
    },
    Cmd {
        id: "pm project create",
        path: &["pm", "project", "create"],
        base_args: &[
            "pm",
            "project",
            "create",
            "--title",
            "T",
            "--definition",
            "D",
            "--kr",
            "K",
        ],
        wave_form: WaveForm::Flag,
        kind: Kind::Mutation,
        special: Special::NONE,
    },
    Cmd {
        id: "pm project update",
        path: &["pm", "project", "update"],
        base_args: &[
            "pm",
            "project",
            "update",
            "--project",
            "test",
            "--definition",
            "D",
            "--kr",
            "K",
        ],
        wave_form: WaveForm::Flag,
        kind: Kind::Mutation,
        special: Special::NONE,
    },
    Cmd {
        id: "pm project archive",
        path: &["pm", "project", "archive"],
        base_args: &["pm", "project", "archive", "--project", "test"],
        wave_form: WaveForm::Flag,
        kind: Kind::Mutation,
        special: Special::NONE,
    },
    Cmd {
        id: "cron add",
        path: &["cron", "add"],
        base_args: &[
            "cron",
            "add",
            "--flow",
            "matrix-test-flow",
            "--schedule",
            "daily",
        ],
        wave_form: WaveForm::Flag,
        kind: Kind::Mutation,
        special: Special::NONE,
    },
    Cmd {
        id: "project start",
        path: &["project", "start"],
        base_args: &["project", "start", "Test Project"],
        wave_form: WaveForm::Flag,
        kind: Kind::Mutation,
        special: Special::NONE,
    },
    Cmd {
        id: "project promote",
        path: &["project", "promote"],
        base_args: &["project", "promote", "test-slug"],
        wave_form: WaveForm::Flag,
        kind: Kind::Mutation,
        special: Special::NONE,
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
    /// Publish-to-no-subscriber: no wave resolved → exit 0 "dropped".
    Drop,
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
            default_expected: Outcome::Resolved,
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
    // Creation flows may name the Wave being registered.
    if env.id == "explicit-unknown" && cmd.id == "pm init" {
        return Outcome::Resolved;
    }

    // Project start now resolves caller authority at the CLI surface before
    // creating anything. An inherited hand-set name without a registry row is
    // stale transport evidence, not an explicit target selection.
    if cmd.id == "project start" && env.id == "stale-name" {
        return Outcome::StaleIdentity;
    }

    if env.id == "absent" {
        if cmd.special.silent_drop {
            return Outcome::Drop;
        }
        if cmd.special.global_default {
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
        .filter(|line| !line.contains("ambient wave identity failed validation; run attributed"))
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

    // Exit 0
    if combined.contains("dropped") || combined.contains("nothing to tune in to") {
        return Outcome::Drop;
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

    // `project promote` reaches an authored agent flow after successful Wave
    // resolution. Keep this resolution test hermetic instead of invoking the
    // developer's real provider CLI.
    let bin = home.join("bin");
    std::fs::create_dir_all(&bin).expect("test bin");
    let codex = bin.join("codex");
    std::fs::write(&codex, "#!/bin/sh\nexit 1\n").expect("fake codex");
    std::fs::set_permissions(&codex, std::fs::Permissions::from_mode(0o755))
        .expect("fake codex permissions");

    // Git repo on a clean main — `lf project start` requires this before
    // reaching wave resolution.
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
    let repo_key = std::fs::canonicalize(repo)
        .expect("canonicalize repo")
        .display()
        .to_string();
    store
        .put_pm_snapshot(&PmSnapshotRow {
            repo: repo_key,
            wave: "product".to_string(),
            provider: "linear".to_string(),
            initiative: "initiative-1".to_string(),
            synced_at: chrono::Utc::now().timestamp(),
            payload: r#"{"projects":[],"items":[]}"#.to_string(),
        })
        .expect("seed pm snapshot");

    // Commit everything so the repo is clean.
    git(&["add", "."]);
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
            WaveForm::Flag | WaveForm::Target => {
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
/// process output (stdout, stderr, exit code). Long-running commands are
/// killed after a timeout; stdin-needing commands receive piped input.
fn run_lf(home: &Path, repo: &Path, cmd: &Cmd, env: &Env) -> std::process::Output {
    let args = build_args(cmd, env);

    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
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
        // Redirect HOME so `lf cron add` writes plists into the temp dir,
        // not the real ~/Library/LaunchAgents.
        .env("HOME", home)
        .env_remove("LF_DB_PATH")
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
        .env_remove("LF_TRACE_ID")
        .env_remove("LF_WAVE_ID");

    if let Some(id) = &env.wave_id {
        command.env("LF_WAVE_ID", id);
    }
    if let Some(stdin_text) = cmd.special.stdin {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("spawn");
        // Write stdin on a separate thread so wait_with_output can drain
        // stdout/stderr concurrently — a child that errors before reading
        // stdin must not deadlock the pipe.
        let stdin = child.stdin.take();
        let text = stdin_text.to_string();
        let handle = std::thread::spawn(move || {
            if let Some(mut stdin) = stdin {
                let _ = stdin.write_all(text.as_bytes());
            }
        });
        let output = child.wait_with_output().expect("wait");
        let _ = handle.join();
        return output;
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
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path().join("home");
    let repo = tmp.path().join("repo");
    let wave = seed(&home, &repo);
    let product_uuid = wave.id().as_str().to_string();
    let stale_uuid = WaveId::new().to_string();
    let envs = make_envs(&product_uuid, &stale_uuid);

    let mut failures = Vec::new();
    let mut total = 0usize;

    for env in &envs {
        // Run reads before mutations so `lf project start` sees a clean repo.
        let mut reads: Vec<&Cmd> = Vec::new();
        let mut mutations: Vec<&Cmd> = Vec::new();
        for cmd in COMMANDS {
            if matches!(cmd.kind, Kind::Read) {
                reads.push(cmd);
            } else {
                mutations.push(cmd);
            }
        }

        for cmd in reads.iter().chain(mutations.iter()) {
            // `lf project start` calls `ensure_clean_main` before wave
            // resolution. Earlier mutations can dirty the repo; reset so
            // project start reaches the resolver.
            if cmd.id == "project start" {
                let _ = std::process::Command::new("git")
                    .args(["reset", "--hard", "HEAD"])
                    .current_dir(&repo)
                    .output();
                let _ = std::process::Command::new("git")
                    .args(["clean", "-fdx"])
                    .current_dir(&repo)
                    .output();
            }

            total += 1;
            let output = run_lf(&home, &repo, cmd, env);
            let outcome = classify(&output);
            let expected = expected_outcome(cmd, env);

            if outcome != expected {
                failures.push(format!(
                    "  `{}` in `{}` → {:?} (expected {:?})\n    exit: {}\n    stdout: {}\n    stderr: {}",
                    cmd.id,
                    env.id,
                    outcome,
                    expected,
                    output.status,
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
/// either a resolver or a machine-wide filter, every cron leaf has exactly one
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
        .map(|path| path.iter().map(|s| s.to_string()).collect())
        .collect();
    let explicit_paths: HashSet<Vec<String>> = EXPLICIT_WAVE_ONLY
        .iter()
        .map(|path| path.iter().map(|s| s.to_string()).collect())
        .collect();

    // 3. Every wave-arg clap command must be classified.
    for path in &found {
        assert!(
            registry_paths.contains(path) || filter_paths.contains(path),
            "clap command {:?} has an optional `wave` arg but is not classified — \
             add resolvers to COMMANDS or machine-wide filters to FILTER_ONLY",
            path
        );
    }

    // 4. Every cron leaf must be classified exactly once. Required-wave cron
    //    commands do not appear in the optional-wave discovery above.
    let cron = root
        .find_subcommand("cron")
        .expect("cron command must exist");
    for subcommand in cron.get_subcommands() {
        let path = vec!["cron".to_string(), subcommand.get_name().to_string()];
        let classifications = usize::from(registry_paths.contains(&path))
            + usize::from(filter_paths.contains(&path))
            + usize::from(explicit_paths.contains(&path));
        assert_eq!(
            classifications, 1,
            "cron command {path:?} must have exactly one Wave-context classification"
        );
    }

    // 5. Every ambient-only, filter-only, and explicit-only command must exist
    //    as a real clap leaf. Explicit-only commands must require `wave`.
    for path in AMBIENT_ONLY.iter().chain(FILTER_ONLY) {
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

    // 6. Every registry entry must map to a real clap command (no stale
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

// ─── Mutation targeting ─────────────────────────────────────────────────

/// Cron installation refuses a development binary before writing host state.
/// The matrix above already proves the command resolves an ambient Wave; this
/// boundary proves that successful resolution cannot leave a disposable binary
/// in launchd.
#[test]
fn cron_add_rejects_a_development_binary_before_mutation() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path().join("home");
    let repo = tmp.path().join("repo");

    std::fs::create_dir_all(&home).expect("home");
    std::fs::create_dir_all(&repo).expect("repo");

    // Git repo — `lf cron add` calls `find_repo_root` before wave resolution.
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(&repo)
            .output()
            .expect("git")
    };
    git(&["init", "-b", "main"]);
    git(&["config", "user.email", "test@loopflow.test"]);
    git(&["config", "user.name", "Test"]);
    std::fs::write(repo.join(".gitkeep"), "").expect("gitkeep");
    git(&["add", "."]);
    git(&["commit", "-m", "init"]);

    let store = open_test_store(&home.join("loopflow.db"));
    let alpha = Wave::new(
        WaveId::new(),
        "alpha".to_string(),
        repo.display().to_string(),
    );
    store.create_wave(&alpha).expect("register alpha");

    let alpha_uuid = alpha.id().as_str();

    let cron = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args([
            "cron",
            "add",
            "--flow",
            "mutation-test",
            "--schedule",
            "daily",
        ])
        .current_dir(&repo)
        .env("LF_HOME", &home)
        .env("HOME", &home)
        .env("LF_WAVE_ID", alpha_uuid)
        .output()
        .expect("run cron add");

    assert!(
        !cron.status.success(),
        "development cron installation should fail"
    );
    let stderr = String::from_utf8_lossy(&cron.stderr);
    assert!(
        stderr.contains("requires an installed release binary"),
        "unexpected cron add error: {stderr}"
    );
    assert!(
        !home.join("Library/LaunchAgents").exists(),
        "development cron installation must not create launchd state"
    );
}
