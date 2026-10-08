# Run existing skills on either harness

LOO-420. Jack Heart selected native
same-harness invocation, translated cross-harness ports, inlined builtins,
`--agent` / `-a`, terminal/headless execution and same-conversation use.

## Status and decisions

PR #1497 merged as `b6bc42998`; PR 2 preserves follow-up.
Reconciled October 8 through `34e77951d`: native queue proof `02a073349`,
headless/pending-restart proofs `496652f1d` and `ed9ad8a61`, and shared receipt
waits that retain each input's unique identity and requested terminal outcome.
Provider admission, terminal-free consumption and pending-queue restart now have
passing candidates; dependent LF integration remains unfinished.
`c91648d63` retains the counterexample: idle-read plus `turn/start` can join a
competing turn with exact receipts but no expansion. `--queue-race` instead uses
Codex 0.160.1's existing
`thread/queue/add`: an already-attached competing writer finishes before the
queued skill expands on a separate native turn. Lost acknowledgement, caller
cancellation, connection handoff, sibling history and a native terminal draft
are covered together. LF admission/capture ownership is still unimplemented;
provider evidence alone cannot close that boundary. Full scope remains required.
LF's retained dispatch and receipt comparison survive driver handoffs;
`cbb402869` preserves saved-Session placement. Neither implements pending skills.
Catalog consolidation and Claude headless native dispatch through canonical commands
and taskless Flows are implemented locally. Headless continuation atomically admits its driver and next capture and
assembles context in the saved Session workspace. Codex native delivery, terminal transport and
current-owner structured delivery remain unfinished.
Jack Heart's October 7 recovery steer retains full implementation through review, without landing or
Task completion. Missing transport/admission remains implementation work.

Jack removed `--ide` after failed reconnects; historical records remain.
LOO-428 keeps uncontrolled Wave/scratch/repository material outside Claude
system instructions; fixed LF instructions remain separate.

[Probe documentation](../scripts/benchmarks/skill-invocation/README.md) owns
receipts and limits. Earlier plans and assumptions remain in the branch history,
including `423ff2ec2` before the resume-workspace repair and `d6848011f` before
compression.

## Native terminal admission: evidence and remaining choice

Claude 2.1.294 fake-API receipts show startup expansion, PTY submission of an
unfinished draft and unexpanded inbox text in one native Session with clean exit.
They prove client mapping, not LF or live-model behavior. Background resume
rejects slash prompts; Channels fidelity is untested. A private plugin expands
arguments but names its snapshot directory. Local Claude dispatch now materializes
captured SKILL.md bytes with sibling-resource symlinks and retains original source
provenance. Real-client/fake-API LF and Flow receipts prove exact arguments,
user-role context and sibling reads after the selected SKILL.md is removed with
a same-name collision. `materialize` enumerates the original directory at launch
and links its current siblings; resources are not captured with the definition.
Resource edits remain visible and removing the bundle removes their availability.
The snapshot's different base directory also leaves parent-relative paths and
plugin-specific variables unresolved. Native controls in full and live admission
remain unproved; these losses are not accepted scope reductions.

**Unselected alternative:** a terminal over the stream driver replaces native
editing, menus and controls, requiring an interaction decision and parity evidence.
The probes do not establish that replacement is unavoidable. Native transport
must preserve drafts and expand commands including `disable-model-invocation: true`;
model-requested Skill use or inlining does not qualify. Catalog, ports and
structured dispatch can progress independently without satisfying the full Task.

## Implemented catalog boundary

The engine catalog now owns lookup, listing, help, Flow capture and export source
collection. Ordering follows the proposed repository/personal/embedded precedence
and source ordering. Source path, dialect and raw declarations survive Flow
serialization after source removal; Claude subagent names no longer select lf's
harness. Generated exports cannot win; bundled Markdown stays out of discovery.
Export preserves third-party files and references original bundle directories.
Cross-harness exports currently retain declarations and warn, without translation.
Catalog/CLI fixtures prove selection and retention, not execution fidelity.

## Implemented dispatch boundary

`SkillInvocation` carries retained Skill/source/declarations and exact arguments,
separate from gathered Work context. Canonical preparation uses it for Claude
headless launches; LF builtins remain inline. A Flow passes captured input to its
ordinary skill subprocess, deleting the child's late catalog selection. Capture,
replay and retry retain this input. Session follow-up uses existing native history
without rerunning the original command. Ordinary non-Git folders skip automatic
Git checkpointing after a successful skill.

Claude's private plugin consumes the captured definition. File-backed NDJSON and
the stream driver send `shouldQuery: false` context, then the exact command.
The stream reader waits for both the context acknowledgement and skill result.
Codex-to-Claude ports provide tool mappings, original declarations and bundle
location with a one-line loss notice. They do not enforce foreign permissions or
model/subagent/preprocessing declarations. Claude-to-Codex argument translation
has focused coverage but no functioning native Codex destination yet.

## Codex catalog selection (October 8)

Codex 0.160.1 returned exit zero while ignoring an explicit `skill` item whose
path was outside its loaded catalog. Registering via the documented
`skills/list.perCwdExtraUserRoots` did nothing; its versioned schema no longer
contains that field. A real-client/fake-API probe proves `skills/extraRoots/set`
registers the snapshot and makes expansion work, but replaces the whole engine's
roots. Clearing them removes prior registration. A shared engine's sibling
conversations may rely on those roots. Session input authority cannot be turned
into permission to replace their catalog.

The failed Codex production prototype was removed; existing Codex inline behavior
remains. This is a design conflict, not permission to narrow the Task or substitute
inlining for native delivery. Native captured-path selection needs an approach
that preserves sibling roots, name collisions, retained bytes and source/asset
semantics. No engine replacement, second Session, global provider config rewrite
or guard refusing shared engines was adopted. The probe is retained at
`scripts/benchmarks/skill-invocation/codex_request_mapping.py`.

An additive registration probe now establishes a candidate: symlink the snapshot
into a normal repository skill root under a unique native `name` alias, then force
catalog reload and pass its explicit canonical path. On Codex 0.160.1, five fake-API
requests preserve a separately seeded extra root, the original personal skill and
fresh plain audit selection. Keeping the original name instead expands the
explicit snapshot but makes a fresh plain audit invocation expand neither source. Reusing
an already-used thread hid this contradiction behind its earlier history; that
misleading check was removed. `--additive original` retains the failing case;
`--additive alias` passes the candidate.

This is provider-only evidence. Before LF adoption, placement and lifetime must
preserve caller files, Git checkpoints, sibling catalog selection and history
through cancellation/handoff. A generated name must preserve the other native
declarations and source/resource semantics. No temporary repository mount or
native-name rewrite was added to production. The original engine-global prototype
remains deleted; a unique alias is a draft approach, not accepted fidelity loss.

## Codex native turn-boundary proof (October 8)

The credential-free `--boundary` probe now drops the start reply in a socket proxy,
cancels the waiter, disconnects it and recovers from native history on a successor
connection to the same engine. Codex 0.160.1 accepts `clientUserMessageId` and retains
it as a user message's `clientId`, including after engine restart. Exactly one
matching receipt preserves the selected path, arguments and separate context;
prior turns remain unchanged. Completion and explicit interruption remain distinct.
No resubmission occurs, so this proves correlation, not idempotency of repeated ids.
A real attached Codex terminal retains an unfinished draft across the skill turn,
then submits it intact afterward and exits zero. No replacement terminal is used.

The seven observed model requests include two native title requests. These counts
remain visible; no cost or live-model claim follows. Fixtures use an existing
on-disk skill. LF admission, active-capture preservation, driver fences, captured
catalog/source fidelity and Claude parity remain separate implementation work.
The probe and its limitations live in the benchmark README.

## Remaining implementation

1. **Current-owner admission, capture ownership and native consumption together.**
   `02a073349` establishes the provider candidate described above. LF still needs
   to retain each pending `SkillInvocation`, context, capture and dispatch identity in existing
   Session history without replacing the active capture or claiming another
   driver. No migration, new queue owner, extra Session or provider extension is
   selected. Same-Session invocation syntax remains an open implementation choice.

   The source boundaries requiring one coordinated change are:

   - `run::resume` clears `skill_input` and `resolved_invocation`;
     `continue_with_context` atomically replaces input and claims a driver through
     `claim_session_input`. This preserves refused continuations, but cannot admit
     a skill through a held owner.
   - Codex `send_input` accepts text. Its writer consumes `seed_capture.take()`
     for the first captured `turn/start`; later starts remain fenced but lack a
     capture-to-message mapping. Every admitted input needs its retained mapping
     at dispatch, including across handoff. A capture may have distinct retry
     continuations, so native identity belongs to each dispatch, not its capture.
   - `engine::agent::_run_harness_once` records all events against one capture
     and returns on the first `TurnCompleted`, ignoring which turn completed.
     Native queue admission alone would misattribute output or finish on the
     competing turn. Dispatch association, event attribution and completion of
     the matching native turn must change together, preserving ordinary text,
     retry identity, active capture and stale-driver fencing.
   - `store::sqlite::session_events::record_session_event` assigns every observed
     native start to `current_capture`, before native input correlation. Filtering
     only the runner's event channel would leave SQLite history misattributed.
     `codex_history::History` currently attributes Process origin only through a
     `turn/start` reply; queued starts need their retained dispatch association.
   - `session_record::runtime::finish_session_driver` calls `close_engine` after
     settling its capture. `inspect_engine_threads` protects other threads but
     does not inspect later queued/active work in the same thread. Matching one
     capture's completion must not authorize terminating another admitted input.
   - Claude's canonical headless path uses file-backed stdin through
     `_run_agent_once`; only Codex/OpenCode enter `_run_harness_once`. A Harness
     trait change alone cannot supply Claude parity. Native Claude terminal
     consumption remains unresolved; tested PTY/inbox injection does not qualify.

   Recovery must read retained dispatch, native queue and turn history without
   resubmitting uncertain input. Handoff retains its native message ID; a new
   continuation gets a different one. Existing exact content comparison records
   missing/duplicate receipts as uncertain; matching bytes alone prove neither
   expansion nor completion. Native interruption and external effects remain
   distinct. Existing Task steers retain their separate text behavior;
   `send_current` is ruled out by the steering counterexample.

   Acceptance traverses LF admission through the held owner, queued expansion,
   capture attribution and the matching turn's completion. It combines an enqueue
   reply dropped after provider acceptance, caller cancellation, driver handoff,
   preserved native draft, active capture and sibling work, without resubmission.
   `--queue-headless` now passes without a terminal: four model requests, no titles,
   one native expansion across dropped acknowledgement/cancellation/handoff.
   `--queue-restart` kills the engine with two accepted inputs still pending;
   queue bytes survive and native resume consumes both on separate turns without
   re-enqueueing or explicit queue-start. Three requests, no titles, one expansion;
   exact input bytes/IDs and the interrupted turn's input history survive.
   These are distinct from the earlier completed-history restart check.
   Resume is asynchronous: the queue can empty before its userMessage appears.
   Recovery must wait for retained input identity, never resend on that gap.
   Store fixtures and provider probes do not compose into LF acceptance;
   gate/review retain the real entry-point proof. LF pending-recovery acceptance
   also needs two retained inputs across engine death before consumption: distinct
   capture attribution and matching-turn settlement, with the second input
   surviving completion of the first. No LF production change was made in this
   recovery-probe cut.

2. Native Codex selection and terminal transport. The additive alias remains a
   provider-only candidate with unresolved placement, lifetime and name semantics.
   Terminal delivery must preserve native controls and unfinished drafts. Neither
   engine-global catalog replacement nor a replacement terminal is approved.
3. Resource fidelity and ports. Changed or removed definitions and bundles,
   parent-relative resources and plugin variables remain uncovered. Available
   declaration equivalents need translation; only declarations with no equivalent
   qualify for a loss notice. Unfamiliar shapes must still run as instructions.

Catalog placement, resource fidelity and declaration translation remain independent
work. The Codex native queue resolves the demonstrated provider race; LF admission,
per-input capture ownership and Claude consumption still need implementation.
Same-Session invocation syntax and the alias's native-name semantics remain open
implementation choices. No narrower acceptance has been selected.

These are substantial implementation work. The admission repair below establishes
preservation on refusal, not successful delivery through a held owner. Full
third-party fidelity and cost comparisons follow implementation; partial headless
dispatch is not Task acceptance.

## LF receipt recovery (October 8)

The headless Codex writer commits `codex_input_dispatch` for its first captured
`turn/start` in Session history before the socket write, under the existing driver
fence. It takes the launch's seed capture once; subsequent starts on the same
writer have no retained input mapping. A failed write leaves that
intent; reusing its native message ID is rejected without another send. No queue,
input replacement, migration, automatic retry or new driver is introduced.
A fresh continuation can share a capture while retaining a different native ID.

Public `session connect` reads full native turn items when the Session has retained
dispatches, then records complete receipt sets across all pages. Older Sessions
keep the smaller history read. `content_matches` compares the ordered input
blocks, normalizing only empty native text annotations: true/false for one match,
null for missing or duplicate receipts. This comparison is implemented;
turn-boundary admission, native expansion and pending consumption remain above.
The competing-start proof demonstrates that matching bytes cannot settle delivery.
The controlled-client real Codex/fake-API LF proof preserves exact text, the active
capture, sibling work and native generation across handoffs. It retains stale-client
rejection and nested Process ancestry. Its fixture now uses `exec_command`; external
egress stays denied by the outer wrapper rather than an invalid nested macOS sandbox.
The store proof retains structured input across a failed transport write and handoff,
with missing/duplicate matches and separate continuation identity. Neither proof
performs new skill admission through a held owner, drops an LF RPC acknowledgement,
or demonstrates LF terminal draft preservation; the earlier provider-only draft and
lost-reply evidence remains separate.

Review found that a capture-scoped duplicate check allowed another capture to reuse
the native ID and match the first input's receipt. The check now spans the Session;
separate continuations still receive distinct IDs. Tests retain changed content and
missing/duplicate sets without Started/Completed authority.

Review found that using the capture itself as the native ID would reject legitimate
retry continuations or conflate their different input bytes. Keeping the mapping in
existing Session history fixes that boundary without a new execution owner.

## Retained implementation boundaries

`7cccd33fe` preserves the complete pre-compression notes; `423ff2ec2` and
`b2228bce8` preserve the earlier catalog/parser reductions and their checks.
`resolved_invocation` keeps source and exact arguments together after its transport
file disappears. One parser retains raw native declarations while validating
Loopflow declarations. Prepared and continued captures share attribution.

Headless resume reserves input and claims its driver in one transaction. Losing
claims leave the old capture untouched; failed publication retains an unpublished
reservation and releases only its driver, without recording a provider turn.
Publication is fenced against handoff and preserves its original failure.
Fixtures establish those boundaries and native identity across handoff, not
successful delivery through a held owner or installed continuity.

`cbb402869` repairs resume placement: saved Session cwd/Task/Wave supply context,
provider cwd and replay; LF Process cwd remains the caller's. CLI fixtures cover
cross-directory context, excluded ambient Wave, preserved historical bytes and
native identity, no original-skill rerun, held owners and taskless Flow correction.
No second workspace owner is introduced.

Probe cleanup `f63d7863b` retains catalog, steering, sequential recovery
and queue-race evidence. Compression separates queue consumption from connection
handoff, shares initialized socket lifetime and RPC reply handling, and removes
duplicate terminal cleanup. Request assessment produces one final report with
all checks, catalog paths and title counts. Missing requests now fail checks
instead of indexing absent evidence; historical expansion still cannot pass as
new expansion. All native assertions remain. Stdio/socket transports and failed
admission counterexamples stay distinct; no new LF authority follows.

Compression after `efd635728` removes the CLI's `discover_skill` and
`resolve_definition` adapters; execution, help and listing call the engine
resolver directly. Two adapter tests duplicated the engine's namespaced/colon
coverage and are deleted. Native receipt recovery groups user messages by client
ID once before acquiring the store transaction, preserving order, missing matches
and duplicates across turns. Review retained observation-only writes and the
existing dispatch fence; no delivery or completion authority changes.

## Delete — do not maintain

- Removed `lf/discovery.rs`: the engine owns resolution and builtin metadata;
  navigation owns the surviving help/list formatting. Resolution tests now live
  beside `engine/target.rs`; callers no longer cross a discovery re-export layer.
- Removed the Codex probe's `--lf` branch, which required the deleted prototype's
  private snapshot layout. `c91648d63` retains that fixture. The surviving provider
  probes select one mode instead of four booleans; native LF entry-point acceptance
  remains required after placement is implemented. No receipt assertion on a
  surviving path was removed.
- Removed repeated socket initialization, nested terminal cleanup and per-mode
  report emission; the probe retains one owner for each and separate queue checks.
- Consolidated native receipt waits into `_wait_for_inputs`: one unique receipt
  and the requested outcome are required for every retained input. Socket and
  stdio reads retain separate transports and caller timeouts; pending-queue
  restart remains distinct from completed-history restart. Probe mode declarations
  now share their argparse setup. Review retained duplicate/missing uncertainty,
  explicit interruption and all native preservation assertions.
- Replace `seed_capture.take()` and `_run_harness_once`'s single-capture/first-
  completion assumptions in the structured admission cut. Preserve original
  capture ownership, plain-input completion, retry identity and stale-driver fencing.
- Replace Codex's implicit `current_capture` start attribution and its
  `turn/start`-reply-only origin correlation in that same cut. Retain uncorrelated
  history as unknown. Settlement must preserve later same-thread queued inputs;
  a capture's completion does not grant authority over them.
- `Harness::send_input(&str)` and its next-turn consumers remain until structured
  admission uses the proved native queue with per-input LF ownership, then are replaced.
  Plain text input, builtins, account routing, native history and custody fences remain.
- `send_current`/`inject_live_steers` retain text steering; the native counterexample
  rules them out as the transport for a general Session skill queue.
- Removed caller-context continuation: `resume -> run -> implicit_binding`.
  `build_prompt_at` now receives saved Session placement directly. Process cwd
  and replay cwd remain distinct facts; no second workspace owner is introduced.

The catalog replaced external/npx/rams discovery, Flow/export resolvers, CLI
discovery adapters and repeated source lookup. Export pruning preserves third-party bundles and symlinks.
Capture admission replaced pre-claim input replacement; native context
acknowledgements cannot complete the command. Earlier deletion details remain at
`d6848011f:scratch/run-any-claude-or-codex.md`; the complete prior account is at
`7cccd33fe:scratch/run-any-claude-or-codex.md`.

## Acceptance still outstanding

Acceptance requires unchanged third-party arguments, controls, assets and source collisions on both
harnesses through `lf audit` and `lf --agent codex audit` in a repository without
`.lf`, matching help and source reporting. Existing fixtures prove catalog/Flow
selection, not these executions. Matched startup and total context costs must
include disk reads and resumed history. Intelligence's October 1 ablation found
that omitted context can return through file reads, and its fresh-turn pilot did
not measure resumed history. Repeated baselines and correctness remain necessary;
smaller launch text alone proves no improvement. Strict improvement awaits gate
and interaction review.

Release's entry-point lesson applies: acceptance traverses `lf audit`, harness
selection, source reporting, retained Flows and same-Session admission, including
a held owner, lost acknowledgement and driver handoff.

Review fixed export overwrites, pruning inside third-party bundles, repeated
catalog scans during listing, and prompt attribution to replacement skill files.
This cut fixed early completion on context acknowledgement, Flow child source
rediscovery and post-success Git errors in ordinary folders. Review rejected
Codex's false-success snapshot route and engine-global replacement.
Release is the only immediate child directory with memory; its GOAL.md and complete
MEMORY.md were read. Its operation-entry and false-success lessons apply to native
dispatch and admission. Related Intelligence context/attribution findings were
read selectively. The local `origin/main` reference remains at the PR base
`812d8cc55`; no upstream fetch or provider refresh was performed.

Check (2026-10-08): reuse recorded probe pytest (25), Ruff and isolated queue-headless/queue-restart (4/3 requests, no titles), queue-race/boundary (7 each, 2 titles); prose reconciliation checked with `git diff --check` and `lf context --skill realign --json`. LF admission, fidelity and gate/review acceptance remain.
