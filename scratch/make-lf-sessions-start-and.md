# LOO-428 — Codex hook coexistence

Jack Heart approved setup-free Codex coexistence and publication for review on
2026-10-07; no landing. The accepted scope preserves Flow-output changes, native
terminal behavior, wrappers and PATH, with live cmux Sessions untouched and no
real accounts in automated checks.

## Ownership and remaining acceptance

LOO-429 / [PR #1498](https://github.com/loopflowstudio/loopflow/pull/1498) owns
prompt transport and combined oversized-context acceptance. Its later decision
supersedes 428's argv-only/fixed-system constraints: all assembled context in one
native system/instructions file, a fixed 48-byte user trigger, no split, relaunch
fallback or lf-side refusal wording. Provider errors remain visible.

429 records Jack Heart's Claude interactive context readback, not plan-mode,
cmux tracking or combined-candidate acceptance. Its Codex attempt hit the trust
collision owned here. Its October 7 landing direction now permits delivery after
queued preparation despite those unobserved checks. That permission applies to
429; 428 remains publication-only. Prompt placement is settled independently of
this hook repair. Evidence: October 7 Task status and the complete sibling design at
`/Users/jack/src/loopflow.put-all-assembled-context-back/scratch/put-all-assembled-context-back.md`;
that checkout remains owned by 429 and was not edited here.

Gate owns affected suites on the candidate after integration with main. Capable
review owns launch and agent-list proof in a separate cmux window with wrappers
on PATH; the earlier access-policy denial remains valid. LOO-429 owns the combined
oversized Claude/Codex launch. Homepage capture follows landing and installation.
Source fixtures prove neither installed behavior nor actual cmux tracking.

## Terminal selector removal (Jack Heart, 2026-10-08)

Jack Heart requested removal of `--tui` after PR #1497 removed `--ide` and
`session.launch`. Terminal launch remains the sole interactive surface. A direct
TTY launches interactively by default; `-i` retains the explicit interactive
choice for piped or detached callers, and `-b` retains headless execution.
Delete `Cli.tui`, its dispatch branches and generated flag documentation. Move
internal callers and fixtures to the surviving interactive selection; preserve
native capture, reconnect, historical `tui` surface records and provider TUI code.
Reject the removed flag at a valid skill entry. Verify default TTY, explicit
interactive without a TTY, headless batch and the real terminal capture path.

## Implementation and preservation

A disposable `lf-capture-*.config.toml` profile in the effective Codex home adds
lf's SessionStart hook as an independent native layer. Existing config/trust stays
unchanged. Ordinary return/error removes the file; interruption can leave it
unselected. This is launch-local configuration, never one-time setup.

A five-second parse-only invocation of the same PATH command supplies trust and
an incomplete `--model`. Exact duplicate-trust diagnostics mean the wrapper
supplies trust; missing-model-value diagnostics mean lf must supply it. Neither
starts a Session. The provider inherits terminal descriptors. No host detector,
PATH substitution or provider relaunch. Successful exit without native capture
retains the failed Session and launch evidence, adopting no unrelated history.

Native 0.160.1 ignores config-based bypass. TUI diagnostics omit native identity.
Two CLI hook tables replace one another; profile plus CLI hooks both execute.
These findings rule out the earlier alternatives without persistent writes.

Flow output prints position/name and agent messages. `--verbose` adds token
accounting and INFO diagnostics. Prompt-bearing traces are removed. Task Started
commits with the Flow in its actual checkout; no schema change.

## Removed mechanisms

Removed: whole-table CLI capture injection, unconditional trust insertion,
`interactive_launch_diagnostic` and its argv-only fixture, `mark_task_started`,
duplicate progress printers and prompt-bearing debug dumps. Compression also
removed the hook-string helper/test and the fixture's custom TOML serializer.
Native behavior proof, exact capture fencing and transactional Started remain.

## Evidence and review limits

`tests/e2e/codex_terminal.py` uses native Codex 0.160.1, fresh provider/lf homes,
a loopback model and synthetic host hooks through lf in a headless PTY. Plain and
wrapped launches complete a turn and capture their exact native IDs; wrapped
launch executes SessionStart and Stop. Both reopen the same native conversation.
After launch, config bytes remain unchanged and temporary profiles disappear.
Native resume may save its own screen-reader setting; lf does not write it. No account/login.
Normal `/exit` avoids the original fixture's premature Ctrl-C; there is no retry.

Compression found stale hook assertions after reconnect: they reread launch
receipts. Removing those receipts proved neither hook emitted again. Direct native
resume without lf reproduces this; evidence is in
`/private/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/lf-codex-terminal-ta3b8um1`.
The fixture now checks reopened native identity and displayed history, and claims
host-hook execution only at launch. Reconnect hook emission remains unproved.

The missing-capture regression retains its Session and excludes unrelated history.
Prior Flow-output/Task-binding proofs remain applicable. Release is the only
immediate child Wave; its goal and full memory were read. Its operation-entry
lesson applies here: launch, reconnect and host tracking have separate evidence.
Launch hook receipts cannot establish fresh reconnect hooks or actual cmux tracking.
Source review found no bounded implementation mismatch requiring repair.

## Retained measurements and history

macOS `ARG_MAX` was 1,048,576; empty-env `/usr/bin/true` accepted one ASCII argument
of 1,048,529 bytes, rejected 1,048,530. This is command-specific overhead. The
cmux Claude wrapper accepted 122,880 and rejected 122,881 bytes; Codex accepted
262,144 in that experiment. No real account was used. These superseded transport
diagnostics do not constrain 429's selected file delivery.

Prompt history: 4ab46f419 (terminal argv), 8ae246c24 (native instruction files),
9f47767c6 (repo content back to user role for classifier/plan mode), 56f2cc79c
(opt-in operating guidance), 5e872f06e (persistent headless Claude stdin).
Earlier interface table and complete pre-compression notes remain in the launch
snapshot `.lf/tmp/context/036c563040dd8f5e56d2b30fd95d88df8385cd239ce93cd8058bf95c101c8955.md`.
`lf commit` excludes scratch, so that existing snapshot preserves the local notes.

Checks (2026-10-08): build/fmt/Clippy, Ruff, six focused selector/prompt tests, isolated `terminal_launch_tests` and native `codex_terminal.py` PASS; native PTYs now exercise default terminal launch/reconnect without `--tui`; generated CLI reference refreshed; gate owns affected suites, capable review owns cmux tracking, LOO-429 owns combined oversized input.
