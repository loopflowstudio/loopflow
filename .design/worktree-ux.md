# Worktree UX

**What to build:** Split loopflow into focused CLIs and enhance Maestro UI with worktree/PR actions.

## User quotes

> "Better wt integration, capabilities for worktrees in Maestro. create/view PR, land, diff with other worktree, diff with main"

> "lfpr - landing and github integration. lfwt - wt functionality above wt. lfd - background agents and monitoring. lf - prompting cli only"

> "lfops - internal/meta lf stuff, then we can get rid of ops"

## Context

Today loopflow bundles everything in `lf ops`. This design splits into focused tools:

| CLI | Purpose | Current location |
|-----|---------|------------------|
| `lf` | Prompting only - run tasks, pipelines | `lf run`, `lf pipeline` |
| `lfd` | Background agents, daemon, monitoring | `lfd` (already exists) |
| `lfwt` | Worktree operations above `wt` | NEW |
| `lfpr` | GitHub PRs, landing, merge workflows | `lf ops pr`, `lf ops land` |
| `lfops` | Internal/meta: init, install, doctor | `lf ops init/install/doctor` |

This branch:
1. Creates `lfwt` (new)
2. Creates `lfpr` (moves from `lf ops pr`)
3. Creates `lfops` (moves from `lf ops`)
4. Removes `lf ops` subcommand
5. Adds worktree/PR endpoints to Maestro API

---

## Data Structures

Extend existing `Worktree` dataclass to include PR state:

```python
# src/loopflow/worktrees.py

@dataclass
class Worktree:
    # ... existing fields ...
    pr_state: str | None = None  # "open", "merged", "closed", "draft"
```

---

## Key Functions

### `lfwt` - Worktree operations

```python
# src/loopflow/lfwt.py

app = typer.Typer(help="Worktree operations")

@app.command(name="list")
def list_cmd() -> None:
    """Show all worktrees with status and PR info."""

@app.command()
def diff(
    target: str = typer.Argument(..., help="Worktree or branch to diff"),
    other: str = typer.Argument(None, help="Second worktree/branch (optional)"),
    base: str = typer.Option("main", "--base", "-b"),
    web: bool = typer.Option(False, "-w", "--web", help="Open GitHub compare view"),
    terminal: bool = typer.Option(False, "-t", "--terminal", help="Print to terminal"),
) -> None:
    """Show diff for a worktree.

    lfwt diff feature-a           # opens in Cursor (or configured IDE)
    lfwt diff feature-a -w        # opens GitHub compare view in browser
    lfwt diff feature-a -t        # prints to terminal
    lfwt diff feature-a feature-b # diff between worktrees
    """
    # Default: open in IDE (Cursor)
    # -w/--web: open https://github.com/org/repo/compare/main...feature-a
    # -t/--terminal: git diff to stdout

@app.command()
def compare(a: str, b: str, ...) -> None:
    """Compare two implementations and analyze differences."""
    # Move from lf ops compare

@app.command()
def cd(name: str) -> None:
    """Print path to worktree (for shell integration)."""

def main() -> None:
    if len(sys.argv) == 1:
        sys.argv.append("list")
    app()
```

### `lfpr` - PR and landing operations

```python
# src/loopflow/lfpr.py

app = typer.Typer(help="Pull request operations")

@app.command()
def create() -> None:
    """Create PR for current branch."""
    # Move logic from lf ops pr create

@app.command()
def view() -> None:
    """Open PR in browser."""

@app.command()
def update() -> None:
    """Update PR title/body from branch changes."""
    # Move logic from lf ops pr update

@app.command()
def land(
    auto: bool = typer.Option(False, "-a", "--auto", help="Auto-merge when checks pass"),
) -> None:
    """Land PR (squash-merge to base branch)."""
    # Move logic from lf ops pr land

@app.command()
def commit(
    push: bool = typer.Option(False, "-p", "--push"),
    add: bool = typer.Option(True, "-a/-A", "--add/--no-add"),
) -> None:
    """Generate commit message and commit."""
    # Move from lf ops commit

def main() -> None:
    if len(sys.argv) == 1:
        sys.argv.append("view")  # Default: open current PR
    app()
```

### `lfops` - Meta/internal operations

```python
# src/loopflow/lfops.py

app = typer.Typer(help="Loopflow meta operations")

@app.command()
def init(
    prompts_only: bool = typer.Option(False, "--prompts"),
    style_only: bool = typer.Option(False, "--style"),
    all_prompts: bool = typer.Option(False, "--all"),
    yes: bool = typer.Option(False, "--yes", "-y"),
) -> None:
    """Initialize repo with loopflow."""
    # Move from lf ops init

@app.command()
def install() -> None:
    """Install loopflow dependencies (Claude, Codex, worktrunk, etc)."""
    # Move from lf ops install

@app.command()
def doctor() -> None:
    """Check loopflow dependencies and repo status."""
    # Move from lf ops doctor

@app.command()
def version() -> None:
    """Show loopflow version."""

@app.command()
def status() -> None:
    """Show running sessions."""
    # Move from lf ops status

def main() -> None:
    if len(sys.argv) == 1:
        sys.argv.append("doctor")
    app()
```

### `lf` - Simplified (prompting only)

```python
# src/loopflow/cli/__init__.py (simplified)

app = typer.Typer(help="Run LLM tasks")

# Keep only:
app.command()(run_module.run)      # lf <task>
app.command()(run_module.inline)   # lf : "prompt"
app.command()(run_module.pipeline) # lf <pipeline>
app.command()(run_module.cp)       # lf cp

# Remove:
# - app.add_typer(ops.app)  # was lf ops
```

### Entry points in pyproject.toml

```toml
[project.scripts]
lf = "loopflow.cli:main"
lfd = "loopflow.lfd:main"
lfwt = "loopflow.lfwt:main"
lfpr = "loopflow.lfpr:main"
lfops = "loopflow.lfops:main"
```

### Worktree module extensions

```python
# src/loopflow/worktrees.py (add)

def get_pr_state(repo_root: Path, branch: str) -> str | None:
    """Return PR state using gh pr view --json state."""

def diff_against(repo_root: Path, branch: str, base: str = "main") -> str:
    """Get diff of branch against base."""

def diff_between(repo_root: Path, branch_a: str, branch_b: str) -> str:
    """Get diff between two branches."""
```

### Maestro Swift App - WorktreeRow enhancements

```swift
// Maestro/Maestro/Views/WorktreeSidebar.swift

struct WorktreeRow: View {
    // ... existing properties ...
    let onCreatePR: () -> Void
    let onLand: () -> Void
    let onRunPipeline: (String) -> Void  // pipeline name

    // Hover actions: PR/Land + pipeline launcher
    private var hoverActions: some View {
        HStack(spacing: 8) {
            // Pipeline picker (dropdown or menu)
            Menu {
                ForEach(availablePipelines) { pipeline in
                    Button(pipeline.name) { onRunPipeline(pipeline.name) }
                }
            } label: {
                Image(systemName: "play.fill")
            }

            // PR actions
            if worktree.prURL == nil {
                Button("Create PR") { onCreatePR() }
            } else {
                Button("Land") { onLand() }
            }
        }
        .opacity(isHovering ? 1 : 0)
    }
}
```

### WorktreeService - Add PR/Land/Pipeline actions

```swift
// Maestro/Maestro/Services/WorktreeService.swift

func createPR(for worktree: Worktree) async throws -> URL {
    // Run: lfpr create (in worktree.path)
    // Parse URL from output
}

func landWorktree(_ worktree: Worktree) async throws {
    // Run: lfpr land (in worktree.path)
}

func getDiff(for worktree: Worktree, base: String = "main") async throws -> String {
    // Run: lfwt diff worktree.branch --base base
}

func runPipeline(_ name: String, in worktree: Worktree) async throws {
    // Run: lf <name> (in worktree.path)
    // Opens terminal or streams output to panel
}

func availablePipelines(for worktree: Worktree) -> [String] {
    // Read .lf/config.yaml from worktree, return pipeline names
    // e.g., ["ship", "review", "polish"]
}
```

### Worktree model - Add prState

```swift
// Maestro/Maestro/Models/Worktree.swift

struct Worktree: Identifiable, Codable, Hashable {
    // ... existing fields ...
    let prState: String?  // "open", "merged", "closed", "draft"
}
```

---

## Migration

Move existing code:
- `src/loopflow/cli/pr.py` → `src/loopflow/lfpr.py`
- `src/loopflow/cli/land.py` → merge into `lfpr.py`
- `src/loopflow/cli/meta.py` → `src/loopflow/lfops.py`
- `src/loopflow/cli/status.py` + `sessions.py` → merge into `lfops.py`
- `src/loopflow/cli/compare.py` → `src/loopflow/lfwt.py` (as `lfwt compare`)
- `src/loopflow/cli/commit.py` → `src/loopflow/lfpr.py` (as `lfpr commit`)

Delete:
- `src/loopflow/cli/ops.py`

Hard cut - no deprecation aliases.

---

## Constraints

1. **Delegate to wt.** `lfwt` adds value above `wt`, doesn't replace it. Use `wt` for create/remove/switch. `lfwt` is for viewing status and diffs.

2. **PR state via gh CLI.** Use `gh pr view --json state`. No GitHub API dependencies.

3. **Default commands.** `lfwt` → `lfwt list`. `lfpr` → `lfpr view`. `lfops` → `lfops doctor`.

4. **Works from any worktree.** Commands detect main repo and work regardless of which worktree you're in.

5. **No inter-CLI dependencies.** Each CLI is standalone. `lfpr` doesn't call `lfwt`, etc.

6. **Diff opens in IDE by default.** `lfwt diff` respects `ide` config from `.lf/config.yaml`. Cursor is default. Use `-w` for GitHub, `-t` for terminal.

---

## Done When

### CLI

```bash
# lfwt: list worktrees (default command)
lfwt
# BRANCH              STATUS      PR
# main                0↑ 0↓       -
# feature-a           5↑ 0↓ dirty #123 open
# feature-b           2↑ 1↓       #124 merged

# lfwt: diff against main (opens in Cursor by default)
lfwt diff feature-a
# Opens worktree in Cursor, Source Control shows diff

# lfwt: diff in GitHub
lfwt diff feature-a -w
# Opens https://github.com/org/repo/compare/main...feature-a

# lfwt: diff to terminal
lfwt diff feature-a -t
# Prints git diff to stdout

# lfwt: diff between worktrees
lfwt diff feature-a feature-b
# Opens diff in Cursor (generates .diff file)

# lfpr: create PR
lfpr create
# Creates PR, prints URL

# lfpr: view current PR (default command)
lfpr
# Opens PR in browser (or prints "no PR")

# lfpr: land PR
lfpr land
# Squash-merges to base, cleans up

# lfops: check status (default command)
lfops
# Shows doctor output

# lfops: initialize repo
lfops init
# Creates .lf/, .claude/commands/

# lfops: install dependencies
lfops install
# Installs Claude, worktrunk, etc.

# lf: still works for prompting
lf review
lf implement: add feature

# lf ops: REMOVED (commands moved to lfops, lfpr)
```

### Maestro Swift App

- WorktreeRow shows PR state (open/merged/closed/draft)
- Hover reveals action buttons:
  - Play button → pipeline picker menu (ship, review, etc.)
  - "Create PR" (if no PR) or "Land" (if PR open)
- Clicking pipeline runs `lf <pipeline>` in worktree (opens terminal)
- Clicking "Create PR" runs `lfpr create` in worktree, shows URL
- Clicking "Land" runs `lfpr land` in worktree, refreshes list
- Context menu includes "View Diff" that opens diff in Cursor
