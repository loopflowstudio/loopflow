# Rust lfd: Tradeoffs Exploration

Should the loopflow daemon (`lfd`) be rewritten in Rust?

## What to build

A Rust implementation of `lfd` that handles daemon responsibilities (socket server, database, scheduling, process management) while delegating prompt assembly and agent launching to the existing Python `lf` CLI.

## Current architecture

```
lfd (Python daemon)
├── Unix socket server (asyncio)
├── SQLite database (sessions, runs, triggers)
├── Manager (concurrency slots, PR limits)
├── Periodic checks (triggers, autoprune, draft PRs)
└── Execution (heavily coupled to lf internals)
    ├── Context assembly (format_prompt, gather_prompt_components)
    ├── Flow loading (load_flow, FlowStep)
    ├── Agent launching (build_model_command, get_runner)
    ├── Worktree management (create, remove, find_worktree_root)
    └── Git operations (autocommit, find_main_repo)
```

The daemon has two distinct parts:
1. **Infrastructure** (server, db, scheduling) — language-agnostic
2. **Execution** (run iterations) — deeply coupled to Python lf

## Architecture options

### Option A: Full Rust rewrite

Reimplement everything in Rust, including prompt assembly and agent launching.

**Requires:**
- Duplicate all context assembly logic
- Reimplement flow/step loading
- Reimplement goal rendering
- Maintain two implementations forever

**Verdict:** Too expensive. The coupling exists for good reason—lfd *orchestrates* lf's capabilities.

### Option B: Rust daemon, shell to lf (Recommended)

Rust handles infrastructure. Execution shells out to `lf` commands.

```
lfd (Rust)
├── Unix socket server (tokio)
├── SQLite database (rusqlite)
├── Manager (concurrency, PR limits)
├── Periodic checks
└── Execution: shell to `lf --flow <name> --worktree <path>`
```

**Requires:**
- New `lf` flags for daemon-controlled execution
- Protocol for lf to report progress back
- Rust daemon manages worktree lifecycle, lf does the work

### Option C: Keep Python, optimize

Fix the actual problems (if any) without rewriting.

## Arguments for Rust

### 1. Daemon stability
A daemon should be rock-solid. Rust's memory safety and lack of GC pauses could improve reliability for something that runs 24/7.

### 2. Resource efficiency
Python's asyncio works but has overhead. A Rust daemon would use less memory and CPU while idle—important for something always running.

### 3. Startup time
`lfd serve` starts faster in Rust. Matters for launchd restarts.

### 4. Distribution
A single static binary is easier to distribute than a Python package with dependencies. Could `brew install lfd` separately from `uv tool install loopflow`.

### 5. Version decoupling
Forces clean separation: daemon updates rarely (stable infrastructure), CLI updates often (evolving features). This is arguably *better* than the current forced sync.

### 6. Type safety for protocol
The JSON-over-newline protocol with Request/Response/Event would benefit from Rust's type system. Harder to make protocol mistakes.

## Arguments against Rust

### 1. Team expertise
If the team is Python-native, a Rust component adds maintenance burden. Every contributor needs to know Rust.

### 2. Debugging complexity
When lfd shells to lf and something breaks, debugging crosses language boundaries. Logs, stack traces, and profiling become harder.

### 3. The coupling is deep
Looking at `execution/runner.py`, lfd doesn't just call lf—it reaches into:
- `format_prompt`, `gather_prompt_components` (context assembly)
- `load_flow`, `FlowStep`, `evaluate_flow_result` (flow logic)
- `build_model_command`, `get_runner` (agent launching)
- Worktree creation, git autocommit, PR messages

Shelling to `lf` requires exposing all this via CLI flags or a new protocol.

### 4. Build complexity
Two build systems (uv + cargo), two test suites, two CI pipelines. Release coordination becomes harder even with version decoupling.

### 5. macOS-only anyway
lfd only runs on macOS (launchd integration). The "single binary distribution" argument is weaker when the target is narrow.

### 6. The problems might not exist
Is the Python daemon actually causing issues? If it's working fine, a rewrite is premature optimization.

## The version sync question

**Current state:** lf and lfd share a version because they're one Python package.

**Tradeoff:**
- Good: Single install, guaranteed compatibility, shared code
- Bad: Coupled releases, different stability profiles

**If Rust:** Forces decoupling. Installation becomes two steps:
```bash
uv tool install loopflow  # lf CLI
brew install lfd          # daemon
```

Need a compatibility contract—likely the socket protocol version. As long as the protocol is stable, lf and lfd can version independently.

This might actually be *better*. The daemon should change rarely (it's infrastructure). The CLI changes often (new steps, context options, models). Coupling them forces daemon releases when only the CLI changed.

## What would the Rust daemon need?

If we go with Option B:

### Core daemon (Rust)
```rust
// Protocol types
struct Request { method: String, params: Value, id: Option<String> }
struct Response { ok: bool, result: Value, error: Option<String>, id: Option<String> }
struct Event { event: String, data: Value }

// Database models
struct Session { id: String, step: String, worktree: String, status: SessionStatus, ... }
struct Loop { id: String, area: String, flow: String, status: TriggerStatus, ... }
struct Run { id: String, trigger_id: String, status: RunStatus, ... }

// Manager
struct Manager { slots: HashSet<String>, max_slots: usize, max_prs: usize }
```

### New lf interface
```bash
# lf needs flags for daemon-controlled execution
lf --flow ship --worktree /path/to/wt --run-id abc123 --report-to ~/.lf/lfd.sock
```

The `--report-to` flag makes lf send progress events to the daemon socket instead of just running standalone.

### Migration path
1. Ship Rust lfd alongside Python lfd
2. Environment variable or config to choose
3. Deprecate Python lfd after Rust proves stable

## Constraints

- Must maintain socket protocol compatibility (Concerto depends on it)
- Must handle all current trigger types (loop, subscription, schedule)
- Must integrate with launchd for auto-start
- Execution must remain correct (lf does the actual work)

## Done when

If we proceed:
```bash
# Install separately
brew install lfd
uv tool install loopflow

# Daemon starts via launchd
lfd install

# Same commands work
lfd loop ship src/
lfd status

# Protocol unchanged—Concerto still works
```

## Open questions

1. **Is there a real problem?** What's actually wrong with Python lfd that Rust would fix?

2. **Concurrency model:** tokio vs async-std vs threads? For a daemon this simple, might not matter.

3. **Database sharing:** Can Rust and Python both access ~/.lf/lfd.db safely? SQLite WAL mode helps, but need to verify.

4. **Protocol versioning:** How do we handle protocol changes when lf and lfd version independently?

5. **Feature parity:** The Python daemon has evolved organically. A Rust rewrite needs to match all features or explicitly deprecate some.
