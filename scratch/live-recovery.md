# LOO-811 live recovery

November 8, 2026. Direction in the recovery conversation: "can we get that installed so that loopflow is routing properly again" and "you can do 811".

## Remaining work

[interactive-limit-recovery.md](interactive-limit-recovery.md) owns the expanded
scope, native counterexamples and remaining preservation cases. Guarded runtime
admission is now integrated through the Session owners and broker. Two native
Task primary Sessions pass bulk recovery for both providers through Desktop's
server protocol, retaining their history/model/owners and editable drafts plus
later input. Both providers pass cancellation while the credential lock is held:
no native retry or late account change, and drafts complete on the original login.
Both providers also retain a pending approval while another Task recovers;
answering it executes its effect once. Cancellation/save-back, blocked Keychain
stdin and partial-install tests pass.
Exact commands, candidate hashes, initial fixture failures and remaining checks
are in the design. Earlier unconditional sends establish none of these guarantees.
Publication checks on main `8155d1fcd` passed after DTO fixture repairs; see
the design’s “Publication review — November 9” for commands and results. Compression reran both
native bulk cases on candidate `21dd464633268c528d094ca7ef2d3180ff195e56b05d42b9cae6b8cb6aa263b1`;
approval/cancellation receipts are from earlier candidates. Rendered Desktop,
installed and real-login checks remain open. Held-lock cancellation precedes
installation; it does not establish cancellation of an already-sent native login.

November 9 QA reran both providers' bulk, approval and held-lock cancellation
cases on merged candidate `473e1d7353201b8a6ca85633c056ac74ee29926fc608eedadb41481ab282f963`;
all six passed, with no probe processes or scoped Keychain items left. Public
CLI empty/returning-state and private-server restart checks also passed, alongside
17 headless Desktop checks. [qa.html](qa.html) records transcripts and limits;
commands and receipts are in the design's “QA — November 9, merged candidate”.
These synthetic/native and automated view checks do not establish installed
real-login recovery or physical typing after reopen. Publication review completed the broader matrix, including late/lost
Codex installation replies; configured checks remain for demo.

November 9 sync onto `8155d1fcd` kept both harness README sections. The new
observer measurement fixture needed a no-op recovery tick for `watch`.
`cargo test -p loopflow --lib harness::codex_connection::tests::task_update_wakes_an_idle_conversation_from_the_side < /dev/null`
(inherited LF_* unset) — 1 passed after that repair; existing large `__eh_frame`
linker warning. Gate owns broader checks and static analysis.

The ordinary-typing incident remains unresolved: the LOO-813 handoff reported
physical x/z invisible in Desktop and a passive owner snapshot; pasting y then
revealed xzy, and both affected conversations subsequently worked after Reset.
Those observations establish no cause. Quota/draft fixtures and the separate
installation inventory do not establish physical keyboard repair after reopen.

The implemented experience is described in `docs/sessions.md`, subscriptions
and Desktop docs; explicit restart limitations remain visible.
November 9's `lf open --host desktop --file docs/sessions.md --file
docs/subscriptions.md --file swift/README.md` reported all three opened. No demo
or approval of the expanded outcome occurred. November 8's Keychain concept
review, QA and gate cover only that repair.

Use disposable environments. No production account switches or Session restarts,
installed-app or updater changes, paid inference, manufactured exhaustion, merge
or Auto-Ship. No isolated real-login environment was available; shared production
logins had open conversations. Demo/review retains the live observations below.

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
- Real-provider Claude no-model quota behavior with account-wide usage below
  100%. Synthetic native probes and the focused routing test now cover retaining
  the rejected model's hold despite a generic healthy poll; real capacity remains
  unobserved.
- `resume --account` for running Codex and Claude, and `session move`. The earlier
  product probe observed running Claude clients reload after cache expiry and
  complete retries/drafts; current Task-bound synthetic checks pass, while
  real-login preservation remains open.
- Whether refreshing Claude invalidates earlier access tokens used by a rental.

## Keychain repair scope and check history

The two-item direction remains:
"implement a focused repair for the existing two-item expectation, with regression
coverage where lookup order changes and both items must follow every switch."
The retry direction was: "Reconcile USER plus the selected legacy item under
existing locks using the newest native credential, retaining lexical selection
and third-item scope from scratch/questions.md."

Both repairs are implemented. Keep USER plus one eligible legacy item, lexical
selection, newest-native save-back, credential/refresh locks and reload-marker
errors. No service-wide synchronization or rollback is promised. Stand-in tests
cover failed writes/retries, access-only and both-token rotation, complete repeats,
normal switches, marker recovery and unsuffixed fallback. Native checks below
cover seeded stale state and a real marker failure, not rejected Keychain writes.

Pre-curation details and exact earlier commands/results:
`f5b3539644f30b22842273bdba995077eb2bd905:scratch/live-recovery.md`, headings
“Candidate repair and remaining verification — November 8” and “Finish a partial
switch — implemented; live checks remain”. Those include the two candidate
hashes, native receipts under `.lf/tmp/loo-811-two-item-{repair,compress}/`, and
superseded pending-gate notes. Earlier concept alternatives remain in
`d4941f045600750beffbbfc73934bd4bbf9abfbb:scratch/live-recovery.md`.

Latest focused checks, November 9 (inherited LF_* unset):

- `cargo test -p loopflow --lib keychain_ < /dev/null` — built; 5 passed, existing large `__eh_frame` linker warning.
- `cargo test -p loopflow --lib lf::commands::account::account_first_tests::claude_keychain:: < /dev/null` — 3 passed; gate owns affected suites/clippy after runtime changes.
- `cargo fmt --check`, `git diff --check`, `lf context` — passed; both budgets fit.

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
production defect surfaced; no runtime code changed. The later publication gate
passed; the live login/installed/running-client observations remain outstanding.

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
remain open. Scoped QA includes same-command retry after a real marker failure;
native Keychain rejection remains unobserved.

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
