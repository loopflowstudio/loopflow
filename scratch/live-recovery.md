# LOO-811 live recovery

November 8, 2026. Direction in the recovery conversation: "can we get that installed so that loopflow is routing properly again" and "you can do 811".

## Observed before validation

- Installed lf began at 0.15.1+e3c888924, before LOO-747's merge 90a82635fc. `lf install` is downloading the dev package. Subsequent invocations now report 0.15.2+dad2621c1, which contains that merge; the original installer is still downloading its app artifact.
- The repository-local `account.codex` value was `[manabot-eng]`, overriding the global pool. `lf unset account.codex` removed it from `/Users/jack/src/loopflow/lf.config.local.yaml`.
- Fresh provider reads accepted jack@loopflow.studio (69% weekly used), loopflow-eng@loopflow.studio (68%), and jackstah@gmail.com (0%). manabot-eng was unavailable/missing, with 99% in its prior usage reading.
- `lf account next --repo /Users/jack/src/loopflow.loo-811-check-usage-limit --json` now selects jack@loopflow.studio: "31% left before Wed 08:04: the most to spend in the time left".
- Both Claude logins reported their Fable windows at 100%; account-wide windows were 98% and 80%. `lf account next` reported Claude blocked; model-specific observations follow below.

## Remaining

Carry out this Task's original brief against installed lf and real or idle logins, spending no paid quota on validation probes. Do not exhaust an account to manufacture a limit. Do not print or directly read raw credentials; use supported provider operations and metadata. Avoid disrupting active conversations. Record each actual check and any claim that remains unobserved.

If verification finds a code defect, repair it in this checkout and publish for review; merging has not been requested. The parent conversation is finishing the install. Leave the Competitor architecture research Session alone; it is open in Rex for continued discussion.

## Validation scope (2026-11-08)

The existing native-Claude shared-home fixture covers two real macOS Keychain
items, a third item, and a held login overriding a repository order. Its local
endpoint and synthetic credentials establish client behavior without paid quota.
No runtime change is justified by the observations so far.

Delete — do not maintain: service-only single-item cleanup and the retired
`account claude use` spelling, replaced in the fixture. The fixture's ambiguous
`_native_login` reader is replaced by named Keychain reads and a direct file read
off macOS; each `use` checks the selected USER item. The E2E README owns native
fixture instructions; the Rust README links there. No deletion remains.

Installed observations: `lf account claude --json` accepted both managed logins
on November 8 (98%/80% account-wide; Fable 100% on both). `lf session list --all
--on claude --limit 0 --json` reported 22 Opus Sessions; the same selector with
`--blocked --on claude:opus` returned none. There were no Sonnet Sessions.
`lf account next -a claude:sonnet --json` warned "This command does not start an
agent; --agent has no effect." Its model-unspecified Claude result was blocked;
that does not establish that Sonnet launches are blocked.

Native observation: lf 0.15.2+dad2621c1 and Claude 2.1.319 changed both the
USER-named item and the older service-only item. A third item retained the
previous synthetic login, but plain Claude sent the selected login. This used a
temporary scoped service in the real Keychain, not the production unsuffixed
service. The fixture records the third item's state without requiring stale
credentials as product behavior. The first attempt stopped at the obsolete
`account claude use` syntax before launching Claude; cleanup succeeded.

Compression reruns on November 8 exposed a narrower two-item guarantee: after
first → second → first, `fixture-legacy` retained the second login while the
USER item and plain Claude followed first. Three runs with an added assertion
requiring both items after every switch failed at that return switch; parsed
JSON comparison still failed, with only accessToken/refreshToken differing.
The first switch's two-item check remains; subsequent switches check the USER
item. This does not establish that every item follows every switch. The writer
updates USER and the first service-only match, which can be USER itself.

Still unobserved, requiring an isolated real-login environment or a naturally
occurring failure:

- A running Codex refresh after switching the shared home, including whether it
  rewrites auth.json or loses the other login's rotated token.
- A no-model spent line paired with account-wide usage below 100%. Current
  polls alone establish neither the line nor the suspected retry loop.
- `resume --account` for running Codex and Claude, `session move`, and running
  Claude's credential reload. The native fixture starts a fresh Claude each time.
- Whether refreshing Claude invalidates an earlier access token used by a rental.

No suitable isolated real-login environment was supplied. Shared production
logins have open conversations; switching or forcing refresh there conflicts
with the nondisruption constraint. LOO-811's full acceptance remains open.
Gate owns the affected architecture check; demo/review owns these live checks.

Check: `uv run python scripts/test_network.py uv run --script tests/e2e/claude_shared_home.py --claude /Users/jack/.local/bin/claude --lf /Applications/Loopflow.app/Contents/MacOS/lf --output .lf/tmp/loo-811-claude-shared` with inherited LF_* unset and stdin closed — passed November 8; two-item updates, three-item client selection, hold/order/release and shared launches passed; third item unchanged. Cleanup returned successfully and no fixture processes remained. Ruff format/check and `git diff --check` passed.

Check: same native command with `--output .lf/tmp/loo-811-compress < /dev/null` — passed November 8 after simplification; all eight result fields matched the earlier run, including the unchanged third item. Ruff check/format and `git diff --check` passed; gate owns affected suites, demo owns the remaining live checks.
