# Decisions

## 2026-06-30 — A chord is a wave with children; family terms replace "members"

**Context:** The `goals` redesign revisited the wave/chord data model. Chords once had their own `chords` + `chord_members` tables (migration 011), which were dropped (migration 028) and folded into the `waves` table via `parent_wave_id`, `wave_type`, and `position`. The structure was already self-referential, but the vocabulary still carried "member" from the abandoned table, and the chord concept read as a separate type rather than a kind of wave.

**Decision:** A **chord is just a wave with children** — `wave_type = chord`, children pointing back via `parent_wave_id`. No separate chord entity, no `chord_members`. The relationship vocabulary is **parent / child / sibling**, not "member." The wave-structure domain drops "member" entirely. (The VSM/govern "member" — `s2-scan` member backlogs, `s5-scan` member configs — is a different domain; rename later only if it bleeds into the wave structure.)

**Implications:** One self-referential `waves` table expresses both waves and chords. Code, docs, and APIs in the wave-structure domain say parent/child/sibling. This simplifies queries (a chord's contents = children where `parent_wave_id = id`) and removes the two-concepts (wave vs chord) split in favor of one type with a `wave_type` discriminator.

## 2026-06-30 — RLM is removed; goals are the subagent-running framework

**Context:** RLM (Recursive Language Model) was a map-reduce framework — split an oversized input into chunks, fan out cheap sub-agents, aggregate — shipped as a ~150-line `RLM.md` injected unconditionally into *every* prompt's system section, plus config knobs (`rlm_agent`, `rlm_max_parallel`, `rlm_max_depth`) and depth-guard env machinery (`RLM_DEPTH` auto-increment, `propagate_rlm_env`, `seed_rlm_env`). The `goals` redesign makes the looping Wave the orchestrator that dispatches flows as inner work — its own, narrower model for running sub-agents. RLM's always-on prompt tax and its capacity-oriented framing no longer earn their place. (Supersedes the 2026-06-24 line "`RLM` becomes the unconditional system section.")

**Decision:** Delete RLM entirely — the doc, the const, the `<lf:rlm>` system-section injection, the config fields, and the depth-guard env machinery. The goals operating prompt (the universal "you are a looping orchestrator, delegate don't implement" contract carried in the launch system layer) is the new home for subagent-running guidance. The runaway stop-condition becomes goals' own blocks→human, not `RLM_MAX_DEPTH`.

**Implications:** Every prompt drops ~150 lines of always-on map-reduce playbook. The map-reduce *technique* (chunk a huge input) is no longer documented in-prompt; if it's wanted back, it returns as an invokable step (`.lf/steps/`), not an unconditional injection. No recursion depth guard ships until the goals framework provides one — acceptable because the autonomous loop's stop-condition is human escalation by design.

## 2026-06-30 — Goals: a looping prompt primitive that supersedes direction

**Context:** `direction` was removed as machinery on 2026-06-19 (wave model is `area × flow`; direction text redistributed into step-skills and AGENTS.md). The `goals` redesign introduces a persistent **Looping Agent** per wave that runs a **Goal** — a prompt run in a loop — steered by a live Asana roadmap, reusing existing steps/flows as inner work.

**Decision:** Add **goal** as a third prompt primitive alongside step (run once) and flow (steps composed). A goal is "a step that loops." It is the conceptual reincarnation of direction in its *looping, measurable* form, and the product developer's primary authoring surface ("I just want loopflow to be a good way to run my agents"). The bar: builtin steps/flows must be expressive enough to build **the clients and the servers** (mobile client, CLI client, server) from goals with **zero step authoring**. Backend priority follows the 2026-06-19 vendor-handoff thesis: codex/claude cloud sessions **first** (adapt to the developer's workflow), hosted lfd + Ghostty **second** (ours to own). Self-extension (goals authoring steps) is the rare, mostly-internal path — no permission cage.

**Implications:** New `goal/` primitive with the standard `.lf/` override model. The loop ticker (cold stateless runs) is replaced by a persistent Looping Agent reading the goal prompt each iteration. `pm.rs` inverts from down-mirror to live Asana read + write-back. Concerto surfaces per-repo looping sessions (launcher+link for cloud, dashboard for hosted lfd). Vocabulary gaps to close: greenfield scaffold, run-the-artifact, client↔server integration, platform-specific build/test. Tracked in `wave/goals/`.
