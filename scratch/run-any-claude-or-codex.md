# Run installed skills on either harness

LOO-420, PR 2. Jack Heart's October 8 comments
`5f149330-5f5d-4a71-bb05-289b13fe2d94` and
`76407cd0-b271-45e7-8953-62abe7f9df2b` select ordinary installed-skill invocation,
matching help/list, native matching-harness execution, translated ports, exact
arguments, reachable assets and declarations. `--agent/-a` remains. Builtins stay
inline. Review is the delivery boundary; no landing or Task completion is authorized.
Busy-terminal injection and generic engine recovery remain excluded.

Reconciled October 8. Prior plan and contrary evidence: `c6c75dc68` in this file.

## Current implementation

One catalog selects repository, personal and embedded sources for launch,
help/list, Flow capture and export. Third-party source files remain unchanged.
Gathered repository/Work context stays separate from skill arguments.

| Launch | Behavior |
| --- | --- |
| Claude source → headless Claude | Native command parser loads an unchanged bundle from its original directory |
| Claude source → terminal Claude | Native plugin snapshot retains declarations and exact arguments; separate user context is a JSON literal in the skill body, with argument/preprocessing syntax escaped |
| Codex SKILL.md → Codex, either surface | Explicit native Markdown skill reference selects the original path, including outside the provider's discovered catalog |
| Cross-harness or unfamiliar shape | Translated arguments and tool instructions; original directory/declarations stay visible; a warning names unenforced controls |
| Codex custom prompt → either harness | One-based positional placeholders and named `NAME=value` arguments expand before submission |
| Changed/removed Flow source | Captured Claude native plugin or captured Codex instructions, without reselection |
| LF builtin | Existing inline path |

Unfamiliar native declaration keys produce one warning without refusal. Metadata
such as license is retained without claiming enforcement. Native Claude owns its
model, tool, hook and subagent semantics. Cross-harness permission/model/subagent
controls remain reported instructions; model equivalence is not guessed.

Ordinary third-party launches without Work attribution or a captured Flow omit
Loopflow operating and conversation guidance. Budget enforcement remains; notices
appear for managed memory, scratch or bounded excerpts. Attributed Work and Flow
steps keep their guidance. This reversible scope choice removes unrelated
instructions from the drop-in path. Captured Flow input,
not the in-memory resolved invocation, distinguishes a Flow: ordinary CLI selection
also retains a resolved invocation.

Native Claude snapshots substitute the original `${CLAUDE_SKILL_DIR}` and name
its original directory for supporting and parent-relative paths. Removing an
entire resource bundle is not preserved. Session placement, reservations, capture
preservation and native continuation remain on their existing paths. Resume does
not execute the original skill again. Native terminal captures still have no
headless AgentProcessRequest; source and snapshot retention checks are separate.
Failed terminal plugin preparation reaches capture settlement before returning.

## Delete — do not maintain

Completed: delete Claude terminal's translated matching-harness branch; reuse
native plugin preparation. Delete Codex's silently ignored out-of-catalog `skill`
input item; both surfaces use explicit native links. No catalog mounts, provider
registration or additional resolver is needed.

Compression also removes the unused replay-argument parser and its exclusive
tests; argument fidelity is asserted on actual model requests. Codex owns its
text-input JSON in the harness. Claude writes each complete snapshot once, and
catalog selection skips file reads for names already supplied by a higher source.
Provider discovery now uses the existing executable PATH resolver, deleting the
`--version` subprocess and its availability cache. Launch failures remain failures;
an installed executable no longer needs a successful version command.

Previously removed queue, engine-restart, dispatch receipt and PTY/inbox machinery
remains at `1e4ae02a5` and `7dd9819b2`; provider-only probes at `31e0640eb`;
unreachable ClaudeHarness plugin state at `dd2cdba82`. Preserve the ordinary lf
fixtures and baseline Session continuity. Do not restore any of those paths.

## Counterexamples and remaining acceptance

- Claude 2.1.294 places SessionStart and UserPromptSubmit hook additional context
  in system instructions. A native shell-preprocessing directive becomes an
  instruction to call a tool, so it also fails to provide first-request context.
  Neither rejected mechanism remains. The native snapshot's JSON context passes
  exact decoded-text, user-role, multiline-argument and declared-model checks.
  LF's existing `&#36;` reference notation is decoded separately by the assertion.
- Codex 0.160.1 silently ignores a typed skill input outside its catalog. An
  explicit Markdown skill reference works there. The ordinary `.codex/skills`
  fixture proves this on the headless path; terminal command preparation already
  used that syntax. Valid prompt frontmatter does not turn a single-file prompt
  into a native SKILL.md bundle.
- Full gate owns the complete pinned-source × harness × surface acceptance,
  discovery/help agreement, remaining declaration cases and continuity suites.
  Focused proofs exercise native Claude terminal, single-file command collision,
  unknown declarations, native Codex outside its catalog, cross-harness custom
  prompt arguments and captured Flow removal. Provider print/exec substitutes
  for terminal UI; rendered interaction and live model compliance are unproved.
- Serial comparisons give both launches the same context text and restore the
  provider home before the plain launch. Both must expand the unchanged source,
  preserve arguments and return the bundled asset. Two API requests remain.
  The earlier unequal-context/concurrent measurements survive at `966af16fd`.

## Startup and prompt cost (October 8)

The debug binary, local fake APIs and timestamp wrapper measure this fixture,
not production latency. Before removal, equivalent-context serial pairs added
3,029 request JSON bytes for Claude terminal and 2,248 for Codex headless.
Budget and conversation guidance accounted for 2,154 and 2,126 bytes respectively.

| Matching native launch | Extra JSON bytes | Added seconds, existing Machine | Added seconds, fresh Machine |
| --- | ---: | ---: | ---: |
| Claude terminal / internal-comms | 875 | 0.713 | 1.009 |
| Codex headless / skill-installer | 122 | 0.748 | 1.112 |

Remaining Claude text names the snapshot, original asset directory and JSON-safe
context; Codex retains document provenance and separate input blocks, partly offset
by different transport metadata. Request JSON bytes are not token counts.

Almost all added time precedes provider startup: 0.643 s Claude / 0.654 s Codex
on initialized disposable Machines. Fresh-store creation adds about 0.3–0.4 s.
The traced Claude native launch preparation fell from 288 to 37 ms after removing
the executable version probe. Its final trace also measures prompt preparation
184 ms, capture publication 89 ms and Process admission 38 ms. The remaining
entry/setup time is not individually attributed; no overall latency win is claimed
from single samples. Budget tokenization, durable Session capture and launch
authority remain intact. No generic recovery or store redesign was introduced.

Review findings fixed: version probes rejected executable wrappers without a
version command; plain baselines inherited the preceding provider's cached state.
The fixture now resets that state and distinguishes total time from first request.
Residual cost prevents a blanket “strictly better” claim; gate and authored review
own the remaining acceptance, including whether this tradeoff meets the outcome.

No new product decision is identified. Full gate and authored review remain.

## Evidence boundary

`launch.py` uses pinned unchanged Anthropic `internal-comms` and OpenAI
`skill-installer` bundles fetched before network containment. Ordinary lf commands
run in fresh Machine/provider directories with no inherited authority or credentials.
The actual provider reads a bundled asset and returns its contents to a local fake
API. The synthetic custom-prompt mode separately proves argument conventions.
`request_mapping.py --terminal --command` proves native model selection, source
collision handling, exact arguments, unknown-declaration reporting and decoded
context containing literal argument/shell syntax. The flow fixture removes source
after capture and retains the original selection and operating guidance.

Release was the only immediate child Wave found; its goal and complete memory
were read during realignment. Its operation-entry lesson applies: successful
provider output alone cannot establish the requested launch behavior.

Check (2026-10-08): build; isolated lib `engine::process_prompt` (29) and `cli_discovery_needs_an_executable` (1); pytest request mapping (9); contained native serial comparisons, terminal command mapping and captured-Flow launch; fmt, Ruff and all-target Clippy pass. Full affected verification belongs to gate; earlier evidence remains at `966af16fd`.
