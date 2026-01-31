# Rust Core Engine (Stage 2)

## Problem

The Python `lfd` daemon shells out to `lf` for every step execution, coupling daemon reliability to Python runtime quirks. Context assembly, flow parsing, and token counting happen in Python—logic that needs to be predictable, fast, and portable for 24/7 managed clusters.

Stage 2 builds `lf-core`: the Rust engine that owns execution semantics. Both `lf` and `lfd` call into this engine—`lf` via PyO3 bindings, `lfd` via the Rust library API.

Who benefits: Wave operators running `lfd loop` overnight. Enterprise deployments needing deterministic behavior. Anyone hitting Python's memory or GC issues during long-running flows.

Why now: The Python implementation works but accumulates state between runs. Fork execution is brittle. Token counting is a guess. The Rust workspace already exists with working flow parsing—this stage completes the execution story.

## Approach

Complete `lf-core` as a self-contained engine that executes flows end-to-end, including agent invocation. The engine owns:

1. **Flow parsing** — already works for step/fork/choose/loop structures
2. **Tick-based runtime** — already handles linear steps and pauses at interactive steps
3. **Fork execution** — already creates parallel worktrees and runs branches
4. **Context assembly** — skeleton exists, needs full parity with Python `gather_prompt_components`
5. **Token counting** — fallback exists, needs tiktoken-rs for accuracy
6. **Agent invocation** — spawn runner (claude/codex/gemini) with assembled prompt

lf-core exposes two interfaces:
- **PyO3 bindings** — Python `lf` imports lf-core directly, no subprocess overhead
- **Rust library** — `lfd` (after Stage 3) uses the library API directly

Python `lf` becomes a thin CLI wrapper that imports lf-core via PyO3. This inverts the current dependency: instead of lf-core depending on lf, lf depends on lf-core. It also makes lf-core available as a Python package for scripting—useful given Python's popularity in AI tooling.

**Key insight:** The existing implementation is ~80% complete. The gaps are:
- Choose/LoopUntilEmpty execution (parsed but not executed)
- Full context assembly (docs, diff_files, clipboard, summaries, wave)
- tiktoken-rs integration
- Agent invocation (subprocess spawning, output handling)

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Full Rust daemon (Stage 2+3 combined) | Ship faster, but bigger blast radius | Risk is too high. Staged rollout lets us validate engine correctness before replacing the daemon. |
| lf-core shells to Python `lf` for agent invocation | Smaller Stage 2 scope, reuse existing code | Wrong dependency direction. lf-core should be self-contained. Creates Python→Rust→Python call chain. |
| gRPC between daemon and engine | Clean boundary, explicit contract | Overkill for in-process calls. Save for Stage 3 when daemon moves to Rust. |
| Skip tiktoken, keep byte heuristic | Ship faster | Token counting accuracy matters for context trimming. Wrong counts = dropped content or context overflow. |
| Binary interface instead of PyO3 | Simpler build, language-agnostic | Subprocess overhead on every call. PyO3 is worth the build complexity for the primary use case. |

## Key decisions

1. **lf-core is self-contained.** It spawns agent runners directly (claude/codex/gemini), not via Python `lf`. This keeps the dependency direction clean: `lf` depends on `lf-core`, not the reverse.

2. **PyO3 for Python integration.** Python `lf` imports lf-core directly via PyO3 bindings—no subprocess overhead, no binary interface needed for the primary use case. Rust `lfd` uses the library API directly.

3. **lf-core is stateless.** No database access. Run state (SQLite) is an lfd concern. lf-core takes inputs, produces outputs, spawns agents. The caller manages persistence.

4. **Complete Choose/Loop execution this stage.** The parsing exists. Execution is the gap. Without it, flows like `roadmap-reduce` (which uses forks) work but `grind` (which uses choose) doesn't.

5. **tiktoken-rs for cl100k_base.** The `tiktoken-rs` crate wraps OpenAI's tokenizer. It's accurate for Claude (close enough) and well-maintained. Fallback to bytes/3 only if the crate fails to load.

6. **Context assembly: diff_files before docs.** When trimming, drop docs first (they're reference material). Keep diff_files (the actual work). This matches Python's priority order.

7. **Config parity.** lf-core reads the same config files as Python lf: `~/.lf/config.yaml` (global) and `.lf/config.yaml` (repo), with the same precedence rules. No behavior change for users.

## Scope

**In scope:**
- Choose and LoopUntilEmpty execution in tick_flow
- Full context assembly matching Python parity
- tiktoken-rs integration with fallback
- Agent invocation (spawn claude/codex/gemini, handle output)
- Config loading (same files and precedence as Python)
- PyO3 bindings for Python integration
- Golden flow tests: ship, grind, roadmap-reduce, roadmap-polish
- Event emission for step/flow lifecycle

**Out of scope:**
- Replacing Python daemon (Stage 3)
- gRPC engine contract (Stage 3)
- Git workflow operations (Stage 4)
- Postgres backend (Stage 5)
- Database/state management (lfd concern, not lf-core)

## Done when

```bash
# Config loading matches Python behavior
cargo test --package lf-core config_loading_parity

# Flow parsing matches Python structure
cargo test --package lf-core flow_parsing_parity

# Agent invocation spawns runner correctly
cargo test --package lf-core agent_invocation

# Tick through auto flow end-to-end
cargo test --package lf-core tick_auto_flow_end_to_end

# Tick to interactive step, verify WAITING state
cargo test --package lf-core tick_interactive_pauses

# Fork branches execute and synthesize
cargo test --package lf-core tick_fork_advances_after_branches

# Choose branches work
cargo test --package lf-core tick_choose_selects_branch

# LoopUntilEmpty terminates correctly
cargo test --package lf-core tick_loop_until_empty

# Token counting with tiktoken-rs
cargo test --package lf-core token_counting_tiktoken

# Context assembly matches Python output
cargo test --package lf-core context_assembly_parity

# PyO3 bindings work from Python
python -c "import lf_core; lf_core.run_step('debug', clipboard=True)"

# Python lf uses lf-core via PyO3
lf debug -c  # under the hood: import lf_core; lf_core.run_step(...)
```

Observable outcome: `lf debug -c` executes via lf-core imported through PyO3. Python `lf` is a thin wrapper. Same UX, Rust engine underneath.

## Implementation sequence

1. **Config loading** — Port config.py to Rust, same files and precedence
2. **Agent invocation** — Spawn runner subprocess, capture/stream output
3. **Context assembly** — Port Python's gather_prompt_components to Rust
4. **tiktoken-rs** — Add dependency, wire up count_tokens
5. **Choose execution** — Add branch selection to tick_flow based on prompt evaluation
6. **LoopUntilEmpty execution** — Add iteration with termination condition
7. **PyO3 bindings** — Expose lf-core API to Python
8. **Golden flow tests** — ship, grind, roadmap-reduce with fixture comparison

## Risks

**Choose prompt evaluation:** The choose construct requires evaluating a prompt to select a branch. Currently this means invoking an LLM. For testing, we'll use a mock that returns deterministic choices. For production, lf-core invokes the runner. For now: Choose branches are selected by the first matching option, or error if ambiguous.

**tiktoken-rs version drift:** The crate may lag behind OpenAI's tokenizer updates. Acceptable: token counts within 5% of Python's tiktoken are fine for context budgeting.

**Worktree cleanup on fork failure:** If a fork branch fails mid-execution, worktrees may be left behind. Current code attempts cleanup but doesn't guarantee it. Acceptable: stale worktrees are annoying but not data loss. The daemon's autoprune handles cleanup.

**Runner configuration parity:** lf-core needs to read the same config that Python `lf` uses to select runners. Must support claude/codex/gemini selection, model preferences, and any runner-specific flags.

## Open questions

1. **Output handling** — When lf-core runs an agent, does it:
   - Stream to stdout in real-time (for interactive use)?
   - Capture and return structured output (for programmatic use)?
   - Both, controlled by a flag?

   Needs exploration. Affects how Python `lf` wraps the call.

2. **Interactive step handling** — Interactive steps need a TTY. The expected path is `session.connect` from lfd. For direct library calls without TTY, lf-core should return a clean error or WAITING status. Details TBD, but failure should be clear.

3. **Tick as a concept** — Flows use ticks internally, but there's no cross-process state. Tick is probably a library-internal concept that lfd uses, not an exposed interface. The Python `lf` CLI runs flows to completion; it doesn't need tick-level control.

## Resolved

- **Config location**: All of the above—`~/.lf/config.yaml`, `.lf/config.yaml`, CLI args. Same precedence as Python. No user behavior change.
- **Python integration**: PyO3 bindings, not subprocess. Python `lf` imports lf-core directly.
- **Database**: lf-core is stateless. Run state is an lfd concern.
- **Python lf scope**: Thin wrapper. lf-core does the work. Python provides easy access to Rust engine for the AI scripting community.
