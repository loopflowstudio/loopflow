# Docs voice: concept review

2026-09-26 · LOO-309 · Reviewed HEAD `7a3a79f5c`

## Judgment and accepted intent

Keep the implemented model. This bounded review found no blocking conceptual
defect requiring another implementation pass. The docs explain the existing
product without adding a second audience, planning tier, or content authority.
Reader comprehension remains unmeasured; this judgment does not establish the
growth outcome or complete the Task.

Jack requested the full public-docs rewrite for Scott and Kim: CS 101 knowledge,
Mac app first, literal technical content, explanations beside commands, and no
marketing voice. His reviewed direction removes the earlier page-count limit.
The [design](the-docs-read-in-the.md), [accepted feedback](docs-voice-feedback.md),
and [slice evidence](docs-slice-evidence.md) remain the scope and proof owners.
Preserve URLs, exact reference material, operational exceptions, saved history,
and the distinction between review completion and merge authority. Installation,
new app walkthroughs, runtime redesign, and the people-only Flow view remain
outside this Task.

## Usage first

Retain the existing usage in Conducting, Authoring, and Troubleshooting. The
following review scenario summarizes their current contract; it proposes no
new button or behavior and is not an observed desktop walkthrough:

1. Open the Task's review conversation in Sessions. Read the findings and give
   feedback. The agent saves that feedback and marks the review ready.
2. Choose Complete when the conversation is finished. Its feedback returns to
   the Flow; the later deciding step determines whether work continues or
   repeats. Completing this review does not approve a merge.
3. If the terminal was closed before completion, reopen the listed Session.
   Inspect Task status and Sessions before restarting anything: an unfinished
   Task can still have a worker or a review waiting for attention.
4. If execution stopped because account access failed, repair access and use
   the documented Task resume command with a reason. Replay is for a new Run
   from an old request, not recovery of the Task's saved Flow position.

Affected guidance: `docs/conducting.md` (Steer; Inspect and resume),
`docs/authoring.md` (Flows; Working notes and feedback),
`docs/troubleshooting.md` (Task Work stops advancing; Rate limits), and the
glossary. Existing concept-review instructions already distinguish evidence
from decisions. No public-doc or skill rewrite is proposed by this review.

## Core model in one screen

| Reader action or question | Representation and owner | Public operation or transition |
| --- | --- | --- |
| Select ongoing work, then a piece to finish | Wave → Task; `WorkRef` also represents the internal chapter Project; `WorkStatus` records unfinished/done/abandoned | `lf status`, `lf roadmap`, `lf task status`; Project is not another navigation step |
| See what one AI launch did | `RunManifest` with `RunId`, parent, subject attribution, and later evidence in Home-local bundles | `lf runs`; `lf replay` creates another Run |
| Return to a conversation | Session kind and state, projected by `session_actions`; a Flow review is tied to its saved position and Run | `lf session open`, agent readiness, reviewer completion |
| Continue a Task after a stop | `FlowPosition` owns the captured invocation, execution cursor, review summary, worker claim, and failure | `lf task resume`; recover existing work rather than manufacture a new Task |
| Change steps or repeat part of the work | Authored Flow; invocation executes it, occurrence identifies a step, pass records loop visits | Author files for new invocations; a deciding step owns a declared backward edge |
| Read the same explanation in another format | `DocPage` records in `DOCS_AREAS`; canonical prose in `docs/*.md` | HTML, Markdown URLs, negotiated Markdown, and full agent index consume those pages |

Sources: `rust/loopflow/src/durable.rs:87`, `:131`, `:326`;
`rust/loopflow/src/run_record.rs:195`;
`rust/loopflow/src/ops/human_session.rs:61`, `:93`;
`website/main.py:194`, `:321`. These are existing types, not a proposed schema.

## Findings and challenged alternatives

### 1. Review completion has one job

The rewritten usage removes obsolete Approve/Iterate controls and explains
Complete as returning feedback. Source agrees: `session_actions` requires a
ready Flow review; `controller/task/mod.rs::complete_human_flow_step` checks
the current review identity, copies its summary into direction, and finishes
that step. `ops/human_session.rs::complete_flow` then attempts continuation.
If continuation fails, it reports that feedback was saved and supplies a
recovery command. Closing the provider is not the completion operation.

Current → retained experience: review, save feedback, Complete, then decide.
Collapsing review completion into navigation would remove a meaningful choice
and contradict the accepted Flow contract. No state or API deletion follows
from this branch's clearer explanation.

### 2. The similar nouns protect different recovery guarantees

Conducting distinguishes Work, Run, and Session beside their operations;
Troubleshooting explicitly separates unfinished Task state from execution and
open conversations. `resume_task_async` in `ops/task.rs:4620` resolves the same
Task and saved Flow position, checks failure recovery, and reconciles its PR
before continuing. A failure requiring a newly compiled invocation directs the
caller to restart; a recoverable failure requires a reason recording what
changed. The docs' general resume explanation should not be read as bypassing
these checks; exact recovery remains in the command reference and diagnostics.

Current → retained experience: select Wave → Task; consult launch history or
conversations when the question calls for them. Merging these nouns into a
single “job” would obscure whether to reopen a conversation, resume a Task, or
replay a request. Likewise, invocation, occurrence, and pass describe different
parts of repeatable execution; the Authoring explanation keeps those meanings.

The remaining cost is vocabulary density, especially in the glossary's combined
rows and advanced reference sections. That is an editorial risk, not observed
reader failure. A new terminology system or second beginner edition is not
justified by this evidence and would conflict with the accepted single-reader
approach.

### 3. One content model already reaches the consumers

Before this branch's compression, page records were split into title/slug
tuples and a description map, then joined again with a title fallback. Current
code reads `DOC_PAGES` directly in the curated index, full index, sitemap, and
Markdown missing-page suggestions. No Python references to `DOCS_NAV` or
`DOC_DESCRIPTIONS` remain.

Reader experience is preserved: open a page, follow a real glossary section,
or retrieve the same explanation as Markdown. For an unknown Markdown URL,
`markdown_not_found` offers nearby public pages and the index; HTML retains
its existing overview redirect. Public routes enforce `PUBLIC_DOC_SLUGS` and
architecture keeps its separate routes. No replacement lifecycle, queue, or
persistence store was added.

`website/dev.py::sync_docs` remains the writer of deployment copies, including
installer-link rewrites and removal of obsolete copied pages. `doc_path` reads
those copies before the source checkout. These are generated deployment files,
not independent authored docs. Deleting that lookup merely to reduce code
would change deployment assumptions beyond this review. Existing source-agreement
and retrieval tests cover the relevant contract.

### 4. Clearer prose must retain limits on authority

The glossary now distinguishes Flow boundaries from security containment;
Configuration and Security describe managed Task permission bypass explicitly.
Subscriptions separates account preference from account restriction. These are
necessary distinctions, not candidate synonyms. Retain the slice review's
source-backed corrections and evidence limits. A simpler phrase such as “the
Task stays inside its worktree” would reintroduce the disproven safety claim.

## Evidence, unresolved questions, and next proof

This pass inspected the branch diff, changed website consumers, review/recovery
source, and affected usage. Public docs and website files have no changes from
the post-correction commit `deff56feb` through this reviewed HEAD. The only new
file from this pass is this review; earlier behavioral claims are not invalidated.

Reuse the slice evidence: full website suite **79 passed, 3 skipped** before
its bounded prose corrections; focused docs, agent-reader, and README-parity
suite **22 passed** after them; architecture and lint checks passed. These are
recorded results, not reruns in this step. No runtime or content mutation needs
another suite solely because a new review phase began.

No blocking product decision is required. The unresolved proof remains a
non-engineer reading the docs, an observed Mac app path, and live credential
behavior; none was performed here. The three mobile title-check skips remain
explicit. Website rendering is not desktop-use or comprehension evidence.

The smallest next reader proof is to have Scott or Kim read Conducting's review
and recovery instructions and explain what Ready means, what Complete changes,
and what to do after closing the terminal. Record attributed answers rather than
infer understanding from working links. The first-task app walkthrough remains
with LOO-306. Reader findings may motivate specific wording changes; they do not
authorize an unbounded runtime redesign.

Return this judgment as evidence to the authored Flow. No additional concept
implementation is recommended. This review records no navigation decision,
publication, merge, or Task completion.
