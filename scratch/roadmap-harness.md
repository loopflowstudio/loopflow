# Roadmap Harness (Draft)

## User intent (verbatim)

- "make a roadmap/harness/<>.md sequenced files"
- "numbered"
- "Ill handle implemetning them in sequence. I want you to scope them in to oredered projects, and ask me questions about the technical design or intent behind each one"

## What to build

Define a numbered set of `roadmap/harness/*.md` project specs that can be implemented in order to ship wave chat as a memory-first runtime.

## Proposed project sequence (working draft)

1. `roadmap/harness/01-contract.md` — Chat API contract + event schema + turn lifecycle.
2. `roadmap/harness/02-persistence.md` — SQLite schema + repository methods for messages/memory.
3. `roadmap/harness/03-agent-runtime.md` — `lf-agent` turn loop + model/provider abstraction.
4. `roadmap/harness/04-tools.md` — Memory tools + wave tools + guardrails/permissions.
5. `roadmap/harness/05-client-surface.md` — Python API/CLI chat methods + REPL flow.
6. `roadmap/harness/06-verification.md` — E2E behavior, compaction path, failure handling.

## Decisions so far

### 01-contract
- `send_message` is hard-required for user-visible output (no fallback path in v1).
- Event schema is stable in v1; treat event shapes as a client-facing contract.

## Open questions (to answer in design conversation)

### 01-contract
- Resolved above.

### 02-persistence
- Should memory blocks be mutable-in-place rows or append-only with latest-view projection?
- How much chat history should each turn load by default (count/token/time window)?

### 03-agent-runtime
- Max loop iterations per turn before force-stop?
- Anthropic-only hardcoded in v1, or keep provider flag/plumbing from day one?

### 04-tools
- Which wave tools are mandatory in v1: `read_file`, `write_file`, `shell` all at once or staged?
- For `shell`, do we require allowlisted commands initially?

### 05-client-surface
- Should `chat_repl` ship in the same milestone as API methods, or after core paths stabilize?
- Do we expose raw tool/event traces in Python responses or keep them behind a debug flag?

### 06-verification
- Is the done bar "tests pass" only, or include a scripted live run against `lfd` + real model?
- What failure modes must be explicitly tested in v1 (timeout, malformed tool call, missing `send_message`, DB lock)?

## Constraints (draft)

- Numbered roadmap files under `roadmap/harness/` are the planning artifact.
- Sequence should minimize backtracking between projects.
- Keep scope to first implementation draft, not final production hardening.

## Done when (draft)

- `scratch/roadmap-harness.md` is finalized with agreed sequence and project-level constraints.
- `roadmap/harness/01-06*.md` specs can be authored directly from this plan without guessing architecture.
