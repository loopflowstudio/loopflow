# Run installed skills on either harness

LOO-420, PR 2. Jack Heart's October 8 comments
`5f149330-5f5d-4a71-bb05-289b13fe2d94` and
`76407cd0-b271-45e7-8953-62abe7f9df2b` select ordinary installed-skill invocation,
matching help/list, native matching-harness execution, translated ports, exact
arguments, reachable assets and declarations. `--agent/-a` remains. Builtins stay
inline. Review is the delivery boundary; no landing or Task completion is authorized.
Busy-terminal injection and generic engine recovery remain excluded.

Reconciled October 8 against `754efacb2`. The earlier feedback describing
headless-Claude-only preparation is superseded: `process_prompt.rs` prepares
SkillInvocation for Claude and Codex; the Codex harness and terminal caller
consume it. Native Claude terminal invocation remains the main implementation gap.

## Current implementation

One catalog selects repository, personal and embedded sources for launch,
help/list, Flow capture and export. Third-party files remain unchanged. Source
frontmatter and arguments now reach prompt preparation on both harnesses and
surfaces. Gathered repository/Work context remains separate from skill arguments.

| Launch | Behavior |
| --- | --- |
| Claude source → headless Claude | Native command parser; an installed bundle loads directly with `--plugin-dir` pointing at its original directory, without copying or changing the bundle |
| Unchanged Codex skill with name/description → headless Codex | Native `skill` input item naming the installed path, with exact argument text in a separate text item; provider discovery outside the exercised `.agents/skills` bundle remains unproved |
| Unchanged Codex skill with name/description → terminal Codex | Native Markdown skill reference plus user context; headless execution of the terminal command is proved |
| Cross-harness | Arguments and tool-name instructions translate; original directory and frontmatter remain visible; one warning names controls this launch does not enforce |
| Either source → terminal Claude | Translated instructions, declarations, arguments and user context; **native Claude terminal invocation remains unimplemented** |
| Changed/removed Flow source | Captured Claude plugin definition, or captured Codex instructions; no reselection of a newly installed same-name skill |
| LF builtin | Existing inline path |

Claude Flow snapshots substitute the original `${CLAUDE_SKILL_DIR}` and name the
original directory for supporting and parent-relative paths. The old sibling-link
copy is deleted. Removing the entire original resource bundle is not preserved.
Codex silently ignored a native `skill` item pointing outside its discovered
catalog; changed/removed definitions therefore use captured instructions directly.

Independent Session preservation remains: saved placement, atomic reservation and
driver claim, capture preservation on refusal, pending input on publication failure,
and baseline native history/fences. Resume does not rerun the original skill.

## Counterexamples and remaining work

1. **Native Claude terminal + separate user context.** Claude 2.1.294 puts both
   SessionStart and UserPromptSubmit `additionalContext` hook text in the API's
   system field. The ordinary `lf --tui` command fixture caught this with a unique
   repository-content marker. Both hook implementations were deleted. Retaining
   translated terminal instructions is a safe fallback, not fulfillment of Jack's
   native invocation requirement. A supported transport must preserve native
   declarations and exact arguments without moving repository/Task content into
   system instructions. Successful output alone did not detect the hook violation.
2. **Remaining fidelity.** Exercise single-file Claude command collisions,
   unfamiliar native declarations and Codex custom-prompt argument conventions
   through ordinary lf entry points. Codex native dispatch checks unchanged bytes
   and name/description, but not provider catalog membership; the proved
   `.agents/skills` bundle does not establish every discovered `.codex/skills`
   or prompt path. Native Claude model selection is proved;
   cross-harness permission/model/subagent declarations currently remain reported
   instructions. Supported equivalents remain to be applied where available;
   model equivalence remains unguessed. Terminal rendering/interaction has not
   been exercised; the terminal-command fixture substitutes provider print/exec.
3. **Cost and complete acceptance.** Native matching checks make the same two API
   requests as a plain skill (one tool read, one answer). The final concurrent
   fixtures add 3.5–4 seconds and 8–13 KB of request JSON, including default
   Loopflow context. These are startup/context observations, not tokens,
   real API latency or proof of “strictly better.” The plain baseline currently
   asserts only successful exit and two requests; unlike the lf case, it does not
   assert source expansion, exact arguments or returned asset contents. Equivalent
   baseline behavior must be established before interpreting its cost comparison.
   Remaining acceptance includes both pinned sources on both harnesses/surfaces,
   declaration handling and help/list agreement with the selected source, closing
   measured regressions without a new numeric target. Full gate and review remain.

No new product decision is identified. The accepted outcome remains intact;
the rejected terminal transport needs an implementation replacement.

## Removed mechanisms

Dispatch identities, receipt recovery, queue/engine-restart and PTY/inbox probes
remain archived at `1e4ae02a5` and `7dd9819b2`; unreachable ClaudeHarness plugin
state at `dd2cdba82`. `754efacb2` removed the remaining provider-only probes
(sources: `31e0640eb:scripts/benchmarks/skill-invocation/`) and simplified launch
preparation. The replay-argument parser remains in the ordinary lf fixture;
baseline Session continuity remains in the existing e2e suite. Claude snapshots
accept only captured Claude source; unused Codex materialization is deleted.
Terminal prompt assembly belongs to the skill caller, not the generic launcher.
Codex borrows its first-turn invocation; argument expansion reuses declaration parsing.

Compression against `35bb84ef1`: before reduction 56 files, +5,818/-1,833;
`dd2cdba82` had 51 files, +2,769/-1,911. At `31e0640eb` plus local notes:
53 files, +3,356/-1,923. The subsequent cut removed the 718-line provider-probe
stack, retaining its replay parser and answer-echo counterexample.

## Evidence boundary

`launch.py` fetches pinned, unchanged Anthropic `internal-comms` and OpenAI
`skill-installer` bundles outside the repository before network containment. Real
provider clients then run ordinary lf commands against local fake APIs, with
fresh Machine/provider directories and no inherited LF authority or credentials.
The provider executes Read/exec_command against a real bundled file; its tool
result returns to the fake API. Checks cover default context, quoted/multiline
argument text, unchanged source and no preexisting `.lf` configuration. These
prove provider transport and tool execution, not live model compliance or login.
`request_mapping.py` additionally proves native Claude model selection, exact
arguments and captured Flow selection after source removal with a collision.

Release is the only immediate child Wave found in this checkout, including
directories without a goal. Its goal and complete memory were read in this pass;
its operation-entry lesson still applies. Review found and removed
the hook role violation and undiscovered Codex snapshot dispatch. Claude terminal
native fidelity stays explicit rather than being inferred from successful output.

Check (2026-10-08): `git diff --check` and `lf context --skill realign --json` pass within budget; reuse prior `cargo build -p loopflow --bin lf`, isolated `engine::skill_invocation` (3 tests), `uv run pytest scripts/benchmarks/skill-invocation/test_request_mapping.py` (12 tests), contained `request_mapping.py --flow` and `launch.py` (Codex native and both terminal commands), formatting, all-target Clippy and Ruff passes; full gate remains with gate, native Claude terminal fidelity remains implementation work.
