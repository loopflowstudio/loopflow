# LOO-447 — stop and resume

Jack Heart's October 10 decision `fd830a3f` supersedes live takeover for both
Claude and OpenCode. Stop their headless AgentProcess, then resume the same native
conversation. Publish when this works; do not land. Removing Codex's live handoff
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
  No installation or landing is authorized. Linux CI owns platform acceptance.

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

Implementation and review are complete; publication remains. No product decision is open.

## Checks

`cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `git diff --check` and six focused `cargo test -p loopflow --lib` cases (public resume, Claude/OpenCode stop, OpenCode creation, all-provider authority, foreground exclusion): pass; Linux acceptance remains CI-owned.
