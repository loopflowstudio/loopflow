# Open Sessions from persisted Claude history

Status: implementation design, proposed by kickoff on 2026-09-28; not a record
of Jack Heart's approval. Continues the [incident analysis](session-opening-incident.md)
and its [restoration receipt](session-opening-proof.json).

## Problem

Jack Heart could not open three waiting reviews from either the CLI or desktop.
Loopflow allocated Claude conversation IDs, but those IDs had no saved native
conversations. Every open retried `claude --resume` against the allocation.
The three reviews were restored; this change prevents the same first-launch
failure from stranding another Session.

A Session must survive a provider exiting before saving history. Its boundary,
review state, context, and account remain authoritative. Starting a process,
allocating an ID, saving a conversation, and completing a review are different
events. Opening must honor those distinctions without a second Session lifecycle.

## The demo

Run `lf session open '<boundary-id>'` after its first Claude launch exited
without saving history: the same named Session opens with its original seed.
After Claude saves a turn, exit and run that identical command again: it resumes
the saved conversation, with no review completion or Flow advancement.

## Approach

Keep `provider-session.json` as the durable **reference** to the intended native
ID and account. Independently inspect Claude's native history before treating
that reference as persisted. Extend the existing explicit-open replacement
path to a consumed Claude launch with confirmed absent history. Preserve all
boundary state and prepare its replacement before publishing the new binding.

### Ownership and integration

| Existing owner | Change |
| --- | --- |
| `run_record.rs::ProviderSessionRef`, `read_provider_session` | Reference remains routing/correlation evidence. Old receipts and event-derived references receive the same inspection. Add an optional history root to this private receipt, populated from the effective Claude launch environment. |
| `provider_account.rs` | Resolve the recorded account's history location read-only. Expose the effective local history root from the existing route; do not create directories, refresh credentials, or select another account to inspect history. |
| `harness/claude.rs` (small sibling history module if needed) | Own one Claude-specific history inspector with filesystem errors preserved. No provider registry or SDK subprocess. |
| `lf/commands/util.rs` | Keep pre-spawn identity/account recording. Record the effective history root, and make inspect → stop → spawn → client publication one operation under the existing provider-client lock. Split starting a client from waiting for exit. |
| `lf/commands/run.rs::launch_prompt` | Emit Claude `ProviderSessionObserved` only after independent persistence evidence. Preserve process exit outcome separately. Capture the actual TUI seed for recovery. |
| `ops/human_session.rs` | Use the inspector for opening; separate owned startup from persistence. Extend Ask/Task replacement and remove recovery's readiness clearing. |
| `ops/flow_session.rs` | Apply the same classification to standalone Flow recovery, which currently preserves any receipt forever. Remove its readiness clearing too. |
| `store/durable.rs`, `store/sqlite/durable.rs` | Add a narrow conditional Session binding update. The general setter resets worker generation. Retain existing claim ordering: claims carry the position version and cannot survive a version increment unchanged. |
| Session CLI tests and Session README | Prove and document ordinary open recovery, pending startup, preserved history, and unresolved inspection. No new public state or flag. |

### Native evidence

Use an internal `Result<ClaudeHistory>` with `Absent` and `Persisted`; an error
means unresolved inspection. `Persisted` describes local conversation evidence,
not a guarantee that every future provider resume will succeed.

1. Locate history in the **launching Home and recorded account**. For new Runs,
   record the effective `CLAUDE_CONFIG_DIR` (or Claude's default) in the existing
   private receipt, after account/environment application. Lease-based routes
   can use ambient local storage despite identifying a managed account;
   account ID alone does not imply an account-directory location.
2. For old managed receipts without a root, use the exact recorded local
   account's configured Home. Do not synthesize an empty directory or substitute
   the current preferred account. Old ambient/forwarded receipts whose historical
   root cannot be established remain unresolved. The three incident receipts
   have recorded local managed accounts and are covered.
3. Inspect exact `<session-id>.jsonl` candidates under that root's `projects`
   directories, including directories other than the current cwd. Do not check
   only a sanitized cwd path: custom names, long paths and moves make that
   incomplete. Do not search other accounts or subagent transcripts.
4. Positively recognize complete conversation records: a matching session ID,
   UUID and timestamp, with user/assistant message content, or a native system
   `local_command` record with content. These shapes have installed-CLI loader
   evidence. Stream records; do not log transcripts or mirror the provider's
   entire conversation-chain parser. A truncated final line after recognized
   records does not erase those records.
5. `Absent` requires a known, accessible root and a successful search with no
   matching file, or only readable zero-byte matching files. A missing `projects`
   child under an accessible root is empty storage. A missing account root,
   inaccessible directory, unreadable candidate, malformed nonempty file, or
   metadata-only/unknown transcript is unresolved, never permission to replace.
   Multiple candidates are preserved; ambiguity cannot authorize replacement.
   An unresolved scan cannot become a definitive absence result.

This deliberately leaves metadata-only files requiring diagnosis. The incident
has **no matching files**, and a conservative predicate fixes it without
discarding partial or unfamiliar histories. Only the provider can authoritatively
accept a resume. If existing history is rejected by Claude, surface that error
and retain the binding; never restart with the seed on arbitrary resume failure.

`ProviderSessionObserved` on new interactive Claude launches means inspection
found persistence. A zero process exit with `Absent` does not emit it. An
inspection failure must not rewrite the process outcome or hide its error.
Historical events keep their bytes and do not bypass inspection. Codex and
OpenCode observation semantics are unchanged in this slice.

### Opening and client handoff

Prepared boundaries are already durable and listable before provider startup.
Keep that property. A live owned process may be `Active` while still starting;
neither `Active` nor `open_argv` promises persistence.

Replace `session_run_is_resumable` as the startup shortcut with an explicit
owned-start check. `spawn_session_run` can return its supervised child and release
the boundary launch lock once the native client is owned, even if history is
pending. It must not kill an owned client because transcript creation exceeds
the existing 30-second startup deadline. That deadline still diagnoses a child
that never establishes ownership. Do not weaken the history predicate used for
native resume to match this startup predicate.

Explicit open takes the boundary launch lock, rereads the boundary, and inspects
the current Run under its provider-client lock:

| Observation | Action |
| --- | --- |
| Prepared Run | Launch once through the existing prepared-marker claim. |
| Live owned client, no persisted history yet | Leave it running. Wait at most 30 seconds for persistence or exit, then report that the Session is still starting. Retain its binding and release locks between probes. |
| Persisted history | Move the owned client only after checking history; reinspect after exit, then resume the same ID/account. Failed handoff never selects replacement. |
| Consumed Run, settled launcher, no owned client, confirmed absent history | Recheck under both locks; prepare and publish one replacement. Provider-reference presence and exit code do not change this decision. |
| Inaccessible or ambiguous history/account/ownership | Explain the failed inspection and perform no stop or replacement. |

Lock order is boundary launch lock → provider-client lock. SQLite/Flow record
transactions stay short and never wait for a provider. Split the blocking
command helper into start/publication and wait phases using the existing
child/`ProviderClientGuard` lifetime. Release both locks after client registration,
before waiting for its interactive lifetime. Ready, rename, list and metadata
open remain usable. Avoid reacquiring the same file lock through a wrapper.

For competing openers, capture the client generation seen when open began.
A newly published live client from another opener is the result of that open:
report its ownership instead of immediately moving it again. A later separate
open may still move it. Retain PID/start-time ownership checks. For receipt-bearing
absence recovery, require the existing terminal receipt and no `launching` marker
as well as no owned client. This avoids guessing that a launcher between marker
consumption and client publication has died. The manifest does not record a
launcher PID. A crash leaving no terminal evidence stays unresolved; do not
invent a new ownership receipt to broaden this slice. Normal first-provider
exits, including the three incident Runs, have terminal receipts.

Metadata-only open and list must not resume or replace a bound Run. Retain their
existing ability to prepare an old unbound boundary. Move standalone Flow's
destructive recovery behind the actual-open branch.

### Replacement preserves the Session

Prepare a candidate before its binding changes. Copy the Session name, record
a Run parent link to the replaced Run, pin the recorded Claude account/history
root, and retain cwd, model, seed and boundary token. The Ask record's original
requester/parent attribution stays unchanged. Old manifests, events, context,
receipts and provider files stay intact.

For new TUI Runs, capture `built.prompt` as the actual delivered context channel
instead of only separately assembled headless channels. Recovery reads that
existing private artifact and reuses it through normal launch; it does not
reconstruct a prompt from today's scratch, skill or account defaults. Older
captures contain system/task channels rather than the exact TUI string: preserve
both archived channels in order, with a separator. This preserves their context;
byte equality with the historical TUI seed is not claimed. Missing/corrupt
archived context for a receipt-bearing launch is unresolved, not permission to
substitute current repository content. Never-started preparation retains normal
first-launch assembly.

Feed saved inputs through the existing prepared Run path and
`launch_session_with_env`; no public replay command or second launcher.
Propagate the recorded account explicitly through fresh launch as well as
resume. Resolve it once and inspect the same effective root the command uses.
Failure to route that account is unresolved, not failover authorization.

Publish as follows:

- **Ask:** under its launch lock, reread and retain the complete record; change
  only `session_run_id` through the atomic file writer.
- **Task review:** compare Task, invocation, boundary, version and old Run in a
  narrow store update. Change only `session_run_id` and increment
  `position_version`. Preserve cursor/review, readiness, failure, worker generation
  and timestamps. Normal human positions cannot acquire worker claims through
  `claim_task_worker`; retain the existing `claim_json IS NULL` mutation
  condition. A changed/claimed position is not ours to rebind: reread and preserve
  its owner's authority, without rewriting the claim's embedded version or
  adding a claim-transfer protocol. Concurrent advancement/completion loses the
  comparison; never launch the losing candidate.
- **Standalone Flow:** use locked `flow_run::update`; change only the active
  boundary's Run ID/directory after candidate preparation. Preserve readiness,
  completed flag, cursor, failure, invocation and other content.

Do not persist an intermediate `None` binding. Preparation, context read or
name-copy failure leaves the old binding. A losing candidate stays an unlaunched
private prepared artifact, never an independent Session.

After binding, retire the superseded no-history Run's independent surface through
the existing provider-resolution marker. This is Run retirement, not boundary
completion. Use the replacement's parent link to finish cleanup idempotently
after interruption. Listing must exclude a bound replacement's superseded
no-history ancestor during that window without hiding genuine historical
conversations. No successor table or rewritten original evidence.

## De-risking

| Question | Finding | Impact on design |
| --- | --- | --- |
| What proves persistence? | Claude 2.1.284 rejects absent, empty and snapshot-only files. It loads a complete user record and native local-command history. A minimal synthetic assistant made a nonempty fixture fail; adding timestamp/cwd/version made it load without isolating which field mattered. | Neither existence nor message count certifies resume. Recognize narrow evidence; preserve unknown files. See [probes](session-opening-research.md). |
| Can first launch exit without history? | An isolated stream process was alive, received SIGTERM before input, exited 143 and left no transcript. Resume failed. A subsequent native local-command launch persisted history and native resume returned 0. | Exit and persistence are independent. This loader/lifecycle probe does not satisfy the Loopflow TUI demo. |
| Does cross-directory lookup work? | Print-mode loader found user-containing history in another project, but rejected command-only history there that loaded in the cwd project. | Search all candidates to prevent false absence. Do not promise lookup success or relocate transcripts. |
| Does the SDK solve inspection? | Its reader catches `OSError` and returns no session. Provider docs say transcript schema is internal. | No SDK/runtime dependency or SDK absence as replacement authority. Bound the parser and preserve uncertainty. |
| Can opening terminate startup? | `resume_native_run` stops clients before reading the receipt; `spawn_session_run` has `kill_on_drop` and a 30-second resumability deadline. | Inspect before stop; separate owned startup from saved history. |
| Are all boundaries covered? | Ask/Task and standalone Flow have separate replacement callers; all trust receipts and clear readiness. | Shared inspection integrated into all three existing stores. |
| Is binding update state-preserving? | The general setter resets generation. Claims embed position versions, and the claim API forbids claiming human review positions. | Preserve generation and existing unclaimed-position ordering. Never increment underneath a nonempty claim; no new claim-transfer mechanism is needed. |
| Are seed/account retained today? | Replacement reassembles current context and fresh launch routes independently. TUI capture records split channels although launch passes `built.prompt`. | Freeze the real seed and carry archived context and exact account into replacement. |
| What caused the incident exits? | Unknown: original terminal output was not captured. Original hashes still match; restored histories still exist. | No attribution to auth, EOF, cancellation or handoff. Prevention does not depend on guessing. |
| Other providers and desktop? | Their persistence contracts are unverified here; no rendering environment exists. | Claude scope. CLI evidence does not prove desktop rendering. |

## Alternatives considered

| Approach | Tradeoff | Why not |
| --- | --- | --- |
| Resume first; restart on missing-session error | Avoids a local reader. | Interactive error is unstructured and the installed loader can reject existing files. Does not make stopping startup safe. Blocked on the same evidence problem. |
| Publish reference only after a hook/successful exit | Reduces premature receipt publication. | Loses early account/ID routing and leaves old receipts broken. Hooks and successful exits do not establish persistence. |
| Retain allocation; inspect persistence; recover through open | Needs a small version-sensitive reader and unresolved outcomes. | Chosen: fixes old local receipts, preserves uncertain history and reuses boundary/ownership mechanisms. |

## Key decisions

- Native history is evidence; receipts are identity. No cached boolean becomes
  a second persistence authority.
- Owned startup and persistence have separate predicates. A prepared or starting
  boundary remains visible; readiness still belongs to review feedback.
- Recovery requires **absence plus quiescence**, not exit status, absence at one
  guessed path, or a provider error string.
- The identity Jack Heart opens survives; only its backing Run changes. No
  `--replace`, database repair, new Session DTO state or operator decision.
- Preserve ambiguous transcripts even when a disposable equivalent was rejected.
  Safe recognition can be narrower than Claude's full parser.
- Publication meaning, startup/handoff, replacement integrity and proof are one
  indivisible change. A reader alone leaves the incident open.

Wild success: a failed first launch becomes an ordinary second open, with the
same name/seed; later opens resume Jack Heart's conversation. Wild failure: an
upstream format change or wrong root looks absent and silently restarts a review.
Conservative errors and upgrade probes prevent that failure, even when they
mean reporting an unresolved open.

## Scope

- In scope: interactive Claude references, persistence inspection, startup and
  handoff, explicit recovery for Ask/Task/standalone Flow boundaries, seed/account
  carryover, focused tests and documentation.
- Out of scope: Codex/OpenCode persistence changes, retry services, automatic
  advancement, restoring deleted transcripts, replacing arbitrary unbound
  interactive conversations, auth repair, binary installation, assigning the
  original exit cause, or claiming desktop rendering proof.
- Follow-on: bounded private startup failure diagnostics that preserve the
  observed exit/error without credentials or entire environments. Independently
  useful; not a prerequisite for this prevention.

## Done when

1. Parameterize a CLI proof: fake Claude receives its seed and exits before
   writing history, once 0 and once nonzero. Run retains ID/account without a
   persistence observation. Same boundary open creates one prepared replacement,
   receives archived seed markers from both channels, and later resumes its
   saved ID. Change current context/model/account defaults before reopening to
   catch accidental regeneration/rerouting.
2. Preserve binding/files for existing history (also another project), permission
   failure, missing account root, metadata-only/malformed history, and resume
   failure. Cover zero-byte and absent files separately. Simulate I/O errors
   deterministically instead of relying on chmod under root.
3. A controlled delayed writer is not stopped or marked observed early by open.
   A delay past 30s leaves an owned client alive. After persistence, handoff
   resumes it. A competing opener cannot publish another replacement or duplicate
   client. Use barriers for race tests, not timing retries.
4. Exercise Ask, Task and standalone Flow owners. Assert names, context, account,
   readiness (including nonempty readiness), review feedback, cursor, invocation,
   failure and generation remain unchanged. Verify a concurrently advanced or
   claimed position defeats stale recovery without altering the new owner's claim.
   Stale CAS/preparation failure
   preserves the old binding. Concurrent list sees an old or new prepared Run.
   Test interrupted retirement cleanup.
5. In a disposable account/workspace, demonstrate the **real CLI TUI path**:
   interrupt first start without history → same `lf session open` gets its
   original seed → save an identifiable response → exit → same command reopens
   it. Record versions, IDs, command, native evidence and boundary comparisons.
   No production review, invented feedback, completion or advancement. Use the
   normal secret-management path if credentials are needed; never copy account
   dotfiles. This remains implementation proof; kickoff's print-mode and local
   command probes do not satisfy it.

Focused commands during implementation:

```bash
cargo test -p loopflow --test session_cli_tests
cargo test -p loopflow --lib ops::human_session::tests -- --test-threads=1
cargo test -p loopflow --lib ops::flow_session::tests -- --test-threads=1
# Run new history/binding tests by their focused names.
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Start with the smallest new behavioral proof. Gate owns affected suites once;
reuse unchanged evidence instead of repeating larger suites each phase.
Label fake-provider tests simulations. Desktop uses the same `open_argv`; a
separate rendering check is required before claiming desktop rendering works.

## Forbidden outcomes

- Receipt/hook ID, successful exit or a nonempty file treated as proof of resume.
- Replacement of unreadable, malformed, ambiguously located or starting history;
  restarting on arbitrary provider failure.
- A different account or today's reconstructed prompt in the replacement.
- Cleared readiness/feedback, lost claims, advanced Flow, or a null/unprepared
  binding while fixing the backing Run.
- Retry daemons, parallel recovery commands, duplicate Session identities,
  transcript mirrors or SDK dependency for a local inspection.
- Locks held for the interactive lifetime, duplicate clients, or orphan Sessions
  after replacement.
- Loader fixtures, print-mode probes or metadata-only opens called proof of the
  real seeded TUI experience.

## Internal slices

1. **Evidence contract:** Claude inspector/root provenance, exact TUI context
   capture, and allocation/observation separation. Keep one reference.
2. **Opening integration:** separate start/wait, pending handoff and lock scope;
   preserve all three boundaries; carry seed/account; retire superseded surfaces.
3. **Acceptance:** finish race/state tests, real disposable TUI demo and Session
   docs; review the complete change for simpler ownership.

These are implementation cuts of one change, not shippable partial fixes.
Diagnostics remains a later independent slice.

## This slice

Implement the complete Claude prevention. Its focused proof is one
receipt-present/history-absent CLI scenario covering exit 0/nonzero, opening with
saved context, then reopening persisted history. Add safety/state cases and the
real TUI proof before calling it complete.

## Slice ledger

- 2026-09-28, prior restore: three reviews repaired and native open/resume recorded
  in [incident evidence](session-opening-incident.md). No review completed.
- 2026-09-28, kickoff at `a11a6567d`: mapped writers/three boundary stores; ran
  Claude 2.1.284 loader/lifecycle probes in disposable directories; rechecked
  original hashes/restored histories. No production code, installed runtime,
  credentials or Session bindings changed. Source tests inspected; no suite
  rerun for this design-only pass.
- Review changed the design: separate startup from persistence to avoid killing
  delayed writers; binding-only Task update; preserve standalone Flow readiness;
  frozen context/account carryover; retain metadata-only and cross-directory
  counterexamples. A final store audit found claims embed position versions;
  human positions cannot normally be claimed, and recovery must retain that
  existing ownership ordering rather than incrementing underneath a claim.
  See [assumptions](questions.md).
