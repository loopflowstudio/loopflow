# Release Decisions — unreleased

Append-only ledger of release-worthy intent and policy decisions for the current
cycle. Promoted to `release/v<version>/DECISIONS.md` at release time.

## 2026-06-19 — Loopflow is the layer above: hand off interactive sessions to the vendors

**Context:** We built our own interactive surfaces — mobile pairing (`lf op
pair`, QR/Tailscale), a native SwiftUI chat UI, and an lfd session-hosting layer
that parses Claude/Codex/opencode streams into our own event model (~7,800 LOC of
`lfd/sessions/harness`). Then we started tearing it back out, in place, on `main`,
twice, with nothing written down. The pattern is the tell: reimplementing the
vendors' own session UIs is work that doesn't compound. The vendors ship better
chat clients, IDE integrations, and mobile apps than we will, and they ship them
faster.

**Decision:** Loopflow is the orchestration layer. It runs headless agent work
(steps, flows, waves) and **does not host interactive sessions.** When a human
drives a session, it happens in the vendor's own surface:

- **Launch new sessions in the vendor's app** — `lf <step>` opens a fresh session
  directly in the Codex / Claude Code app, automatically, when configured that
  way. This is the headline new capability, and it is terminal-first: a plain
  `lf` invocation does it; Concerto is just another caller.
- **Embedded terminal** stays — a tmux pane running the vendor's own TUI (Claude
  Code, Codex CLI, opencode) inside Concerto. Concerto frames it; the vendor
  renders it.
- **Bounce to the vendor's IDE** — open the worktree in VS Code / Cursor /
  JetBrains where the vendor's extension runs the session. Not websites — IDE
  integrations.
- **opencode → TUI only**, for now.

Concerto stays as the macOS surface, raised a layer: wave monitoring plus the
frame around vendor TUIs. It is no longer a chat client.

**Implications:**

- **Dropped:** native SwiftUI chat UI; the `lfd/sessions/harness` stream-parsing
  layer (it existed only to feed the native UI; confirmed separable from the
  embedded terminal); the entire mobile surface — iOS target, `lf op pair`,
  pairing tokens, remote-lfd-for-phone connection infra.
- **Kept:** the embedded terminal (tmux-backed pane).
- **New build:** config-driven vendor-session launch from `lf` and Concerto. The
  open question is the launch mechanism each vendor exposes (URL scheme vs CLI vs
  `open -a`) and how much context (worktree, initial prompt) it accepts — a spike
  gates the config design.
- **Mobile wave archived.** Mobile happens through the vendors' own mobile apps,
  not a loopflow iOS app.
- The in-place teardown on `main` was dropped (stashed) in favor of doing it
  deliberately as part of executing this thesis.

## 2026-06-19 — Run steps as vendor Skills; retire the assembled prompt for handoffs

**Context:** The first cut of vendor-session launch passed loopflow's whole
assembled prompt to the vendor CLI as a positional argument. That prompt is ~100KB
(repo docs, scratch/, wave/, diff, surface, directions). Three independent walls
hit at once: (1) a single argv entry is capped at 128KB on Linux and vendor TUIs
truncate far below that — the seed arrived cut off; (2) Claude's subscription auth
flags **system prompts** that name competitor agents, and loopflow's context is
full of "Codex / OpenCode / Gemini", so `--append-system-prompt-file` is poisoned
for the Claude harness; (3) the GUI deep links (`claude://code/new`,
`codex://threads/new`) take no system-prompt parameter and cap the user seed at
~5KB. Every path to "inject our context at launch" was blocked. Meanwhile both
vendors shipped the same answer to "reusable instructions": **Skills** — the open
`SKILL.md` standard, discovered per-repo and globally, loaded progressively (name
+ description up front, body only on invoke). Verified on-machine that a synced
`/step` fires under **headless** `claude -p` and `codex exec`, not just
interactively.

**Decision:** Stop assembling a prompt for the handoff. Loopflow's execution model
becomes **files on disk + a tiny seed**:

- **Steps are Skills.** A sync emits each step as a `SKILL.md` into four targets —
  `.claude/skills/` and `.agents/skills/`, each at repo and global scope. Claude
  emits carry `disable-model-invocation: true` (explicit-only, zero context cost);
  Codex self-caps its skill index. Body stays out of context until `/step` fires.
- **The seed is `"<surface preamble> /step"`** — the only per-run injection, kept
  small. Identical shape headless and interactive; the surface preamble is the
  one thing that varies (headless "never ask, decide and note ambiguity" vs cli
  "ask and wait").
- **Ambient context moves to AGENTS.md / CLAUDE.md** (vendor auto-loads,
  always-on): repo conventions, **VOICE.md**, orientation ("read `scratch/<branch>.md`
  and `wave/<name>/` first"), and any wave-standing perspective. The agent reads
  scratch/ and wave/ on demand via file tools — we point, we do not dump.
- **Directions are removed as a first-class concept.** A direction was a
  perspective fragment injected into the assembled prompt; with no assembled
  prompt and a human (or a surface preamble) steering, it has no delivery vehicle.
  A direction is a degenerate skill. Standing perspective for an autonomous wave
  becomes a line in that wave's AGENTS.md; occasional perspective is an invoked
  skill. The wave model simplifies from **area × direction × flow** to
  **area × flow**.

**Implications:**

- **Headless and interactive unify** onto one execution model: pre-sync skills,
  then `exec`/open with a surface-stamped `/step` seed. Headless stops assembling
  a ~100KB prompt.
- **Removed:** the `direction` config field and wave-YAML key, the `-d/--direction`
  flag, `builtins/directions/`, the direction loader and prompt-injection path,
  and the `with_direction*` goldens (~43 non-test Rust refs).
- **The `--tui/--ide` launcher and the skills work are one milestone, one branch.**
  The launcher (already committed on session-handoff) is non-functional alone — it
  seeds a blob the TUIs truncate and the GUI deep links can't carry. It picks the
  *surface*; skills make the *seed* (`/step`) work. Shipping the launcher without
  skills would land a broken feature, so they go together here.
- **System prompts are off the table for the Claude harness, by policy** — recorded
  so no one re-discovers the competitor-mention block the hard way.
- **`lf-prompt` is unfaithful** — it skips `drop_native_instruction_docs`, so its
  dump overcounts the real prompt (showed CLAUDE/AGENTS/STYLE triple-included when
  the real launcher already drops them). Fix it to match the launcher, or stop
  trusting it for size measurements.
- **Symlinked agent docs** (`CLAUDE.md`/`AGENTS.md` → `STYLE.md`) are already
  deduped by the launcher; the `lf-prompt` discrepancy was the only place the
  triple-count appeared.
