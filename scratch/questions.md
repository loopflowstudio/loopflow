# Remaining product choices and planning assumptions

Kickoff resolved routine mechanisms in `growth-thoughts.md`. These points must
remain visible without reopening Jack Heart's established direction.

- **TaskSession reset:** Jack proposed a main TaskSession for each launched Task
  and clarified Flow switching now or at the next inner loop point. Whether Ctrl-C
  replaces that primary TaskSession, as with repo/Wave Sessions, is not yet chosen.
  Conversation reset must preserve the Flow and any pending switch.
- **Loops without a unique inner boundary:** current Flow definitions permit
  crossing repeat intervals. Kickoff recommends naming their concrete endpoints
  and requiring a selection before accepting a deferred switch; keep execution
  unchanged meanwhile. A decider already claimed has passed the promised
  pre-launch boundary. This is proposed edge-case behavior for Unit 2 review,
  not an accepted reinterpretation of Jack's “finish” instruction. Unit 1 does
  not depend on this choice.
- **Desktop absent:** the plan eagerly starts primary Sessions while desktop is
  present and preserves ordinary Session lifetimes on close. It adds no background
  wake service. If Jack wants fresh primary turns automatically triggered while
  desktop is closed, that is additional runtime scope.
- **Chapter persistence:** Default Flow edits the current Project recommendation
  through Wave settings. Carrying that preference across chapter rotation is an
  unaccepted extension; keep the existing chapter ownership.
- **Primary wake mechanism:** upstream `3dc89bc9a` removed the resident, listener,
  external chat bridge, and turn claims. The outbox remains without a production
  dispatcher. Unit 2 must prove native turn delivery and implement durable
  primary-owned claims/recovery; this is no longer a resident-consumer migration.
  Restoring a service or replacing native terminal input requires design review.
  External chat integration remains outside the accepted scope.
- **Chapter alignment at future launch:** Product fits the workspace/user-contract
  objective. Its current shared read reports no chapter and unavailable Tasks;
  current KRs and existing Task overlap cannot be verified. Product memory warns
  this may reflect selected Home rather than absent planning. Jack selected local
  kickoff, so this is a future placement question, not a blocker to this plan.

Selected reversible design choices: a compact Session/shell strip beside retained
panes; existing multiplexer zoom plus separate collapse state; per-Task layout and
file retention; Rust-resolved worktree association; direct Ask completion; current
Flow capture semantics; and a fresh primary Session on explicit Ctrl-C only.

Replacement context uses current repository/Wave guidance and work references,
with previous history accessible on demand. It does not silently replay the prior
conversation. Direct Flow/Ask Sessions and ordinary shells keep their existing
Ctrl-C behavior.

Wave/repo roles are decided: Wave combines autonomous operation and design-to-Task
capture; repo owns onboarding, general help, and last-resort diagnosis. Scope-aware
prompts must work without a resident. Jack accepted automatic Wave turns for
operational blockers, including when Jack works elsewhere. Provider turn delivery,
coalescing, and ownership against explicit operating Runs are required Unit 2 work.
Jack also accepted reading existing Task/Session output, including Sessions
associated by Task worktree, and one narrow push capability: any repo-associated
Session can request repo attention with a source reference and reason. This is
required Unit 2 scope. General messaging, duplicated conversations, and a new
caller-wait protocol are outside the minimal design.
The navigation indication for direct participation is a review recommendation,
with boundary availability kept distinct from readiness to Complete.

TaskSession now supplies the ongoing Task conversation while interactive substeps
keep their own windows. The recommended mechanism operates the existing Flow
runtime, with one cursor and worker authority. Jack's “finish, then switch” / “switch
now” clarification replaces the earlier interpretation of editing remaining steps
in place: both start a new invocation and preserve the TaskSession and existing
work. “Finish” now has an exact meaning: reach the next loop point of the innermost
active loop, then exit the old Flow instead of running its decider. The queued
switch is durable and names that exact occurrence; it does not wait for an outer
loop or the entire Flow to complete.
