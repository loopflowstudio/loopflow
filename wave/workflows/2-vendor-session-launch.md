# Vendor-session launch

**Finish line:** `lf <step>` launches a new interactive session in the vendor's
own surface with the prompt loaded and the worktree selected. Terminal-first: a
plain `lf` invocation opens the vendor CLI/TUI; Concerto can call the same
launcher or explicitly open the vendor app.

## Context

Loopflow is the layer above (see `release/unreleased/DECISIONS.md`, 2026-06-19).
It runs headless agent work and hands interaction to the vendor. This replaces the
retired `chat-session-api` task — we are not building a native-chat backend; we
launch the vendor's session and get out of the way.

Two launch targets, one config knob:

- **cli** — open the vendor CLI/TUI in a terminal: bare terminal, tmux, or
  Concerto's embedded pane
- **ide** — open the vendor's standalone GUI app by URL scheme

OpenCode is **cli-only**. Cursor is excluded because it has no stable GUI
folder+prompt launch. The web-URL map in `lf/commands/util.rs`
(`open_web_client`) is the wrong target and gets replaced by a session launcher
that uses CLI commands or `engine::platform::open_url`.

```yaml
# .lf/config.yaml
agent: claude
session:
  launch: cli    # cli | ide
```

## Gating spike — answered

The launch mechanism each vendor exposes decided the config and the ceiling. Full
findings: `scratch/vendor-session-launch.md`. Summary:

1. *New session?* Yes — vendor CLI (`claude "<p>"`, `codex -C <wt> "<p>"`,
   `opencode <wt> --prompt`) opens a fresh interactive TUI.
2. *Worktree?* Yes — `cd`/`current_dir` for Claude, `codex -C`, opencode
   positional worktree.
3. *Seed prompt?* Interactive Codex and Claude launches **pre-fill but do not
   auto-run**. OpenCode `--prompt` auto-submits on recent builds.
4. *App mechanism?* Codex uses `codex://threads/new?path=&prompt=`. Claude uses
   `claude://code/new?folder=&q=` — not `claude-cli://`, which opens a terminal
   CLI. Both app paths pre-fill the prompt.

## Done when

- `lf <step>` with `session.launch: cli` opens a fresh vendor CLI session in the
  step worktree with the prompt loaded
- `session.launch: ide` opens the standalone Codex or Claude app at the worktree
  with the prompt loaded
- Missing GUI handlers and OpenCode fall back to `cli` with a one-line notice
- Choosing the target is explicit and cheap, not a heuristic
