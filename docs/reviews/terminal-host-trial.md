# lf in existing terminal hosts · 2026-10-07

**Herdr built and the isolated pane trial ran.** A skill, conversational input/return
and a two-step taskless Flow work with a synthetic provider in herdr and a plain
PTY. Jack Heart's real cmux Task Flow completed six steps and published
[PR #1496](https://github.com/loopflowstudio/loopflow/pull/1496); both reported
interactive Task launches failed. No installed host or real account was changed.

| Path | Plain Ghostty control¹ | herdr 0.9.3 — real pane, fixture provider | cmux — observed by Jack |
|---|---|---|---|
| One skill | PTY fixture success | Fixture success; input/return | Unknown |
| Interactive conversation | PTY fixture success | Fixture success; input/return after resize | Small taskless conversation reported working in concurrent review²; original Task-bound launches failed |
| Two-skill taskless Flow | Fixture success, ordered output | Fixture success, ordered output and scrollback | Unknown |
| Task checkout | Fresh blocked: no Linear team; seeded placement succeeds | Same boundaries; workspace opens lf's checkout | Unknown |
| Task Flow | Fixture blocked: managed account required | Same; three failed first-step attempts | Six steps succeeded; no host status, noisy output |
| PR publish | Default branch refused; checkout blocked at publisher substitute | Same; no GitHub publication attempted | Succeeded inside Flow; standalone unknown |

¹ Ghostty rendering was **not run**. PTY success proves CLI transport only.
² The concurrent [keyboard-review notes](../../scratch/terminal-host-keyboard.md)
record Jack's small Claude conversation succeeding, screenshot evidence of question
attention and post-answer clearing, and his preference to prioritize interactive
Sessions over headless Flows. They also report first-step Flow output with Running;
the full Flow result remains unknown. These are separately authored review receipts,
not new automated trials here. The different prompt/path does not resolve either
original Task launch failure. The review owns preserving its active screenshots.
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

**Keyboard checks:** in dedicated cmux/Ghostty workspaces, run a standalone skill,
taskless conversation and two-step Flow; exercise fresh disposable Task checkout
and standalone publication. Record exact versions/shims, elapsed time, working,
input-needed, return and exit; attention while unfocused and clearing on return;
colors, resize, scrollback and retained input. Compare provider name, lf Session/Task,
terminal title and host label for fresh/native-resumed/lf/Task-bound launches before
and after rename. Real signed-in herdr needs the same state/attention checks and a
successful Task Flow. LOO-425 already supplies cmux Task execution and in-Flow
publication; it need not be repeated just to populate this matrix.
