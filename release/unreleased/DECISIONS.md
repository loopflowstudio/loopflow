# Decisions

## 2026-06-30 — A chord is a wave with children; family terms replace "members"

**Context:** The `goals` redesign revisited the wave/chord data model. Chords once had their own `chords` + `chord_members` tables (migration 011), which were dropped (migration 028) and folded into the `waves` table via `parent_wave_id`, `wave_type`, and `position`. The structure was already self-referential, but the vocabulary still carried "member" from the abandoned table, and the chord concept read as a separate type rather than a kind of wave.

**Decision:** A **chord is just a wave with children** — `wave_type = chord`, children pointing back via `parent_wave_id`. No separate chord entity, no `chord_members`. The relationship vocabulary is **parent / child / sibling**, not "member." The wave-structure domain drops "member" entirely. (The VSM/govern "member" — `s2-scan` member backlogs, `s5-scan` member configs — is a different domain; rename later only if it bleeds into the wave structure.)

**Implications:** One self-referential `waves` table expresses both waves and chords. Code, docs, and APIs in the wave-structure domain say parent/child/sibling. This simplifies queries (a chord's contents = children where `parent_wave_id = id`) and removes the two-concepts (wave vs chord) split in favor of one type with a `wave_type` discriminator.

## 2026-06-30 — Goals: a looping prompt primitive that supersedes direction

**Context:** `direction` was removed as machinery on 2026-06-19 (wave model is `area × flow`; direction text redistributed into step-skills and AGENTS.md). The `goals` redesign introduces a persistent **Looping Agent** per wave that runs a **Goal** — a prompt run in a loop — steered by a live Asana roadmap, reusing existing steps/flows as inner work.

**Decision:** Add **goal** as a third prompt primitive alongside step (run once) and flow (steps composed). A goal is "a step that loops." It is the conceptual reincarnation of direction in its *looping, measurable* form, and the product developer's primary authoring surface ("I just want loopflow to be a good way to run my agents"). The bar: builtin steps/flows must be expressive enough to build **the clients and the servers** (mobile client, CLI client, server) from goals with **zero step authoring**. Backend priority follows the 2026-06-19 vendor-handoff thesis: codex/claude cloud sessions **first** (adapt to the developer's workflow), hosted lfd + Ghostty **second** (ours to own). Self-extension (goals authoring steps) is the rare, mostly-internal path — no permission cage.

**Implications:** New `goal/` primitive with the standard `.lf/` override model. The loop ticker (cold stateless runs) is replaced by a persistent Looping Agent reading the goal prompt each iteration. `pm.rs` inverts from down-mirror to live Asana read + write-back. Concerto surfaces per-repo looping sessions (launcher+link for cloud, dashboard for hosted lfd). Vocabulary gaps to close: greenfield scaffold, run-the-artifact, client↔server integration, platform-specific build/test. Tracked in `wave/goals/`.
