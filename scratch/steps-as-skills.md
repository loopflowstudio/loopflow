# Steps as Skills: the handoff execution model

Build plan for **this branch** (session-handoff). The `--tui`/`--ide` launcher
already committed here is non-functional on its own: it seeds loopflow's assembled
prompt, which the vendor TUIs truncate and the GUI deep links can't carry. The
launcher only becomes real once steps are exposed as vendor **Skills** and the
seed shrinks to a vendor skill invocation — so the two ship together, not as separate branches.
Turns the converged thesis — *loopflow stops assembling prompts; steps become
vendor Skills* — into a buildable milestone. The load-bearing assumption (a synced
skill fires under headless exec) is **verified on-machine**, both vendors. See
`release/unreleased/DECISIONS.md` (2026-06-19, "Run steps as vendor Skills").

## Goal

Loopflow stops assembling prompts for handoffs. Steps become vendor **Skills** on
disk; the launch seed shrinks to a surface preamble plus the vendor skill
invocation. One execution model for headless and interactive, both surfaces, both
vendors.

## Status

**Done — committed on this branch (do not redo):**

- `--tui`/`--ide` launcher + `session.launch: tui | ide` config (selects the
  *surface*). Builds, clippy-clean, tests pass.
- `--web` removed; `LaunchTarget::Cli` renamed to `Tui`; docs updated.
- Native agent-doc dedup confirmed working in the launcher
  (`drop_native_instruction_docs` strips `CLAUDE.md`/`AGENTS.md` + symlink targets).
- Decision recorded in `release/unreleased/DECISIONS.md`.
- **Verified on-machine:** synced skills fire under headless `claude -p` and
  `codex exec`, with the body loaded only on invoke (sentinel probe). Claude uses
  `/step`; Codex handoffs use `$step`.
- **Verified — `--ide` GUI path works** for both Claude (`claude://code/new`) and
  Codex (`codex://threads/new`): the deep link opens the app and **pre-fills** the
  seed. Pre-fill (not auto-send) is the **intended** shape — the human lands with
  the skill invocation staged, reviews, and presses Enter to fire it; that *is* the
  take-over-and-review handoff. The *only* failure mode observed was the prompt
  getting **cut off** at the deep-link length cap (~5KB `q`). The skill seed is a
  dozen characters, so the skills work removes the one blocker — the GUI mechanics
  need no further change.

**Remaining — this milestone, in build order:**

1. **Skill sync** — a `lf op sync-skills` (name TBD): each resolved step →
   `SKILL.md` into four targets (`.claude/skills` + `.agents/skills`, repo +
   global). Frontmatter transform; `disable-model-invocation: true` on Claude
   emits; provenance marker + prune of stale generated skills; confirm before the
   first global (`~/`) write. *(detail: §1)*
2. **Seed swap** — the interactive launch path (`run.rs` `launch_prompt`) sends
   a harness-specific skill invocation (`/<step>` for Claude, `$<step>` for Codex)
   instead of `built.prompt`; sync skills first. *(detail: §2)*
3. **Ambient context on disk and in skills.** *Done — committed.* The loopflow
   operating manual moved into this repo's agent doc (`STYLE.md`, reached through
   `CLAUDE.md`/`AGENTS.md` symlinks), and branch orientation is embedded directly in
   the step bodies that need it. The launch seed carries only surface instructions,
   voice, and message context. *(detail: §3)*
4. **Headless unification** — wave/flow headless runs pre-sync, then `exec` the
   skill seed; stop assembling the ~100KB prompt. *(detail: §4)*
5. **Remove Directions** — deferred. `direction` is a first-class wave field threaded
   through DTOs, SQL migrations, HTTP routes, and the Rust/Python/Swift mirrors
   (~580 Rust refs, not ~43) — a wire-format migration, not a flag removal. It needs
   its own pass under the DTO fixture discipline. *(detail: §5)*
6. **Verify** — a `scripts/` sentinel-probe: sync a step, fire it under `claude -p`
   and `codex exec`, assert the step's effect.

The numbered detail for each remaining item is in **Approach** below.

## Problem

The first vendor-session launcher passed loopflow's ~100KB assembled prompt to the
vendor CLI as a positional argument. Every path to "inject our context at launch"
is walled off:

- **argv / TUI limits** — a single argument caps at 128KB on Linux; vendor TUI
  composers truncate far below. The seed arrived cut off mid-document.
- **system-prompt policy** — Claude's subscription auth flags system prompts that
  name competitor agents. loopflow's context is wall-to-wall "Codex / OpenCode /
  Gemini", so `--append-system-prompt-file` is poisoned for the Claude harness.
- **GUI deep links** — `claude://code/new` and `codex://threads/new` accept no
  system-prompt parameter and cap the user seed at ~5KB.

Meanwhile both vendors converged on the same primitive for reusable instructions:
**Skills** (the open `SKILL.md` standard). A step is already a markdown instruction
file — the same shape as a skill. So we stop fighting the launch channel and put
the work on disk where every surface reads it.

## The model: files on disk + a tiny seed

The old assembled prompt decomposes into exactly three homes. Only the third
travels through the launch channel, and it stays small.

| Home | On disk | Loaded | Carries |
|---|---|---|---|
| **AGENTS.md / CLAUDE.md** | yes | vendor auto-loads, always-on | repo conventions and any repo-owned operating manual |
| **Skills** (`.claude/skills`, `.agents/skills`) | yes | progressive — name+desc up front, body on invoke | step bodies and branch orientation |
| **The seed** | no | typed into the session | `"<surface preamble> /step"` for Claude, `"<surface preamble> $step"` for Codex |

### Verified

| Surface | Seed | Result |
|---|---|---|
| `claude -p "/lfprobe"` | explicit slash | emitted a sentinel that existed only in the skill body |
| `codex exec "/lfprobe"` | explicit slash | `sed`'d `SKILL.md` on invoke, then emitted the sentinel |

Both discovered the skill from project-local dirs and ran the body under headless
exec. Codex's visible read confirms progressive disclosure: 80 synced steps cost
~80 index lines, not 80 bodies — headless included.

## Expose / invoke — the 2×2

Both vendors, both surfaces, read the same on-disk `SKILL.md` and pull the body
only on invoke. The surface axis nearly collapses; the seams are small.

| | Claude TUI | Claude GUI | Codex TUI | Codex GUI |
|---|---|---|---|---|
| Explicit | `/step` | `/step` or + → Skills | `$step` handoff (`/step` also works in exec) | `$step` |
| Reads on-disk skills | ✅ | ✅ | ✅ | ✅ |
| Body in context until invoked | no | no | no | no |

**The only real seams the sync must encode:**

1. **Path dialect** — Claude `.claude/skills/<step>/SKILL.md`; Codex
   `.agents/skills/<step>/SKILL.md`. ×{repo, global} = four targets.
2. **Context knob** — Claude emits set `disable-model-invocation: true`
   (explicit-only, zero index cost); Codex has no per-skill switch but auto-caps
   its index (~2% / 8KB), so no action needed there.
3. **Ignore deprecated single-file forms** — Claude `commands/*.md`, Codex
   `~/.codex/prompts/*.md`. Both superseded by Skills.

## Approach

### 1. The sync — `lf op sync-skills` (name TBD)

Transform every resolved step (builtin + `~/.lf/steps/` + `.lf/steps/`) into a
`SKILL.md` and write to the four targets:

| Step scope | Claude target | Codex target |
|---|---|---|
| global (`~/.lf/steps`, builtins) | `~/.claude/skills/<step>/SKILL.md` | `~/.agents/skills/<step>/SKILL.md` |
| repo (`.lf/steps`) | `<repo>/.claude/skills/<step>/SKILL.md` | `<repo>/.agents/skills/<step>/SKILL.md` |

Frontmatter transform (loopflow step → `SKILL.md`):

- `description` ← the step's one-line summary (the line after frontmatter).
- body ← the step body, unchanged.
- Claude emit adds `disable-model-invocation: true`.
- Drop loopflow-only keys (`requires`, `produces`, `interactive`, `agent`,
  `action_style`) — or fold the useful ones into the body as a hint.

**Scope: global is fine.** Decided — sync all builtins to `~/.claude/skills` and
`~/.agents/skills`; they show up in every project's session, and that's acceptable.
No namespacing for now; **prune later** if the global menu gets noisy.

Still needed: **provenance + safe cleanup.** Mark generated skills (a frontmatter
marker) so a re-sync can prune stale ones without clobbering a user's own skill.

### 2. The seed — replace the assembled-prompt blob

The interactive launch path (the `--tui`/`--ide` work already in this branch)
stops sending `built.prompt`. It sends a harness-specific skill invocation plus the
surface preamble: `/<step>` for Claude and `$<step>` for Codex.

- **surface preamble** ← the surface doc (cli / headless / concerto_*), the one
  per-run modifier. Small. Headless carries "never ask, decide, note ambiguity in
  `scratch/questions.md`"; cli carries "ask and wait."
- **`/<step>` or `$<step>`** ← the step name. The skill body does the rest.

### 3. Ambient context → agent docs + step bodies

The always-on context now has two homes:

- **Repo operating manual** — loopflow's own operating rules live in `STYLE.md`,
  which `CLAUDE.md` / `AGENTS.md` symlink to. User repos no longer receive
  `LOOPFLOW.md` as injected product context.
- **Branch orientation** — the "read `scratch/`, matching `wave/`, and the repo
  agent doc" block is embedded in the step bodies that need it, so it loads with
  the skill.

The launch seed still carries the resolved voice doc because voice is per-session
ambient guidance and stays small enough for GUI deep links.

### 4. Unify headless onto the same model

Headless wave/flow runs stop assembling a ~100KB prompt. They pre-sync skills,
then `codex exec` / `claude -p` a surface-stamped skill seed. Same shape as the
interactive handoff; the surface preamble is the only difference. (Verified that
skill invocation fires under both headless execs.)

**Flows are unchanged.** Flow orchestration stays the purview of Cadenza and the
`lf` CLI — a flow is still loopflow chaining steps (`implement → compress → lint →
gate`). The *only* thing this milestone changes is **where the interactive step
runs**: inside a flow, an interactive step hands off to the vendor session (skill
seed), a headless step `exec`s. Flows do **not** become skills. So `lf code` keeps
running exactly as a flow; only its interactive steps relocate to the vendor.

### 5. Remove Directions

Deferred. The `direction` machinery is a DTO/config/wave migration, not a cleanup
inside this handoff PR. The direction *text* can still survive later, redistributed
by where the perspective belongs:

- **Most direction text → embedded into the relevant step-skills.** A perspective
  that shapes how a particular step is done lives in that step's `SKILL.md` body.
- **Some direction text → AGENTS.md.** Perspective that should be always-on for a
  repo or wave (its standing point of view) lives in the agent doc.

Machinery to delete in that later pass: the `direction` config field, wave-YAML
key, `-d/--direction` flag, `builtins/directions/`, the direction loader and
prompt-injection path, DTO fixtures, SQL migrations, and Swift/Python mirrors.

## Scope

- **In:** the steps→`SKILL.md` sync (4 targets, frontmatter transform,
  `disable-model-invocation` on Claude, provenance + prune); the harness-aware
  skill seed replacing the assembled blob; ambient context moved to agent docs and
  step bodies; headless unification onto the skills seed.
- **Out:** model-*auto* skill invocation by description (the seed is always an
  explicit skill invocation; auto is unproven and off the critical path). Direction
  DTO/config removal. Flow-as-skill conversion. Concerto "open in app" UI. The
  larger `lfd/sessions/harness` / native-chat teardown (separate branch). Session
  resume.

## De-risking

| Question | Finding | Impact |
|---|---|---|
| Does a synced skill invocation fire under headless exec? | **Yes**, both vendors (sentinel-in-body probe). | The whole unification stands. |
| Does the body stay out of context until invoked? | **Yes** — Codex read `SKILL.md` on invoke; both vendors do progressive disclosure. | 80 steps ≠ 80 bodies in context. |
| Skill path per vendor? | Claude `.claude/skills`, Codex `.agents/skills`; both repo + global. | Four sync targets. |
| Can we inject context as a system prompt? | **No** for Claude — competitor-mention auth block; and GUI deep links take no system-prompt param. | Context lives on disk (AGENTS.md + skills), never the system prompt. |
| Auto-invocation by description, headless? | **Untested.** Not needed — seed is explicit. | Revisit only if a wave must auto-fire a perspective. |
| Global-skill scope (`~/.claude/skills`) headless? | Tested project-local (the relevant case); global is the same mechanism per vendor docs, not separately verified. | Low risk; confirm during build. |

## Done when

- `lf op sync-skills` writes every step as a `SKILL.md` to the four targets, with
  Claude emits explicit-only and generated skills marked + prunable.
- `lf <step>` (interactive) opens the worktree and seeds the harness-specific
  skill invocation (`/<step>` for Claude, `$<step>` for Codex); the vendor session
  runs the step from its synced skill.
- A headless wave run pre-syncs and `exec`s the same skill seed — no assembled
  prompt.
- Loopflow's operating manual lives in this repo's agent doc; orientation lives in
  step bodies; the agent reads scratch/ and wave/ on demand.
- Direction removal has a separate migration plan; this PR keeps direction DTOs and
  config intact.

Verify with a script under `scripts/` that syncs a step, fires it under
`claude -p` and `codex exec`, and asserts the step's effect — the same
sentinel-probe shape that de-risked this design.
