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

The output records versions, elapsed seconds, marker observations and whether
Claude's replayed native arguments match. A plausible model answer does not prove native
dispatch or argument substitution. Exit zero requires both standalone invocations
and Claude's separate context hook to return their fixture markers; Claude must
also replay the exact original argument. Prefix, suffix and multi-block cases
record counterexamples without assuming future providers retain those limitations.

On 2026-10-07, Claude 2.1.293 recognized standalone stream-json invocation.
Prefixing context or using separate text blocks bypassed native expansion.
Suffixing context put it inside the provider's `<command-args>` receipt. A
`UserPromptSubmit` hook supplied context while preserving the argument. Codex
0.160.1 `exec` returned the `.agents/skills` fixture marker for its `$name`
invocation; this script does not inspect Codex's native expansion receipt.

These are synthetic native fixtures. They establish neither third-party ports,
frontmatter/tool/model fidelity, bundled-file behavior, IDE handoff, Session
continuation, nor Loopflow capture. The one-shot timings are diagnostic samples,
not an lf-versus-slash-command latency comparison. App-server's explicit skill
input is documented but has not been exercised by this script.

## Native continuity

```sh
uv run python scripts/benchmarks/skill-invocation/continuity.py \
  --provider codex --executable /path/to/codex
uv run python scripts/benchmarks/skill-invocation/continuity.py \
  --provider claude --executable /path/to/claude
```

Use the provider binary directly to avoid terminal wrappers. Each run creates
synthetic skills in a temporary ordinary directory and uses native authentication.
Claude disables model tools and MCP; Codex uses read-only sandboxing, disables
MCP/apps/plugins through launch overrides, and asks for no tools. Existing skills,
settings and conversations are not edited. These calls consume provider usage and
leave their new native conversations available for receipt inspection after the
temporary fixtures are removed. Output names those conversations and transcripts.
Each protocol wait is bounded to 55 seconds; cleanup waits five seconds before
killing only the probe's process group.

Codex sends an explicit `skill` item beside the invocation and a separate text
context item through `turn/start`. After restarting app-server, it resumes the
same thread and repeats with a new argument, supplying context again. This checks
history retention and new input delivery, not recall without context resupply.
Checks require returned skill/path and text items, unchanged first-turn history,
and native user-role expansion of
the selected file for each turn. Claude checks native command replay, source
directory and substituted arguments across three processes. Before its second
turn, the probe deletes the hook program and omits hook settings; the original
context attachment must survive. The first answer and reasoning cannot echo the
context marker, preventing answer recall from masquerading as context retention.
The third process explicitly reinstalls the hook. Receipt parsing has offline
tests for wrong sources, altered arguments, assistant echoes and stale turns.
Both scripts share Claude launch settings and receipt parsing; continuity consumes
decoded events directly. Run the offline checks with
`uv run pytest scripts/benchmarks/skill-invocation/test_continuity.py -q`.

On 2026-10-07, Codex 0.160.1 and Claude 2.1.293 passed these transport checks.
**This is not a fidelity pass:** Claude records `hook_additional_context` with
`renderedRole: "system"`. This rendering field does not establish the role sent
to the model. [Claude's glossary](https://code.claude.com/docs/en/glossary#system-reminder)
describes reminders inside user messages or, for some models, system-role messages.
The earlier claim of a proven authority conflict is withdrawn; preserving
user-level reference authority still needs request-mapping evidence. Cross-session hook
isolation, active-turn steering, GUI handoff, third-party controls and Loopflow's
own capture/dispatch remain unproved. The probe does not use Codex's experimental
`additionalContext` field; its context is a separate user text block.
