# Full docs rewrite: implementation review

2026-09-26 · LOO-309 · review-slice

Reviewed the complete branch diff from `102522bc25adc146d4b8cc3f343677ed8170f636`
through `94ec126eb1dc1a593caa6e88dffb2ca05a9e65b3`, plus the bounded corrections
recorded below. The worktree was clean before review.

Design: [The docs read in the new voice](the-docs-read-in-the.md).
Accepted scope: [Jack's design review](docs-voice-review.md).
Evidence limits: [Assumptions](questions.md).

## Intent and scope

Jack requested the full public-docs rewrite, with no page-count PR limit.
Scott and Kim are the readers: CS 101 knowledge, without assumed engineering
habits. All nine guide/reference pages, the glossary, overview terminology,
page descriptions, and the agent index are included. Get started is audited
for vocabulary; its walkthrough belongs to LOO-306.

The intended architecture remains one canonical Markdown corpus, one glossary,
and one public page inventory. HTML, Markdown endpoints, and the full agent
index consume that corpus. Architecture remains a separate developer path.
No runtime change, new app walkthrough, renamed URL, second audience track,
Project operator, or alternative content store is authorized.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| Full scope | Rewrite explanations throughout all in-scope pages | Complete diff covers the page plan; exact command/flag reference material remains | Full diff review and design's page-by-page ledger | Pass |
| Situation and app context | Explain when each page helps; app first where supported | Openings establish use, existing app relationship, or file/command work; no new desktop walkthrough | Reviewed page openings and complete prose diff | Pass |
| Terms on arrival | Plain explanations supported by one glossary | Expanded two-table glossary and real section links; review corrected remaining namespace/access vocabulary and overloaded boundary wording | Editorial inventory, direct page reading, rendered link checks | Pass as editorial review; comprehension unmeasured |
| Defaults and consequences | Preserve real behavior and explain choices | Context cost, configuration scope, headless reviews, Home-local reads, fallback, and Task permissions explained | Source review; corrected direct-launch model precedence against `engine/launch.rs` | Pass |
| Review completion | Ready is not Complete; Complete returns feedback, not merge approval | Conducting and Authoring describe the separate review and decision steps | `ops/human_session.rs::session_actions`, Task completion path, served source agreement | Pass for documented contract; desktop interaction unperformed |
| Permission accuracy | No invented worktree containment | Configuration and Security describe managed Task bypass and required-access probes | `controller/task/mod.rs` sets boundary and skip permissions; `ops/task.rs` probes roots/accounts; `engine/agent.rs` supplies vendor flags | Pass by source inspection |
| Account behavior | Preserve selection, fallback, and remote limits | Existing-account requirement, route preference versus restriction, pinned conversations, foreground forwarding retained | Diff and `provider_account.rs` inspection, including 95% threshold; website assertions | Pass for docs; live account exercise unperformed |
| Public retrieval | Every public page renders and serves revised Markdown | All 12 public slugs render; HTML links/fragments resolve; both Markdown access paths agree with canonical source after sync | Focused browser suite below | Pass |
| Shared front door | Preserve README opening and use definition in agent index | README/index parity retained; `/llms.txt` starts with the accepted definition | Parity and agent-reader tests | Pass |
| One content authority | No duplicate inventory, fallback summary, or architecture leakage | Four consumers read `DOC_PAGES`; generated copies still come from `sync_docs` | Negative searches and owner inspection below; full-index/sitemap/404 tests | Pass |
| Wider website health | Preserve navigation, accessibility, and developer pages | Full suite succeeds, with three explicitly skipped mobile title checks | Full website suite below | Pass with listed skips |
| Reader outcome | A non-engineer explains what to do and why | No attributed reader observation available | No read-aloud conducted | Gap: growth-program follow-up |

## Findings fixed in this review

1. The new model-selection sentence omitted saved configuration and conflated
   the skill's `agent` with `default_agent`. The reference now states the
   actual direct-launch precedence and distinguishes the managed Task path.
   Evidence: `engine/launch.rs:127–142`, `engine/config.rs::default_agent`,
   and `controller/task/mod.rs:749–759`.
2. The glossary called both a Flow transition and security containment an
   “execution boundary.” The product row now names **Flow boundary** and
   explicitly points out the security meaning.
3. The vocabulary audit found `namespace`, `canonical`, and access-control
   `capability`/`grant` without glossary entries. Added short definitions,
   explained the control secret in Security, and replaced “ambient login”
   and “webhook ingress” with ordinary wording. No flags or reference-table
   rows changed during this review.

These are bounded prose corrections. They do not change the accepted design
or invalidate the implementation's behavior model. The earlier implementation
ledger predates these corrections; this review supersedes its claim of complete
vocabulary coverage for those specific terms.

## Ownership and negative proof

Repository search found no remaining code references to `DOCS_NAV` or
`DOC_DESCRIPTIONS`. `DOCS_AREAS` creates `DOC_PAGES`; the curated index, full
index, sitemap, and missing-page suggestions read those records directly.
The removed title-as-description fallback is absent. `doc_path` retains the
existing deployment-copy/source lookup; `dev.py::sync_docs` is its writer,
and the source-agreement test verifies those copies. Generated files are
ignored outputs, not authored changes.

Public routes still enforce `PUBLIC_DOC_SLUGS`. Architecture has its own
inventory and routes and is excluded from the public agent corpus. The diff
does not change runtime, wire types, planning stores, architecture pages,
README, Get started, or homepage content. Obsolete Project-operation and
Approve/Iterate instructions were removed from the affected explanations.
The change advances the complete design without a parallel content authority.

## Verification and limits

- `uv run python scripts/check_architecture.py`: passed all eight coverage
  categories; zero unexplained owners, mirrors, shims, or stale vocabulary.
- `cd website && uv run python dev.py test`: **79 passed, 3 skipped in
  67.51s**. The skips are the three mobile viewport instances of the title
  overlap test when `.nav-title` is absent. This run preceded the bounded
  prose corrections above.
- After those corrections:
  `cd website && uv run --extra test pytest tests/e2e/test_docs.py tests/e2e/test_agent_readers.py tests/test_readme_index_sync.py -q`:
  **22 passed in 13.55s**. The existing fixture synchronizes docs, starts the
  real local website server, and uses unattended Playwright Chromium.
- `uv run ruff check website/main.py website/tests/e2e/test_docs.py website/tests/e2e/test_agent_readers.py`:
  passed. `git diff --check`: passed.

The local server and browser exercise the configured website path, including
Conducting, its glossary fragments, its revised Markdown, and the agent index.
They do not prove a deployed release, reader comprehension, desktop actions,
or live credential behavior. No production service was changed for proof.

## Recommended next action

The implemented slice is the full Task scope and is coherent for publication.
Publish the reviewed branch with `lf pr publish`; retain this evidence for
subsequent reviews. No blocking implementation findings remain. Reader
read-aloud and observed Mac app walkthrough evidence remain explicitly open
in their existing growth work. A following decision step owns Flow navigation;
this review neither lands the PR nor completes the Task.
