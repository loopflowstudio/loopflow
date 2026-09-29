# Concept review · LOO-298 · iteration 9

2026-09-28 · HEAD `fb100f486cf69d03bd87a6fda748c2c75588a906` plus retained working edits.

**Judgment: keep the accepted model; implementation remains incomplete.**
This is the autonomous review after an explicitly incomplete slice, not Jack
Heart's requested code-complete concept review. The next owner conversion needs
no new product decision. The native retry limitation remains a separate open
implementation tradeoff. This review supplies evidence and selects no Flow edge.

The supplied immutable concept-review instructions govern this invocation. The
installed skill and current builtin now describe an interactive-only review;
they do not replace this captured autonomous boundary. No additional Session or
worker is needed. Supervisor comment `5f7f99f4-ecc7-4973-8ee8-da9a6ea042b4`
confirms this boundary and ownership of this document.

## Experience and usage

Jack needs to find and continue the same conversation from Desktop or terminal,
keep its name and Task assignment, and recover failed work without tracking
internal launch identities. A command's failure, a provider turn's failure and
a Flow's pending decision remain distinct facts because recovery depends on them.

The accepted usage is already clear in [CLI Sessions](../docs/lf.md#sessions).
Keep that guidance and its link to the single
[implementation-status owner](../docs/architecture-reference.md#cutover-status).
This concrete walkthrough is the review draft, not a claim of end-to-end proof:

```sh
lf -b implement
lf session list --interactive false --json
lf session rename SESSION 'Parser review'
lf session bind SESSION --task LOO-298
lf session connect SESSION
```

Use the Session ID from the list. The initial list does not require a Task filter:
the conversation may still be unbound. Bind states the permanent target and
writes once; same-target retry is a no-op, and done/landed Tasks remain eligible.
First bind sets Started at assignment time. Earlier usage keeps its recorded
owner under the explicitly provisional prospective-attribution assumption.

After the driving lf process exits, connecting again must retain the same
Session, native conversation, name, feedback and assignment. A surviving engine
is reused. Explicit restart must preserve the conversation and shared-engine
siblings; public `--restart` is still unimplemented, so this walkthrough does
not advertise that command. Existing `--replace` denotes client replacement and
cannot be presented as proof of the full engine-restart contract.

For a failed Flow step, continuation retains the failed outcome and records the
successful successor in the same conversation. Only the selected successful
completion may settle the boundary. A review still requires explicit completion;
connecting, closing a pane or marking ready never supplies navigation.

Affected guidance remains `docs/lf.md`, `docs/architecture-reference.md`,
`docs/architecture/{execution,data}.md`, README/index and the returned Chapter
skills. No new usage concept or skill rewrite is proposed. At final consistency,
refresh the existing status map: it still lists mechanical history conversion as
unfinished and records an older checkpoint. Keep detailed receipts in the working
evidence rather than adding another public status ledger.

## Model in one screen

| Action or fact | Owner and identity | State-changing API responsibility |
| --- | --- | --- |
| Invoke any lf command | Exec / `execs`; one actual process | Admit once, record causal parent/caller, record observed command outcome once |
| Converse, rename, bind, connect | AgentSession / `agent_sessions`; stable conversation ID | Reserve/publish captured input; rename/bind on this owner; transfer driver separately from provider generation |
| Retry or continue an agent | AgentSession history; exact native turn/event references | Append outcome/usage with original attribution; preserve prior events and missing evidence |
| Start/resume a Flow | FlowSession / `flow_sessions`; captured graph and cursor | Select exact agent completion or mechanical history; consume under Flow version/claim with advancement |
| Run managed Task work | Task's selected FlowSession pointer | Use the common driver; other attributed Flows remain possible |
| Rotate a Chapter | Existing Linear Projects and statuses | Converge on explicit target and stable Project IDs; retain active Task identity and Project Flow |

Provider process/endpoint evidence supports connection and exact control. It is
not a fourth execution lifecycle. History entries are references, not resumable
attempt objects. Causal Exec ancestry grants neither signaling nor settlement.

## Findings and consequences

### 1. Navigation attribution is conceptually right but fails valid retry

Invariant: **a decision belongs to the native turn that issued it and can be
consumed only with that selected turn's successful completion.**

Current source creates the caller token at `harness/codex.rs:1098`, transports
it in the thread's tool environment, and compares it with the admitted child
Exec in `store/sqlite/flows.rs:618` (`require_turn_authority`). Decide, route and
blocked use that shared authority boundary. These are subordinate facts on the
accepted owners; no new product object is justified.

The retained slice results on candidate `66f410dd…f7d0280` show both sides:

- `review-slice-native-late`: the old child exits 1 with original-caller rejection;
  the retry supplies no decision and the Flow stays unresolved.
- `review-slice-native-replace`: the legitimate successor's Advance receives the
  same rejection. The command exits 1; selection moves from 5 to 8 but successful
  completion 11 is not consumed.

Those actual CLI/Codex fixtures use synthetic Responses and private Homes.
This review reread their saved results; it launched no provider or behavioral test.
The failed-thread environment does not carry the new token, as detailed in the
[bounded native experiment](native-turn-retry-tradeoff.md). Looking up the latest
token for an old child would restore the reproduced stale-writer defect.

Keep automatic retries and original-turn attribution as requirements. The smallest
recorded native proposal extends the provider's existing unsubscribed-thread
reload eligibility to a completed failed thread, preserving no-active-turn and
no-subscriber conditions. That proposal is unimplemented and does not establish
upstream availability, delivery cost or sibling preservation. An upstream fix,
provider fork, tool proxy or shared-engine replacement is not selected here.
Requiring explicit retry alone has not been shown to refresh the environment.

Before any substantial native expansion, return a concrete implementation cost
and paired proof to the supervisor. Any reduction of retry behavior needs Jack's
decision. This unresolved tradeoff does not block independent owner/import work.

### 2. Run still determines whether a conversation can exist and launch

The normal path still crosses a fourth owner:

| Current dependency | Accepted simpler dependency |
| --- | --- |
| `create_session(session, run, review)` in `store/sessions.rs:50` | Conversation admission owns its durable launch metadata and immutable input |
| `flow_turn_selection` joins published Run at `store/sqlite/flows.rs:814` | Flow selects its reserved AgentSession and exact history under its own fence |
| `publish_attempt` updates Run at `store/sqlite/flows.rs:1072` | Publication compares the existing reservation on its final owner |
| `FLOW_SELECT` reads Run publication/fallback outcome at `store/sqlite/flows.rs:32` | Flow boundary reads Session/Flow evidence, keeping command outcome on Exec |
| `SESSION_SELECT` inner-joins current Run at `store/sqlite/sessions.rs:12` | SQL Session summary/detail reads AgentSession and subordinate history |

The public ordinary-retry fixture `review-slice-native-owners` completes with one
Session/thread/generation/Exec, retains failure then success and consumes event 6
once, but leaves one Run row. Its zero-Run assertion remains red. This is an
ownership failure even though the command succeeds.

Complete admission/publication and their readers together, through TaskLauncher,
ordinary launches, saved reviews and Asks. Preserve reservation versus immutable
artifact publication, failure-before-provider admission, exact history selection,
account evidence and uncertain effects. Delete the superseded Run writers/joins
as their facts move; a wrapper or rename cannot satisfy this boundary. Historical
inputs must remain recoverable until import preservation is proved.

### 3. Historical evidence is still discarded by classification

Source inspection retains the prior findings in `ops/session_import.rs`:
`store` returns Unchanged solely on Session-ID existence (282–295); `headless`
drops Flow occurrence membership (315–345); `flow_review` skips nonhuman or
finished captures (428–437); `declared_work` converts resolver errors to absence
(122–135). Ask/review paths mark Runs claimed before successful storage.

The simpler product keeps historical identity independent of launch eligibility
and current planning. Use exact recorded relationships and explicit unknowns;
compare replay content rather than treating identity as proof of equality. Keep
all [import obligations](import-preservation.md), including interrupted conversion,
completed keyed answers and several old work boundaries in one real Exec.
Deleting inputs before those checks would make the model smaller by losing data.

### 4. Desktop still makes current planning a prerequisite for ancestry

`WorkspaceProjection.swift:80` builds Session breadcrumbs by searching current
roadmaps. An unmatched Session returns nil Wave/Task at line 93; `subject(for:)`
likewise returns nil after searching only that projection. The Session remains
reachable, but its known Task disappears from that navigation surface.

The accepted interaction is to retain the bound Task breadcrumb when its Project
leaves current planning. Read typed Session ancestry directly and enrich it with
available Task facts; missing roadmap membership must not imply an unbound
conversation or require a fabricated Project. Preserve pane ID and draft while
changing only the displayed ancestry. This is source evidence, not a rendered
failure. `RegistryQuery.sessions` currently requests `--limit 0` to retain all
conversations; do not replace it with unstable offset pages merely to cap a read.

## Next action and proof

The next complete implementation boundary is agent reservation/publication plus
Session readers on their final owners. Its smallest existing distinguishing
proof is the zero-Run ordinary automatic-retry CLI fixture: failure then success
in one conversation/Exec, exact successful consumption, retained usage and no Run
row. Pair that boundary with failed publication before provider launch and
completed keyed-Ask/review recovery. A zero-row count alone is insufficient.

Native repair must pass the paired late-child and legitimate-retry cases on one
candidate, then preserve missing-decision, ordinary retry and a live shared-engine
sibling. Store-only passes cannot prove the tool-environment transport. Any change
to engine lifecycle or caller transport returns those claims for behavior review;
earlier connect/recovery receipts cannot automatically cover it.

The full [remaining-work contract](remaining-work.md) continues to govern:
final Run deletion and all providers/callers; populated canonical import and bind
history; general Exec admission and measured dense SQL discovery; coordinated
Rust/Swift history and numeric graph IDs with pane retention; Chapter public
second-Home sync, exact Started retention, actual default-Flow launch and final
upgrade; integrated checks and final docs/skills. Publication and stacking retain
their scoped repairs. Cancellation/refused-start stays with the existing LOO-305
disposition; process settlement is separately unproved. Configured provider/Desktop,
backed-up real-Home migration and release acceptance retain their restrictions
and need explicit procedures/proof; fixture results do not discharge them.

## Evidence and attribution

This pass inspected current source, docs, working diffs and the three saved native
receipts. It reuses slice evidence rather than rerunning unchanged behavior.
All 24 source hashes in the slice receipt match the current files; local review
links resolve and the tracked working diff passes whitespace checking. No
behavioral suite or build ran in this review.
The receipt `.lf/tmp/cut-i/review-slice-source.json` owns the bounded production
measurement (+193/−173, net +20 against `70e8db9c6`, excluding tests/docs/generated
files and including the caller draft). That conversion has deleted no final Run
owner. It is distinct from compression's net −17 and from whole-branch reduction.
No new performance, hosted CI or deployed acceptance claim follows.

Supervisor comment `5abd67b5-ca17-4d75-8bdb-bbd285ff43c6` reports upstream main
`bc51da30a49c5db626e2677054f4d550bbbd124e`, four commits beyond base `1dce02734`,
at its observation. Later comment `cb67b4d4-ef13-4728-82c0-0c765b399bd0` reports
CI36517146691 terminal: Rust has 1,304 passes, one failure, 14 skipped and 643
unrun. The migration test still prohibits `projects.status`, now required planning
data. These are relayed observations, not fresh hosted reads by this reviewer.
At the next implementation boundary, remove only that obsolete assertion and run
the migration module without fail-fast on a disposable materialized source copy;
preserve Task-status absence, obsolete Project reason/time checks and populated
Project preservation. At the next owned integration boundary, preserve local work,
coordinate scratch capture, inspect `lf rebase --plan` and integrate through lf.
The terminal evidence is retained in evidence.md; reconcile
conflicts with the smallest affected proof and read back Task base/PR head.
Rebase alone supplies no reason to publish the incomplete native conversion.

Resumption under supervisor comment `e085fbe4-34d2-4a6d-bc5c-a741d2308606`
reuses this completed review. HEAD remains `fb100f486`; all 24 recorded source
hashes still match. Reading the saved Flow event rows as lists confirms old-child
rejection, valid-retry rejection and ordinary success with one retained Run.
The earlier inspection-script AttributeError supplies no product finding.
The pinned installed Session inventory refresh succeeded. No build, behavioral
rerun, new approval or navigation follows from this resumption.

Attribution correction: `be418650-8459-4be9-ba12-055080ae24df` was the supervisor
directing selective fixture publication under Jack's existing authorization;
it was not a new instruction spoken by Jack. Other scratch files remain with
their recorded owners. This review changes only this document, publishes nothing,
and leaves navigation to the saved decision step.
