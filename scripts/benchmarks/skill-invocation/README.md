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
The rendering-only probe did not prove an authority conflict; the request-mapping
probe below now establishes it for the tested binary and model aliases. Cross-session hook
isolation, active-turn steering, GUI handoff, third-party controls and Loopflow's
own capture/dispatch remain unproved. The probe does not use Codex's experimental
`additionalContext` field; its context is a separate user text block.

## Claude request mapping

```sh
uv run python scripts/test_network.py uv run --no-sync python \
  scripts/benchmarks/skill-invocation/request_mapping.py --channel queued
```

Runs the real Claude client against a local fake Messages API with external
egress denied, a disposable home/config and dummy authentication. No live model
or real credentials are used. Output includes the binary version/digest,
requested model, actual request roles, selected source, exact native arguments
and request count. The fake response contains no fixture markers. Run its
offline assertions with `uv run pytest scripts/benchmarks/skill-invocation/ -q`.

`queued` sends a user context message with `shouldQuery: false`, then the exact
slash command. `staged` saves that context and exits without an API request,
then resumes the native Session to invoke the skill. Both then resume again
with a new argument and no context resupply. Success requires one model request
per invocation, user-only context and native source/argument agreement.

On October 7, Claude 2.1.294 passed queued and staged delivery for Sonnet;
queued delivery also passed for Opus and for 2.1.293's Sonnet. `--channel hook`
is the counterexample: 2.1.294 sends the context in the API `system` field for
both model aliases, initially and after resume. It correctly exits 1 because
the user-role requirement fails. This settles client request mapping; it proves
no live-model behavior, Loopflow capture, live-terminal admission, tool policy,
cross-harness translation or startup-cost improvement.

The production resume path replaces the captured input before claiming a new
driver. It cannot deliver through a surviving native terminal's current driver.
A saved native Session and a live input queue require different admission;
the design keeps that boundary unresolved rather than starting a second writer.


## Decisions and remaining work

Jack Heart selected native invocation on the skill's own harness, translated
ports on the other harness, and inlined Loopflow builtins on October 7, 2026.
Jack then selected `--agent` / `-a` for the harness and optional model, and
removed `--ide` after the installed reconnect probe below. The selector rename
and app-launch removal are implemented. LOO-420's native discovery, dispatch and
ports remain unfinished; these probes do not implement them.

The remaining design has these constraints:

- One catalog must supply discovery, help, execution and export source collection.
  Replace the separate resolver, rams alias, fuzzy npx fetching and recursive
  Markdown scan together. Installed npx skills remain ordinary skill folders.
  Repository-before-personal-before-builtin ordering is proposed; the precise
  cross-format ordering is unselected. Honor provider home overrides and prove
  the invoked native source is the selected file when names collide.
- Carry selected source, dialect, exact arguments and separate bounded context
  through canonical prompt preparation, capture and retained Flow instructions.
  Preserve supporting-file access and source bytes. Do not confuse Claude's
  subagent declaration with Loopflow's harness selector. Generated builtin
  exports cannot override current embedded instructions.
- Translate cross-harness arguments, tool and model declarations; report loss
  in one line and continue with runnable instructions for unfamiliar shapes.
  Permission hints do not establish enforced tool policy. Never overwrite an
  existing native skill to stage a port. No database migration is selected.
- An invocation in an existing Session must retain native history and Work
  attribution and enter through its current driver. Selecting another skill
  needs an explicit interface and admission proof; active-turn steering is
  unproved. Lost acknowledgements cannot justify duplicate execution.
- Use Claude's user-message channel, resolve live-Session admission, then prove Loopflow capture and
  unchanged third-party skills on both harnesses, including controls and bundled
  assets. Compare repeated matched plain-versus-lf startup and whole-turn context
  costs, including context read from disk and resumed conversations. Retention
  with resupplied context does not prove recall.

### Installed app-launch counterexample

On October 7, installed lf 0.13.9 launched Claude and Codex with `--ide` in
separate disposable data directories. Both commands exited zero and recorded a
launcher Process and interactive Session, but no native thread, endpoint,
provider PID or birth identity. Both actual `session connect` commands exited
1 because no connection or confirmed engine exit was recorded. OS acceptance
proved neither visible app execution nor a reconnectable conversation. The
candidate removes this path; it does not repair historical app Sessions.

The installed binary SHA-256 was
`4cf8c9e9a39a916dbe57be9abc3e6eb688ed3ad776ff9243b68537d69592103a`.
The disposable Sessions were `session_787c00c79b9142e38145b3b927fe147c`
(Claude) and `session_8896e66e4c034126ab7ec36aca19b899` (Codex). Native probe
receipts remain in Codex thread `01a11922-4b62-7011-9213-ce33542394a3` and Claude
Session `a162c8e3-4001-43fe-af9b-d6af17181c38`. These are evidence identities,
not control authority over a running provider.
