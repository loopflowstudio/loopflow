# lf in existing terminal hosts · 2026-10-07

**Herdr built and the isolated pane trial ran.** A skill, conversational input/return
and a two-step taskless Flow work with a synthetic provider in herdr and a plain
PTY. Jack Heart's real cmux Task Flow completed six steps and published
[PR #1496](https://github.com/loopflowstudio/loopflow/pull/1496); both reported
interactive Task launches failed. No installed host or real account was changed.

| Path | Plain Ghostty control¹ | herdr 0.9.3 — real pane, fixture provider | cmux — observed by Jack |
|---|---|---|---|
| One skill | PTY fixture success | Fixture success; input/return | Unknown |
| Interactive conversation | PTY fixture success | Fixture success; input/return after resize | Task-bound Codex and large-prompt Claude failed; taskless unknown |
| Two-skill taskless Flow | Fixture success, ordered output | Fixture success, ordered output and scrollback | Unknown |
| Task checkout | Fresh blocked: no Linear team; seeded placement succeeds | Same boundaries; workspace opens lf's checkout | Unknown |
| Task Flow | Fixture blocked: managed account required | Same; three failed first-step attempts | Six steps succeeded; no host status, noisy output |
| PR publish | Default branch refused; checkout blocked at publisher substitute | Same; no GitHub publication attempted | Succeeded inside Flow; standalone unknown |

¹ Ghostty rendering was **not run**. PTY success proves CLI transport only.
Evidence and outcome remain separate in the [receipts and repeat recipe](terminal-host-trial/README.md).
The runner used a copied **lf 0.13.9** executable (revision unknown), not a build of
this checkout. Jack's exact binaries, shims and timings remain unknown.

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

**Established stalls.** The control and herdr expose two shared adoption boundaries:
a fresh Task needs repository Linear configuration; the seeded Task needs a managed
provider account. Their account error repeats three times, adding noise. These are
fixture observations, not three herdr defects. **Unfiled:** planning/account owner
unresolved; demonstrate the supported newcomer path with a disposable issue and
account, and explain the first dependency before execution. Repeated account-error
noise joins the polish brief below. Successful real Task execution remains unproven.

The three cmux stalls, provisionally ranked:

1. **Interactive startup:** Codex rejected duplicate
   `--dangerously-bypass-hook-trust`; Claude's 36,090-token assembled prompt hit
   the shim's 122,880-byte argument limit. Token count does not establish bytes.
   Jack names [LOO-422](https://linear.app/loopflow/issue/LOO-422) and
   [LOO-423](https://linear.app/loopflow/issue/LOO-423) collectively; per-failure
   ownership is unresolved. Retest both exact paths after repair.
2. **No headless status:** `cmux list-status` stayed empty through
   `lf -b task run LOO-425 pursue`; branch and directory appeared. Notifications
   are unknown. **Unfiled: status owner unknown.** Acceptance: real-provider
   working/input-needed/exited states and attention from another pane.
3. **Unreadable output:** whole prompts/schema in INFO logs, token tables and
   repeated Started warnings obscured progress. The small local fixture also
   prints per-step tables and INFO, but does not reproduce the large prompt wall.
   **Unfiled: polish owner unknown.** Acceptance: default output exposes progress
   and failures at ordinary width, including one useful account failure; detailed
   diagnostics remain available.

**Titles:** native `claude --resume` showed *Loopflow operating guide*; lf workspaces
showed directory or command. Native resume's Running, branch, directory and hooks
worked. Three steers repeat one observation; first-content naming remains Jack's
hypothesis. Owner: LOO-422. Source: Jack's October 7 [LOO-421](https://linear.app/loopflow/issue/LOO-421)
steers, especially comment `592307f3-4f29-4233-93c6-26c298583abf`; no raw capture
was supplied. Host title/status derivation remains unresolved; the snapshots and
reports establish labels, not their precedence or source. Upstream #1490 reads
OSC 7501 in Desktop; lf emission remains unfinished, so it does not resolve the
cmux finding. No follow-ups were filed or updated here; that deliverable remains open.

**Keyboard checks:** in dedicated cmux/Ghostty workspaces, run a standalone skill,
taskless conversation and two-step Flow; exercise fresh disposable Task checkout
and standalone publication. Record exact versions/shims, elapsed time, working,
input-needed, return and exit; attention while unfocused and clearing on return;
colors, resize, scrollback and retained input. Compare provider name, lf Session/Task,
terminal title and host label for fresh/native-resumed/lf/Task-bound launches before
and after rename. Real signed-in herdr needs the same state/attention checks and a
successful Task Flow. LOO-425 already supplies cmux Task execution and in-Flow
publication; it need not be repeated just to populate this matrix.
