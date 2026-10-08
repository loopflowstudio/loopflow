# Terminal-host adoption trial · 2026-10-07

**Interactive Flows now preserve the native agent UI.** Jack Heart exercised the
fixed binary in cmux and herdr. Separate account-free fixtures cover CLI transport
and planning boundaries. His earlier cmux Task Flow published
[PR #1496](https://github.com/loopflowstudio/loopflow/pull/1496); the original
large-prompt Claude and duplicate-flag Codex failures remain with LOO-428/429.

| Path | Plain Ghostty control¹ | herdr 0.9.3 — fixture and Jack's native trial | cmux — observed by Jack |
|---|---|---|---|
| One skill | PTY fixture success | Fixture success; input/return | Unknown |
| Interactive conversation | PTY fixture success | Fixture success; input/return after resize | Small taskless conversation reported working in concurrent review²; original Task-bound launches failed |
| Two-skill taskless Flow | Fixture success, ordered output | Fixed binary reached native step two; Jack confirmed shell return | Fixed binary reached native Claude step two²; final exit unverified |
| Task checkout | Fresh blocked: no Linear team; seeded placement succeeds | Same boundaries; workspace opens lf's checkout | Unknown |
| Task Flow | Fixture blocked: managed account required | Same; three failed first-step attempts | Six steps succeeded; no host status, noisy output |
| PR publish | Default branch refused; checkout blocked at publisher substitute | Same; no GitHub publication attempted | Succeeded inside Flow; standalone unknown |

¹ The standalone Ghostty control remains PTY-only. Jack later used Ghostty to
display herdr; that is a different path.
² The concurrent [keyboard-review notes](terminal-host-trial/keyboard.md)
record Jack's small Claude conversation succeeding, screenshot evidence of question
attention and post-answer clearing, and his preference to prioritize interactive
Sessions over headless Flows. They preserve the failed Flow launch and later fixed-binary demonstrations.
These are manual review receipts, separate from automated fixtures. The different prompt/path does not resolve either
original Task launch failure. The review owns preserving its active screenshots.
Jack subsequently reported that both `lf -m claude run ux-flow` and its `-i`
variant streamed steps instead of opening the native conversation UI. Before
the review Session's repair, checkout [`Cli::step_args`](../../rust/loopflow/src/lf/mod.rs) added
`--batch`, which the [Flow skill launcher](../../rust/loopflow/src/lf/commands/flow.rs)
retained. This identifies a Loopflow mode-forwarding bug; it does not identify the
installed binary's revision or implicate cmux rendering. Jack subsequently
required an interactive Flow fix in this branch and a fixed-binary cmux demo
(October 7, LOO-421 comment `fa289cba-babc-4afa-a70a-b7c2c0a5ab6b`). The ongoing
branch now forwards the caller's mode into Flow steps. All 28 Flow tests pass,
including a PTY regression for input, step transitions and interruption; the
fixed-binary cmux screenshot now shows native Claude on the second step with
the step's title and question visible. Final exit remains pending Jack's check.
Headless propagation stays lower priority. The earlier findings-only boundary
is superseded for this repair, without publication or landing authorization.

Evidence and outcome remain separate in the [receipts and repeat recipe](terminal-host-trial/README.md).
The runner used copied **lf 0.13.9** (revision unknown), not a checkout build.
LOO-428 also records Jack's lf as 0.13.9; his exact host/shim versions and timings
remain unknown. A source fix cannot rebut his installed-release observations.

**What herdr showed.** Snapshots labeled the synthetic process `claude` while its
state was `unknown` during input and return; this does not establish real-agent
recognition or Waiting. Workspace labels were `repo` and `repo.trial-task`, with
no terminal title in the snapshot. Foreground cwd followed lf's conversation
checkout, then returned; opening the lf-created Task checkout preserved that path.
The TUI accepted resize and pane reads retained both Flow steps across scrollback.
Color/visual quality, attention-list behavior, notifications and branch display
remain unverified. No host worktree operation or status injection was used.

**Later native herdr trial.** Jack manually started the fixed-binary Flow in
herdr inside a separate Ghostty window. Its API recognized Claude and reported
working on the second native step, with the correct cwd and Claude Code title.
Jack reported sound when the agent responds and judged herdr to be working as
designed; the precise sound source is unconfirmed. Jack subsequently confirmed
final shell return and the background structured-question scenario: attention
while another herdr workspace is selected, then clearing after answering. These
are participant-reported passes; exact labels and timing were not captured.
Jack also reported the resize/scrollback scenario working, with the unsent draft
preserved and submitted once. Native resume remains untested.
Jack raised a separate Flow UX gap: a deliberate Continue action rather than
using Ctrl-C to advance. Its design is unresolved. Details and receipt paths
are in the keyboard-review notes.

**Time.** The first build failed after 26 s because Zig 0.15.2 was incompatible;
the Zig 0.16.0 attempt timed out at 600 s. Reusing its cache completed the build
in another **148.5 s**. Open dependency connections and cache growth supported
continuing; the timeout did not establish a deadlock. In the retained shared run,
first fixture output took **0.90 s from command launch in PTY / 1.13 s in herdr**.
These single samples include a deliberate 350 ms fixture delay. Install-to-useful
real-agent time is unknown; build/download and fixture preparation are separate.

**Three stalls per host.** For the PTY control and herdr: (1) fresh Tasks need
Linear configuration; (2) placed Tasks need a managed account; (3) that unchanged
account failure retries three times. These are shared lf boundaries, not three
host defects. No additional Ghostty GUI defect is established. The supported
no-Linear lifecycle already belongs to [LOO-406](https://linear.app/loopflow/issue/LOO-406).
The prerequisite explanation and repeated account error are filed in the existing
polish Task, [LOO-423](https://linear.app/loopflow/issue/LOO-423).

For cmux, provisionally ranked:

1. **Interactive startup:** Codex rejected duplicate
   `--dangerously-bypass-hook-trust`; Claude's 36,090-token assembled prompt hit
   the shim's 122,880-byte argument limit. Token count does not establish bytes.
   [LOO-428](https://linear.app/loopflow/issue/LOO-428) already owns both exact
   failing launches, with cmux wrappers retained on PATH.
2. **No headless status:** `cmux list-status` stayed empty through
   `lf -b task run LOO-425 pursue`; branch and directory appeared. Notifications
   are unknown. [LOO-422](https://linear.app/loopflow/issue/LOO-422) owns status:
   working/input-needed/exited and attention from another pane remain acceptance.
3. **Unreadable output:** whole prompts/schema in INFO logs, token tables and
   repeated Started warnings obscured progress. LOO-428 already owns this repair;
   LOO-423 retains other polish. The fixture reproduces tables/INFO, not the large
   prompt wall. Default output must expose progress and failures at ordinary width.

**Title/status sources.** The new isolated herdr comparison emitted two distinct
OSC 2 titles from identical original scripts named `trial-agent` and `claude`, then
through lf. Pane `terminal_title` followed both signals; workspace/tab labels stayed
`repo`/`1`. Changing the executable name to `claude` triggered recognition directly
and through lf; state stayed `unknown` during input/return and reached `idle` on
exit. Thus process naming contributes recognition and terminal output supplies the
pane title independently of workspace naming. No real-provider state parser or
rendered label precedence is proved. [Captures and derivation evidence](terminal-host-trial/README.md#title-and-status-derivation)
retain the direct/wrapped comparison, cmux behavioral reports and their limits.

Jack's native `claude --resume` showed *Loopflow operating guide* while Running,
branch, directory and hooks worked; lf workspaces showed directory or command.
Three steers repeat one observation; first-content naming remains his hypothesis.
His explicit title assignment to **LOO-422** survives LOO-423's broader wording.
The [cmux behavioral report](https://github.com/manaflow-ai/cmux/issues/3749)
demonstrates a hook-fed status path; a separate
[rename trial](https://github.com/manaflow-ai/cmux/issues/10217) shows provider names
reaching tabs. Neither establishes Jack's version or the source/precedence of his
Claude title. That exact attribution requires his keyboard comparison; live cmux,
its settings and provider history remain outside this trial's access. Desktop's
OSC 7501 reader is not lf emission or cmux acceptance.

**Follow-ups reconciled.** Supported Product/Infrastructure planning refreshes
found existing LOO-422/423/428/406; no duplicate Tasks were created. LOO-422 now
contains the title/status evidence and LOO-423 the uncovered account guidance/noise
finding; [readbacks](terminal-host-trial/follow-ups.json) verify both additions and
the preserved briefs. The original observations remain attributed to Jack's October 7
[LOO-421 thread](https://linear.app/loopflow/issue/LOO-421), especially comment
`592307f3-4f29-4233-93c6-26c298583abf`. Filing does not establish a fix or completion.

**Remaining keyboard checks:** native resume, standalone Ghostty control and
cmux final Flow exit; exercise fresh disposable Task checkout
and standalone publication. Record exact versions/shims, elapsed time, working,
input-needed, return and exit; attention while unfocused and clearing on return;
colors, resize, scrollback and retained input. Compare provider name, lf Session/Task,
terminal title and host label for fresh/native-resumed/lf/Task-bound launches before
and after rename. Real signed-in herdr still needs a successful Task Flow. LOO-425 already supplies cmux Task execution and in-Flow
publication; it need not be repeated just to populate this matrix.
