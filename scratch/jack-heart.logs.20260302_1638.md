# Move lf logs out of the repo

## What to build

Rename `.lf/log/` to `.lf/prompts/` (prompt handoff, stays in-repo) and move `.lf/logs/` (diagnostic output) to `~/.lf/logs/<repo>/<worktree>/`. Write prompts to both locations so they survive worktree deletion.

## Current state

Three in-repo log paths, two naming conventions, one leaking into commits:

| Path | Writer | Purpose | Auto-gitignore? |
|------|--------|---------|-----------------|
| `.lf/log/` | `write_prompt_log` (engine/prompt.rs:1825) | Prompt handoff — agent reads system prompt from here | Yes (ensure_gitignore_entry) |
| `.lf/logs/` | `write_message_output_log` (ops/messages.rs:357) | Ops diagnostic output (commit msg agent stdout/stderr) | **No** — this is what leaks |

Plus `~/.lf/output/` for daemon wave run logs (already out of repo, not changing).

## New state

| Path | Purpose | Writer |
|------|---------|--------|
| `.lf/prompts/` (in worktree) | Agent reads system prompt at runtime | `write_prompt_log` |
| `~/.lf/logs/<repo>/<worktree>/` | Durable logs — prompts + diagnostic output | `write_prompt_log` + `write_message_output_log` |

### Identifiers

- `<repo>`: directory name of the main repo root (e.g. `loopflow`, `loopflow.logs`)
- `<worktree>`: wave/worktree name, or `main` for the default worktree

Collisions from two unrelated repos with the same directory name: not worth solving now.

## Key functions

```rust
/// Resolve the durable log directory for a repo + worktree.
/// Creates the directory if it doesn't exist.
fn lf_log_dir(repo_root: &Path) -> PathBuf {
    let repo_name = repo_root.file_name().unwrap().to_str().unwrap();
    // Derive worktree name: if this is a worktree, extract the suffix;
    // otherwise "main"
    let worktree_name = worktree_name_or_main(repo_root);
    dirs::home_dir().unwrap()
        .join(".lf/logs")
        .join(repo_name)
        .join(worktree_name)
}
```

```rust
/// Write prompt to both locations:
/// 1. .lf/prompts/<file> — agent reads this at runtime
/// 2. ~/.lf/logs/<repo>/<worktree>/<file> — survives worktree deletion
pub fn write_prompt_log(
    repo_root: &Path,
    prompt: &str,
    step_name: &str,
    flow_parents: Option<&[String]>,
) -> Result<PathBuf, CoreError> {
    // In-repo: .lf/prompts/
    let prompts_dir = repo_root.join(".lf/prompts");
    fs::create_dir_all(&prompts_dir)?;
    ensure_gitignore_entry(repo_root, ".lf/prompts/")?;

    let filename = format_log_filename(step_name, flow_parents);
    let local_path = prompts_dir.join(&filename);
    fs::write(&local_path, prompt)?;

    // Durable: ~/.lf/logs/<repo>/<worktree>/
    let durable_dir = lf_log_dir(repo_root);
    fs::create_dir_all(&durable_dir).ok(); // best-effort
    fs::write(durable_dir.join(&filename), prompt).ok();

    Ok(local_path) // agent uses the local path
}
```

```rust
/// Write diagnostic output to ~/.lf/logs/<repo>/<worktree>/ only.
/// No longer writes to .lf/logs/ in the repo.
fn write_message_output_log(
    repo: &Path,
    label: &str,
    stdout: &str,
    stderr: &str,
) -> Option<PathBuf> {
    let log_dir = lf_log_dir(repo);
    std::fs::create_dir_all(&log_dir).ok()?;
    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S-%f");
    let filename = format!("{timestamp}-{}-{label}.log", std::process::id());
    let path = log_dir.join(filename);
    // ... write contents ...
    Some(path)
}
```

### worktree_name_or_main

Derive from existing `wave_name_from_worktree_and_main` logic in `engine/worktrees.rs`:
- If the current dir is a git worktree, extract the suffix after `<repo>.`
- Otherwise return `"main"`

## Constraints

- `.lf/prompts/` must be inside the worktree. Sandboxed agents may not have access to `~/.lf/`.
- The durable copy at `~/.lf/logs/` is best-effort. If it fails (permissions, disk), don't block the run.
- `lfd` calls the same `write_prompt_log` — both paths work for daemon and CLI.
- Error messages that reference `.lf/logs/` (run.rs:288) need to point to the new `~/.lf/logs/` path.

## Changes

1. **`engine/prompt.rs`**: rename `write_prompt_log` to write to `.lf/prompts/` + `~/.lf/logs/<repo>/<worktree>/`. Update gitignore entry from `.lf/log/` to `.lf/prompts/`.
2. **`ops/messages.rs`**: change `write_message_output_log` to write to `~/.lf/logs/<repo>/<worktree>/` instead of `.lf/logs/`.
3. **`lf/commands/run.rs`**: update error message at line 288 to reference new path.
4. **New function**: `lf_log_dir(repo_root)` in engine (or shared util) for `~/.lf/logs/<repo>/<worktree>/` resolution.
5. **New function**: `worktree_name_or_main(repo_root)` — may already exist as `wave_name_from_worktree_and_main`, just needs a fallback to `"main"`.
6. **Tests**: update `write_prompt_log` tests to check both paths. Update `write_message_output_log` test expectations.
7. **Gitignore**: `.lf/log/` entry becomes `.lf/prompts/`. `.lf/logs/` entry can be removed from repos that have it (no longer needed).
8. **Docs/wave references**: `wave/sandboxes/01-integration-and-validation.md` references `.lf/logs/<step>.context.md` — update.
9. **Sandbox test script**: `scripts/test_sandbox_platforms.sh` creates `.lf/logs/` — update to `.lf/prompts/`.

## Done when

```bash
# Prompts written to both locations
lf research
ls .lf/prompts/          # prompt + context files
ls ~/.lf/logs/loopflow/main/  # same files, plus any diagnostic output

# No .lf/log/ or .lf/logs/ in repo
test ! -d .lf/log
test ! -d .lf/logs

# Gitignore updated
grep '.lf/prompts/' .gitignore

# Tests pass
cargo test --all
```
