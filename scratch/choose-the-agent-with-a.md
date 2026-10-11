# Global agent selection (LOO-440)

Jack Heart requested one global `-a` / `--agent` declaration on October 8,
2026. His later steer gives an explicit selection precedence over Task, Flow,
step and skill agents, including nested child commands. Without the flag,
existing definition/default selection remains. Jack Heart approved the demo and
shipping on October 10: gate, landing and completion after verified merge. No
logins, credential reads or real-provider tests are authorized.

## Baseline

Main baseline `86d0e5e6a` builds. All six supplied command forms reach semantic
dispatch in a fresh fixture Machine; none misreads the agent as a skill. Unknown
Task/repository/Flow diagnostics are expected in that empty fixture and prove
parsing only. Root help silently ignores selection. Source shows saved Task
agent precedence and no inherited override for agent-issued child commands.

## Reconciled implementation (2026-10-08)

One Clap global replaces PR-local declarations and precedence. Generic argument
partitioning still serves external skills and preserves literal `--` arguments;
duplicate parser tests and the separate before/after lists are removed.
Help and the generated reference describe the global override once.

`LF_AGENT_OVERRIDE` carries explicit selection before prompt preparation and
capture. Both prepared and direct provider launches apply it; Task/Flow defaults
never become inherited overrides. Independent Session shells clear it. Remote
transport forwards inherited selection; a top-level remote flag stays in argv.
Replay changes only its new capture. Native resume retains its provider and
permits a model override, rejecting a provider change before launch.

Compression keeps one partition of global flags and skill arguments. The public
CLI fixture asserts the exact six Claude Sessions across Task, nested Flow and
provider-child launches, including a child that explicitly requests Codex.
Separate no-flag launches retain skill/step models. Native-history refusal keeps
all six Sessions. Synthetic accounts use `--isolate`; an earlier shared activation
failed at a macOS Keychain write without using real providers or logins.

Main's startup changes from `497885050` are integrated. They remove provider
probes and reuse resolved directories; the override does not restore those
probes or require another resolver. Release's child memory was read in full:
its operation-entry lesson supports checking recorded Sessions through the
public CLI, rather than treating successful parsing as launch acceptance.

## Remaining work

Jack Heart accepted the demo and shipping on October 10. Hosted CI, remote
execution, installed/live-provider acceptance and delivery remain unproved;
real-provider tests remain prohibited. Gate does not land or complete the Task.

The remaining product failure is the unchanged homepage's desktop overflow:
`test_page_fits_and_terminals_scroll_within_their_cells[chromium-1440]` measures
1,475 px at a 1,440 px viewport. A standalone headless rendering of the existing
homepage HTML/CSS reproduces it; the rotating headline's `white-space: nowrap`
(`website/static/style.css:1400`) overflows with fallback fonts. Homepage layout
is outside this branch's gate scope. The nested Python sandbox check remains
CI-owned; its unchanged standalone network-denying proof previously passed.

## Main reconciliation and gate (2026-10-11)

Merged pinned main `21dae527b4`; repair checkpoint `894b435823` reconciles the
agent/config/prompt modules, native attachment, conversation context and the
fixture's explicit Project workflow. Review retains one global declaration and
one inherited explicit override. The six-Session acceptance, no-flag defaults,
literal arguments and native-history refusal pass.

Gate reproduced the earlier Claude verification failure using only a synthetic
ambient login: native observation consumed the managed fixture's HTTP responses.
The fixture now pins an empty native home and keeps managed login locks below
its own root. Replay's second invocation also lacked the first invocation's
ledger pin after main reconciliation; both now use the fixture store.

Four checkout tests exposed a read-only pruning query that treated every
unfinished Task as having a checkout. It now excludes NULL worktrees, with the
existing ownership test covering planning-only Tasks while retaining allocated
unfinished checkouts. These are the concrete simulated-review findings; no
agent-selection API or production locking change was needed.

The earlier release-lock failure did not reproduce in 30 isolated runs, the
materialized suite, or final focused verification. The assertion now preserves
the acquisition error if it recurs. This is a current pass, not an established
cause or repair of the earlier failure. Historical gate evidence remains at
`03cf138188` and `.lf/tmp/gate/run-25073/`; current full-run logs are under
`.lf/tmp/gate/run-13647/`.

Checks: `uv run python scripts/test.py --base 21dae527b4 --reuse-passing` (LF authority cleared, disposable provider homes) passed architecture/fmt/Clippy; Rust 2,396 passed/1 replay failure/18 skipped, Python 403 passed/5 failed, website 74 passed/1 overflow failure; final `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo build -p loopflow --bin lf` and network-isolated nextest for replay, cached identity, Task ownership and release lock passed (4); network-isolated `pytest python/tests/test_checkout_refresh.py -k "worktree_refresh_preserves_local_main or worktree_refresh_reports_fetch_failure_then_recovers or worktree_plans_leave_checkout_and_remote_refs_unchanged"` passed (7); nested sandbox coverage remains CI-owned; `git diff --check` passed.
