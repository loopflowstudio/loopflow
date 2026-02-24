# Review: codex --yolo + chrome default off

## What was implemented

Two config simplifications:

1. **Codex `--yolo` flag**: Replaced the verbose 4-flag combination (`--sandbox danger-full-access -c approval_policy="never"`) with the single `--yolo` flag when `skip_permissions` is set. This matches how Gemini already handles it.

2. **Chrome off by default**: Changed `.lf/config.yaml` from `chrome: true` to `chrome: false`.

## Key choices

- **`--yolo` over `--sandbox danger-full-access`**: Codex CLI now supports `--yolo` directly. Using it is cleaner and consistent with Gemini's skip_permissions path. No behavioral change.
- **Chrome off**: Chrome integration is opt-in rather than default-on, reducing agent startup overhead for the common case.

## How it fits together

`build_codex_command()` in `agent.rs` constructs the CLI invocation for Codex. The `skip_permissions` branch was the only code path that changed. The non-skip path (`--sandbox workspace-write` + `--full-auto`) is untouched.

## Risks and bottlenecks

Minimal. The `--yolo` flag must exist in the Codex CLI version being used. If someone pins an older Codex version, it would fail at invocation time with a clear CLI error.

## What's not included

No migration or deprecation handling for older Codex versions. This is intentional per CLAUDE.md: "Don't maintain backwards compatibility unless explicitly required."

## Gate additions

Added `build_codex_command_yolo` test to match the existing `build_gemini_command_yolo` test, covering the changed code path.
