---
priority: high
asana_id: '1216257840999672'
roadmap_item: 2-looping-agent-cloud
---

# Looping Agent — codex/claude cloud backend (a)

Design + vendor research for shipping a Goal as a recurring loop inside
codex/claude's own cloud, roadmap-wired and deep-linkable.

## 1. Problem & finish line

Today a Wave loops on *our* runtime: `lfd`'s client-side cron poller
(`triggers/cron.rs`, a 30s tokio interval) fires `spawn_immediate_activation`,
which shells a rendered goal prompt into a local tmux `lf` session. That loop
only turns while `lfd` is up on a machine that's on. We want to **rent the
persistence** — hand the loop to the vendor's cloud and let *their* scheduler
keep it alive.

Backend (a) is the "adapt to your workflow" bet: steps are already Skills, runs
already hand off to a vendor session via the `/step`/`$step` seed
(`lf/commands/run.rs:258`). A Goal is the *looping* version of that same
handoff — the vendor's recurring trigger re-runs the rendered goal prompt; we
supply the prompt + roadmap wiring, not the runtime.

**Finish line.** From Concerto (or `lf`), one repo's Goal runs as a recurring
loop in claude/codex cloud, reads and writes its Asana roadmap, and is reachable
again via a Concerto deep-link.

## 2. A1 vs A2 — build A2

**Decision: A2 (lfd scaffolds, the vendor owns the loop), with one thin,
safe sliver of A1 as a follow-on.** The vendor-API research makes A1-as-the-
foundation untenable *today*, on both vendors, for different reasons.

### What the vendors actually expose (mid-2026)

**Claude — Routines** (research preview, shipped ~Apr 2026). This is the one
primitive that matches the finish line's "native schedule": a saved
prompt + repos + connectors that runs **server-side on Anthropic infra with the
machine off**. Triggers: Schedule (cron, **1-hour minimum**), GitHub events,
and a per-routine **API `/fire`** endpoint.

- Created only via the **web form** (`claude.ai/code/routines`), the Desktop
  app, or the CLI **`/schedule`** (schedule triggers only; API/GitHub triggers
  are web-only). **There is no "create a routine" API.** An orchestrator cannot
  POST a routine into existence.
- Fresh clone per run from the default branch; Claude pushes to `claude/`-
  prefixed branches. Repo access via the Claude GitHub App / `/web-setup`.
- Connectors are your claude.ai MCP integrations **or a committed `.mcp.json`
  in the cloned repo** — this is our roadmap seam (see Design).
- Auth is **claude.ai subscription login** (Pro/Max/Team/Enterprise), *not* an
  API key. The routine belongs to the user's account and acts *as them*.
- The `/fire` endpoint (`POST …/routines/{id}/fire`, `Authorization: Bearer`,
  beta header `experimental-cc-routine-2026-04-01`) *triggers* an existing
  routine and returns `{ claude_code_session_id, claude_code_session_url }`
  where the URL is `https://claude.ai/code/session_…`. It does **not** create or
  schedule anything.

**Codex — Cloud tasks + Automations.** You can launch a cloud task
programmatically: `codex cloud exec --env <id> "<prompt>"` submits to OpenAI's
infra and returns a task URL; `codex cloud list --json` enumerates
`{id, url, status, …}`. **But there is no server-side schedule.** "Automations"
are client-side cron in the desktop app — *the machine must be on and the app
running*. There is no public REST task API (open asks: codex #24777/#25466/
#8317), and cloud tasks require **ChatGPT login** (device-code, beta), not an
API key, with no delegation model for a third party acting on the user's behalf.

### Why not A1

A1 = "lfd drives the vendor cloud API to launch a session **and register a
recurring trigger**." That register step is the crux, and it doesn't exist as a
stable, uniform API on either vendor:

- **Claude has no create-routine API.** The recurrence is registered by a human
  in the web form or `/schedule`. lfd can `/fire` a routine but cannot make one.
- **Codex has no server-side schedule at all.** To get recurrence lfd would have
  to run its *own* always-on daemon shelling `codex cloud exec` on our cron,
  re-authenticating a beta ChatGPT device token — i.e. lfd re-owns exactly the
  runtime it set out to rent, and we're back to `triggers/cron.rs` with extra
  fragility.

So A1 forces per-vendor, brittle automation (browser-driving claude.ai, or a
codex babysitter daemon) against surfaces the vendors label research-preview /
beta / "may change." That's the "more magic, more coupling to moving vendor
APIs" the roadmap item flags, and it breaks the thesis — we'd own the loop we
promised to rent.

**A2 reaches the same finish line** for Claude *with a genuinely hosted loop*
(a Routine runs machine-off), respects "your workflow" hardest, and keeps lfd
out of the lifecycle. The human presses go once; the vendor keeps it alive.

### The safe A1 sliver (follow-on, not the foundation)

Once a Claude Routine exists with an API trigger, lfd *can* `/fire` it — a
stable-enough, single-call nudge ("a roadmap item just landed, run now"). That's
the only place "lfd drives the vendor API" pays off without owning creation or
lifecycle. Build it after A2 lands, behind the beta header, for Claude only.

## 3. Design

One new command, `lf op cloud <vendor>`, that renders the Goal into vendor-
native scaffolding and drops the human at the exact "press go" spot. It plugs
into the existing ops surface: a variant on `OpsCommand` (`lf/mod.rs:156`), an
arm in `commands/ops/mod.rs`, a handler in `src/ops/cloud.rs`. It reuses three
seams that already exist.

**What lfd renders (all committed into the repo the vendor clones):**

1. **The loop prompt.** Reuse `render_goal` (`engine/flow.rs:321`) verbatim —
   the same operating prompt + wave memory + roadmap handle + metrics the local
   loop gets. Write it as the routine/task prompt. The goal body already tells
   the agent to read the roadmap, dispatch, re-measure, repeat.
2. **The flows.** `sync_skills` (`engine/skills.rs:47`) already emits
   `.claude/skills/*/SKILL.md` and `.agents/skills/*/SKILL.md`. The cloud clone
   picks these up natively — the vendor session can invoke every flow as a
   skill. Nothing new; just ensure they're committed, not gitignored, for the
   cloud path (today `ensure_repo_skill_excludes` hides them — the cloud variant
   must commit them or the fresh clone won't see them).
3. **Roadmap access — the real gap.** The local loop reaches Asana by shelling
   `lf op pm` → native `AsanaClient` (`ops/pm.rs`, OAuth). A fresh cloud clone
   has **no `lf` binary and no local OAuth token.** So the cloud session needs
   Asana over **MCP**, which loopflow does not generate today. Add a
   `.mcp.json` emitter: write an Asana MCP server entry keyed to the wave's
   `pm.asana_project` (from GOAL.md frontmatter, `wave_config.rs`). Claude
   Routines read committed `.mcp.json`; Codex reads `.mcp.json` too. This is the
   one net-new capability — everything else is wiring.

**The handoff (per vendor):**

- **Claude:** print/seed the `/schedule` invocation carrying the rendered goal,
  or open `claude.ai/code/routines` prefilled. The human picks a cadence
  (≥ 1h) and creates the Routine. Reuse `skill_launch_seed`'s sizing discipline
  (`run.rs:250` — "small enough for the GUI deep-link cap") so the seed fits.
- **Codex:** emit `codex cloud exec --env <id> "$<goal-skill>"` for a one-shot
  cloud run, plus a note to add an Automation for recurrence (client-side today
  — call out the limitation in output). `AGENTS.md` carries repo instructions.

**Deep-link back out.** Capture the vendor handle and store it on the Wave:
- Claude Routine → routine detail URL; a fired run →
  `claude_code_session_url` (`https://claude.ai/code/session_…`).
- Codex → task `url` from `codex cloud list --json`.

Add a nullable `cloud_session_url` (and vendor tag) to the Wave/RepoWork so
Concerto's per-repo view shows a "Open cloud loop" affordance. Concerto's
`loopflow://` handler (`ConcertoApp.swift:294`) is inbound-only; the back-link
is the reverse — Concerto calls `NSWorkspace.shared.open(vendor_url)` in a
browser. No new URL scheme needed; just persist and surface the vendor URL.

**Asana wiring rides along** via #3: the `.mcp.json` Asana entry is what makes
"reads its Asana roadmap" true in the cloud, and the goal prompt already
instructs read → dispatch → write-status-back. Round-trip closes because the MCP
server exposes the same list/create/update/complete surface `AsanaClient` wraps.

**Deep-link shape (existing, for reference).** Vendor IDE deep-links already
exist in `lf/commands/util.rs:75` — `claude://code/new?folder=…&q=…` and
`codex://threads/new?path=…&prompt=…`. Those open a *local* IDE session, not a
cloud loop, so the cloud command uses the CLI/web surfaces above rather than
these — but they're the precedent for prompt-carrying, cap-bounded handoff URLs.

## 4. Technical risks

- **Vendor-API instability.** Routines are "research preview"; `/fire` is behind
  a dated beta header the vendor may rotate; Codex cloud CLI + device-auth are
  beta and the REST task API is a wishlist item. Mitigation: A2 depends on the
  *stable* surfaces (committed prompt/skills/`.mcp.json`, human-pressed
  `/schedule`); confine every beta call (`/fire`, `codex cloud exec`) to the
  optional sliver, feature-flagged, easy to disable when a header rotates.
- **Lifecycle ownership.** With A2 the vendor owns start/stop/retry — we don't
  see failures unless we poll (Claude: only the web run list / `/fire` return;
  Codex: `codex cloud list`). "Close-the-loop" (feed run state back into
  re-measure) is harder across the cloud boundary than the local tmux path.
  Accept it for the first increment; the Wave shows the deep-link, the human
  inspects the vendor UI.
- **Auth.** Neither cloud path uses an API key — both are subscription/ChatGPT
  logins that act *as the user*. lfd holds no delegated credential and shouldn't
  try to; the human authenticates in the vendor once. The one credential lfd
  *does* mint is the Asana MCP token in `.mcp.json` — treat it as a secret
  (env-var reference, not inlined), since it's committed into a cloud-cloned
  repo. Prefer the environment's secret store (Routines env vars) over a literal
  token in `.mcp.json`.
- **Fresh-clone gap.** No `lf` binary in the cloud env means anything the goal
  prompt assumes about `lfq`/`lf op` breaks. For cloud, the goal must reach
  tools only via MCP/skills, or the setup script must install `lf`. First
  increment: MCP-only roadmap access, skills for flows, no `lf` dependency.

## 5. Build plan

**Increment 1 — reach the Done-when for one repo, Claude, A2.**
1. `.mcp.json` emitter: Asana MCP entry from `pm.asana_project`, secret by env
   reference. (The one net-new capability.)
2. `lf op cloud claude`: render goal via `render_goal`, ensure skills committed,
   write `.mcp.json`, print the `/schedule` seed + `claude.ai/code/routines`
   link. Human creates the Routine.
3. Persist `cloud_session_url` on the Wave; surface "Open cloud loop" in
   Concerto (`NSWorkspace.open`).
4. Verify: create the goals-wave Routine, confirm it runs machine-off, reads the
   Asana roadmap over MCP, and the Concerto link reopens the session.

**Follow-ons.**
- `lf op cloud codex`: `AGENTS.md` + `.mcp.json` + `codex cloud exec` handoff;
  document the client-side-Automations recurrence limitation.
- A1 sliver: `/fire` an existing Claude Routine from lfd (beta header, flagged)
  to nudge a run when a roadmap item lands.
- Close-the-loop: poll `codex cloud list` / routine runs, feed status into
  re-measure so the Wave reflects cloud progress.
