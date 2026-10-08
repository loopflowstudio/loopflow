# Native skill invocation

```sh
uv run python scripts/benchmarks/skill-invocation/probe.py \
  --claude /path/to/claude --codex /path/to/codex
```

Runs six bounded, live-provider probes in a temporary ordinary directory. Uses
existing provider authentication without reading credentials. No installed skills
or settings are edited. Claude has no model tools or MCP servers; its context
hook reads one fixture file. Codex runs read-only with user config disabled.
These calls consume normal provider usage.

The output records versions, elapsed seconds, marker observations and Claude's
replayed native command arguments. A plausible model answer does not prove native
dispatch or argument substitution. Exit zero requires both standalone invocations
and Claude's separate context hook to return their fixture markers; Claude must
also replay the exact original argument. Prefix, suffix and multi-block cases
record counterexamples without assuming future providers retain those limitations.

On 2026-10-07, Claude 2.1.293 recognized standalone stream-json invocation.
Prefixing context or using separate text blocks bypassed native expansion.
Suffixing context put it inside the provider's `<command-args>` receipt. A
`UserPromptSubmit` hook supplied context while preserving the argument. Codex
0.160.1 `exec` loaded the `.agents/skills` fixture by its `$name` invocation.

These are synthetic native fixtures. They establish neither third-party ports,
frontmatter/tool/model fidelity, bundled-file behavior, IDE handoff, Session
continuation, nor Loopflow capture. The one-shot timings are diagnostic samples,
not an lf-versus-slash-command latency comparison. App-server's explicit skill
input is documented but has not been exercised by this script.
