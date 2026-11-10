# LOO-816 Codex service tier

Task directive: optional typed preference, absence inherits Codex configuration;
all launches, including resume, receive it without rewriting provider files.
Later conversation changes survive observers and reattachment. Key and aliases
remain the reversible assumptions in `questions.md`; `docs/config.md` owns the
intended experience.

Use the existing settings catalog and resolution, carry the typed choice through
AgentConfig and replay captures, and apply it through thread `serviceTier`.
Only native launches also set the server default for `/new` and `/fork`;
headless launches need no duplicate config override. Reconnecting to a live
server sends no tier. Observer subscriptions stay free of settings.

Delete — do not maintain: no replaced production path. Retain the existing
serviceTier-omission test as the unset case. No remaining deletion targets.

Remaining: gate's affected suites and static checks. Affected owners: config
settings/files/remote forwarding; prompt preparation; agent Session/replay/capture;
Codex harness/observer; context_launch_tests, settings_launch_tests,
session_launch_contract, session_cli_tests and agent_startup_tests. No schema
migration. Real-provider conversation behavior remains demo/review; launch tests
use an isolated protocol/terminal stand-in.

2026-11-09: installed Codex 0.167.0 generated ThreadStartParams,
ThreadResumeParams and ThreadForkParams with optional string `serviceTier` via
`codex app-server generate-json-schema --out /tmp/loo-816-codex-schema`.
An isolated app-server `config/read` returned `default` and `priority` for the
corresponding `-c service_tier` launch arguments. No provider turn was started.

`lf context`: within the configured token and byte targets.
`cargo test -p loopflow --lib codex_service_tier`: passed (2); macOS linker warned
that this test binary's `__eh_frame` exceeds compact unwind's 16 MB limit.
`cargo test -p loopflow --test integration codex_service_tier_reaches_native_and_headless_launch_and_resume`: passed after compression (1, 49s); no test providers left running.
`cargo fmt` and `git diff --check`: passed. Clippy and affected suites belong to gate.
`lf open --host desktop --file docs/config.md`: failed in write-docs because
“task_559f10c642144c8a86dc71b5c7728a70 has no workspace in this Desktop window;
nothing changed.” Docs remain at their path; no alternate editor opened.
