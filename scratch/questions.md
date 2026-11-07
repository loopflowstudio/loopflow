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
- LOO-717 owns cleanup/lifetime integration. The earlier handoff describes a
  superseded lease implementation; read current 717 evidence before integrating.
- New hands-on proof must use separate work. Earlier rendering was visibly broken,
  not accepted. Retain all demo roots and owners; no input settlement or teardown
  authorization has been received.

LOO-717 merged through PR #1811; its Desktop ownership and shared stream
transport are incorporated. Parent review history remains at
`5f4ee78ee9:scratch/questions.md` and `5f4ee78ee9:scratch/loo-717-shell-lifetimes.md`.
Keep this Task’s walkthroughs; parent scratch filenames are independent reviews.

- `lf open --task` on a Task with no checkout: Jack Heart chose “start the task” (November 7) over keeping Task & workflow. Opening now places an open local Task, which claims it like any checkout, then launches the usual layout and `lf resume`. No Graph starts. Repository and Wave openings still place nothing; done and abandoned Tasks keep Task & workflow.
- Jack Heart's “otherwise lg2m ship it” predates the LOO-717 merge (04:51Z, November 7); its coverage of this tree is unconfirmed. `lf task status` shows Auto-Ship approved at 06:08:38Z, after that merge, by Session “LOO-498 Rex workspace” for Jack Heart. What set it is not recorded where this pass looked, and launches can set it, so it is not read as a decision. Open choice: (a) the approval stands; (b) hold for review, with `lf ship --stop`. Left unchanged.
- Broad `lf open` starts no agents (November 7 demo, [workspace-demo.md](workspace-demo.md); built). Assumed rule, reversible: an explicit Task or named Session resumes anything; a repository or Wave opening resumes only a Session with a running provider, judged for the Session a pane names, not the Task's primary. Every other pane gets Task & workflow. The Wave's own conversation follows the same rule: no running provider, no Conversation tab. Alternative: broad openings never resume, which would not return people to live conversations. Open: whether a broad open should move input from a live conversation in another terminal, which `docs/workspaces.md` documents today.
- A Task with no conversations and no resolvable harness now fails its receipt and creates no tab, rather than a pane that exits. A Task with an earlier conversation skips the check because resume reuses its saved agent. Panes do not receive `lf -a`, so `lf -a claude open` still fails here though the error names `-a`; `lf set --global default_agent` is the working fix. Forwarding `-a` is unbuilt; check first what it does to a Task whose ended Session used another harness (`config.rs` has a `Resume::Import` path, not traced here).
