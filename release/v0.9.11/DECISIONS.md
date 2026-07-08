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
(skills, flows, waves) and **does not host interactive sessions.** When a human
drives a session, it happens in the vendor's own surface:

- **Launch new sessions in the vendor's app** — `lf <skill>` opens a fresh session
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

## 2026-06-19 — Run skills as vendor Skills; retire the assembled prompt for handoffs

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
`/skill` fires under **headless** `claude -p` and `codex exec`, not just
interactively.

**Decision:** Stop assembling a prompt for the handoff. Loopflow's execution model
becomes **files on disk + a tiny seed**:

- **Skills are Skills.** A sync emits each skill as a `SKILL.md` into four targets —
  `.claude/skills/` and `.agents/skills/`, each at repo and global scope. Claude
  emits carry `disable-model-invocation: true` (explicit-only, zero context cost);
  Codex self-caps its skill index. Body stays out of context until `/skill` fires.
- **The seed is `"<surface preamble> /skill"`** — the only per-run injection, kept
  small. Identical shape headless and interactive; the surface preamble is the
  one thing that varies (headless "never ask, decide and note ambiguity" vs cli
  "ask and wait").
- **Ambient context moves to AGENTS.md / CLAUDE.md** (vendor auto-loads,
  always-on): repo conventions, **VOICE.md**, orientation ("read `scratch/<branch>.md`
  and `wave/<name>/` first"), and any wave-standing perspective. The agent reads
  scratch/ and wave/ on demand via file tools — we point, we do not dump.
- **Directions are removed as a first-class concept.** A direction was a
  perspective fragment injected into the assembled prompt; with no assembled
  prompt and a human (or surface preamble) steering, it has no delivery vehicle.
  The machinery goes; the direction *text* survives, redistributed by where the
  perspective belongs: **most embeds into the relevant skill-skills** (perspective
  that shapes how a skill is done), **some moves to AGENTS.md** (always-on standing
  point of view for a repo/wave). The exact split is worked out with concrete
  examples at build time. The wave model simplifies from **area × direction ×
  flow** to **area × flow**.
- **Flows are unchanged; only the interactive session relocates.** Flow
  orchestration stays the purview of Cadenza and the `lf` CLI — a flow is still
  loopflow chaining skills. Flows do *not* become skills. Inside a flow, an
  interactive skill hands off to the vendor session (`/skill` seed); a headless skill
  `exec`s. `lf code` still runs as a flow.
- **Skill sync is global, prune later.** All builtin skills sync to
  `~/.claude/skills` and `~/.agents/skills` — they appear in every project's
  session, which is acceptable. No namespacing for now; prune if the menu gets
  noisy. Generated skills carry a provenance marker so re-sync can prune safely.

**Implications:**

- **Headless and interactive unify** onto one execution model: pre-sync skills,
  then `exec`/open with a surface-stamped `/skill` seed. Headless stops assembling
  a ~100KB prompt.
- **Removed:** the `direction` config field and wave-YAML key, the `-d/--direction`
  flag, `builtins/directions/`, the direction loader and prompt-injection path,
  and the `with_direction*` goldens (~43 non-test Rust refs).
- **The `--tui/--ide` launcher and the skills work are one milestone, one branch.**
  The launcher (already committed on session-handoff) is non-functional alone — it
  seeds a blob the TUIs truncate and the GUI deep links can't carry. It picks the
  *surface*; skills make the *seed* (`/skill`) work. Shipping the launcher without
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

## 2026-06-24 — Ambient context moves to disk; one seed path, harness-aware sigil

**Context:** The skills milestone (2026-06-19) routed both interactive and headless
named-skill runs through the `/skill` seed but carried the always-on context (the
`LOOPFLOW.md` operating manual, the orientation header) in the seed or as an
injected system doc. Two things were off. First, `LOOPFLOW.md` was injected into
every session as product context even though it's loopflow's own operating manual,
and the orientation header was a seed-level afterthought disconnected from the
skills that actually depend on `scratch/`. Second, the seed hard-coded a `/skill`
invocation for every vendor, but Codex's interactive composer reserves `/` for
built-in commands — skills fire there with `$skill`.

**Decision:**

- **Both surfaces keep the `/skill` seed.** Skills fire under `claude -p` and
  `codex exec` (re-verified on-machine, sentinel probe), so headless stays on the
  seed too — it is *not* sent back to the assembled prompt. The seed must carry the
  surface run-mode preamble for both: the headless warning ("no user present,
  decide and keep moving, note ambiguity in `scratch/questions.md`") and the cli
  "ask and wait" line. `surface.instructions()` already supplies this.
- **Harness-aware invocation sigil.** `skill_launch_seed` emits `$skill` for Codex
  and `/skill` for Claude. `$` works in *both* Codex paths (exec and the interactive
  composer); `/` only works in `codex exec`, not the composer. Claude uses `/`
  everywhere.
- **`LOOPFLOW.md` leaves the product.** The operating manual is no longer injected
  into any prompt; its content moves into loopflow's own agent doc (`STYLE.md`, which
  `CLAUDE.md`/`AGENTS.md` symlink to), auto-loaded by the vendor only when working on
  loopflow. `LOOPFLOW_DOC` and the `loopflow_doc` prompt field are deleted; `RLM`
  becomes the unconditional system section.
- **Orientation embeds into the skills that need it.** The orientation block (read
  `scratch/`, `wave/`, the agent doc) is embedded directly into the body of every
  skill that references `scratch/` (all 37 — build, govern, ops), so it travels with
  the skill instead of riding the seed. Dropped from `skill_launch_seed`;
  `ORIENTATION.md` deleted.

**Implications:**

- The 2026-06-19 "headless and interactive unify onto one execution model" line
  holds — they share both the skills *and* the seed path. The only per-surface
  difference is the run-mode preamble; the only per-harness difference is the sigil.
- User repos no longer receive the loopflow operating manual as ambient context.
  Operational knowledge now comes from the skill bodies (skills) plus the repo's own
  agent doc. Acceptable: the manual is loopflow-specific, and a `git`-managed agent
  doc is the right home for it.
- Orientation is duplicated across 37 skill files (no include mechanism for skill
  `.md`). That duplication is the cost of "embedded in the relevant skills"; re-run
  the embed if the set of scratch-dependent skills changes.

## 2026-06-29 — Self-hosted loopflow is the default automation shape

**Context:** Release and cron automation had been drifting toward a private studio-hosted deployment model, with Terraform and secret handling living outside this repo. The release server needs to be inspectable, reproducible, and maintainable from loopflow itself, whether it runs on a Mac mini, Tailscale host, Fly.io, or cloud VM.

**Decision:** Loopflow automation is self-hosted by default. The public repo carries the runnable container and deployment shape; Doppler supplies secrets; studio discovery is removed rather than the assumed path. Nightly package verification proves artifacts without deploying, weekly release is the automated publishing cadence, and local dev machines get a single `scripts/pull-local-bin.sh` update path.

**Implications:** Container mode uses self-hosted bearer-token auth only. Deployment docs and compose defaults must avoid hidden global-host assumptions. Infrastructure config can be committed when it contains topology and mechanics, but credentials stay in Doppler or local env files.

## 2026-06-29 — Hashimoto-style review is a standing quality ritual

**Context:** Loopflow's core work benefits from a specific review posture: operationally boring, API-centered, skeptical of needless abstraction, and clear about what breaks under real use.

**Decision:** Every unit of work gets a Mitchell Hashimoto-style simulated code review before it is considered done. The point is not impersonation; it is a concrete quality lens for simplicity, operations, API shape, docs, and deletable complexity.

**Implications:** Agents should either fix findings immediately or record them in PR notes. Rubber-stamp review theater does not satisfy the ritual.

## 2026-06-29 — Loopflow and Cadenza share one release cadence

**Context:** Loopflow's release automation should not become a one-off snowflake while Cadenza drifts onto a separate schedule. Both projects need the same operational rhythm, even though their artifacts and signing requirements differ.

**Decision:** Loopflow and Cadenza use carbon-copy nightly and weekly schedules: nightly package verification at `0 9 * * *` UTC without deployment, and weekly release at `0 12 * * 0` UTC gated by the same package verification. Repo-specific parameters live inside each repo's workflow body.

**Implications:** Changing the cadence means changing both repositories together. Cadenza does not inherit Loopflow's Rust package matrix, and Loopflow does not inherit Cadenza's signing-sensitive TestFlight path; only the rhythm and gate semantics are shared.

## 2026-06-29 — Remove studio auth

**Context:** Secure remote execution still matters, but the global studio-discovery server made loopflow feel centrally hosted by default and pushed deployment mechanics into private infrastructure.

**Decision:** Delete studio auth, daemon registration, and hosted discovery. Remote `lfd` access is self-hosted bearer-token auth only; each repo owns its deployment config and keeps secrets in Doppler or host-local env.

**Implications:** Concerto connects to explicit self-hosted URLs and tokens instead of studio sign-in. Container compose requires `LFD_AUTH_TOKEN`. Token rotation happens in the repo/host secret system, not through a studio connection-token ledger.

## 2026-06-29 — Release automation gets its own wave

**Context:** Release automation now spans CI cadence, self-hosted daemon infrastructure, local freshness, Cadenza parity, and future product replication. Keeping that intent only in conversation makes future agents rediscover scope and success criteria.

**Decision:** Add `wave/release/` as the owner for daily verification, weekly publishing, self-hosted cron infrastructure, local updater freshness, and product-release parity. Root gardens it alongside desktop, mobile, and workflows.

**Implications:** Release work is no longer incidental workflow plumbing. Changes to schedules, deploy shape, Doppler assumptions, or cross-repo parity should update the release wave metadata and include a Mitchell Hashimoto simulated review when they ship.

## 2026-06-29 — First maintained Loopflow host is private

**Context:** The release automation goal needs one real self-hosted `lfd` server, not a generic cloud abstraction. The first target is a private tailnet host, but public repo metadata should not expose personal hostnames or tailnet addresses.

**Decision:** Target a private Tailscale-connected host as the first maintained Loopflow `lfd` cron host. Local clients use Tailscale HTTP with bearer-token auth first; Caddy/TLS remains available for later public or polished access. Concerto, `lfq`, Codex, and Claude sessions should point at that host rather than a studio control plane. Host-specific names, addresses, users, and tokens stay in local env, Doppler, or private machine config.

**Implications:** Setup scripts and docs optimize for a private Tailscale host without committing personal topology. Secrets stay in Doppler or host-local env, agent credentials are made available to the private executor, and remote repo paths are paths on that host. Cadenza remains cheap and product-specific: one prod server, regular/hotfix releases, and local/TestFlight clients pointed at prod unless a deliberate staging need appears.

## 2026-06-30 — Spend over $100/month is the next human blocker

**Context:** Release automation needs to keep moving without checking in for every reversible skill, but cloud hosts and agent providers can create open-ended spend.

**Decision:** Continue autonomous release-infra iteration until actual or projected automation spend would exceed $100/month. Card/bank transactions are the source of truth; AWS, Fly.io, Claude/Anthropic, OpenAI/Codex, OpenCode, Doppler, and release-host services should use the company card when the vendor supports card billing.

**Implications:** Cost tracking is part of release infrastructure, not bookkeeping after the fact. Provider dashboards can warn early, but the monthly budget gate is enforced from transaction exports/API data. Spending above the threshold requires human approval before proceeding.

## 2026-06-30 — Release infra is measured by cadence, host health, and budget

**Context:** The release-infra work had become several intertwined threads: nightly package checks, weekly releases, local `lf` refresh, a self-hosted `lfd` cron host, Cadenza production deployment, and cost controls. Without a written operating contract, progress was easy to mistake for whichever deployment task happened most recently.

**Decision:** Treat the goal as one release system with explicit measures: nightly package verification green, weekly release gated by that verification, local refresh as one command, self-hosted cron host reachable and observable, and automation/runtime spend under $100/month. Loopflow owns the primitives and cron host; Cadenza mirrors the cadence and proves the product-repo deployment shape. Secrets stay in Doppler or host-local env; private host details stay out of public repos.

**Implications:** The next Loopflow work should prioritize local `lf`/`lfd` refresh and the self-hosted cron host before adding more product-specific deployment features. Any attempt to introduce a global studio-hosted default server is out of scope unless explicitly re-decided. Cost above $100/month is the next true human blocker.

## 2026-06-30 — Mac private cron hosts use native lfd first

**Context:** Bringing up the private Mac host exposed Docker Desktop assumptions that make the container stack a poor first skill on macOS: launchd PATH, Docker Desktop context, credential helper behavior, and slow first-run image builds. Native `lfd` already supports launchd, remote bind, and bearer-token auth, and it became healthy with fewer moving parts.

**Decision:** Use native launchd `lfd` as the default Mac private cron-host path. Keep Docker Compose as an explicit self-hosted/container option, especially for Linux or hosts that need container isolation, but do not block the Mac private host on it.

**Implications:** The Mac host runbook centers on `deploy/native-lfd-host.sh`. Secure remote execution still needs a containerized executor story, but daemon availability, wave scheduling, and remote API access should not wait for Docker Compose first-run polish.

## 2026-06-30 — The desktop app is "Loopflow"; Concerto stays the dev nickname

**Context:** The macOS app shipped as "Loopflow Concerto" — `Loopflow Concerto.app`, `LoopflowConcerto.dmg`, display name "Loopflow Concerto." Internally everyone calls it Concerto. The product users actually use is Loopflow; `lf`/`lfd` are the lower-level internals. The two-word product name muddied that.

**Decision:** The user-facing product is **Loopflow**. Rename every surface a user sees — `CFBundleDisplayName`/`CFBundleName`, the app bundle (`Loopflow.app`), the DMG volume and download keys (`Loopflow-<version>.dmg`, `Loopflow-latest.dmg`), and in-app titles. **Concerto stays the internal/dev nickname** — the Swift target/executable, the bundle id `com.loopflow.concerto` (keeping it preserves granted TCC permissions and deep-link registration), `scripts/concerto-dev.py`, and the debug `Concerto Dev.app` are unchanged. No tooling reorg.

**Implications:** The DMG download key changes, so the loopflowstudio website download link must update in the same window or it 404s (cross-repo, flagged, not done here). Keeping the bundle id means no permission re-grant. The app's bundle version is now stamped from the release tag at build time rather than frozen at `1.0`, so Loopflow reports the same version as `lf --version`.

## 2026-06-30 — One local build entry; per-worktree `local-bin/`

**Context:** Three overlapping local-install paths (`pull-local-bin.sh` → `~/.local/bin`, `install.py local` → `/Applications` + bin, `concerto-dev.py run` → `~/Applications`) all wrote to **global** locations, so sibling worktrees fought over the same `lf` and the same `/Applications` app — you couldn't run two worktrees' builds side by side.

**Decision:** `scripts/install.py local` is the single local build entry. It builds this worktree's `lf`, `lfd`, and `Loopflow.app` into `<worktree>/local-bin/` (gitignored, isolated per worktree). `--use` *promotes* a build: symlink `~/.local/bin/{lf,lfd}` → the worktree's `local-bin`, and copy `local-bin/Loopflow.app` → `/Applications`. Build is isolated; `--use` chooses the active one. The desktop app is a first-class release artifact alongside `lf`/`lfd`, named as such in `release/README.md` and `release/SCHEDULE.md`.

**Implications:** `pull-local-bin.sh` stays as the CLI-only quick path; `concerto-dev.py` stays for fast debug iteration. Symlinked `lf`/`lfd` mean rebuilds take effect with no re-promote; the app still needs `--use` to re-copy. Switching active worktrees is one `install.py local --use` from that worktree.

## 2026-06-30 — Wave → Loop; the MVP vocabulary is Loop / Worker / Flow

**Context:** The product reads as its own model — Loop + flow → Loopflow — but the runtime noun was "Wave." The goals-wave design (committed earlier today) deliberately kept "Wave" and added "Goal" as the loop prompt. A design pass on the vocabulary landed on "Loop" as the clearer central noun: it says what the thing is (an always-running, steerable session) and it's the word in the product name.

**Decision:** The MVP vocabulary is **Loopflow** (product) → **Loop** (always-running interactive session: steers with the human, aligns to the **Goal** + metrics, schedules against a budget) → **Worker** (a hosted tmux session running one **Flow** on one **Task**) → **Flow** (the grain a Loop dispatches whole) → **Skill** (the internal atom `lf` executes). **Loop renames Wave**, superseding the goals-wave "keep Wave" call. Tasks live in **Asana**, generated in the Loop's conversations. A blocked Worker is a joinable session (embedded Ghostty in Concerto), holding its slot but spending no budget while it waits. **Chords** (a Loop whose children are Loops) and **gardening** are parked as workflows, not MVP nouns.

**Implications:** The grain line sits at Worker→Flow: a Loop reasons in Flows and Tasks, never in Skills. The Wave→Loop rename is wide (`loopflow.api`, `wave/<name>/`, Concerto UI, DTOs mirrored three ways, config keys, goldens) and is its own migration wave — this decision fixes the vocabulary, not the rename. Design captured in `scratch/vocabulary.md`.
