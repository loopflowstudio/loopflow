# Live loopflow walkthrough

## Writing pass 1

Run ID: `run_2285e52a8e11406ba08f269a0e82c59a`.
Carried direction: none supplied for this initial writing pass.

Advance follows the Flow's forward edge to the next declared step, or finishes
the Flow when no steps remain. At an autonomous decision boundary, loop-decide
records Advance with supporting evidence through the typed Flow protocol. At a
human boundary, the human must explicitly authorize that exact gate; readiness
or provider exit does not approve it. Advancing one boundary leaves later human
gates intact and grants no delivery or merge authority.

This first pass deliberately covers only Advance, as required by the transport
fixture. The next writing pass must explain Iterate and Blocked and record its
distinct Run ID and the direction carried back to it.

The human's observation of the initial approval-to-worker handoff has not been
received through Ask. Leave that evidence pending for loop-decide to obtain
after the second writing pass. This artifact does not establish demo acceptance.

## Writing pass 2

Run ID: `run_08a1e57ad88e47b9b163aa4e700f88ab` (read from `LF_RUN_ID`).
Carried direction received:

> The first writing pass made the required staged progress: scratch/live-loopflow/walkthrough.md explains Advance and records Run run_2285e52a8e11406ba08f269a0e82c59a. In a second fresh writing Run, preserve that first-pass evidence, add concise explanations of Iterate as the declared backward edge carrying direction and Blocked as a request for human evidence or judgment through Ask, and record your distinct actual Run ID and this carried direction. Keep edits inside scratch/live-loopflow. Proof: the walkthrough contains all three concepts, two distinct writing Run IDs, and the direction received by the second. Leave the actual human approval-to-worker handoff observation pending for loop-decide to obtain through lf flow blocked after this writing pass; do not invent it or claim demo acceptance.

Iterate follows the current occurrence's declared backward edge to its earlier
target. Its summary carries direction into the next pass, so subsequent work
addresses the remaining gap. Here, that direction brought a fresh writing Run
back to add Iterate and Blocked while preserving the first pass's Advance
explanation. Pass counts describe history; they impose no pass limit.

Blocked requests missing evidence or human judgment through Ask. The decision
agent calls `lf flow blocked` with the specific question; the keyed Ask runs
unblock and waits for human Complete. Completion returns the human's summary
to the decision agent for reassessment. It neither chooses Advance/Iterate nor
approves a separate human Flow gate. Session readiness alone does not release
the waiting caller.

The writing portion now covers all three concepts and records two distinct
writing Runs with the second Run's carried direction. The actual human
approval-to-worker handoff observation remains pending. loop-decide must obtain
it through `lf flow blocked`: did the Ghostty approval handoff make the next
action clear, and what felt confusing, if anything? Another writing iteration
cannot supply that evidence. No demo acceptance is claimed; the final exact
human gate also remains required.

## Human observation in Ask

Jack's actual response, 2026-09-25:

> The session is open? Im a little confused on what were demoing but i dont think anything about the handoff itself was necessarily any worse

This supplies the previously missing observation. Jack expressed uncertainty
about the session being open and the purpose of the demo; he did not report
that the handoff itself was worse. This is not an affirmative finding that the
next action was clear or that the demo passed.

The Ask response clarified the exercise: initial approval starts work, Iterate
carries direction into a second pass, and Ask returns human feedback to the
waiting decision agent. Human Complete releases this Ask; the final Flow gate
remains a separate approval.

Human Complete returned this summary through the waiting `lf flow blocked`
command, which exited successfully in decision Run
`run_5586692baf53486e96c6176d2f3f7d5d`. The Ask identity was
`ask_once_4aa034f190bc7fe22cd1764831999a5f52597de5f3c08d11c052913baa76f95b`.
Readiness alone had left that command waiting. This records the actual return
of feedback to the same decision Run, not an inference from the ready artifact.

Reassessment: the writing obligations and the required human observation are
now present. The reported confusion concerns the exercise's purpose; this
feedback does not establish a runtime defect. The Ask explanation addressed
that question, but Jack's understanding afterward remains unconfirmed. Advance
can reach the declared final human demo gate to resolve that concern and request
explicit confirmation. It cannot count this feedback as a successful clarity
assessment or approve that gate.

For the final review: this staged writing exercise demonstrates real fresh Runs,
direction carried backward by Iterate, and human feedback returned through Ask
Complete to the waiting decision Run. The final human gate is the next action.
Its reviewer should explain those observations, surface Jack's reported
confusion, and obtain his explicit confirmation before final Advance. The tool
shell required an explicit development executable and saved binding; see
decision-evidence.md for that transport limitation. No whole-branch acceptance
is implied.
