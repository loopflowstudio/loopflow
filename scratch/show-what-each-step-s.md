# LOO-347: show what each step's context is made of

Status: draft, written 2026-10-01 for Jack Heart's request ("we should be doing a
lot more to understand and track our context usage"). Jack is not reviewing this
Task; decisions here are executive and recorded in `scratch/questions.md`.

## What exists

Every agent Session capture under `~/.lf/runs/<prefix>/<key>/` already holds:

- `context.json`: the exact submitted prompt split into attributed assets
  (instructions, memory, scratch, goal, documents, assembly glue) with cl100k
  token counts, plus inclusion/reduction decisions.
- `events.jsonl`: raw provider output and normalized conversation events. Tool
  results, per-request provider usage and Claude compaction boundaries are in it.

Two gaps explain LOO-298's hand reconstruction:

1. Steers are buried inside one "inherited launch goal" asset. Nothing records
   how many there were, who wrote them, or what they cost.
2. Nothing reads the record back by source. `lf usage` shows one input number.

## Design

No new store. The capture is the record; a reader derives the breakdown.

**Capture (one addition).** `ContextAssetKind::Steer`. Attribution claims the
`<lf:steers>` block of the launch message as its own asset. When the launch
knows the steers (Task seed), each one adds an `Included` decision carrying its
author, `steer:<id>` and token weight. Count and authors come from those.

**Reader.** `context_usage.rs` turns one capture into a `StepContext`: nine
sources, always present, tokens `null` when the record cannot say.

| Source | Measured from | Unit |
| --- | --- | --- |
| instructions | operating/surface/provider/repo/skill assets | cl100k |
| memory, scratch, goal, steers | matching assets | cl100k |
| other | documents, diff, summary, clipboard, user message, assembly glue | cl100k |
| carried | first provider request minus assembled input: resume history plus the provider's own preamble and tool definitions | provider tokens |
| tool_output | tool results returned to the main conversation | cl100k |
| compaction | observed compaction boundaries; tokens in context before each | provider tokens |

Also per step: assembled total, first and peak provider request, and named gaps.

**Budgets.** The existing `context_budgets` flag memory, scratch, goal and steers
(both against the goal budget) and the assembled total (input budget). Sources
without a configured budget show none. LOO-346 states the rest.

**Where it shows.**

- `lf mon show <session>`: a context section per step; `--context` prints only it
  (`--json` for the `StepContext`).
- `lf usage --context [--task X]`: one row per step plus Task totals;
  `--json` emits `ContextReport { steps, totals }`.
- Desktop: Task detail reads `lf usage --task X --context --json`.

## Remaining

- Run `lf usage --task LOO-298 --context` on an installed build and compare with
  the hand reconstruction. LOO-298's existing captures predate steer attribution,
  so its steers stay inside `goal` there.
- Confirm the compaction marker against a real compacted Claude step.
- Look at the Desktop Session history line.
- LOO-346 states budgets for instructions, carried, tool output and compaction.

## Checks

- After merging main and the compress pass: `cargo test -p loopflow --lib context_usage`, `--lib attributed_context`, `--test dto_fixtures context_report`, `cargo fmt`, `cargo clippy --all-targets -- -D warnings`: pass. Swift unchanged since its last pass. Affected suites: gate.
