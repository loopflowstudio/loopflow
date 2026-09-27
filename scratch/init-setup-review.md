# Where lf init belongs in newcomer setup

## Feedback

During interactive design review, Jack asked: "have we looked at lf init".
The kickoff design proposed removing the immediate documentation step but did
not record an inspection of the skill or assess reusing it for the app journey.

Jack subsequently clarified: "init convo is there as an option to consider,
but it doesnt fit perfectly into our world where we prefer scoped sessions.
maybe a repo-wide init would be better. if we can get by without it, great".

## Evidence

- [Current design](a-newcomer-gets-from-download.md).
- [Existing init skill](../rust/loopflow/src/engine/builtins/ops/skill/init.md):
  discovers agents/accounts, guides chosen connections and configuration,
  supports direct skills without Linear, and supplies a next command. Assumes
  repository setup; offers the Mac app at the end.
- [Launcher](../rust/loopflow/src/lf/commands/run.rs): `launch_prompt` checks
  harness availability before execution. An agent-run init cannot install the
  first agent required to run itself.
- [Get started](../docs/getting-started.md): currently puts `lf init` immediately
  after the installer and lists an authenticated coding tool and Git project as
  prerequisites.

Source inspection only. No init run, account connection, or live UI trial.

## Agreed direction and design changes

Use deterministic app actions for installation and account connection. No init
conversation, synthetic Session scope, or mandatory init after project selection.
Keep existing repo init separately available; Jack's repo-wide suggestion does
not authorize a redesign within this Task. The design records init's existing
responsibilities and the bootstrap constraint so implementation can reuse the
underlying auth operations without duplicating broad conversational setup.

Early GitHub sign-in and optional Linear sharing are recorded in
[account requirements](account-requirements-review.md); DMG acquisition in
[first-launch review](dmg-first-launch-review.md).

## Next useful action

Implement the setup actions and document the newcomer path without init. No
remaining product question about the init handoff. First-launch activation,
`gh` acquisition, and live acceptance remain engineering work in the current
design. No Flow navigation decision or implementation acceptance is recorded.
