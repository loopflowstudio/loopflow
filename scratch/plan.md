# LOO-816 Codex service tier

Task directive: optional typed preference, absence inherits Codex configuration;
all launches, including resume, receive it without rewriting provider files.
Later conversation changes survive observers and reattachment. Key and aliases
remain the reversible assumptions in `questions.md`; `docs/config.md` owns the
intended experience.

Implemented through the existing settings catalog and resolution, AgentConfig,
replay captures and thread `serviceTier`. Only native launches also set the
server default for `/new` and `/fork`; headless launches need no duplicate
config override. Native reconnection to a live server omits the tier. Observer
subscriptions send only the thread ID. No schema migration or replaced path.

Delete — do not maintain: no predecessor to remove; this adds a preference to
the existing launch path and regression coverage to the existing native fixture.
The old fixture-only retention probe is removed: live reattachment now owns
that check. Child-tier checks keep only `/new` and `/fork`; the fixture accepts
the surviving thread `serviceTier` path without a second config fallback.
Unset resume and child inheritance share the launch matrix. The live server's
reply plus unchanged process identity makes a separate signal-0 probe redundant.

Remaining: gate's affected suites and static checks. Affected owners: config
settings/files/remote forwarding; prompt preparation; agent Session/replay/capture;
Codex harness/observer; context_launch_tests, settings_launch_tests,
session_launch_contract, session_cli_tests and agent_startup_tests.

The live reattachment regression changes the conversation from default to priority,
then runs `lf --setting codex.service_tier=default resume`. Public resume joins
the retained terminal; input through that viewer triggers a server tier readback.
It checks unchanged server identity, endpoint, thread and history, and one server
launch. This covers public live attachment; it does not force the lower-level
CodexHarness reconnection branch. Existing observer tests check ID-only subscriptions.
Real Codex `/new`, `/fork` and conversation tier retention remain demo/review;
the stand-in models those semantics rather than verifying the provider's behavior.
The key/alias and replay assumptions in `questions.md` remain open for demo.

2026-11-09 realignment: refreshed `origin/main` remains at `28b0af568`; no newer
upstream changes or Task review comments changed the approach. No Wave is bound.

2026-11-09: installed Codex 0.167.0 generated ThreadStartParams,
ThreadResumeParams and ThreadForkParams with optional string `serviceTier` via
`codex app-server generate-json-schema --out /tmp/loo-816-codex-schema`.
An isolated app-server `config/read` returned `default` and `priority` for the
corresponding `-c service_tier` launch arguments. No provider turn was started.

`lf context`: within the configured token and byte targets after realignment.
`cargo test -p loopflow --lib codex_service_tier`: passed (2); macOS linker warned
that this test binary's `__eh_frame` exceeds compact unwind's 16 MB limit.
`cargo test -p loopflow --test integration codex_service_tier_reaches_native_and_headless_launch_and_resume`: passed after compression (1, 49s); no test providers left running.
`cargo test -p loopflow --test integration codex_service_tier_survives_live_reattachment`: passed (1, 8.30s) with a retained terminal owner; a plain pipe launch had no `/dev/tty`.
`cargo test -p loopflow --test integration codex_service_tier` (inherited `LF_*` unset, stdin closed): passed after simplification (2, 68.67s), including unset resume and child inheritance; gate owns broader suites.
`cargo fmt` and `git diff --check`: passed. Clippy and affected suites belong to gate.
`lf open --host desktop --file docs/config.md`: failed in write-docs because
“task_559f10c642144c8a86dc71b5c7728a70 has no workspace in this Desktop window;
nothing changed.” Docs remain at their path; no alternate editor opened.
