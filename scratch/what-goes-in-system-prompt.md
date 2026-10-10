# What goes in the system prompt

Status: draft. Decisions marked accepted are Jack's, 2026-10-09, in the design
Session. Everything else is proposal. Jack's subsequent LOO-444 steer authorizes
building proposed defaults and naming them at demo, not changing accepted channels.

Channel contract resolved (Jack, 2026-10-09, "option B"): the added-instructions
slot is byte-identical within a launch profile, and surface instructions,
participant name and reply guidance stay in it. A launch profile is the surface,
the reply settings and the participant; two launches sharing all three send the
same bytes. First-turn preparation and the whole-file refresh operation are implemented; the
fixed slot and provider delivery remain unfinished. Jack's later `e1fdb81b-75ff-4cca-b789-243ad84bad14` steer authorizes
that production cut. Implementation inspection found a native-terminal transport
boundary below; no fixed-slot decision remains open.
Reconciled 2026-10-09 against main `906576f39`, including `3e1e6245c` (#1512).
Local main and origin/main both name `906576f39`; no remote fetch or newer
upstream inspection is claimed.

First-turn transport resolved (Jack, 2026-10-09, "A is fine"): the first turn
stays a command-line argument on terminal launches. No editor, paste or stdin
transport is built. A first turn that would exceed the terminal's argument cap
fails before launch with a message naming the size and the cap; that is the one
permitted size refusal. The editor and paste probes are evidence only and are
not a production path. No further transport probes are authorized.

## Problem

Jack wants meaningful native resume titles and current, compactable context,
without restoring giant terminal messages or cmux arguments. Loopflow must add
to the harness's own instructions, not replace them. The original design transcript
and out-of-scope search measurements remain in the preserved reconciliation input.

## What to build

Loopflow launches an agent as a guest in its harness: a small fixed
instruction slot, the skill and request as the first turn, and one capped
context block injected into the conversation at start and after compaction.

The harness keeps what it owns: its built-in prompt, its loading of the repo's
agent guide, its compaction, its titles.

| Content | Channel | Status |
|---|---|---|
| Operating guide, surface instructions, user name, reply guidance | Added-instructions slot, byte-identical within a launch profile (surface, reply settings, participant) | Accepted |
| The skill, then any launch message | First user turn | Combined turn accepted; skill-first order is the proposed default under the later steer |
| Paths to `scratch/` and `wave/<address>/`; as much of Wave memory and scratch as fits; a listing with sizes for the rest and a line saying to read it | Context block | Accepted: "This seems fine" |
| The active skill, again | Context block, after compaction only | Accepted: "I like this" |
| Diff, `-d` docs | Not inlined; command or paths | Proposal |
| Clipboard, summaries, steers, Task brief | Complete private files under `.lf/prompts/`, referenced from the block | Proposed default |

Context block fill rule (proposal): a file is preloaded whole or listed, never
cut. Order: the Wave's `MEMORY.md`, then `scratch/<branch>.md`, then other
scratch files smallest first, until the cap.
Reserve space for paths, the complete listing and (after compaction) the skill
before choosing file bodies. A listing or skill can itself exceed the cap:
complete manifest/saved-skill files with pointers are a proposed extension,
not accepted replacement of the promised listing/skill. Pointer-read behavior
and the channel choice remain unresolved in `scratch/questions.md`.

## The demo

`lf design "fix the flaky gate"` inside cmux, in a worktree with 200 KB of
scratch, on Claude and on Codex. The chat opens with the design skill and
"fix the flaky gate"; `codex resume` lists real text; the agent already knows
the Wave memory and sees the scratch listing; after `/compact` the skill and
current scratch are back. On Codex the native base instructions remain alongside the added developer
instructions; test preservation rather than a model-specific opening sentence.

`AgentConfig.system_prompt` holds only additions; `task_prompt` holds skill/request.
The context-block operation carries paths, whole documents, listed UTF-8 sizes
and the compact-only saved active skill. Provider delivery is not connected yet.

## Key functions

- `build_context_block(repo_root, wave, moment) -> ContextBlock` — reads the
  folders at call time; `moment` is start or after-compaction.
- `lf __context-block --moment <start|compact>` — hidden command the hooks run,
  beside the existing `lf __provider-session`.
- Claude/Codex: `SessionStart` with startup and compact sources. Codex
  `PostCompact` is not a context-delivery hook.
- Codex app-server: additive `developerInstructions`, with session-scoped hook
  configuration/trust; the TUI bypass flag does not cover this surface.
- OpenCode: the same fixed `system` addition on each owned user turn, not only
  the first. Native terminal/compaction conversation delivery needs a proven
  integration before the obsolete assembly is deleted.
- Wave content: read the existing SQLite owner each time, including the selected
  Wave's ancestors as `gather_wave_docs` does today. Logical `wave/...` names are
  not proof that checkout files contain those bytes. Listed snapshots must be
  complete and readable; scratch reads current local files.

## Constraints

- No Loopflow-added repo or scratch text in a provider system prompt. This
  interpretation preserves harness-owned guide loading; OpenCode's native guide
  placement creates a literal conflict with the broader wording, still needing
  review. Moving context alone does not prove safe first-turn transport.
- Added instructions add to the harness's prompt, never replace it. Codex moves
  from `model_instructions_file` to `developer_instructions`.
- No launch argument approaches cmux's 122,880-byte cap.
- One rule for interactive, headless and persistent launches. Where lf owns the
  message stream it sends the same block under the same cap.
- The cap is the harness's own effective limit, not a config number. Count
  provider units, not just Unicode scalars: both harnesses spill a 10,000-scalar
  emoji block. Whole-file selection must prevent that native truncation too.
- Limits stop deciding what is sent. `memory_tokens` and `scratch_tokens`
  remain as size targets; `lf context` measures the folders against them.

## Done when

Focused block checks exist; complete-channel checks remain with gate (one Cargo
filter per invocation):

```sh
cargo test -p loopflow process_prompt
cargo test -p loopflow --test context_launch_tests
cargo test -p loopflow --lib context_block
cargo test -p loopflow --test context_block_tests
cargo test -p loopflow generated_titles
cargo test -p loopflow native_hooks_name_requests
```

Required behavior through lf's actual launch paths, not only the native probes:

- Claude and Codex start in cmux with 200 KB scratch and arguments comfortably
  below 122,880 bytes; first turn and native resume text contain skill then request.
  Large skill/request text must not simply move the oversized argument elsewhere.
- Added instructions are byte-identical across launches of one profile; no Loopflow-added file
  bodies or request appear there. Codex retains its native base and AGENTS.md,
  with no `model_instructions_file` override on terminal or app-server paths.
- Startup and manual/automatic compaction deliver current whole documents or
  readable complete listings; compact refresh also restores the active skill.
  Boundary cases include Unicode, many files and an oversized listing/skill.
- Supported resume/engine replacement retains native Session identity, refreshes
  context and hook trust, and never resends the initial skill/request as new work.
- Every supported surface uses this contract without truncation or
  an old/new switch; the only size refusal is a first turn over the argument cap. `lf context` reports size targets without gating launch.

Headless gate owns model-request/transport checks; demo owns cmux presentation
and native resume-list judgment. Native probes alone satisfy neither boundary.

## Production slice and remaining cut

The Rust changes now send builtin skill then request as the first turn; the constant
trigger and naming exceptions are deleted. Source-size targets measure without
excerpting memory, scratch, messages or live steers; goal/input ceilings and the
budget notice are removed. Measurement belongs only to `lf context` and usage
inspection, not launch preparation. `lf context` reads authored documents directly,
without assembling a prospective skill, Task seed, diff, clipboard or agent config.
Its obsolete `--skill` selector and original/submitted counters are deleted; totals
sum each complete source once. Rust/Swift usage reports drop the always-empty
assembled-budget fields while retaining measured assembly and unknown evidence.
Terminal dispatch checks the actual UTF-8 argument
(including space for NUL) before spawning; headless streams have no argument cap.
Codex app-server uses additive `developerInstructions` on start and resume.
OpenCode includes its addition on every owned turn.

This is an **incomplete internal slice**, not a shippable interpretation of the
channel contract. Repository context still enters the old file-backed system
channel; two terminal Codex writers still set `model_instructions_file`. Removing
those safely requires the fixed slot and conversation block together. Native
installed skills retain their invocation/argument/declaration path. No launch
flag selects old versus new behavior. No installed or provider acceptance is claimed.

The whole-file refresh operation now exists in Rust as `build_context_block`
and `lf --wave <address> __context-block --repo <checkout> --moment start|compact`.
It reads current scratch and the existing SQLite Wave/ancestor owner, snapshots
complete Wave bytes privately under `.lf/prompts/`, and reserves metadata and the
compact-only saved skill before selecting whole file bodies. The selected Wave's
memory precedes the branch plan, then remaining scratch by byte size; ties use
path order. Other inherited documents follow. Complete reference files are listed,
not inlined. The callback returns only a SessionStart `additionalContext` envelope.
**No launcher installs this hook yet.** Its CLI/source proof is not provider delivery.

The shared ceiling is 10,000 rendered UTF-8 bytes: Codex 0.161.0's default
2,500 approximate-token limit uses [ceil(UTF-8 bytes / 4)](https://github.com/openai/codex/blob/rust-v0.161.0/codex-rs/utils/string/src/truncate.rs),
and this also avoids the observed Claude Unicode spill. JSON metadata, reference escaping, paths and skill text
all count. If reserved metadata does not fit alongside the skill, a complete
private manifest replaces the inline listing; an oversized skill stays whole at
its saved path with an explicit read instruction. This includes combined overflow,
not only a listing that individually exceeds the cap. An unusually long path
uses a repository-relative manifest pointer rather than refusing for size.
These remain proposed defaults to demonstrate, not accepted UX. Unreadable scratch
now reports the exact failed file instead of silently claiming a complete listing.
This repairs the review finding in the shared scratch reader; no alternate reader
or filesystem Wave fallback was added.

Remaining production work:
1. Connect this operation to saved native hook settings/trust on Claude and Codex
   terminal/app-server, plus proven OpenCode terminal/compaction delivery. Retain
   capture/naming hooks, native resume and engine replacement without replaying
   the launch request. No launch flag may select old versus new delivery.
2. Replace `format_content_sections` / `format_wave_sections` and their consumers
   with the block, then delete their old inlining. Diagnostic formatting must use
   the final channels too. The operation alone does not remove old assembly.
3. Make additions byte-identical per accepted profile, move participant context into
   that slot for native skills too, replace the two remaining Codex terminal writers,
   and remove the installed-skill context split. Preserve assets and exact arguments.
4. Save Task briefs/steers, clipboard and summaries as complete private files and
   pass their paths to the callback (`--reference`); pass the captured active skill
   as `--skill-file`. Diff/docs become commands/paths. Preserve attribution and usage.
5. Gate owns real lf/fake-API integration and affected suites; demo owns cmux,
   pointer-read behavior and native resume-list judgment. Source callback checks
   establish neither automatic delivery nor the fixed-slot contract.

Review removed launch-only budgeting/report state and the retired transport runner;
Clippy also exposed its now-unused skill instruction accessor, deleted with the
translation test retained on the surviving path. Review previously found a masked fixture dependency: Flow tests read the old system file
instead of the provider's turn. Their stand-in now records both actual thread
and turn requests. That is source coverage, not an executed pass.

Checks: `cargo test --offline -p loopflow --lib context_block` (7), `cargo test --offline -p loopflow --test context_block_tests` (1), fmt, Clippy, `git diff --check` and installed `lf context` pass; provider delivery/affected suites remain with gate and cmux/pointer-read judgment with demo.

### Integrated upstream boundary

Main #1512 changes engine lifetime, not prompt delivery. Codex/OpenCode engines
now follow driver lifelines; replacing a provably dead driver ends its old engine
and resumes saved native history on a new one. Live handoff retains the lifeline.
The deleted surviving-engine recovery path is not a context-refresh integration
point. Context hooks and their complete source files must remain usable across
supported native resume and engine replacement without replaying the initial
skill/request or restoring the abandoned in-flight turn. Launch-scoped trust
must be established on the replacement engine as well as the first one.
These are preservation requirements for the production cut, not outcomes proved
by the provider-only probes. #1512's source and throwaway-child tests establish
no installed or real-provider lifeline acceptance.

Release is the only immediate child directory with memory in this checkout;
its goal and full memory were read. Its operation-entry lesson still applies:
all three Codex instruction writers and actual lf launch/resume paths need
coverage; provider-only passes cannot stand in for that integration.

## Delete — do not maintain

The remaining production targets belong to the same single PR; the first Rust
slice above removes the constant turn and launch bounding, not conversation delivery. The rejected transport runner `first_turn_transport.py` and its exclusive
`test_first_turn_transport.py` are deleted. Their complete editor evidence/code is
archived at `d81f12c42:scripts/benchmarks/skill-invocation/`; stdin/paste code is at
`d008a9761`. No transport probe remains to maintain or rerun. Shared native Codex
settings in `launch.py` remain used by context-delivery and skill-fidelity probes.

- Removed: `INITIAL_TURN_PROMPT`, its naming special cases and exclusive tests.
  Attributed request selection and existing-name protection remain.
- The `<lf:scratch>` block and file bodies in `format_wave_sections`.
  `format_prompt` remains diagnostic-only; replace its duplicate assembly with the
  final channels when conversation delivery lands. Its goldens are diagnostic
  snapshots, never acceptance of system-channel repository content.
- Removed: context bounding/excerpts, preserved-excerpt store, notice, goal/input
  ceilings and launch-time measurement. `measure_context` owns size-target reporting
  from documents only; no second submitted-source count or skill preview remains.
- Codex terminal `model_instructions_file` in `engine/agent.rs` and
  `lf/commands/util.rs::build_session_command`, including its incorrect comment.
  App-server replacement in `harness/codex.rs` is removed.
- The separate context split for installed skills in `prepare_process_prompt`,
  retaining native invocation, declarations, argument fidelity and captured assets.
- Removed: LOOPFLOW.md launch-excerpt guidance; scratch inlining guidance remains
  until the conversation-block cut. Removed unused `SkillInvocation::instruction_text`,
  formerly used only by launch budgeting; translated skill/argument coverage remains.
- Tests asserting scratch or memory text inside `system_prompt`.

Must survive: the size targets, `lf context`, `lf monitor usage --context`,
prompt logs under `.lf/prompts/`, source-boundary escaping.

## Forbidden outcomes

- A flag or config choosing between inline and injected delivery.
- The skill or request present in both the slot and the first turn.
- A file cut partway to fit the cap.
- Anything in the added-instructions slot that varies between two launches of
  the same profile: Task, Wave, branch, paths, time, skill text, request.
- Surface instructions, participant name or reply guidance moved into the
  context block.

## Internal slices

One PR; the split is unusable without the block. `This slice` is the production
cut in Rust; further probe work happens only where the cut needs a fact it lacks.

0. Fixed-slot metadata is resolved by option B; implement its accepted profile
   rule. Interpret native-guide wording as no Loopflow-added repository text,
   retaining harness guide loading; name that proposed default at demo.
1. First turn is skill then message, passed as an argument (Jack, option A).
   The constant turn and launch bounding are removed; only an oversized first
   turn may refuse. Native skills retain their accepted invocation path.
   Supported resume without launch-input replay still needs the complete cut.
2. Whole-file byte budgeting, overflow pointers and the callback now exist.
   Connect native hooks/trust and complete OpenCode terminal/compaction proofs. A passing marker can survive native middle truncation.
   Codex app-server hook trust needs production
   integration that preserves saved settings and existing capture hooks, on both
   thread start and supported resume after engine replacement.
3. Build the whole cut: fixed additive slot on every harness surface, real first
   turn with bounded transport, refreshed conversation block from existing owners.
   Preserve installed-skill fidelity and captured Flow skills. Remove old
   inlining, bounding, constant turn and provider overrides in the same change.
4. Make `lf context` measure size targets without launch refusal; retain usage
   reporting, prompt logs and source escaping. Exercise lf entry points against
   fake APIs, then the cmux/native-resume demo. Release's entry-point lesson
   applies: provider-only passes cannot establish Loopflow integration.

## Follow-ups (intent only; each gets its own design)

- Attribute tool reads to Scratch or Memory in `lf monitor usage --context` by
  the path read. Interactive Sessions need the harness's own transcript: lf's
  capture holds only their launch input.
- Before-compaction write to `scratch/`. Accepted: "Makes sense to do."
  Mechanism open: in a terminal session lf cannot make the agent take a turn.
- Search over past Sessions: `lf session search`, scoped to the Task by
  default, reading harness transcripts; raw excerpts, no model call. Index
  versus scan to be decided by measurement (Jack: "Probably worth performance
  testing this one specifically"); see Evidence.
- Not accepted: native skill delivery; `MEMORY.md` as an index; scheduled
  memory curation.

## Evidence (2026-10-09)

- Codex 0.161.0, lf-launched session: base instructions were Loopflow's
  144,605 characters with no "You are Codex". A plain session's base is 21,420
  characters. `AGENTS.md` arrives separately as a user message.
- Probe: `codex exec -c developer_instructions=…` kept the built-in base and
  added the text as a developer message. One small headless request.
### Prerequisite probes and contrary evidence

`context_delivery.py` in `scripts/benchmarks/skill-invocation/` runs real native
clients against local fake APIs with new Homes and external egress denied. It
checks actual model requests, not assistant claims. Claude excludes resume hooks
so their output cannot pass for after-compaction refresh. Codex proves that
PostCompact ran even though its context never reached the model.

- Claude 2.1.295: startup/compact output is user content, fresh bytes replace
  old bytes after manual compaction. Evidence:
  `/tmp/loo444-context-proof/claude-tx250ian/`. The older 2.1.294 system-field
  observation in the benchmark README remains evidence of that version only.
- Codex 0.161.0 app-server: additive instructions survive manual compaction;
  native base and AGENTS.md remain. Compaction regenerates the addition and
  replaces old hook context with fresh conversation content. The fixture uses
  the model's fallback native base, not a required “You are Codex” prefix.
  Startup/compact SessionStart context is
  developer conversation content; PostCompact output is ignored. Evidence:
  `/tmp/loo444-context-proof/codex-b2te08w4/`. An initial probe with the TUI
  bypass flag ran no hooks; fixture-only per-thread trusted hashes fixed the
  probe without saved config changes. This is not production trust integration.
- [Codex 0.161.0 output spill source](https://github.com/openai/codex/blob/rust-v0.161.0/codex-rs/hooks/src/output_spill.rs)
  sets the default at 2,500 approximate tokens. [Claude hook documentation](https://code.claude.com/docs/en/hooks#json-output)
  caps individual strings at 10,000 characters. Boundary-size behavior
  is exercised below; the earlier short-marker checks did not establish it.
- [OpenCode 1.18.33 request preparation](https://github.com/anomalyco/opencode/blob/v1.18.33/packages/opencode/src/session/llm/request.ts)
  appends `input.user.system` after its own prompt. The [prompt loop](https://github.com/anomalyco/opencode/blob/v1.18.33/packages/opencode/src/session/prompt.ts)
  passes the latest user message, whose `system` comes from that request. Thus
  first-turn-only delivery is not persistent. This is source verification,
  not an OpenCode live wire or compaction proof.

Review found two invalid acceptance shortcuts: a resume hook could conceal a
missing compact hook, and a hardcoded native-base opening depends on model
metadata. The probe excludes resume injection and checks preserved native base
content separately from additive markers. No production consumer is changed.

The probes share decoded message text, fake-API lifetime, native Codex settings
and PTY ownership. Marker checks inspect text, not serialized block metadata;
Unicode and quotes remain literal. Terminal replies scan only newly received
bytes plus a query-length overlap, preserving split/repeated queries without
rescanning the transcript. First-turn assessment reads only the first request;
later requests cannot repair missing launch text. Native guide messages remain
permitted before it. These probes predate Jack's option A and the first Rust cut.
Their argv concern now applies only to the remaining context-delivery writers. Both installed-skill probe entry points
now use the current `-i` selector, not removed `--tui`; gate owns lf entry checks.

### Rejected transport evidence (archived)

Jack's option A supersedes the transport investigation, not its observations.
Codex 0.161.0 exec accepted 225,023-byte stdin; TUI rejected stdin before a request.
Paste preserved 285,023 Unicode bytes but normalized CRLF and consumed literal
paste terminators. The external editor preserved Unicode/CRLF/terminators but
trimmed trailing whitespace (285,027 → 285,023 bytes), despite exact file copying
and clean exit. Neither a successful exit nor a marker proves exact delivery.
No production transport or lf/resume/cmux acceptance follows from these probes.
Code, examples and historical results: `d81f12c42:scripts/benchmarks/skill-invocation/README.md`
and its archived transport runner; earlier stdin/paste runner: `d008a9761`.
Full prior scratch and evidence paths remain in
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo444-compress-notes-re16ye_8/`.
No further transport probes are authorized.

### Hook-size counterexamples

`context_delivery.py` now compares the entire injected string at startup and
manual compaction. In `/tmp/loo444-hook-size-proof/`, 10,000 ASCII characters
arrive intact on Claude 2.1.295 (`claude-d0c91f0t`) and Codex 0.161.0
(`codex-85zy66ez`). At 10,000 Unicode scalars / 39,901 UTF-8 bytes, both spill
and truncate (`claude-avatk8kr`, `codex-e1y3mynh`). Codex retains both marker
checks despite cutting the middle, invalidating marker-only acceptance. A
2,500-scalar / 9,901-byte Codex block arrives intact (`codex-dhdmg8w5`). These
samples require provider-unit budgeting; they do not establish every boundary
or automatic compaction. Native spill files do not satisfy whole-file inlining.
The callback implements complete-list/skill overflow with readable pointers;
provider pointer-read behavior, OpenCode and launch integration remain unfinished.
The old assembly deletion list is unchanged.

Historical probe/check results predate the Rust cut; they are not production verification.

Retained result JSON read during reconciliation confirms 10,000-byte ASCII
refresh on both providers, Codex's Unicode middle truncation, and 285,023-byte
exec delivery with TUI stdin rejection. The successful Unicode paste and clean-exit
CRLF/terminator failures remain distinct from the earlier timeout; none establishes
a lossless production transport. No provider probes were rerun.
Latest evidence: `/tmp/loo444-compress-shared-context/` and
`/tmp/loo444-compress-shared-transport/`; prior compressed-probe results remain
under `/tmp/loo444-compress-context-proof/` and `/tmp/loo444-compress-transport-proof/`.

Earlier probe checkpoint: `87ae76eaf`. Complete pre-reconciliation scratch,
including original evidence paths and archived transcript, is preserved at
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo444-realign-saved-40zhwtws/`.
Those snapshots preserve the earlier local scratch. Pre-compression notes are also preserved at
`/tmp/loo444-compress-notes-SaPexN/`; pre-reconciliation notes at
`/tmp/loo444-realign-notes-8VzCoo/`.
