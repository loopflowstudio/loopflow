# Try lf inside herdr and cmux

Approved for implementation · LOO-421 · 2026-10-07

## Accepted outcome and constraints

Jack Heart wants evidence of newcomer adoption in existing terminal workspaces:
a skill/conversation/Flow, then a Task, then a Wave, with Desktop optional.
This slice covers the first two levels in plain Ghostty, herdr and cmux.
Jack accepted the revised plan on October 7 (“ok lets proceed”), then explicitly
corrected stopping after a build timeout (“dont justgive up. keep going”).
The timeout remains an attempt receipt; continuing the same source/cache produced
a working herdr binary and isolated real-pane trials.

Jack authorized isolated headless trials and a source build. The concurrent
keyboard review later received permission for dedicated new windows and
Jack-started provider commands; that exception does not authorize this automated
trial to drive live cmux. Existing windows and conversations remain untouched. No logins, credential reads, provider-account
use, host integrations or lifted host implementation/configuration are permitted.
Signed-in checks belong to Jack. Jack's later October 7 direction (Linear comment
`fa289cba-babc-4afa-a70a-b7c2c0a5ab6b`) authorizes an interactive Flow Rust fix in
this branch and a fixed-binary cmux demo, owned by the ongoing review Session.
That supersedes the findings-only repair boundary for this specific fix.
The implementation Flow stopped after evidence reconciliation. Jack subsequently
requested landing this branch after the fixed-binary trials (October 7). That
authorizes delivery of the repair and findings; remaining trials keep the Task open.

## Evidence and remaining work

The [findings](../docs/reviews/terminal-host-trial.md) own the three-host matrix,
stalls, follow-up owners and keyboard checks. The [repeat recipe and receipts](../docs/reviews/terminal-host-trial/README.md)
retain build attempts, confinement, synthetic skill/conversation/Flow trials,
Task/account/publication boundaries and direct/wrapped title probes. Their pinned
installed lf 0.13.9 has no known source revision; checkout fixes cannot rebut
Jack's release observations. Expected refusal is not successful adoption.

1. **Signed-in and rendered evidence.** Plain Ghostty GUI remains untested;
   PTY evidence is separately labeled. Concurrent `terminal-host-keyboard.md`
   records a small cmux conversation, question attention/clearing and first-step
   Flow output. Jack prioritizes interactive Sessions. That review owns its active
   screenshots. Remaining cmux checks include standalone
   skill, full taskless Flow, fresh checkout, standalone publication and the exact
   original failing launches. LOO-425 already supplies cmux Task Flow/in-Flow
   publication evidence. Jack subsequently confirmed native herdr recognition/step handoff, background
   question attention/clearing, shell return and resize/draft behavior. Successful
   signed-in Task execution and native resume remain unproven. The durable
   [keyboard record](../docs/reviews/terminal-host-trial/keyboard.md) owns this evidence.
2. **Exact cmux title source.** Herdr's original direct/wrapped probes establish
   OSC 2 pane titles independently of workspace/tab labels and executable-name
   recognition, with unknown input/return state. They do not identify every parser
   or rendered precedence. Public cmux reports distinguish hook-fed status from
   names reaching tabs. Jack's first-content hypothesis still needs his keyboard
   comparison; live cmux/settings/provider history remain outside this trial's
   access. Three title steers repeat one observation.
3. **Follow-up fixes and acceptance.** Filing is reconciled: LOO-428 owns both
   interactive launch failures and Flow noise, LOO-406 no-Linear planning, LOO-422
   status and Jack's explicit title assignment, LOO-423 other polish plus account
   guidance/repeated errors. [Readbacks](../docs/reviews/terminal-host-trial/follow-ups.json)
   retain additions and intact prior briefs, including the reconciled LOO-422
   remote-success/local-lock write. No duplicate Task or competing worker exists.
4. **Interactive Flow repair and demo.** Jack's keyboard trial of
   `lf -m claude run ux-flow` and its `-i` variant still showed streamed steps.
   Before the review Session's repair, checkout `Cli::step_args` added `--batch`
   and the skill launcher retained it. Jack then explicitly required interactive
   Flow mode to work and requested a fixed-binary cmux demo in this branch.
   The fix forwards the caller's mode, including terminal-default behavior, into
   each ordinary skill and Task Flow launch. A successful native exit advances;
   failed/interrupted exit stops. PTY regression and all 28 Flow tests pass;
   Jack's cmux screenshot shows native step two with its title and question;
   final exit remains to check. Headless propagation stays lower priority.
5. **Gate/review.** Human usability, color/notification judgments and real-account
   evidence remain separate from automated checks. No external-progress or
   Desktop-duration KR credit is earned.

## Reconciliation choices

Upstream #1490 reads OSC 7501 reports in Desktop; that supplies neither lf
emission nor cmux proof. LOO-422's current brief names LOO-415 as the relay/write
relationship, superseding the earlier unresolved mapping from memory. No status
injection or host-specific integration is added.

## Current implementation

### Wrapper compatibility investigation · 2026-10-07

Jack Heart requested studying what hosts require from an agent wrapper. Read-only
inspection covered the installed cmux 0.65.0 wrapper resources, pinned herdr
`4dc23bb15d4a2fd2c093abfb509f903c3015bf56`, and official public interfaces.
The following are behavioral contracts and proposed lf requirements, not copied
implementation, newly authorized integrations, or completed live acceptance:

- Preserve the native provider terminal, executable discovery and per-pane host
  environment. cmux's PATH wrappers attach lifecycle/attention hooks. Herdr can
  recognize a provider within the foreground job and use its live terminal UI.
  The restored interactive Flow path supports this; replacing the native UI with
  rendered stream output changes the available evidence.
- The installed cmux Claude wrapper supports attention for structured questions
  and plan/permission decisions, plus clearing when work continues. Ordinary
  prose questions carry no equivalent guaranteed signal. This explains the
  different screenshot states without requiring text heuristics in lf.
- Keep native agent attention and lf's Task/Flow context distinct. A separately
  keyed cmux status can describe the Task and step without replacing provider
  state. Herdr's explicit reports take precedence over screen detection, so a
  coarse outer-Flow report could hide a provider's actual question.
- Do not continually overwrite provider terminal titles: herdr can use activity
  in a native title as state evidence. cmux also has optional automatic naming.
  Prefer separately owned Task/step metadata; the exact title source in Jack's
  earlier resume remains unproven.
- Herdr's public self-reporting interface accepts state, Session/resume identity,
  and release. Reports must address the owning pane and preserve ordering. A
  future lf integration should resume the lf Session with its context, without
  implying that a stopped Flow resumes. Publish changed Session identity at a
  step handoff and clear only lf-owned metadata when its owner exits.
- Hooked native launches, headless CLI invocations and app-server launches are
  different integration paths. The installed cmux Codex wrapper excludes
  app-server from session hook injection. Preserve large-prompt support and
  composable flags/settings while keeping wrappers on PATH; bypassing wrappers
  to avoid launch errors would lose the host integration under investigation.

References: [cmux wrapper](https://github.com/manaflow-ai/cmux/blob/main/Resources/bin/cmux-claude-wrapper),
[Codex wrapper](https://github.com/manaflow-ai/cmux/blob/main/Resources/bin/cmux-codex-wrapper),
[cmux metadata API](https://cmux.com/docs/api#sidebar-metadata-commands),
[herdr detection](https://herdr.dev/docs/agents/),
[herdr reporting interface](https://herdr.dev/docs/add-herdr-support/).
Upstream main/docs can differ from the inspected installed/pinned versions.
No additional real-account trial or host mutation was performed for this research.

### Relationship to LOO-428 and LOO-429

Jack Heart requested this comparison. Supported Task status reads returned
available planning without stale/error flags: LOO-428 revision
`2026-10-08T04:51:53.084Z`, LOO-429 revision `2026-10-08T05:32:50.949Z`.

- LOO-421 owns the host trial and the specifically authorized interactive Flow
  mode regression fix. It has not repaired duplicate Codex flags, large native
  prompts, default log noise, or implemented host state reporting.
- LOO-428 owns compatible launches with wrappers retained, large prompt
  transport, and readable Flow output. The small native Claude demo does not
  establish either original failing Task launch's acceptance.
- LOO-429 owns prompt placement: proposed curated instructions/goal/memory in
  harness instruction files, uncontrolled context in the user message, plus a
  single rejection fallback and real large-context cmux proof. It explicitly
  excludes stdin for interactive input and investigates the resulting titles.
- Their prompt work overlaps. Proposed coordination: LOO-429 defines assembly
  placement; LOO-428 consumes it for transport and keeps duplicate-flag/log work.
  Moving curated context alone cannot bound a large scratch/diff user argument.
  Falling back to the old placement must not restore the same argument-limit
  failure. These are design consequences to reconcile, not accepted scope edits.
- Recognition, attention, titles, resume identity and outer Flow metadata from
  the host-interface investigation mostly inform LOO-422. Preserve native host
  integration while fixing launches; do not add a competing status source merely
  because small interactive launches now work.

One runner owns verdicts and confinement/cleanup. `_observe_titles` now uses
`_scenario`, including failure receipts when the host socket is unavailable.
Explicit argv supports direct probes and lf commands through the same path.
Raw snapshots, earlier attempts and Jack's contrary release observations remain.
The interactive Flow fix changes launch-mode forwarding, with no schema change.

The runner fails on unexpected CLI results, reordered/missing steps, lost input
return, capture overflow or cleanup problems; encountered product prerequisites
stay blocked. PID/birth tracking and confinement are unchanged. No fake accounts
are seeded to turn Task execution green. Install-to-real-use time stays unknown;
fixture/build durations remain single samples without a performance target.

Check: `git diff --check` PASS; Flow skill launch inspected; prose-only reconciliation reuses the prior both-host/title smoke pass and 19 verdict checks (receipts `/private/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/lf-trial-receipts-nu9t5yht`); signed-in/rendered checks: Jack; broader verification: gate.
