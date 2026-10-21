# Two people, one repo: dividing and delivering work

Read-only review, 2026-10-21. Scenario: Dustin joins Jack in a Loopflow repo
(etude has no `planning:` in `.lf/config.yaml`; loopflow uses Linear). Ordered by
how much it would hurt Dustin.

## 1. Out of the box, Jack and Dustin do not share a plan — needs Jack (default) + clear fix (docs)

- **Problem:** Dustin clones etude, runs `lf task create`; Jack never sees the Task.
- **Docs:** README.md:11 / docs/index.md:11 ("The plan is shared through Linear or your Git
  repository"), website/home.html:49, docs/lf.md:365,383.
- **Accurate?** Partly. Default `PlanningConfig::Git { shared: None }`
  (rust/loopflow/src/config.rs:232-238) publishes to `refs/loopflow/planning/users/<user-key>`:
  one ref per person. Sharing needs `shared: <name>` committed in `.lf/config.yaml`.
  lf.md:383 states this, but only as ref-layout detail.
- **Useful?** No. The front door says the plan is shared, and nothing tells a team to opt in.
- **Fix:** In lf.md, "Choose planning synchronization", lead with: "Working with someone
  else? Commit this to `.lf/config.yaml` so you both read one plan:" plus the
  `planning: {provider: git, shared: team}` block. Scope the README/index opening: "shared
  through Linear or a shared Git ref". Product question for Jack: should a repository's
  default be a shared ref?

## 2. Two operators act on each other's Tasks — needs Jack (product gap)

- **Problem:** Jack and Dustin each run `lf --wave runs wave-operate`. Dustin's pass finds
  Jack's started Task with no live Flow (Jack's Flow runs on Jack's laptop) and runs it.
- **Docs/skills:** wave_operate.md:38-41 ("Give every owned, unfinished Task … exactly one
  disposition"). Owned by anyone, not by you. docs/lf.md:427 ("Workflows, Sessions,
  Processes … stay local").
- **Accurate?** The skill's text matches code; the outcome is the bug.
  - Dustin's `lf task status` shows `execution.state: idle`, "No Task Flow has run", for a
    Task Jack is running elsewhere. Processes are Machine-local.
  - "moving" requires an observed live Flow, which Dustin can never observe for Jack's work.
  - `lf task run` then claims it: `claim_in` appends a claim whenever the writer is local
    (store/sqlite/task_ownership.rs:384-393). The writer for someone else's Task is your own
    Wave Machine (`writer_in`, :209-233).
- **Useful?** Nothing tells an operator to leave a teammate's Tasks alone or report them as
  "someone else's".
- **Fix:** Add a disposition such as "theirs: owned by another person; report owner and
  leave it". Have status report a teammate-owned Task's execution as unknown, not idle.
  Document it in lf.md near the claim table.

## 3. "Take over Jack's Task" works, but its consequences are undocumented — needs Jack (handoff model) + clear fix (docs)

- **Problem:** Jack goes on holiday; Dustin wants to finish LOO-X.
- **Docs:** docs/lf.md:170-187 (claim table, "the later claim wins", steer to earlier
  claimant), :178 ("You cannot hand off … someone else's claim"), README.md:184-191.
- **Accurate?** Yes for ownership. Code matches: no refusal, a later-claim steer to Jack's
  live Sessions (task_ownership.rs:452-482). Unstated effects:
  - **Checkout:** Dustin's `task run` places a fresh checkout on his Machine.
  - **Branch, Linear:** the recorded provider branch is reused
    (ops/task.rs:956-961, 1237-1238), so Dustin checks out Jack's pushed branch, and both
    may push to it.
  - **Branch, Git planning:** with no recorded branch, Dustin gets `dustin/<id>-…`
    (ops/task.rs:1239-1241). That is a second branch and a second PR for one Task, despite
    "A Task has zero or one pull request".
  - **What doesn't travel:** Jack's Workflow position, Flow history, Sessions and
    uncommitted scratch design (lf.md:427).
- **Useful?** It covers the bookkeeping. It doesn't answer "what do I get, and what must
  Jack do first?"
- **Fix:** Add a "Take over a teammate's Task" recipe:
  1. Jack pushes or commits scratch, then runs `lf task unclaim`.
  2. Dustin runs `lf task claim`, then `lf task run` (or `lf task workflow move` to the
     node Jack reached).
  3. List what does not travel.

  Product question for Jack: is a two-branch, two-PR result under Git planning intended?
  Verify the Linear `branchName` behavior per viewer.

## 4. No "who has what" view — clear fix (docs) / product gap (display)

- **Problem:** Dustin wants the Wave's Tasks with their owners.
- **Docs:** docs/conducting.md:29,44 ("`lf wave show` … sorted by what they need").
- **Accurate?** Partly. JSON has `runtime.owner {person, machine}`, but the text table has
  no person column. Its third column is `next_move.owner` ("wave"), which reads as an
  owner.
  - `lf task status` prints "Owner: Jack Heart on Jacks-MacBook-Pro-2", but its log lines
    are a raw epoch and a Machine id (ops/task_ownership.rs:98-119).
  - Pre-ownership claims show `person: null` (seen live in `lf wave show --json`).
- **Useful?** No. The question "who's on it" has no direct answer.
- **Fix:** Show the owner's name in `lf wave show` and format the log timestamps.
  Meanwhile, document `lf wave show --json | jq '…runtime.owner'` and `lf task status` in
  conducting.md "See everything".

## 5. A comment to a teammate only steers with Linear — clear fix (docs) / needs Jack (Git parity)

- **Problem:** Dustin runs `lf comment LOO-X "use the smaller approach"` for Jack's running
  agents.
- **Docs:** README.md and index opening ("a comment on a Linear task reaches its agents"),
  conducting.md:134-135, home.html:49.
- **Accurate?** Partly, already flagged by the Linear docs pass. Git-imported comments are
  stored without a steer (store/sqlite/planning_write.rs); only Linear ingestion creates
  steers (ops/linear_observe.rs). The ownership steer is the exception
  (task_ownership.rs:444).
- **Fix:** The homepage line needs the same scoping, or Git ingestion should create steers.

## 6. Reviewing each other's design or demo — needs Jack

- **Problem:** Jack wants to review Dustin's design before Dustin builds.
- **Docs:** authoring.md:260-261 ("nodes where you take part in the Task conversation"),
  :126 ("no substitute reviewer"), README.md:310 (primary conversation lives on the
  owner's Machine).
- **Accurate?** Yes, and that's the gap. The node's person is whoever is in the owner's
  conversation, on the owner's Machine. There is no path for Jack:
  - he gets no Session, no node notification, and can't make a Workflow move;
  - moves are local (lf.md:427).
- **Useful?** Nothing says how a second person reviews. The working path is GitHub: a draft
  PR that still contains the scratch design until `lf submit` clears it (lf.md:1128-1131).
- **Fix (docs, now):** "Reviewing a teammate's design": publish a draft PR at the design
  node (`lf pr publish`, `lf pr request-review -r jack`); Jack comments on GitHub or uses
  `lf comment` (steers with Linear only, #5); the owner moves the Workflow.
- **Product question:** should a node name its reviewer?

## 7. When the other person's PR lands first — clear fix

- **Problem:** Jack's PR merges; Dustin's branch is now behind or conflicts, and Dustin's
  copy of Jack's Task looks stale.
- **Docs:** lf.md:575-600 (PR commands, review-request semantics: accurate and good),
  :1139-1142 (`lf sync` at a "deliberate maintenance boundary"), :427-428 ("A completed plan
  does not move another machine's Workflow or clean its checkout").
- **Accurate?** Yes.
- **Useful?** Framed as maintenance, not "my teammate just merged".
- **Fix:** One recipe in lf.md delivery:
  - `lf sync --plan` then `lf sync` after a teammate merges, with `lf sync --continue` on
    conflicts;
  - `lf land --wait-and-fix` if the branch must be up to date to merge;
  - `lf task status` shows a teammate's Task done; its local checkout, if any, is yours to
    clean.

## 8. Branch names are author-scoped; nobody says so — clear fix

- **Problem:** "Will my branches collide with Jack's?"
- **Code:** with no recorded provider branch, it is `<git user>/<id>-<title>`
  (ops/task.rs:1239-1241); a name clash fails with "branch … already exists; choose another
  workspace name with --name" (:1246-1250).
- **Docs:** only README.md:52, about bare `lf`.
- **Fix:** One sentence in lf.md near `lf task run EXP-12 --name fix` (:194), with the Linear
  exception from #3.

## 9. Internal cutover status in the planning section — clear fix

- **Docs:** lf.md:401-404 ("This build implements the enrollment baseline below. LOO-412
  replaces it … Manual per-Wave enrollment is superseded direction").
- **Useful?** No. Dustin can't act on a Task ID or "superseded direction".
- **Fix:** Delete it, or move it to architecture-reference.md.

Fine as is:
- Cleanup in repo_operate is scoped to your own PRs and branches (repo_operate.md:14-20).
- PR review semantics, including stale approvals, are documented (lf.md:591-600).
