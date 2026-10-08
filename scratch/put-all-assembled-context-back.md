# Put all assembled context in the system file

Jack Heart approved implementation and publication for review on 2026-10-07
(LOO-429). The final three steers supersede the original split, retry and error
wording proposals: all assembled content goes in the system file, with one path.
Claude's historical refusal was a first-response error inside its terminal.

## Design and review findings

Canonical Process preparation renders the full prompt, including skill and Task
brief, as system context. A fixed 48-byte user turn starts execution. Terminal
launches pass Claude's append-system-prompt-file or Codex's model_instructions_file.
Persistent Claude and Codex harnesses use the same file-backed placement; Codex
thread start/resume receives the file through its existing config object.

Terminal, batch and persistent launches share `write_system_prompt_file` in
`engine/agent.rs`, including structured reply guidance and empty-context handling.
Files stay in the prompt-log directory for native resume. Capture attribution,
reference escaping, source order and native permission settings remain intact.
Main's LOO-420 integration removes IDE deep links and keeps terminal/headless
execution. The launch fixtures use its renamed `-a` agent selector. LOO-428's
flags and Flow output are untouched.

## Completed reduction

Removed `format_claude_system_prompt`, `format_claude_task_prompt` and
`build_claude_session_turn_args` in the earlier implementation. Queue compression
also removed `PromptFormatMode`, `format_context_prompt`, `format_task_prompt`,
their exclusive tests and the single-use `format_reference_sections` wrapper.
Repository-wide caller inspection found only full rendering in production.
`format_prompt` now takes just the components; escaping, source order and full
rendering checks survive. No remaining deletion targets were identified.

## Reconciled evidence (2026-10-08)

The split renderers, classifier claims and unused Claude turn launcher are gone.
The active stream launcher covers large context and structured reply guidance;
full rendering and attribution tests cover the surviving system channel. The queue
pass removed the unused partial-render APIs as well; no fallback or refusal
classifier remains.

Terminal fixtures exercise the real CLI with argument-limited provider scripts.
The Codex headless fixture checks thread configuration, the short first turn and
the retained file after exit. It does not resume a native provider. Claude's
stream test checks file contents and arguments, not provider acceptance. These
boundaries follow Release's entry-point lesson: a helper proof cannot establish
the composed user experience. No additional code repair was identified. Source
inspection after sync confirms the shared writer and full renderer remain in use.
The adjacent CLI documentation's obsolete IDE-launch claim is removed.

## Measurements

October 7 retained captures supplied these old user-prompt byte counts. The new
assembled launch sends a 48-byte trigger. These are recorded payload comparisons,
not observed cmux launches; old terminal launches also included operating context.

| Task / skill | Old user prompt | Capture key |
|---|---:|---|
| LOO-428 / realign | 145,625 | f8c733a6de5647cab23510f6c1064965 |
| LOO-418 / compress | 128,272 | 5a6c4cc1e5954f1288654ed4ebb3859e |
| LOO-429 / implement | 94,657 | e330f0cb6d8045bc9d5826188e47828a |

Each capture is at `~/.lf/runs/<first-two-key-characters>/<key>/manifest.json`.
Read-only measurement used `exec.task_prompt` UTF-8 bytes. The regression's
providers reject arguments at 122,880 bytes and read the larger context from the
file. Claude's interactive first response and visible title are now observed below;
plan mode and Codex remain unobserved.

## Remaining acceptance

### Demo preparation (2026-10-07)

The candidate at `c143359f9` built successfully. The provider-free preparation
command below created a fixture and printed its marker and native launch command:

```sh
uv run --no-project python scripts/check_context_launch.py --agent claude --prepare-only
```

Evidence is retained at
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/lf-context-check-sg4eoswl`.
This establishes fixture preparation only; the agent invoked no real provider.

### Claude interactive result (2026-10-07)

Jack Heart supplied `~/Desktop/Screenshot 2026-10-07 at 10.56.50 PM.png`
and asked whether it showed the intended result. Claude Code v2.1.294 returned
marker `30b57325421a3963faafcfbd`, provider name OpenCode, and the first memory
heading, “Program Status direction (LOO-398, 2026-10-07).” Read-only inspection
confirmed the marker matches the retained fixture at
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/lf-context-check-ro3s07wd/repo/probe.md`.
The visible first user turn is the short context trigger; the response shows no
refusal. The host and tab display “Supplied context instructions.”

The agent assesses this as successful Claude interactive context readback.
The screenshot shows “auto mode on,” so it does not establish plan-mode behavior.
Plan mode remains unverified. Jack Heart later authorized landing despite the
remaining observations; the landing direction below governs.

### Codex interactive blocker (2026-10-07)

Jack Heart ran the Codex check and supplied terminal output. Assembly reported
28,325 tokens; the expected marker was `62122b947caa37de1d9184d1`, with the
fixture retained at
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/lf-context-check-nj38lms_`.
Codex rejected its arguments before a first response:

```text
error: the argument '--dangerously-bypass-hook-trust' cannot be used multiple times
Error: session launcher exited with status exit status: 2
```

This matches LOO-428's assigned duplicate-Codex-flag scope. The output establishes
an argument-parsing failure, not a system-context refusal or successful delivery.
No Codex readback or title acceptance is established. The eventual retry needs
LOO-428's flag repair, a rebuilt candidate and Jack Heart's configured cmux wrapper
path to establish marker readback and visible title. Claude plan mode is an
independent observation. The all-context system-file design remains unchanged.

The outstanding acceptance belongs to Jack Heart: a candidate build followed by
these real-account checks from this checkout inside cmux, with its wrappers on
PATH. Only Jack Heart's execution or approval authorizes the provider calls.

```sh
cargo build -p loopflow --bin lf
uv run --no-project python scripts/check_context_launch.py --agent claude
uv run --no-project python scripts/check_context_launch.py --agent claude --plan
uv run --no-project python scripts/check_context_launch.py --agent codex
```

The script preserves native account/host configuration and creates a disposable
Loopflow Machine and Git fixture containing the real Infrastructure memory,
goal and a large reference. Acceptance compares the first response against the
printed marker and observes native plan mode and host/provider titles.
`--prepare-only` writes the fixture and prints the command without invoking a
provider. Its assembled preview
was 174,826 bytes. No installed-store migration or installation is involved.

The fixed first user message may change host/provider titles; LOO-422 owns that
broader experience. Gate/CI owns the full affected suites. Release is the only
immediate child with memory in this checkout. Its complete goal and memory were
read on October 8; operation-entry proofs and preservation of original failure
evidence remain applicable and already live in Infrastructure memory.

Compress's focused test, format and Clippy passes remain at
`2d333d18c:scratch/put-all-assembled-context-back.md`; earlier transport and
attribution results remain at `7611c2866:scratch/put-all-assembled-context-back.md`.

The sync checkpoint `752e2bc5a` records a passing terminal fixture for both
providers against main `873f40e30`. The final sync to `812d8cc55` succeeded at
`2d333d18c`; no new product checks are claimed for that later merge.

## Gate findings (2026-10-08)

The affected gate found eight stale fixture assumptions: bare-session and Codex
Flow checks inspected argv/user turns, and the OpenCode stand-in ignored its
system field. Fixtures now consume the supplied system context while retaining
Task direction, preceding-step evidence, steer freshness, structured decision,
retry and history assertions. No production repair was needed. Review found
no remaining split renderer or new operational abstraction.

Checks (gate): `uv run python scripts/test.py --base 812d8cc55 --reuse-passing` — architecture PASS, website 76 PASS, Rust 2,262 PASS/8 fixture failures/17 skipped, Python 406 PASS/1 nested-sandbox denial; materialized network-isolated `cargo nextest run -p loopflow --test default_conversation_tests --test flow_tests --test session_lifecycle_tests --no-fail-fast --build-jobs 4 --test-threads 4` after fixture repairs — 47 PASS/1 skipped; direct `uv run pytest python/tests/test_desktop_performance.py::test_checkout_observation_preserves_read_boundary_and_detects_changes -q` using its own network-denying sandbox — PASS. `cargo fmt --all -- --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings`, `git diff --check`, and `lf context --skill gate` — PASS (all context sources within budget). Original gate receipt: `.lf/tmp/gate/run-17487`; full final-tree matrix remains CI-owned.

## PR walkthrough (2026-10-07)

Jack Heart requested `pr-review` after discussing advancement. The local
[HTML walkthrough](pr-review.html) reviews published PR #1498 from
`35e759aaf4e79e7dc8eef2e8d30048f10172b45a` to
`c143359f9eb76c0d75e5da0b86b3aff8d1e27714`, tracing assembly, terminal handoff,
failure capture and persistent-provider context files. It separates the published
source from the later demo evidence above. No further implementation
defect was demonstrated; the material remaining gaps are Codex's LOO-428 launch
dependency, plan mode and native resume. No Flow was advanced or merge requested.

Walkthrough checks: revision-pinned excerpts and local HTML anchors verified;
desktop/narrow captures and expanded disclosure inspected with `lf screenshot`;
product checks were not rerun for this artifact.

## Landing direction (reconciled 2026-10-08)

Jack Heart requested landing and Task completion after the PR walkthrough. This
supersedes the earlier hold; it does not establish the unobserved acceptance checks.
Jack Heart clarified that the Task must advance through queued preparation
before landing. Queue Flow `c73755d7-02ce-45cc-855a-b8105cd4979b` completed
compress and its checks, then stopped at sync because its tracked scratch note
remained dirty. The note is committed at `752e2bc5a`, and sync succeeded. Jack's
October 8 continuation places this reconciliation before the caller's Task
`ship` edge (gate → `pr land -c`). Affected verification and delivery remain;
compress is complete. Still-valid checks follow the repository reuse cadence.
The remaining live-account checks stay unproved; they no longer hold landing.
Durable findings are retained in Infrastructure memory; this
design, `scratch/pr-review.html` and its five captures are preserved at
`7611c2866` before scratch cleanup. That checkpoint also retains the exact earlier
demo notes and superseded landing holds.
