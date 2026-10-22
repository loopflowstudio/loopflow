# v0.12.27

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.27 keeps long-running Task work moving with fewer repeated conflicts, less replayed context, and account choices that survive Flow recovery. Branch updates now merge upstream changes while preserving authored commits, and agent launches distinguish current direction from progress reports and historical references. Upgrade for more reliable delivery continuity, with command migrations and the removal of the resident Wave service and chat surfaces to account for.

## Update Task branches without replaying their history

Long Task branches could repeatedly reopen conflicts as updates replayed commits. `lf sync` now merges main or the live stack parent into the branch, preserving authored commit IDs and resolved conflicts through synchronization and landing preparation.

- Use `lf sync --manual` for local synchronization; after resolving conflicts, run `lf sync --continue`. Manual synchronization does not push. GitHub still performs the final squash onto main.
- Stacked children retain their edits after a parent lands by squash. New children clear inherited scratch in a dedicated commit, while later parent updates preserve the child's own notes and deletions.
- Interrupted or unowned merges can be resolved without a live foreign operation owner losing protection. Publication and landing recognize externally completed merges and reconcile the Task's recorded base.
- Acknowledged PR identity is saved before follow-up reads or draft promotion. Failed follow-up operations retain the Task association and reviewer copy; Linear linkage remains retryable, and existing PRs can be adopted without prior local publication.

## Keep account status current and choices durable

Managed account status now refreshes identity and usage by default. Per-provider choices belong to the saved invocation, so local Flow work no longer depends on the launching CLI staying alive to retain them.

- `lf auth status` refreshes managed accounts; `--cached` provides offline, read-only inspection. Failed refreshes retain dated evidence, and expired usage displays as unknown.
- Flow and Task invocations save Claude and Codex `--account` or `--only-account` selections for autonomous steps, review children, and resumes. Restart an invocation to change its saved choices. SSH forwarding still requires the origin broker.
- Managed Codex authorization opens through the saved Chrome profile using app-server. Failed or canceled login preserves existing credentials. Claude identity checks bind observations to the verified credentials and reject mismatched or duplicate identities during status and routing.
- Live Codex status shows available banked resets. Only `lf auth redeem-reset codex <email>` spends one, reporting before/after usage and an idempotency key for retrying a lost reply. A failed follow-up refresh preserves the confirmed redemption outcome.

## Give each agent step relevant direction

Long threads and accumulated notes could overwhelm launches or send routine progress back as new instructions. Launch context is now bounded, complete overflow sources remain available locally, and successful steps acknowledge the direction already delivered to them.

- Wave memory is budgeted at 8,000 tokens, scratch notes collectively at 16,000, and launch messages at 16,000. Oversized sources become marked excerpts pointing to complete local snapshots.
- Launch input is capped at 64,000 cl100k tokens and 512 KiB, including structured-reply guidance. Remaining oversized input produces an actionable error before contacting the provider. Provider-native instructions, tools, later reads, and conversation history are outside this limit.
- Successful visits acknowledge delivered direction per structural step and invocation. Failed or interrupted attempts acknowledge nothing; undelivered late comments remain eligible.
- `lf task comment` inside a Run defaults to progress, excluded from steering. Use `--steer` to send direction explicitly. Historical unmarked comments remain eligible because their authorship cannot safely be inferred.
- Submitted references neutralize dollar-prefixed skill mentions while direct requests remain active. Scratch warnings identify session-specific instructions and historical mentions by path and line; warnings are advisory and source files remain unchanged.

## Operate Waves without a resident service

Wave planning and Task delivery no longer carry the resident listener, daemon, or chat stack. Bounded `lf --wave <name> wave/operate` runs, Tasks, chapters, and metrics remain available; this removal does not replace the Task/Flow/chapter/Run model.

- Remove `lfd`, Wave chat/reply/thread commands, webhook reception, and provider-delivery handling.
- Remove app chat views, streaming clients, and resident liveness controls, along with daemon packaging and installation requirements.
- PR landing retains its foreground supervisor. Historical daemon installation receipts remain readable, and this removal changes no database tables or migrations.

## Operational notes

- **Command migration:** replace `lf rebase` with `lf sync`; there is no CLI alias. Saved Flow commands migrate automatically with their arguments and structure preserved. Replace `lf auth status --verify` with ordinary status; use `--cached` where read-only behavior is required.
- **Account storage:** state remains in the existing Home database. A forward migration adds Claude credential binding. Cached status reports reset credits as unknown.
- **Hosted CI:** PRs containing scratch artifacts defer the full matrix and skip `tests-result`. Scratch-free PRs run all checks regardless of draft status. Merge-queue and main runs reject scratch artifacts; a skipped checkpoint is not passing proof, and pre-merge enforcement depends on the required merge queue. Rust tests collect failures with `--no-fail-fast`.
- **Validation limits:** branch and account recovery proofs used local fixtures or fake providers. Hosted acceptance, real browser OAuth, native Session resume, and real reset redemption remain unverified. The saved-reference change reports added regressions but no test or lint run; the failed-search fix has focused passing evidence, but its later full gate was blocked by the disk reserve.

## Small changes

- Resource recovery limits busy uv cache pruning to 15 seconds, retains the busy cache, and continues eligible build-artifact cleanup.
- Failed searches containing quoted denial text no longer falsely block Task handoff in the recorded cases. Blockers require a failed command and a recognized diagnostic line; existing blocked Tasks are not automatically recovered.
- Design and kickoff skills mark planned removals as **Delete — do not maintain**. Implement and compress handle those removals before polishing surviving code, keeping required cutovers, migrations, and behavioral coverage together.