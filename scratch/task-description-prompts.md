# Task descriptions and update history

Finish: Task authors produce a readable current brief, with dated operational
updates in comments. Comments are collapsed by default in the intended Task UI;
current blockers, true dependencies, accepted scope and contrary evidence remain
understandable without opening them. No live Linear descriptions or comments are
changed by this prompt pass.

Observed: the captured LOO-285 description contains the exact chapter allocation
paragraph. Current Rust source contains neither that heading nor its no-launch
sentence; this does not identify the historical author. Existing filing skills
already ask for user-centered briefs but leave the destination for updates vague.
Chapter skills prohibit prepending application history but did not explicitly
cover appended amendments or Task comments.

Scope: amend the five full Task-brief authors and direct design/terminal filing
paths, the prompt-authoring audit, and the human prompt guide. Keep skills
self-contained. No new universal injected policy, shared template loader, PM
mutation, comment transport or UI implementation. Record the collapsed-comments
presentation direction for native work; do not fabricate a comment thread from
text still stored in Linear's description.

Review against LOO-285: benefit and retry/publication outcomes belong in the
brief. Preserve the existing no-duplication, delayed/overlapping opportunities,
failed-verification evidence and two-settlement requirement; define unexplained
terms from the design rather than silently deleting them. Chapter ID and review
scheduling belong in the chapter receipt, with a readable Task update in comments.
The queue entry alone does not establish a technical dependency on LOO-279.

## Review cases

| Case | Description | Comment / linked record |
| --- | --- | --- |
| New Task | Problem, beneficiary, concrete improvement, acceptance and binding constraints | No invented history or automatic comment on filing |
| Chapter allocation | Unchanged brief; a genuine blocking dependency remains explicit | Readable allocation/queue update; machine IDs and review timestamps in chapter receipt |
| Failed verification | Current failure and its consequence summarized when relevant | Dated result with link to full evidence; never imply success |
| Accepted scope change | Reconciled current brief, exact acceptance retained | Decision and previous reasoning remain history |
| Routine poll | No description change | No repeated no-op comment |
| Proposed plan | Proposed brief and draft updates only | No authority to publish comments from a proposal |

Review changed the candidate: explicitly separated queue ordering from dependency,
kept binding failures visible when comments are collapsed, and covered appending
as well as prepending history. Direct design/terminal filing, split-wave and
implementation follow-ups receive concise guidance because they can author Tasks
without loading the full Wave brief instructions. Builtins remain self-contained;
no new injected global policy or template mechanism.

Validation is contract compilation plus this manual scenario review. The example
is a writing illustration, not an approved rewrite of LOO-285 or proof that every
model-generated description will improve. Existing descriptions need a separate
source-edit pass; comments need a real read/render path before the UI can show them.

Final focused verification: `cargo test -p loopflow --lib engine::builtins::tests`
passes all 14 tests after the last builtin edit. Log:
`/tmp/loo291-task-prompts-final-tests.log`; reviewed source hashes:
`/tmp/loo291-task-prompts-hashes.json`. Changed Markdown whitespace checks pass.
The existing golden cases select other skills and ambient guidance is unchanged;
no golden regeneration or broad suite is claimed. No code, live planning,
installed skill copy, comment posting, commit or publication changed in this pass.
