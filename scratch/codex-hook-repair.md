# LOO-496 — Codex context hooks

Jack Heart authorized autonomous repair, landing, release and installed acceptance.
First repair (native context binding): PR #1576, merged 2026-10-16 as 27a07643fb.
Its 2026-10-15 diagnosis: 100 failed `__context-block` calls (fantasia, etude),
all exit 1 with `No such file or directory`; the saved delivery file was missing.
This pass gets a new PR, per Jack's 2026-10-27 steer.

## Second cause: Loopflow's Codex hooks never run (2026-10-27)

Installed v0.14.18 with Codex 0.164.0: no Loopflow hook ran, and a saved steer
stayed pending. Probes against `codex app-server` (disposable CODEX_HOME,
loopback model) established:

- Hooks declared in `thread/start` config run only when that same config's
  `hooks.state` trusts their hash. Untrusted ones are skipped with no warning
  or notification.
- `hooks/list`, which backs the native `/hooks` screen, lists launch layers only
  and never shows thread-config hooks. So you can never review them: zero
  installed hooks shown, as observed in the installed acceptance.
- Thread-level `state` takes precedence over stale user-level entries at the
  same key. Jack's `~/.codex/config.toml` has such entries for
  `/<session-flags>` keys; none match any current declaration hash.
- Root-level and subcommand `-c hooks.<Event>=…` replace a wrapper's same-event
  hooks, so moving declarations to launch flags would drop host hooks.

Cause: #1531 (2026-10-15) removed the per-thread trust Loopflow gave its own
exact declarations, assuming Codex's review UI would own them. No review path
exists for thread hooks. The e2e fixture hid this by pre-trusting Loopflow's
hooks itself.

Repair: `harness::context::codex_config` trusts exactly Loopflow's declarations
on the thread, at their indices after wrapper groups. Host entries and persisted
decisions are untouched; nothing global is written. The fixture now trusts only
the wrapper, seeds stale user entries at Loopflow's IDs, and requires a steer
saved between turns to reach the model (`pending: 0`).

Limitation: the trust hash follows Codex's normalization
(codex-rs/hooks discovery, verified on 0.164.0). If a future Codex changes it,
hooks go silently untrusted again. The native e2e is the guard and must run per
Codex upgrade.

Remaining: gate, new PR, land, release by the Infrastructure Wave operator,
installed fresh+resumed acceptance in cmux, then complete the Task.

Checks: pre-fix lf + new fixture FAIL (no context in the model request); fixed lf
`tests/e2e/codex_terminal.py` plain+wrapped PASS, and PASS via
`--cmux-wrapper /Applications/cmux.app/.../cmux-codex-wrapper` (Codex 0.164.0).

Coordination (rechecked 2026-10-27): PRs #1731 (LOO-638) and #1720 (LOO-628) are open and touch none of
this change's files (harness/context.rs, context_launch_tests.rs, codex_terminal.py).
Focused: `cargo test --lib harness::context` 4/4, `--test integration context_launch_tests`
11/11, `cargo fmt --check`, `cargo clippy -p loopflow --all-targets -D warnings`, ruff PASS.
Done on the branch: 926da5a5bd (repair) plus a fixture simplification commit; the
fixture's Loopflow pre-trust and hook-receipt query are deleted. Branch is level
with origin/main (2026-10-27). Not yet published.

Concept review (2026-10-27): no change. Loopflow vouches for its own hook
declarations per thread; host hooks and saved trust are untouched, and nothing
global is written, which the Task brief requires. #1531 removed that trust as an
incidental part of LOO-462, not as a decision about hook review, so restoring it
reopens no product choice. One owner (`codex_config`) serves start and resume.
Silent untrust after a Codex hash change stays the known limitation; the native
e2e per Codex upgrade is its guard, and the PR notes should say so.

Published 2026-10-27: PR #1734 (new repair PR). First publish swept local run
instructions and scratch into the branch; removed in a follow-up commit. Delivery
commands must run with the per-command core.excludesFile from questions.md.

## Demo (2026-10-27, headless, standing in for Jack Heart)

Experience: the real native path, with branch `target/debug/lf` (HEAD a707105d02),
real Codex 0.164.0 (`~/.local/bin/codex`), and cmux's installed wrapper
(`/Applications/cmux.app/.../cmux-codex-wrapper`, dated 2026-10-05), via
`tests/e2e/codex_terminal.py`. Load average was 180 to 210.

- Plain and fixture-wrapped: PASS. Two turns were captured, the session reconnected,
  and zero live providers remained.
- Through the cmux wrapper: PASS. In both cases, all 4 model requests carried the
  Loopflow context block. `launch.txt` and `resume.txt` contain no "Hook failed"
  or "exited with code 1". The saved steer was delivered (`pending: 0`).
- False start: `--codex "$(command -v codex)"` inside cmux resolves to cmux's
  shim, so cmux's wrapper ran twice. Under load the app-server then exceeded
  lf's 15s socket wait. The cause was the harness invocation, not the repair.
  The wrapper alone opened its socket in 4.7s, compared with 0.9s for bare Codex.
  Pass the real binary to the e2e.

Decision: advance. Remaining: PR #1734 CI, land, release, then installed
fresh+resumed acceptance in a real cmux Session.

Gate (2026-10-27): tree identical to adc36034a7, so the focused checks above stand
(no rerun). Diff reviewed, no polish needed. PR #1734 CI green except
macos-process-test (macos-26) and loopflow-ui-test, still running; CI owns them.
Branch is one commit behind origin/main (#1731), which touches none of these files.
