# Run existing skills on either harness

LOO-420. Jack Heart selected native
same-harness invocation, translated cross-harness ports, inlined builtins,
`--agent` / `-a`, terminal/headless execution and same-conversation use.

## Status and decisions

PR #1497 merged as `b6bc42998`; PR 2 preserves follow-up.
Reconciled October 8 against compression checkpoint `423ff2ec2` and the
resume-workspace repair and structured-steering counterexample. The locally available main ref remains `812d8cc55`; no newer
upstream state was fetched.
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
receipts and limits. The preceding plan and assumptions are preserved at
`/tmp/loo420-before-realign-WUrPka/`; the pre-admission notes remain at
`/tmp/loo420-before-admission/`.

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

## Remaining implementation, in dependency order

1. Structured subsequent-turn input through the current owner. October 8's
   Codex 0.160.1 fake-API counterexample invalidates extending `send_current`
   into the skill-delivery path: single and duplicate structured steers succeed
   without native expansion; identical RPC ids do not deduplicate model input.
   The fresh-turn control expands the same skill. Task steers remain best-effort
   text and cannot own Session skill admission. The probe preserves both replies;
   it demonstrates unsafe redelivery, not actual lost-acknowledgement recovery.
2. **Revised approach, still unproved:** admit retained `SkillInvocation` plus
   context into the Session's own history without replacing its active capture.
   Its existing driver must consume pending input at a native turn boundary.
   The caller must not start another driver or steal an in-progress draft.
   Capture identity must correlate dispatch with native history before retry;
   RPC ids and turn-id acknowledgements alone cannot do that. A lost reply stays
   uncertain until native evidence resolves it. Prove native receipt recovery,
   cancellation, handoff and exactly-once application before implementing automatic
   resubmission. No migration, new queue owner, extra Session or provider protocol
   extension is selected. Same-Session invocation syntax remains unselected.
   Claude's plain headless path uses file-backed stdin through `_run_agent_once`;
   only Codex/OpenCode enter `_run_harness_once`. Changing the Harness trait alone
   would miss Claude's canonical launch too. Native terminal consumption remains
   unresolved; a replacement terminal and fidelity losses remain unapproved.
3. Native Codex selection and terminal transport. The additive alias remains a
   provider-only candidate with unresolved placement, lifetime and name semantics.
   Terminal delivery must preserve native controls and unfinished drafts. Neither
   engine-global catalog replacement nor a replacement terminal is approved.
4. Resource fidelity and ports. Changed or removed definitions and bundles,
   parent-relative resources and plugin variables remain uncovered. Available
   declaration equivalents need translation; only declarations with no equivalent
   qualify for a loss notice. Unfamiliar shapes must still run as instructions.

These are substantial implementation work. The admission repair below establishes
preservation on refusal, not successful delivery through a held owner. Full
third-party fidelity and cost comparisons follow implementation; partial headless
dispatch is not Task acceptance.

## Implemented admission and compression

Compression checkpoint `423ff2ec2` removes prompt assembly’s second read of `--skill-input` and the
skill-only CLI cache. `resolved_invocation` retains the selected definition and
exact arguments together through Work binding; the transport file can disappear
after selection without changing either. Native context rendering uses borrowed
content sections instead of cloning the full context. Prepared and continued
captures share `capture_subjects`; listings and help share `definition_source`,
removing `SkillSource::display_source`. Driver custody and publication fences
remain unchanged. String-only next-turn input remains a deletion target; text steering retains
its separate meaning and cannot acquire skill invocation by changing its type.

Removed `human_session::continue_conversation` and headless resume's prepared-file
round trip. `claim_session_input` reserves the next capture and claims its driver
in one transaction, using the existing shared driver writer and custody checks.
`continue_with_context` publishes context and the process request under that claim.
Rejected admission leaves the previous capture untouched; failed publication retains
an unpublished reservation, releases only its driver and records no provider turn.

The CLI fixture proves held-owner preservation for Claude and Codex. Store/capture
fixtures prove rollback after a competing claim, unchanged native thread/provider
generation through driver handoff, and publication failure without completion.
The taskless Flow correction fixture still succeeds through the real `session resume`
entry point. These are source/fixture results, not installed continuity or native
terminal draft-preservation proofs. Review also fenced publication against driver
handoff, retained its original failure if release fails, and kept manifest cwd
truthful to the actual process rather than substituting the Session's saved cwd.

## Resume workspace repair

Headless `session resume ID MESSAGE` builds directly from the saved Session cwd,
Task and Wave, bypassing caller-checkout binding and ambient Wave fallback.
The provider and replayable capture use that workspace; the LF Process retains
its actual caller cwd. Old captures, Session attribution and the original skill
remain unchanged. The cross-directory CLI fixture proves saved scratch context,
exclusion of caller scratch and a registered ambient Wave, native resume identity,
no original-skill rerun and preserved historical bytes. It uses a stub provider,
not live-model or installed acceptance. Held-owner and taskless correction proofs
still pass. Review retained the provider cwd in the manifest because replay uses
it, and corrected nested Wave selection to use the full slug.

## Delete — do not maintain

- Delete `Harness::send_input(&str)` with its next-turn consumers when structured
  native-boundary delivery has a proven recovery contract. Preserve plain text
  input and builtins, account routing, native history and custody fences.
- Do not convert `send_current`/`inject_live_steers` into skill execution. Their
  text-steering behavior remains required; the native counterexample disproves
  them as the predecessor for a general Session skill queue.
- Removed caller-context continuation: `resume -> run -> implicit_binding`.
  `build_prompt_at` now receives saved Session placement directly. Process cwd
  and replay cwd remain distinct facts; no second workspace owner is introduced.

## Removed mechanisms

Removed: separate external discovery, rams alias, fuzzy npx lookup/fetch,
recursive Markdown-only listing, Flow's separate resolver and late `.agents`
fallback, and exclusive fetch fixtures. Installed npx folders use ordinary names.
The single catalog serves listing, help, loading, Flow capture and export.
Removed `resolve_local_definition`, the copied export map,
pruning cycle state (pruning never follows directory symlinks), and duplicate
description parsers. Listings and exports share authored-description extraction.
Prompt accounting uses the retained Skill source instead of rediscovering a
possibly different file after capture.

Removed `find_skill_source_path` and its exclusive fixture: help reads
the loaded Skill's provenance without another catalog scan. Flow lookup drops
its duplicate namespace branch and the two path-list helpers. `prepare_native`
is folded into its sole caller, `claude_plugin`; snapshots write the destination
instructions once, retaining native bytes, port declarations and sibling links.
Review retained the separate context-result count: an acknowledgement still
cannot finish the skill. Native selection now avoids cloning skills that stay inline.

Flow child source rediscovery and implicit skill re-execution on follow-up are
also removed. Structured subsequent input remains unfinished; pre-claim capture replacement is
removed by the admission repair above.

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

Check (2026-10-08): `cargo test -p loopflow --test session_cli_tests --no-run` builds; network-isolated `session_cli_tests headless_resume` and `session_lifecycle_tests taskless_structured_output_correction_is_bounded_and_preserves_the_conversation` pass; `codex_request_mapping.py --redelivery` reproduces both counterexamples; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, Ruff and `lf context --skill implement --json` pass. Gate/review retain full fidelity, cost and third-party acceptance.
