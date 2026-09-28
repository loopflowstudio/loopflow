> Transferred on 2026-09-28 at Jack's request. Active hardening now lives in
> `/Users/jack/src/loopflow.session-launch-hardening/scratch/jack-heart/session-launch-hardening.md`,
> on a branch created directly from main. The source below is historical context.
> Source edits were transferred and removed here; LOO-298 work was not changed.

# 5 Whys: review launch failure and disappearing Task state

2026-09-28. Jack requested this analysis after opening a review failed with
`unexpected argument '--tui'` and the current CLI could no longer find LOO-332
or LOO-333. Jack then assigned this worktree to the hardening, since the product
work has its own Tasks. No installation, provider account, Task, or Session was
changed by this analysis or branch preparation. The accepted product designs
remain archived in `../discord.md`, `../task-automation.md`, and
`../recursive-vsm.md`; their destination worktrees own implementation.

## Branch scope and implementation design

This worktree (`loopflow.discord`, branch `jack-heart/discord`) now owns reliable
access to existing reviews across installation changes and diagnosable Session
startup. Keep the existing branch and checkout; their names need not change.
LOO-332 retains scheduled Task delivery; LOO-333 retains recursive Wave/VSM
operation. LOO-298 retains the Session/Run/Exec data model. No new Task was filed.

**Target behavior.** Jack opens an existing review in Ghostty and reaches the
same conversation and pending decision even if the selected installation has
changed. An early failure identifies the attempted executable and owning data
without requiring Jack to reconstruct installation history. A later retry keeps
the failed evidence and does not approve the review, duplicate the Task, or
overwrite either installation's private data.

**Ownership.** Reuse LOO-298's Session identity, Run attempts, Exec provenance,
and the existing installation receipts. Resolve the compatible executable and
store together for an existing review. Installation selection must not silently
turn an existing-work lookup into a claim that the work is absent everywhere.
If two retained copies diverge, surface that concrete ambiguity rather than
selecting the newest timestamp or merging rows. No second Session registry,
launch-attempt store, or automatic database synchronization.

**Integration evidence.** On inspection, LOO-298 is at `ce97003cf` and already
has native conversation handoff plus Exec lifecycle/admission work. Its
`spawn_session_run` still reports early exit without the child runtime in the
error, but its Session persistence differs materially from this checkout.
Integrate against those actual owners before implementing persistent launch
evidence. Do not add a temporary parallel record to this older model.
`lf rebase --plan` currently proposes `generated_only / reset_to_base` against
main; that does not establish a correct integration onto LOO-298. No rebase was
applied. Preserve these designs through the eventual supported integration.

**This slice: establish the hardening boundary and integration requirements.**
The RCA is checkpointed; this design assigns the remaining work. Source changes
and behavioral proofs have not begun. The initial idea of immediately adding
pre-spawn records to the older Run implementation is superseded by the observed
LOO-298 Exec owner. The next implementation slice must use that owner.

**Implementation order after integration.**

1. Resolve and record the child execution context before startup through the
   existing Exec/Run evidence. Include actual binary identity, owning store,
   cwd and sanitized operation; retain spawn failure/early exit/timeout across
   a later successful attempt. Keep prompts and credentials out of diagnostics.
2. Make ordinary existing-Session discovery/opening retain access when the
   machine selects another installation. Reuse compatible installation receipts
   and normal Session operations; preserve review ownership and provider history.
3. Exercise the installed terminal path, then align the builtin control skill
   and user docs with the observed open/preparation semantics.

**Done when.** A disposable installation test prepares a review under A,
selects B with a separate store, and opens the same review through its supported
owner. A simulated child rejects arguments before provider startup; evidence
names that actual child and survives retry. Both stores remain intact, no
competing Task Flow starts, and the review remains pending until explicitly
completed. Finally demonstrate a real Ghostty open/resume; simulated providers,
JSON preparation, an opened window, or an installation preview alone do not
satisfy that final proof.

**Immediate recovery remains separate.** The retained `88ff4519…` installation
and `local-afee63d734c7482cb94d1071af26d9ea` database passed exact-store
preflight and full restoration preview: 33 executable references resolve,
`promote (no migration to apply)`. No global restoration was applied. Branch
hardening does not imply changing the currently selected installation.

## The Problem

Jack could not reliably reach existing review Sessions: one child CLI rejected
its launch arguments, and a subsequent installation selection hid the Tasks
and Run records needed to investigate and recover it.

## Observations and limits

These are related symptoms, not yet one proven causal chain.

1. Jack supplied the terminal error for
   `run_bcccb7c9a14e46daa2432efcc9141d30`: the child rejected `--tui` and exited
   with status 2 before becoming resumable. The terminal excerpt does not name
   the child executable or its installation/store. Its login timestamp does
   not establish when the failed launch occurred.
2. The current `lf runs <id> --json` reports that Run absent on this Home.
   Current Task status likewise reports LOO-332 and LOO-333 absent. These are
   current-selection lookup results, not evidence of deletion.
3. Installation receipt `switch-bf3e5be1f3104e6494ec4d6c74a9bfc4.json`, modified
   at 13:09 local time, records a settled switch from development installation
   `local-afee63d734c7482cb94d1071af26d9ea` to
   `published-a412473db10544eeba1f7b30e64605c9`. The selected database changed
   from `~/.lf-dev/installed/local-afee63d734c7482cb94d1071af26d9ea/loopflow.db`
   to `~/.lf/loopflow.db`. The CLI artifact changed from digest prefix
   `88ff4519` to `d7bf7c66`. The receipt alone does not establish who requested
   the switch or why.
4. Read-only SQLite inspection of that retained development database found
   both Task identities and their expected worktrees. This is retained-state
   evidence, not a replacement for the shared current-state readers.
5. The failed Run exists under the same retained installation. Its Ask record
   is `ask_once_169971ecaee50b33611f3b686370df5356000e7ea7b19ccff7ae460554e5375d`.
   It belongs to LOO-332 and asks to resolve “repeat at step 3 requires a
   decision,” following failed loop-decide Run
   `run_c699e1712ec5445ea46769dd110a8f3b`. It is an unblock boundary, not proof
   that the Task reached its planned demo.
6. That Run now has a provider-session record and provider-client metadata
   modified at 13:31. Its manifest names runtime `lf-88ff4519…`, with a null
   runtime digest. Newer records cannot identify the executable used by the
   earlier failed attempt or establish that the review completed. No recovery
   was performed by this analysis.
7. Harmless `--tui --help` checks pass now for the installation gate, retained
   development CLI, retained development app helper, current app helper, and
   LOO-332's current debug CLI. The older Cargo-installed CLI also advertises
   `--tui`. This contradicts treating any of these current bytes as the proven
   offending binary. Different earlier bytes, a wrapper, or argument rewriting
   remain hypotheses.
8. Earlier attempts to open LOO-329/330/331 reached Claude but failed with “No
   conversation found.” That is a separate provider-resume failure, not the
   same observed CLI parsing failure. Their cause is not established here.

Receipt source: `~/.lf-machine/install/receipts/`. Run/Ask sources:
`~/.lf-dev/installed/local-afee63d734c7482cb94d1071af26d9ea/{runs,human-sessions}/`.
Source inspection below uses this checkout at `80140f582`; the prior install
receipt reports source revision `280670217`. Current source explains relevant
mechanisms but is not proof of the original failing executable's behavior.

## Chain

Unreachable reviews → selection-scoped lookup → installation switches both
code and data → retained work lacks ordinary discovery across that boundary →
installation correctness does not ensure continuity of access to existing work.

**Problem**: Existing Tasks and review evidence became unreachable through the
ordinary CLI during recovery from a failed review launch.

**Why 1**: The commands queried the newly selected published database. The
Tasks and failed Run remained in the previous development installation.
The switch receipt and read-only retained records establish this distinction.

↳ Earlier feedback should identify the selected installation and retained owner
when an exact Task/Session lookup misses. “Not found here” must not imply gone.

**Why 2**: Selecting an installation also selects its database and observability
directory. `store/mod.rs::database_path_from_env` returns `selection.store`;
`default_lf_home_dir` derives the directory from that store. The stable Home ID
does not itself distinguish these installation-specific histories.

↳ Preserve ordinary access to retained work when the selected database changes;
do not fix discovery by making a new binary open an incompatible old store.

**Why 3**: Session opening resolves through current execution context. In the
inspected source, `human_open_argv` contains the resolved CLI and Session ID;
`serve_ask_locked` resolves the CLI again and constructs `--tui` arguments.
`current_home_execution_context` explicitly chooses the current Home rather
than historical control pins. This assumes current scope can still reach the
work. The observed store switch breaks that assumption for these Tasks.

↳ Resolve an existing Session through its owning installation/store as one
unit, or perform an explicit supported transfer. Reuse existing installation
receipts and execution context; avoid another Session registry or cursor.

**Why 4**: Isolation and activation have explicit ownership, but the inspected
handoff does not carry a complete continuity contract through preparation,
terminal startup, child launch, and later recovery. It also leaves a diagnostic
gap: `spawn_session_run` reports Run ID and exit status without recording the
resolved child executable/store in that failure. Runtime identity is finalized
inside the launched CLI, too late for an argument-parser failure. Later launch
metadata cannot reconstruct that failed attempt.

↳ Record a small, sanitized pre-spawn attempt on the existing Run: executable
identity, installation/store, cwd, operation/flag names, and exit outcome. Do
not record credentials, full environment, or prompt bodies. Use the resolved
execution context for both launch and diagnostics.

**Why 5 (Root)**: The supported boundary is narrower than the experience Jack
needs. Installation selection, Session preparation, and provider startup each
have proofs, but their composition must preserve access to the same work across
installation changes and must explain failures before the provider starts.
Isolation is necessary; loss of discoverability is not an acceptable consequence.

The inspected tests demonstrate that gap without establishing that the entire
repository lacks coverage: `session_cli_tests::command` pins CLI and Home to
one fixture; `boundary_launch_and_resume_remain_openable_while_provider_waits`
uses a simulated provider without an installation switch. The development
handoff test deliberately puts a wrong CLI on PATH, but follows `--json`
preparation/reopening without launching the provider. The disposable installation
suite includes review-readiness ownership, not this prepare → switch → terminal
open sequence. These are useful proofs with narrower boundaries.

### Where the flag-error chain stops

The supplied parser error proves a launch-contract failure before provider
startup. It does **not** prove which binary rejected the flag or that the later
installation switch caused it. The source selects a child and supplies `--tui`,
but the failed attempt's exact executable/argv are not available in the inspected
records. Further “whys” about that specific mismatch would be speculation.
The established systemic defect here is insufficient pre-spawn provenance;
changing flag syntax without identifying the rejected invocation is unjustified.

## Unanswered Whys

| Branch Point | Unexplored Question | Priority |
|--------------|---------------------|----------|
| Flag rejection | Which exact executable, wrapper and argv rejected `--tui`? Can terminal or launch logs preserve the original attempt? | High |
| Installation change | What invoked the 13:09 switch, and what continuity contract was intended for development-owned work? | High |
| Newer Run records | What performed the later launch, and did it reach a usable review or only provider registration? | High |
| LOO-332 boundary | Why did loop-decide omit the required decision? Separate from terminal startup. | High |
| Provider resume | Why did 329/330/331 have Claude resume IDs without discoverable conversations in the selected account? | High |
| Test coverage | Does the final LOO-298 implementation already solve any of these boundaries beyond this older checkout? Reconcile before filing prevention. | High |

## Fixes

| Level | Fix | Prevents |
|-------|-----|----------|
| Immediate | Recover the exact retained Session through its compatible installation/store and supported open path, after checking newer activity; preserve its pending decision. | Losing the existing review or launching a duplicate |
| Structural | Resolve Session ownership and child execution from one existing installation context; retain access across selection changes without merging incompatible databases. | Current selection making existing work appear absent |
| Diagnostic | Persist sanitized pre-spawn provenance and failure outcome on the existing Run before the child parses arguments. | An unidentifiable failed child being mistaken for the currently installed CLI |
| Systemic | Prove the whole installed review journey across a selection change, including pre-provider failure and same-Session retry. | Passing component tests while the actual review remains inaccessible |
| Skill/process | Treat `session open --json` as preparation; follow returned launch instructions in the requested terminal and verify provider readiness. Distinguish CLI failure, provider-resume failure and absent selected-store records. | Reporting preparation as an opened review or collapsing different incidents into one guess |

## Changes to Implement

- [ ] Reconcile this evidence with LOO-298's current Session/Run model and any
  ongoing recovery before choosing the implementation owner.
- [ ] First coherent prevention: reuse the resolved execution context to record
  pre-spawn executable/store evidence and an actionable failed-attempt outcome;
  preserve it when the same Session retries successfully.
- [ ] Add one behavioral proof: a child rejects its arguments before provider
  startup; the failure names its actual runtime/store and the same Session can
  recover without completing review or losing the prior failed attempt.
- [ ] Follow with continuity: prepare a review in installation A, select B with
  a separate store, and open that existing review in a fresh terminal. Reach
  the same owning work through supported routing; preserve both stores and
  leave the review gate pending. Simulate only the provider in automated proof.
- [ ] Demonstrate the real Ghostty path with the appropriate retained Session
  once its current activity is known. A window, JSON record, provider ID, or
  successful help command alone does not satisfy this proof.

Review finding: preserving development/published database isolation is essential.
Neither copying tables into the published store nor replacing `--tui` blindly
addresses the demonstrated ownership and evidence gaps. No implementation,
release, or live recovery is claimed by this report.
