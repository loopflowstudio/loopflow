# LOO-498 — remaining review

Current accepted direction: [rex-workspace.md](rex-workspace.md). Jack Heart
requested ordinary native tab creation plus `lf resume`, and removal of the custom
viewer. Never demonstrate by reopening his active Task or touching its panes.
Prior judgments: `21bbb00329:scratch/questions.md`; passive viewing and its
readiness protocol are superseded.

- Background arrivals show Task/workflow in the conversation slot, preserving
  input ownership; explicit opening runs resume. This is a reversible implementation
  choice, not a request for another review ceremony.
- Native cmux interaction and two-Machine behavior retain only fixture proof.
- New hands-on proof must use separate work. Earlier rendering was visibly broken,
  not accepted. Retain all demo roots and owners; no input settlement or teardown
  authorization has been received.

LOO-717 merged through PR #1811; its Desktop ownership and shared stream
transport are incorporated. Parent review history remains at
`5f4ee78ee9:scratch/questions.md` and `5f4ee78ee9:scratch/loo-717-shell-lifetimes.md`.
Keep this Task’s walkthroughs; parent scratch filenames are independent reviews.

- `lf open --task` on a Task with no checkout: Jack Heart chose “start the task” (November 7) over keeping Task & workflow. Opening now places an open local Task, which claims it like any checkout, then launches the usual layout and `lf resume`. No Graph starts. Repository and Wave openings still place nothing; done and abandoned Tasks keep Task & workflow.
- Jack Heart's “otherwise lg2m ship it” predates the LOO-717 merge (04:51Z, November 7); its coverage of this tree is unconfirmed. `lf task status` showed Auto-Ship approved at 06:08:38Z and shows 12:44:43Z on the later reading (November 7), both after that merge, by Session “LOO-498 Rex workspace” for Jack Heart. What set it is not recorded where this pass looked, and launches can set it, so it is not read as a decision. Open choice: (a) the approval stands; (b) hold for review, with `lf ship --stop`. Left unchanged.
- Broad `lf open` starts no agents (November 7 demo, [workspace-demo.md](workspace-demo.md); built). Assumed rule, reversible: an explicit Task or named Session resumes anything; a repository or Wave opening resumes only a Session with a running provider, judged for the Session a pane names, not the Task's primary. Every other pane gets Task & workflow. The Wave's own conversation follows the same rule: no running provider, no Conversation tab. Alternative: broad openings never resume, which would not return people to live conversations. Open, restated as question 1 below.
- A Task with no conversations and no resolvable harness now fails its receipt and creates no tab, rather than a pane that exits. A Task with an earlier conversation skips the check because resume reuses its saved agent. A pane starting a Task's first conversation receives `--agent` from `lf -a`; an ended Session keeps its harness. The `Resume::Import` path stays untraced because this rule never reaches it.

Open product choices after the November 7 concept review:

1. Should a repository or Wave opening move a conversation that is live in another terminal or Desktop? Today it does, and `docs/workspaces.md` says so; running broad `lf open` twice moves every live conversation into the second session and leaves Task & workflow and shell in the first. The Task text pulls both ways: “return people to the right Loopflow Task Sessions” and “without stealing active input”. (a) Keep it: a broad open is a deliberate command. (b) Broad openings show Task & workflow for every Task and `lf open --task` picks one up; `Resume::Running` goes. Built as (a), reversible. The built directions in rex-workspace.md do not depend on this answer.
2. Auto-Ship coverage of this tree: the (a)/(b) choice above, unchanged.
3. Should a Rex tab showing Task & workflow, with no Conversation pane, keep a completed Task's worktree? Today it does not. Jack Heart's November 3 note read “using Task windows counts as open”; the approved contract reads “retained Rex Task Session terminals prevent cleanup regardless of focus”, and that approval predates Task & workflow taking the Conversation slot, so it does not settle this. (a) No: only a conversation holds, and the docs say so. (b) Yes: Task & workflow takes a hold, which needs a viewer record for `lf task view`. Built as (a), reversible, and `docs/workspaces.md` describes it.
