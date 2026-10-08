# Run existing skills on either harness

LOO-420. Jack Heart selected native
same-harness invocation, translated cross-harness ports, inlined builtins,
`--agent` / `-a`, terminal/headless execution and same-conversation use.

## Status and decisions

PR #1497 merged as `b6bc42998`; PR 2 preserves follow-up.
Reconciled October 8 against probe `54b309ab6` and compression `b5f26fee8`.
`cbb402869` retains the resume-workspace repair and steering counterexample.
The probe supersedes the earlier feedback's unproved lost-reply boundary: it proves
provider-level recovery and terminal draft preservation. LF now retains native
message correlation and recovers receipt evidence across driver handoffs; pending
skill admission and native-boundary consumption remain unimplemented.
The locally available main ref remains `812d8cc55`; no newer upstream state was fetched.
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

1. **Current-owner delivery; provider transport proved, LF integration outstanding:**
   admit retained `SkillInvocation` plus context into the Session's own history without replacing its active capture.
   Its existing driver must consume pending input at a native turn boundary.
   The caller must not start another driver or steal an in-progress draft.
   Current `run::resume` explicitly clears `skill_input` and `resolved_invocation`;
   `continue_with_context` replaces input and claims a driver atomically through
   `claim_session_input`. Those paths prove safe continuation/refusal, not pending
   structured delivery through a held owner. Codex `send_input` still emits only
   text. Its writer now adds a durable native message ID, but does not consume
   pending structured input. The next integration proof must traverse LF admission
   and that owner, not just repeat the socket experiment.
   Capture identity must correlate dispatch with native history before retry;
   RPC ids and turn-id acknowledgements alone cannot do that. A lost reply stays
   uncertain until native evidence resolves it. The Codex probe establishes native
   receipt recovery, caller cancellation, connection handoff and draft preservation.
   The capture-key-as-message-ID candidate is superseded: LF's existing retry
   can send a different continuation within the same capture. The writer retains
   a distinct native message ID with its capture, exact outgoing parameters,
   Process and provider generation before dispatch under the Session fence.
   A queued input must retain that same message ID through handoff; a new
   continuation gets its own ID. Receipt observations retain every matching
   user message, including empty/duplicate sets, without granting origin or
   completion authority. Pending-input settlement must still validate exactly
   one receipt against retained skill/path, arguments and context. Native
   completion/interruption and external effects remain separate facts.
   Prove active-capture preservation and LF driver handoff end to end before any
   automatic resubmission. Missing or duplicate native receipts remain uncertain.
   No migration, new queue owner, extra Session or provider protocol
   extension is selected. Same-Session invocation syntax remains unselected.
   Claude's plain headless path uses file-backed stdin through `_run_agent_once`;
   only Codex/OpenCode enter `_run_harness_once`. Changing the Harness trait alone
   would miss Claude's canonical launch too. Claude native terminal consumption remains
   unresolved; a replacement terminal and fidelity losses remain unapproved.
   October 8's Codex steering probe rules out `send_current` as this transport:
   both single and duplicate steers omit native expansion, and identical RPC ids
   repeat input. The new lost-reply proof uses `turn/start` at a boundary, not
   steering. Existing Task steers retain their separate best-effort text behavior.
2. Native Codex selection and terminal transport. The additive alias remains a
   provider-only candidate with unresolved placement, lifetime and name semantics.
   Terminal delivery must preserve native controls and unfinished drafts. Neither
   engine-global catalog replacement nor a replacement terminal is approved.
3. Resource fidelity and ports. Changed or removed definitions and bundles,
   parent-relative resources and plugin variables remain uncovered. Available
   declaration equivalents need translation; only declarations with no equivalent
   qualify for a loss notice. Unfamiliar shapes must still run as instructions.

These are substantial implementation work. The admission repair below establishes
preservation on refusal, not successful delivery through a held owner. Full
third-party fidelity and cost comparisons follow implementation; partial headless
dispatch is not Task acceptance.

## LF receipt recovery (October 8)

The headless Codex writer commits `codex_input_dispatch` in Session history before
its socket write, under the existing driver fence. A failed write leaves that
intent; reusing its native message ID is rejected without another send. No queue,
input replacement, migration, automatic retry or new driver is introduced.
A fresh continuation can share a capture while retaining a different native ID.

Public `session connect` reads full native turn items when the Session has retained
dispatches, then records complete receipt sets across all pages. Older Sessions
keep the smaller history read. These are observations, not settled delivery:
content validation, turn-boundary admission and pending consumption remain above.
The controlled-client real Codex/fake-API LF proof preserves exact text, the active
capture, sibling work and native generation across handoffs. It retains stale-client
rejection and nested Process ancestry. Its fixture now uses `exec_command`; external
egress stays denied by the outer wrapper rather than an invalid nested macOS sandbox.
The store proof retains structured input across a failed transport write and handoff,
with missing/duplicate matches and separate continuation identity. Neither proof
performs new skill admission through a held owner, drops an LF RPC acknowledgement,
or demonstrates LF terminal draft preservation; the earlier provider-only draft and
lost-reply evidence remains separate.

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

`b5f26fee8` separates Codex catalog, steering and recovery experiments.
One app-server context owns spawn, initialization and bounded shutdown; recovery
closes that context before starting the engine that reads persisted receipts.
Socket history reads share one decoder; waiter cancellation has one cleanup path.
Argparse owns mutually exclusive probe selection. All receipt assertions remain,
including ambiguous duplicates, distinct interrupted/completed outcomes and title
request counts. Review retained separate stdio and socket readers because they
exercise different transports; no new provider or LF authority follows.

Compression after `efd635728` removes the CLI's `discover_skill` and
`resolve_definition` adapters; execution, help and listing call the engine
resolver directly. Two adapter tests duplicated the engine's namespaced/colon
coverage and are deleted. Native receipt recovery groups user messages by client
ID once before acquiring the store transaction, preserving order, missing matches
and duplicates across turns. Review retained observation-only writes and the
existing dispatch fence; no delivery or completion authority changes.

## Delete — do not maintain

- `Harness::send_input(&str)` and its next-turn consumers remain until structured
  native-boundary delivery has a proven recovery contract, then are replaced.
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
read selectively; no provider state refresh or upstream fetch was performed.

Check (2026-10-08, compress): `cargo build -p loopflow --bin lf`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`, isolated focused `cargo test` (15 unit + 3 discovery/CLI tests), `git diff --check` and `lf context --skill compress --json` pass; gate/review retain pending-input admission, LF lost-ack/draft proofs, native fidelity, costs and third-party acceptance.
