# LOO-811 live recovery

November 8, 2026. Direction in the recovery conversation: "can we get that installed so that loopflow is routing properly again" and "you can do 811".

## Outcome and remaining work

Validate LOO-747's recovery against installed lf and native clients with fake or
idle logins, without paid inference, manufactured exhaustion, credential output
or disruption to open conversations. Fixture success does not complete the Task.

Still unobserved:

- The repaired two-item switch through installed lf, including the default-home
  unsuffixed service in an isolated OS account. Candidate checks used a temporary
  scoped service; installed checks below predate the repair.
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

## Candidate repair and remaining verification — November 8

The repair selects USER and one distinct existing account from service-scoped
metadata and verifies both writes. Existing account-name filtering remains; among
eligible non-USER accounts, lexical order is the headless choice recorded in
questions.md. Activation/save-back, refresh locks, service selection and stdin
credential writes remain. No service-wide synchronization is promised.

Regression coverage changes lookup order, checks both items after every switch
and detects a legacy write that did not stick. The native fixture checks A → B → A
and account-selected launches; its third-item observation remains separate.

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

Earlier concept-review alternatives remain in
`d4941f045600750beffbbfc73934bd4bbf9abfbb:scratch/live-recovery.md` and its
`scratch/concept-review.html`. The current page reflects the review below.

## Concept review — finish a partial switch

**November 8 implementation: focused tests pass; gate and review remain.**
A successful repeat of `lf account use claude=work@` finishes a partial
installation even when the native item already names work@. The two-item scope
and existing command remain; no new persisted state or public API was added.

The pre-repair counterexample was USER=A, legacy=A → select B → USER=B,
legacy write fails → retry B. The writer reports the first failure correctly,
but `activation::activate` returned early on `same_login`, which reads USER only.
`install_login` then said "already signed in" and `use_login` installed the hold;
legacy stayed A. The failed write also skipped the credential marker touch.
The original writer rejection test stopped at the first error. This path was
traced in source, not exercised against native Keychain.

The subscriptions guide now says: “If a Claude login installation fails, fix the reported
storage problem and repeat `lf account use claude=work@`. It finishes installing
that login before reporting success.” Existing hold, routing, resume and
shared-home guidance otherwise stays unchanged.

Implementation direction (November 8):

### Delete — do not maintain

Completed: Claude's pre-lock and post-lock same-login returns and silent success
on reload-marker open errors (except an absent file) are removed. The same-owner
path reconciles native items. Codex's fast path, save-back, locks and the single
Keychain selector/writer remain. No deletion targets remain.

Compression keeps runtime setup inside the async test fixture. One retry scenario
runs against scoped and unsuffixed services, replacing the separate default-home
retry test while extending its rotation, repeat and normal-switch coverage.

### Required behavior

- Keep provider identity separate from installation completeness. Keychain item
  names are storage addresses, not extra managed accounts. Reconcile USER and
  the one selected eligible legacy item before an already-active Claude activation
  succeeds, including the same-owner return after save-back.
- Derive completeness from the native items under the existing credential and
  refresh locks; add no table, retry record or public API. Keep one service/item
  selection and write/verification path. On a same-login retry, use the current
  native credential, retaining any rotation, rather than reinstalling an older
  stored profile. Successful recovery must also reach the reload marker update.
- Preserve outgoing credentials, Session identity/history/input, scoped service
  selection and the unsuffixed fallback. Keep the lexical headless choice and
  third-item scope in questions.md. A failed partial installation must remain an
  error; no rollback or all-service synchronization is implied.
- Exercise the activation/command path through rejected legacy write → error →
  storage restored → same-command success with both items current. Include a
  newer native credential for the same login, marker recovery, an already-complete
  repeat and A → B → A. Direct writer tests alone miss the early return.

Next review shows that retry alongside the existing normal-switch checks. The
installed and running-client observations listed above remain with demo/review;
this refinement neither completes those checks nor changes delivery scope.

`cargo test -p loopflow --lib lf::commands::account::account_first_tests::claude_keychain:: < /dev/null`
(inherited LF_* unset) — 4 passed November 8: rejected-write retries, access-only
and both-token rotation, marker failure/recovery, complete repeats, normal switches
and unsuffixed fallback using a Keychain stand-in; linker repeated the existing
large `__eh_frame` warning. This does not establish native or installed acceptance.
`cargo build -p loopflow --bin lf < /dev/null` (inherited LF_* unset), `cargo fmt`
and `git diff --check` — passed November 8 for the retry repair.
Gate retains provider_auth, provider_account activation/callers, account command
suites, docs checks and clippy.

Compression recheck November 8: `cargo test -p loopflow --lib lf::commands::account::account_first_tests::claude_keychain:: < /dev/null`
(inherited LF_* unset) — built and passed all 3 consolidated tests, including the
retry scenario in both scoped and unsuffixed homes; same `__eh_frame` warning.
`cargo fmt --check`, `git diff --check`, `lf context` — passed; memory and scratch
remain below both limits. Production code was unchanged in this compression pass.

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
Claude; cleanup succeeded. Correcting the syntax let the earlier fixture pass;
those checks asserted legacy only on the first switch. Each cleanup returned
successfully with no fixture processes left running.

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
