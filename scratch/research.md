# Research: Loopflow Codebase

## System understanding

Loopflow is a three-tier orchestration system for AI coding agents. The core abstraction is the **wave**: area × direction × flow × stimulus. Everything else exists to make waves run reliably.

### Architecture

**Three CLI tools with distinct responsibilities:**

| Tool | Entry Point | Responsibility |
|------|-------------|----------------|
| `lf` | `src/loopflow/lf/cli.py` | Interactive step/flow launcher |
| `lfd` | `src/loopflow/lfd/cli.py` | Daemon for persistent wave orchestration |
| `lfops` | `src/loopflow/lfops/commands.py` | Git automation and PR management |

**Directory structure:**
```
src/loopflow/
├── lf/           # Step execution, context assembly, model launching
├── lfd/          # Wave persistence, triggers, daemon server
├── lfops/        # Git ops, PR creation, worktree management
└── templates/    # Built-in steps, flows, directions
```

### Data flow

**lf command → prompt assembly → agent execution:**

1. CLI parses flags into `ContextConfig`
2. `gather_prompt_components()` collects: area files, git diff, repo docs, clipboard
3. Token budgets applied during gathering (area: 50k, docs: 30k, diff: 20k)
4. `format_prompt()` wraps components in XML sections
5. `launcher.py` builds model command (claude/codex/gemini)
6. Subprocess runs agent with assembled prompt

**lfd daemon → wave orchestration:**

1. Unix socket server at `~/.lf/lfd.sock` (JSON-over-newline protocol)
2. SQLite at `~/.lf/lfd.db` stores waves, runs, step history
3. Triggers evaluated every 30s: loop (always), watch (git diff), cron (schedule)
4. Worker subprocess spawned per wave with `start_new_session=True`
5. Concurrency managed via slot acquisition (default 3 parallel)
6. Events broadcast to subscribers (Concerto UI, CLI watchers)

### Key abstractions

**Step:** Markdown file with frontmatter. Located in `.lf/steps/`, `.claude/commands/`, or builtins. Resolution order: repo → global → builtin.

**Flow:** YAML DAG of steps. Supports linear chains, parallel batches (`after:` dependencies), forks (multi-worktree parallel with synthesis), and conditional branches (`choose:`).

**Direction:** Markdown file shaping judgment. Composes—`-d designer,product-engineer` layers both perspectives.

**Wave:** Persistent config binding area + direction + flow + stimulus. Tracked in SQLite with status: IDLE, RUNNING, WAITING, ERROR.

## Tensions

- **Context size vs. information density:** Budgets trim content but may drop important context. The trimming algorithm is greedy (drops largest first) without semantic awareness.

- **Interactive vs. autonomous:** Flows support both, but interactive steps require the daemon to pause and wait for `lfd connect`. State machine complexity grows with flow complexity.

- **Worktree proliferation:** Forks create parallel worktrees for isolation. Cleanup relies on user running `lfops wt prune`. Waves now persist worktrees across iterations, adding another dimension.

- **Python daemon on macOS-only:** The daemon uses launchd integration, limiting platform portability. The Rust rewrite doc (`rust-lfd.md`) explores alternatives but hasn't been actioned.

## Observations

### Complexity

**`context.py` (1100+ lines):** The context assembly system handles many cases—areas, waves, summaries, clipboard, images, git diff modes. `gather_prompt_components()` at line 808 orchestrates everything. The token trimming logic (lines 237-284) uses a greedy algorithm that could benefit from documentation explaining priority choices.

**`flow.py` (1200+ lines):** Flow execution handles linear, fork, synthesize, and choose patterns. The `run_flow()` function (lines 970-1175) is the main entry point. Fork execution (lines 558-646) creates worktrees and runs steps in parallel—this is the most complex code path.

**`lfd/daemon/server.py`:** 30+ request handlers with mixed responsibilities. The periodic check loop and pub/sub system are clean, but the handler registry is implicit (method naming convention).

### Quality

**Tests:** 629 test functions across 33 files. Good coverage on parsing (`test_flows.py`, `test_context.py`, `test_frontmatter.py`). `test_lfd.py` has 79 tests covering daemon operations. Notable gap: no dedicated tests for `flow.py:run_fork()` or `run_synthesize()`.

**Documentation:** Excellent user-facing docs in `docs/`. CLAUDE.md and STYLE.md provide clear guidance. `roadmap/` has design docs for future work (enterprise, Rust port). Some internal docs are dated—`lfd-reliability.md` lists phases, but phase 6 (transaction boundaries) remains TODO.

**Error messages:** CLI error messages are generally clear. `lfops doctor` checks dependencies. The daemon logs to `~/.lf/logs/lfd.log` with full stack traces on exceptions.

### Potential

**Skill system is extensible:** `skills.py` supports external skill sources (superpowers, SkillRegistry) with prefix-based invocation. The architecture allows adding more sources without core changes.

**Protocol is clean:** The JSON-over-newline socket protocol with Request/Response/Event types is well-structured. A Rust port could maintain wire compatibility while improving reliability.

**Wave model is unified:** All stimulus types (once, loop, watch, cron) share the same Wave model. Adding new stimuli would be straightforward.

**DAG execution is general:** The topological batching and fork/synthesize patterns are general enough for complex workflows. The `after:` dependency syntax is intuitive.

## Open questions

- **Wave worktree lifecycle:** With persistent worktrees, what happens when branches conflict with merged PRs? The autoprune logic handles this partially, but edge cases exist.

- **Token budget tuning:** Are the default budgets (50k/30k/20k) optimal? The docs mention "trimming happens during gathering" but don't explain how to tune for specific codebases.

- **Interactive step state:** When a flow pauses at an interactive step, how does Concerto UI know to show a "connect" button vs. "resume"? The event system broadcasts `wave.waiting`, but the UI integration isn't clear from the Python code alone.

## Recommendations

### Document token trimming strategy

**Observation:** `context.py:237-284` trims components by size without explaining the priority order. Comments at lines 450-490 mention dropping `diff_files` before `clipboard` but don't explain why.

**Cost:** Low (comments only, 30 min).

**Benefit:** Future maintainers and LLM agents can make informed context decisions. Users tuning budgets understand trade-offs.

**Verdict:** Worth it—clarity investment that prevents future confusion.

### Add fork/synthesize tests

**Observation:** `tests/test_flows.py` tests parsing but not execution. `flow.py:run_fork()` and `run_synthesize()` are tested only through integration flows.

**Cost:** Medium (need to mock worktree creation and parallel execution).

**Benefit:** Confidence that the most complex code path works correctly. Regression protection for fork behavior.

**Verdict:** Worth it—fork is a key feature used in roadmap flows.

### Clean up worker.py

**Observation:** `lfd/execution/worker.py` exists alongside `runner.py`. The relationship isn't documented. One may be unused or redundant.

**Cost:** Low (investigate, then delete or document).

**Benefit:** Reduced confusion about execution architecture.

**Verdict:** Worth investigating—either remove dead code or clarify the split.

### Route choose_branch() through standard execution

**Observation:** `flow.py:choose_branch()` (lines 420-450) calls the runner directly rather than going through the standard step execution path. This bypasses logging and error handling.

**Cost:** Low (isolated function refactor).

**Benefit:** Consistent execution semantics across all flow patterns.

**Verdict:** Worth it if choose is used in production flows.
