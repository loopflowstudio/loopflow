# Vendor-session launch

Kickoff design for `wave/workflows/2-vendor-session-launch.md`. Turns the
session-handoff thesis (`scratch/session-handoff.md`, decision 2026-06-19) into a
buildable milestone, with the gating spike answered.

## Problem

Loopflow is the layer above: it orchestrates headless work and hands interaction
to the vendor. The missing capability is the handoff itself — `lf <step>` should
open a fresh interactive session **in the vendor's own surface**, in the right
worktree, ideally seeded with the step's prompt. Today `lf --web` only copies the
prompt to the clipboard and opens a marketing web page (`claude.ai/new`,
`chatgpt.com`); that is the wrong target and seeds nothing.

Who benefits: the conductor who runs `lf design` and lands directly in a Claude
Code / Codex session on the worktree, instead of copy-pasting context by hand.

## Approach

One config knob, `session.launch`, dispatched to a per-target launcher. The launch
*mechanism* differs by vendor, and the spike (below) settled which mechanism each
target uses and what it can carry.

```yaml
# .lf/config.yaml
agent: claude          # harness:model, already parsed (config.rs:386 parse_agent)
session:
  launch: tui          # tui | embedded | app | ide
```

| Target | Mechanism | Worktree | Seed prompt | Notes |
|---|---|---|---|---|
| **tui** | vendor CLI in place (`cd <wt> && claude "<prompt>"`) | cd | **auto-sent** | default for bare terminal |
| **embedded** | same CLI, inside a tmux pane | cd | **auto-sent** | the kept embedded terminal; Concerto default |
| **app** | vendor URL scheme (`claude-cli://`, `codex://`) | scheme param | **pre-filled, not sent** | new vendor-owned window; version-gated |
| **ide** | GUI CLI (`code -n`, `cursor -n`, `idea`) | path arg | not directly | opens worktree; session-start is best-effort |

The decisive finding: **CLI launch auto-submits the seed prompt; URL-scheme launch
only pre-fills it.** So `tui`/`embedded` deliver the full "land in a running
session" experience; `app`/`ide` deliver "land in the right place, prompt ready,
press Enter." That asymmetry is the vendor's safety choice, not ours — accept and
document it rather than fight it.

`open_web_client` (`lf/commands/util.rs:23`) gets repurposed into a `launch_session`
that dispatches on target, reusing `engine::platform::open_url` for the scheme/app
paths and the existing harness-name knowledge for the CLI paths.

## De-risking

The wave item's gating spike, answered. Mechanisms verified against current vendor
docs (June 2026); code claims verified against the working tree.

| Question | Finding | Impact on design |
|---|---|---|
| Launch a *new* Claude Code session w/ worktree + prompt? | **Yes.** `cd <wt> && claude --name "lf-<step>" --model <m> "<prompt>"` opens an interactive TUI and **auto-sends** the prompt. There is **no `--cwd` flag** ([#26287](https://github.com/anthropics/claude-code/issues/26287)) — must `cd`. Avoid `-w/--worktree`: it *creates* a new worktree under `.claude/worktrees/`, it does not target ours. | `tui`/`embedded` = `cd` + CLI. Loopflow already owns the worktree, so just `cd` into it. |
| Claude **app** launch with context? | `claude-cli://open?cwd=<abs>&q=<prompt>` (v2.1.91+) opens a new terminal window in the user's last-used emulator; prompt **pre-filled, not sent**. `open -a "Claude" /path` **fails** — Electron single-instance drops args ([#54614](https://github.com/anthropics/claude-code/issues/54614)). | `app` target = the `claude-cli://` scheme via `open_url`, **not** `open -a`. Accept pre-fill-without-send. Gate on version; fall back to `tui`. |
| Codex new session w/ worktree + prompt? | **Yes.** `codex -C <wt> -m <model> -s workspace-write -a on-request "<prompt>"` opens the TUI and **auto-runs** the prompt (`-C/--cd` sets the dir). Desktop app scheme `codex://new?path=<wt>&prompt=<p>` exists but **pre-fills only** and is semi-official (community-documented). | Codex `tui` mirrors Claude. Codex `app` = `codex://` scheme, same pre-fill caveat, treat as best-effort. |
| opencode new session w/ worktree + prompt? | `opencode <wt> --prompt "<seed>" -m <provider/model> --agent <name>` launches the TUI in the dir. **Auto-submit unconfirmed** — historically pre-fill only ([#3937](https://github.com/sst/opencode/issues/3937), PR #4510 unverified). No URL scheme, no app. | Confirms **opencode = tui-only**, as planned. Verify auto-submit live; if it pre-fills, that is acceptable. |
| IDE bounce — open worktree + start a session? | `code -n <wt>` / `cursor -n <wt>` (code-fork flags) / `idea <wt>` open the worktree in a new window. **No launcher both opens an arbitrary path and starts an AI session in one call.** VS Code/Cursor extensions self-activate via `workspaceContains:<glob>`; JetBrains has no equivalent and `jetbrains://` can't open a path. | `ide` target opens the folder reliably; session-start is best-effort. Optionally write a `.claude/`-style marker into the worktree to trip `workspaceContains`. Don't promise auto-session for `ide`. |
| What existing code is the seed? | `open_web_client` (`util.rs:23`) maps harness → **web URLs** (wrong target). `engine::platform::open_url` is the open primitive. **Correction to the thesis doc:** `which("cursor")` in `ops/mod.rs:1541` is **doctor-only**; there is **no IDE launcher and no `lf ide` command**. `IdeConfig` (`config.rs:130`) is parsed but **never consumed**. Harness→CLI is hardcoded `Command::new("claude"/"codex"/"opencode")` in `lfd/sessions/harness/<name>.rs`. | Build `app`/`ide`/`tui` launch atop `open_url` + the harness-name map. Repurpose `open_web_client`; wake the dormant `IdeConfig` or replace it with `session.launch`. The IDE path is greenfield, not a refactor. |

Residual unknowns (small, live-testable in minutes, not a blocking spike):
- Confirm `claude "<prompt>"` auto-sends vs pre-fills (docs imply auto-send; only the deep-link doc states pre-fill explicitly).
- Confirm opencode `--prompt` auto-submits after PR #4510.

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| URL scheme for everything (incl. tui) | One mechanism, clickable links | Pre-fill-only (no auto-send), version-gated (`claude-cli://` needs v2.1.91+), `codex://` semi-official, opencode has none. Loses the best experience. |
| CLI for everything (incl. app) | Auto-sends prompt, exact cwd | Can't produce a *new vendor-owned window/app* — CLI runs in the calling terminal. `app` exists precisely to escape the terminal. |
| Auto-detect target from environment | Zero config | The thesis is explicit: "the choice is explicit and cheap, not a heuristic." A surprised user in the wrong surface is worse than a config line. |
| Per-vendor launch config blocks | Maximum flexibility | Over-fit. One `session.launch` enum + per-target dispatch covers the real cases; vendors that lack a target fall back to `tui`. |

## Key decisions

- **CLI is the primitive; URL scheme is the "app" escape hatch.** Auto-submit only
  on the CLI paths. This is the load-bearing finding and it shapes the whole UX.
- **`app` uses the vendor's URL scheme, never `open -a`.** `open -a` cannot carry
  cwd or prompt for Electron apps; the scheme can.
- **`ide` opens the folder, not the session.** Honest scope: we land the user in
  the right worktree; the vendor's extension owns whether a session starts.
- **opencode is tui-only** — confirmed by absence of any app/scheme, not a
  limitation we're imposing arbitrarily.
- **Per-surface defaults:** bare terminal → `tui`; Concerto → `embedded`, with
  explicit "open in app / open in IDE" actions. Same launcher, different default.
- **Graceful fallback:** if the chosen target's mechanism is unavailable (old
  Claude without `claude-cli://`, no `cursor` on PATH), fall back to `tui` with a
  one-line notice rather than failing.

## Scope

- **In scope:** `session.launch` config + parse; a `launch_session(target, harness,
  worktree, prompt)` dispatcher replacing `open_web_client`; the four targets for
  Claude + Codex; opencode `tui`; `lf <step>`-first wiring; fallback-to-tui.
- **Out of scope:** the Concerto "open in app / IDE" buttons (separate desktop
  task — `wave/desktop` consumes this launcher); the teardown of
  `lfd/sessions/harness` and native chat (separate teardown branch); resuming
  existing sessions (`--resume`/`--continue`) — this milestone is *new* sessions;
  the IDE marker-file activation trick (best-effort follow-up).

## Done when

- `lf <step>` with `session.launch: tui` opens a fresh vendor session in the
  step's worktree with the prompt auto-sent — Claude and Codex.
- `session.launch: app` opens a new vendor window via the scheme with the prompt
  pre-filled in the right cwd; missing scheme falls back to `tui`.
- `session.launch: ide` opens the worktree in `code`/`cursor`/`idea`; missing
  binary falls back to `tui`.
- opencode launches its TUI in the worktree.
- Choosing the target is one config line; no heuristic.

Verify with a script under `scripts/` that, per target, asserts the right process
launched against the right cwd (mechanism-level — full interactive send is
manual).
