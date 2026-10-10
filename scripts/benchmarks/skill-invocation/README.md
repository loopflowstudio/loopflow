# Installed skill launch checks

```sh
uv run python scripts/benchmarks/skill-invocation/launch.py \
  --fetch --fixtures /tmp/lf-skill-fixtures
uv run python scripts/test_network.py uv run --no-sync python \
  scripts/benchmarks/skill-invocation/launch.py --lf target/debug/lf \
  --fixtures /tmp/lf-skill-fixtures --source internal-comms --provider codex
uv run python scripts/test_network.py uv run --no-sync python \
  scripts/benchmarks/skill-invocation/request_mapping.py --lf target/debug/lf --flow
uv run pytest scripts/benchmarks/skill-invocation/test_request_mapping.py -q
uv run python scripts/test_network.py uv run --no-sync python \
  scripts/benchmarks/skill-invocation/launch.py --fixtures /tmp/lf-skill-fixtures \
  --source internal-comms --provider claude --terminal --warm-machine \
  --output /tmp/lf-skill-comparison
```

Fetch immutable third-party bundles before network containment. `launch.py` copies
them unchanged into a fresh workspace without `.lf` configuration, runs ordinary
lf commands with default Loopflow context, and makes the real provider read a
bundled file. The local fake API receives the resulting tool output. Provider
homes, Machine directories and credentials are isolated; external egress is denied.

Sources are pinned in `launch.py`:

- Anthropic [internal-comms at 683bc88e](https://github.com/anthropics/skills/tree/683bc88e56f3e09ba94f7055977f3d3aa499f202/skills/internal-comms).
- OpenAI [skill-installer at 49f948fa](https://github.com/openai/skills/tree/49f948faa9258a0c61caceaf225e179651397431/skills/.system/skill-installer).

Use either source with `--provider claude` or `--provider codex`. `--terminal`
exercises lf's terminal command builder, substituting provider print/exec mode
for unattended execution. It proves neither terminal rendering nor interactive
behavior. Claude terminal exercises native expansion with separate gathered user
context. `--flow` removes the source
file after capture and checks that its instructions still reach the provider.

Matching-provider cases run lf then a plain native skill serially, with identical
context text and freshly restored provider homes. `--warm-machine` initializes the
disposable Loopflow store before timing; omit it to include first-use setup.
JSON separates provider startup, first request and time after the last request
from total duration. `--output` saves fake-API requests and debug traces for
attribution. Both launches use the same timestamping wrapper. Run comparisons
without competing builds or benchmarks. JSON also reports request size and checks
exact quoted/multiline arguments,
user-only context, source retention and actual asset reads. Request size is not
a token measurement; local fake-API latency is not real API latency. Equal request
counts do not establish a blanket performance improvement. Both the lf case and
plain baseline must expand the source, preserve exact arguments and return the
bundled asset through a provider tool. `--folder .codex/skills` exercises a source
outside the automatically discovered catalog. `--custom-prompt` checks a synthetic
Codex prompt's one-based positions and named arguments against the same API.

`request_mapping.py` uses a synthetic Claude skill to test native argument parsing,
model declaration application, unknown-declaration reporting and separate user
context. `--terminal --command` exercises a single-file command with a competing
personal skill; JSON-decoded context must retain literal argument and shell syntax. `--flow` removes its
source after capture and adds a same-name collision. Manifest bytes prove captured
source retention. Its reference-file assertion is filesystem reachability only;
`launch.py` supplies the actual provider tool read.

Observed client boundaries: Claude 2.1.294 sends hook `additionalContext` in the
API system field, including UserPromptSubmit hooks. Codex 0.160.1 ignores a native
skill input whose path is outside its discovered catalog. Neither can be accepted
from successful exit or assistant text alone. Explicit Markdown skill links load
Codex sources outside that catalog. Native Claude terminal snapshots encode context
as a JSON literal in the skill's user message, preserving declarations and arguments.
A shell preprocessing directive is insufficient: Claude 2.1.294 rewrites it as an
instruction to call a tool, leaving the first request without its context.

Earlier queue, engine-restart and PTY/inbox probes are archived at `1e4ae02a5`;
provider-only `probe.py`, `continuity.py` and their exclusive tests at `31e0640eb`.
Exact argument checks read the provider's model request directly.
Baseline reconnect and plain-provider continuity remain in
`tests/e2e/codex_connect.py` and `tests/e2e/claude_shared_home.py`.

## Startup comparison (2026-10-08)

Jack Heart requested comparison with regular LF before landing PR #1504.
Three alternating pairs per harness compare PR base `35bb84ef1496` with
candidate `510bf4abbc95`, using the same debug profile, initialized disposable
Machines, pinned bundles and a local fake API with external egress denied.

| Median seconds | Base | Candidate |
| --- | ---: | ---: |
| Claude terminal, before provider start | 0.772 | 0.628 |
| Claude terminal, first model request | 0.995 | 0.866 |
| Codex headless, before provider start | 0.717 | 0.672 |
| Codex headless, first model request | 0.803 | 0.746 |

All twelve launches delivered source, arguments, user context and a bundled
asset read. Base cannot discover `.claude/skills`, so both revisions also received
the unchanged Claude source in `.claude/commands`. Base failed its post-provider
Git status check in the ordinary-folder fixture; candidate exited successfully.
These are startup comparisons, not successful end-to-end baseline timings.
The small sample shows no added startup second relative to regular LF; it proves
neither production latency nor parity with a slash command in a running terminal.
Raw samples, exact-source archive and binary hashes remain in
`/tmp/loo420-base-comparison` on the measuring machine.

Separate equal-context comparisons with plain native launches retained 875 extra
request JSON bytes for Claude terminal and 122 for Codex headless. Warm-Machine
samples added 0.988/0.702 seconds respectively, with 0.606/0.626 seconds before
provider startup. Request bytes are not token counts. Removing the executable
version probe reduced one Claude launch-preparation trace from 288 to 37 ms;
removing repeated budget counts established no overall latency improvement.
The remaining cost buys context, durable capture and launch ownership; it remains
measured overhead, not evidence of a blanket performance improvement.

Gate on October 8 exercised both pinned sources on both harnesses and surfaces,
including actual asset reads, exact arguments and user-only gathered context.
All eight cases pass after fixing Codex terminal's missing `--` prompt separator:
translated YAML frontmatter had been parsed as an option. Captured-source removal,
native model/declaration mapping, native-home continuity and Codex reconnect with
stale-client rejection also pass. Print/exec substitutes still do not prove terminal
rendering or live model compliance.

## Context delivery prerequisites

```sh
uv run python scripts/test_network.py uv run --no-sync python \
  scripts/benchmarks/skill-invocation/context_delivery.py \
  --provider claude --output /tmp/lf-context-proof
# Repeat with --provider codex.
uv run pytest scripts/benchmarks/skill-invocation/test_context_delivery.py -q
```

Checks native model-request channels across manual compaction. Disposable Homes,
loopback fake APIs and fixture-only hook trust keep real accounts untouched.
Request bodies, hook receipts and results remain under the output directory.
Claude excludes resume hooks so they cannot impersonate compact refresh. Codex
checks both that PostCompact ran and that only SessionStart supplied fresh context.
These probes do not launch Loopflow or prove automatic compaction, terminal UI,
large-input transport, OpenCode behavior or the completed LOO-444 cutover.

On October 9, Claude 2.1.295 delivered startup/compact context as user content,
contrary to the 2.1.294 observation above. Codex 0.161.0 delivered SessionStart
context as developer conversation content and ignored PostCompact context;
additive developer instructions and its native base survived manual compaction.
The app-server probe uses exact per-thread fixture hook hashes: the TUI bypass
flag did not authorize app-server hooks. Never substitute global trust changes.

### Hook-size boundary

```sh
uv run python scripts/test_network.py uv run --no-sync python \
  scripts/benchmarks/skill-invocation/context_delivery.py \
  --provider codex --context-chars 10000 --output /tmp/lf-hook-size-proof
# Repeat with --provider claude, and with --unicode on both (expected failures).
```

Claude 2.1.295 and Codex 0.161.0 preserve 10,000 ASCII characters at start and
manual compaction. Both spill and truncate 10,000 Unicode scalars / 39,901 UTF-8
bytes. Codex retains the boundary markers around a cut in the middle; compare
whole strings. A 2,500-scalar / 9,901-byte Codex block passes. Count provider units
before selecting whole files, not just Unicode scalars; a native spill preview
is still truncation. These samples prove no Loopflow integration or automatic
compaction.

### Rejected first-turn transports

Jack Heart selected terminal argv on October 9, with a size/cap error only for
an oversized first turn. No editor, paste, stdin or envelope path remains.
Archived probe and tests: `d81f12c42:scripts/benchmarks/skill-invocation/`.
Codex 0.161.0 rejected terminal stdin; paste changed CRLF and consumed literal
paste terminators; the editor preserved 285 KB Unicode/CRLF/terminators but
trimmed trailing whitespace. Exact-copy receipts and clean exits did not prove
exact submission. These observations establish no lf, resume or cmux acceptance.
