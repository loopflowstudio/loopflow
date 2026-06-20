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

`lf <step>` launches a new interactive session **in the vendor's own surface**,
automatically, when configured. Terminal-first — a plain `lf` call does it;
Concerto calls the same path. Two launch targets, the only real distinction being
terminal vs the vendor's standalone app:

- **cli** — vendor CLI/TUI in a terminal (`cd <wt> && codex/claude/opencode
  "<prompt>"`). Renders wherever a terminal does: bare term, tmux, or Concerto's
  embedded pane.
- **ide** — the vendor's **standalone GUI app**, via its URL scheme: Codex
  (`codex://threads/new?path=&prompt=`) and Claude Code (`claude://code/new?folder=&q=`,
  the desktop app's "Code" tab — **not** `claude-cli://`, which opens the terminal CLI).

opencode is **cli-only** (no app). Cursor is **not** an `ide` target — it has no
clean way to open its GUI at a worktree with a seeded prompt.

## Config shape (sketch)

Reuse the existing `agent: harness:model` knob; add a launch target.

```yaml
# .lf/config.yaml
agent: codex             # harness
session:
  launch: cli            # cli | ide
```

Per-surface defaults: bare terminal → `cli` in place; Concerto → `cli` in the
embedded pane, with an explicit "open in app" action that fires `ide`. The choice
is explicit and cheap, not a heuristic.

The web-URL map in `lf/commands/util.rs` (`open_web_client`: claude.ai,
chatgpt.com) is the wrong target and gets repurposed into a `launch_session`
dispatcher atop `engine::platform::open_url`. (Per the spike: `IdeConfig` in
`config.rs` is parsed-but-unused and `which("cursor")` in `ops/mod.rs` is
doctor-only — there is no launcher today, so this is greenfield.)

## Open question — answered

**What launch mechanism does each vendor expose, and does it auto-send the
prompt?** Answered. Full findings and the resulting milestone design:
`scratch/vendor-session-launch.md`. The short version:

- **Vendor CLI** (`cd <wt> && codex/claude "<p>"`) opens the TUI with the prompt
  **pre-filled, not sent** — auto-run is headless-only (`codex exec`, `claude -p`).
  Claude has no `--cwd`; must `cd`. The lone interactive auto-send is opencode
  `--prompt` (recent builds).
- **App launch** is the vendor's **GUI URL scheme** (Codex `codex://threads/new`,
  Claude `claude://code/new` — not `claude-cli://`, which is the terminal CLI),
  which **pre-fills but does not send**, and is **not** `open -a` (Electron drops
  args). Claude also gates each deep-linked folder behind a confirmation.
- **opencode** has no app, no scheme → `cli` only. **Cursor** has no GUI
  folder+prompt launch → excluded from `ide`.

The load-bearing correction: **interactive launches pre-fill — none auto-run** (bar
opencode). The earlier "CLI auto-sends" claim was wrong, which collapses the old
four-target split into two: `cli` (terminal) vs `ide` (standalone app). Both land
you in the right place, prompt ready, one keypress to go — the honest shape for a
take-over-and-review handoff.

## Staging

The original split was docs → teardown → build. This branch ended up taking the
first executable slice too: remove mobile pairing and add the `lf`-first vendor
session launcher. The remaining teardown is still separate.

1. **This PR** — record the decision, archive/reframe the wave plans, remove
   mobile pairing (`lf op pair`, QR/link parsing, pairing payloads), and ship
   `session.launch` for CLI/app handoff from `lf`.
2. **Teardown** — own branch for the larger hosting strata: native chat rendering
   and `lfd/sessions/harness`.
3. **Desktop consume** — Concerto adds explicit "open in app" UI on top of the
   launcher while keeping the embedded terminal as the default frame.

## Wave moves (this PR)

- `wave/mobile/` → **archived**. Mobile is the vendors' apps now.
- `wave/desktop/` → drop `native-chat-ux`; reframe vision to "layer above" —
  embedded terminal frames the vendor TUI; no native chat client.
- `wave/workflows/` → retire `chat-session-api` (we are not hosting our own
  session API); add `vendor-session-launch` (the new capability).
