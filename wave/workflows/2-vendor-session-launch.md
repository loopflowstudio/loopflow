# Vendor-session launch

**Finish line:** `lf <step>` launches a new interactive session in the vendor's
own surface — the Codex / Claude Code app, an embedded TUI, or the vendor's IDE
integration — automatically, driven by config. Terminal-first: a plain `lf`
invocation does it; Concerto calls the same path.

## Context

Loopflow is the layer above (see `release/unreleased/DECISIONS.md`, 2026-06-19).
It runs headless agent work and hands interaction to the vendor. This replaces the
retired `chat-session-api` task — we are not building a native-chat backend; we
launch the vendor's session and get out of the way.

Three launch targets, one config knob:

- **app** — open a new session in the Codex / Claude Code app
- **embedded** — vendor TUI in a tmux pane (the kept embedded terminal)
- **ide** — open the worktree in VS Code / Cursor / JetBrains with the vendor's
  extension (not websites)

opencode is **tui-only** for now. The web-URL map in `lf/commands/util.rs`
(`open_web_client`) is the wrong target and gets repurposed atop
`engine::platform::open_url`. Note: there is no IDE launcher today — the
`which("cursor")` check in `ops/mod.rs` is doctor-only and `IdeConfig` is parsed
but unused, so the `ide` target is greenfield.

```yaml
# .lf/config.yaml
agent: claude
session:
  launch: app    # app | embedded | ide | tui
```

## Gating spike — answered

The launch mechanism each vendor exposes decided the config and the ceiling. Full
findings: `scratch/vendor-session-launch.md`. Summary:

1. *New session?* Yes — vendor CLI (`claude "<p>"`, `codex -C <wt> "<p>"`,
   `opencode <wt> --prompt`) opens a fresh interactive TUI.
2. *Worktree?* Yes — `cd` (Claude has no `--cwd`), `codex -C`, opencode positional.
3. *Seed prompt?* **CLI auto-sends it; URL schemes only pre-fill.** This split is
   the load-bearing decision.
4. *Mechanism?* CLI for `tui`/`embedded`; vendor URL scheme (`claude-cli://`,
   `codex://`) for `app` — **not** `open -a` (Electron drops args); GUI CLI
   (`code -n`/`cursor -n`/`idea`) for `ide`. opencode: CLI/TUI only.

## Done when

- `lf <step>` with `session.launch: app` opens a new vendor session in the right
  worktree, no manual steps
- Per-surface defaults work: bare terminal → vendor TUI in place; Concerto →
  embedded, with explicit "open in app / open in IDE" actions
- All four launch targets reachable from config
- opencode lands in its TUI
- Choosing the target is explicit and cheap, not a heuristic
