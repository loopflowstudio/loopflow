# Context allocation and subwave memory composition (LOO-331)

Status: approved implementation, narrowed by Jack Heart's September 30 steer.
Original review: [Child memory discovery](child-memory-discovery.md).
Historical audit and its limitations remain below; they are not current measurements.

## Accepted scope and ownership

Jack approved implementation and waived another review. LOO-331 now owns:

- `realign` reading immediate-child memories before curating a selected parent,
  promoting shared lessons and retaining local detail in the child.
- One delivery of `GOAL.md`, no redundant Objective excerpt, and no repeated
  operating instructions. Equal memory text in different scopes remains distinct.

LOO-354 / PR #1375 owns address-derived ancestry and memory-only scopes. Jack
identified repository `MEMORY.md` as the root of that chain. LOO-356 owns the
combined memory budget and excerpt attribution. LOO-362 owns the planned 16,000
memory-token default and gradual curation. LOO-329 owns operational identity;
LOO-330 owns address/navigation presentation. These boundaries supersede the
broader implementation list from the original review. No ancestry, budget,
registry, or Session storage changes belong in this patch.

On September 30, `gh pr view 1375 --json state,mergeCommit` reported OPEN with
no merge commit. `lf sync --manual origin/main` fetched main at `de074a2eb`;
Loopflow selected its scratch-only reset strategy and restored all design/audit
files. No production changes existed to merge. The scoped implementation remains
independent of PR #1375. No current cross-design agreement beyond Jack's explicit
ownership split is claimed.

## Implementation

`realign` discovers immediate-child `MEMORY.md` files using ordinary filesystem
tools, including memory-only directories. It reads relevant sections of large
files within the available context budget, explores deeper descendants when
relevant, and states unread or unavailable coverage. Shared lessons move into the
selected parent; local details stay in the child. Inherited memory guides work
without changing the ordinary curation target. The existing operating guidance
teaches the nested file convention without injecting a directory index.

Wave binding leaves the authored goal body to the gathered document instead of
also embedding it in the launch seed. Builtin goals without an authored file
retain their existing seed path. IDE skill launches with Wave documents use
the assembled prompt because short native-skill seeds carry no documents.
Main already removed the Objective excerpt;
prepared-prompt tests preserve that behavior.

The existing native-document deduplication pass now also removes repeated
sources across automatic docs, explicit docs, and full changed-file context.
Canonical path plus exact contents identifies a repeated source; equal contents
at different paths survive. A `LOOPFLOW.md` document equal to the builtin text
is omitted only when the system channel supplies operating guidance. Customized
instructions and explicitly requested guidance with operate disabled survive.
Diff patches remain evidence of changes, including deliberate quoted duplicates.

Existing `ContextDecisionKind::Deduplicated` records retain source paths, reasons,
and original sizes through `PreparedTurnContext`. The generated Wave header no
longer claims to be sourced from `GOAL.md`; its full document owns that path.
No new schema, record store, or content-hash deduplication policy is introduced.

## Delete — do not maintain

- Removed authored-goal seed copies, `drop_native_instruction_docs` and its
  removed-document list, and the misleading GOAL source path on generated framing.
  Preparation, capture, and `lf-prompt` use `drop_duplicate_docs` and existing
  context decisions. Builtin-goal fallback, flow lists, memory, metrics and
  executive instructions remain.
- Removed `dedup_documents`: file gathering already deduplicates requested paths.
  The shared assembly filter handles duplicates across document sources.
- Do not rebuild the already-removed Objective excerpt, retired Run ownership,
  automatic child index, or per-skill memory loading infrastructure.

## Proof and remaining work

Focused prepared-prompt tests cover direct and Wave launches, complete goal
content once, captured source/decision attribution, native and symlink duplicate
sources across automatic scratch and explicit docs, equal text in distinct memory
files, customized guidance, and guidance with operate disabled. A focused assembly
test also covers a document requested as changed-file context and its deduplication
decision. The IDE launch test exercises `build_prompt_at` with native skill
execution enabled and checks the submitted goal and captured source. Its isolated
home prevents a regression from exporting skills into personal directories.
Golden fixtures carry the authored `realign` procedure.

CI owns completion of the broader Rust suite: gate's host-security monitor stopped
it after 1,103 passes, terminating four tests and leaving 922 unrun (17 skipped).
No assertion failure preceded that stop. The runner reported sustained
`syspolicyd` pressure at 323% CPU; this is an unavailable full-suite result, not a
product pass. Architecture, formatting, Clippy and website checks passed.
Demo/review owns observing
an agent discover a relevant child lesson and curate a useful parent entry; prompt
presence does not prove that behavior. A current Session-history allocation audit,
provider pressure, omitted reads and delivered-outcome measurements remain unrun.
The old scripts still read retired Run records and cannot establish current results.

Review: source inspection found that the generated Wave header was attributed to
GOAL.md although it contains no file bytes; removing that attribution lets the
full document explain its actual source. The same review found that the IDE
short-seed path would lose the now document-owned goal; Wave-document launches
therefore retain the assembled prompt. Deduplication is limited to source identity
and the known operating document so identical memory scopes retain meaning.
Compression review found that deduplicated changed-file context was classified
as a generic document. The shared filter now retains its Diff classification;
no extra deduplication pass or record type is needed.
Gate review added the missing IDE launch proof and removed trailing whitespace
from the preserved audit scripts; it found no production repair necessary.

Check: `uv run python scripts/test.py --base de074a2ebd2cb54e6f7dde799400dae36c87bbbe --reuse-passing` passed architecture, fmt, Clippy and website (78 passed, 3 skipped); Rust stopped for host-security pressure after 1,103 passes, full completion deferred to CI. Final test isolation verified with `cargo test -p loopflow --lib ide_wave_skill_launch_delivers_the_authored_goal`, `cargo fmt --all -- --check` and `cargo clippy --all-targets --jobs 4 -- -D warnings`; `cargo test -p loopflow --test golden_prompt` and `git diff --check de074a2eb` passed.

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
