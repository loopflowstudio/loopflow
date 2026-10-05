# Assumptions and dated evidence — LOO-378

The current native smoke runner uses disposable provider storage and synthetic
Responses, with no account dependency. The earlier authenticated probe below
resolved a feasibility question; its account route and capacity observations
are historical, not prerequisites or permission to change installed routing.
Failed-attachment preservation remains an implementation problem within Jack
Heart's accepted scope. Delayed driver transfer is a proposed mechanism; no
change to takeover or review-completion semantics has been accepted.

## Dated probe evidence — October 5 UTC

Historical observations from the earlier probe, not current capacity or routing
claims. Retained here separately from the implementation plan.

### Test-owned provider authentication — resolved October 5 UTC

An empty `CODEX_HOME` has no login, and Doppler holds no OpenAI key; neither is
needed. The existing managed Codex account works for an isolated Session without
copying a credential or changing machine-wide authentication:

```bash
env -i HOME="&#36;HOME" PATH="&#36;PATH" TERM=xterm-256color \
  LF_HOME="&#36;(mktemp -d)" CODEX_HOME="&#36;HOME/.lf/accounts/codex/manabot-eng" \
  target/debug/lf -m codex --mode batch : "..."
```

This is the variable `--isolate` sets for a managed account, applied by hand
because a disposable Home has no account catalog. `env -i` drops the inherited
Task, Run and account variables. The provider reads and refreshes its login in
that home in place. Evidence: one batch turn returned its exact marker (exit 0);
the disposable Home received its own database and runs; no engine survived.

Limits the runner must respect:

- Each run writes a rollout under that home's `sessions/`. The probe's own file
  was removed by thread ID. The runner removes only threads it created.
- Turns spend manabot-eng@loopflow.studio's weekly window (Pro, 0% used at
  04:55 UTC October 5). The installed store does not see this usage. Keep prompts
  tiny, and seed each history once and reuse it across samples.
- Do not use ambient `~/.codex`: it holds loopflow-eng@loopflow.studio at 100%
  weekly with a paid credit balance, so a turn there may bill credits.
- The `lf home ssh` lease broker is not a local path: it targets a remote Home,
  runs that Home's installed `lf`, and cannot select a disposable Home.
- The Claude path was not probed. `lf account route` reports no eligible managed
  Claude account (identities unverified); record that path as unmeasured unless
  a later check shows otherwise.

Routing in the installed Home was stale, not exhausted: `lf account codex`
replaced a two-day-old "100% used" reading for manabot-eng with a live 0%, and
the repository route now leads with it. Still unavailable: loopflow-eng (weekly
100% until October 9), jackstah@gmail.com (session 100% until 09:41 UTC October
5), and jack@loopflow.studio, whose stored credential reports
`fixture@example.com` and needs `lf account connect codex jack@loopflow.studio`
from Jack. That last one is not required for this Task.

Fixture note: completed Flow steps leave their Codex engines running (two were
idle in this checkout after the `code` Flow finished). A headless Flow-step
Session in the disposable Home is therefore a candidate live fixture; a plain
batch run closes its engine and is not.
