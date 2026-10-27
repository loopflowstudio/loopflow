# Docs redo brief (LOO-610)

Jack Heart decided on 2026-10-26 that LOO-610 rewrites the README and `docs/` around
the reorganized `lf` surface. The README draft is `scratch/README.draft.md`; read it
first. It is the voice and the entry point every page continues from.

## Rules

- **Staged, not all at once (Jack).** Each page opens from what the reader already did
  (README steps: 1 Launch with `lf :`, a Skill, a Graph; 2 Work: hand off as a Task;
  3 Sessions: `lf resume`) and adds one idea at a time. Reference detail goes at the
  bottom of the page or is left to `docs/lf-reference.md`.
- **Every example visibly does something only lf does.** A bare command that shows
  nothing (`lf resume` alone) doesn't earn a code block.
- **Claim only what exists on this branch.** The authority is `docs/lf-reference.md`
  in this checkout (generated from the compiled CLI) plus the code. Not yet existing,
  so never claim: `--watch` on reads (LOO-604), `lf session new`, resume takeover by
  default, `--fork` (LOO-618), `lf usage --by` grouping, per-step machines or
  per-step interactivity (LOO-622). If unsure, check the reference or the code.
- **Command tree on this branch:** `lf ops` holds `pr release ci tokens commit sync wt
  machine account` (with `machine pool|ondemand`). There is no `lf repo` and no
  `wave cron`. Use the shortest unique form in examples: `lf land`, `lf commit`,
  `lf account`, `lf machine list`, `lf release run`. Scheduling is the person's own
  cron/launchd running an ordinary lf command.
- **Voice (AGENTS.md "User-Facing Documentation" and "README.md" sections):** examples
  first, terse prose, direct and imperative, benefit first and mechanism as proof, no
  throat-clearing, tables for reference not teaching. Name the reader "you".
- Preserve wording Jack wrote when the surface it describes is unchanged; when you
  must change it, keep it and add an HTML comment `<!-- changed from: … -->`.
- Write only your assigned files under `scratch/docs-draft/`. Do not edit `docs/`,
  `README.md` or code: another agent is changing the code in this checkout.
- Each page ends with a one-line "Next:" pointing to the next stage page.

## Stage order and pages

getting-started → work → sessions → ops → surfaces → telemetry → orchestration (Waves
live in work.md under `## Waves`), plus troubleshooting.

| Page | Covers | Source docs to mine (in `docs/`) |
|---|---|---|
| getting-started.md | install → `lf :` → a Skill → a Task → resume → land; the first hour (LOO-509: match the released product) | getting-started.md, index.md |
| work.md | Tasks, comments/steers, Task status and list, Waves (goal, memory, plan, Projects/KRs), planning providers (Git ref default, Linear) | planning.md, waves.md, Task/Wave parts of lf.md |
| sessions.md | resume (`--all`, `--task`, provider ID), steer, rename/bind, accounts routing as it affects launches, what survives a crash | conducting.md (inspect/resume), Session parts of lf.md, subscriptions.md (routing basics) |
| ops.md | publish/open/submit/land/abandon, CI repair, release, commit/sync/wt, machines (add/use/pool/ondemand), accounts (connect/use/route) | lf.md "Publish and finish" etc., subscriptions.md, machine parts of authoring.md/lf.md |
| surfaces.md | `lf open` per host, Desktop and `lf desktop` pane commands, JSON for your own UI | conducting.md (Mac app), agent-api.md (observe), `lf desktop` in the reference |
| telemetry.md | `lf history`, `history show`, `history usage` (`--task/--wave/--weekly/--context`), `lf ops tokens`, `self doctor`, live processes | conducting.md (health/usage) |
| orchestration.md | Skills, Graphs, Workflows, customizing builtins, `-a` per step, `--machine`, Goals | authoring.md, Graph parts of lf.md |
| troubleshooting.md | what survives, what went wrong, what to run next (LOO-404) | troubleshooting.md |
