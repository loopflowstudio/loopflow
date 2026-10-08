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
and request count. Each observation groups pass/fail assertions under `checks`;
all captured requests contribute to its models, roles and count. The fake response
contains no fixture markers. Run its
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
admission through the current owner remains implementation work. The current
string-only harness API also needs separate context and native invocation input;
the provider probes do not implement either boundary.

## Native terminal admission

```sh
uv run python scripts/test_network.py uv run --no-sync python \
  scripts/benchmarks/skill-invocation/request_mapping.py --channel terminal
```

Reproduces two rejected delivery paths using a real Claude terminal, a disposable
home and a fake local API. Only the probe's own PTY and native inbox receive input.
The SessionStart fixture child consumes its own socket capability from the
environment without recording it. External egress is denied; no real credentials
or model are used. Exit zero means the counterexamples reproduced, not that live
skill delivery works. A missing inbox or changed provider behavior fails the probe.

Claude 2.1.294 produced three requests under one native Session
ID: startup `/lf-mapping alpha` expanded the exact source and arguments with
user-role context; PTY injection appended `/lf-mapping beta` to an unfinished
draft and submitted both without expansion; the authenticated inbox delivered
`/lf-mapping gamma` as text without expansion. `/exit` returned zero. The fake
response contains no markers. These are API receipts, not assertions about a
model's answer. Plain PTY injection loses editor fidelity; successful socket
delivery does not prove native command handling.

The tested binary's inbox handler deliberately skips slash parsing. The documented
[background-resume path](https://code.claude.com/docs/en/sessions#resume-a-running-background-session)
also refuses slash-prefixed prompts. An LF terminal over the persistent stream
driver is a concrete alternative, but replaces native interactive controls and
requires a product decision. These findings reject the tested injection paths;
they do not establish that replacing the terminal is unavoidable or prevent
catalog and structured dispatch implementation. Loopflow admission and third-party
fidelity still require proofs through Loopflow's own entry points.


### Explicit Claude plugin selection

Run `request_mapping.py --channel plugin` under the same network wrapper.
It loads the fixture through a private plugin whose skill directory is a symlink
to the unchanged source. On Claude 2.1.294, initial and resumed requests each
expanded exact arguments with user-only context in one model request. The strict
`selected_source` assertion fails: Claude reports the plugin symlink, not the
original directory, as its base directory. Exit 1 preserves that distinction.
This is a candidate for unambiguous startup selection; bundled assets, source
identity through symlinks, retained source contents, and live admission remain
unproved. The probe neither replaces native editing nor establishes lf dispatch.


## Decisions and remaining work

Jack Heart selected native invocation on the skill's own harness, translated
ports on the other harness, and inlined Loopflow builtins on October 7, 2026.
Jack then selected `--agent` / `-a` for the harness and optional model, and
removed `--ide` after the installed reconnect probe below. The selector rename
and app-launch removal are implemented. LOO-420's native discovery, dispatch and
ports remain unfinished; these probes do not implement them.

[LOO-420](https://linear.app/loopflow/issue/LOO-420) owns implementation and
acceptance: one catalog, faithful native dispatch, translated ports, existing
Session admission, unchanged third-party fixtures and matched startup/context
measurements. The working design lives in `scratch/run-any-claude-or-codex.md`
while the Task is in progress.

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
