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
IDE deep links retain their existing full-prompt/vendor-seed transport; they have
no system-file option. LOO-428's flags and Flow output are untouched.

## Reconciled evidence (2026-10-07)

The split renderers, classifier claims and unused Claude turn launcher are gone.
The active stream launcher covers large context and structured reply guidance;
full rendering and attribution tests cover the surviving system channel. Source
review found no remaining split, fallback or refusal-classifier implementation.

Terminal fixtures exercise the real CLI with argument-limited provider scripts.
The Codex headless fixture checks thread configuration, the short first turn and
the retained file after exit. It does not resume a native provider. Claude's
stream test checks file contents and arguments, not provider acceptance. These
boundaries follow Release's entry-point lesson: a helper proof cannot establish
the composed user experience. No additional code repair was identified.

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
Jack Heart has not yet supplied plan-mode results or authorized landing.
No design revision is agreed. The subsequent Codex attempt is recorded below.

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
No Codex readback or title acceptance is established. Preserve the configured
wrapper path for the eventual retry; do not bypass it to claim cmux acceptance.
Recommended next action: integrate LOO-428's flag repair when available, rebuild
the candidate, and have Jack Heart rerun the Codex command below to obtain the
marker readback and visible title. Claude plan mode can be checked independently
now. The all-context system-file design remains unchanged; landing stays withheld.

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

Jack Heart authorized publication for review, with landing withheld pending the
real first-response check.
The fixed first user message may change host/provider titles; LOO-422 owns that
broader experience. Gate/CI owns the full affected suites. Release is the only
immediate child with memory in this checkout. Its goal, memory headings,
publication/acceptance, scheduled-failure and pending-version sections were read;
the retained landing incident was not reread.

Checks: `git diff --check` and `cargo fmt --check` PASS; retained compression results reused: `cargo test -p loopflow --test context_launch_tests` (2), `cargo test -p loopflow --lib claude_stream_context` (1), `cargo test -p loopflow --lib ad_hoc_batch_launch` (1), `cargo clippy --all-targets -- -D warnings` PASS; earlier assembly/attribution/probe checks at `4475ceb04:scratch/put-all-assembled-context-back.md`; full affected suites deferred to gate/CI, real-account first response to Jack Heart.

`lf context --skill realign` reports all sources within budget; memory is
15,963/16,000 tokens. The earlier launch
snapshot included the workspace diff and exceeded the goal source budget; its
complete source remains in the existing context archive. No limit was raised.

## PR walkthrough (2026-10-07)

Jack Heart requested `pr-review` after discussing advancement. The local
[HTML walkthrough](pr-review.html) reviews published PR #1498 from
`35e759aaf4e79e7dc8eef2e8d30048f10172b45a` to
`c143359f9eb76c0d75e5da0b86b3aff8d1e27714`, tracing assembly, terminal handoff,
failure capture and persistent-provider context files. It separates the published
source from the unpublished demo evidence above. No further implementation
defect was demonstrated; the material remaining gaps are Codex's LOO-428 launch
dependency, plan mode and native resume. No Flow was advanced or merge requested.

Walkthrough checks: revision-pinned excerpts and local HTML anchors verified;
desktop/narrow captures and expanded disclosure inspected with `lf screenshot`;
product checks were not rerun for this artifact.

## Landing direction (2026-10-07)

Jack Heart requested landing after the PR walkthrough. This supersedes the
earlier hold on landing; it does not establish the unobserved acceptance checks.
Jack Heart clarified that the Task must advance through queued preparation
before landing. Run the existing `queue` Flow (compress → sync → realign → gate),
then the Task’s `ship` edge (gate → `pr land -c`) only if queue succeeds. The ship
gate should reuse still-valid checks under the repository verification cadence.
The remaining live-account checks stay unproved; they no longer hold landing. Durable findings are retained in Infrastructure memory; this
design and `scratch/pr-review.html` are checkpointed before scratch cleanup.
