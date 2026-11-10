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

Remaining: gate's affected suites and static checks. Affected owners: config
settings/files/remote forwarding; prompt preparation; agent Session/replay/capture;
Codex harness/observer; context_launch_tests, settings_launch_tests,
session_launch_contract, session_cli_tests and agent_startup_tests.

The requested live reattachment regression is implemented and passed: it changes
the conversation from default to priority, then runs
`lf --setting codex.service_tier=default resume`. Public resume joins
the retained terminal; input through that viewer triggers a server tier readback.
It checks unchanged server identity, endpoint, thread and history, and one server
launch. This covers public live attachment; it does not force the lower-level
CodexHarness reconnection branch. Existing observer tests check ID-only subscriptions.
Real Codex `/new`, `/fork` and conversation tier retention remain demo/review;
the stand-in models those semantics rather than verifying the provider's behavior.
The key/alias and replay assumptions in `questions.md` are implementation choices,
not unresolved product choices; demo still checks the provider behavior above.

## Concept review — 2026-11-09

Outcome for loop-or-next: nothing to change and no open product choice. Keep
the current design; no next implement direction. This is a headless review
conclusion, not a recorded response from a person.

The important distinction is `default` versus unset: standard speed is a choice;
inheritance is absence. Existing usage docs explain both and normal precedence.
Keep the provider's spellings with the required friendly aliases. No new command,
skill instruction, Session tier record or continuously enforced policy is needed.

Walkthrough: set global `default`, launch, change the conversation to `priority`,
then reattach: it stays `priority`. If the provider dies, resume launches again
with the current preference and retained native history; it does not replay the
interrupted turn. This follows the requirement to apply the preference on launch.
Replay instead keeps the captured override, including absence; absence cannot
promise the same effective tier if provider configuration has since changed.

Counterexamples to keep: unsetting a repository preference can reveal a global
one; reattaching with a conflicting invocation preference must not reset the
live conversation. Neither needs another state type or precedence rule. Next show
real Codex child inheritance and tier retention, whose stand-in results do not
establish provider behavior. Review page: `scratch/concept-review.html`.

2026-11-09 second realignment: refreshed `origin/main` remains at `28b0af568`;
Task status has no new review comments and no bound Wave. The implementation
now covers the preceding feedback's public reattachment gap. Existing focused
results apply; this pass changes only the plan and assumption wording.

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

2026-11-09 QA: branch build and real CLI settings/precedence/unset checks passed.
Real Codex 0.167.0 readbacks confirmed native launch and child API inheritance,
unchanged tier/process/thread on public live reattachment, and current tier on
native and headless resume. Headless Graphs resolved standard/default,
fast/priority and unset/provider priority correctly. No feature repair needed.
Authenticated replies and typed `/new`, `/fork`, `/fast` remain unchecked:
the isolated native client stops at sign-in; headless replies return HTTP 401;
the configured Doppler project has no OpenAI credential. API probes do not
establish those terminal commands. `scratch/qa.html` holds transcripts and limits.
All private probe processes and groups were stopped; affected suites and static
checks remain with gate. `.lf/skills/qa.md` now records the working probe route.
