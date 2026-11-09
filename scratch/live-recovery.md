# LOO-811 live recovery

November 8, 2026. Direction in the recovery conversation: "can we get that installed so that loopflow is routing properly again" and "you can do 811".

## Outcome and remaining work

Shipping docs for the expanded recovery scope are in `docs/sessions.md`
(Recover after a usage limit), `docs/subscriptions.md` (When an account runs
out), and `swift/README.md` (Session panes). November 9 write-docs describes
intended behavior; implementation and validation remain. Main, #1863 and #1839
were reconciled at `4276fb28b`, retaining the Keychain repair and both recovery
notes. `interactive-limit-recovery.md` now holds the draft replacement design,
the native Codex continuation probe and preservation cases. Existing account
resume ends the native client; automatic recovery needs a provider-generation
handoff that retains its editor, pending requests and exit ownership. That
lifecycle change remains unimplemented; no automatic restart or Desktop
recovery action is published.

November 9: `lf open --host desktop --file docs/sessions.md --file docs/subscriptions.md --file swift/README.md`
reported all three opened in Desktop. `git diff --check` passed; prose-only
changes need no build. No new product question arose; the reported provider
remains unspecified, and the docs cover both Claude and Codex.

The expanded scope, original report and preservation cases are in
[interactive-limit-recovery.md](interactive-limit-recovery.md).

Compression checks, November 9 (inherited LF_* unset for tests):
- `cargo test -p loopflow --lib keychain_ < /dev/null` — built; 5 passed, existing large `__eh_frame` linker warning.
- `cargo test -p loopflow --lib lf::commands::account::account_first_tests::claude_keychain:: < /dev/null` — 3 passed; gate retains affected suites and clippy.
- `cargo fmt --check`, `git diff --check`, `lf context` — passed; both context budgets remain satisfied.

Validate LOO-747's recovery against installed lf and native clients with fake or
idle logins, without paid inference, manufactured exhaustion, credential output
or disruption to open conversations. Fixture success does not complete the Task.

Still unobserved:

- A partial native Keychain write followed by the same-command retry. The retry
  repair has command-installation coverage with a Keychain stand-in. Native QA
  below now covers seeded stale-item reconciliation and a real marker permission
  failure/retry; it does not cause a native Keychain write rejection.
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

## Finish a partial switch — implemented, verification remains

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

The implementation follows the iteration direction: "Reconcile USER plus the
selected legacy item under existing locks using the newest native credential,
retaining lexical selection and third-item scope from scratch/questions.md."

Claude's same-login returns no longer bypass installation. The same-owner path
after save-back writes and verifies the two selected items from the current
native credential under the credential and refresh locks. Marker open/touch
failures propagate; an absent marker remains valid because Claude reloads when
it is absent. Codex's fast path, outgoing save-back and service fallback remain.
No rollback or synchronization of every service item is promised.

The three consolidated tests call the command's `install_login` through
activation. They cover rejected writes and failed retries, access-only and
both-token rotation, marker failure/recovery, complete repeats and normal
switches. The retry/switch scenario runs against scoped and unsuffixed services;
the third item remains unchanged. These are stand-in results,
not an executable CLI/hold test or Session identity/input continuity observation.

Remaining: gate runs affected provider_auth, activation/caller and account command
suites, docs checks and clippy. Demo/review shows native retry alongside normal
switches and retains every installed/running-client check above, including Session
identity, history and continued input. No further product choice is needed for
this repair; delivery remains review-only.

Concept-review, November 8: no further design change or open product choice.
Normal switching, partial-write retry and rotation fit one interaction: repeat
the account command to finish installation. Keychain item names remain storage
addresses, not managed accounts; completeness needs no persisted retry state.
Advance to gate and demo with the remaining observations above. Existing usage
guidance needs no further change, and no skill instruction changes are needed.

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

Realign November 8: reused those results; only the plan and Wave memory changed.
`git diff --check` and `lf context` — passed; both context budgets remain satisfied.

## QA — November 8

Report: [qa.html](qa.html). Candidate `79053ecc7.dirty`, SHA256
`edb2c714ba90a3d446aa0338ec759113dfdc71443d10d52bc5b36a78b8f704b9`;
Claude 2.1.319. `cargo build -p loopflow --bin lf` passed with inherited LF_*
unset. Fresh private-home CLI reads, unknown-login/provider errors and `use auto`
passed without seeded data; the native home stayed empty.

Supplemental native checks used synthetic stored accounts, scoped real Keychain
items and a loopback inference refusal with external egress denied. Both items
followed A → B → A and account-selected launches. Repeating B repaired a seeded
stale legacy item and advanced the reload marker. An actual chmod-0400 marker
made A installation and its first retry exit 1 with `Permission denied`; both
Keychain items already held A while the previous B hold remained. Restoring 0600
and repeating A succeeded and moved the hold to A. History bytes and the third
item stayed unchanged. Hold/order/release and plain-Claude selection passed.

Receipts: `.lf/tmp/loo-811-qa/{empty-cli.json,native/results.json,cleanup.json}`.
Probe: `.lf/tmp/loo-811-qa/native_recovery.py`, a copy of the existing native
fixture with these extra scenarios and command transcripts. Cleanup independently
confirmed all three scoped items absent, the temporary home removed and zero
processes referencing it. These fixture results do not complete live QA. No
production defect surfaced; no runtime code changed. Gate checks and the live
login/installed/running-client observations listed above remain outstanding.

## Publication checks — November 8

`lf sync` merged cleanly with main at `7165925e5`. Review retained the scoped
two-item repair and the live acceptance limits above. The shared test module's
`pub(crate)` visibility exposed an architecture scanner false positive: it counted
test-only `sh` and `sleep` subprocesses. The scanner now excludes visible
`#[cfg(test)]` modules; a regression also checks that real subprocesses still count.

- `scripts/test.py --reuse-passing --rust-filter 'test(provider_auth::) | test(provider_account::) | test(lf::commands::account::) | test(documented_commands::)'` with inherited LF_* unset and the candidate on PATH — Rust inventory, build, formatting, 196 selected tests, clippy with warnings denied, and 73 website tests passed. The initial architecture phase failed before the scanner repair; log: `.lf/tmp/loo-811-publish-gate.log`.
- `uv run pytest python/tests/test_architecture.py` — 24 passed after the repair.
- `uv run python scripts/check_architecture.py` — passed after the repair, 36/36 subprocess edges.
- Ruff check/format for the changed Python files and `git diff --check` — passed.

No runtime code changed during publication review. The Rust launch comment and
concept review now describe same-login reconciliation and the later native QA.
Existing native receipts remain applicable; installed and real-login acceptance
remain open. The Wave memory's statement that all native runs predate retry is
superseded by the scoped QA observations above; native Keychain rejection remains
unobserved.

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
