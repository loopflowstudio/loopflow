# Rust Core Engine (Stage 2)

## Problem

The Python `lfd` daemon shells out to `lf` for every step execution, coupling daemon reliability to Python runtime quirks. Context assembly, flow parsing, and token counting happen in Python—logic that needs to be predictable, fast, and portable for 24/7 managed clusters.

Stage 2 builds `lf-core`: the Rust engine that owns the execution semantics. Python `lfd` calls into this engine. Later stages replace the Python daemon entirely.

Who benefits: Wave operators running `lfd loop` overnight. Enterprise deployments needing deterministic behavior. Anyone hitting Python's memory or GC issues during long-running flows.

Why now: The Python implementation works but accumulates state between runs. Fork execution is brittle. Token counting is a guess. The Rust workspace already exists with working flow parsing—this stage completes the execution story.

## Approach

Complete `lf-core` to execute flows end-to-end by shelling to `lf --step` for actual agent invocation. The engine owns:

1. **Flow parsing** — already works for step/fork/choose/loop structures
2. **Tick-based runtime** — already handles linear steps and pauses at interactive steps
3. **Fork execution** — already creates parallel worktrees and runs branches
4. **Context assembly** — skeleton exists, needs full parity with Python `gather_prompt_components`
5. **Token counting** — fallback exists, needs tiktoken-rs for accuracy

The engine does NOT invoke models directly. It shells to `lf --step <name> --worktree <path> --direction <d1>,<d2>` for actual execution. This keeps the boundary clean: lf-core owns the flow graph, `lf` owns the agent invocation.

**Key insight:** The existing implementation is ~80% complete. The gaps are:
- Choose/LoopUntilEmpty execution (parsed but not executed)
- Full context assembly (docs, diff_files, clipboard, summaries, wave)
- tiktoken-rs integration
- Python integration via FFI or subprocess

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Full Rust daemon (Stage 2+3 combined) | Ship faster, but bigger blast radius | Risk is too high. Staged rollout lets us validate engine correctness before replacing the daemon. |
| Shell to `lf flow` instead of `lf --step` | Simpler invocation, but no tick control | Daemon loses step-by-step visibility. Can't pause at interactive steps. |
| gRPC between daemon and engine | Clean boundary, but adds latency | Overkill for in-process calls. Save for Stage 3 when daemon moves to Rust. |
| Skip tiktoken, keep byte heuristic | Ship faster | Token counting accuracy matters for context trimming. Wrong counts = dropped content or context overflow. |

## Key decisions

1. **Shell to `lf --step`, not `lf flow`.** The daemon controls tick-by-tick execution. Flows are data structures, not black-box commands.

2. **Complete Choose/Loop execution this stage.** The parsing exists. Execution is the gap. Without it, flows like `roadmap-reduce` (which uses forks) work but `grind` (which uses choose) doesn't.

3. **tiktoken-rs for cl100k_base.** The `tiktoken-rs` crate wraps OpenAI's tokenizer. It's accurate for Claude (close enough) and well-maintained. Fallback to bytes/3 only if the crate fails to load.

4. **Context assembly: diff_files before docs.** When trimming, drop docs first (they're reference material). Keep diff_files (the actual work). This matches Python's priority order.

5. **Python integration via `lf-core` CLI.** Add `cargo build --bin lf-core` that exposes `lf-core tick <run-id> --db <path>` and `lf-core context <opts>`. Python daemon calls the binary. FFI can come later if subprocess overhead matters.

## Scope

**In scope:**
- Choose and LoopUntilEmpty execution in tick_flow
- Full context assembly matching Python parity
- tiktoken-rs integration with fallback
- `lf-core` binary for Python integration
- Golden flow tests: ship, grind, roadmap-reduce, roadmap-polish
- Event emission for step/flow lifecycle

**Out of scope:**
- Replacing Python daemon (Stage 3)
- gRPC engine contract (Stage 3)
- Git workflow operations (Stage 4)
- Postgres backend (Stage 5)

## Done when

```bash
# Flow parsing matches Python structure
cargo test --package lf-core flow_parsing_parity

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

# CLI integration works
lf-core context --repo . --step implement --direction product-engineer
lf-core tick run-123 --db ~/.lf/lfd.db

# Python daemon calls Rust engine
pytest tests/test_lfd.py -k rust_engine
```

Observable outcome: `lfd loop ship src/` executes with Rust lf-core handling tick_flow. Same behavior as Python, but deterministic state transitions.

## Implementation sequence

1. **Choose execution** — Add branch selection to tick_flow based on prompt evaluation
2. **LoopUntilEmpty execution** — Add iteration with termination condition
3. **Context assembly** — Port Python's gather_prompt_components to Rust
4. **tiktoken-rs** — Add dependency, wire up count_tokens
5. **lf-core CLI** — Binary with tick and context subcommands
6. **Python integration** — Daemon shells to lf-core instead of inline code
7. **Golden flow tests** — ship, grind, roadmap-reduce with fixture comparison

## Risks

**Choose prompt evaluation:** The choose construct requires evaluating a prompt to select a branch. Currently this means shelling to an LLM. For testing, we'll use a mock that returns deterministic choices. For production, the daemon may need to invoke the model directly—but that's Stage 3 territory. For now: Choose branches are selected by the first matching option, or error if ambiguous.

**tiktoken-rs version drift:** The crate may lag behind OpenAI's tokenizer updates. Acceptable: token counts within 5% of Python's tiktoken are fine for context budgeting.

**Worktree cleanup on fork failure:** If a fork branch fails mid-execution, worktrees may be left behind. Current code attempts cleanup but doesn't guarantee it. Acceptable: stale worktrees are annoying but not data loss. The daemon's autoprune handles cleanup.
