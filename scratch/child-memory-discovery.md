# Child memory discovery review

2026-09-30. Interactive review with Jack Heart; decisions settled.

Design: [Context allocation and subwave memory composition](audit-context-allocation-and-subwave.md).
Evidence: the design's September 28 audit and [audit scripts](loo331-audit/).
The audit did not establish live child-memory discovery or curation behavior.

## Feedback and design changes

Jack agreed with reading child memory when relevant and questioned duplicating
a child index in automatic context. Jack clarified that discovery need not use
the CLI: an agent familiar with where memory is written can use the filesystem.

The revised design removes the automatic child index and any requirement to
use `lf` for discovery. Agents can inspect `wave/<address>/` and read child
`MEMORY.md` files with ordinary tools. Proposed omission evidence in Run records
is separate from prompt content. The subsequent redesign uses current Session
capture evidence and no longer enumerates child/sibling omissions by default.

Jack then favored instructions for reading child memories in the prompt over
special loading infrastructure. The revised design puts that procedure in
`realign`: discover child memory files and read them using ordinary tools before
curating the parent. No skill-specific assembly or automatic child union is
needed. The working design includes proposed instruction text.

## Broader decisions from Jack

Jack approved address-derived ancestor inheritance, subject to a budget for
the combined ancestor memory so hierarchy depth can grow without unbounded
prompt growth. Memory-only scopes are acceptable. Jack approved curation of
the selected scope, reading immediate children and promoting broadly useful
lessons, with deeper exploration as needed.

Jack approved removing known duplicate delivery and the redundant reread
instruction. Scratch inclusion stays unchanged; this work must interface with
the budgeting efforts. Jack wants budgets that are respected, without choosing
values or a shape yet. Jack reported being told that 100k total tokens is a
threshold to fear; this is a reported concern, not a measured cutoff or an
agreed configured limit.

Jack corrected the obsolete Run-record premise and requested updating this
branch from main, then redesigning against the current architecture. Do not
implement the draft's `context.json` / Run-record additions or restore that
retired storage design. The earlier audit remains dated historical evidence.

## Remaining choices and next action

Jack asked whether the broader parent-and-child loading is specific to certain
skills. The original draft proposed that exception for `update-wave`, now
retired; current guidance assigns memory curation to `realign`. Jack's later
feedback replaces that loading exception with prompt instructions. Direct-child
reading and selected-scope curation were subsequently approved as described
above. Validate actual file reads and useful memory edits; prompt delivery
alone is insufficient.

The review work was checkpointed as `8c1381df2`, then `lf sync --manual main`
updated the scratch-only branch to `de074a2eb` and restored the design files.
The installed CLI has replaced `lf rebase` with `lf sync`. This was a local
operation with no publication or conflict resolution required.

The design is now rewritten against main. `context_budget.rs` already enforces
8k tokens for combined memory, 16k for scratch, 16k for launch messages, and 64k
for rendered input. It preserves complete oversized sources and sends labelled
excerpts. Those defaults are inherited implementation values, not newly chosen
by Jack. Provider-added context and later reads lie outside the launch ceiling.

AgentSession history now owns captured inputs. `PreparedTurnContext`, existing
decision kinds and subordinate `context.json` payloads remain; the redesign
uses them without restoring Run ownership. The reread header fix is already
upstream. Scratch keeps main's existing bounded behavior. Current source-block
attribution must survive excerpting, including scope labels cut by the budget.

Jack approved retaining main's labelled excerpts for an oversized memory chain,
with the full source available on disk and accurate scope labels. All review
choices are settled. Next useful work is implementing the revised design,
reconciling boundaries with Product/Infrastructure, and collecting current
behavioral evidence. No implementation or Flow navigation decision is recorded
here. Jack authorized terminal completion of this Session when ready.
