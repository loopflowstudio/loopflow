# Loopflow Codebase Summary

## Core Architecture

**Loopflow** is a Python CLI tool that orchestrates LLM coding agents (Claude Code, Codex, Gemini CLI) through prompt chains. Each prompt inherits state from the previous step and hands off cleanly to the next.

### Key Design Principles
- **Tight loops**: Do one thing, hand off cleanly
- **Auto mode** (default): Non-interactive execution with output streaming and logging
- **Interactive mode** (`-i`): Full chat interface with Ctrl+C termination
- **Git worktree-based**: Each feature gets isolated directory via `wt` CLI

## Data Structures

### Core Types

```python
# Configuration
@dataclass
class Config(BaseModel):
    agent_model: str = "claude:opus"  # backend:variant format
    pipelines: dict[str, PipelineConfig]
    yolo: bool = False  # Skip permission prompts
    push: bool = False
    pr: bool = False
    context: list[str]  # Default files/directories
    exclude: list[str]  # Patterns to ignore
    interactive: list[str]  # Tasks defaulting to interactive
    voice: Optional[list[str]] = None  # Default voices
```

```python
# Prompt Assembly
@dataclass
class PromptComponents:
    run_mode: str | None
    docs: list[tuple[Path, str]]  # Repo documentation
    diff: str | None  # Raw branch diff
    diff_files: list[tuple[Path, str]]  # Files + explicit context
    task: tuple[str, str] | None  # (name, content)
    repo_root: Path
    clipboard: str | None
    loopflow_doc: str | None  # Bundled LOOPFLOW.md
    voices: list[Voice] | None  # Persona overlays
```

```python
# Task Configuration (frontmatter)
@dataclass
class TaskConfig:
    interactive: bool | None = None
    include: list[str] | None = None  # Override excludes
    exclude: list[str] | None = None
    model: str | None = None
    voice: list[str] | None = None
```

```python
# Session Tracking (lfd daemon)
@dataclass
class Session:
    id: str  # UUID
    task: str
    repo: str  # Main repo path
    worktree: str  # Current worktree path
    status: SessionStatus  # running, completed, error
    started_at: datetime
    model: str  # claude, codex, gemini
    run_mode: Literal["auto", "interactive"]
```

```python
# Pipeline Definitions
@dataclass
class PipelineStep:
    task: str | None = None
    pipeline: str | None = None  # Nested pipeline
    parallel: list["PipelineStep"] | None = None
    race: RaceConfig | None = None  # Multi-model competition
    config: StepConfig | None = None  # Per-step overrides
```

## Public APIs

### Main Commands

```python
# Core execution
def run(task: str, auto: bool, interactive: bool, model: str) -> int
# Run task with LLM model

def inline(prompt: str, auto: bool, model: str) -> int  
# Run inline prompt

def pipeline(name: str, worktree: str, model: str) -> int
# Execute named pipeline

def cp(paths: list[str], exclude: list[str]) -> None
# Copy file context to clipboard
```

### Context Gathering

```python
def gather_prompt_components(
    repo_root: Path,
    task: Optional[str] = None,
    inline: Optional[str] = None,
    context: Optional[list[str]] = None,
    exclude: Optional[list[str]] = None,
    voices: Optional[list[str]] = None,
) -> PromptComponents
# Assemble all prompt components

def format_prompt(components: PromptComponents) -> str
# Format components into final prompt string
```

### Model Runners

```python
class Runner(ABC):
    @abstractmethod
    def launch(self, prompt: str, auto: bool, stream: bool) -> LaunchResult
    
    @abstractmethod  
    def is_available(self) -> bool

def get_runner(model: str) -> Runner
# Factory for ClaudeRunner, CodexRunner, GeminiRunner
```

### Worktree Management

```python
def create(repo_root: Path, name: str, base: str) -> Path
# Create worktree for new branch

def list_all(repo_root: Path) -> list[Worktree] 
# List worktrees with PR status, diff stats

def remove(repo_root: Path, name: str) -> bool
# Remove worktree and branch
```

## Key Patterns

### Prompt Chain Architecture

Tasks are **single-purpose** and chain via shared state:

| Prompt | Requires | Produces |
|--------|----------|----------|
| `design` | — | `.design/<branch>.md` |
| `implement` | `.design/<branch>.md` | code, tests |
| `polish` | code on branch | passing tests |
| `review` | code on branch | verdict in `.design/` |

Common sequences:
- `design → implement → polish`
- `review → iterate → polish`

### File Structure

```
.lf/
  config.yaml      # Repo configuration
  <task>.lf        # Prompt files
  voices/          # Persona definitions
  pipelines/       # Pipeline YAML definitions

.claude/commands/   # Claude Code compatible prompts
  <task>.md

.design/
  <branch>.md      # Design doc for current branch  
  questions.md     # Open questions from auto runs
```

### Configuration Hierarchy

**CLI > frontmatter > global > defaults**

```yaml
# .lf/config.yaml
agent_model: claude:opus
context: [src, tests]
exclude: ["*.lock", node_modules]
interactive: [design, iterate]
```

```markdown
<!-- .claude/commands/implement.md -->
---
interactive: false
model: codex:o3
include: 
  - tests/**  # Override global exclude
---
Turn design into working code...
```

### Session Management (lfd daemon)

**Fire-and-forget pattern**: Session logging uses 0.5s timeout, fails silently if daemon unavailable.

```python
# Auto mode execution flow
session = Session(id=uuid4(), task=task_name, status=RUNNING)
log_session_start(session)  # Fire-and-forget to lfd

collector_cmd = [
    "python", "-m", "loopflow.lfd.collector",
    "--session-id", session.id,
    "--autocommit",  # Git add + commit + push
    "--", *model_command
]
process = subprocess.Popen(collector_cmd)
exit_code = process.wait()

log_session_end(session.id, status)  # Fire-and-forget
```

### Model Command Building

```python
# Model-agnostic command construction
def build_model_command(model: str, auto: bool, stream: bool) -> list[str]
# Returns CLI args for claude/codex/gemini

# Claude: ["claude", "--print", "--dangerously-skip-permissions"]
# Codex: ["codex", "exec", "--json", "--sandbox", "workspace-write"] 
# Gemini: ["gemini", "--output-format", "stream-json", "--yolo"]
```

### Token Analysis

```python
@dataclass
class TokenTree:
    root: TokenNode
    
    def add(self, category: str, name: str, tokens: int, path: list[str]) -> None
    def format(self, threshold_pct: float = 0.05) -> str
    # Adaptive detail based on size

def analyze_components(components: PromptComponents) -> TokenTree
# Hierarchical breakdown: loopflow > docs > files > task
```

## Direct Implementation Details

### Prompt Format Structure

```
# Auto mode header (if applicable)
Run mode is auto (headless). Proceed without pausing...

# System documentation  
<lf:loopflow>
{bundled LOOPFLOW.md}
</lf:loopflow>

# Task with voices
The task.

<lf:voice:architect>
{voice content}
</lf:voice:architect>

<lf:task:implement>
{task content with {{template}} substitution}
</lf:task:implement>

# Repository context
Repository documentation. Follow STYLE carefully.

<lf:docs>
<lf:README>...</lf:README>
<lf:STYLE>...</lf:STYLE>
</lf:docs>

# Branch changes
<lf:diff>
{git diff main...HEAD}
</lf:diff>

# Files (diff files + explicit context, deduplicated)
<lf:files>
<lf:file path="src/cli.py">...</lf:file>
</lf:files>

# Clipboard content (if -v flag)
<lf:clipboard>
{pbpaste output}
</lf:clipboard>
```

### CLI Entry Points

```bash
# Main binary: lf
lf <task>           # Auto-detect task vs pipeline
lf <task> -i        # Interactive mode
lf : "inline prompt" # Inline execution
lf cp src tests -v  # Copy context to clipboard

# Operations: lfops  
lfops init          # Scaffold .lf/ + .claude/commands/
lfops install       # Install claude, codex, worktrunk via brew
lfops doctor        # Check dependencies
lfops pr -a         # Create PR (add, commit, push first)
lfops land -w <wt>  # Squash-merge to main via GitHub or local

# Worktrees: lfwt
lfwt list           # Show all worktrees + PR status
lfwt diff <branch>  # Open diff in Cursor or GitHub
lfwt compare a b    # LLM analysis of two implementations

# Daemon: lfd  
lfd install         # Install launchd service
lfd status          # Show daemon + agent status
lfd new <agent>     # Create agent definition
```

### Model Integration Paths

**API Integration** (structured responses):
```python
# llm_http.py - For commit/PR messages only
agent = Agent("anthropic:claude-sonnet-4-20250514", output_type=CommitMessage)
result = agent.run_sync(prompt)
# Falls back to CLI if API unavailable
```

**CLI Integration** (interactive + auto modes):
```bash
# Environment strips API keys so CLIs use subscriptions
claude --print --dangerously-skip-permissions "prompt"
codex exec --json --sandbox workspace-write "prompt"  
gemini --output-format stream-json --yolo "prompt"
```

### Pipeline Execution Patterns

**Sequential**: One task after another in same worktree
**Parallel**: Multiple tasks in temporary worktrees, merged at completion
**Race**: Same task with different models, judge picks winner

```yaml
# .lf/pipelines/compete.yaml
steps:
  - race:
      models: [claude:opus, codex:o3, gemini:2.5-pro]
      judge: compare  # Built-in judge prompt
    task: implement
```

The codebase emphasizes **rapid iteration** with **minimal ceremony**—tight integration with existing git workflows while providing powerful orchestration of multiple AI coding agents.