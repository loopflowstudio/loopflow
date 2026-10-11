# LOO-447 — stop and resume

Jack Heart's October 10 decision `fd830a3f` supersedes live takeover for both
Claude and OpenCode. Stop their headless AgentProcess, then resume the same native
conversation. Jack approved #1524 and gate/landing on October 10
(`55bc3fcd`), with completion after verified merge. Removing Codex's live handoff
is separate work and is not part of this PR. Earlier decisions and proof details
remain at `5e95ce9a4:scratch/stop-and-take-over-claude.md`.

## Accepted outcome

- Claude and OpenCode stop through the recorded AgentProcess under the current
  attachment. Superseded callers cannot signal; foreground providers are excluded.
- Connect stops the headless group before launching Claude `--resume` or OpenCode
  `--session` with the saved native conversation. Help/output call this resume,
  not live takeover. Missing native identity refuses before stopping useful work.
- Session/native identity and pending history survive. Process death never means
  a successful provider turn. Unknown OS identity, ambiguous owners and surviving
  helpers do not authorize invented settlement.
- Jack excluded configured-provider runs, SIGKILL timing and historical orphans.
  No installation is authorized. Linux CI owns platform acceptance.

## Delete — do not maintain

Removed both live-takeover preparations: Claude durable replay/uncorrelated result
receipts; OpenCode relay, saved request/permission recovery, startup-title adoption,
independent stderr and their exclusive fixtures. Removed the shared named-lifeline
rewrite: ordinary launch lifelines and Codex handoff remain as on the base.
No independent transport process, terminal client or native supervisor remains.

The surviving changes are common exact group stop, harness stop consumers and
public stop-before-resume. Codex retains generations, fences, connection and FIFO;
its existing unrelated-conversation refusal now precedes common group close.
OpenCode abort uses its existing fenced writer. Creation carries permissions instead
of the invalid PATCH; the obsolete write-retry helper and direct-child stop are gone.

## Proof and remaining work

The public-open fixture runs both providers with isolated stores and stand-ins.
It checks missing-identity preservation, group stop before saved native resume,
stale attachment refusal and an unfinished turn without invented completion.
Harness stop fixtures exercise current/stale ownership. Existing common close tests
cover foreground exclusion and surviving helpers. These establish no configured
provider or installed acceptance.

Implementation and review were complete at `f2096ffce`; #1524 is published.
Gate found two stale fixtures: a missing Codex connection masked non-group
refusal, and the socket stand-in exited before exact-authority shutdown. Both
are repaired without weakening group-death or attachment checks. Linux CI and
verified merge remain. The original live-takeover demo and both-death-order acceptance are
superseded by Jack's stop-and-resume decision, not claimed as passing. Codex's
handoff remains unchanged apart from the shared close path. No product decision
is open. Release's distinction between process exit and product outcome also
applies here: a stopped group proves neither turn completion nor successful resume.

## Checks

`uv run python scripts/test.py --base 5f44fd9e --reuse-passing`: architecture, fmt, Clippy and website (76) pass; materialized Rust 2394/2396 passed, two fixture failures repaired; materialized `cargo nextest run -p loopflow -E 'binary(agent_tests) | binary(context_launch_tests) | test(resume_refuses_live_agent_that_cannot_be_ended) | test(public_open_stops_headless_group_and_resumes_saved_conversation)'` 19/19 pass; post-repair fmt/Clippy, Ruff, `git diff --check` and skill-alignment pytest (4) pass; full Linux matrix remains CI-owned.

Published head `aa8ce6b274` passed only the `scratch-clear` checkpoint;
the test matrix was skipped. That is not Linux acceptance. Gate repairs are
fixture-only; production stop authority and Codex live handoff remain unchanged.
