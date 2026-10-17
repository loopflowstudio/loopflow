# LOO-415 native transport demo — October 17, 2026

Jack Heart invoked demo for [PR #1581](https://github.com/loopflowstudio/loopflow/pull/1581),
then reported “worked” after reconnecting the unanswered native Claude question.
Jack subsequently requested `lf ship -b!`, explicitly authorizing the ship Flow
and merge. This supersedes the earlier no-merge boundary; it does not assert that
unperformed acceptance checks passed.

## Observed

Built commit `2523534b29` with `cargo build -p loopflow --bin lf` (passed).
The live-store launch failed before Claude started: this build requires the Git
repository locator and the 0.14.4 schema frontier while the selected shared store
was on 0.14.3. No shared-store migration was performed. The attempted same-identity
bind through the older selected runtime did not create the newer locator.
Later, the installed runtime was observed at 0.14.4. Running its same-identity
bind with the conversation's old `LF_BIN` pin removed created the missing locator;
Task reads then worked. This conversation did not run an installation or upgrade.

Continued with a disposable experimental `LF_HOME`, the branch binary, and the
real installed Claude Code 2.1.302 / Claude Max account. No fake provider/API.
This establishes local native transport behavior, not installed-store integration.

- First question (Orchid/Quartz) was answered before closing the tab; Jack asked
  to try again. Reattachment recovered the same process, but this first round
  does not prove preservation of unanswered interaction.
- Sent one explicitly requested fresh question (Cedar/Amber) in the same native
  Session. Jack closed its cmux tab with the question unanswered.
- Confirmed the demo viewer was gone and Claude PID `60408` remained alive.
- Opened a fresh terminal with public `lf session attach` using the demo's
  binary/home. The original question and transcript returned. Jack said “worked”.
- Jack selected Amber in that restored question. Captured screen shows the answer
  and exact response `DEMO_SECOND:Amber`.
- AgentProcess `e6e554e8-7472-42fd-a726-1f2eebfcd9d7`, PID `60408`, and Session
  `session_01c8cb2697074ced881121b809bfd97a` stayed unchanged. No automatic prompt
  resubmission or replacement provider was observed. Native request IDs were not
  independently extracted, so the screen and unchanged process are the proof.

Evidence: [launch helper](evidence/transport-demo/launch.py),
[before](evidence/transport-demo/provider-before.json),
[after](evidence/transport-demo/provider-after.json),
[restored question](evidence/transport-demo/reattached-screen.txt),
[completed answer](evidence/transport-demo/final-screen.txt).

## Remaining and next action

Run the authorized ship edge through gate and landing. Preserve the outstanding
SSH/outage, nested launch, load, migration-frontier, exact terminal-mode restoration
and Flow OSC acceptance in [the design](loo-415-transport-handoff.md). This demo
does not waive those checks. The native demo terminal remains open and idle;
its closure is not Task approval. Jack's explicit ship instruction supplies the
delivery authorization.
