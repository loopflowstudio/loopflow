---
status: todo
phase: 1
---
# OpenCode Command Builder and Launch

Add `build_opencode_command()`, wire into `build_model_command()`, and handle runtime differences in `launch_agent`.

## Current

`build_model_command()` dispatches to `build_claude_command`, `build_codex_command`, or `build_gemini_command`. Unknown backends fall through to claude. Each builder translates `LaunchConfig` fields into agent-specific CLI args.

## Build

### Command builder (`agent.rs`)

```rust
pub fn build_opencode_command(config: &LaunchConfig) -> Vec<String> {
    let mut cmd = if config.auto {
        vec!["opencode".to_string(), "run".to_string()]
    } else {
        // Interactive: launch TUI directly
        vec!["opencode".to_string()]
    };

    if let Some(ref variant) = config.model_variant {
        cmd.push("--model".to_string());
        cmd.push(variant.clone());
    }

    if config.auto && config.stream {
        cmd.push("--format".to_string());
        cmd.push("json".to_string());
    }

    cmd
}
```

### Dispatch (`agent.rs`)

```rust
match model {
    "claude" => build_claude_command(config),
    "codex" => build_codex_command(config),
    "gemini" => build_gemini_command(config),
    "opencode" => build_opencode_command(config),
    _ => build_claude_command(config),
}
```

### Launch integration (`agent.rs`)

In `launch_agent`, opencode needs permission auto-approve in auto mode. OpenCode uses `OPENCODE_CONFIG_CONTENT` env var for runtime config overrides:

```rust
if backend == "opencode" && config.auto {
    cmd.env(
        "OPENCODE_CONFIG_CONTENT",
        r#"{"permission":"allow"}"#,
    );
}
```

This is equivalent to Claude's `--dangerously-skip-permissions` and Gemini's `--yolo`. Without it, `opencode run` blocks on permission prompts.

No `context_file` env var needed here — that's PR 03.

### Re-exports (`mod.rs`)

Add `build_opencode_command` to the re-export list.

### Unit tests

```rust
#[test]
fn build_opencode_command_auto() {
    let config = LaunchConfig { auto: true, ..Default::default() };
    let cmd = build_opencode_command(&config);
    assert_eq!(cmd, vec!["opencode", "run"]);
}

#[test]
fn build_opencode_command_with_model() {
    let config = LaunchConfig {
        auto: true,
        model_variant: Some("anthropic/claude-sonnet-4-5".into()),
        ..Default::default()
    };
    let cmd = build_opencode_command(&config);
    assert_eq!(cmd, vec!["opencode", "run", "--model", "anthropic/claude-sonnet-4-5"]);
}

#[test]
fn build_opencode_command_interactive() {
    let config = LaunchConfig::default();
    let cmd = build_opencode_command(&config);
    assert_eq!(cmd, vec!["opencode"]);
}

#[test]
fn build_opencode_command_stream() {
    let config = LaunchConfig { auto: true, stream: true, ..Default::default() };
    let cmd = build_opencode_command(&config);
    assert!(cmd.contains(&"--format".to_string()));
    assert!(cmd.contains(&"json".to_string()));
}
```

## Constraints

- `opencode run` is the non-interactive entry point (not `-p` — that's the other opencode project)
- Model variant passes through verbatim — opencode handles `provider/model` parsing
- No default model variant (like codex) — opencode uses its own config
- `context_file` and `chrome` are not applicable to opencode (skip them)
- `skip_permissions` maps to `OPENCODE_CONFIG_CONTENT` env var, not a CLI flag
- `cwd` works via `Command::current_dir` (standard)

## Done when

```bash
cargo test -p loopflow -- opencode
cargo fmt --check
cargo clippy -- -D warnings
```
