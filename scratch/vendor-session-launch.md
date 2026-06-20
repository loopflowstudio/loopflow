# Vendor-session launch

Kickoff design for `wave/workflows/2-vendor-session-launch.md`. Turns the
session-handoff thesis (`scratch/session-handoff.md`, decision 2026-06-19) into a
buildable milestone. The launch-mechanism spike is answered, and the design
reshaped after it: there are **two** launch surfaces, not four, and **neither
auto-runs the prompt** — both pre-fill it.

## Problem

Loopflow is the layer above: it orchestrates headless work and hands interaction
to the vendor. The missing capability is the handoff itself — `lf <step>` should
open a fresh interactive session **in the vendor's own surface**, in the right
worktree, with the step's prompt loaded. Today `lf --web` only copies the prompt
to the clipboard and opens a marketing web page (`claude.ai/new`, `chatgpt.com`);
that is the wrong target and seeds nothing.

Who benefits: the conductor who runs `lf design` and lands directly in a **Codex**
(or **Claude Code**) session on the worktree, instead of copy-pasting context by
hand. The handoff moment *is* the verification seam — where the human takes over
to read and steer. So landing them in their preferred surface, on the right
worktree, with the prompt ready, is the whole job. Auto-firing the prompt is not
the point and, as the spike found, isn't on offer anyway.

## Approach

One config knob, `session.launch`, with two values — the only real distinction is
**terminal vs the vendor's standalone app**.

```yaml
# .lf/config.yaml
agent: codex           # harness:model, already parsed (config.rs:386 parse_agent)
session:
  launch: cli          # cli | ide
```

| Target | Surface | Mechanism | Seed prompt |
|---|---|---|---|
| **cli** | vendor CLI/TUI in a terminal — bare, tmux, or Concerto's embedded pane | `cd <wt> && codex/claude/opencode "<prompt>"` | **pre-filled** (opencode auto-submits on recent builds) |
| **ide** | the vendor's **standalone GUI app** | vendor URL scheme | **pre-filled** |

`ide` mechanisms, per harness (live-checked 2026-06-19):

- **Codex** → `codex://threads/new?path=<wt>&prompt=<p>` — opens a **real GUI app**,
  confirmed working on macOS (`codex://new` is an alias).
- **Claude** → `claude://code/new?folder=<wt>&q=<p>` — opens the **Claude Code
  desktop GUI** (the "Code" tab), at the folder, prompt pre-filled. Bundle
  `com.anthropic.claudefordesktop` claims the `claude:` scheme.
  - **Do not use `claude-cli://`** — that's a *separate* scheme that opens the CLI
    in a terminal, not the GUI. The GUI scheme is `claude://code/new`.
- **opencode** → no standalone app, no scheme → falls back to `cli`.

**`ide` is a genuine GUI surface for both Codex and Claude.** opencode is
terminal-only — not a limitation we impose, just what exists. Two Claude wrinkles:
deep-linked folders require a **confirmation each launch** (security gate, even for
trusted folders), and the app is single-instance but runs **parallel sessions in
the sidebar** — a deep link adds a session, it does not clobber an open one.

The correction that reshaped this doc: **interactive launches pre-fill the prompt;
they do not auto-run it.** `codex "<p>"` and `claude "<p>"` open the TUI with the
prompt typed but not sent; the URL schemes do the same in the app. Auto-execution
only exists in *headless* mode (`codex exec`, `claude -p`), which isn't an
interactive session. The lone exception is opencode `--prompt` (auto-submits on
builds after ~Dec 15 2025). So the earlier "CLI auto-sends" claim was wrong, and
the `tui`/`embedded`/`app`/`ide` four-way split it justified is gone. Both
surfaces land you in the right place, prompt ready, one keypress to go — which is
exactly right for a take-over-and-review handoff.

`open_web_client` (`lf/commands/util.rs:23`) gets repurposed into a
`launch_session(target, harness, worktree, prompt)` that dispatches on target,
reusing `engine::platform::open_url` for the scheme paths and the existing
harness-name knowledge for the CLI paths.

## Why `embedded` isn't a target

Concerto's embedded terminal is a *rendering surface* for `cli`, not a separate
launch. The launch is identical — `cd <wt> && <cli> "<prompt>"` — Concerto just
hosts the resulting terminal in a tmux pane. So "embedded" is a Concerto display
choice layered on `cli`, not a third value of `session.launch`.

## Why Cursor isn't an `ide` target

There is no clean way to open the Cursor GUI Composer at a worktree with a seeded
prompt. The `cursor://anysphere.cursor-deeplink/prompt?text=` deeplink has **no
folder parameter** — it fires at whatever window is focused — and `cursor <wt>`
opens the folder but seeds nothing; combining them is an open, unshipped feature
request. Cursor's robust programmatic path is its headless `agent` CLI, which
stays in the terminal and never opens the GUI. Cursor isn't the surface this
milestone targets anyway. If Cursor ships a folder+prompt GUI launch, revisit.

## De-risking

The gating spike, answered. Mechanisms verified against current vendor docs and
GitHub issues (2026); code claims verified against the working tree.

| Question | Finding | Impact |
|---|---|---|
| Codex CLI session w/ worktree + prompt? | `codex -C <wt> -m <model> -s workspace-write -a on-request "<prompt>"` opens the TUI with the prompt **pre-filled** (`-C/--cd` sets the dir). Auto-run is `codex exec` (headless). | `cli` for Codex = `-C` + CLI. Press Enter to run. |
| Codex **app** launch? | `codex://threads/new?path=<abs>&prompt=<enc>` — **officially documented**, reliable on macOS, **pre-fills** composer text. Buggy on Windows. | `ide` for Codex = this scheme via `open_url`. The clean case. |
| Claude CLI session w/ worktree + prompt? | `cd <wt> && claude --model <m> "<prompt>"` opens the TUI with the prompt **pre-filled, not sent**. No `--cwd` ([#26287](https://github.com/anthropics/claude-code/issues/26287)) — must `cd`. Avoid `-w/--worktree`: it *creates* a worktree under `.claude/worktrees/`. Auto-run is `-p/--print` (headless). | `cli` for Claude = `cd` + CLI. Press Enter to run. |
| Claude **app** launch? | **`claude://code/new?folder=<abs>&q=<prompt>` opens the Claude Code desktop GUI (Code tab)**, at the folder, prompt **pre-filled** ([deep-links doc](https://code.claude.com/docs/en/deep-links), [support](https://support.claude.com/en/articles/14729294)). Distinct from `claude-cli://`, which opens the CLI in a terminal. `open -a "Claude" /path` fails (Electron arg-drop, [#54614](https://github.com/anthropics/claude-code/issues/54614)) — use the scheme. | `ide` for Claude = `claude://code/new` via `open_url`. Folder needs per-launch confirmation; sessions are parallel in the sidebar (no clobber). |
| opencode session w/ worktree + prompt? | `opencode <wt> --prompt "<seed>" -m <provider/model> --agent <name>` launches the TUI in the dir and **auto-submits** on builds after ~Dec 15 2025 (PR [#4510](https://github.com/sst/opencode/issues/3937)); **pre-fill only** before. No app, no scheme. | opencode = `cli` only. The one true interactive auto-send. |
| What existing code is the seed? | `open_web_client` (`util.rs:23`) maps harness → **web URLs** (wrong target). `engine::platform::open_url` is the open primitive. `IdeConfig` (`config.rs:130`) is parsed but **never consumed**; `which("cursor")` in `ops/mod.rs:1541` is doctor-only — there is no `lf ide` launcher today. Harness→CLI is hardcoded `Command::new("claude"/"codex"/"opencode")` in `lfd/sessions/harness/<name>.rs`. | Build `cli`/`ide` atop `open_url` + the harness-name map. Repurpose `open_web_client`; replace the dormant `IdeConfig` with `session.launch`. Greenfield, not a refactor. |

**Live check — does `ide` open a real GUI? (mostly answered 2026-06-19)**

1. **GUI or just a terminal? — Resolved.** Both Codex (`codex://threads/new`) and
   Claude (`claude://code/new`) open a genuine GUI. The earlier "Claude is
   terminal-only" scare was firing the wrong scheme — `claude-cli://` opens the
   CLI in a terminal; `claude://code/new` opens the Code-tab GUI. Two distinct
   schemes, two distinct surfaces.
2. **Isolated session or clobber?** — **Claude: no clobber** (deep links add
   parallel sessions to the sidebar). **Codex: accepted as-is** — we ship on
   whatever the app does by default; not a blocker.

Smaller residual unknown:
- Confirm the exact opencode release that first shipped `--prompt` auto-submit.

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Keep the four targets (`tui`/`embedded`/`app`/`ide`) | Maps each surface explicitly | `embedded` is Concerto rendering `cli`; `app` and the old `ide` both meant "a GUI" — and the auto-send asymmetry that justified the split turned out false. Two values cover it. |
| URL scheme for `cli` too | One mechanism, clickable links | Version-gated (`claude-cli://` needs v2.1.91+), opencode has none, and it buys nothing now that the CLI also only pre-fills. CLI is the portable primitive. |
| Cursor GUI as an `ide` target | Popular editor | No folder+prompt GUI launch exists (deeplink has no folder param). Would ship a fragile two-step or a terminal-only fallback dressed as a GUI. |
| Auto-detect target from environment | Zero config | The thesis is explicit: the choice is explicit and cheap, not a heuristic. A surprised user in the wrong surface is worse than a config line. |

## Key decisions

- **Two targets: `cli` and `ide`.** Terminal vs the vendor's standalone app. No
  `tui`/`embedded`/`app` sprawl — `embedded` is a Concerto rendering of `cli`.
- **Both pre-fill; nothing auto-runs interactively** (except opencode `--prompt`).
  The config picks the *surface*, not running-vs-ready. This is the honest
  handoff: land in the right place, prompt loaded, press Enter.
- **`ide` = the standalone vendor app via URL scheme** — Codex
  (`codex://threads/new`) and Claude (`claude://code/new`, **not** `claude-cli://`),
  never `open -a`.
- **Cursor is excluded** — no clean GUI folder+prompt launch.
- **opencode is `cli`-only** — no app, by absence not by our choice.
- **Graceful fallback:** if the chosen `ide` app/scheme is unavailable (old Claude,
  no Codex app), fall back to `cli` with a one-line notice rather than failing.
- **Per-surface defaults:** bare terminal → `cli`; Concerto → `cli` in the embedded
  pane, with an explicit "open in app" action that fires `ide`.

## Implementation

Build order, with the real code anchors (line numbers as of this branch — confirm
before editing).

**1. Config — `config.rs`.** Add a `session` block; remove the dormant `IdeConfig`
(parsed-but-unused, ~`config.rs:130`).

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LaunchTarget {
    #[default]
    Cli,
    Ide,
}

#[derive(Debug, Clone, Default)]
pub struct SessionConfig {
    pub launch: LaunchTarget,   // session.launch: cli | ide
}
```

Parse `session.launch` next to the existing `agent` knob (`parse_agent`,
~`config.rs:386`). Local repo config, so a `Default` of `Cli` is fine — this is
**not** a wire DTO, so the no-defaults rule doesn't apply.

**2. Dispatcher — `lf/commands/util.rs`.** Replace `open_web_client` (~`util.rs:23`,
maps harness→web URL, the wrong target) with:

```rust
fn launch_session(
    target: LaunchTarget,
    harness: Harness,        // claude | codex | opencode
    worktree: &Path,
    prompt: &str,
) -> Result<(), LaunchError>
```

- **`Cli`** — spawn the harness CLI with cwd = worktree (`Command::current_dir`),
  prompt as the positional arg:
  - codex: `codex -C <wt> [-m <model>] -s workspace-write -a on-request "<prompt>"`
  - claude: `current_dir(<wt>)` then `claude [--model <m>] "<prompt>"` — no `--cwd`,
    and do **not** pass `-w/--worktree` (it creates its own worktree).
  - opencode: `opencode <wt> --prompt "<prompt>" [-m <provider/model>] [--agent <name>]`
- **`Ide`** — build the scheme URL, hand to `engine::platform::open_url`:
  - codex: `codex://threads/new?path=<enc(wt)>&prompt=<enc(prompt)>`
  - claude: `claude://code/new?folder=<enc(wt)>&q=<enc(prompt)>`
  - opencode: no scheme → fall back to `Cli`.
  URL-encode both the **absolute** worktree path and the prompt (percent-encode;
  space→`%20`, newline→`%0A`; never leave a bare `&` in the prompt). Reuse the
  existing encoder or `percent-encoding`.

The `model` comes from the parsed `agent: harness:model`; omit the flag when no
model is set.

**3. Fallback.** `Ide` with no available scheme — opencode always; codex/claude if
`open_url` reports "No application knows how to open URL" — falls back to `Cli`
with a one-line notice. Never hard-fail on a missing GUI.

**4. Wiring.** Route the `lf <step>` path that currently calls `open_web_client`
(the `--web`/clipboard branch) to
`launch_session(cfg.session.launch, harness, worktree, prompt)`, using the step's
worktree and assembled prompt. Bare-terminal default is `Cli`.

**5. Verify — `scripts/`.** A script that, per target × harness, asserts the right
process spawned (`cli`) or the right scheme string handed to the opener (`ide`)
against the right cwd. Capture the spawn/`open_url` arg rather than really
launching; the final Enter stays manual.

## Scope

- **In scope:** `session.launch: cli | ide` config + parse; a `launch_session`
  dispatcher replacing `open_web_client`; `cli` for Codex/Claude/opencode; `ide`
  for Codex + Claude via their schemes; `lf <step>`-first wiring; fallback-to-`cli`.
- **Out of scope:** the Concerto "open in app" button (separate desktop task —
  `wave/desktop` consumes this launcher); the teardown of `lfd/sessions/harness`
  and native chat (separate teardown branch); resuming existing sessions
  (`--resume`/`--continue`) — this milestone is *new* sessions; Cursor as an `ide`
  target; the file-browser pane.

## Done when

- `lf <step>` with `session.launch: cli` opens a fresh vendor CLI session in the
  step's worktree with the prompt loaded — Codex, Claude, opencode.
- `session.launch: ide` opens the standalone Codex or Claude app at the worktree
  via the URL scheme with the prompt pre-filled; a missing app or old version
  falls back to `cli` with a notice.
- Choosing the surface is one config line; no heuristic.

Verify with a script under `scripts/` that, per target, asserts the right process
or scheme launched against the right cwd (mechanism-level — the final Enter is
manual).
