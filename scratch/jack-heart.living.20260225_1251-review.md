# Gate review: wave memory context integration

## What was implemented

- Added a dedicated `DocumentSource::WaveMemory` and wired it into context normalization, gathering, token accounting, trimming, and prompt formatting.
- Added `gather_wave_memory_doc()` to load `wave/<wave>/MEMORY.md` and excluded `MEMORY.md` from regular wave docs so memory is tracked independently.
- Extended the `<lf:wave>` block to include memory guidance plus the current memory content (or an explicit empty-state marker).
- Updated ops prompts (`update-wave`, `add-to-wave`) to distill durable memory into canonical docs and trim duplicated long-form memory entries.
- Added tests for memory loading, memory formatting, memory trimming order, and wave-doc separation.
- Updated parity fixture/golden prompt output to reflect wave memory injection.

## Key choices

- **Single-file memory (`MEMORY.md`)**: simpler than a directory schema and matches the updated design direction.
- **Independent source accounting (`WaveMemory`)**: keeps memory visible in token breakdown and enables deterministic trimming precedence.
- **Trim order: area docs → wave memory → summaries/docs**: preserves task-critical and architectural context while making memory compressible under budget pressure.
- **Memory embedded in `<lf:wave>`**: keeps wave intent + wave memory in one block so agents read/write memory as part of wave context, not as unrelated docs.

## How it fits together

`GatherSpec::normalize()` auto-includes `WaveMemory` for wave-scoped runs that include repo docs. `gather_documents()` now assembles scratch docs, wave docs, wave memory, then repo docs; `gather_context()` routes memory into `PromptComponents::wave_memory`. `trim_context_with_breakdown()` accounts memory tokens and drops memory before summaries/docs under pressure, and `format_reference_sections()` renders memory instructions/content inside the `<lf:wave>` section.

## Risks and bottlenecks

- `MEMORY.md` read failures are currently soft-fail (`None`) like other doc reads; silent misses could hide filesystem issues.
- Persistence semantics across all execution surfaces (especially detached/ephemeral worktrees in some paths) still rely on existing run/worktree behavior; this branch does not add explicit post-step/session memory flush logic.
- Large `MEMORY.md` content can still consume context budget in small-token runs before trimming kicks in.

## What's not included

- Cross-wave/shared memory behavior.
- Memory lifecycle features beyond trim pressure + ops distillation guidance (no explicit aging/inheritance automation).
- New memory-specific APIs or tools.
- Explicit runtime enforcement that every session/step writes memory back.

## Wave alignment

This advances Phase 02 goals in `wave/living/README.md` by giving every wave-scoped prompt access to accumulated wave memory and by instructing agents where to write durable observations. It also aligns with the updated maintenance story by adding memory-distillation guidance to `update-wave` and `add-to-wave`.

## Validation run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p loopflow wave_memory -- --nocapture`
- `cargo test -p loopflow --test context_tests`
- `cargo test -p loopflow golden_prompt`
- `uv run pytest python/tests/ -q`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py -v`

Note: `cargo test --all` fails locally on two docker startup recovery tests when `/var/run/docker.sock` is unavailable:
- `lfd::executor::docker::tests::docker_startup_rehydrates_running_agents_and_cleans_orphans`
- `lfd::executor::docker::tests::docker_startup_lost_agent_does_not_flip_terminal_run_wave_status`
