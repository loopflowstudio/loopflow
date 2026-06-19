# Session handoff: loopflow as the layer above

Loopflow stops hosting interactive sessions. It hands off to the vendors. This
doc records the decision's shape and the build it queues.

## Why

We built mobile pairing, a native SwiftUI chat UI, and an lfd layer that parses
vendor session streams into our own event model. We then reversed it in place on
`main`, twice, with no record. Reimplementing the vendors' own session surfaces
doesn't compound — they ship better clients, IDE integrations, and mobile apps
than we will. So: orchestrate headless work; hand interaction to the vendor.

See `release/unreleased/DECISIONS.md` (2026-06-19) for the standing decision.

## Two pieces, shipped separately

This is **not** one change. Keep them apart.

### 1. Teardown (cleanup)

Remove what we built to host sessions. Almost entirely deletion. The half-done
in-place version was stashed off `main` (`stash@{0}`, 2026-06-19) and dropped —
redo it deliberately here.

| Remove | Keep |
|---|---|
| Native SwiftUI chat UI (`SelectableAssistantTextView`, native-chat rendering) | Embedded terminal (tmux pane) |
| `lfd/sessions/harness` (~7,800 LOC stream parsing → our event model) | Wave monitoring surfaces |
| Mobile: iOS target, `lf op pair`, pairing tokens, remote-lfd-for-phone | Flow engine, journals, catalog, attention, PM, usage |

Confirmed: `terminal_sessions.rs` / `types::terminal_session` have **no**
dependency on `lfd::sessions::`, so the embedded terminal survives the harness
deletion cleanly.

### 2. Vendor-session launch (new functionality)

`lf <step>` launches a new interactive session **in the vendor's own app**,
automatically, when configured. Terminal-first — a plain `lf` call does it;
Concerto calls the same path. Three launch targets:

- **app** — open a new session in the Codex / Claude Code app
- **embedded** — vendor TUI in a tmux pane (the kept embedded terminal)
- **ide** — open the worktree in VS Code / Cursor / JetBrains with the vendor's
  extension (not websites)

opencode is **tui-only** for now.

## Config shape (sketch)

Reuse the existing `agent: harness:model` knob; add a launch target.

```yaml
# .lf/config.yaml
agent: claude            # harness
session:
  launch: app            # app | embedded | ide | tui
```

Per-surface defaults: bare terminal → vendor TUI in place; Concerto → embedded,
with an explicit "open in app / open in IDE" action. The choice is explicit and
cheap, not a heuristic.

The web-URL map in `lf/commands/util.rs` (`open_web_client`: claude.ai,
chatgpt.com) is the wrong target and gets repurposed to app/IDE launch, atop
`engine::platform::open_url`. (Correction after the spike: the `which("cursor")`
check in `ops/mod.rs` is doctor-only and `IdeConfig` is parsed-but-unused — there
is no `lf ide` launcher today, so the IDE path is greenfield.)

## Open question — answered

**What launch mechanism does each vendor expose?** Answered. Full findings and the
resulting milestone design: `scratch/vendor-session-launch.md`. The short version:

- **Vendor CLI** is the primitive and the only path that **auto-sends** the seed
  prompt: `cd <wt> && claude "<p>"`, `codex -C <wt> "<p>"`, `opencode <wt>
  --prompt`. Claude has no `--cwd` — must `cd`.
- **App launch** is the vendor's **URL scheme** (`claude-cli://`, `codex://`),
  which **pre-fills but does not send**, and is **not** `open -a` (Electron drops
  args). Version-gated; fall back to CLI.
- **IDE launch** is the GUI CLI (`code -n`/`cursor -n`/`idea`): opens the worktree,
  but no launcher also *starts* a session — extension self-activation is
  best-effort.
- **opencode is genuinely tui-only** — no app, no scheme.

The asymmetry (CLI auto-sends, scheme pre-fills) is the load-bearing finding: it
splits the config into "land in a running session" (`tui`/`embedded`) vs "land in
the right place, prompt ready" (`app`/`ide`).

## Staging

1. **This PR** — record decision, this design doc, archive mobile wave, reframe
   desktop + workflows wave plans, **answer the launch-mechanism spike** (folded
   into `scratch/vendor-session-launch.md`). No code.
2. **Teardown** — own branch off the redone plan. Get the hosting strata out.
3. **Build** — vendor-session launch, `lf`-first, then Concerto action. Design
   ready in `scratch/vendor-session-launch.md`; residual unknowns are two
   minute-long live checks, not a blocking spike.

## Wave moves (this PR)

- `wave/mobile/` → **archived**. Mobile is the vendors' apps now.
- `wave/desktop/` → drop `native-chat-ux`; reframe vision to "layer above" —
  embedded terminal frames the vendor TUI; no native chat client.
- `wave/workflows/` → retire `chat-session-api` (we are not hosting our own
  session API); add `vendor-session-launch` (the new capability).
