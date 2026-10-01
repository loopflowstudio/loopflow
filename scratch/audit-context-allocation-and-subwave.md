# Context allocation and subwave memory composition (LOO-331)

Status: **reviewed direction approved, including labelled budget excerpts.** Jack Heart
approved the decisions below on September 30. This design was rewritten after
`lf sync --manual main` moved this scratch-only branch to `de074a2eb`, preserving
its notes. No production implementation has been made in this review.

Feedback and exact approval boundaries: [Child memory discovery](child-memory-discovery.md).
Remaining integration and evidence gaps: [questions](questions.md). Historical audit and limitations are
preserved below; they are not measurements of the current system.

## Intended experience and accepted decisions

A release conversation inherits infrastructure's memory and its own release
memory without requiring the registry to reconstruct file ancestry. Increasing
hierarchy depth must not increase automatic memory context without a bound.

Jack approved:

- Derive memory ancestry from `wave/<address>/`, root first and leaf last.
  More specific guidance takes precedence among memory scopes. Budget the
  combined chain, including the leaf, rather than granting each ancestor a
  separate allowance. Existing higher-priority instructions still govern.
- A memory-only scope is valid without a `GOAL.md` or registry row. Reading
  a scope does not create an operational Wave or assign Task/Session ownership.
- Discover child memories through ordinary filesystem tools. No child index
  is injected, and `lf` is not a required discovery interface.
- `realign` instructs the agent to read immediate-child memories before
  curating the selected parent. Explore deeper descendants when relevant.
  Promote broadly useful lessons into the parent; retain local detail in the
  child's memory. This is a skill procedure, not special prompt assembly.
- Remove known duplicate delivery: one `GOAL.md`, no redundant Objective
  excerpt, and no accidental repeat of operating instructions. Distinct memory
  scopes remain distinct even when their text matches.
- Retain main's scratch behavior and interface with the existing budgeting
  work. Do not add this draft's proposed scratch-indexing policy.
- Respect context budgets. Jack did not choose values or a budget shape and
  reported concern about 100k total context; this is not an agreed limit or
  evidence of a universal model-quality cutoff.
- Redesign against current AgentSession ownership. Do not restore Run records.

## Current system after syncing main

Source inspection at `de074a2eb`; no live acceptance claims.

| Concern | Current implementation | Consequence |
|---|---|---|
| Execution ownership | `agent_sessions` and immutable Session history own conversations and captured inputs; `execs` own actual processes; FlowSession owns progression. | Context evidence belongs to a specific captured Session input, not a generic Run. |
| Capture payload | `session_record.rs` retains `PreparedTurnContext` in `context.json` as a subordinate payload referenced by Session history. | The filename still exists; its presence does not restore Run ownership. Use the current capture path. |
| Context decisions | `trace.rs` already defines `Included`, `Excluded`, `Summarized`, `StatOnly`, `Truncated`, and `Deduplicated`. | Do not add a parallel omission schema. |
| Memory gathering | `work/wave/context.rs` still follows registry `parent_wave_id`, then renders a combined string; lookup failure falls back to the leaf. | Replace the memory chain lookup with address ancestry. Keep registry identity and execution authority separate. |
| Checkout selection | Ordinary memory gathering resolves the origin checkout; `gather_wave_memory_from` can read the execution checkout. | Preserve explicit content checkout selection and report actual source paths. Do not silently switch which version is read. |
| Wave documents | `engine/prompt.rs` gathers immediate Markdown files, excluding `MEMORY.md`. | Ancestor memory does not imply automatic ancestor docs or goals. |
| Prompt preparation | `engine/exec.rs::prepare_exec_prompt` gathers, removes native instruction duplicates, bounds components, renders, then checks total input. | Memory composition must pass through this existing budget path. |
| Reread instruction | The header already says to use supplied memory and read relevant omitted sections. | The approved header correction is already upstream; preserve it. |
| Readers | `lf runs` reads retained AgentSession input/provider history selected through SQLite. | Reuse this surface and exact captured-input identity; do not scan old manifest folders as the authoritative population. |

The merged budgeting change is `da31960c7` (#1362), implemented in
`engine/context_budget.rs` and documented in `docs/lf-reference.md`.

## Budget integration

Reuse one budget owner: `engine/context_budget.rs`. Current values are starting
values inherited from main, not newly approved thresholds or tuning results.

| Layer | Token ceiling (`cl100k_base`) | Byte ceiling |
|---|---:|---:|
| Combined ancestor and owned memory | 8,000 | 64 KiB |
| All scratch context | 16,000 | 128 KiB |
| Launch goal/message | 16,000 | 128 KiB |
| Rendered assembled input | 64,000 | 512 KiB |

Memory is bounded as a collection. Adding another ancestor never allocates
another 8,000 tokens. Count the rendered memory content, with scope labels and
any excerpt notice, under the existing memory ceiling; retain the total-input
check after rendering. Do not introduce per-Wave limits, another config system,
or a second budget engine in this work.

Main's current overflow behavior keeps a head-and-tail excerpt, inserts a
visible notice, and preserves the exact full gathered source at a content-hashed
path under `.lf/tmp/context/`. `ContextDecisionKind::Truncated` records the
reduction and original size. Remaining explicit content exceeding the total
limit causes a launch error before contacting the provider.

**Approved by Jack:** retain main's labelled-excerpt behavior for the memory
chain, preserving the complete source on disk and accurate scope attribution.
Reuse existing budget defaults; no new whole-file selection policy is needed.
The original draft's blanket prohibition on all truncation is withdrawn; the
accepted requirement forbids silent truncation.

A chain-wide excerpt can cut through a scope block or remove intermediate
scope headers. The implementation must preserve honest source attribution and
must not label all surviving text as leaf-owned. Preserve the complete gathered
source and use original source/range evidence when mapping an excerpt. Do not
claim an ancestor was fully supplied when only some of its text survived.

The 64k ceiling counts Loopflow's assembled system/task input. Native provider
instructions, tool schemas, existing conversation history, subsequent file
reads, and responses are outside it. Thus it does not guarantee a conversation
stays below 100k. Report observed provider context pressure separately where
available; unavailable measurements stay unknown. Do not substitute cumulative
input usage for current context occupancy. Curation instructions should ask for
relevant sections of large child files rather than automatically ingesting every
full archive after launch.

## Composition and discovery

For `infrastructure/release`, enumerate `infrastructure`, then
`infrastructure/release`. For deeper addresses, include each path prefix once.
No fixed depth limit; the combined memory budget remains fixed. Missing files
are explicit gaps in the selected chain; absent and unreadable are different
outcomes. A missing intermediate file does not prevent finding deeper memory.

Under budget, render complete memory bodies root first and leaf last, labelled
with address, source path, and inherited/owned relation. When over budget, apply
the agreed shared overflow policy. No sibling or child bodies are automatically
added. An unbound conversation gains no implicit Wave memory.

A memory-only scope is discoverable by its `MEMORY.md` path. Operational Wave
creation, explicit CLI Work selection and Task attribution remain Infrastructure's
responsibility. No registry row is required to read an ancestor or child file.

Proposed self-contained addition to builtin `realign`:

> Before curating the selected Wave's memory, inspect `wave/<address>/` for
> immediate child directories containing `MEMORY.md`. Read the child memories
> for lessons that apply across the parent scope; for large files, inspect
> headings and read relevant sections within the available context budget.
> Explore deeper descendants when the work calls for them. Promote broadly
> useful lessons into the selected parent's memory and keep child-specific
> detail in its owning file. State unread or unavailable coverage honestly.

Ordinary work follows the same file-location convention when child knowledge
is relevant. Teach the convention once in the existing memory guidance; do not
inject a generated directory listing into every prompt. Inherited memory guides
work; ordinary curation edits the selected scope, not its ancestors.

## Explain selection through existing Session evidence

The Session and its immutable captured input are the attribution boundary.
Preserve exact rendered channels through `PreparedTurnContext` and the existing
Session capture path. Use existing assets/decisions for source paths, labels,
hashes, token attribution, and reduction reasons. Add only information needed
to identify the selected chain and distinguish owned/inherited source content;
do not create a new record store or a replacement attempt object.

At gathering time, retain each selected memory address, actual source path,
relation and content until rendering/budgeting have attributed the final bytes.
The current single combined string labelled with the leaf path cannot explain
multiple source files by itself. An internal list of source blocks is sufficient;
keep it within the existing prompt/capture pipeline.

Missing expected ancestor files can use `Excluded` decisions with an honest
reason and unknown size/hash. Included hashes describe the actual included
bytes; full-source identity must not be confused with an excerpt hash. Preserve
existing `Truncated` and `Deduplicated` decisions. No enumeration of unrelated
siblings or all descendant candidates is required by this redesign.

Child reads during curation belong to tool/provider history, not launch assets.
A later file read cannot retroactively become automatic launch context. The
reader should distinguish full inclusion, excerpted inclusion, missing source,
and later reads; missing tool evidence must remain unavailable.

## Examples

| Work | Automatic memory | Additional reading |
|---|---|---|
| Release scope, combined chain below budget | Infrastructure, then release, both complete | Children only when relevant |
| Release scope, combined chain above budget | Shared memory budget applies; visible reduction and source access required | Relevant omitted sections |
| Infrastructure scope | Infrastructure memory within the same budget; no child list | Discover release via the filesystem when relevant |
| Infrastructure curation | Same ordinary memory composition plus `realign` instructions | Immediate-child memories, with deeper exploration when useful |
| No selected Wave scope | No automatic Wave memory | Explicitly requested files only |

## Remaining implementation and boundaries

1. Replace registry-derived context ancestry with path-derived source blocks,
   preserving caller-selected checkout and clear missing/unreadable outcomes.
2. Feed the complete chain through existing memory and total-input budgets.
   Retain labelled excerpts; preserve address attribution
   through any reduction. Exercise paths where memory is already embedded in
   a Wave seed so it cannot bypass the shared memory allowance.
3. Keep existing Session capture ownership. Attribute source blocks and explain
   reductions using the existing decision model and retained input reader.
4. Add child-reading/curation instructions to `realign` and the compact memory
   location convention to existing guidance. No automatic child union.
5. Verify known duplicate delivery against current assembly; remove remaining
   duplicate source delivery. The reread header is already fixed upstream.

LOO-329 owns durable Wave identity, creation, relocation, and selecting an
operational subwave for work. LOO-330 owns address/navigation presentation.
LOO-331 owns memory composition, budget integration, and explaining actual
context through Session evidence. The earlier Product agreement to an injected
child index is superseded by Jack's review. Related designs have not been
rechecked after this sync; do not claim current cross-design alignment.

Excluded: new memory storage, relevance search, memory-only scopes gaining
execution authority, per-skill loading infrastructure, new scratch allocation,
provider resume injection, and changing the current budget values without
measurement. No implementation has begun in this review.

## Proof and evaluation

- A focused prepared-prompt proof covers nested memory-only ancestors with no
  registry, ordering, ownership, absent/unreadable files, and no sibling/child
  injection. Address and file source must match actual rendered bytes.
- A deep/oversized chain stays within the aggregate memory and total-input
  ceilings, including labels and notices, while the complete source remains
  accessible. Confirm embedded-memory launch paths cannot evade the limit.
- Session-capture evidence explains full, excerpted and missing memory using
  the existing reader path and immutable captured-input identity.
- Duplicate proof checks source delivery: a goal excerpt and its full file
  overlap despite having different hashes. Two scopes with identical memory
  text retain their distinct meaning. Reuse the current native-doc dedup path.
- Observe parent work finding a relevant release lesson via ordinary tools,
  and curation promoting an appropriate lesson. Record misses and unread scope;
  instruction presence alone does not establish behavior.

Use matched skill/provider/Flow cohorts from current Session history. Measure
launch tokens by source, aggregate memory before/after reduction, omitted reads,
provider context pressure when available, steering and delivered outcome. Report
cost only where observed. Do not reuse the old four-day population as a current
baseline or infer quality from token reductions. The historical audit scripts
need adaptation before they can select current Session-owned inputs.

Review findings addressed: the old plan proposed an obsolete evidence owner,
duplicated an existing budget mechanism, and imposed an unnecessary child index.
The redesign removes all three. A remaining risk is losing scope labels inside
chain-wide excerpts; acceptance above requires truthful attribution.

Check: `git diff --check` passed before sync; sync completed without conflicts;
current redesign is documentation only, with source inspection rather than a
new behavioral test run.

## Historical evidence limits

The following audit was produced on September 28 against source `632b67ec7`
and the old Home-local Run population. It is retained as dated evidence only.
Budgeting and Session ownership changed afterward. Counts have not been rerun.
The operating-instructions table labels 530 as Runs within a 525-Run population;
that denominator needs reconciliation before reuse. The script's repository
prefix filter also needs exact repository-identity validation. Reported
correlation does not establish that turn count causes the observed consumption.

## Historical source audit — September 28

Observations from source at `632b67ec7`. These describe code, not behavior.

| Mechanism | Where | What it does |
|---|---|---|
| Wave docs | `engine/prompt.rs::gather_wave_docs` | Reads `wave/<name>/*.md`, immediate files only, README first, excluding `MEMORY.md`. No ancestors, no descendants. |
| Wave memory | `work/wave/context.rs::gather_wave_memory_from` | Resolves a chain through registry `parent_wave_id`, renders root first and leaf last, labelled `inherited from` / `owned by`. |
| Chain fallback | same, `unwrap_or_else(\|\| vec![wave])` | When the registry is unavailable or has no row, the chain is the leaf alone. Ancestors drop without a record. |
| Memory path | `work/wave/memory.rs::Memory::for_wave` | `wave/<name>/MEMORY.md`. A slash-qualified name maps to a nested directory. |
| Wave discovery | `ops/pm.rs::collect_local_waves` | Recursive, but only directories holding `GOAL.md`. |
| Address | `work/wave/mod.rs::WaveLocator` | Accepts `a/b`; rejects empty, `.`, `..`, backslash. |
| Scratch | `gather_scratch_docs` → `gather_md_files_from` | Every `.md` under `scratch/`, recursive, whole. |
| Wave header | `format_content_sections` | Names one path, `wave/<leaf>/MEMORY.md`, and says "Read it before every iteration". |
| Repo guide | `AGENT_NATIVE_FILES` | `CLAUDE.md` / `AGENTS.md` are skipped by assembly and left to the harness. |

Two separate hierarchies exist. The registry's `parent_wave_id` drives memory
inheritance. Directory nesting drives discovery and file location. Nothing
ties them together: nesting `release/` under `infrastructure/` confers no
inheritance, and a registry parent link confers no file location.

Descendant aggregation does not exist anywhere. No code path reads a child's
memory into a parent's prompt.

Launch paths that gather memory: `lf/commands/run.rs` (direct skill and inline
prompt), `ops/run.rs::render_wave_context` (Wave resident), `bin/lf-prompt.rs`.
Native resume bypasses assembly, as Wave memory already records.

## Historical measurements — September 24–28

Population: every Run record in this Home,
`~/.lf-dev/installed/local-afee63d7…/runs`. 600 manifests, all with
`context.json`, all `coverage: assembled`, created 2026-09-24 23:51 UTC to
2026-09-28 19:21 UTC. 525 belong to the loopflow repository; 75 belong to
`etude` and `kata` and are excluded below. Token counts are Loopflow's own
`cl100k_base` accounting, not provider-billed tokens.

Scripts: `scratch/loo331-audit/*.py`. They read records and write nothing to
the Home.

### Subwave and memory inclusion

| Question | Result |
|---|---|
| Registry Waves with a parent | 0 of 6 |
| Prompts containing `inherited from` / `owned by` memory headers | 0 of 525 |
| Context assets sourced under `wave/infrastructure/release/` | 0 |
| Runs with a slash-qualified Wave subject | 0 |
| Runs with a Wave subject that lack memory or goal | 0 of 344 |
| Runs with no Wave subject | 181 of 525 |

Ancestor inheritance has never executed on this Home in the window. Its only
evidence is the unit test `child_memory_walks_parent_scope`.

Release work, the motivating case:

| Run | Skill | Subjects | Memory tokens |
|---|---|---|---|
| `run_982ab7c8` and five more `release-notes` Runs, 09-25 to 09-27 | release-notes | none | 0 |
| `run_33f3e809`, worktree `loopflow.release-24`, 09-28 18:28 | 5whys | none | 0 |
| 271 infrastructure Runs | various | `wave:infrastructure` | 9,989 to 18,421, release file absent |

`wave/infrastructure/release/MEMORY.md` entered git at 19:09 UTC on
2026-09-28 in `632b67ec7`. Only two Runs touch that path by command, and both
are the LOO-330 and LOO-331 kickoffs reading it for this design. Which Run
authored the file is not established by these records.

### Token allocation by source

525 Runs, 28.6M accounted tokens.

| Source | Share | Runs | Average when present |
|---|---|---|---|
| Scratch files | 67.0% | 402 | 2,957 per file, 8 files median, 99 max |
| Wave memory | 14.5% | 346 | 11,620 |
| Task and step message (`goal`, step/task scope) | 8.6% | 390 | 7,586 |
| Operating instructions | 3.0% | 530 | 1,722 |
| Assembly framing | 2.4% | — | 78 per segment |
| Explicit docs | 1.9% | — | 897 |
| Skill instructions | 1.5% | 488 | 967 |
| Everything else | 1.1% | — | — |

Per Run: median 40,648, p90 126,079, max 199,841. 113 Runs exceed 100k.
Scratch is more than half the prompt in 244 Runs and more than 80% in 101.

By Wave, per Run: product 105,039 (scratch 84,679), infrastructure 68,465
(scratch 43,062), growth 14,630, no Wave 32,964.

One worktree explains much of the tail. `loopflow.data-model-one-table-per`
carries `scratch/from-loo291/`, seven imported files of 5k to 10k tokens each,
delivered to 101 Runs. That is about 5.4M tokens for a handoff imported once.

### Duplicate material

| Duplicate | Runs | Tokens |
|---|---|---|
| `GOAL.md` twice: Objective section as `goal`, whole file as `document` | 346 of 346 Wave Runs | ~89k total, 258 per Run |
| `LOOPFLOW.md` twice, system and task channel | 18 | 20,340 |
| Memory included, then read again by command | 254 of 346 (heuristic) | not measured |
| Included scratch file read again by command | 224 of 402 (heuristic) | not measured |

The re-read counts match a command that names the file alongside `cat`,
`sed -n`, `head`, `nl`, `rg`, `grep` or `wc`. They over-count a read that
precedes an edit and under-count reads through a provider file tool. The
header's instruction to "Read it before every iteration" is a plausible cause.
That is a hypothesis.

### Omission is unrecorded

10,374 inclusion decisions across 600 Runs. Every one is `included`. The
record cannot express a component that was eligible and left out, so the
release file's absence appears nowhere. It was found by looking for it.

### Provider side

| Measure | Codex (355 Runs) | Claude-labelled (56 Runs) |
|---|---|---|
| First usage observation minus accounted prompt | p50 22,493, p90 22,626 | p50 22,525, p90 39,546 |
| Accounted share of first observation | p50 65% | p50 68% |
| Cumulative input over accounted prompt | p50 31×, p90 227× | p50 64×, p90 143× |
| Peak input over context window | p50 43%, p90 83%, max 95% | p50 66%, p90 92%, max 95% |

About 22.5k input tokens per Run are outside Loopflow's record: harness system
prompt, tool schemas, and the repo guide the harness loads itself. `CLAUDE.md`
is 24,365 bytes. The first observation can span more than one request, so
this is an upper bound on the unattributed prefix.

Correlation between accounted prompt size and cumulative input is 0.34 across
411 Runs. Prompt size explains little of total consumption; turn count
dominates.

### Missing measurements

- **Cost.** `cost_usd` is null in the records sampled. No dollar effect is
  stated.
- **Behavior.** No record links a context component to an outcome. Nothing
  here shows that less scratch or more memory changes quality.
- **Outcome.** 86 of 525 Runs have no usage observation. Terminal receipts
  were not joined in this audit.
- **Window.** Four days, one Home. Exactly 600 records; no pruning code was
  found in `run_record.rs`, and the count is unexplained.
- **Harness label.** Some Runs labelled `claude` report a 258,400 token window
  and Codex-style commands. The harness split above may be mislabelled.
- **Slash-qualified launch.** What `--wave infrastructure/release` does today
  is read from source, not run. No dry-run assembly command exists.
