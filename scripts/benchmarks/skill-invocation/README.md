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
