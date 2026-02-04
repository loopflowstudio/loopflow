# Prompt Pass: File-Based Prompt Delivery

## Problem

Claude Code gets sluggish with very large prompts passed as command-line arguments. The input history fills with giant pastes, degrading UX.

## Solution

Write assembled prompts to disk, pass a short bootstrap prompt that instructs the agent to read the file.

## Current State

**Python**: Already implemented.
- `src/loopflow/lf/logging.py`: `write_prompt_log()`, `build_bootstrap_prompt()`
- `src/loopflow/lf/execution.py`: Uses these in `_execute_interactive()` and `_execute_auto()`

**Rust**: Not yet implemented.
- `rust/loopflow-engine/src/prompt.rs`: Has `write_prompt_log()` ✓
- `rust/lf/src/commands/step.rs`: Passes full prompt directly to `launch_agent()` ✗

## What to Build

Wire Rust `lf` to use file-based prompt delivery, matching Python behavior.

## Data Structures

Already exist in `prompt.rs`:

```rust
pub fn write_prompt_log(
    repo_root: &Path,
    prompt: &str,
    step_name: &str,
    flow_parents: Option<&[String]>,
) -> Result<PathBuf, CoreError>
```

Need to add to `loopflow-engine`:

```rust
/// Build the short bootstrap prompt that tells the agent to read the file.
pub fn build_bootstrap_prompt(prompt_path: &Path) -> String {
    format!("Read {} for your initial context and then engage.", prompt_path.display())
}
```

## Key Changes

### 1. loopflow-engine/src/prompt.rs

Add `build_bootstrap_prompt()`:

```rust
pub fn build_bootstrap_prompt(prompt_path: &Path) -> String {
    format!("Read {} for your initial context and then engage.", prompt_path.display())
}
```

Export from `lib.rs`.

### 2. lf/src/commands/step.rs

Change `run()` to write prompt log and use bootstrap:

```rust
// After line 58: let prompt = format_prompt(&components);
// Add:
let prompt_path = write_prompt_log(&repo_root, &prompt, step_name, None)?;
let bootstrap = build_bootstrap_prompt(&prompt_path);

// Change line 85 from:
let result = launch_agent(model, &prompt, &launch_config)?;
// To:
let result = launch_agent(model, &bootstrap, &launch_config)?;
```

Same pattern for `run_interactive()`:

```rust
// After line 119: let prompt = format_prompt(&components);
let prompt_path = write_prompt_log(&repo_root, &prompt, "interactive", None)?;
let bootstrap = build_bootstrap_prompt(&prompt_path);

// Change line 137 to use bootstrap
let result = launch_agent(model, &bootstrap, &launch_config)?;
```

### 3. .lf/.gitignore

Ensure `.lf/log/` is gitignored. Add to `lf init` or create during first `write_prompt_log()`:

```rust
// In write_prompt_log(), after create_dir_all:
let gitignore_path = repo_root.join(".lf/.gitignore");
if !gitignore_path.exists() {
    fs::write(&gitignore_path, "log/\n")?;
}
```

## Constraints

- Bootstrap prompt must be short (one line) to keep input history clean
- Prompt files must be readable `.md` for debugging
- Must not break existing flows that don't specify flow_parents
- Python and Rust must produce identical file formats

## Done When

- [ ] `cargo test -p loopflow-engine` passes with new `build_bootstrap_prompt` tests
- [ ] `lf implement "message"` writes to `.lf/log/` and uses bootstrap prompt
- [ ] `.lf/log/` is gitignored via `.lf/.gitignore`
- [ ] Claude Code input shows short bootstrap, not full prompt
- [ ] Prompt files are human-readable for debugging
