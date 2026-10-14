# Session entry API simplification

Scope update from Jack Heart, 2026-10-14, during the running pursue Flow. Accepted behavior; implemented in this checkout.

## Implementation status — 2026-10-14

Implemented in this checkout. `SessionTarget` selects contextual, Session, Task
or Wave entry; Task discovery shares primary ranking and main-checkout discovery
ranks interactive conversations across repository worktrees. Explicit selectors
normalize before Machine routing, bypass ambient Work binding, and connect uses
the selected checkout's current launch policy. Prepared first opens now also use
`agent::resolve_session_config`, removing their stale saved-model override.

`resume` has no ID; `connect` accepts ID/Task/Wave. `--prepare` controls provider
launch independently of JSON. `session new` replaces a primary conversation;
`connect --takeover` preserves identity. Flow corrections use explicit connect
with `--message`. Desktop prepares Task targets directly and consumes that
preparation without a second ensure/connect sequence. CLI callers, DTO mirrors,
documentation and existing launch/lifecycle fixtures use the new spellings.

Implementation, compression, upstream conflict recovery and reconciliation are
complete. Queue gate passed after the context fallback and fixture repairs
recorded in claude-permissions.md; Jack Heart authorized landing with waiting. See delivery-boundary.md. Remote preparation shares
its SSH command across text and JSON for connect and resume. Caller-mode reduction
and Wave worktree delivery policy remain unresolved and outside this accepted scope.

## Accepted direction

Jack asked to simplify the external Session APIs as well as their provider-launch implementation. Jack clarified: “to be clear i do like resume , but i dont tink it should take an ID”.

Keep contextual `lf session resume`, without a Session ID argument. `connect` accepts an explicit Session ID or Work target. Both resolve to the same connection operation. The earlier conversational suggestion to delete resume entirely is superseded by this clarification.

Jack further specified that resume “can take a task or a wave to override its worktree-based finding” and should “take the current worktree (or else the expliictly passed work object) and find the relevant session”.

Selection contract, including Jack's subsequent corrections (later statements supersede earlier main-checkout behavior):

1. Explicit `--task` or `--wave` overrides ambient worktree discovery: `lf --task LOO-123 resume` / `lf --wave infrastructure resume`. Use that Work object's relevant conversation and existing primary/selection rules.
2. A worktree with an active Task privileges the Task's Session over unrelated, merely newer conversations in the same checkout.
3. From the main checkout, `lf resume` selects the most recent interactive Session across **all worktrees of this repository**. It never defaults to the repository primary Session just because the caller is in main. Main means the repository's main checkout, not merely a branch named main.
4. From a linked worktree without an active Task, `lf resume` selects its most recent interactive Session. Preserve native last-human-input recency with last-opened fallback; background assistant activity must not win.
5. For resume, no eligible conversation means a clear not-found result, not automatic conversation creation.

`lf connect ID` bypasses contextual discovery and connects to the named Session. `lf resume` is the existing preferred shorthand for `lf session resume`; preserve that navigation behavior.

Jack explicitly requested “lf [session] connect --wave infra should work” after asking how to launch the Wave primary and enter its worktree in one command. Accepted direct entry:

```sh
lf connect --wave infra
lf session connect --wave infra
```

Both find or prepare that Wave's primary Session and connect in its worktree. No shell-level ensure/JSON extraction/connect choreography is required. Implement the same explicit Work-target shape for Task selection: `lf connect --task ISSUE`. The selector union is Session ID, Task, or Wave; reject conflicting explicit targets. Keep existing root-position Work selectors usable, and test the requested after-command spelling as well as the shorthand and canonical forms.

Work-target connect reuses the existing ensure/primary lifecycle when no primary exists. Resume remains discovery of an existing relevant Session. `--prepare` may prepare the selected primary and return connection data, but never starts the provider; `--json` changes representation only. Opening runs the provider in the resolved checkout; changing the parent shell's cwd is a separate concern.

Use one target-resolution API for worktree and explicit Work selection, then one Session connection API for the resolved Session. Keep Task/Wave identity and checkout resolution in existing Work owners. No Session ID positional argument on resume, and no extra Task/Wave identity inferred from the invoking Session after an explicit target is supplied.

Acceptance adds main-checkout repository-wide recency, active-Task preference, unowned linked-worktree local recency, no repository-primary default, explicit Task and Wave overriding an unrelated cwd, shared current-settings policy at the selected Session's checkout, and preservation of the selected conversation's identity/account. Include stale/missing native history without silently creating another conversation.

Remove the ID-taking resume form and migrate internal callers, documentation and tests together. Preserve contextual selection, explicit connection, native history and headless continuation. Do not add an alias or compatibility parser that preserves the duplicate ID-taking form. Update the existing launch contract suite to prove both selection forms reach the same policy and provider boundary.

## Additional accepted simplifications

Jack accepted all three suggestions with “yes, great to all of the above”:

- Task/Desktop callers consume the same target resolver. Eliminate duplicated selection/ranking and caller choreography while preserving explicit creation/preparation effects where required.
- `--json` changes representation only. Explicit `--prepare` returns a connection command without starting the provider. Text and JSON share this behavior; Desktop now requests explicit preparation. Existing explain/read behavior stays passive.
- Creating a replacement conversation and taking over the same conversation have distinct names. Implemented spellings are `session new` for the former and `connect --takeover` for the latter. History, primary reassignment semantics and process-control authority remain intact; callers/docs/tests use these names and the ambiguous old spellings have no compatibility aliases.

Add contract tests for identical effects across output formats, explicit preparation without provider launch, shared CLI/Desktop selection, new-conversation identity changes and takeover identity preservation. For `connect --wave`, prove first preparation, repeat reuse without duplicate Session/worktree, provider cwd, current settings, explicit target over unrelated caller cwd, and parity between `lf connect` and `lf session connect`.

## Remaining discussion

- Reduce `Refuse`/`Try`/`Replace` caller choices where provider state already determines a sensible action. Preserve explicit authority for stopping another owned client; removing duplicate APIs must not silently broaden process control.
- Headless syntax uses the reversible implementation choice `--message TEXT` on contextual resume and explicit connect (recorded in questions.md). No positional resume ID remains.
- Wave/repository primary worktree delivery remains open. Jack clarified the pain: “wave worktrees dont have a clear lifecycle” and edits to Wave memory leave it “unclear when that should get landed”. Existing persistent-worktree mechanics retain checkouts after delivery and restart a merged branch on the next commit/publication; the unresolved policy is when a completed batch of Wave edits should be published and landed. Direct Work-target connect preserves current worktree placement; no new automatic delivery policy or deletion of existing worktrees is authorized here.

Keep live attachment, stopped-session process launch, fresh conversation creation and historical replay semantically distinct. Share target selection and current launch-policy resolution, not identity-changing effects.

## Delete — do not maintain

Completed in this diff:

- `SessionCommand::Resume.id`, checkout-only `select_connection`, and ID-taking resume callers: replace with shared contextual/Session/Task/Wave target resolution; preserve native recency, identity and account.
- JSON-selected launch effects in `session::open` and SSH dispatch: explicit `--prepare` owns preparation in either output format.
- `SessionCommand::Replace`, `connect --replace`, `OpenMode::Replace`: rename to `new` and `--takeover`, preserving primary succession and attachment authority. Keep `--try` pending the separate open-mode decision.
- Desktop `ensureTaskSession` plus connection choreography: use Work-target preparation and consume its prepared command directly.

Compression keeps `SessionConnectionArgs` as the shared resume/connect option
shape: selection validation, previews and execution consume it directly instead
of unpacking and repacking five independent flags. Removed the unreachable
Session branch in laptop credential preparation; SSH Session operations already
use destination-owned accounts. Repaired the Workflow provider-mismatch fixture’s
stray `--message` and Desktop’s always-true prefix assertions. No remaining
entry-migration deletion targets; caller-mode reduction remains undecided above.
