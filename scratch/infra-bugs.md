# Reliable provider launches

Accepted direction: Jack Heart requested commands robust across terminal
integrations, without a cmux-specific patch. Jack approved Codex implementation
on 2026-10-13, then Claude first-message investigation, OpenCode HTTP delivery
and visible startup feedback on 2026-10-14. Claude background/attach reconnect
is excluded: Jack reported that approach had already failed. Its stop-and-resume
behavior remains required.

## Remaining work

- Installed-release verification remains. The release
  must include both this branch's launch changes and stale-provider fix #1542
  (`def8881d82`). Passing branch builds do not establish installed recovery.
- OpenCode HTTP delivery is accepted but unimplemented. Its prerequisite remains
  an actual terminal rendering and answering a permission raised by the HTTP
  first turn, then completing a typed follow-up. Plugin instructions, exact
  input, native approvals, ownership, retry and history remain acceptance criteria.
- The original perceived `lf incident -i -c` hang was not reproduced and remains
  unattributed. No UI-readiness latency claim is supported.

Reconciled 2026-10-14 against fetched `origin/main` at `cf754f02cd`: the v0.13.14
release commit (`de368eda46`) already contains #1542, but not this branch's
protocol/stdin transport changes. Release source inclusion proves neither
publication nor installed recovery. Infrastructure recovery and Release
follow-through memory retain the same distinction.

## Surviving design

Codex native and headless launches share `harness/codex.rs` app-server startup,
account routing, lifeline and attachment fencing. Native argv contains only short
options and an endpoint/native ID. Instructions, hooks, roots and exact input
travel through JSON-RPC. Do not replace base instructions, impersonate an account,
overwrite user profiles, or encode whitespace for a wrapper parser.

The bootstrap connection configures the server and closes before terminal
attachment. The terminal creates/resumes the thread and owns approvals; a shared
pending input submits the captured first request after subscription. Read native
history before opening the terminal: an early resume exit must neither replay the
original request nor report an already-recorded request as unsent. Empty threads
cannot be resumed before their first turn materializes native history. Passive
resubscriptions acquire no write authority. Stale input receives an RPC rejection
while display continues.

Remote Codex rejects `--no-daemon` and `--add-dir`. Workspace roots travel in
provider configuration, preserving configured roots. Client close/reset ends the
relay normally; provider failures and nonzero terminal exits still fail. Failed
startup retains the original captured input, with one owner for provider shutdown
and settlement. The foreground readiness timeout explicitly kills and reaps its
launcher; missing exit evidence never means fabricated success.
Provider and relay sockets use the caller's temporary root. Gate reproduced
native Codex failing inside an isolated clone because `/tmp/lf-codex-*` was
outside its sandbox; honoring `TMPDIR` preserves that sandbox without widening it.

Claude supplies initial input through an anonymous temporary stdin file, avoiding
wrapper parsing and pipe-buffer blocking; the native client reopens the controlling
terminal for keyboard input. System-prompt files and stop/resume are unchanged.
OpenCode retains argv, its argument cap and native stop/resume.

Startup feedback is terminal-only, shows stages and elapsed time, clears on
failure and joins its writer before native rendering. Redirected output stays clean.

## Delete — do not maintain

Removed: `build_codex_command`, `context::codex_args`, inline TOML encoding,
SessionStart identity-hook injection, Claude prompt argv, and their exclusive
argv tests. Protocol replies establish Codex native identity; Claude retains its
supplied native ID and OpenCode observes its startup log.

This reduction also removes unused `agent::codex_permission_args` and its two
helpers, `Commands::ProviderSession`, `observe_provider_session`, and the obsolete
hook-only `terminal_launch_tests.rs`. Permission assertions now exercise protocol
parameters; failed-start/history preservation lives in the existing startup test.
Linear Flow fixtures and native launch/resume tests share `native_codex.py` through
one installer. No pending deletion target remains.

The async Codex worker is separate from the invoking-thread terminal driver.
Native skill expansion happens once at their shared launch boundary. The worker
reuses the admitted attachment rather than reopening the store to validate it twice.
Review found the retired callback had only test callers and an old failure test
still required its removed diagnostic; both consumers moved with the deletion.

Updating the native PTY probe exposed a real startup regression: replacing the
client's entire thread request discarded native tools, UI settings (including the
wrapper's explicit `bypass_hook_trust`) and interactive approval policy. Startup
now overlays the prepared instructions/configuration onto the native request;
passive subscriptions still discard overrides. The shared headless permission
default is omitted for ordinary native launches. One provider config read retains
writable roots and the original session-flags hook declarations. Host hooks keep
their indices and trust state; only appended Loopflow hooks receive new hashes.
The provider trace also exposed duplicate shell-policy `-c` flags after
`app-server`: Codex's parser dropped the wrapper's earlier global overrides.
Those flags are removed; the same shell policy already travels with each thread.
The probe now uses supported flags and permits native UI onboarding preferences
while asserting every supplied user setting survives unchanged. Exact hook trust
also requires omitting absent matchers from the normalized hash; hashing null
marks matcherless hooks modified. The actual provider's `hooks/list` confirmed
that normalization. The probe trusts only its two declared hooks by hash.
A blanket hook-bypass flag alone still prompted for host-hook review on reconnect
in the isolated Codex 0.162.1 probe; that variant is not verified as recovered.

## Acceptance evidence and limits

Full dated investigation and previous checks are preserved at
`5cbabcb81a:scratch/infra-bugs.md` and
`5cbabcb81a:scratch/cmux-session-reconnect.md`.

- Codex 0.162.1 through the unmodified cmux wrapper accepted 1,100,000 instruction
  bytes over app-server with 94-byte argv (initialization 0.061s, thread 0.070s),
  without credentials or a model turn. This established transport only.
- A real PTY/account probe then answered the initial request in 5.96s and a typed
  reconnect follow-up in 8.57s, both exit 0. It retained one Session, two completed
  turns/outputs, successful hooks and zero live providers; argv was 125 bytes.
  Pane/title effects were disabled. Evidence: system-temporary
  `lf-native-transport-smoke-4cjkwi1d`.
- Actual cmux launch and resume subsequently passed: native thread
  `01a13ae2-5f99-7943-ae9f-99fe8457a2a6` returned newly typed
  `CMUX_VERIFIED_RESUME_3`; fresh launch returned `CMUX_LAUNCH_OK`; Ctrl-D exited
  successfully with zero live providers. Evidence: system-temporary
  `lf-cmux-launch-hagz2wwe`. The earlier 0.97-second readiness estimate is invalid:
  it matched old screen text and exited before subscription. These are branch-build
  results, not installed-release recovery.
- Claude 2.1.299 consumed exact 440,000-byte stdin and a typed follow-up in a real
  terminal with an isolated home and local stand-in API. Evidence:
  `/tmp/lf-claude-input-probe.py`, system-temporary `lf-claude-input-skgpq3t8`.
  Canonicalizing the trust directory fixed a symlink fixture error; title
  generation was distinguished from the initial conversation request.
- OpenCode 1.18.33 stdin accepted initial input but not typed follow-up. HTTP
  server/attach later preserved both, yet the actual permission probe left a
  pending `bash` request in `/permission` while the UI showed it running without
  an approval dialog. Fixture approval success was misleading. No permission
  was approved by the harness. Early session selection also failed to navigate;
  health is not UI subscription. No sleeps or speculative API changes resolve it.
  The exploratory implementation was removed. Retained diff:
  `/tmp/lf-opencode-native-approval-investigation.patch`; scripts:
  `/tmp/lf-opencode-{attach,lf,permission}-probe.py`; permission logs:
  system-temporary `lf-opencode-permission-{ha20prq8,ij_23gsa,re_agvj5}`.
  The attach UI/API mismatch is suspected; a guessed v2 query's HTTP 400 proves
  nothing about that explanation.

Gate review found that the native probe's reconnect assertion could pass by
displaying the previous answer. It now types a follow-up, requires a distinct
answer, checks exactly two retained completed turns and verifies zero live
providers, retaining host-hook trust and supplied user settings. Both plain and
wrapped Codex 0.162.1 passed after the socket-directory repair; evidence:
system-temporary `lf-codex-terminal-epwobl8d`. History, usage and doctor also read
the earlier disposable `lf-codex-terminal-hmgzvjju` store successfully; doctor's
binary-freshness comparison remained unavailable in the temporary source checkout.
The README now describes protocol/stdin delivery instead of a shared
instructions-file path.

The full gate found Claude Flow and bare-conversation fixtures still reading
first-turn argv. They now read stdin; the Flow fixture reopens a controlling PTY
for typed input, preserving wait/interrupt checks. The foreground quietness and
isolated-clone fixtures now use the shared native Codex protocol stand-in instead
of a non-protocol holding script or the retired identity callback.
The PTY fixture drains terminal echo and closes the command builder's duplicate
slave descriptor so macOS can finish session-leader teardown. Its four input and
interruption scenarios pass in 8.6s. All four clone-isolation tests pass with the
caller-selected temporary root.

The broad Rust gate ran 2,590 tests: 2,587 passed, three stale fixtures failed,
and 22 tests were skipped. The repaired and affected selection passed all 167
tests after resolving the PTY teardown. No leaked-output warning recurred.

`scripts/test.py --reuse-passing` stopped before product checks when its resource
preflight exceeded 60s. Separate inventory isolated a five-second uv-cache
measurement timeout; direct `du` completed, and free disk stayed above 57 GiB.
The planned checks ran through the runner's bounded `_run_command` entrypoint,
with four workers, network isolation and disk monitoring. There is no reusable
aggregate runner pass. Logs: `/tmp/infra-bugs-{checks,repairs,finish}.log`.

Checks (2026-10-14): `git diff --check`, `cargo fmt --all -- --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings`, `uv run python scripts/check_architecture.py`, `uv run python scripts/check_rust_test_inventory.py`, and `uv run ruff check tests/e2e/codex_terminal.py rust/loopflow/tests/support/native_codex.py` — passed; materialized `cargo nextest run --all` — broad results and all repaired/affected cases as above, exact filtered commands in the gate logs; materialized `cargo test -p loopflow --test integration --jobs 4 data_clones:: -- --test-threads 4` — 4 passed; `uv run pytest python/tests/test_loopflow_skill_alignment.py` under network isolation — 4 passed; `cd website && uv run python dev.py test` — 75 passed; `tests/e2e/codex_terminal.py` via `scripts/test_network.py` with `target/materialized-tests/debug/lf` and native Codex — plain/wrapped typed reconnect passed; disposable `lf history list --json`, `lf usage --json`, `lf doctor --json` — passed with the documented freshness warning. Installed-release verification remains delivery-owned; OpenCode still needs actual-terminal permission acceptance.
