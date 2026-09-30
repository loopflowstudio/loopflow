# Complete CLI catalog · LOO-338

Jack Heart requested this catalog before implementation on 2026-09-30. Baseline: `a6b1bc3df`. The compiled Clap tree is authoritative; historical docs supplied design evidence only. All baseline rows have a concept verdict; integration gaps are explicit. Jack Heart’s latest steer removes the demo wait and delegates monitoring choices. Later destination decisions below supersede earlier behavior-preserving rename assumptions. The target is not a claim of implementation.

## Counts and reproduction

- Before: **141 commands below the root**, 18 hidden; 142 rows including the root. **440 flag entries** (including automatic help/version), **95 positional arguments**, **10 extra aliases** (one command alias, nine flag aliases).
- Current integrated slice: **127 commands below root**, 16 hidden; **424 flag entries**, **84 positionals**, **zero aliases**. [Exact integrated surface](cli-integrated-surface.md). Model additions and remaining catalog disagreements prevent a final-completion claim.
- `cargo run -p loopflow --example cli_catalog > scratch/cli-catalog-before.json` — passed against the unchanged parser.
- `target/debug/lf help --all` in a disposable Home — exit 0, 125 lines, no Home state created; public help omits hidden commands and options by design.
- Raw [Clap metadata](cli-catalog-before.json) and [public help](cli-help-before.txt) accompany this catalog. Stable C/A identifiers below link research to rows.

## Evidence rules

Caller citations distinguish executable/agent references, documented public use, and tests. A source reference is not proof of live use. Option rows cite their declaration and the owning command's callers; that does not assert those callers pass every optional flag. No repository match does not prove no external user exists. Retain a public option only when it changes a distinct real behavior; delete proven no-ops and duplicate aliases. Dynamic external definitions are not enumerable commands: root/skill/flow fallthrough and the bare default are accounted for below.

Keep means retain the real operation at its owner; it does not waive the required overviews, first-result path, truthful JSON/errors or child readiness. R01–R10 in [fresh research](cli-research-20260930.md) add these cross-cutting acceptance requirements. The integrated monitoring decisions below supersede earlier open demo choices. Remaining ownership contradictions stay explicit in the working design.

## Destination audit · 2026-09-30

Jack Heart stopped owner moves and requested a concept cull against ongoing work.
This section supersedes earlier blanket keep rationales, fixed `mon` alias,
`worktree` spelling and the assumption that renaming preserves every operation.
Each row below now names its concept, distinct input, destination work and decision
status. Callers prove migration obligations, never that a command earns retention.
Targets marked open are proposals for Jack's demo, not permission to cut evidence.

### Sources read and what changed

Read the current Run's `manifest.json` **runtime_path** and used that installed
executable with its inherited owning Home for `roadmap --json` and
`roadmap --all --json` (both exit 0). `$LF_BIN`'s first roadmap read returned an
incomplete different-store view; it did not prove missing Tasks. The owning
runtime resolved LOO-298/334/339/340 and the cross-Wave roster. No source build
opened an installed store, no auth was repaired and no planning was changed.
`gh pr list --state open --limit 100 --json number,title,headRefName,body,url`
returned seven open PRs: #1296, #1299, #1300, #1313, #1318, #1354 and #1356.
These are dated observations, not a claim that their implementations are shipped.

Read all five repository Wave memories (Infrastructure, Release, Product,
Intelligence and Growth), plus the named sibling plans below. Paths identify
read-only observations of independently changing checkouts; no sibling was edited.

| Work / inspected source | Destination affecting the catalog | Rows / consequence |
|---|---|---|
| LOO-298 / #1296, `loopflow.data-model-one-table-per/scratch/{data-model-one-table-per,exec-per-step,naming}.md`; parser and `commands/exec.rs`; HEAD `f02565d8d` plus uncommitted naming work | Exec = actual process; AgentSession = conversation; FlowSession = saved graph/cursor; every step has an Exec; loop passes are lenses. Finite commands replace lfd, residents and builtin Wave chat. | C011 connect; C081/C093/C094/C126/C127 delete; C089/C096/C098/C111 fold into ordinary Flow execution; C132/C133 keep saved FlowSession readers; C135–137 delete in-turn navigation; C090/C091 repository chapter ownership. New `__flow-step`, Session history/bind and Exec inventory must enter the integrated Clap extraction, not a guessed Run rename. |
| LOO-334 / #1354, `loopflow.resolve-tasks-from-linear-and/scratch/{resolve-tasks-from-linear-and,task-command-equivalence}.md`, HEAD `3b4cd6592`; current Task directive | Task existence follows Linear/Git, not presence in one execution DB. Repository connection and normalized acquisition; explicit Work declaration, ordinary Flow execution, official child executable and preserved saved work. | C079 authored Wave discovery; C080 scoped portfolio; C082/C083 repository connection/sync; C085/C088 remove registry lifecycle writers. C100 must inspect a Task without starting it. Root `--as` survives; duplicate root `--task`/`--wave` fold into it. |
| LOO-339, current roadmap marks completed; LOO-340 open; later read of `loopflow.keep-account-status-live-and/scratch/keep-account-status-live-and.md` | Observed email/subject, mismatch and duplicate rejection, staged reconnect. Live status default with explicit cached mode; one machine account store, per-provider launch accounts (no individual step overrides), banked reset observation and explicit redemption. | C044–51 reuse those owners. Bare Account absorbs duplicate Status, but its cached default must change with LOO-340. `--verify` becomes default behavior, not a second live toggle. Preference and restriction stay distinct. Reset redemption is a new explicit mutation owned by LOO-340, never automatic. |
| LOO-332, current Task and `loopflow.scheduled-task-operation/scratch/task-automation.md` | Finite repository ticks, land returns after handoff, no persistent PR watcher. | C033 merges into C034 land; do not add `--no-wait` to preserve two lifecycle paths. Cron placement must reconcile repository automation with Wave cadence. Completion still requires merged evidence. |
| LOO-333, current Task and retained `scheduled-task-operation/scratch/recursive-vsm.md` (its registered checkout has no current scratch plan) | Ordinary finite Wave/repository thinking; independent Task progress; Discord separate. | Delete resident/chat surfaces, not replace them with a new supervisor or messaging namespace. Skills/Flows own this behavior. |
| LOO-329 / #1318 and LOO-330, sibling subwave designs; LOO-331 context-allocation design | Slash-qualified authored Waves and ancestor files, no memory service. | C079 list local responsibilities and empty goals; C084/C087 one rename operation, no stopped-resident relocation. Identity-through-rename differs between sibling drafts: remains their integration question. No `memory` command. |
| LOO-303 / #1300 workspace design; LOO-327 / #1313 prototype and Product memory | Saved FlowSessions differ from authored templates; passive navigation never starts work. Native file browser requires exact base/revision, changed paths and draft patch. | C132/C133 retain saved inventory/detail; template list/help merges must not remove these. C101 merges into C102 files projection only with native DTO/revision preservation. Prototype PR is older than Product's native implementation memory; use current consumers as proof. |
| LOO-336 current Task; Intelligence memory | Prospective usage attribution remains; post-hoc is a future read-time decision. Exact provider conclusion, missing usage and failures before provider launch remain evidence. | C113 spending has a question Exec process history cannot answer. Do not turn absent cost into zero or assign all historical usage to a newly bound Task. C124 events/final comparison below. |
| LOO-309 / #1299 sibling docs design; Growth memory; Jack's new steering | Direct product docs for capable readers; commands as typed, not repeated owner prefixes. | Shortest unique commands in guides/skills; canonical reference/help; CI ambiguity test. `wt` retained. `top` and `ps` explicitly retained. `mon` comes from unique-prefix resolution, never a registered alias. |

The LOO-334 scratch design also discusses local planning ownership in SQLite;
Jack's newer instruction says the DB is execution record, not Task existence.
This catalog follows Jack's instruction and does not ratify the older local-store
proposal. Similarly LOO-303's copied Run model and LOO-330's rename identity text
are older than newer LOO-298/329 direction. Their user outcomes survive; copied
schema/naming does not override current owners. No cross-Home registry, credential
copy or resident is introduced to hide these integration questions.

### `runs` versus `ps` / `top`: measured surface, open demo choices

Measurement is **source/output-schema inspection**, not a live provider trial.
On this branch `ActivitySnapshot` has four fields: schema version, observation
time, `nodes`, and `provider_processes`.
A node has 10 fields (id, parent, kind, label, repo, worktree, Wave, PID, start,
state). Provider processes have eight OS/claim fields. `ps` takes one snapshot;
`top` refreshes the same snapshot every two seconds on a TTY and delegates to
`ps` for redirected/JSON output. Jack explicitly keeps both. Neither includes
provider usage, completed command outcomes, conversation history or saved Flow
progress. Shared observations do not justify deleting either chosen interface.

The current `runs` history reads seven days and caps at 50 records; `--parent`
reads all direct children without that cap. `usage` shares that history scanner
and JSON projection but defaults to 30 days with no 50-row cap and adds input,
output, cache-read, cost, finality and gap columns. This is real duplication in
today's implementation. LOO-298 changes the owners: Exec list includes mechanical
commands and their outcomes; usage is AgentSession evidence. Preserve those
questions without retaining a duplicate Run object or pretending costs belong
to every Exec.

| Baseline view / rows | Information absent from ps/top | Proposed keep-or-cut for Jack at demo |
|---|---|---|
| Bare history C124 | Finished command/provider outcome, captured identity, cost and past work; ps/top are current OS observations | **Keep the question, cut Run history.** Use Monitor's Exec inventory from LOO-298 and Session history for provider outcomes. No second Exec list alongside Monitor list. Final target/filters await integrated extraction. |
| `--active` A470 | Exact provider/conversation linkage, Task selection, discovery coverage and gaps independent of the history window | **Candidate fold into ps** once its Session join exposes these fields. Otherwise retain active conversation view. Unknown/unavailable must survive; process counts alone are insufficient. |
| `--watch` A471 | NDJSON active-conversation updates and stdin-close lifetime; top's redirected JSON is one snapshot | **Candidate cut separate watch** only if ps or Session inventory owns equivalent stream/lifetime. TTY refresh is not a substitute for NDJSON. |
| Positional ID A472 | One captured execution's manifest/context/provider output after process exit | **Keep monitor show** accepting real Exec or Session IDs. Resolve collisions explicitly; no manufactured Run DTO. |
| `--parent` A473 | All direct child executions, including completed children outside recent limits | **Keep on Exec history**, reusing LOO-298 indexed parent/paging; traversing live ps cannot answer it. Complete paging must replace the uncapped answer, not silently truncate it. |
| `--events` A474 | Append-only conversation/operation evidence, including failures and unknown attribution | **Keep evidence, likely cut this flag** in favor of Session history and Exec/FlowSession detail. Preserve original raw artifact access where normalized history is incomplete. |
| `--final` A475 | Durable provider final, fallback completion, or explicitly inexact streamed prose | **Keep this question on Session inspection**; ps/top have no equivalent. Whether this is a flag on show or history is open. No provider final on mechanical Execs. |
| `--resume` A476 | Native conversation continuation; an effect, not monitoring | **Merge into session connect**, preserving stable Session and account/ownership restrictions. Never resume an Exec. |
| `--task` A477 | Performed-work filter including historical/terminal Task attribution | **Keep filter in Monitor history and conversation inventory**, backed by LOO-298/334 identity. OS cwd is insufficient. |
| `--project` A478 | Historical chapter/Project attribution | **Candidate cut public Project selector**, preserve history through Wave/chapter and stable evidence. Prove no lost cross-chapter query before removal. |
| `--wave` A479 | Work attribution independent of current process tree | **Keep Wave filter** using Session/FlowSession work and historical provenance, not a stored resident name. |
| `--json` / help A480–481 | Serialization and discovery, not another user object | Apply the same protocols to surviving commands. Delete obsolete Run shape; migrate Swift and fixtures together. |

The above is an **open keep-or-cut list requested by Jack**, not final approval
of active/watch/event/filter deletion. C124 records the selected merge into
Monitor; its argument rows retain this explicit decision boundary.

### Other cull decisions and integration conditions

- Fold single-reader duplicates: user name → user; PR status → bare PR; account
  status → bare account; route show → bare route. Keep mutation children distinct.
- Merge changed-file inventory into diff's files projection (C101), Skill list
  into catalog (C140), and Flow template validation into typed help (C134).
  Saved FlowSession list/show survive; authored definitions remain in list/help.
- Mode flags become one mode choice (interactive/batch/tui/ide). Changed-context
  booleans become one files/patch/both/none choice; browser on/off becomes one
  optional value. Omission still inherits configuration. Remove redundant root
  Work selectors in favor of `--as`; query-local filters remain.
- LOO-340’s newer sibling plan removes individual step overrides: capture one
  per-provider selection for the whole launch and all children. Account preference
  allows fallback; only-account restricts spending. Keep both
  meanings through LOO-340's provider bundles and remote/background children.
- Doctor diagnoses consistency; prune mutates only precisely owned processes;
  tokens measures Git source content. None duplicates a process snapshot or
  provider-consumption report. Prune's placement remains a demo choice, not
  justification for deleting exact signal checks.
- Historical install preflight/local-preflight, candidate advancement and switch
  recovery have different executable/store authority. Keep that separation;
  remove daemon artifact options with LOO-298. Hidden screenshot/provider/review
  callbacks retain real child obligations. The lease probe is unresolved until
  destination readiness demonstrably replaces its check.
- Task run and Task worker cannot remain a separate executor. Preserve the
  `--as task:X flow` destination; the convenience spelling itself is a demo
  choice. Historical Wave recovery is removed only after LOO-298 preservation.
- Keep established `wt`, `ps`, `top`, `pr`, `ci`, `cron`, `ssh`. No expansion to
  worktree/processes is justified. Selected long renames for demo are auth →
  account (Jack's decision), session open → connect (LOO-298), stop-run →
  stop-exec (retired model), and the owner/group moves. Other synonyms are not
  added. Unique-prefix `mon` is a navigation rule change that still needs proof.

### Counts and finish boundary

Actual compiled baseline → current checkpoint `23c588154`: **141 → 139 commands
below root; 440 → 436 flags; 95 → 92 positionals; 10 → 0 extra aliases**. Repo
moves change no counts. These remain the only implemented before/after counts.

Revised baseline verdicts: **48 keep, 66 rename, 17 merge, 11 delete command
rows** (142 including root); **397 keep, 1 rename, 90 merge, 47 delete argument
rows** (535). Argument deletion includes automatic help on removed commands;
merging an operation carries inputs and is not 90 independent removed flags.
The renderer lists every merge/deletion below. These are dispositions, **not a
forecast of the final Clap size**: LOO-298 and LOO-340 add real Session/Exec/account
inputs absent from this baseline. Re-extract their integrated command trees and
catalog the additions before claiming a complete final surface or demo readiness.

Before further owner moves: resolve integration against the named work, preserve
the open demo choices, and make the accepted cull real in vertical consumer cuts.
The next independently authorized correction is `wt` plus shortest unique docs
and the CI resolver test. No more prefix-only cut is justified by this audit.

## Integrated Account cut · 2026-09-30

LOO-340 `32b6eef00` is integrated through local `lf rebase --manual`, retaining
LOO-339 identity validation and the Account rename. Its two committed slices
add no commands: live observation replaces `--verify` with `--cached`; Codex
browser ownership changes its existing connect implementation. Later shared-store,
per-provider child selection and reset-credit additions are not yet integrated.

C045 and C051 are implemented: bare Account owns status, and bare account route
owns route inspection. Existing provider/details/JSON and route-scope inputs move
to these readers. The current Clap extraction measures **139 → 137 commands**,
**436 → 433 flags**, **92 → 92 positionals**, with zero aliases. Two removed help
flags and the duplicate overview/status JSON option account for the flag decrease.
From the original baseline: **141 → 137**, **440 → 433**, **95 → 92**, **10 → 0**.
`--cached` is the newly named input absent from the baseline; it explicitly avoids
provider/broker contact and writes. No other newly integrated CLI inputs exist.

Public CLI proofs preserve identity rejection, current/expired/missing capacity,
read-only routing and absent-Home cached reads. Removed leaves and --verify reject
before effects. Provider/browser proof is synthetic. Full execution-model and
planning integration, all remaining verdicts and Jack Heart's demo remain open;
this is not the final compiled surface. Detailed commands and integration conflicts
are recorded in the existing working design's Account integration slice.

## Task changed-file merge · 2026-09-30

C101 is implemented as `task diff --files`, with Desktop, reference documentation
and public CLI tests moved together. The Changes parser leaf is deleted. Files
mode retains TaskChangesSnapshot; patch mode retains TaskDiffSnapshot and draft
comparison. No wire field, file revision or storage owner changes. Path and draft
inputs select patches and conflict with files mode before effects.

Compiled counts: **137 → 136 commands below root**, **433 → 431 flags**,
**92 → 91 positionals**, **zero aliases** (17 hidden commands unchanged).
From baseline: **141 → 136**, **440 → 431**, **95 → 91**, **10 → 0**.
The new `diff --files` input replaces the Changes leaf's help, base and JSON
flag entries; its issue positional is shared with Diff. This is an implemented
concept merge, not completion of the remaining catalog or destination work.

## Skill catalog merge · 2026-09-30

C140 is implemented: `list skill [namespace] [--json]` owns skill discovery.
The Skill List parser leaf and duplicate inspection dispatch are deleted;
emitted namespace invocations and current reference examples use List. Existing
catalog Entry fields and ordering remain unchanged. Public CLI proof compares
scoped entries to the same skills in the mixed catalog and checks text, nested
namespace membership, reserved definitions and absence of runtime writes.

Retiring the verb frees `list` as an explicit skill name: absent definitions fail
with exit 2 and `skill not found: list`; authored list skills remain selectable,
including through the delimiter. No rejection shim, alias or provider launch is
added. Flow inventory/inspection and the dependent execution model are unchanged.

Compiled counts: **136 → 135 commands below root**, **431 → 429 flags**,
**91 → 90 positionals**, **zero aliases**, with 17 hidden commands. The removed
entries are List's duplicate JSON/help flags and namespace positional. Baseline
totals are now **141 → 135**, **440 → 429**, **95 → 90**, **10 → 0**.
These are intermediate counts; Monitor, dependent integrations and full acceptance
remain open. LOO-334 `9fea0552f` still records its failed production-store
continuation proof; no unfinished model was imported for this independent cut.

## Post-rebase decisions · 2026-09-30

Jack Heart's latest steer removes the demo/review wait and delegates remaining
keep-or-cut decisions. The supervisor owns shipment after implementation and
focused proof; this catalog does not claim that finish line yet.

Rebased with `lf rebase --manual` onto main `3dc89bc9a` (#1360), ending at
`7164a0d81`. Removed nine service command rows and their arguments from the
active verdict table: webhook namespace/serve/register, Wave probe/recover,
chat-connect/resident callbacks, reply and chat. Removed the four daemon artifact
options as well. Original rows remain in the baseline JSON and Git history;
none is a pending rename or obligation to recreate a retired service.

C002, C028 and C134 now merge into Home User, bare Task PR and typed Flow help.
The first preserves Git/config name precedence and optional JSON; the second
preserves missing-PR and actual-PR output on ordinary branches; the third expands
the Flow and rejects missing definitions or invalid review IDs before printing.
The retired `validate` verb is available as an authored Flow name. No alias or
rejection shim reserves it. Desktop and reference callers migrate with User.

Compiled 135 → 123 commands, 429 → 393 flags, 90 → 82 positionals. Main accounts
for nine commands, 29 flags and seven positionals; this cut accounts for three
commands, three help flags, four redundant toggles and one positional.
`--diff files|patch|both|none` selects the full changed-context combination;
`--chrome on|off` selects browser capability. Omission inherits configuration. No model integration or
Monitor implementation is hidden in those counts.

### Selected monitoring destinations

These decisions replace the older open-demo table; they do not claim that the
Exec/Session implementation is integrated here.

- Keep ps and top with their existing snapshot/TTY behavior under Monitor.
- Keep one Exec history list under Monitor, including complete parent queries
  and Work filters. Delete the Run-history command when that owner is integrated.
- Keep active conversation discovery and its NDJSON watch under Monitor active.
  OS presence cannot replace Session linkage, coverage gaps or stdin-close lifetime.
- Keep Monitor show for real Exec or Session IDs; Session inspection owns final
  conclusions and raw events. Mechanical Execs have no provider final.
- Cut resume from monitoring; Session connect owns continuation. Replay remains
  a distinct explicit spend of captured inputs. Keep usage and Work activity:
  neither spending nor planning changes can be inferred from the process tree.
- Keep historical Project filtering until the repository chapter reader offers
  the equivalent complete query. Removing it now would lose an observed query.
- Keep explicit event/final projections on Session detail until normalized
  history proves equivalent coverage; do not silently discard failures or unknowns.

Dependency refresh: LOO-298 is now `25548d567`, with implemented current-state
cutover and updated no-archive policy. Its `remaining-work.md` still names
integrated checks, Chapters and configured acceptance. LOO-334 is `1fd99a284`,
whose revised plan still requires common Flow and planning integration. Earlier
copied-Flow failures remain historical evidence, not proof that either current
branch is unchanged or permanently unavailable. Main here still has RunSnapshot
and no Exec inventory. Dependent Monitor implementation must integrate the actual
owners, not rename RunSnapshot. Remaining independent option and owner cuts also
remain required. The catalog and final CLI do **not** yet match.

## Integrated monitoring decisions · 2026-09-30

Jack Heart's latest steer delegates the former demo choices. The local model
integration is LOO-298 `25548d567`, followed by this branch's cull. Main's removed
service rows stay removed. The first Task-base update failed; the subsequent main rebase completed locally
at f0a2a57c6 and resolved lineage. Full catalog agreement remains unfinished.

- **Merge history inventories.** `monitor list` is the single Exec inventory,
  preserving paging, parent, caller, work, search and outcome filters. Delete
  both root Exec and Runs parser owners; no Run DTO is created or relabeled.
- **Keep active discovery and watch.** Exact Session/provider linkage, coverage,
  gaps and stdin-bounded NDJSON differ from ps/top. Native Desktop now consumes
  `monitor active`; ps/top remain independently chosen interfaces.
- **Keep Session detail, final and raw events.** `monitor show` resolves Exec or
  Session identity. An optional input must belong to that Session. Exec outcome
  is not a provider conclusion. Resume belongs only to Session connect.
- **Keep usage and its historical filters.** Accounting has its own time window,
  missingness and attribution. Native Task history consumes that reader. Parent
  selection preserves all matching captured inputs without a date cutoff; Exec
  parent paging answers a separate process ancestry question. Public Project
  filtering stays until the promised cross-chapter replacement exists.
- **Keep exact cleanup.** Process prune belongs to Monitor; worktree prune is
  distinct. Bare prune became ambiguous and the documentation check rejected it.
  Guides now use `lf mon prune`. No signal-ownership check was removed.
- **Name the actual callback.** C017 becomes hidden `session stop-client`, not
  stop-exec: it signals the native client owned by a retained input, not an lf
  Exec. The remote caller moved in the same cut.
- **Consolidate launch choice.** `--mode` replaces interactive/batch/tui/ide;
  `--as` replaces root Task/Wave selectors. Query-local filters remain distinct.
  Home operations, Ask and Cron now have their catalog owners. SSH's remote
  target boundary was explicitly repaired after the move.

Remaining disagreements are not accepted deletions: managed Task continuation
and independent attributed Flow capture differ in this model. Catalog C096,
C098 and C111 require owner reconciliation before their predecessor can go.
Planning discovery and LOO-332 finite landing are likewise not implemented by
these name changes. Final catalog agreement and live first-result/readiness
proof remain open; do not ship solely from these intermediate counts.

## Commands

P:LINE refers to the [baseline Clap declarations](https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/mod.rs). E references are caller/source evidence. N references expand in the rationale section. Option caller references inherit the named command row; they do not claim every optional input is passed by that caller.

| Row | Canonical path | Target owner | Concept owned | Callers / source | Rationale | Verdict → target | Follows / decision |
|---|---|---|---|---|---|---|---|
| C000 | lf | root | Default direct provider conversation and command entry point | dispatch: [E000]; P:14 | N000 | keep → lf | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C001 | lf user | lf home | Configured participant display name | agent instructions: [E001]; documented public use: [E002]; documented public use: [E003]; P:225 | N001 | rename → lf home user | LOO-338; Jack Heart 2026-09-30; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C002 | lf user name | lf home user | Participant display name | Desktop: [E004]; runtime/source reference: [E005]; agent instructions: [E001]; P:216 | N002 | merge into → lf home user | LOO-338; Jack Heart 2026-09-30; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C003 | lf : | lf | Execution of caller-authored prompt text | documented public use: [E002]; documented public use: [E006]; documented public use: [E007]; P:231 | N003 | keep → lf : | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C004 | lf desktop | lf home | Desktop application activation | agent instructions: [E008]; documented public use: [E002]; P:236 | N004 | rename → lf home desktop | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C005 | lf screenshot | lf home | Raster capture of a page with a bounded browser lifetime | agent instructions: [E009]; agent instructions: [E010]; documented public use: [E011]; P:238 | N005 | rename → lf home screenshot | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C006 | lf __screenshot-supervisor (hidden) | lf | Capture-child lifetime after its owning caller disappears | runtime/source reference: [E012]; documented public use: [E011]; documented public use: [E013]; P:244 | N006 | keep → lf __screenshot-supervisor | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C007 | lf __provider-session (hidden) | lf | Attribution of a provider-native conversation to its execution | runtime callback: [E014]; P:250 | N007 | keep → lf __provider-session | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C008 | lf ask | lf session | Synchronous request for a new durable review conversation | agent instructions: [E015]; agent instructions: [E016]; agent instructions: [E017]; P:252 | N008 | rename → lf session ask | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C009 | lf session | lf | Conversation discovery and lifecycle namespace | Desktop: [E018]; runtime/source reference: [E019]; runtime/source reference: [E020]; P:257 | N009 | keep → lf session | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C010 | lf session list | lf session | Resumable conversations and pending review obligations | Desktop: [E021]; Desktop: [E022]; agent instructions: [E023]; P:728 | N010 | keep → lf session list | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C011 | lf session open | lf session | Continuation of a durable AgentSession | Desktop: [E024]; Desktop: [E025]; runtime/source reference: [E020]; P:736 | N011 | rename → lf session connect | LOO-298; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C012 | lf session complete | lf session | Completion of a conversation's review obligation | Desktop: [E026]; Desktop: [E027]; runtime/source reference: [E028]; P:748 | N012 | keep → lf session complete | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C013 | lf session rename | lf session | Conversation title with explicit authorship precedence | Desktop: [E029]; Desktop: [E018]; agent instructions: [E030]; P:750 | N013 | keep → lf session rename | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C014 | lf session ready | lf session | Author readiness for a pending review without completing it | runtime/source reference: [E019]; runtime/source reference: [E031]; runtime/source reference: [E032]; P:761 | N014 | keep → lf session ready | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C015 | lf session serve-flow (hidden) | lf session | Execution of one captured Flow review boundary | runtime child: [E033]; P:767 | N015 | keep → lf session serve-flow | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C016 | lf session serve-ask (hidden) | lf session | Execution of one captured ad-hoc review request | runtime child: [E034]; P:777 | N016 | keep → lf session serve-ask | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C017 | lf session stop-run (hidden) | lf session | Stop the precisely owned native client for a retained Session input | runtime child: [E035]; P:780 | N017 | rename → lf session stop-client | LOO-338 catalog and primary-source research; implemented; selected autonomously under Jack Heart’s latest steer |
| C018 | lf install | lf home | Selection and activation of a verified machine installation | script: [E036]; runtime/source reference: [E037]; runtime/source reference: [E038]; P:262 | N018 | rename → lf home install | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C019 | lf install schedule | lf home install | Automatic activation of published software on a machine schedule | runtime/source reference: [E039]; documented public use: [E040]; test: [E041]; P:1148 | N019 | rename → lf home install schedule | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C020 | lf install recover-switch (hidden) | lf home install | Recovery of an interrupted installation selection transaction | runtime child: [E042]; P:1155 | N020 | rename → lf home install recover-switch | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C021 | lf install preflight (hidden) | lf home install | Published-candidate compatibility with the selected installation store | script: [E043]; runtime/source reference: [E044]; runtime/source reference: [E045]; P:1165 | N021 | rename → lf home install preflight | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C022 | lf install local-preflight (hidden) | lf home install | Development-candidate compatibility with an explicitly retained private store | runtime/source reference: [E046]; test: [E047]; test: [E048]; P:1172 | N022 | rename → lf home install local-preflight | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C023 | lf install advance-switch (hidden) | lf home install | Receipt-authorized schema advancement by the pinned candidate | runtime child: [E049]; P:1180 | N023 | rename → lf home install advance-switch | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C024 | lf install promote (hidden) | lf home install | Atomic activation of a specific candidate's CLI, app and store | script: [E050]; runtime/source reference: [E051]; runtime/source reference: [E052]; P:1189 | N024 | rename → lf home install promote | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C025 | lf install rollback (hidden) | lf home install | Restoration of retained installed bytes compatible with the existing store | No literal caller found; usage unestablished; P:1230 | N025 | rename → lf home install rollback | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C026 | lf pr | lf task | Current branch's pull request and its delivery lifecycle | Desktop: [E053]; runtime/source reference: [E054]; runtime/source reference: [E055]; P:267 | N026 | rename → lf task pr | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C027 | lf pr checks | lf task pr | Required check outcomes and failure logs for one pull request | test: [E056]; test: [E057]; test: [E058]; P:1249 | N027 | rename → lf task pr checks | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C028 | lf pr status | lf task pr | Current pull request summary | No literal caller found; usage unestablished; P:1257 | N028 | merge into → lf task pr | LOO-338; Jack Heart 2026-09-30; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C029 | lf pr next | lf task pr | Continuation of a Task's serial PR chain after a settled merge | runtime/source reference: [E054]; test: [E059]; P:1260 | N029 | rename → lf task pr next | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C030 | lf pr publish | lf task pr | Publication of a ready review without merge intent | Desktop: [E060]; agent instructions: [E061]; agent instructions: [E062]; P:1267 | N030 | rename → lf task pr publish | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C031 | lf pr open | lf task pr | Draft preparation and presentation for browser review | Desktop: [E053]; runtime/source reference: [E055]; runtime/source reference: [E063]; P:1277 | N031 | rename → lf task pr open | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C032 | lf pr submit | lf task pr | Preparation for a reviewer's future merge decision | agent instructions: [E064]; agent instructions: [E065]; agent instructions: [E066]; P:1287 | N032 | rename → lf task pr submit | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C033 | lf pr arm | lf task pr | Authorized delivery handoff | runtime/source reference: [E067]; agent instructions: [E068]; agent instructions: [E069]; P:1306 | N033 | merge into → lf task pr land | LOO-332; adopted target; implementation not implied |
| C034 | lf pr land | lf task pr | Authorized delivery handoff | runtime/source reference: [E070]; runtime/source reference: [E071]; runtime/source reference: [E072]; P:1325 | N034 | rename → lf task pr land | LOO-332; adopted target; implementation not implied |
| C035 | lf pr abandon | lf task pr | Discarding branch and PR artifacts without deleting the planning Task | runtime/source reference: [E054]; agent instructions: [E073]; test: [E074]; P:1344 | N035 | rename → lf task pr abandon | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C036 | lf wt | lf task | Physical checkout lifecycle namespace | runtime/source reference: [E075]; runtime/source reference: [E076]; runtime/source reference: [E077]; P:272 | N036 | rename → lf task wt | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C037 | lf wt create | lf task wt | Allocation of an untracked sibling checkout | runtime/source reference: [E077]; test: [E078]; P:1638 | N037 | rename → lf task wt create | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C038 | lf wt switch | lf task wt | Selection of an existing physical checkout | runtime/source reference: [E075]; test: [E079]; P:1646 | N038 | rename → lf task wt switch | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C039 | lf wt list | lf task wt | Inventory of physical checkouts and their branch facts | runtime/source reference: [E076]; agent instructions: [E080]; documented public use: [E081]; P:1651 | N039 | rename → lf task wt list | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C040 | lf wt prune | lf task wt | Cleanup of eligible inactive or terminal physical checkouts | documented public use: [E082]; test: [E083]; P:1662 | N040 | rename → lf task wt prune | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C041 | lf wt remove | lf task wt | Removal of one explicitly selected physical checkout | documented public use: [E084]; test: [E085]; P:1669 | N041 | rename → lf task wt remove | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C042 | lf rebase | lf task | Integration of the current checkout with its selected Git base | runtime/source reference: [E086]; runtime/source reference: [E087]; runtime/source reference: [E088]; P:277 | N042 | rename → lf task rebase | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C043 | lf commit | lf task | Local Git checkpoint of current changes | agent instructions: [E089]; agent instructions: [E090]; agent instructions: [E091]; P:297 | N043 | rename → lf task commit | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C044 | lf auth | lf | Provider access, identity and capacity | script: [E092]; runtime/source reference: [E093]; runtime/source reference: [E094]; P:306 | N044 | rename → lf account | LOO-339; LOO-340; adopted target; implementation not implied |
| C045 | lf auth status | lf account | Provider access, identity and capacity | script: [E092]; script: [E095]; script: [E096]; P:1493 | N045 | merge into → lf account | LOO-339; LOO-340; implemented in Account integration slice; public CLI fixture proof; full Task incomplete |
| C046 | lf auth disconnect | lf account | Revocation of local or managed provider credentials | documented public use: [E097]; P:1503 | N046 | rename → lf account disconnect | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C047 | lf auth connect | lf account | Acquisition of local or managed provider credentials | runtime/source reference: [E093]; runtime/source reference: [E094]; runtime/source reference: [E098]; P:1508 | N047 | rename → lf account connect | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C048 | lf auth set | lf account | Account metadata and remembered browser venues | script: [E099]; documented public use: [E100]; test: [E101]; P:1521 | N048 | rename → lf account set | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C049 | lf auth route | lf account | Explanation of ordered account selection for a routing scope | script: [E102]; runtime/source reference: [E103]; agent instructions: [E104]; P:1545 | N049 | rename → lf account route | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C050 | lf auth route set | lf account route | Replacement of the ordered accounts in a routing scope | script: [E102]; runtime/source reference: [E103]; documented public use: [E105]; P:1554 | N050 | rename → lf account route set | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C051 | lf auth route show | lf account route | Configured account routing | script: [E106]; agent instructions: [E104]; documented public use: [E107]; P:1564 | N051 | merge into → lf account route | LOO-340; implemented in Account integration slice; public CLI fixture proof; full Task incomplete |
| C052 | lf release | lf repo | Repository release lifecycle namespace | script: [E108]; script: [E109]; script: [E110]; P:311 | N052 | rename → lf repo release | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C053 | lf release run | lf repo release | Recovery or completion of a verified repository release | script: [E108]; script: [E111]; runtime/source reference: [E112]; P:1577 | N053 | rename → lf repo release run | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C054 | lf release check | lf repo release | Unreleased changes eligible for the next repository release | runtime/source reference: [E113]; documented public use: [E114]; P:1584 | N054 | rename → lf repo release check | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C055 | lf release notes | lf repo release | Authored release narrative and its archival destination | No literal caller found; usage unestablished; P:1589 | N055 | rename → lf repo release notes | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C056 | lf release bump | lf repo release | Version values in repository manifests | No literal caller found; usage unestablished; P:1601 | N056 | rename → lf repo release bump | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C057 | lf release tag | lf repo release | Published Git version tag | No literal caller found; usage unestablished; P:1608 | N057 | rename → lf repo release tag | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C058 | lf release publish | lf repo release | Hosted release draft, artifacts and publication state | script: [E115]; test: [E116]; P:1615 | N058 | rename → lf repo release publish | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C059 | lf release status | lf repo release | Observed release workflow and hosted publication status | No literal caller found; usage unestablished; P:1629 | N059 | rename → lf repo release status | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C060 | lf repo | lf | Repository administration namespace | runtime/source reference: [E117]; runtime/source reference: [E118]; documented public use: [E119]; P:316 | N060 | keep → lf repo | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C061 | lf repo reteam | lf repo | Reconciliation of the repository's planning Team across Waves | runtime/source reference: [E117]; runtime/source reference: [E118]; documented public use: [E120]; P:1444 | N061 | keep → lf repo reteam | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C065 | lf home | lf | Machine placement and installation namespace | script: [E121]; runtime/source reference: [E122]; agent instructions: [E123]; P:321 | N062 | keep → lf home | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C066 | lf home id | lf home | Stable execution-destination identity of this machine | script: [E124]; runtime/source reference: [E125]; agent instructions: [E123]; P:1477 | N063 | keep → lf home id | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C067 | lf home observe | lf home | Observed network route to a known execution destination | script: [E126]; runtime/source reference: [E127]; agent instructions: [E128]; P:1482 | N064 | keep → lf home observe | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C068 | lf sync-skills (hidden) | lf home | Export of authored skills into provider-native skill directories | documented public use: [E129]; P:327 | N065 | rename → lf home sync-skills | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C069 | lf cron | lf wave | Wave schedule declaration and execution namespace | script: [E130]; runtime/source reference: [E131]; runtime/source reference: [E132]; P:336 | N066 | rename → lf wave cron | LOO-332; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C070 | lf cron add | lf wave cron | Installation of one manually declared scheduled invocation | test: [E133]; P:1355 | N067 | rename → lf wave cron add | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C071 | lf cron list | lf wave cron | Installed schedule inventory | script: [E134]; test: [E135]; test: [E136]; P:1367 | N068 | rename → lf wave cron list | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C072 | lf cron preflight | lf wave cron | Read-only feasibility of declared Wave schedules on their owning Home | script: [E130]; test: [E137]; test: [E138]; P:1376 | N069 | rename → lf wave cron preflight | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C073 | lf cron sync | lf wave cron | Reconciliation of installed jobs with authored Wave schedule declarations | script: [E139]; runtime/source reference: [E132]; test: [E140]; P:1382 | N070 | rename → lf wave cron sync | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C074 | lf cron run (hidden) | lf wave cron | Receipt-bearing execution of one scheduled invocation | test: [E141]; P:1389 | N071 | rename → lf wave cron run | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C075 | lf cron history | lf wave cron | Observed scheduled invocation outcomes | script: [E142]; runtime/source reference: [E131]; runtime/source reference: [E143]; P:1401 | N072 | rename → lf wave cron history | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C076 | lf cron trigger | lf wave cron | Explicit firing of an installed scheduled invocation | script: [E144]; test: [E145]; test: [E146]; P:1416 | N073 | rename → lf wave cron trigger | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C077 | lf cron remove | lf wave cron | Removal of an installed scheduled invocation | test: [E147]; P:1431 | N074 | rename → lf wave cron remove | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C078 | lf wave | lf | Repository-authored responsibility and planning namespace | Desktop: [E148]; Desktop: [E149]; Desktop: [E150]; P:341 | N075 | keep → lf wave | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C079 | lf wave list | lf wave | Repository-authored responsibilities | Desktop: [E148]; Desktop: [E149]; Desktop: [E151]; P:824 | N076 | keep → lf wave list | LOO-334; LOO-329; LOO-330; adopted target; implementation not implied |
| C080 | lf wave status | lf wave | Wave-scoped planning and work evidence | Desktop: [E150]; Desktop: [E151]; Desktop: [E152]; P:838 | N077 | merge into → lf roadmap --wave WAVE | LOO-298; LOO-334; adopted target; implementation not implied |
| C082 | lf wave connect | lf wave | Repository planning connection | runtime/source reference: [E153]; agent instructions: [E154]; agent instructions: [E155]; P:867 | N078 | merge into → lf repo connect | LOO-334; adopted target; implementation not implied |
| C083 | lf wave sync | lf wave | Refresh of shared planning observations | Desktop: [E156]; runtime/source reference: [E157]; runtime/source reference: [E158]; P:884 | N079 | rename → lf repo sync | LOO-334; adopted target; implementation not implied |
| C084 | lf wave rename | lf wave | Rename of an authored responsibility and its provider mapping | test: [E159]; P:892 | N080 | keep → lf wave rename | LOO-329; LOO-334; demo/integration choice open; see destination audit |
| C085 | lf wave forget | lf wave | Retired empty Wave registration cleanup | documented public use: [E160]; test: [E161]; P:898 | N081 | delete → — | LOO-334; adopted target; implementation not implied |
| C086 | lf wave place | lf wave | Execution placement of a durable responsibility | agent instructions: [E162]; agent instructions: [E163]; documented public use: [E164]; P:906 | N082 | keep → lf wave place | LOO-332; LOO-334; demo/integration choice open; see destination audit |
| C087 | lf wave relocate | lf wave | Rename of an authored responsibility and its provider mapping | runtime/source reference: [E165]; documented public use: [E166]; documented public use: [E167]; P:913 | N083 | merge into → lf wave rename | LOO-298; LOO-329; LOO-334; adopted target; implementation not implied |
| C088 | lf wave retire | lf wave | Retired resident Wave lifecycle | documented public use: [E168]; P:923 | N084 | delete → — | LOO-298; LOO-334; adopted target; implementation not implied |
| C090 | lf wave new-chapter | lf wave | Repository-wide chapter boundary | runtime/source reference: [E169]; runtime/source reference: [E170]; runtime/source reference: [E171]; P:941 | N085 | rename → lf repo new-chapter | LOO-298; adopted target; implementation not implied |
| C091 | lf wave history | lf wave | Recorded repository chapter boundaries | Desktop: [E172]; documented public use: [E173]; documented public use: [E174]; P:955 | N086 | rename → lf repo history | LOO-298; adopted target; implementation not implied |
| C092 | lf wave update-plan | lf wave | Content of the current chapter plan | agent instructions: [E175]; agent instructions: [E176]; agent instructions: [E177]; P:962 | N087 | keep → lf wave update-plan | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C095 | lf task | lf | Concrete work and its delivery namespace | Desktop: [E178]; Desktop: [E179]; Desktop: [E180]; P:365 | N088 | keep → lf task | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C096 | lf task __worker (hidden) | lf task | Captured FlowSession driver admission | documented public use: [E181]; documented public use: [E182]; P:974 | N089 | merge into → lf flow resume | LOO-298; LOO-334; adopted target; implementation not implied |
| C097 | lf task checkout | lf task | Allocation or recovery of a tracked Task checkout without execution | Desktop: [E183]; agent instructions: [E184]; agent instructions: [E185]; P:976 | N090 | keep → lf task checkout | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C098 | lf task run | lf task | Execution of a Task-selected Flow | Desktop: [E186]; Desktop: [E179]; runtime/source reference: [E187]; P:989 | N091 | merge into → lf --as task:TASK flow | LOO-298; Jack Heart 2026-09-30; demo/integration choice open; see destination audit |
| C099 | lf task create | lf task | Creation of a planning Task with optional execution | Desktop: [E188]; runtime/source reference: [E189]; runtime/source reference: [E190]; P:1008 | N092 | keep → lf task create | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C100 | lf task status | lf task | One Task's disposition, execution evidence and available actions | runtime/source reference: [E191]; runtime/source reference: [E192]; runtime/source reference: [E193]; P:1033 | N093 | keep → lf task status | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C101 | lf task changes | lf task | Checkout comparison against a selected base | Desktop: [E194]; documented public use: [E195]; test: [E196]; P:1040 | N094 | merge into → lf task diff --files | LOO-327; LOO-338; implemented in Task diff files slice; public CLI and Desktop consumer proof; full Task incomplete |
| C102 | lf task diff | lf task | Inspection of checkout changes against a Task's base | Desktop: [E197]; documented public use: [E198]; test: [E199]; P:1048 | N095 | keep → lf task diff | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C103 | lf task file | lf task | Revision-bearing content of one file and its recovery versions | Desktop: [E200]; Desktop: [E180]; documented public use: [E201]; P:1060 | N096 | keep → lf task file | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C104 | lf task save | lf task | Conditional replacement of file content with retained recovery bytes | Desktop: [E202]; documented public use: [E203]; P:1070 | N097 | keep → lf task save | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C105 | lf task complete | lf task | Successful disposition of a Task after delivery settlement | documented public use: [E204]; documented public use: [E205]; test: [E206]; P:1079 | N098 | keep → lf task complete | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C106 | lf task delete | lf task | Deletion of the shared planning issue with retained execution history | runtime/source reference: [E207]; agent instructions: [E208]; documented public use: [E209]; P:1087 | N099 | keep → lf task delete | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C107 | lf task edit | lf task | Current planning title and description of a Task | Desktop: [E210]; documented public use: [E211]; test: [E212]; P:1089 | N100 | keep → lf task edit | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C108 | lf task comment | lf task | Append-only direction and discussion for a Task | Desktop: [E178]; Desktop: [E213]; Desktop: [E214]; P:1099 | N101 | keep → lf task comment | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C109 | lf task interrupt | lf task | Interruption of a Task's current provider turn | Desktop: [E215]; documented public use: [E216]; documented public use: [E217]; P:1108 | N102 | keep → lf task interrupt | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C110 | lf task wait | lf task | Waiting for an observable Task condition | documented public use: [E218]; documented public use: [E219]; documented public use: [E220]; P:1114 | N103 | keep → lf task wait | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C111 | lf task restart | lf task | Explicit replacement of captured Flow execution | Desktop: [E221]; runtime/source reference: [E222]; agent instructions: [E223]; P:1125 | N104 | merge into → lf flow resume --restart | LOO-298; adopted target; implementation not implied |
| C112 | lf tokens | lf repo | Tracked source size and history measured in lines and tokens | implementation boundary: [E224]; Desktop: [E225]; Desktop: [E226]; Desktop: [E227]; P:370 | N105 | rename → lf repo tokens | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C113 | lf usage | lf monitor | Provider-authored consumption attributed to work | runtime/source reference: [E228]; runtime/source reference: [E229]; documented public use: [E230]; P:379 | N106 | keep → lf monitor usage | LOO-298; LOO-336; adopted target; implementation not implied |
| C114 | lf __telemetry-scorecard (hidden) | lf | Repository maintainer metric evaluation from recorded evidence | runtime/source reference: [E231]; test: [E232]; test: [E233]; P:398 | N107 | keep → lf __telemetry-scorecard | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C115 | lf ci | lf repo | Home-wide history of failed-CI response and settlement | runtime/source reference: [E234]; runtime/source reference: [E193]; documented public use: [E235]; P:404 | N108 | rename → lf repo ci | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C116 | lf ps | lf monitor | One current process observation | script: [E236]; runtime/source reference: [E237]; agent instructions: [E238]; P:419 | N109 | rename → lf monitor ps | Jack Heart 2026-09-30; LOO-298; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C117 | lf top | lf monitor | Continuous process monitoring | runtime/source reference: [E237]; agent instructions: [E239]; documented public use: [E240]; P:425 | N110 | rename → lf monitor top | Jack Heart 2026-09-30; LOO-298; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C118 | lf prune | lf monitor | Cleanup of proven owned orphan processes and stale process receipts | runtime/source reference: [E241]; agent instructions: [E242]; documented public use: [E240]; P:431 | N111 | rename → lf monitor prune | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C119 | lf doctor | lf home | Diagnosis of ledger, installation and planning consistency | Desktop: [E243]; Desktop: [E244]; runtime/source reference: [E245]; P:440 | N112 | rename → lf home doctor | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C120 | lf list | lf | Discovery of executable commands and authored definitions | CI: [E246]; runtime/source reference: [E247]; runtime/source reference: [E248]; P:449 | N113 | keep → lf list | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C121 | lf help | lf | Read-only explanation and validation of commands and authored definitions | runtime/source reference: [E248]; documented public use: [E129]; documented public use: [E249]; P:455 | N114 | keep → lf help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C122 | lf roadmap | lf task | Repository portfolio of Tasks and unlinked checkout/PR work | Desktop: [E250]; Desktop: [E150]; Desktop: [E251]; P:464 | N115 | rename → lf task list | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C123 | lf activity | lf monitor | Chronological changes in durable Work, delivery and direction | Desktop: [E252]; runtime/source reference: [E253]; runtime/source reference: [E254]; P:476 | N116 | rename → lf monitor activity | LOO-298; LOO-334; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C124 | lf runs | lf monitor | Execution and conversation evidence inspection | Desktop: [E255]; Desktop: [E226]; Desktop: [E256]; P:497 | N117 | merge into → lf monitor | LOO-298; Jack Heart 2026-09-30; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C125 | lf replay | lf monitor | New execution from captured provider inputs | documented public use: [E257]; documented public use: [E258]; documented public use: [E259]; P:545 | N118 | keep → lf monitor replay | LOO-298; Intelligence memory; adopted target; implementation not implied |
| C128 | lf op (hidden) | root | Retired rejection-only namespace with no remaining operation | documented public use: [E260]; test: [E261]; P:598 | N119 | delete → — | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C129 | lf ssh | lf home | Execution on a selected Home through an explicit SSH transport | runtime/source reference: [E262]; runtime/source reference: [E263]; runtime/source reference: [E264]; P:610 | N120 | rename → lf home ssh | LOO-338 catalog and primary-source research; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C130 | lf run | lf | Explicit flow-first execution of a named definition | runtime/source reference: [E248]; documented public use: [E181]; documented public use: [E265]; P:646 | N121 | keep → lf run | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C131 | lf flow | lf | Workflow execution and captured navigation namespace | runtime/source reference: [E266]; runtime/source reference: [E267]; runtime/source reference: [E268]; P:652 | N122 | keep → lf flow | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C132 | lf flow list | lf flow | Saved FlowSession inventory | Desktop: [E269]; runtime/source reference: [E268]; documented public use: [E270]; P:683 | N123 | keep → lf flow list | LOO-298; adopted target; implementation not implied |
| C133 | lf flow show | lf flow | Saved FlowSession graph and progress | documented public use: [E271]; test: [E272]; P:688 | N124 | keep → lf flow show | LOO-298; adopted target; implementation not implied |
| C134 | lf flow validate | lf flow | Read-only authored Flow compilation | documented public use: [E273]; test: [E274]; P:690 | N125 | merge into → lf help flow | LOO-338; Jack Heart 2026-09-30; implemented on integrated Exec/Session tree; focused proof recorded in working design |
| C135 | lf flow decide | lf flow | Retired command-side decision verdict | runtime/source reference: [E266]; runtime/source reference: [E275]; documented public use: [E276]; P:692 | N126 | delete → — | LOO-298; adopted target; implementation not implied |
| C136 | lf flow route | lf flow | Retired command-side branch verdict | runtime/source reference: [E267]; documented public use: [E277]; documented public use: [E278]; P:699 | N127 | delete → — | LOO-298; adopted target; implementation not implied |
| C137 | lf flow blocked | lf flow | Retired command-side blocked verdict | runtime/source reference: [E266]; runtime/source reference: [E279]; documented public use: [E280]; P:701 | N128 | delete → — | LOO-298; adopted target; implementation not implied |
| C138 | lf flow resume | lf flow | Continuation of a captured ordinary workflow invocation | runtime/source reference: [E281]; runtime/source reference: [E282]; documented public use: [E283]; P:706 | N129 | keep → lf flow resume | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C139 | lf skill | lf | Explicit execution of a named skill despite kind or command collisions | runtime/source reference: [E284]; runtime/source reference: [E285]; runtime/source reference: [E248]; P:657 | N130 | keep → lf skill | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| C140 | lf skill list | lf skill | Authored skill inventory | runtime/source reference: [E285]; documented public use: [E286]; P:669 | N131 | merge into → lf list skill | LOO-338; implemented in skill catalog merge; public CLI discovery proof; full Task incomplete |
| C141 | lf skill show | lf help | Read-only skill definition inspection | documented public use: [E287]; test: [E288]; P:675 | N132 | merge into → lf help skill | LOO-338; adopted target; implementation not implied |

## Merge and deletion checklist

| Row | Former command | Verdict / surviving owner |
|---|---|---|
| C002 | lf user name | merge into → lf home user |
| C028 | lf pr status | merge into → lf task pr |
| C033 | lf pr arm | merge into → lf task pr land |
| C045 | lf auth status | merge into → lf account |
| C051 | lf auth route show | merge into → lf account route |
| C080 | lf wave status | merge into → lf roadmap --wave WAVE |
| C082 | lf wave connect | merge into → lf repo connect |
| C085 | lf wave forget | delete → — |
| C087 | lf wave relocate | merge into → lf wave rename |
| C088 | lf wave retire | delete → — |
| C096 | lf task __worker | merge into → lf flow resume |
| C098 | lf task run | merge into → lf --as task:TASK flow |
| C101 | lf task changes | merge into → lf task diff --files |
| C111 | lf task restart | merge into → lf flow resume --restart |
| C124 | lf runs | merge into → lf monitor |
| C128 | lf op | delete → — |
| C134 | lf flow validate | merge into → lf help flow |
| C135 | lf flow decide | delete → — |
| C136 | lf flow route | delete → — |
| C137 | lf flow blocked | delete → — |
| C140 | lf skill list | merge into → lf list skill |
| C141 | lf skill show | merge into → lf help skill |

## Options and positional arguments

Primary short and long flags share one row. Hidden and automatic arguments are included; required/default metadata remains in the raw JSON. Purpose and distinct effect justify retained options; literal caller absence alone is not evidence of a dead public input.

| Row | Canonical path and argument | Owner / callers | Concept / distinct input | Rationale / source | Verdict → target | Follows / decision |
|---|---|---|---|---|---|---|
| A000 | lf --docs | C000 | Default direct provider conversation and command entry point: additional source material selected for the prompt. | N133; P:20 | keep → --docs | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A001 | lf --clipboard / -c | C000 | Default direct provider conversation and command entry point: clipboard material selected for the prompt. | N134; P:24 | keep → --clipboard | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A002 | lf --model / -m | C000 | Default direct provider conversation and command entry point: provider/model selection for execution. | N135; P:28 | keep → --model | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A003 | lf --account | C000 | Default direct provider conversation and command entry point: ordered account preference with permitted fallback. | N136; P:40 | keep → --account | LOO-340; adopted target; implementation not implied |
| A004 | lf --only-account | C000 | Default direct provider conversation and command entry point: hard descendant account-spending boundary without fallback. | N137; P:50 | keep → --only-account | LOO-340; adopted target; implementation not implied |
| A005 | lf --__account-lease-probe (hidden) | C000 | Default direct provider conversation and command entry point: redundant pre-launch broker compatibility probe. | N138; P:54 | keep → --__account-lease-probe | LOO-340; R04; integration proof required |
| A006 | lf --yolo | C000 | Default direct provider conversation and command entry point: provider permission-prompt bypass, not Git force or mutation consent. | N139; P:58 | keep → --yolo | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A007 | lf --interactive / -i | C000 | Default direct provider conversation and command entry point: interactive launch using the configured provider surface. | N140; P:62 | merge into → --mode interactive | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A008 | lf --batch / -b | C000 | Default direct provider conversation and command entry point: headless launch. | N141; P:66 | merge into → --mode batch | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A009 | lf --tui | C000 | Default direct provider conversation and command entry point: interactive terminal surface override. | N142; P:70 | merge into → --mode tui | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A010 | lf --ide | C000 | Default direct provider conversation and command entry point: interactive vendor-app surface override. | N143; P:74 | merge into → --mode ide | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A011 | lf --chrome | C000 | Default direct provider conversation and command entry point: explicit browser capability override. | N144; P:78 | keep → --chrome on\|off | LOO-338 catalog and primary-source research; implemented after main rebase; assembled prompt and browser-capability proof |
| A012 | lf --no-chrome | C000 | Default direct provider conversation and command entry point: explicit browser capability override. | N145; P:82 | merge into → --chrome off | LOO-338 catalog and primary-source research; implemented after main rebase; assembled prompt and browser-capability proof |
| A013 | lf --diff-files | C000 | Default direct provider conversation and command entry point: selected changed-code evidence in the prompt. | N146; P:86 | merge into → --diff files | LOO-338 catalog and primary-source research; implemented after main rebase; assembled prompt and browser-capability proof |
| A014 | lf --no-diff-files | C000 | Default direct provider conversation and command entry point: selected changed-code evidence in the prompt. | N147; P:90 | merge into → --diff patch\|none | LOO-338 catalog and primary-source research; implemented after main rebase; assembled prompt and browser-capability proof |
| A015 | lf --diff | C000 | Default direct provider conversation and command entry point: selected changed-code evidence in the prompt. | N148; P:94 | keep → --diff files\|patch\|both\|none | LOO-338 catalog and primary-source research; implemented after main rebase; assembled prompt and browser-capability proof |
| A016 | lf --no-diff | C000 | Default direct provider conversation and command entry point: selected changed-code evidence in the prompt. | N149; P:98 | merge into → --diff files\|none | LOO-338 catalog and primary-source research; implemented after main rebase; assembled prompt and browser-capability proof |
| A017 | lf --max-turns | C000 | Default direct provider conversation and command entry point: maximum provider turns for this execution. | N150; P:102 | keep → --max-turns | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A018 | lf --wave / -w | C000 | Default direct provider conversation and command entry point: Wave target or scope. | N151; P:106 | merge into → lf --as wave:ID | LOO-298; adopted target; implementation not implied |
| A019 | lf --task | C000 | Default direct provider conversation and command entry point: Task target or scope. | N151; P:110 | merge into → lf --as task:ID | LOO-298; adopted target; implementation not implied |
| A020 | lf --as | C000 | Default direct provider conversation and command entry point: explicit Task or Wave declaration, inherited unchanged by descendants. | N152; P:118 | keep → --as | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A021 | lf --__cwd (hidden) | C000 | Default direct provider conversation and command entry point: exact checkout pin for an already-bound internal launch. | N153; P:122 | keep → --__cwd | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A022 | lf --no-loopflow | C000 | Default direct provider conversation and command entry point: omission of operating guidance for deliberate prompt isolation. | N154; P:126 | keep → --no-loopflow | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A023 | lf --help / -h | C000 | Shared help protocol applied to default direct provider conversation and command entry point. | N155; P:14 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A024 | lf --version / -V | C000 | Default direct provider conversation and command entry point: release version selection or executable build identity. | N156; P:1579 | keep → --version | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A025 | lf user --help / -h | C001 | Shared help protocol applied to configured participant display name. | N155; P:225 | keep → --help | LOO-338; Jack Heart 2026-09-30; implemented after main rebase; focused public CLI proof |
| A026 | lf user name --json | C002 | Shared json protocol applied to participant display name. | N157; P:218 | merge into → lf home user --json | LOO-338; Jack Heart 2026-09-30; implemented after main rebase; focused public CLI proof |
| A027 | lf user name --help / -h | C002 | Shared help protocol applied to participant display name. | N157; P:216 | merge into → lf home user --help | LOO-338; Jack Heart 2026-09-30; implemented after main rebase; focused public CLI proof |
| A028 | lf : <prompt> | C003 | Execution of caller-authored prompt text: caller-authored execution instructions. | N158; P:233 | keep → <prompt> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A029 | lf : --help / -h | C003 | Shared help protocol applied to execution of caller-authored prompt text. | N155; P:231 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A030 | lf desktop --help / -h | C004 | Shared help protocol applied to desktop application activation. | N155; P:236 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A031 | lf screenshot <source> | C005 | Raster capture of a page with a bounded browser lifetime: page or local HTML selected for capture. | N159; P:198 | keep → <source> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A032 | lf screenshot --output / -o | C005 | Raster capture of a page with a bounded browser lifetime: capture artifact destination. | N160; P:202 | keep → --output | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A033 | lf screenshot --width | C005 | Raster capture of a page with a bounded browser lifetime: capture viewport width. | N161; P:206 | keep → --width | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A034 | lf screenshot --height | C005 | Raster capture of a page with a bounded browser lifetime: capture viewport height. | N162; P:210 | keep → --height | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A035 | lf screenshot --help / -h | C005 | Shared help protocol applied to raster capture of a page with a bounded browser lifetime. | N155; P:238 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A036 | lf __screenshot-supervisor <source> | C006 | Capture-child lifetime after its owning caller disappears: page or local HTML selected for capture. | N159; P:198 | keep → <source> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A037 | lf __screenshot-supervisor --output / -o | C006 | Capture-child lifetime after its owning caller disappears: capture artifact destination. | N160; P:202 | keep → --output | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A038 | lf __screenshot-supervisor --width | C006 | Capture-child lifetime after its owning caller disappears: capture viewport width. | N161; P:206 | keep → --width | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A039 | lf __screenshot-supervisor --height | C006 | Capture-child lifetime after its owning caller disappears: capture viewport height. | N162; P:210 | keep → --height | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A040 | lf __screenshot-supervisor --help / -h | C006 | Shared help protocol applied to capture-child lifetime after its owning caller disappears. | N155; P:244 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A041 | lf __provider-session --help / -h | C007 | Shared help protocol applied to attribution of a provider-native conversation to its execution. | N155; P:250 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A042 | lf ask --skill | C008 | Synchronous request for a new durable review conversation: named instructions for the review conversation. | N163; P:719 | keep → --skill | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A043 | lf ask <question> | C008 | Synchronous request for a new durable review conversation: new review request content. | N164; P:722 | keep → <question> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A044 | lf ask --help / -h | C008 | Shared help protocol applied to synchronous request for a new durable review conversation. | N155; P:252 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A045 | lf session --help / -h | C009 | Shared help protocol applied to conversation discovery and lifecycle namespace. | N155; P:257 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A046 | lf session list --json | C010 | Shared json protocol applied to resumable conversations and pending review obligations. | N165; P:730 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A047 | lf session list --all | C010 | Resumable conversations and pending review obligations: scope across all repositories or all locally authored Waves. | N166; P:733 | keep → --all | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A048 | lf session list --help / -h | C010 | Shared help protocol applied to resumable conversations and pending review obligations. | N155; P:728 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A049 | lf session open <id> | C011 | Continuation of a durable AgentSession: stable conversation identity. | N167; P:737 | keep → <id> | LOO-298; adopted target; implementation not implied |
| A050 | lf session open --json | C011 | Shared json protocol applied to continuation of a durable agentsession. | N165; P:739 | keep → --json | LOO-298; adopted target; implementation not implied |
| A051 | lf session open --replace | C011 | Continuation of a durable AgentSession: owned-client replacement before conversation continuation. | N168; P:742 | keep → --replace | LOO-298; adopted target; implementation not implied |
| A052 | lf session open --try | C011 | Continuation of a durable AgentSession: explicit concurrent native continuation attempt. | N169; P:745 | keep → --try | LOO-298; adopted target; implementation not implied |
| A053 | lf session open --help / -h | C011 | Shared help protocol applied to continuation of a durable agentsession. | N155; P:736 | keep → --help | LOO-298; adopted target; implementation not implied |
| A054 | lf session complete <id> | C012 | Completion of a conversation's review obligation: stable conversation identity. | N167; P:748 | keep → <id> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A055 | lf session complete --help / -h | C012 | Shared help protocol applied to completion of a conversation's review obligation. | N155; P:748 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A056 | lf session rename <id> | C013 | Conversation title with explicit authorship precedence: stable conversation identity. | N167; P:751 | keep → <id> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A057 | lf session rename <name> | C013 | Conversation title with explicit authorship precedence: owner name or local checkout/slug identity. | N170; P:753 | keep → <name> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A058 | lf session rename --suggest | C013 | Conversation title with explicit authorship precedence: generated title provenance that yields to an authored name. | N171; P:756 | keep → --suggest | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A059 | lf session rename --json | C013 | Shared json protocol applied to conversation title with explicit authorship precedence. | N165; P:758 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A060 | lf session rename --help / -h | C013 | Shared help protocol applied to conversation title with explicit authorship precedence. | N155; P:750 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A061 | lf session ready <summary> | C014 | Author readiness for a pending review without completing it: durable completion, readiness or verdict explanation. | N172; P:763 | keep → <summary> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A062 | lf session ready --help / -h | C014 | Shared help protocol applied to author readiness for a pending review without completing it. | N155; P:761 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A063 | lf session serve-flow <task_id> | C015 | Execution of one captured Flow review boundary: stable Task ownership of the captured child. | N173; P:768 | keep → <task_id> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A064 | lf session serve-flow <invocation_id> | C015 | Execution of one captured Flow review boundary: captured invocation identity fencing the review child. | N174; P:769 | keep → <invocation_id> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A065 | lf session serve-flow <flow> | C015 | Execution of one captured Flow review boundary: selected authored workflow. | N175; P:770 | keep → <flow> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A066 | lf session serve-flow <node_id> | C015 | Execution of one captured Flow review boundary: captured review node identity. | N176; P:771 | keep → <node_id> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A067 | lf session serve-flow <skill> | C015 | Execution of one captured Flow review boundary: named instructions for the review conversation. | N163; P:772 | keep → <skill> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A068 | lf session serve-flow <iteration> | C015 | Execution of one captured Flow review boundary: captured review occurrence iteration. | N177; P:773 | keep → <iteration> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A069 | lf session serve-flow --help / -h | C015 | Shared help protocol applied to execution of one captured flow review boundary. | N155; P:767 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A070 | lf session serve-ask <id> | C016 | Execution of one captured ad-hoc review request: stable conversation identity. | N167; P:777 | keep → <id> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A071 | lf session serve-ask --help / -h | C016 | Shared help protocol applied to execution of one captured ad-hoc review request. | N155; P:777 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A072 | lf session stop-run <run_id> | C017 | Termination of the exact provider execution owned by a completed review: exact execution to terminate after its owning review. | N178; P:780 | keep → <run_id> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A073 | lf session stop-run --help / -h | C017 | Shared help protocol applied to termination of the exact provider execution owned by a completed review. | N155; P:780 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A074 | lf install --help / -h | C018 | Shared help protocol applied to selection and activation of a verified machine installation. | N155; P:262 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A075 | lf install schedule <frequency> | C019 | Automatic activation of published software on a machine schedule: machine software-update calendar cadence. | N179; P:1151 | keep → <frequency> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A076 | lf install schedule --help / -h | C019 | Shared help protocol applied to automatic activation of published software on a machine schedule. | N155; P:1148 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A077 | lf install recover-switch --switch | C020 | Recovery of an interrupted installation selection transaction: installation transaction receipt identity. | N180; P:1158 | keep → --switch | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A078 | lf install recover-switch --help / -h | C020 | Shared help protocol applied to recovery of an interrupted installation selection transaction. | N155; P:1155 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A079 | lf install preflight --json | C021 | Shared json protocol applied to published-candidate compatibility with the selected installation store. | N165; P:1168 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A080 | lf install preflight --help / -h | C021 | Shared help protocol applied to published-candidate compatibility with the selected installation store. | N155; P:1165 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A081 | lf install local-preflight --store | C022 | Development-candidate compatibility with an explicitly retained private store: exact private store selected for candidate validation. | N181; P:1174 | keep → --store | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A082 | lf install local-preflight --json | C022 | Shared json protocol applied to development-candidate compatibility with an explicitly retained private store. | N165; P:1176 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A083 | lf install local-preflight --help / -h | C022 | Shared help protocol applied to development-candidate compatibility with an explicitly retained private store. | N155; P:1172 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A084 | lf install advance-switch --switch | C023 | Receipt-authorized schema advancement by the pinned candidate: installation transaction receipt identity. | N180; P:1182 | keep → --switch | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A085 | lf install advance-switch --help / -h | C023 | Shared help protocol applied to receipt-authorized schema advancement by the pinned candidate. | N155; P:1180 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A086 | lf install promote --from-build | C024 | Atomic activation of a specific candidate's CLI, app and store: explicit development-candidate activation mode. | N182; P:1192 | keep → --from-build | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A087 | lf install promote --coordinated-build (hidden) | C024 | Atomic activation of a specific candidate's CLI, app and store: candidate handoff to the receipt-pinned coordinator. | N183; P:1195 | keep → --coordinated-build | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A088 | lf install promote --fresh | C024 | Atomic activation of a specific candidate's CLI, app and store: explicit replacement of an incompatible disposable development store. | N184; P:1198 | keep → --fresh | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A089 | lf install promote --reuse-home | C024 | Atomic activation of a specific candidate's CLI, app and store: selection of retained development data. | N185; P:1201 | keep → --reuse-home | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A090 | lf install promote --cli-target | C024 | Atomic activation of a specific candidate's CLI, app and store: CLI activation symlink destination. | N186; P:1204 | keep → --cli-target | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A093 | lf install promote --app-source | C024 | Atomic activation of a specific candidate's CLI, app and store: matching app candidate bundle. | N187; P:1213 | keep → --app-source | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A094 | lf install promote --app-target | C024 | Atomic activation of a specific candidate's CLI, app and store: app activation destination. | N188; P:1216 | keep → --app-target | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A095 | lf install promote --legacy-app-target | C024 | Atomic activation of a specific candidate's CLI, app and store: retired app destination included in atomic replacement. | N189; P:1219 | keep → --legacy-app-target | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A096 | lf install promote --sync-skills | C024 | Atomic activation of a specific candidate's CLI, app and store: provider-native skill export after successful activation. | N190; P:1222 | keep → --sync-skills | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A097 | lf install promote --preview | C024 | Atomic activation of a specific candidate's CLI, app and store: read-only calculation of this mutation’s effects. | N191; P:1225 | keep → --preview | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A098 | lf install promote --help / -h | C024 | Shared help protocol applied to atomic activation of a specific candidate's cli, app and store. | N155; P:1189 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A099 | lf install rollback --cli-target | C025 | Restoration of retained installed bytes compatible with the existing store: CLI activation symlink destination. | N186; P:1233 | keep → --cli-target | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A100 | lf install rollback --candidate | C025 | Restoration of retained installed bytes compatible with the existing store: retained immutable candidate executable. | N192; P:1236 | keep → --candidate | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A103 | lf install rollback --help / -h | C025 | Shared help protocol applied to restoration of retained installed bytes compatible with the existing store. | N155; P:1230 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A104 | lf pr --help / -h | C026 | Shared help protocol applied to current branch's pull request and its delivery lifecycle. | N155; P:267 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A105 | lf pr checks --watch / -w | C027 | Required check outcomes and failure logs for one pull request: continuous observation of this result. | N193; P:1251 | keep → --watch | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A106 | lf pr checks --logs / -l | C027 | Required check outcomes and failure logs for one pull request: failure-log detail for observed CI checks. | N194; P:1253 | keep → --logs | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A107 | lf pr checks --help / -h | C027 | Shared help protocol applied to required check outcomes and failure logs for one pull request. | N155; P:1249 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A108 | lf pr status --help / -h | C028 | Shared help protocol applied to current pull request summary. | N195; P:1257 | merge into → lf task pr --help | LOO-338; Jack Heart 2026-09-30; implemented after main rebase; focused public CLI proof |
| A109 | lf pr next <slug> | C029 | Continuation of a Task's serial PR chain after a settled merge: next serial PR branch identity. | N196; P:1263 | keep → <slug> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A110 | lf pr next --help / -h | C029 | Shared help protocol applied to continuation of a task's serial pr chain after a settled merge. | N155; P:1260 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A111 | lf pr publish --model / -m | C030 | Publication of a ready review without merge intent: provider/model selection for execution. | N135; P:1269 | keep → --model | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A112 | lf pr publish --title | C030 | Publication of a ready review without merge intent: authored title for the selected planning or review object. | N197; P:1271 | keep → --title | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A113 | lf pr publish --body | C030 | Publication of a ready review without merge intent: authored PR review narrative. | N198; P:1273 | keep → --body | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A114 | lf pr publish --help / -h | C030 | Shared help protocol applied to publication of a ready review without merge intent. | N155; P:1267 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A115 | lf pr open --model / -m | C031 | Draft preparation and presentation for browser review: provider/model selection for execution. | N135; P:1279 | keep → --model | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A116 | lf pr open --title | C031 | Draft preparation and presentation for browser review: authored title for the selected planning or review object. | N197; P:1281 | keep → --title | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A117 | lf pr open --body | C031 | Draft preparation and presentation for browser review: authored PR review narrative. | N198; P:1283 | keep → --body | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A118 | lf pr open --help / -h | C031 | Shared help protocol applied to draft preparation and presentation for browser review. | N155; P:1277 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A119 | lf pr submit --strict | C032 | Preparation for a reviewer's future merge decision: strict delivery verification policy. | N199; P:1289 | keep → --strict | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A120 | lf pr submit --create-pr / -p | C032 | Preparation for a reviewer's future merge decision: permission to create a missing review artifact during submit. | N200; P:1291 | keep → --create-pr | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A121 | lf pr submit --complete / -c | C032 | Preparation for a reviewer's future merge decision: Task disposition after successful delivery. | N201; P:1293 | keep → --complete | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A122 | lf pr submit --next | C032 | Preparation for a reviewer's future merge decision: future serial PR continuation intent. | N202; P:1295 | keep → --next | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A123 | lf pr submit --worktree / -w | C032 | Preparation for a reviewer's future merge decision: checkout selected for delivery. | N203; P:1297 | keep → --worktree | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A124 | lf pr submit --message / -m | C032 | Preparation for a reviewer's future merge decision: commit content or appended discussion text. | N204; P:1299 | keep → --message | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A125 | lf pr submit --title | C032 | Preparation for a reviewer's future merge decision: authored title for the selected planning or review object. | N197; P:1301 | keep → --title | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A126 | lf pr submit --body | C032 | Preparation for a reviewer's future merge decision: authored PR review narrative. | N198; P:1303 | keep → --body | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A127 | lf pr submit --help / -h | C032 | Shared help protocol applied to preparation for a reviewer's future merge decision. | N155; P:1287 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A128 | lf pr arm --strict | C033 | Authorized delivery handoff: strict delivery verification policy. | N205; P:1308 | merge into → lf task pr land --strict | LOO-332; adopted target; implementation not implied |
| A129 | lf pr arm --local | C033 | Authorized delivery handoff: settlement through the invoking process rather than the Home supervisor. | N205; P:1310 | merge into → lf task pr land --local | LOO-332; adopted target; implementation not implied |
| A130 | lf pr arm --complete / -c | C033 | Authorized delivery handoff: Task disposition after successful delivery. | N205; P:1312 | merge into → lf task pr land --complete | LOO-332; adopted target; implementation not implied |
| A131 | lf pr arm --next | C033 | Authorized delivery handoff: future serial PR continuation intent. | N205; P:1314 | merge into → lf task pr land --next | LOO-332; adopted target; implementation not implied |
| A132 | lf pr arm --worktree / -w | C033 | Authorized delivery handoff: checkout selected for delivery. | N205; P:1316 | merge into → lf task pr land --worktree | LOO-332; adopted target; implementation not implied |
| A133 | lf pr arm --message / -m | C033 | Authorized delivery handoff: commit content or appended discussion text. | N205; P:1318 | merge into → lf task pr land --message | LOO-332; adopted target; implementation not implied |
| A134 | lf pr arm --title | C033 | Authorized delivery handoff: authored title for the selected planning or review object. | N205; P:1320 | merge into → lf task pr land --title | LOO-332; adopted target; implementation not implied |
| A135 | lf pr arm --body | C033 | Authorized delivery handoff: authored PR review narrative. | N205; P:1322 | merge into → lf task pr land --body | LOO-332; adopted target; implementation not implied |
| A136 | lf pr arm --help / -h | C033 | Shared help protocol applied to authorized delivery handoff. | N205; P:1306 | merge into → lf task pr land --help | LOO-332; adopted target; implementation not implied |
| A137 | lf pr land --strict | C034 | Authorized delivery handoff: strict delivery verification policy. | N199; P:1327 | keep → --strict | LOO-332; adopted target; implementation not implied |
| A138 | lf pr land --local | C034 | Authorized delivery handoff: settlement through the invoking process rather than the Home supervisor. | N206; P:1329 | keep → --local | LOO-332; adopted target; implementation not implied |
| A139 | lf pr land --complete / -c | C034 | Authorized delivery handoff: Task disposition after successful delivery. | N201; P:1331 | keep → --complete | LOO-332; adopted target; implementation not implied |
| A140 | lf pr land --next | C034 | Authorized delivery handoff: future serial PR continuation intent. | N202; P:1333 | keep → --next | LOO-332; adopted target; implementation not implied |
| A141 | lf pr land --worktree / -w | C034 | Authorized delivery handoff: checkout selected for delivery. | N203; P:1335 | keep → --worktree | LOO-332; adopted target; implementation not implied |
| A142 | lf pr land --message / -m | C034 | Authorized delivery handoff: commit content or appended discussion text. | N204; P:1337 | keep → --message | LOO-332; adopted target; implementation not implied |
| A143 | lf pr land --title | C034 | Authorized delivery handoff: authored title for the selected planning or review object. | N197; P:1339 | keep → --title | LOO-332; adopted target; implementation not implied |
| A144 | lf pr land --body | C034 | Authorized delivery handoff: authored PR review narrative. | N198; P:1341 | keep → --body | LOO-332; adopted target; implementation not implied |
| A145 | lf pr land --help / -h | C034 | Shared help protocol applied to authorized delivery handoff. | N155; P:1325 | keep → --help | LOO-332; adopted target; implementation not implied |
| A146 | lf pr abandon <branch> | C035 | Discarding branch and PR artifacts without deleting the planning Task: Git branch selected for discard. | N207; P:1346 | keep → <branch> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A147 | lf pr abandon --force / -f | C035 | Discarding branch and PR artifacts without deleting the planning Task: explicit override of dirty-state protection. | N208; P:1348 | keep → --force | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A148 | lf pr abandon --help / -h | C035 | Shared help protocol applied to discarding branch and pr artifacts without deleting the planning task. | N155; P:1344 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A149 | lf wt --help / -h | C036 | Shared help protocol applied to physical checkout lifecycle namespace. | N155; P:272 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A150 | lf wt create <name> | C037 | Allocation of an untracked sibling checkout: owner name or local checkout/slug identity. | N170; P:1640 | keep → <name> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A151 | lf wt create --plan | C037 | Allocation of an untracked sibling checkout: read-only operation preview or authored chapter/account plan input. | N209; P:1643 | keep → --plan | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A152 | lf wt create --help / -h | C037 | Shared help protocol applied to allocation of an untracked sibling checkout. | N155; P:1638 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A153 | lf wt switch <name> | C038 | Selection of an existing physical checkout: owner name or local checkout/slug identity. | N170; P:1648 | keep → <name> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A154 | lf wt switch --help / -h | C038 | Shared help protocol applied to selection of an existing physical checkout. | N155; P:1646 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A155 | lf wt list --format | C039 | Inventory of physical checkouts and their branch facts: machine serialization selector duplicated by JSON. | N210; P:1653 | rename → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A156 | lf wt list --full | C039 | Inventory of physical checkouts and their branch facts: ignored worktree-list presentation switch. | N211; P:1655 | delete → — | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A157 | lf wt list --sync | C039 | Inventory of physical checkouts and their branch facts: explicit refresh of remote facts before this read. | N212; P:1659 | keep → --sync | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A158 | lf wt list --help / -h | C039 | Shared help protocol applied to inventory of physical checkouts and their branch facts. | N155; P:1651 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A159 | lf wt prune --dry-run | C040 | Cleanup of eligible inactive or terminal physical checkouts: read-only preview of the selected mutation. | N213; P:1665 | keep → --dry-run | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A160 | lf wt prune --help / -h | C040 | Shared help protocol applied to cleanup of eligible inactive or terminal physical checkouts. | N155; P:1662 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A161 | lf wt remove <name> | C041 | Removal of one explicitly selected physical checkout: owner name or local checkout/slug identity. | N170; P:1671 | keep → <name> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A162 | lf wt remove --force / -f | C041 | Removal of one explicitly selected physical checkout: explicit override of dirty-state protection. | N208; P:1673 | keep → --force | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A163 | lf wt remove --help / -h | C041 | Shared help protocol applied to removal of one explicitly selected physical checkout. | N155; P:1669 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A164 | lf rebase --plan | C042 | Integration of the current checkout with its selected Git base: read-only operation preview or authored chapter/account plan input. | N209; P:280 | keep → --plan | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A165 | lf rebase --manual | C042 | Integration of the current checkout with its selected Git base: local ownership of rebase conflict handling. | N214; P:283 | keep → --manual | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A166 | lf rebase --continue | C042 | Integration of the current checkout with its selected Git base: continuation of the existing local rebase. | N215; P:286 | keep → --continue | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A167 | lf rebase --abort | C042 | Integration of the current checkout with its selected Git base: cancellation of the existing local rebase. | N216; P:289 | keep → --abort | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A168 | lf rebase --adopt | C042 | Integration of the current checkout with its selected Git base: explicit adoption of an already-running raw rebase. | N217; P:292 | keep → --adopt | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A169 | lf rebase <onto> | C042 | Integration of the current checkout with its selected Git base: Git integration base. | N218; P:294 | keep → <onto> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A170 | lf rebase --help / -h | C042 | Shared help protocol applied to integration of the current checkout with its selected git base. | N155; P:277 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A171 | lf commit --message / -m | C043 | Local Git checkpoint of current changes: commit content or appended discussion text. | N204; P:299 | keep → --message | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A172 | lf commit --push / -p | C043 | Local Git checkpoint of current changes: PR publication duplicated inside local checkpointing. | N219; P:301 | merge into → lf task pr open | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A173 | lf commit --no-add | C043 | Local Git checkpoint of current changes: restriction of checkpoint content to the existing index. | N220; P:303 | keep → --no-add | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A174 | lf commit --help / -h | C043 | Shared help protocol applied to local git checkpoint of current changes. | N155; P:297 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A175 | lf auth --help / -h | C044 | Shared help protocol applied to provider access, identity and capacity. | N155; P:306 | keep → --help | LOO-339; LOO-340; adopted target; implementation not implied |
| A176 | lf auth status <provider> | C045 | Provider access, identity and capacity: provider whose connection or routing is selected. | N221; P:1494 | merge into → lf account <provider> | LOO-339; LOO-340; adopted target; implementation not implied |
| A177 | lf auth status --verify | C045 | Provider access, identity and capacity: live account identity and capacity observation; replaced by the live default in LOO-340. | N222; P:1496 | delete → — | LOO-339; LOO-340; adopted target; implementation not implied |
| A178 | lf auth status --details | C045 | Provider access, identity and capacity: credential-source and capacity evidence detail. | N221; P:1498 | merge into → lf account --details | LOO-339; LOO-340; adopted target; implementation not implied |
| A179 | lf auth status --json | C045 | Shared json protocol applied to provider access, identity and capacity. | N221; P:1500 | merge into → lf account --json | LOO-339; LOO-340; adopted target; implementation not implied |
| A180 | lf auth status --help / -h | C045 | Shared help protocol applied to provider access, identity and capacity. | N221; P:1493 | merge into → lf account --help | LOO-339; LOO-340; adopted target; implementation not implied |
| A181 | lf auth disconnect <provider> | C046 | Revocation of local or managed provider credentials: provider whose connection or routing is selected. | N223; P:1504 | keep → <provider> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A182 | lf auth disconnect <email> | C046 | Revocation of local or managed provider credentials: managed login identity. | N224; P:1505 | keep → <email> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A183 | lf auth disconnect --help / -h | C046 | Shared help protocol applied to revocation of local or managed provider credentials. | N155; P:1503 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A184 | lf auth connect <provider> | C047 | Acquisition of local or managed provider credentials: provider whose connection or routing is selected. | N223; P:1509 | keep → <provider> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A185 | lf auth connect <email> | C047 | Acquisition of local or managed provider credentials: managed login identity. | N224; P:1510 | keep → <email> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A186 | lf auth connect --chrome-profile | C047 | Acquisition of local or managed provider credentials: ordered browser venues for provider login. | N225; P:1512 | keep → --chrome-profile | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A187 | lf auth connect --import | C047 | Acquisition of local or managed provider credentials: adoption of an existing native login. | N226; P:1515 | keep → --import | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A188 | lf auth connect --api-key | C047 | Acquisition of local or managed provider credentials: adoption of an environment-provided API credential. | N227; P:1518 | keep → --api-key | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A189 | lf auth connect --help / -h | C047 | Shared help protocol applied to acquisition of local or managed provider credentials. | N155; P:1508 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A190 | lf auth set <provider> | C048 | Account metadata and remembered browser venues: provider whose connection or routing is selected. | N223; P:1522 | keep → <provider> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A191 | lf auth set <email> | C048 | Account metadata and remembered browser venues: managed login identity. | N224; P:1523 | keep → <email> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A192 | lf auth set --login-email | C048 | Account metadata and remembered browser venues: corrected login identity. | N228; P:1525 | keep → --login-email | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A193 | lf auth set --routing | C048 | Account metadata and remembered browser venues: whether this account participates in selection. | N229; P:1527 | keep → --routing | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A194 | lf auth set --plan | C048 | Account metadata and remembered browser venues: read-only operation preview or authored chapter/account plan input. | N209; P:1529 | keep → --plan | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A195 | lf auth set --clear-plan | C048 | Account metadata and remembered browser venues: removal of configured subscription plan metadata. | N230; P:1531 | keep → --clear-plan | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A196 | lf auth set --paid-through | C048 | Account metadata and remembered browser venues: configured paid subscription expiration. | N231; P:1533 | keep → --paid-through | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A197 | lf auth set --clear-paid-through | C048 | Account metadata and remembered browser venues: removal of configured subscription expiration. | N232; P:1535 | keep → --clear-paid-through | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A198 | lf auth set --clear-cooldown | C048 | Account metadata and remembered browser venues: removal of local failure cooldown without rewriting usage windows. | N233; P:1537 | keep → --clear-cooldown | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A199 | lf auth set --chrome-profile | C048 | Account metadata and remembered browser venues: ordered browser venues for provider login. | N225; P:1540 | keep → --chrome-profile | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A200 | lf auth set --clear-chrome-profiles | C048 | Account metadata and remembered browser venues: removal of remembered browser venues. | N234; P:1542 | keep → --clear-chrome-profiles | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A201 | lf auth set --help / -h | C048 | Shared help protocol applied to account metadata and remembered browser venues. | N155; P:1521 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A202 | lf auth route --help / -h | C049 | Shared help protocol applied to explanation of ordered account selection for a routing scope. | N155; P:1545 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A203 | lf auth route set <provider> | C050 | Replacement of the ordered accounts in a routing scope: provider whose connection or routing is selected. | N223; P:1555 | keep → <provider> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A204 | lf auth route set <accounts> | C050 | Replacement of the ordered accounts in a routing scope: ordered account route members. | N235; P:1557 | keep → <accounts> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A205 | lf auth route set --repo | C050 | Replacement of the ordered accounts in a routing scope: repository routing or observation scope. | N236; P:1559 | keep → --repo | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A206 | lf auth route set --default | C050 | Replacement of the ordered accounts in a routing scope: machine-default account route scope. | N237; P:1561 | keep → --default | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A207 | lf auth route set --help / -h | C050 | Shared help protocol applied to replacement of the ordered accounts in a routing scope. | N155; P:1554 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A208 | lf auth route show --repo | C051 | Configured account routing: repository routing or observation scope. | N238; P:1566 | merge into → lf account route --repo | LOO-340; adopted target; implementation not implied |
| A209 | lf auth route show --default | C051 | Configured account routing: machine-default account route scope. | N238; P:1568 | merge into → lf account route --default | LOO-340; adopted target; implementation not implied |
| A210 | lf auth route show --json | C051 | Shared json protocol applied to configured account routing. | N238; P:1570 | merge into → lf account route --json | LOO-340; adopted target; implementation not implied |
| A211 | lf auth route show --help / -h | C051 | Shared help protocol applied to configured account routing. | N238; P:1564 | merge into → lf account route --help | LOO-340; adopted target; implementation not implied |
| A212 | lf release --help / -h | C052 | Shared help protocol applied to repository release lifecycle namespace. | N155; P:311 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A213 | lf release run <version> | C053 | Recovery or completion of a verified repository release: release version selection or executable build identity. | N156; P:1579 | keep → <version> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A214 | lf release run --target / -t | C053 | Recovery or completion of a verified repository release: release product or SSH destination selected by this operation. | N239; P:1581 | keep → --target | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A215 | lf release run --help / -h | C053 | Shared help protocol applied to recovery or completion of a verified repository release. | N155; P:1577 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A216 | lf release check --target / -t | C054 | Unreleased changes eligible for the next repository release: release product or SSH destination selected by this operation. | N239; P:1586 | keep → --target | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A217 | lf release check --help / -h | C054 | Shared help protocol applied to unreleased changes eligible for the next repository release. | N155; P:1584 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A218 | lf release notes <version> | C055 | Authored release narrative and its archival destination: release version selection or executable build identity. | N156; P:1591 | keep → <version> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A219 | lf release notes --prev-tag | C055 | Authored release narrative and its archival destination: release narrative comparison base. | N240; P:1593 | keep → --prev-tag | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A220 | lf release notes --preview | C055 | Authored release narrative and its archival destination: read-only calculation of this mutation’s effects. | N191; P:1596 | keep → --preview | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A221 | lf release notes --target / -t | C055 | Authored release narrative and its archival destination: release product or SSH destination selected by this operation. | N239; P:1598 | keep → --target | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A222 | lf release notes --help / -h | C055 | Shared help protocol applied to authored release narrative and its archival destination. | N155; P:1589 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A223 | lf release bump <version> | C056 | Version values in repository manifests: release version selection or executable build identity. | N156; P:1603 | keep → <version> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A224 | lf release bump --target / -t | C056 | Version values in repository manifests: release product or SSH destination selected by this operation. | N239; P:1605 | keep → --target | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A225 | lf release bump --help / -h | C056 | Shared help protocol applied to version values in repository manifests. | N155; P:1601 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A226 | lf release tag <version> | C057 | Published Git version tag: release version selection or executable build identity. | N156; P:1610 | keep → <version> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A227 | lf release tag --target / -t | C057 | Published Git version tag: release product or SSH destination selected by this operation. | N239; P:1612 | keep → --target | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A228 | lf release tag --help / -h | C057 | Shared help protocol applied to published git version tag. | N155; P:1608 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A229 | lf release publish <tag> | C058 | Hosted release draft, artifacts and publication state: immutable version label selected for hosted publication. | N241; P:1617 | keep → <tag> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A230 | lf release publish --notes | C058 | Hosted release draft, artifacts and publication state: authored release or Task narrative. | N242; P:1620 | keep → --notes | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A231 | lf release publish --asset | C058 | Hosted release draft, artifacts and publication state: artifact files attached to the hosted release. | N243; P:1623 | keep → --asset | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A232 | lf release publish --finalize | C058 | Hosted release draft, artifacts and publication state: transition of the existing release draft to published. | N244; P:1626 | keep → --finalize | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A233 | lf release publish --help / -h | C058 | Shared help protocol applied to hosted release draft, artifacts and publication state. | N155; P:1615 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A234 | lf release status --target / -t | C059 | Observed release workflow and hosted publication status: release product or SSH destination selected by this operation. | N239; P:1631 | keep → --target | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A235 | lf release status --help / -h | C059 | Shared help protocol applied to observed release workflow and hosted publication status. | N155; P:1629 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A236 | lf repo --help / -h | C060 | Shared help protocol applied to repository administration namespace. | N155; P:316 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A237 | lf repo reteam --apply | C061 | Reconciliation of the repository's planning Team across Waves: application of a planning-team reconciliation preview. | N245; P:1446 | keep → --apply | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A238 | lf repo reteam --help / -h | C061 | Shared help protocol applied to reconciliation of the repository's planning team across waves. | N155; P:1444 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A244 | lf home --help / -h | C065 | Shared help protocol applied to machine placement and installation namespace. | N155; P:321 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A245 | lf home id --json | C066 | Shared json protocol applied to stable execution-destination identity of this machine. | N165; P:1479 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A246 | lf home id --help / -h | C066 | Shared help protocol applied to stable execution-destination identity of this machine. | N155; P:1477 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A247 | lf home observe <home_id> | C067 | Observed network route to a known execution destination: stable execution destination identity. | N246; P:1483 | keep → <home_id> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A248 | lf home observe <route> | C067 | Observed network route to a known execution destination: observed network address for a known Home. | N247; P:1484 | keep → <route> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A249 | lf home observe --json | C067 | Shared json protocol applied to observed network route to a known execution destination. | N165; P:1486 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A250 | lf home observe --help / -h | C067 | Shared help protocol applied to observed network route to a known execution destination. | N155; P:1482 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A251 | lf sync-skills --yes / -y | C068 | Export of authored skills into provider-native skill directories: explicit consent for provider skill-directory writes. | N248; P:330 | keep → --yes | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A252 | lf sync-skills --no-prune | C068 | Export of authored skills into provider-native skill directories: preservation of prior generated skills during export. | N249; P:333 | keep → --no-prune | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A253 | lf sync-skills --help / -h | C068 | Shared help protocol applied to export of authored skills into provider-native skill directories. | N155; P:327 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A254 | lf cron --help / -h | C069 | Shared help protocol applied to wave schedule declaration and execution namespace. | N155; P:336 | keep → --help | LOO-332; integration choice open |
| A255 | lf cron add --wave / -w | C070 | Installation of one manually declared scheduled invocation: Wave target or scope. | N250; P:1358 | keep → --wave | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A256 | lf cron add --flow | C070 | Installation of one manually declared scheduled invocation: selected authored workflow. | N175; P:1361 | keep → --flow | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A257 | lf cron add --schedule | C070 | Installation of one manually declared scheduled invocation: scheduled invocation calendar expression. | N251; P:1364 | keep → --schedule | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A258 | lf cron add --help / -h | C070 | Shared help protocol applied to installation of one manually declared scheduled invocation. | N155; P:1355 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A259 | lf cron list --wave / -w | C071 | Installed schedule inventory: Wave target or scope. | N250; P:1370 | keep → --wave | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A260 | lf cron list --json | C071 | Shared json protocol applied to installed schedule inventory. | N165; P:1373 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A261 | lf cron list --help / -h | C071 | Shared help protocol applied to installed schedule inventory. | N155; P:1367 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A262 | lf cron preflight --wave / -w | C072 | Read-only feasibility of declared Wave schedules on their owning Home: Wave target or scope. | N250; P:1379 | keep → --wave | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A263 | lf cron preflight --help / -h | C072 | Shared help protocol applied to read-only feasibility of declared wave schedules on their owning home. | N155; P:1376 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A264 | lf cron sync --wave / -w | C073 | Reconciliation of installed jobs with authored Wave schedule declarations: Wave target or scope. | N250; P:1385 | keep → --wave | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A265 | lf cron sync --help / -h | C073 | Shared help protocol applied to reconciliation of installed jobs with authored wave schedule declarations. | N155; P:1382 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A266 | lf cron run --wave / -w | C074 | Receipt-bearing execution of one scheduled invocation: Wave target or scope. | N250; P:1392 | keep → --wave | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A267 | lf cron run --flow | C074 | Receipt-bearing execution of one scheduled invocation: selected authored workflow. | N175; P:1395 | keep → --flow | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A268 | lf cron run --scheduled (hidden) | C074 | Receipt-bearing execution of one scheduled invocation: scheduler-origin attribution for receipt obligations. | N252; P:1398 | keep → --scheduled | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A269 | lf cron run --help / -h | C074 | Shared help protocol applied to receipt-bearing execution of one scheduled invocation. | N155; P:1389 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A270 | lf cron history --wave / -w | C075 | Observed scheduled invocation outcomes: Wave target or scope. | N250; P:1404 | keep → --wave | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A271 | lf cron history --flow | C075 | Observed scheduled invocation outcomes: selected authored workflow. | N175; P:1407 | keep → --flow | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A272 | lf cron history --days | C075 | Observed scheduled invocation outcomes: observation window, retaining all-time semantics where supported. | N253; P:1410 | keep → --days | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A273 | lf cron history --json | C075 | Shared json protocol applied to observed scheduled invocation outcomes. | N165; P:1413 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A274 | lf cron history --help / -h | C075 | Shared help protocol applied to observed scheduled invocation outcomes. | N155; P:1401 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A275 | lf cron trigger --wave / -w | C076 | Explicit firing of an installed scheduled invocation: Wave target or scope. | N250; P:1419 | keep → --wave | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A276 | lf cron trigger --flow | C076 | Explicit firing of an installed scheduled invocation: selected authored workflow. | N175; P:1422 | keep → --flow | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A277 | lf cron trigger --wait | C076 | Explicit firing of an installed scheduled invocation: waiting for the fired schedule’s receipt. | N254; P:1425 | keep → --wait | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A278 | lf cron trigger --timeout | C076 | Explicit firing of an installed scheduled invocation: maximum wait for an observable condition. | N255; P:1428 | keep → --timeout | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A279 | lf cron trigger --help / -h | C076 | Shared help protocol applied to explicit firing of an installed scheduled invocation. | N155; P:1416 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A280 | lf cron remove --wave / -w | C077 | Removal of an installed scheduled invocation: Wave target or scope. | N250; P:1434 | keep → --wave | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A281 | lf cron remove --flow | C077 | Removal of an installed scheduled invocation: selected authored workflow. | N175; P:1437 | keep → --flow | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A282 | lf cron remove --help / -h | C077 | Shared help protocol applied to removal of an installed scheduled invocation. | N155; P:1431 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A283 | lf wave --help / -h | C078 | Shared help protocol applied to repository-authored responsibility and planning namespace. | N155; P:341 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A284 | lf wave list --json | C079 | Shared json protocol applied to repository-authored responsibilities. | N165; P:827 | keep → --json | LOO-334; LOO-329; LOO-330; adopted target; implementation not implied |
| A285 | lf wave list --all | C079 | Repository-authored responsibilities: scope across all repositories or all locally authored Waves. | N166; P:831 | keep → --all | LOO-334; LOO-329; LOO-330; adopted target; implementation not implied |
| A286 | lf wave list --current | C079 | Repository-authored responsibilities: current-registration filter without deleting history. | N256; P:834 | keep → --current | LOO-334; LOO-329; LOO-330; adopted target; implementation not implied |
| A287 | lf wave list --help / -h | C079 | Shared help protocol applied to repository-authored responsibilities. | N155; P:824 | keep → --help | LOO-334; LOO-329; LOO-330; adopted target; implementation not implied |
| A288 | lf wave status <wave> | C080 | Wave-scoped planning and work evidence: Wave target or scope. | N257; P:840 | merge into → lf roadmap --wave WAVE <wave> | LOO-298; LOO-334; adopted target; implementation not implied |
| A289 | lf wave status --chapter | C080 | Wave-scoped planning and work evidence: dated chapter identity. | N257; P:843 | merge into → lf roadmap --wave WAVE --chapter | LOO-298; LOO-334; adopted target; implementation not implied |
| A290 | lf wave status --json | C080 | Shared json protocol applied to wave-scoped planning and work evidence. | N257; P:846 | merge into → lf roadmap --wave WAVE --json | LOO-298; LOO-334; adopted target; implementation not implied |
| A291 | lf wave status --sync | C080 | Wave-scoped planning and work evidence: explicit refresh of remote facts before this read. | N257; P:849 | merge into → lf roadmap --wave WAVE --sync | LOO-298; LOO-334; adopted target; implementation not implied |
| A292 | lf wave status --no-sync | C080 | Wave-scoped planning and work evidence: ignored cached-read switch. | N257; P:852 | merge into → lf roadmap --wave WAVE --no-sync | LOO-298; LOO-334; adopted target; implementation not implied |
| A293 | lf wave status --help / -h | C080 | Shared help protocol applied to wave-scoped planning and work evidence. | N257; P:838 | merge into → lf roadmap --wave WAVE --help | LOO-298; LOO-334; adopted target; implementation not implied |
| A297 | lf wave connect <wave> | C082 | Repository planning connection: Wave target or scope. | N258; P:869 | merge into → lf repo connect <wave> | LOO-334; adopted target; implementation not implied |
| A298 | lf wave connect --wave / -w | C082 | Repository planning connection: duplicate spelling of the positional Wave target. | N258; P:872 | merge into → lf repo connect --wave | LOO-334; adopted target; implementation not implied |
| A299 | lf wave connect --all | C082 | Repository planning connection: scope across all repositories or all locally authored Waves. | N258; P:875 | merge into → lf repo connect --all | LOO-334; adopted target; implementation not implied |
| A300 | lf wave connect --team-key | C082 | Repository planning connection: repository planning Team identifier/prefix. | N258; P:878 | merge into → lf repo connect --team-key | LOO-334; adopted target; implementation not implied |
| A301 | lf wave connect --team-name | C082 | Repository planning connection: repository planning Team display name. | N258; P:881 | merge into → lf repo connect --team-name | LOO-334; adopted target; implementation not implied |
| A302 | lf wave connect --help / -h | C082 | Shared help protocol applied to repository planning connection. | N258; P:867 | merge into → lf repo connect --help | LOO-334; adopted target; implementation not implied |
| A303 | lf wave sync <wave> | C083 | Refresh of shared planning observations: Wave target or scope. | N250; P:885 | keep → <wave> | LOO-334; adopted target; implementation not implied |
| A304 | lf wave sync --wave / -w | C083 | Refresh of shared planning observations: duplicate spelling of the positional Wave target. | N259; P:887 | keep → --wave | LOO-334; adopted target; implementation not implied |
| A305 | lf wave sync --all | C083 | Refresh of shared planning observations: scope across all repositories or all locally authored Waves. | N166; P:889 | keep → --all | LOO-334; adopted target; implementation not implied |
| A306 | lf wave sync --help / -h | C083 | Shared help protocol applied to refresh of shared planning observations. | N155; P:884 | keep → --help | LOO-334; adopted target; implementation not implied |
| A307 | lf wave rename <wave> | C084 | Rename of an authored responsibility and its provider mapping: Wave target or scope. | N250; P:893 | keep → <wave> | LOO-329; LOO-334; demo/integration choice open; see destination audit |
| A308 | lf wave rename --title | C084 | Rename of an authored responsibility and its provider mapping: authored title for the selected planning or review object. | N197; P:895 | keep → --title | LOO-329; LOO-334; demo/integration choice open; see destination audit |
| A309 | lf wave rename --help / -h | C084 | Shared help protocol applied to rename of an authored responsibility and its provider mapping. | N155; P:892 | keep → --help | LOO-329; LOO-334; demo/integration choice open; see destination audit |
| A310 | lf wave forget <name> | C085 | Retired empty Wave registration cleanup: owner name or local checkout/slug identity. | N260; P:899 | delete → — | LOO-334; adopted target; implementation not implied |
| A311 | lf wave forget --dry-run | C085 | Retired empty Wave registration cleanup: read-only preview of the selected mutation. | N260; P:901 | delete → — | LOO-334; adopted target; implementation not implied |
| A312 | lf wave forget --json | C085 | Shared json protocol applied to retired empty wave registration cleanup. | N260; P:903 | delete → — | LOO-334; adopted target; implementation not implied |
| A313 | lf wave forget --help / -h | C085 | Shared help protocol applied to retired empty wave registration cleanup. | N260; P:898 | delete → — | LOO-334; adopted target; implementation not implied |
| A314 | lf wave place <name> | C086 | Execution placement of a durable responsibility: owner name or local checkout/slug identity. | N170; P:907 | keep → <name> | LOO-332; LOO-334; demo/integration choice open; see destination audit |
| A315 | lf wave place <home_id> | C086 | Execution placement of a durable responsibility: stable execution destination identity. | N246; P:908 | keep → <home_id> | LOO-332; LOO-334; demo/integration choice open; see destination audit |
| A316 | lf wave place --json | C086 | Shared json protocol applied to execution placement of a durable responsibility. | N165; P:910 | keep → --json | LOO-332; LOO-334; demo/integration choice open; see destination audit |
| A317 | lf wave place --help / -h | C086 | Shared help protocol applied to execution placement of a durable responsibility. | N155; P:906 | keep → --help | LOO-332; LOO-334; demo/integration choice open; see destination audit |
| A318 | lf wave relocate <wave> | C087 | Rename of an authored responsibility and its provider mapping: Wave target or scope. | N261; P:914 | merge into → lf wave rename <wave> | LOO-298; LOO-329; LOO-334; adopted target; implementation not implied |
| A319 | lf wave relocate --repo | C087 | Rename of an authored responsibility and its provider mapping: repository routing or observation scope. | N261; P:916 | merge into → lf wave rename --repo | LOO-298; LOO-329; LOO-334; adopted target; implementation not implied |
| A320 | lf wave relocate --name | C087 | Rename of an authored responsibility and its provider mapping: owner name or local checkout/slug identity. | N261; P:918 | merge into → lf wave rename --name | LOO-298; LOO-329; LOO-334; adopted target; implementation not implied |
| A321 | lf wave relocate --json | C087 | Shared json protocol applied to rename of an authored responsibility and its provider mapping. | N261; P:920 | merge into → lf wave rename --json | LOO-298; LOO-329; LOO-334; adopted target; implementation not implied |
| A322 | lf wave relocate --help / -h | C087 | Shared help protocol applied to rename of an authored responsibility and its provider mapping. | N261; P:913 | merge into → lf wave rename --help | LOO-298; LOO-329; LOO-334; adopted target; implementation not implied |
| A323 | lf wave retire <name> | C088 | Retired resident Wave lifecycle: owner name or local checkout/slug identity. | N262; P:924 | delete → — | LOO-298; LOO-334; adopted target; implementation not implied |
| A324 | lf wave retire --reason | C088 | Retired resident Wave lifecycle: durable explanation for disposition or retry. | N262; P:926 | delete → — | LOO-298; LOO-334; adopted target; implementation not implied |
| A325 | lf wave retire --json | C088 | Shared json protocol applied to retired resident wave lifecycle. | N262; P:928 | delete → — | LOO-298; LOO-334; adopted target; implementation not implied |
| A326 | lf wave retire --help / -h | C088 | Shared help protocol applied to retired resident wave lifecycle. | N262; P:923 | delete → — | LOO-298; LOO-334; adopted target; implementation not implied |
| A331 | lf wave new-chapter --wave / -w | C090 | Repository-wide chapter boundary: Wave target or scope. | N250; P:943 | keep → --wave | LOO-298; adopted target; implementation not implied |
| A332 | lf wave new-chapter --chapter | C090 | Repository-wide chapter boundary: dated chapter identity. | N263; P:945 | keep → --chapter | LOO-298; adopted target; implementation not implied |
| A333 | lf wave new-chapter --plan | C090 | Repository-wide chapter boundary: read-only operation preview or authored chapter/account plan input. | N209; P:948 | keep → --plan | LOO-298; adopted target; implementation not implied |
| A334 | lf wave new-chapter --dry-run | C090 | Repository-wide chapter boundary: read-only preview of the selected mutation. | N213; P:950 | keep → --dry-run | LOO-298; adopted target; implementation not implied |
| A335 | lf wave new-chapter --json | C090 | Shared json protocol applied to repository-wide chapter boundary. | N165; P:952 | keep → --json | LOO-298; adopted target; implementation not implied |
| A336 | lf wave new-chapter --help / -h | C090 | Shared help protocol applied to repository-wide chapter boundary. | N155; P:941 | keep → --help | LOO-298; adopted target; implementation not implied |
| A337 | lf wave history --wave / -w | C091 | Recorded repository chapter boundaries: Wave target or scope. | N250; P:957 | keep → --wave | LOO-298; adopted target; implementation not implied |
| A338 | lf wave history --json | C091 | Shared json protocol applied to recorded repository chapter boundaries. | N165; P:959 | keep → --json | LOO-298; adopted target; implementation not implied |
| A339 | lf wave history --help / -h | C091 | Shared help protocol applied to recorded repository chapter boundaries. | N155; P:955 | keep → --help | LOO-298; adopted target; implementation not implied |
| A340 | lf wave update-plan --wave / -w | C092 | Content of the current chapter plan: Wave target or scope. | N250; P:964 | keep → --wave | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A341 | lf wave update-plan --plan | C092 | Content of the current chapter plan: read-only operation preview or authored chapter/account plan input. | N209; P:966 | keep → --plan | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A342 | lf wave update-plan --help / -h | C092 | Shared help protocol applied to content of the current chapter plan. | N155; P:962 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A349 | lf task --help / -h | C095 | Shared help protocol applied to concrete work and its delivery namespace. | N155; P:365 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A350 | lf task __worker <task_id> | C096 | Captured FlowSession driver admission: stable Task ownership of the captured child. | N264; P:974 | merge into → lf flow resume <task_id> | LOO-298; LOO-334; adopted target; implementation not implied |
| A351 | lf task __worker --help / -h | C096 | Shared help protocol applied to captured flowsession driver admission. | N264; P:974 | merge into → lf flow resume --help | LOO-298; LOO-334; adopted target; implementation not implied |
| A352 | lf task checkout <issue> | C097 | Allocation or recovery of a tracked Task checkout without execution: Task issue identity. | N265; P:977 | keep → <issue> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A353 | lf task checkout --name | C097 | Allocation or recovery of a tracked Task checkout without execution: owner name or local checkout/slug identity. | N170; P:979 | keep → --name | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A354 | lf task checkout --stack-on | C097 | Allocation or recovery of a tracked Task checkout without execution: Task whose active PR supplies the checkout base. | N266; P:982 | keep → --stack-on | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A355 | lf task checkout --directive | C097 | Allocation or recovery of a tracked Task checkout without execution: additional execution direction. | N267; P:984 | keep → --directive | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A356 | lf task checkout --json | C097 | Shared json protocol applied to allocation or recovery of a tracked task checkout without execution. | N165; P:986 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A357 | lf task checkout --help / -h | C097 | Shared help protocol applied to allocation or recovery of a tracked task checkout without execution. | N155; P:976 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A358 | lf task run <issue> | C098 | Execution of a Task-selected Flow: Task issue identity. | N268; P:990 | merge into → lf --as task:TASK flow <issue> | LOO-298; Jack Heart 2026-09-30; demo/integration choice open; see destination audit |
| A359 | lf task run --name | C098 | Execution of a Task-selected Flow: owner name or local checkout/slug identity. | N268; P:992 | merge into → lf --as task:TASK flow --name | LOO-298; Jack Heart 2026-09-30; demo/integration choice open; see destination audit |
| A360 | lf task run --flow | C098 | Execution of a Task-selected Flow: selected authored workflow. | N268; P:995 | merge into → lf --as task:TASK flow NAME | LOO-298; Jack Heart 2026-09-30; demo/integration choice open; see destination audit |
| A361 | lf task run --stack-on | C098 | Execution of a Task-selected Flow: Task whose active PR supplies the checkout base. | N268; P:998 | merge into → lf --as task:TASK flow --stack-on | LOO-298; Jack Heart 2026-09-30; demo/integration choice open; see destination audit |
| A362 | lf task run --directive | C098 | Execution of a Task-selected Flow: additional execution direction. | N268; P:1000 | merge into → lf --as task:TASK flow --directive | LOO-298; Jack Heart 2026-09-30; demo/integration choice open; see destination audit |
| A363 | lf task run --reason | C098 | Execution of a Task-selected Flow: durable explanation for disposition or retry. | N268; P:1003 | merge into → lf --as task:TASK flow --reason | LOO-298; Jack Heart 2026-09-30; demo/integration choice open; see destination audit |
| A364 | lf task run --json | C098 | Shared json protocol applied to execution of a task-selected flow. | N268; P:1005 | merge into → lf --as task:TASK flow --json | LOO-298; Jack Heart 2026-09-30; demo/integration choice open; see destination audit |
| A365 | lf task run --help / -h | C098 | Shared help protocol applied to execution of a task-selected flow. | N268; P:989 | merge into → lf --as task:TASK flow --help | LOO-298; Jack Heart 2026-09-30; demo/integration choice open; see destination audit |
| A366 | lf task create --wave | C099 | Creation of a planning Task with optional execution: Wave target or scope. | N250; P:1011 | keep → --wave | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A367 | lf task create --title | C099 | Creation of a planning Task with optional execution: authored title for the selected planning or review object. | N197; P:1014 | keep → --title | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A368 | lf task create --notes | C099 | Creation of a planning Task with optional execution: authored release or Task narrative. | N242; P:1017 | keep → --notes | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A369 | lf task create --run | C099 | Creation of a planning Task with optional execution: selected execution record or requested immediate Task execution. | N269; P:1020 | keep → --run | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A370 | lf task create --name | C099 | Creation of a planning Task with optional execution: owner name or local checkout/slug identity. | N170; P:1022 | keep → --name | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A371 | lf task create --flow | C099 | Creation of a planning Task with optional execution: selected authored workflow. | N175; P:1025 | keep → --flow | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A372 | lf task create --stack-on | C099 | Creation of a planning Task with optional execution: Task whose active PR supplies the checkout base. | N266; P:1028 | keep → --stack-on | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A373 | lf task create --json | C099 | Shared json protocol applied to creation of a planning task with optional execution. | N165; P:1030 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A374 | lf task create --help / -h | C099 | Shared help protocol applied to creation of a planning task with optional execution. | N155; P:1008 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A375 | lf task status <issue> | C100 | One Task's disposition, execution evidence and available actions: Task issue identity. | N265; P:1035 | keep → <issue> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A376 | lf task status --json | C100 | Shared json protocol applied to one task's disposition, execution evidence and available actions. | N165; P:1037 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A377 | lf task status --help / -h | C100 | Shared help protocol applied to one task's disposition, execution evidence and available actions. | N155; P:1033 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A378 | lf task changes <issue> | C101 | Checkout comparison against a selected base: Task issue identity. | N270; P:1041 | merge into → lf task diff --files <issue> | LOO-327; LOO-338; implemented on task diff --files; existing DTO preserved |
| A379 | lf task changes --base | C101 | Checkout comparison against a selected base: commit baseline for checkout inspection. | N270; P:1043 | merge into → lf task diff --files --base | LOO-327; LOO-338; implemented on task diff --files; existing DTO preserved |
| A380 | lf task changes --json | C101 | Shared json protocol applied to checkout comparison against a selected base. | N270; P:1045 | merge into → lf task diff --files --json | LOO-327; LOO-338; implemented on task diff --files; existing DTO preserved |
| A381 | lf task changes --help / -h | C101 | Shared help protocol applied to checkout comparison against a selected base. | N270; P:1040 | merge into → lf task diff --files --help | LOO-327; LOO-338; implemented on task diff --files; existing DTO preserved |
| A382 | lf task diff <issue> | C102 | Inspection of checkout changes against a Task's base: Task issue identity. | N265; P:1049 | keep → <issue> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A383 | lf task diff <path> | C102 | Inspection of checkout changes against a Task's base: command/definition path or checkout file path. | N271; P:1050 | keep → <path> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A384 | lf task diff --base | C102 | Inspection of checkout changes against a Task's base: commit baseline for checkout inspection. | N272; P:1052 | keep → --base | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A385 | lf task diff --draft | C102 | Inspection of checkout changes against a Task's base: in-memory file content compared without writing the checkout. | N273; P:1055 | keep → --draft | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A386 | lf task diff --json | C102 | Shared json protocol applied to inspection of checkout changes against a task's base. | N165; P:1057 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A387 | lf task diff --help / -h | C102 | Shared help protocol applied to inspection of checkout changes against a task's base. | N155; P:1048 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A388 | lf task file <issue> | C103 | Revision-bearing content of one file and its recovery versions: Task issue identity. | N265; P:1061 | keep → <issue> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A389 | lf task file <path> | C103 | Revision-bearing content of one file and its recovery versions: command/definition path or checkout file path. | N271; P:1062 | keep → <path> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A390 | lf task file --recoveries | C103 | Revision-bearing content of one file and its recovery versions: retained file versions for recovery inspection. | N274; P:1065 | keep → --recoveries | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A391 | lf task file --json | C103 | Shared json protocol applied to revision-bearing content of one file and its recovery versions. | N165; P:1067 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A392 | lf task file --help / -h | C103 | Shared help protocol applied to revision-bearing content of one file and its recovery versions. | N155; P:1060 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A393 | lf task save <issue> | C104 | Conditional replacement of file content with retained recovery bytes: Task issue identity. | N265; P:1071 | keep → <issue> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A394 | lf task save <path> | C104 | Conditional replacement of file content with retained recovery bytes: command/definition path or checkout file path. | N271; P:1072 | keep → <path> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A395 | lf task save --revision | C104 | Conditional replacement of file content with retained recovery bytes: expected file revision for conditional replacement. | N275; P:1074 | keep → --revision | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A396 | lf task save --json | C104 | Shared json protocol applied to conditional replacement of file content with retained recovery bytes. | N165; P:1076 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A397 | lf task save --help / -h | C104 | Shared help protocol applied to conditional replacement of file content with retained recovery bytes. | N155; P:1070 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A398 | lf task complete <issue> | C105 | Successful disposition of a Task after delivery settlement: Task issue identity. | N265; P:1080 | keep → <issue> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A399 | lf task complete --summary | C105 | Successful disposition of a Task after delivery settlement: durable completion, readiness or verdict explanation. | N172; P:1082 | keep → --summary | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A400 | lf task complete --json | C105 | Shared json protocol applied to successful disposition of a task after delivery settlement. | N165; P:1084 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A401 | lf task complete --help / -h | C105 | Shared help protocol applied to successful disposition of a task after delivery settlement. | N155; P:1079 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A402 | lf task delete <issue> | C106 | Deletion of the shared planning issue with retained execution history: Task issue identity. | N265; P:1087 | keep → <issue> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A403 | lf task delete --help / -h | C106 | Shared help protocol applied to deletion of the shared planning issue with retained execution history. | N155; P:1087 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A404 | lf task edit <issue> | C107 | Current planning title and description of a Task: Task issue identity. | N265; P:1090 | keep → <issue> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A405 | lf task edit --title | C107 | Current planning title and description of a Task: authored title for the selected planning or review object. | N197; P:1092 | keep → --title | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A406 | lf task edit --notes | C107 | Current planning title and description of a Task: authored release or Task narrative. | N242; P:1094 | keep → --notes | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A407 | lf task edit --wave / -w | C107 | Current planning title and description of a Task: Wave target or scope. | N250; P:1096 | keep → --wave | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A408 | lf task edit --help / -h | C107 | Shared help protocol applied to current planning title and description of a task. | N155; P:1089 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A409 | lf task comment <issue> | C108 | Append-only direction and discussion for a Task: Task issue identity. | N265; P:1100 | keep → <issue> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A410 | lf task comment <message> | C108 | Append-only direction and discussion for a Task: commit content or appended discussion text. | N204; P:1101 | keep → <message> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A411 | lf task comment --wave / -w | C108 | Append-only direction and discussion for a Task: Wave target or scope. | N250; P:1103 | keep → --wave | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A412 | lf task comment --json | C108 | Shared json protocol applied to append-only direction and discussion for a task. | N165; P:1105 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A413 | lf task comment --help / -h | C108 | Shared help protocol applied to append-only direction and discussion for a task. | N155; P:1099 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A414 | lf task interrupt <issue> | C109 | Interruption of a Task's current provider turn: Task issue identity. | N265; P:1109 | keep → <issue> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A415 | lf task interrupt --json | C109 | Shared json protocol applied to interruption of a task's current provider turn. | N165; P:1111 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A416 | lf task interrupt --help / -h | C109 | Shared help protocol applied to interruption of a task's current provider turn. | N155; P:1108 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A417 | lf task wait <issue> | C110 | Waiting for an observable Task condition: Task issue identity. | N265; P:1115 | keep → <issue> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A418 | lf task wait --until | C110 | Waiting for an observable Task condition: Task condition whose observation ends the wait. | N276; P:1117 | keep → --until | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A419 | lf task wait --timeout | C110 | Waiting for an observable Task condition: maximum wait for an observable condition. | N255; P:1119 | keep → --timeout | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A420 | lf task wait --json | C110 | Shared json protocol applied to waiting for an observable task condition. | N165; P:1121 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A421 | lf task wait --help / -h | C110 | Shared help protocol applied to waiting for an observable task condition. | N155; P:1114 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A422 | lf task restart <issue> | C111 | Explicit replacement of captured Flow execution: Task issue identity. | N277; P:1126 | merge into → lf flow resume --restart <issue> | LOO-298; adopted target; implementation not implied |
| A423 | lf task restart <advice> | C111 | Explicit replacement of captured Flow execution: direction explaining replacement of the captured Task invocation. | N277; P:1127 | merge into → lf flow resume --restart <advice> | LOO-298; adopted target; implementation not implied |
| A424 | lf task restart --flow | C111 | Explicit replacement of captured Flow execution: selected authored workflow. | N277; P:1130 | merge into → lf flow resume --restart --flow | LOO-298; adopted target; implementation not implied |
| A425 | lf task restart --json | C111 | Shared json protocol applied to explicit replacement of captured flow execution. | N277; P:1132 | merge into → lf flow resume --restart --json | LOO-298; adopted target; implementation not implied |
| A426 | lf task restart --help / -h | C111 | Shared help protocol applied to explicit replacement of captured flow execution. | N277; P:1125 | merge into → lf flow resume --restart --help | LOO-298; adopted target; implementation not implied |
| A427 | lf tokens --json | C112 | Shared json protocol applied to tracked source size and history measured in lines and tokens. | N165; P:373 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A428 | lf tokens --days | C112 | Tracked source size and history measured in lines and tokens: observation window, retaining all-time semantics where supported. | N253; P:376 | keep → --days | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A429 | lf tokens --help / -h | C112 | Shared help protocol applied to tracked source size and history measured in lines and tokens. | N155; P:370 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A430 | lf usage --json | C113 | Shared json protocol applied to provider-authored consumption attributed to work. | N165; P:382 | keep → --json | LOO-298; LOO-336; adopted target; implementation not implied |
| A431 | lf usage --days | C113 | Provider-authored consumption attributed to work: observation window, retaining all-time semantics where supported. | N253; P:385 | keep → --days | LOO-298; LOO-336; adopted target; implementation not implied |
| A432 | lf usage --wave | C113 | Provider-authored consumption attributed to work: Wave target or scope. | N250; P:388 | keep → --wave | LOO-298; LOO-336; adopted target; implementation not implied |
| A433 | lf usage --project | C113 | Provider-authored consumption attributed to work: historical Project attribution filter inside a Wave. | N278; P:391 | keep → --project | LOO-298; LOO-336; adopted target; implementation not implied |
| A434 | lf usage --task | C113 | Provider-authored consumption attributed to work: Task target or scope. | N279; P:394 | keep → --task | LOO-298; LOO-336; adopted target; implementation not implied |
| A435 | lf usage --help / -h | C113 | Shared help protocol applied to provider-authored consumption attributed to work. | N155; P:379 | keep → --help | LOO-298; LOO-336; adopted target; implementation not implied |
| A436 | lf __telemetry-scorecard --json | C114 | Shared json protocol applied to repository maintainer metric evaluation from recorded evidence. | N165; P:401 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A437 | lf __telemetry-scorecard --help / -h | C114 | Shared help protocol applied to repository maintainer metric evaluation from recorded evidence. | N155; P:398 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A438 | lf ci --since | C115 | Home-wide history of failed-CI response and settlement: start of the observation window. | N280; P:407 | keep → --since | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A439 | lf ci --wave | C115 | Home-wide history of failed-CI response and settlement: Wave target or scope. | N250; P:410 | keep → --wave | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A440 | lf ci --repo | C115 | Home-wide history of failed-CI response and settlement: repository routing or observation scope. | N236; P:413 | keep → --repo | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A441 | lf ci --json | C115 | Shared json protocol applied to home-wide history of failed-ci response and settlement. | N165; P:416 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A442 | lf ci --help / -h | C115 | Shared help protocol applied to home-wide history of failed-ci response and settlement. | N155; P:404 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A443 | lf ps --json | C116 | Shared json protocol applied to one current process observation. | N165; P:422 | keep → --json | Jack Heart 2026-09-30; LOO-298; adopted target; implementation not implied |
| A444 | lf ps --help / -h | C116 | Shared help protocol applied to one current process observation. | N155; P:419 | keep → --help | Jack Heart 2026-09-30; LOO-298; adopted target; implementation not implied |
| A445 | lf top --json | C117 | Shared json protocol applied to continuous process monitoring. | N165; P:428 | keep → --json | Jack Heart 2026-09-30; LOO-298; adopted target; implementation not implied |
| A446 | lf top --help / -h | C117 | Shared help protocol applied to continuous process monitoring. | N155; P:425 | keep → --help | Jack Heart 2026-09-30; LOO-298; adopted target; implementation not implied |
| A447 | lf prune --dry-run | C118 | Cleanup of proven owned orphan processes and stale process receipts: read-only preview of the selected mutation. | N213; P:434 | keep → --dry-run | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A448 | lf prune --json | C118 | Shared json protocol applied to cleanup of proven owned orphan processes and stale process receipts. | N165; P:437 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A449 | lf prune --help / -h | C118 | Shared help protocol applied to cleanup of proven owned orphan processes and stale process receipts. | N155; P:431 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A450 | lf doctor --planning | C119 | Diagnosis of ledger, installation and planning consistency: diagnosis of repository planning consistency without writes. | N281; P:443 | keep → --planning | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A451 | lf doctor --json | C119 | Shared json protocol applied to diagnosis of ledger, installation and planning consistency. | N165; P:446 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A452 | lf doctor --help / -h | C119 | Shared help protocol applied to diagnosis of ledger, installation and planning consistency. | N155; P:440 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A453 | lf list <path> | C120 | Discovery of executable commands and authored definitions: command/definition path or checkout file path. | N271; P:450 | keep → <path> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A454 | lf list --json | C120 | Shared json protocol applied to discovery of executable commands and authored definitions. | N165; P:452 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A455 | lf list --help / -h | C120 | Shared help protocol applied to discovery of executable commands and authored definitions. | N155; P:449 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A456 | lf help <path> | C121 | Read-only explanation and validation of commands and authored definitions: command/definition path or checkout file path. | N271; P:456 | keep → <path> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A457 | lf help --all | C121 | Read-only explanation and validation of commands and authored definitions: scope across all repositories or all locally authored Waves. | N166; P:458 | keep → --all | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A458 | lf help --help / -h | C121 | Shared help protocol applied to read-only explanation and validation of commands and authored definitions. | N155; P:455 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A459 | lf roadmap --wave | C122 | Repository portfolio of Tasks and unlinked checkout/PR work: Wave target or scope. | N250; P:467 | keep → --wave | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A460 | lf roadmap --json | C122 | Shared json protocol applied to repository portfolio of tasks and unlinked checkout/pr work. | N165; P:470 | keep → --json | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A461 | lf roadmap --all | C122 | Repository portfolio of Tasks and unlinked checkout/PR work: scope across all repositories or all locally authored Waves. | N166; P:473 | keep → --all | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A462 | lf roadmap --help / -h | C122 | Shared help protocol applied to repository portfolio of tasks and unlinked checkout/pr work. | N155; P:464 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A463 | lf activity --since | C123 | Chronological changes in durable Work, delivery and direction: start of the observation window. | N280; P:479 | keep → --since | LOO-298; LOO-334; adopted target; implementation not implied |
| A464 | lf activity --limit | C123 | Chronological changes in durable Work, delivery and direction: bounded row count with explicit truncation evidence. | N282; P:482 | keep → --limit | LOO-298; LOO-334; adopted target; implementation not implied |
| A465 | lf activity --wave | C123 | Chronological changes in durable Work, delivery and direction: Wave target or scope. | N250; P:485 | keep → --wave | LOO-298; LOO-334; adopted target; implementation not implied |
| A466 | lf activity --project | C123 | Chronological changes in durable Work, delivery and direction: historical Project attribution filter inside a Wave. | N278; P:488 | keep → --project | LOO-298; LOO-334; adopted target; implementation not implied |
| A467 | lf activity --task | C123 | Chronological changes in durable Work, delivery and direction: Task target or scope. | N279; P:491 | keep → --task | LOO-298; LOO-334; adopted target; implementation not implied |
| A468 | lf activity --json | C123 | Shared json protocol applied to chronological changes in durable work, delivery and direction. | N165; P:494 | keep → --json | LOO-298; LOO-334; adopted target; implementation not implied |
| A469 | lf activity --help / -h | C123 | Shared help protocol applied to chronological changes in durable work, delivery and direction. | N155; P:476 | keep → --help | LOO-298; LOO-334; adopted target; implementation not implied |
| A470 | lf runs --active | C124 | Execution and conversation evidence inspection: selection of current provider discovery rather than recorded history. | N283; P:500 | merge into → lf monitor --active | LOO-298; Jack Heart 2026-09-30; selected without demo by latest steer; requires LOO-298 owner integration; see post-rebase decisions |
| A471 | lf runs --watch | C124 | Execution and conversation evidence inspection: continuous observation of this result. | N283; P:503 | merge into → lf monitor --watch | LOO-298; Jack Heart 2026-09-30; selected without demo by latest steer; requires LOO-298 owner integration; see post-rebase decisions |
| A472 | lf runs <run> | C124 | Execution and conversation evidence inspection: selected execution record or requested immediate Task execution. | N283; P:506 | merge into → lf monitor <run> | LOO-298; Jack Heart 2026-09-30; selected without demo by latest steer; requires LOO-298 owner integration; see post-rebase decisions |
| A473 | lf runs --parent | C124 | Execution and conversation evidence inspection: execution ancestry or parent Wave conversation target. | N283; P:509 | merge into → lf monitor --parent | LOO-298; Jack Heart 2026-09-30; selected without demo by latest steer; requires LOO-298 owner integration; see post-rebase decisions |
| A474 | lf runs --events | C124 | Execution and conversation evidence inspection: append-only evidence for the selected execution/session. | N283; P:516 | merge into → lf monitor --events | LOO-298; Jack Heart 2026-09-30; selected without demo by latest steer; requires LOO-298 owner integration; see post-rebase decisions |
| A475 | lf runs --final | C124 | Execution and conversation evidence inspection: last durable provider conclusion for the selected execution/session. | N283; P:523 | merge into → lf monitor --final | LOO-298; Jack Heart 2026-09-30; selected without demo by latest steer; requires LOO-298 owner integration; see post-rebase decisions |
| A476 | lf runs --resume | C124 | Execution and conversation evidence inspection: provider continuation duplicated outside its Session owner. | N283; P:530 | merge into → lf monitor --resume | LOO-298; Jack Heart 2026-09-30; selected without demo by latest steer; requires LOO-298 owner integration; see post-rebase decisions |
| A477 | lf runs --task | C124 | Execution and conversation evidence inspection: Task target or scope. | N283; P:533 | merge into → lf monitor --task | LOO-298; Jack Heart 2026-09-30; selected without demo by latest steer; requires LOO-298 owner integration; see post-rebase decisions |
| A478 | lf runs --project | C124 | Execution and conversation evidence inspection: historical Project attribution filter inside a Wave. | N283; P:536 | merge into → lf monitor --project | LOO-298; Jack Heart 2026-09-30; selected without demo by latest steer; requires LOO-298 owner integration; see post-rebase decisions |
| A479 | lf runs --wave | C124 | Execution and conversation evidence inspection: Wave target or scope. | N283; P:539 | merge into → lf monitor --wave | LOO-298; Jack Heart 2026-09-30; selected without demo by latest steer; requires LOO-298 owner integration; see post-rebase decisions |
| A480 | lf runs --json | C124 | Shared json protocol applied to execution and conversation evidence inspection. | N283; P:542 | merge into → lf monitor --json | LOO-298; Jack Heart 2026-09-30; selected without demo by latest steer; requires LOO-298 owner integration; see post-rebase decisions |
| A481 | lf runs --help / -h | C124 | Shared help protocol applied to execution and conversation evidence inspection. | N283; P:497 | merge into → lf monitor --help | LOO-298; Jack Heart 2026-09-30; selected without demo by latest steer; requires LOO-298 owner integration; see post-rebase decisions |
| A482 | lf replay <run> | C125 | New execution from captured provider inputs: selected execution record or requested immediate Task execution. | N269; P:547 | keep → <run> | LOO-298; Intelligence memory; adopted target; implementation not implied |
| A483 | lf replay --help / -h | C125 | Shared help protocol applied to new execution from captured provider inputs. | N155; P:545 | keep → --help | LOO-298; Intelligence memory; adopted target; implementation not implied |
| A498 | lf op <removed> | C128 | Retired rejection-only namespace with no remaining operation: operand of a retired rejection shim. | N284; P:600 | delete → — | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A499 | lf op <rest> | C128 | Retired rejection-only namespace with no remaining operation: discarded passthrough of a retired rejection shim. | N284; P:602 | delete → — | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A500 | lf op --help / -h | C128 | Shared help protocol applied to retired rejection-only namespace with no remaining operation. | N284; P:598 | delete → — | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A501 | lf ssh --account | C129 | Execution on a selected Home through an explicit SSH transport: ordered account preference carried to the destination. | N285; P:610 | keep → --account | LOO-340; adopted target; implementation not implied |
| A502 | lf ssh --only-account | C129 | Execution on a selected Home through an explicit SSH transport: hard account-spending boundary carried to the destination. | N286; P:610 | keep → --only-account | LOO-340; adopted target; implementation not implied |
| A503 | lf ssh <target> | C129 | Execution on a selected Home through an explicit SSH transport: release product or SSH destination selected by this operation. | N239; P:628 | keep → <target> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A504 | lf ssh --repo | C129 | Execution on a selected Home through an explicit SSH transport: repository routing or observation scope. | N236; P:631 | keep → --repo | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A505 | lf ssh --secret | C129 | Execution on a selected Home through an explicit SSH transport: named Doppler-backed capability forwarded to a destination. | N287; P:635 | keep → --secret | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A506 | lf ssh --forward-agent | C129 | Execution on a selected Home through an explicit SSH transport: explicit forwarding of SSH signing authority. | N288; P:639 | keep → --forward-agent | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A507 | lf ssh <lf_args> | C129 | Execution on a selected Home through an explicit SSH transport: destination invocation argv after the transport boundary. | N289; P:643 | keep → <lf_args> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A508 | lf ssh --help / -h | C129 | Shared help protocol applied to execution on a selected home through an explicit ssh transport. | N155; P:610 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A509 | lf run <name> | C130 | Explicit flow-first execution of a named definition: owner name or local checkout/slug identity. | N170; P:647 | keep → <name> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A510 | lf run <args> | C130 | Explicit flow-first execution of a named definition: arguments passed to an authored definition. | N290; P:649 | keep → <args> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A511 | lf run --help / -h | C130 | Shared help protocol applied to explicit flow-first execution of a named definition. | N155; P:646 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A512 | lf flow --help / -h | C131 | Shared help protocol applied to workflow execution and captured navigation namespace. | N155; P:652 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A513 | lf flow list --json | C132 | Shared json protocol applied to saved flowsession inventory. | N165; P:685 | keep → lf flow list --json | LOO-298; adopted target; implementation not implied |
| A514 | lf flow list --help / -h | C132 | Shared help protocol applied to saved flowsession inventory. | N155; P:683 | keep → --help | LOO-298; adopted target; implementation not implied |
| A515 | lf flow show <name> | C133 | Saved FlowSession graph and progress: owner name or local checkout/slug identity. | N170; P:688 | keep → <name> | LOO-298; adopted target; implementation not implied |
| A516 | lf flow show --help / -h | C133 | Shared help protocol applied to saved flowsession graph and progress. | N155; P:688 | keep → --help | LOO-298; adopted target; implementation not implied |
| A517 | lf flow validate <name> | C134 | Read-only authored Flow compilation: owner name or local checkout/slug identity. | N291; P:690 | merge into → lf help flow <name> | LOO-338; Jack Heart 2026-09-30; implemented after main rebase; focused public CLI proof |
| A518 | lf flow validate --help / -h | C134 | Shared help protocol applied to read-only authored flow compilation. | N291; P:690 | merge into → lf help flow --help | LOO-338; Jack Heart 2026-09-30; implemented after main rebase; focused public CLI proof |
| A519 | lf flow decide <decision> | C135 | Retired command-side decision verdict: verdict for a captured repeat boundary. | N292; P:694 | delete → — | LOO-298; adopted target; implementation not implied |
| A520 | lf flow decide <summary> | C135 | Retired command-side decision verdict: durable completion, readiness or verdict explanation. | N292; P:696 | delete → — | LOO-298; adopted target; implementation not implied |
| A521 | lf flow decide --help / -h | C135 | Shared help protocol applied to retired command-side decision verdict. | N292; P:692 | delete → — | LOO-298; adopted target; implementation not implied |
| A522 | lf flow route <path> | C136 | Retired command-side branch verdict: command/definition path or checkout file path. | N293; P:699 | delete → — | LOO-298; adopted target; implementation not implied |
| A523 | lf flow route --help / -h | C136 | Shared help protocol applied to retired command-side branch verdict. | N293; P:699 | delete → — | LOO-298; adopted target; implementation not implied |
| A524 | lf flow blocked <reason> | C137 | Retired command-side blocked verdict: durable explanation for disposition or retry. | N294; P:703 | delete → — | LOO-298; adopted target; implementation not implied |
| A525 | lf flow blocked --help / -h | C137 | Shared help protocol applied to retired command-side blocked verdict. | N294; P:701 | delete → — | LOO-298; adopted target; implementation not implied |
| A526 | lf flow resume <invocation> | C138 | Continuation of a captured ordinary workflow invocation: captured workflow identity. | N295; P:707 | keep → <invocation> | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A527 | lf flow resume --retry | C138 | Continuation of a captured ordinary workflow invocation: explicit retry of a failed captured boundary. | N296; P:709 | keep → --retry | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A528 | lf flow resume --help / -h | C138 | Shared help protocol applied to continuation of a captured ordinary workflow invocation. | N155; P:706 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A529 | lf skill --help / -h | C139 | Shared help protocol applied to explicit execution of a named skill despite kind or command collisions. | N155; P:657 | keep → --help | LOO-338 catalog and primary-source research; adopted target; implementation not implied |
| A530 | lf skill list <namespace> | C140 | Authored skill inventory: skill namespace selected for discovery. | N297; P:670 | merge into → lf list skill <namespace> | LOO-338; implemented in skill catalog merge; public CLI discovery proof; full Task incomplete |
| A531 | lf skill list --json | C140 | Shared json protocol applied to authored skill inventory. | N297; P:672 | merge into → lf list skill --json | LOO-338; implemented in skill catalog merge; public CLI discovery proof; full Task incomplete |
| A532 | lf skill list --help / -h | C140 | Shared help protocol applied to authored skill inventory. | N297; P:669 | merge into → lf list skill --help | LOO-338; implemented in skill catalog merge; public CLI discovery proof; full Task incomplete |
| A533 | lf skill show <name> | C141 | Read-only skill definition inspection: owner name or local checkout/slug identity. | N298; P:675 | merge into → lf help skill <name> | LOO-338; adopted target; implementation not implied |
| A534 | lf skill show --help / -h | C141 | Shared help protocol applied to read-only skill definition inspection. | N298; P:675 | merge into → lf help skill --help | LOO-338; adopted target; implementation not implied |

## Rationale key

- **N000**: Concept: Default direct provider conversation and command entry point. Bare launch versus help: keep existing launch endpoint; startup help must describe it.
- **N001**: Concept: Configured participant display name. Owns Inspect the current user; no equivalent operation identified.
- **N002**: The user namespace has one operation, reading the participant name. Put its JSON/text reader on the object; delete the name leaf.
- **N003**: Concept: Execution of caller-authored prompt text. Owns Run an inline prompt; no equivalent operation identified.
- **N004**: Concept: Desktop application activation. Owns Open or focus Loopflow.app; no equivalent operation identified.
- **N005**: Concept: Raster capture of a page with a bounded browser lifetime. Owns Capture a URL or local HTML file without claiming the user's browser; no equivalent operation identified.
- **N006**: Concept: Capture-child lifetime after its owning caller disappears. Owns Internal owner-loss supervisor for one browser capture; no equivalent operation identified.
- **N007**: Concept: Attribution of a provider-native conversation to its execution. Owns Internal provider callback that records one native interactive session; no equivalent operation identified.
- **N008**: Concept: Synchronous request for a new durable review conversation. Creates a review Session; session open continues an existing conversation.
- **N009**: Concept: Conversation discovery and lifecycle namespace. Owns Inspect and continue Sessions; no equivalent operation identified.
- **N010**: Concept: Resumable conversations and pending review obligations. Owns List Sessions; no equivalent operation identified.
- **N011**: LOO-298 converges live attachment and native resume on connect. Delete its transitional open alias after migrating callers; do not add another lifecycle.
- **N012**: Concept: Completion of a conversation's review obligation. Owns Complete a review, blocked Ask, or interactive session; no equivalent operation identified.
- **N013**: Concept: Conversation title with explicit authorship precedence. Owns Rename a Session; a human name is never replaced by a suggestion; no equivalent operation identified.
- **N014**: Concept: Author readiness for a pending review without completing it. Author reports readiness; complete requires review completion authority.
- **N015**: Concept: Execution of one captured Flow review boundary. Owns Run the exact review skill in its durable terminal; no equivalent operation identified.
- **N016**: Concept: Execution of one captured ad-hoc review request. Owns Run one ad-hoc request in its durable terminal; no equivalent operation identified.
- **N017**: An Exec is an lf process; this callback stops a native client selected by capture identity. Calling it stop-exec would misstate its signal authority.
- **N018**: Concept: Selection and activation of a verified machine installation. Published machine installation; rebase refreshes a source checkout.
- **N019**: Concept: Automatic activation of published software on a machine schedule. Owns Install the latest Loopflow at login and weekly by default (macOS launchd); no equivalent operation identified.
- **N020**: Concept: Recovery of an interrupted installation selection transaction. Owns Continue one interrupted machine install switch from its pinned candidate; no equivalent operation identified.
- **N021**: Concept: Published-candidate compatibility with the selected installation store. Owns Preview whether this build may replace the global lf (read-only). Reads the shared store's migration frontier and validates executable planning references against this binary; mutates nothing and exits non-zero on refusal so a caller can gate on it; no equivalent operation identified.
- **N022**: Concept: Development-candidate compatibility with an explicitly retained private store. Owns Validate this exact local candidate against one receipt-selected store; no equivalent operation identified.
- **N023**: Concept: Receipt-authorized schema advancement by the pinned candidate. Owns Advance the receipt-selected store with this exact candidate's registry; no equivalent operation identified.
- **N024**: Concept: Atomic activation of a specific candidate's CLI, daemon, app and store. Owns Promote this build to the global CLI: content-address it into ~/.lf/bin and atomically repoint the target symlink, under the exclusive promotion lock. Refuses — leaving every target unchanged — on incompatible schema or persisted executable evidence; no equivalent operation identified.
- **N025**: Concept: Restoration of retained installed bytes compatible with the existing store. Owns Repoint the global CLI at retained prior bytes only after that binary's own preflight proves it recognizes the current store frontier; no equivalent operation identified.
- **N026**: Concept: Current branch's pull request and its delivery lifecycle. Owns Pull request lifecycle; no equivalent operation identified.
- **N027**: Concept: Required check outcomes and failure logs for one pull request. CI detail and watch differ from single PR summary.
- **N028**: Bare pr already invokes the same status reader. One summary endpoint retains missing-PR and provider-error distinctions.
- **N029**: Concept: Continuation of a Task's serial PR chain after a settled merge. Rotates settled PR chain; --next on delivery records future continuation intent.
- **N030**: Concept: Publication of a ready review without merge intent. Ready PR publication differs from draft/open and from merge intent.
- **N031**: Concept: Draft preparation and presentation for browser review. Publishes a draft and opens browser; publish produces ready review. Demo choice R08.
- **N032**: Concept: Preparation for a reviewer's future merge decision. Prepares review without merge intent; arm/land request merge.
- **N033**: LOO-332 makes land return after recording exact-head merge/settlement intent. Arm becomes the same operation; do not add a watcher flag to preserve the retired supervisor.
- **N034**: Finite prepare/publish/merge-intent handoff; later scheduled checks own repair and settlement. A successful command means handoff, not merged.
- **N035**: Concept: Discarding branch and PR artifacts without deleting the planning Task. Discards branch/PR artifacts; task delete removes provider planning object.
- **N036**: Concept: Physical checkout lifecycle namespace. Owns Worktree operations; no equivalent operation identified.
- **N037**: Concept: Allocation of an untracked sibling checkout. Untracked checkout allocation; task checkout retains tracked ownership.
- **N038**: Concept: Selection of an existing physical checkout. Owns Switch to a worktree by name, identity leaf, or full branch; no equivalent operation identified.
- **N039**: Concept: Inventory of physical checkouts and their branch facts. Physical checkouts differ from Task portfolio.
- **N040**: Concept: Cleanup of eligible inactive or terminal physical checkouts. Owns Remove clean terminal or inactive worktrees; no equivalent operation identified.
- **N041**: Concept: Removal of one explicitly selected physical checkout. Owns Remove a worktree; no equivalent operation identified.
- **N042**: Concept: Integration of the current checkout with its selected Git base. Owns Rebase current branch onto target (default: main); no equivalent operation identified.
- **N043**: Concept: Local Git checkpoint of current changes. Owns Commit changes; no equivalent operation identified.
- **N044**: One account observation owner: live by default, explicit cached evidence. Names identify observed provider identities; capacity reports used and left with reset/age, not zero for unknown.
- **N045**: Bare Account already delegates to Status. One reader gains live default and explicit --cached from LOO-340, retains identity mismatch/duplicate evidence from LOO-339. Migrate status callers with it.
- **N046**: Concept: Revocation of local or managed provider credentials. Owns Disconnect local credentials or one managed login; no equivalent operation identified.
- **N047**: Concept: Acquisition of local or managed provider credentials. Authenticate/register a login; set edits metadata without authenticating.
- **N048**: Concept: Account metadata and remembered browser venues. Account properties/browser venues; route set orders spending accounts.
- **N049**: Concept: Explanation of ordered account selection for a routing scope. Owns Configure and inspect managed account routing; no equivalent operation identified.
- **N050**: Concept: Replacement of the ordered accounts in a routing scope. Owns Replace a provider's ordered route; no equivalent operation identified.
- **N051**: Default route inspection and its only read leaf are one concept. Keep set as the distinct mutation.
- **N052**: Concept: Repository release lifecycle namespace. Owns Release operations (run, check, notes, bump, tag, status); no equivalent operation identified.
- **N053**: Concept: Recovery or completion of a verified repository release. Whole release workflow; individual steps remain recovery/CI primitives.
- **N054**: Concept: Unreleased changes eligible for the next repository release. Owns Check if PRs have merged since the last tag; no equivalent operation identified.
- **N055**: Concept: Authored release narrative and its archival destination. Owns Generate release notes for a version; no equivalent operation identified.
- **N056**: Concept: Version values in repository manifests. Owns Bump version in manifest files; no equivalent operation identified.
- **N057**: Concept: Published Git version tag. Owns Create a git tag and push it; no equivalent operation identified.
- **N058**: Concept: Hosted release draft, artifacts and publication state. Release assets/publication, not Task PR publication.
- **N059**: Concept: Observed release workflow and hosted publication status. Owns Check release workflow status; no equivalent operation identified.
- **N060**: Concept: Repository administration namespace. Owns Repository provider administration; no equivalent operation identified.
- **N061**: Concept: Reconciliation of the repository's planning Team across Waves. Owns Reconcile linked Waves to the repository's Linear Team; no equivalent operation identified.
- **N062**: Concept: Machine placement and installation namespace. Owns Inspect this Home and observe routes to other Homes; no equivalent operation identified.
- **N063**: Concept: Stable execution-destination identity of this machine. Machine identity differs from participant name and provider account.
- **N064**: Concept: Observed network route to a known execution destination. Route evidence does not move Wave placement or establish remote liveness.
- **N065**: Concept: Export of authored skills into provider-native skill directories. Owns Compile loopflow skills into your home vendor Skills directories; no equivalent operation identified.
- **N066**: Schedule is a finite OS-triggered invocation, not resident lifecycle. Repository Task automation and Wave-specific cadence must share cron; reconcile namespace placement with LOO-332 before another move.
- **N067**: Concept: Installation of one manually declared scheduled invocation. Owns Install or replace a scheduled lf invocation; no equivalent operation identified.
- **N068**: Concept: Installed schedule inventory. Owns List installed loopflow cron jobs; no equivalent operation identified.
- **N069**: Concept: Read-only feasibility of declared Wave schedules on their owning Home. Owns Validate Home authority and declared jobs without changing launchd; no equivalent operation identified.
- **N070**: Concept: Reconciliation of installed jobs with authored Wave schedule declarations. Reconciles declared jobs; add/remove author manual entries; install schedule updates software.
- **N071**: Concept: Receipt-bearing execution of one scheduled invocation. Owns Execute one installed cron job and persist its terminal receipt; no equivalent operation identified.
- **N072**: Concept: Observed scheduled invocation outcomes. Owns Show durable cron receipts; no equivalent operation identified.
- **N073**: Concept: Explicit firing of an installed scheduled invocation. Owns Ask launchd to fire an installed job; no equivalent operation identified.
- **N074**: Concept: Removal of an installed scheduled invocation. Owns Uninstall a scheduled lf invocation; no equivalent operation identified.
- **N075**: Wave has purpose/files and a plan, no running/stopped lifecycle. Keep the object; remove server-era operations.
- **N076**: List authored goals including empty Waves and slash-qualified subwaves from the selected checkout. No resident live/stopped inventory; provider membership alone cannot create Waves.
- **N077**: Fold goal, chapter, KRs and Task facts into the scoped portfolio reader. Remove resident status; retain unavailable planning and exact Session/Exec references, not synthetic Wave liveness.
- **N078**: LOO-334 places provider/Team connection at repository scope. Wave-to-Initiative mapping is an input to that operation, not independent account/setup authority per Wave. Exact connection controls need integration with that work.
- **N079**: One repository planning acquisition owner; Wave is a filter. Refresh cannot manufacture local execution or recreate deleted authored goals.
- **N080**: Retain the operation, replace Initiative-label-only behavior with LOO-329/334 identity semantics. The sibling plans disagree about identity through rename; do not settle that policy here.
- **N081**: Wave existence comes from selected repository definitions. Deleting an empty registry row cannot delete a Wave and reads reimport it; remove this store-management operation. Preserve history.
- **N082**: Execution destination for scheduled work can survive without a resident. Coordinate LOO-332 repository schedules and LOO-334 cross-Home discovery before choosing whether place remains a Wave input or folds into cron. Do not infer signal authority from placement.
- **N083**: Stopped-resident relocation disappears. Rename owns directory/definition identity; execution placement remains separate under place. No duplicate slug/worktree relocation operation.
- **N084**: Remove registry retirement as an independent Wave-existence writer. Explicit authored definition removal and provider planning disposition retain their owners; this is not authorization to erase history or provider plans.
- **N085**: LOO-298 makes the chapter clock repository-wide. Preserve each Wave’s plan and transferred Task identity; do not keep a separate per-Wave boundary writer.
- **N086**: Repository chapter history with Wave filtering is the same boundary history, not a new Chapter namespace.
- **N087**: Concept: Content of the current chapter plan. Owns Replace the current chapter's KRs, targets, and Flow recommendation; no equivalent operation identified.
- **N088**: Concept: Concrete work and its delivery namespace. Owns Linear-backed Task work and bounded workers; no equivalent operation identified.
- **N089**: Task is an explicit Work declaration on the ordinary Flow driver. Migrate exact claim/placement and saved boundary consumption; delete a separate Task worker entry only after equivalent admission/recovery proof.
- **N090**: Concept: Allocation or recovery of a tracked Task checkout without execution. Allocation only, no worker launch; distinct from task run.
- **N091**: Jack identifies task run as shorthand for explicit Task declaration and ordinary Flow execution. No second progression owner; preserve saved Flow selection and launch placement. Whether to retain that extra spelling is a demo item under the no-alias rule.
- **N092**: Concept: Creation of a planning Task with optional execution. Owns File a Task in the current chapter; optionally prepare and run it; no equivalent operation identified.
- **N093**: Concept: One Task's disposition, execution evidence and available actions. Owns Show durable Task facts and current worker evidence; no equivalent operation identified.
- **N094**: Changes and diff read the same checkout/base. A files projection retains complete changed-path/rename/base DTOs; patch retains draft/binary evidence. Migrate the native file browser without extra reads or lost revision safety.
- **N095**: Concept: Inspection of checkout changes against a Task's base. Owns Show this Task's patch, optionally limited to one changed file; no equivalent operation identified.
- **N096**: Concept: Revision-bearing content of one file and its recovery versions. Owns Read one file from this Task's worktree; no equivalent operation identified.
- **N097**: Concept: Conditional replacement of file content with retained recovery bytes. Owns Save UTF-8 stdin with an expected revision and retained recovery files; no equivalent operation identified.
- **N098**: Concept: Successful disposition of a Task after delivery settlement. Task disposition; Session complete returns feedback and cannot substitute for it.
- **N099**: Concept: Deletion of the shared planning issue with retained execution history. Provider deletion with preserved local execution evidence; not interruption.
- **N100**: Concept: Current planning title and description of a Task. Planning title/notes; comment appends direction without rewriting description.
- **N101**: Concept: Append-only direction and discussion for a Task. Thread read/append; preserve read-only omission and explicit write input.
- **N102**: Concept: Interruption of a Task's current provider turn. Owns Interrupt the active provider turn; no equivalent operation identified.
- **N103**: Concept: Waiting for an observable Task condition. Owns Wait without polling an LM; no equivalent operation identified.
- **N104**: FlowSession owns the saved graph and continuation. Restart is an explicit replacement of that selection, independent of Task status, and must preserve prior history.
- **N105**: Concept: Tracked source size and history measured in lines and tokens. Measures tracked repository files/history; repo owns it, not Home (tokens.rs:46). Distinct from provider usage.
- **N106**: After LOO-298 consumption belongs to AgentSession history; Exec history also includes mechanical commands without provider usage. Keep a distinct spending report, remove duplicate RunSnapshot export. Prospective attribution remains until LOO-336 decides otherwise.
- **N107**: Concept: Repository maintainer metric evaluation from recorded evidence. Owns Internal: render the repository maintainer scorecard for telemetry-daily; no equivalent operation identified.
- **N108**: Concept: Home-wide history of failed-CI response and settlement. Owns Show how failed CI is detected, repaired, and landed across this Home; no equivalent operation identified.
- **N109**: Jack explicitly retains ps. Snapshot includes owned lf/provider processes and gaps; no completed-history inference from OS presence or absence.
- **N110**: Jack explicitly retains top alongside ps; same observations, continuous terminal presentation. mon is derived by unique-prefix resolution, not an alias.
- **N111**: Concept: Cleanup of proven owned orphan processes and stale process receipts. Exact owned process cleanup; worktree prune deletes eligible checkout directories.
- **N112**: Concept: Diagnosis of ledger, installation and planning consistency. Diagnostics; monitor displays work, doctor investigates consistency.
- **N113**: Concept: Discovery of executable commands and authored definitions. Catalog lists executable names; object list commands enumerate domain records.
- **N114**: Concept: Read-only explanation and validation of commands and authored definitions. Explains invocations without execution; separate from object state.
- **N115**: Concept: Repository portfolio of Tasks and unlinked checkout/PR work. Existing portfolio becomes task list; add unlinked checkout/PR facts without Task creation.
- **N116**: A Task can have comments, PR changes and planning edits without an Exec. Retain chronological Work evidence; obtain Exec/Session events from LOO-298 and do not duplicate their history stores.
- **N117**: Jack selects the Monitor owner but leaves view/flag keep-or-cut choices open at demo. Compare A470–A481 against ps/top and Exec/AgentSession/FlowSession readers in the measurement table. Do not relabel Run records.
- **N118**: Replay spends again with recorded inputs, unlike Session connect which continues identity. LOO-298 capture references select the historical input; an Exec alone is not necessarily an agent request. Retain restrictions and current readiness.
- **N119**: Concept: Retired rejection-only namespace with no remaining operation. Retired namespace rejection shim; no operation remains.
- **N120**: Concept: Execution on a selected Home through an explicit SSH transport. Owns Run lf on a Home or SSH host carrying your local credentials; no equivalent operation identified.
- **N121**: Concept: Explicit flow-first execution of a named definition. Named execution prefers a Flow; skill/flow explicitly select kind. Not the retired Run object.
- **N122**: Concept: Workflow execution and captured navigation namespace. Captured workflow execution and cursor controls differ from provider conversation.
- **N123**: LOO-298 adds saved FlowSessions here. Retain execution inventory; move authored-template listing to the shared definition catalog so one flag does not select different product objects.
- **N124**: Saved graph, cursor and node/iteration history are not authored definition help. Keep saved inspection; merge template expansion/validation into typed help. Loops are lenses, not child Sessions.
- **N125**: Validation and definition inspection compile the same template. Typed help must report malformed expansion/review IDs nonzero before removing validate. Saved FlowSession inspection remains independent.
- **N126**: LOO-298 consumes the selected successful structured provider result. Remove in-turn cursor command; do not delete retained decision evidence.
- **N127**: LOO-298 selects branch through successful structured result, not an agent-issued navigation command.
- **N128**: LOO-298 records structured blocked result and keyed Ask returns to that conversation. Remove redundant in-turn decision mutation.
- **N129**: Concept: Continuation of a captured ordinary workflow invocation. Captured invocation continuation; task run also owns Task worker admission.
- **N130**: Concept: Explicit execution of a named skill despite kind or command collisions. Explicit kind avoids command and Flow collisions; keep reserved-name escape.
- **N131**: The mixed catalog already filters kinds. Keep one definition enumeration; explicit skill invocation still disambiguates execution.
- **N132**: Already implemented; typed help preserves reserved-name escape without launching.
- **N133**: Controls additional source material selected for the prompt. Input to this operation; same spelling elsewhere selects that other object.
- **N134**: Controls clipboard material selected for the prompt. Input to this operation; same spelling elsewhere selects that other object.
- **N135**: Controls provider/model selection for execution. Input to this operation; same spelling elsewhere selects that other object.
- **N136**: Controls ordered account preference with permitted fallback. Preference permits fallback; restriction forbids spending outside its set. These are different user decisions, not synonyms. LOO-340 provider-keyed Flow account bundles must preserve both semantics at every child.
- **N137**: Controls hard descendant account-spending boundary without fallback. Preference permits fallback; restriction forbids spending outside its set. These are different user decisions, not synonyms. LOO-340 provider-keyed Flow account bundles must preserve both semantics at every child.
- **N138**: Controls redundant pre-launch broker compatibility probe. Keep only until LOO-340 destination readiness checks the same inherited lease before effects; deleting the compatibility probe alone would lose the remote check. This is a readiness boundary, not account state.
- **N139**: Controls provider permission-prompt bypass, not Git force or mutation consent. Different authority/override scopes; do not merge confirmation, force and provider permission (R07).
- **N140**: One launch-mode selector replaces contradictory booleans; retain configured interactive surface.
- **N141**: Same launch choice, noninteractive even with a terminal.
- **N142**: Same launch choice, explicit native terminal venue.
- **N143**: Same launch choice, explicit provider app venue.
- **N144**: Controls explicit browser capability override. One optional browser capability override; omission inherits configuration.
- **N145**: Explicit false belongs to the same value-taking capability option.
- **N146**: One context-content choice: files, patch, both or none; omission inherits configuration.
- **N147**: Choose the complete desired content instead of a negative full-file toggle.
- **N148**: Controls selected changed-code evidence in the prompt. Same content choice preserves the four effective combinations.
- **N149**: Choose the complete desired content instead of a negative patch toggle.
- **N150**: Controls maximum provider turns for this execution. Input to this operation; same spelling elsewhere selects that other object.
- **N151**: One explicit Work declaration; LOO-298 carries LF_AS through descendants. Domain-local task/wave filters remain scoped query inputs.
- **N152**: Controls explicit Task or Wave declaration, inherited unchanged by descendants. Input to this operation; same spelling elsewhere selects that other object.
- **N153**: Controls exact checkout pin for an already-bound internal launch. Input to this operation; same spelling elsewhere selects that other object.
- **N154**: Controls omission of operating guidance for deliberate prompt isolation. Input to this operation; same spelling elsewhere selects that other object.
- **N155**: Controls read-only explanation of this exact invocation. Shared Clap help affordance, not another domain operation.
- **N156**: Controls release version selection or executable build identity. Input to this operation; same spelling elsewhere selects that other object.
- **N157**: Preserve this distinct input only on the surviving owner: The user namespace has one operation, reading the participant name. Put its JSON/text reader on the object; delete the name leaf.
- **N158**: Controls caller-authored execution instructions. Input to this operation; same spelling elsewhere selects that other object.
- **N159**: Controls page or local HTML selected for capture. Input to this operation; same spelling elsewhere selects that other object.
- **N160**: Controls capture artifact destination. Input to this operation; same spelling elsewhere selects that other object.
- **N161**: Controls capture viewport width. Input to this operation; same spelling elsewhere selects that other object.
- **N162**: Controls capture viewport height. Input to this operation; same spelling elsewhere selects that other object.
- **N163**: Controls named instructions for the review conversation. Input to this operation; same spelling elsewhere selects that other object.
- **N164**: Controls new review request content. Input to this operation; same spelling elsewhere selects that other object.
- **N165**: Controls machine serialization of this operation’s documented result (never progress). Keep DTO contract; errors/progress on stderr, streams explicit (R03).
- **N166**: Controls scope across all repositories or all locally authored Waves. Input to this operation; same spelling elsewhere selects that other object.
- **N167**: Controls stable conversation identity. Input to this operation; same spelling elsewhere selects that other object.
- **N168**: Controls owned-client replacement before conversation continuation. Input to this operation; same spelling elsewhere selects that other object.
- **N169**: Controls explicit concurrent native continuation attempt. Input to this operation; same spelling elsewhere selects that other object.
- **N170**: Controls owner name or local checkout/slug identity. Input to this operation; same spelling elsewhere selects that other object.
- **N171**: Controls generated title provenance that yields to an authored name. Input to this operation; same spelling elsewhere selects that other object.
- **N172**: Controls durable completion, readiness or verdict explanation. Input to this operation; same spelling elsewhere selects that other object.
- **N173**: Controls stable Task ownership of the captured child. Input to this operation; same spelling elsewhere selects that other object.
- **N174**: Controls captured invocation identity fencing the review child. Input to this operation; same spelling elsewhere selects that other object.
- **N175**: Controls selected authored workflow. Input to this operation; same spelling elsewhere selects that other object.
- **N176**: Controls captured review node identity. Input to this operation; same spelling elsewhere selects that other object.
- **N177**: Controls captured review occurrence iteration. Input to this operation; same spelling elsewhere selects that other object.
- **N178**: Controls exact execution to terminate after its owning review. Input to this operation; same spelling elsewhere selects that other object.
- **N179**: Controls machine software-update calendar cadence. Input to this operation; same spelling elsewhere selects that other object.
- **N180**: Controls installation transaction receipt identity. Input to this operation; same spelling elsewhere selects that other object.
- **N181**: Controls exact private store selected for candidate validation. Input to this operation; same spelling elsewhere selects that other object.
- **N182**: Controls explicit development-candidate activation mode. Input to this operation; same spelling elsewhere selects that other object.
- **N183**: Controls candidate handoff to the receipt-pinned coordinator. Installation transaction artifact/target identity; required by promotion/recovery, not a public tuning knob.
- **N184**: Controls explicit replacement of an incompatible disposable development store. Input to this operation; same spelling elsewhere selects that other object.
- **N185**: Controls selection of retained development data. Input to this operation; same spelling elsewhere selects that other object.
- **N186**: Controls CLI activation symlink destination. Installation transaction artifact/target identity; required by promotion/recovery, not a public tuning knob.
- **N187**: Controls matching app candidate bundle. Installation transaction artifact/target identity; required by promotion/recovery, not a public tuning knob.
- **N188**: Controls app activation destination. Installation transaction artifact/target identity; required by promotion/recovery, not a public tuning knob.
- **N189**: Controls retired app destination included in atomic replacement. Installation transaction artifact/target identity; required by promotion/recovery, not a public tuning knob.
- **N190**: Controls provider-native skill export after successful activation. Input to this operation; same spelling elsewhere selects that other object.
- **N191**: Controls read-only calculation of this mutation’s effects. Preview existing operation, no second writer. Plan/preview vocabulary requires demo decision R07.
- **N192**: Controls retained immutable candidate executable. Installation transaction artifact/target identity; required by promotion/recovery, not a public tuning knob.
- **N193**: Controls continuous observation of this result. Input to this operation; same spelling elsewhere selects that other object.
- **N194**: Controls failure-log detail for observed CI checks. Input to this operation; same spelling elsewhere selects that other object.
- **N195**: Preserve this distinct input only on the surviving owner: Bare pr already invokes the same status reader. One summary endpoint retains missing-PR and provider-error distinctions.
- **N196**: Controls next serial PR branch identity. Input to this operation; same spelling elsewhere selects that other object.
- **N197**: Controls authored title for the selected planning or review object. Input to this operation; same spelling elsewhere selects that other object.
- **N198**: Controls authored PR review narrative. Input to this operation; same spelling elsewhere selects that other object.
- **N199**: Controls strict delivery verification policy. Input to this operation; same spelling elsewhere selects that other object.
- **N200**: Controls permission to create a missing review artifact during submit. Input to this operation; same spelling elsewhere selects that other object.
- **N201**: Controls Task disposition after successful delivery. Input to this operation; same spelling elsewhere selects that other object.
- **N202**: Controls future serial PR continuation intent. Input to this operation; same spelling elsewhere selects that other object.
- **N203**: Controls checkout selected for delivery. Input to this operation; same spelling elsewhere selects that other object.
- **N204**: Controls commit content or appended discussion text. Input to this operation; same spelling elsewhere selects that other object.
- **N205**: Preserve this distinct input only on the surviving owner: LOO-332 makes land return after recording exact-head merge/settlement intent. Arm becomes the same operation; do not add a watcher flag to preserve the retired supervisor.
- **N206**: Controls settlement through the invoking process rather than the Home supervisor. Input to this operation; same spelling elsewhere selects that other object.
- **N207**: Controls Git branch selected for discard. Input to this operation; same spelling elsewhere selects that other object.
- **N208**: Controls explicit override of dirty-state protection. Different authority/override scopes; do not merge confirmation, force and provider permission (R07).
- **N209**: Controls read-only operation preview or authored chapter/account plan input. Preview existing operation, no second writer. Plan/preview vocabulary requires demo decision R07.
- **N210**: Only json is recognized; other values silently select text (ops/mod.rs:1655).
- **N211**: Ignored: run_wt destructures List with ..; no effect (ops/mod.rs:1516).
- **N212**: Controls explicit refresh of remote facts before this read. Input to this operation; same spelling elsewhere selects that other object.
- **N213**: Controls read-only preview of the selected mutation. Preview existing operation, no second writer. Plan/preview vocabulary requires demo decision R07.
- **N214**: Controls local ownership of rebase conflict handling. Input to this operation; same spelling elsewhere selects that other object.
- **N215**: Controls continuation of the existing local rebase. Input to this operation; same spelling elsewhere selects that other object.
- **N216**: Controls cancellation of the existing local rebase. Input to this operation; same spelling elsewhere selects that other object.
- **N217**: Controls explicit adoption of an already-running raw rebase. Input to this operation; same spelling elsewhere selects that other object.
- **N218**: Controls Git integration base. Input to this operation; same spelling elsewhere selects that other object.
- **N219**: Commit plus push/draft publication duplicates the explicit PR operation; migrate composition.
- **N220**: Controls restriction of checkpoint content to the existing index. Input to this operation; same spelling elsewhere selects that other object.
- **N221**: Preserve this distinct input only on the surviving owner: Bare Account already delegates to Status. One reader gains live default and explicit --cached from LOO-340, retains identity mismatch/duplicate evidence from LOO-339. Migrate status callers with it.
- **N222**: LOO-340 makes observation live by default. Add --cached as the explicit no-contact mode; do not retain a redundant live toggle.
- **N223**: Controls provider whose connection or routing is selected. Input to this operation; same spelling elsewhere selects that other object.
- **N224**: Controls managed login identity. Input to this operation; same spelling elsewhere selects that other object.
- **N225**: Controls ordered browser venues for provider login. Input to this operation; same spelling elsewhere selects that other object.
- **N226**: Controls adoption of an existing native login. Input to this operation; same spelling elsewhere selects that other object.
- **N227**: Controls adoption of an environment-provided API credential. Input to this operation; same spelling elsewhere selects that other object.
- **N228**: Controls corrected login identity. Input to this operation; same spelling elsewhere selects that other object.
- **N229**: Controls whether this account participates in selection. Input to this operation; same spelling elsewhere selects that other object.
- **N230**: Controls removal of configured subscription plan metadata. Input to this operation; same spelling elsewhere selects that other object.
- **N231**: Controls configured paid subscription expiration. Input to this operation; same spelling elsewhere selects that other object.
- **N232**: Controls removal of configured subscription expiration. Input to this operation; same spelling elsewhere selects that other object.
- **N233**: Controls removal of local failure cooldown without rewriting usage windows. Input to this operation; same spelling elsewhere selects that other object.
- **N234**: Controls removal of remembered browser venues. Input to this operation; same spelling elsewhere selects that other object.
- **N235**: Controls ordered account route members. Input to this operation; same spelling elsewhere selects that other object.
- **N236**: Controls repository routing or observation scope. Input to this operation; same spelling elsewhere selects that other object.
- **N237**: Controls machine-default account route scope. Input to this operation; same spelling elsewhere selects that other object.
- **N238**: Preserve this distinct input only on the surviving owner: Default route inspection and its only read leaf are one concept. Keep set as the distinct mutation.
- **N239**: Controls release product or SSH destination selected by this operation. Input to this operation; same spelling elsewhere selects that other object.
- **N240**: Controls release narrative comparison base. Input to this operation; same spelling elsewhere selects that other object.
- **N241**: Controls immutable version label selected for hosted publication. Input to this operation; same spelling elsewhere selects that other object.
- **N242**: Controls authored release or Task narrative. Input to this operation; same spelling elsewhere selects that other object.
- **N243**: Controls artifact files attached to the hosted release. Input to this operation; same spelling elsewhere selects that other object.
- **N244**: Controls transition of the existing release draft to published. Input to this operation; same spelling elsewhere selects that other object.
- **N245**: Controls application of a planning-team reconciliation preview. Input to this operation; same spelling elsewhere selects that other object.
- **N246**: Controls stable execution destination identity. Input to this operation; same spelling elsewhere selects that other object.
- **N247**: Controls observed network address for a known Home. Input to this operation; same spelling elsewhere selects that other object.
- **N248**: Controls explicit consent for provider skill-directory writes. Different authority/override scopes; do not merge confirmation, force and provider permission (R07).
- **N249**: Controls preservation of prior generated skills during export. Input to this operation; same spelling elsewhere selects that other object.
- **N250**: Controls Wave target or scope. Scope of this read/write, distinct from root launch attribution; resolve explicit target before effects (R04).
- **N251**: Controls scheduled invocation calendar expression. Input to this operation; same spelling elsewhere selects that other object.
- **N252**: Controls scheduler-origin attribution for receipt obligations. Input to this operation; same spelling elsewhere selects that other object.
- **N253**: Controls observation window, retaining all-time semantics where supported. Input to this operation; same spelling elsewhere selects that other object.
- **N254**: Controls waiting for the fired schedule’s receipt. Input to this operation; same spelling elsewhere selects that other object.
- **N255**: Controls maximum wait for an observable condition. Input to this operation; same spelling elsewhere selects that other object.
- **N256**: Controls current-registration filter without deleting history. Input to this operation; same spelling elsewhere selects that other object.
- **N257**: Preserve this distinct input only on the surviving owner: Fold goal, chapter, KRs and Task facts into the scoped portfolio reader. Remove resident status; retain unavailable planning and exact Session/Exec references, not synthetic Wave liveness.
- **N258**: Preserve this distinct input only on the surviving owner: LOO-334 places provider/Team connection at repository scope. Wave-to-Initiative mapping is an input to that operation, not independent account/setup authority per Wave. Exact connection controls need integration with that work.
- **N259**: Controls duplicate spelling of the positional Wave target. Input to this operation; same spelling elsewhere selects that other object.
- **N260**: Owning obsolete operation is removed: Wave existence comes from selected repository definitions. Deleting an empty registry row cannot delete a Wave and reads reimport it; remove this store-management operation. Preserve history.
- **N261**: Preserve this distinct input only on the surviving owner: Stopped-resident relocation disappears. Rename owns directory/definition identity; execution placement remains separate under place. No duplicate slug/worktree relocation operation.
- **N262**: Owning obsolete operation is removed: Remove registry retirement as an independent Wave-existence writer. Explicit authored definition removal and provider planning disposition retain their owners; this is not authorization to erase history or provider plans.
- **N263**: Controls dated chapter identity. Input to this operation; same spelling elsewhere selects that other object.
- **N264**: Preserve this distinct input only on the surviving owner: Task is an explicit Work declaration on the ordinary Flow driver. Migrate exact claim/placement and saved boundary consumption; delete a separate Task worker entry only after equivalent admission/recovery proof.
- **N265**: Controls Task issue identity. Input to this operation; same spelling elsewhere selects that other object.
- **N266**: Controls Task whose active PR supplies the checkout base. Input to this operation; same spelling elsewhere selects that other object.
- **N267**: Controls additional execution direction. Input to this operation; same spelling elsewhere selects that other object.
- **N268**: Preserve this distinct input only on the surviving owner: Jack identifies task run as shorthand for explicit Task declaration and ordinary Flow execution. No second progression owner; preserve saved Flow selection and launch placement. Whether to retain that extra spelling is a demo item under the no-alias rule.
- **N269**: Controls selected execution record or requested immediate Task execution. Input to this operation; same spelling elsewhere selects that other object.
- **N270**: Preserve this distinct input only on the surviving owner: Changes and diff read the same checkout/base. A files projection retains complete changed-path/rename/base DTOs; patch retains draft/binary evidence. Migrate the native file browser without extra reads or lost revision safety.
- **N271**: Controls command/definition path or checkout file path. Input to this operation; same spelling elsewhere selects that other object.
- **N272**: Controls commit baseline for checkout inspection. Input to this operation; same spelling elsewhere selects that other object.
- **N273**: Controls in-memory file content compared without writing the checkout. Input to this operation; same spelling elsewhere selects that other object.
- **N274**: Controls retained file versions for recovery inspection. Input to this operation; same spelling elsewhere selects that other object.
- **N275**: Controls expected file revision for conditional replacement. Input to this operation; same spelling elsewhere selects that other object.
- **N276**: Controls Task condition whose observation ends the wait. Input to this operation; same spelling elsewhere selects that other object.
- **N277**: Preserve this distinct input only on the surviving owner: FlowSession owns the saved graph and continuation. Restart is an explicit replacement of that selection, independent of Task status, and must preserve prior history.
- **N278**: Controls historical Project attribution filter inside a Wave. Scope of this read/write, distinct from root launch attribution; resolve explicit target before effects (R04).
- **N279**: Controls Task target or scope. Scope of this read/write, distinct from root launch attribution; resolve explicit target before effects (R04).
- **N280**: Controls start of the observation window. Input to this operation; same spelling elsewhere selects that other object.
- **N281**: Controls diagnosis of repository planning consistency without writes. Input to this operation; same spelling elsewhere selects that other object.
- **N282**: Controls bounded row count with explicit truncation evidence. Input to this operation; same spelling elsewhere selects that other object.
- **N283**: Monitor absorbs this function only where it answers a question absent from ps/top. See the per-view comparison; no final keep/cut assumed.
- **N284**: Owning obsolete operation is removed: Concept: Retired rejection-only namespace with no remaining operation. Retired namespace rejection shim; no operation remains.
- **N285**: Controls ordered account preference carried to the destination. Preference permits fallback; restriction forbids spending outside its set. These are different user decisions, not synonyms. LOO-340 provider-keyed Flow account bundles must preserve both semantics at every child.
- **N286**: Controls hard account-spending boundary carried to the destination. Preference permits fallback; restriction forbids spending outside its set. These are different user decisions, not synonyms. LOO-340 provider-keyed Flow account bundles must preserve both semantics at every child.
- **N287**: Controls named Doppler-backed capability forwarded to a destination. Input to this operation; same spelling elsewhere selects that other object.
- **N288**: Controls explicit forwarding of SSH signing authority. Input to this operation; same spelling elsewhere selects that other object.
- **N289**: Controls destination invocation argv after the transport boundary. Input to this operation; same spelling elsewhere selects that other object.
- **N290**: Controls arguments passed to an authored definition. Input to this operation; same spelling elsewhere selects that other object.
- **N291**: Preserve this distinct input only on the surviving owner: Validation and definition inspection compile the same template. Typed help must report malformed expansion/review IDs nonzero before removing validate. Saved FlowSession inspection remains independent.
- **N292**: Owning obsolete operation is removed: LOO-298 consumes the selected successful structured provider result. Remove in-turn cursor command; do not delete retained decision evidence.
- **N293**: Owning obsolete operation is removed: LOO-298 selects branch through successful structured result, not an agent-issued navigation command.
- **N294**: Owning obsolete operation is removed: LOO-298 records structured blocked result and keyed Ask returns to that conversation. Remove redundant in-turn decision mutation.
- **N295**: Controls captured workflow identity. Input to this operation; same spelling elsewhere selects that other object.
- **N296**: Controls explicit retry of a failed captured boundary. Input to this operation; same spelling elsewhere selects that other object.
- **N297**: Preserve this distinct input only on the surviving owner: The mixed catalog already filters kinds. Keep one definition enumeration; explicit skill invocation still disambiguates execution.
- **N298**: Preserve this distinct input only on the surviving owner: Already implemented; typed help preserves reserved-name escape without launching.

## Caller evidence

[E000]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/bin/lf.rs#L452
[E001]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/init.md#L72
[E002]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L185
[E003]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/config.md#L4
[E004]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L128
[E005]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/webhook.rs#L133
[E006]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/getting-started.md#L22
[E007]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/index.md#L73
[E008]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/init.md#L294
[E009]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/LOOPFLOW.md#L85
[E010]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/pr-review.md#L67
[E011]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L199
[E012]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/screenshot.rs#L29
[E013]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture/codebase.md#L67
[E014]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/util.rs#L787
[E015]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/LOOPFLOW.md#L74
[E016]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/loopflow.md#L40
[E017]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/rebase-conflicts.md#L55
[E018]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/README.md#L115
[E019]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/controller/task/mod.rs#L796
[E020]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/util.rs#L594
[E021]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L259
[E022]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/SessionFixture.swift#L54
[E023]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/loopflow.md#L16
[E024]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L269
[E025]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/SessionFixture.swift#L57
[E026]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L291
[E027]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/SessionFixture.swift#L61
[E028]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/flow_session.rs#L270
[E029]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L282
[E030]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/surfaces/human-present.md#L9
[E031]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/flow_session.rs#L234
[E032]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/human_session.rs#L950
[E033]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/human_session.rs#L2077
[E034]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/human_session.rs#L2093
[E035]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/human_session.rs#L913
[E036]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/install.py#L416
[E037]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/bin/lf.rs#L1444
[E038]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/install.rs#L1
[E039]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/install/published.rs#L221
[E040]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L197
[E041]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/global_commands.rs#L266
[E042]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/install.rs#L2957
[E043]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/publish_release.py#L164
[E044]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/install.rs#L718
[E045]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/install/published.rs#L99
[E046]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/install/recovery.rs#L126
[E047]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/install.rs#L1453
[E048]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/global_commands.rs#L108
[E049]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/install.rs#L3058
[E050]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/install.py#L508
[E051]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/store/mod.rs#L337
[E052]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/store/sqlite.rs#L352
[E053]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/MacLocalWaveAgentLauncher.swift#L70
[E054]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/controller/task/mod.rs#L1120
[E055]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/ops/mod.rs#L143
[E056]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/ops/mod.rs#L1986
[E057]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/wt_ci_logs_tests.rs#L144
[E058]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowTests/WaveChatConnectionTests.swift#L461
[E059]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/task.rs#L3276
[E060]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/MacLocalWaveAgentLauncher.swift#L73
[E061]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/LOOPFLOW.md#L24
[E062]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/pr-publish.md#L12
[E063]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/land.rs#L194
[E064]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/LOOPFLOW.md#L25
[E065]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/pr-land.md#L31
[E066]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/pr-submit.md#L11
[E067]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/pr_landing.rs#L327
[E068]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/LOOPFLOW.md#L26
[E069]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/pr-land.md#L30
[E070]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/run.rs#L719
[E071]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/waves.rs#L1550
[E072]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/pr_landing.rs#L237
[E073]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/task/skill/ship-decomposed.md#L76
[E074]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/task.rs#L2912
[E075]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/agent.rs#L1742
[E076]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/output.rs#L108
[E077]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/telemetry.rs#L2
[E078]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/python/tests/test_checkout_refresh.py#L134
[E079]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/worktree_tests.rs#L392
[E080]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/wave/skill/review-open-work.md#L48
[E081]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/troubleshooting.md#L107
[E082]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/troubleshooting.md#L108
[E083]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/ops/mod.rs#L1967
[E084]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/troubleshooting.md#L114
[E085]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/release_tests.rs#L1468
[E086]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/bin/lf.rs#L1627
[E087]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/ops/mod.rs#L228
[E088]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/checkout.rs#L182
[E089]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/LOOPFLOW.md#L21
[E090]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/start-chapter.md#L77
[E091]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/task/skill/ci-fix.md#L18
[E092]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/bootstrap-cron-host.sh#L66
[E093]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/agent.rs#L2156
[E094]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/auth_status.rs#L350
[E095]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/demo_profile_routing.py#L151
[E096]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/publish_release.py#L124
[E097]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/subscriptions.md#L34
[E098]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/pm/oauth_tests.rs#L236
[E099]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/demo_profile_routing.py#L107
[E100]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/subscriptions.md#L89
[E101]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/auth.rs#L972
[E102]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/demo_profile_routing.py#L132
[E103]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/profile.rs#L24
[E104]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/init.md#L38
[E105]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/config.md#L439
[E106]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/demo_profile_routing.py#L154
[E107]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/subscriptions.md#L17
[E108]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/canonicalize_migrations.py#L8
[E109]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/check_migrations.py#L16
[E110]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/install.py#L9
[E111]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/new_migration.py#L14
[E112]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/release.rs#L733
[E113]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/store/MIGRATIONS.md#L98
[E114]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/config.md#L187
[E115]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/publish_release.py#L193
[E116]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/python/tests/test_release_publisher.py#L178
[E117]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/controller/wave/relocate.rs#L265
[E118]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/pm.rs#L328
[E119]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L193
[E120]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L693
[E121]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/bootstrap-cron-host.sh#L25
[E122]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/home.rs#L11
[E123]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/init.md#L39
[E124]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/verify_branch_data.py#L69
[E125]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/home.rs#L28
[E126]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/verify_branch_data.py#L90
[E127]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/home.rs#L54
[E128]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/init.md#L235
[E129]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L186
[E130]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/bootstrap-cron-host.sh#L47
[E131]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/doctor.rs#L442
[E132]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/cron.rs#L433
[E133]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/wave_resolution_matrix.rs#L479
[E134]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/bootstrap-cron-host.sh#L81
[E135]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/python/tests/test_release_automation.py#L220
[E136]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/wave_resolution_matrix.rs#L90
[E137]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/python/tests/test_release_automation.py#L218
[E138]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/wave_resolution_matrix.rs#L101
[E139]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/bootstrap-cron-host.sh#L78
[E140]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/python/tests/test_release_automation.py#L219
[E141]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/wave_resolution_matrix.rs#L103
[E142]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/bootstrap-cron-host.sh#L128
[E143]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/cron.rs#L593
[E144]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/bootstrap-cron-host.sh#L116
[E145]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/python/tests/test_release_automation.py#L221
[E146]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/wave_resolution_matrix.rs#L105
[E147]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/wave_resolution_matrix.rs#L106
[E148]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Models/Wave.swift#L7
[E149]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Models/WaveViewModel.swift#L30
[E150]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Models/WaveWorkMap.swift#L92
[E151]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L7
[E152]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/AppTestMode.swift#L109
[E153]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/pm.rs#L286
[E154]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/init.md#L171
[E155]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/wave/skill/split-wave.md#L53
[E156]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L302
[E157]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/ssh.rs#L15
[E158]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/waves.rs#L728
[E159]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/wave_resolution_matrix.rs#L99
[E160]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L589
[E161]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/status_tests.rs#L538
[E162]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/init.md#L239
[E163]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/loopflow.md#L150
[E164]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L651
[E165]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/controller/wave/README.md#L106
[E166]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L188
[E167]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture/homes.md#L133
[E168]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L588
[E169]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/waves.rs#L900
[E170]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/chapter.rs#L289
[E171]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/chapter_tests.rs#L599
[E172]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L74
[E173]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L189
[E174]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L530
[E175]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/loopflow.md#L72
[E176]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/task/skill/prompt.md#L157
[E177]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/wave/skill/split-wave.md#L54
[E178]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Models/TaskComments.swift#L3
[E179]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/MacLocalWaveAgentLauncher.swift#L32
[E180]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/README.md#L61
[E181]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L187
[E182]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture/codebase.md#L65
[E183]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/MacLocalWaveAgentLauncher.swift#L100
[E184]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/loopflow.md#L114
[E185]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/task/skill/advance.md#L42
[E186]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L188
[E187]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/controller/task/mod.rs#L854
[E188]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/MacLocalWaveAgentLauncher.swift#L129
[E189]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/pm/task_planning_tests.rs#L352
[E190]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/task_pm.rs#L154
[E191]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/bin/lf.rs#L756
[E192]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/waves.rs#L1039
[E193]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/work/task/mod.rs#L130
[E194]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L176
[E195]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L374
[E196]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/task.rs#L1976
[E197]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L228
[E198]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L375
[E199]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowTests/RegistryQueryTests.swift#L561
[E200]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L247
[E201]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L376
[E202]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L253
[E203]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L377
[E204]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L361
[E205]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/waves.md#L404
[E206]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/pm.rs#L1391
[E207]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/pm/task_planning_tests.rs#L571
[E208]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/loopflow.md#L95
[E209]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/agent-api.md#L130
[E210]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L216
[E211]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L341
[E212]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/wave_resolution_matrix.rs#L82
[E213]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L202
[E214]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/README.md#L106
[E215]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/MacLocalWaveAgentLauncher.swift#L135
[E216]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/agent-api.md#L103
[E217]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/conducting.md#L125
[E218]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/agent-api.md#L39
[E219]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/getting-started.md#L99
[E220]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L348
[E221]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L196
[E222]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/task_flow.rs#L76
[E223]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/loopflow.md#L172
[E224]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/tokens.rs#L46
[E225]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L325
[E226]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/MacLocalWaveAgentLauncher.swift#L265
[E227]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/Views/TelemetryDashboardView.swift#L422
[E228]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/usage.rs#L1
[E229]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/subscription.rs#L8
[E230]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/agent-api.md#L161
[E231]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/bin/lf.rs#L1783
[E232]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/python/tests/test_architecture.py#L165
[E233]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/flow.rs#L446
[E234]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/ci.rs#L1
[E235]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L192
[E236]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/scripts/test.py#L521
[E237]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/top.rs#L1
[E238]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/loopflow.md#L182
[E239]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/loopflow.md#L181
[E240]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L200
[E241]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/install.rs#L576
[E242]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/builtins/ops/skill/loopflow.md#L184
[E243]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/MacLocalWaveAgentLauncher.swift#L191
[E244]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/Views/TelemetryDashboardView.swift#L566
[E245]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/doctor.rs#L1
[E246]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/.github/workflows/package-build.yml#L67
[E247]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/error.rs#L17
[E248]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/navigation.rs#L268
[E249]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L14
[E250]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Models/NowProjection.swift#L14
[E251]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/Views/RoadmapView.swift#L45
[E252]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Models/WorkActivity.swift#L3
[E253]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/activity.rs#L1
[E254]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/store/sqlite/durable.rs#L501
[E255]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L26
[E256]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/LoopflowMac/Views/TaskRunsView.swift#L6
[E257]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L198
[E258]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture/execution.md#L172
[E259]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/conducting.md#L75
[E260]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L771
[E261]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/bin/lf.rs#L2301
[E262]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/bin/lf.rs#L180
[E263]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/home.rs#L184
[E264]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/ssh.rs#L1
[E265]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/authoring.md#L103
[E266]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/controller/task/mod.rs#L749
[E267]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/flow.rs#L946
[E268]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/flow.rs#L44
[E269]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/swift/Loopflow/Services/RegistryQuery.swift#L182
[E270]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L36
[E271]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L192
[E272]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/global_commands.rs#L130
[E273]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L193
[E274]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/global_commands.rs#L131
[E275]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/flow.rs#L426
[E276]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L475
[E277]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L462
[E278]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture/planning.md#L96
[E279]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/flow.rs#L177
[E280]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L476
[E281]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/flow.rs#L93
[E282]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/ops/flow_session.rs#L255
[E283]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/architecture-reference.md#L489
[E284]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/engine/error.rs#L19
[E285]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/src/lf/commands/list.rs#L79
[E286]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L34
[E287]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/docs/lf-reference.md#L168
[E288]: https://github.com/loopflowstudio/loopflow/blob/a6b1bc3dff4f826291ec094d7720adbf125777a8/rust/loopflow/tests/cli_discovery.rs#L106

## Extra aliases

| Canonical row | Extra spelling | Verdict |
|---|---|---|
| A001 `lf --clipboard` | `-C` | delete; retain primary short/long spellings |
| A002 `lf --model` | `-M` | delete; retain primary short/long spellings |
| A007 `lf --interactive` | `-I` | delete; retain primary short/long spellings |
| A008 `lf --batch` | `-B` | delete; retain primary short/long spellings |
| A018 `lf --wave` | `-W` | delete; retain primary short/long spellings |
| A111 `lf pr publish --model` | `-M` | delete; retain primary short/long spellings |
| A115 `lf pr open --model` | `-M` | delete; retain primary short/long spellings |
| A171 `lf commit --message` | `-M` | delete; retain primary short/long spellings |
| A172 `lf commit --push` | `-P` | delete; retain primary short/long spellings |
| C041 `lf wt remove` | `rm` | delete; canonical operation only |

## Dynamic/default entry points

| Surface | Owner and purpose | Caller/source | Overlap | Verdict |
|---|---|---|---|---|
| Bare `lf` | Existing default agent launch | `bin/lf.rs::run_default_agent` | Root help must explain default; not a new setup operation | keep; demo choice if switching bare default to help |
| `lf NAME` | Command-first, then flow-first definition lookup | `bin/lf.rs`, `lf/navigation.rs` | Derived convenience, not an alias registry | keep |
| `lf skill NAME` | Explicit skill selection | `ops/human_session.rs` and `ops/cron.rs` | Required to avoid same-named Flow and command collisions | keep |
| `lf flow NAME` | Explicit Flow selection | `lf/commands/flow.rs` | Saved invocation and cursor differ from skill launch | keep |

## Required additions and cross-cutting work

- `monitor` overview and `account` overview reuse existing evidence readers; include missingness and next action. Neither creates a second scheduler or credential store. Account now uses its integrated live reader with explicit `--cached`; `--json` emits the existing report schema. Monitor remains required.
- `monitor show ID` and `monitor active` replace the multiplexed `runs` path using LOO-298 Exec/Session owners. History filters, parent identity and DTOs follow that model.
- `task list` adds unlinked checkout/PR evidence; `wave list` includes locally authored goals without requiring planning credentials. These carry earlier omitted recommendations forward.
- First local result uses existing direct execution in a fresh directory before planning connection. Verify a real provider result; fix actual setup obstacles rather than introduce synthetic Tasks.
- Foreground/background/remote launch readiness checks required access at the destination while retaining inherited restrictions.
- Every rename/delete migrates parser, typed Flow dispatch, skill text, scripts, Desktop argv and tests in the same cut. Saved execution identity remains LOO-298's responsibility.
- Add no fixed aliases. Jack selected unique-prefix resolution for monitor/mon; Account has no registered short name.
- Retain draft/open behavior pending Jack's demo choice; do not silently reinterpret mutation as a view.
- Public docs lose draft/deferral labels only after the above paths are implemented and the public-CLI walkthrough passes. No landing before Jack's demo review.

## Review findings

- `wt list --full` is parsed but ignored. `wave status --no-sync` is parsed and discarded. Both delete verdicts are based on handlers, not missing caller strings.
- `wt list --format` accepts arbitrary strings but only recognizes `json`; replace with `--json` and retain the same serialized data.
- `skill show` calls the same definition-help renderer as `help skill NAME`; merge the duplicate operation into typed help, preserving reserved-name inspection.
- `commit --push` duplicates draft publication owned by PR open. Preserve commit-only behavior and compose PR publication explicitly.
- Do not delete required internal installation/process callbacks merely because public help hides them. Exact artifact, worker, Session and process evidence grant different authority.
- Counts include namespace and help entries. Moving nodes does not itself reduce leaf operations; report real deleted arguments/aliases separately at demo.
