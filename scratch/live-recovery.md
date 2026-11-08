# LOO-811 live recovery

November 8, 2026. Direction in the recovery conversation: "can we get that installed so that loopflow is routing properly again" and "you can do 811".

## Outcome and remaining work

Validate LOO-747's recovery against installed lf and native clients with fake or
idle logins, without paid inference, manufactured exhaustion, credential output
or disruption to open conversations. Fixture success does not complete the Task.

Still unobserved:

- A running Codex refresh after switching the shared home: whether it rewrites
  auth.json and loses the other login's rotated refresh token.
- Claude's no-model "You've hit your limit" line paired with every account-wide
  usage window below 100%, and whether clearing that cooldown repeatedly selects
  the spent login until the attempt budget runs out. Polls alone establish neither.
- `resume --account` for running Codex and Claude, `session move`, and running
  Claude's credential reload. Check Session identity, retained history and continued
  input alongside account selection. The native fixture starts fresh Claude clients.
- Whether refreshing Claude invalidates earlier access tokens used by a rental.

The current iteration retains the Task's two-item requirement. Its direction is:
"implement a focused repair for the existing two-item expectation, with regression
coverage where lookup order changes and both items must follow every switch."
The earlier concept-review alternatives do not select a narrower or broader promise.

No isolated real-login environment was available. Shared production logins had open
conversations. Demo/review owns the remaining live checks above, using disposable
or idle logins or a naturally occurring failure. Delivery remains review-only.

## Two-item repair — November 8

Keep activation/save-back, refresh locks, service selection and stdin credential
writes. Query only metadata for the selected Keychain service; select USER and one
distinct existing account. With two items this is unambiguous. With more, choose
the lexically first non-USER account for stable selection independent of lookup
order; there is no service-wide synchronization. Verify both writes before success.
No new account concept, persistence or schema is needed. Native metadata search
uses the existing transitive security-framework dependency directly; password
reads/writes keep using security(1), retaining its Keychain access behavior.

Delete — do not maintain (all removed; no remaining targets):
- The service-only single-account metadata lookup and text parser: replace with a
  service-scoped attributes-only query; preserve default-home unsuffixed fallback.
- The fixture's fixed lookup order and first-switch-only legacy assertion: replace
  with alternating lookup order and both-item checks after every switch.
- The fake security command's metadata output, now unused by the native query.

Compression keeps the two selected items in one reusable iterator for writing and
verification. Native fixture logins are decoded objects, serialized only for
storage; every comparison, including the separate third-item observation, ignores
JSON formatting. The profile and its digest use the same serialized bytes.

Checks on November 8 (inherited LF_* unset):
- `cargo test -p loopflow --lib provider_auth::tests::keychain_switch_ < /dev/null` — 3 passed; linker warned about large `__eh_frame`.
- `cargo build -p loopflow --bin lf < /dev/null` — passed.
- `uv run python scripts/test_network.py uv run --script tests/e2e/claude_shared_home.py --claude /Users/jack/.local/bin/claude --lf target/debug/lf --output .lf/tmp/loo-811-two-item-repair < /dev/null` — passed with Claude 2.1.319; both items followed every switch, plain Claude followed, hold/order and shared launches passed; third item unchanged. Cleanup completed.
- `uv run ruff check tests/e2e/claude_shared_home.py` / `uv run ruff format --check tests/e2e/claude_shared_home.py` / `git diff --check` — passed after formatting one long line; native result reused for that whitespace-only change.

Candidate SHA256: `93053e164ac5e2fd4bd4d78df95243f84801a6b93ceb185ecb7e4dcdd79e20c3`.
Native receipt: `.lf/tmp/loo-811-two-item-repair/results.json`. This proves the
candidate with synthetic logins, not installed repair or live token rotation.
Gate owns affected provider_auth, provider_account::activation and account command
suites plus clippy. Demo/review retains the running-client and refresh checks above.

Compression checks on November 8 (inherited LF_* unset):
- `cargo test -p loopflow --lib provider_auth::tests::keychain_switch_ < /dev/null` — 3 passed; same large `__eh_frame` linker warning.
- `cargo build -p loopflow --bin lf < /dev/null` — passed.
- `uv run python scripts/test_network.py uv run --script tests/e2e/claude_shared_home.py --claude /Users/jack/.local/bin/claude --lf target/debug/lf --output .lf/tmp/loo-811-two-item-compress < /dev/null` — passed; all eight result fields match the repair run, including both items following every switch and the third unchanged. Cleanup completed.
- `cargo fmt --check`, Ruff check/format, `git diff --check` — passed; gate retains affected suites and clippy.
- `lf context` — memory and scratch remain below both token and byte limits.

Compressed candidate SHA256: `8f5c270b27973262028fa1050ab80385fae9799fdc118aebc8e6d9b48376d4fa`.

Proposed Wave memory correction: two-item scope is retained and candidate native
switching now passes; the historical return-switch failure and all remaining live
acceptance stay recorded. Wave files were left as inherited in this step.

Historical concept-review alternatives and HTML are retained as review history;
the repair direction supersedes their "Blocked" status. Earlier full text is in
`.lf/prompts/20261108-092820-a1268f37-aaa0-49fe-b5c2-e55569938be3-Wds4op-summary.md`.

## Native observations — November 8

Installed lf 0.15.2+dad2621c1 and Claude 2.1.319 passed the shared-home fixture
using synthetic logins, a local endpoint and a temporary scoped service in the
real macOS Keychain. Fresh Claude followed the USER item through switches;
hold/order/release and shared launches passed. The production unsuffixed service
was not exercised.

Both USER and legacy items changed on the first switch. After first → second →
first, the legacy item retained second while USER and plain Claude followed first.
Three compression runs requiring both items after every switch failed there;
parsed JSON comparison still differed only in accessToken/refreshToken. The pre-repair writer
updated USER and the first service-only match, which could be USER itself. A third
item also retained the earlier login while plain Claude used the selected one.
Staleness is an observation, not required product behavior.

The first attempt stopped at obsolete `account claude use` syntax before launching
Claude; cleanup succeeded. The repaired fixture and its simplified rerun passed;
each cleanup returned successfully with no fixture processes left running.

Check: `uv run python scripts/test_network.py uv run --script tests/e2e/claude_shared_home.py --claude /Users/jack/.local/bin/claude --lf /Applications/Loopflow.app/Contents/MacOS/lf --output .lf/tmp/loo-811-compress < /dev/null` with inherited LF_* unset — passed November 8; all eight fields matched the earlier `.lf/tmp/loo-811-claude-shared` run, including third-item unchanged. This earlier result checked the legacy item only on the first switch; it does not substitute for the stronger candidate check above.

## Routing observations — November 8

At recovery start, installed lf reported 0.15.1+e3c888924; subsequent calls reported
0.15.2+dad2621c1 containing LOO-747. The app artifact download was still running at
that observation; this does not establish completed installation of the pair.
Removing repository-local `account.codex: [manabot-eng]` restored the global pool.
Provider reads accepted jack@loopflow.studio (69% weekly used),
loopflow-eng@loopflow.studio (68%) and jackstah@gmail.com (0%); manabot-eng was
unavailable/missing with 99% in its prior reading. `lf account next` selected
jack@loopflow.studio: "31% left before Wed 08:04: the most to spend in the time left".

Claude reads accepted both managed logins (98%/80% account-wide; Fable 100% on
both). Session inventory reported 22 Opus Sessions, none under
`--blocked --on claude:opus`, and no Sonnet Sessions. `lf account next -a
claude:sonnet --json` warned "This command does not start an agent; --agent has no
effect." Its model-unspecified blocked result does not establish blocked Sonnet
launches.
