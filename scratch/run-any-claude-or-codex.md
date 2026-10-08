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
LOOPFLOW.md. Attributed Work and Flow steps keep it. This reversible scope choice
removes the unrelated operating manual from the drop-in path. Captured Flow input,
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
- The plain baseline now asserts the same source expansion, exact arguments and
  provider-returned asset contents as lf before comparing costs. Omitting the
  operating manual removes 6.6–6.7 KB from matched request JSON, leaving about
  2.3 KB (Codex) / 3.1 KB (Claude terminal) over plain. Both use two API requests.
  Concurrent fresh-machine fixtures add roughly 3.4–3.5 seconds; a serial trace
  adds 1.1 seconds, with 191 ms in prompt preparation. Startup attribution and
  remaining regressions need resolution before claiming “strictly better.”
  JSON bytes are not tokens, and fake-API latency is not live model latency.

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

Check (2026-10-08): build, isolated skill-invocation tests (4), request-mapping tests (12), contained native-terminal/command-collision/Codex-folder/custom-prompt/Flow fixtures, formatting, Ruff, all-target Clippy and context-budget readback pass (memory 15,942 tokens; scratch 1,639); full affected verification belongs to gate.
