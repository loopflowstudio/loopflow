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

The review found that changing only assembly would leave the actual terminal
launcher passing the full prompt as an argument, and changing only terminal
launch would move large headless Claude context into another argument. Both
paths are covered. Context files remain in the existing prompt-log directory so
native resume can still read them after the driver ends. Capture attribution,
reference escaping, source order and native permission settings remain intact.
IDE deep links retain their existing full-prompt/vendor-seed transport; they have
no system-file option. LOO-428's flags and Flow output are untouched.

## Delete — do not maintain

Deleted the Claude-specific system-safe/content split formatters and classifier
claims. Full rendering and source attribution tests cover the surviving system
channel. No layout flag, provider retry or refusal classifier was added.

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
file. Native first-response acceptance and generated titles remain unobserved.

## Remaining acceptance

Jack Heart runs or approves these commands from this checkout inside cmux, with
its wrappers still on PATH:

```sh
uv run --no-project python scripts/check_context_launch.py --agent claude
uv run --no-project python scripts/check_context_launch.py --agent claude --plan
uv run --no-project python scripts/check_context_launch.py --agent codex
```

The script preserves native account/host configuration and creates a disposable
Loopflow Machine and Git fixture containing the real Infrastructure memory,
goal and a large reference. It prints the expected marker; compare the first
response, native plan mode and host/provider titles. `--prepare-only` writes the
fixture and prints the command without invoking a provider. Its assembled preview
was 174,826 bytes. No installed-store migration or installation is involved.

Publish for review and stop. Landing requires the real first-response check.
The fixed first user message may change host/provider titles; LOO-422 owns that
broader experience. Gate/CI owns the full affected suites. Release child memory
was reviewed by headings and its publication/acceptance sections; unrelated
release-incident sections were not reread.

Checks: `cargo test -p loopflow` focused context-launch (2), assembly/budget (103), launch/attribution (41) and Wave-goal (1) tests PASS; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, Ruff and docs alignment (4) PASS; manual probe preparation/preview PASS; real-account first response deferred to Jack Heart.

Authored context is within budget (memory 15,947/16,000 tokens, scratch under
1,000/12,000 at reconciliation). `lf context` also reports the generated Task
seed above its 16,000-token source budget while it includes this PR's workspace
diff; the existing bounding path retains its complete source. No limit was raised.
