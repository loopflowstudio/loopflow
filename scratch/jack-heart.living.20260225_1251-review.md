# Review: Wave Memory (Phase 02)

## What was implemented

Wave-scoped agents now carry forward durable learning by reading and updating `wave/<wave>/MEMORY.md` through normal prompt context. No new tools or APIs — memory enters via `DocumentSource::WaveMemory` and agents write it back via standard file access.

Concrete changes:

- **New document source** (`DocumentSource::WaveMemory`) wired through gather, prompt assembly, token accounting, and trimming.
- **Prompt injection** — `<lf:wave>` block now includes structured memory guidance (sections, budget, update instructions) plus current memory content or an explicit `(no memory yet)` marker.
- **MEMORY.md excluded from wave docs** — handled as its own source so it trims independently.
- **Trimming order** — memory drops before summaries/docs but after area docs. Task context > accumulated observations.
- **Context header** — reports memory tokens as a separate line item.
- **Ops prompts** — `update-wave` and `add-to-wave` now instruct agents to distill durable memory into canonical docs and trim duplicates.
- **Design doc deleted** — `wave/living/02-wave-memory.md` (the original design) removed; replaced by implementation + Phase 02 retrospective in `wave/living/README.md`.

## Key choices

1. **Single file over directory.** Original design had `memory/` with topic files (`SUMMARY.md`, `codebase.md`, `patterns.md`, `preferences.md`). Collapsed to one `MEMORY.md` — easier to reason about, natural size pressure, simpler for agents to write.

2. **Distillation via ops steps, not a new consolidation pass.** `update-wave` and `add-to-wave` already touch wave docs, so they now also prune memory. Lighter than adding a separate memory-consolidation mechanism.

3. **Soft-fail on read errors.** `gather_wave_memory_doc` returns `Ok(None)` if the file can't be read. Acceptable for now; logged as a known gap (Phase 04).

4. **Memory trims before summaries/docs.** An agent that loses past observations but can see the current diff does better than one that remembers everything but can't see what it's working on.

## How it fits together

`GatherSpec::normalize()` automatically includes `WaveMemory` when a wave is set and repo docs are enabled. `gather_documents()` reads `MEMORY.md` separately from other wave docs (which filter it out). The prompt assembler injects memory content inside the `<lf:wave>` block with structured guidance. Trimming treats it as an independent budget item between area docs and summaries.

## Risks and bottlenecks

- **Memory quality** — agents write varying-quality observations; wrong-but-plausible entries persist until an ops step catches them. Mitigated by distillation instructions in `update-wave`/`add-to-wave`.
- **Persistence across execution surfaces** — unverified whether any headless/session path writes to a detached workspace that could drop `MEMORY.md` edits. Tracked in `scratch/questions.md`.
- **Read-failure silence** — `MEMORY.md` read errors soft-fail with no logging. Fine while memory is supplementary; needs logging before it becomes load-bearing (Phase 04).

## What's not included

- Cross-wave / shared memory — memory is wave-private by default.
- Automated aging / inheritance on `split-wave`.
- Read-failure logging or warnings.
- New memory-specific APIs or tools.
- Memory lifecycle management beyond manual ops-step distillation.

## Wave alignment

- **Goals**: "Every wave-spawned agent starts with the wave's accumulated knowledge" — verified. "Agents write durable observations back to wave memory without special tools" — mechanism is in place (prompt instructs, filesystem delivers).
- **Metrics**: Two of three metrics verified and marked in README. Third (budget discipline) relies on trimming + ops distillation, which are implemented but not yet stress-tested.
- **Risks**: Memory bloat and quality risks are acknowledged in README with mitigations. No new risks introduced.

## Validation

All relevant test suites pass:

- `cargo fmt --all -- --check` — clean
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo test -p loopflow wave_memory` — 3 passed
- `cargo test -p loopflow --test context_tests` — 19 passed
- `cargo test -p loopflow golden_prompt` — 1 passed
- `uv run pytest python/tests/ -q` — 47 passed
- `tests/e2e/test_smoke.sh` — passed
