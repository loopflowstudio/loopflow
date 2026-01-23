# lfops next: Land and Continue

Atomic operation: land current worktree, create fresh one in the same "space."

## What to build

`lfops next` — merges the current PR, waits for it to land, cleans up the worktree, and creates a new one preserving the working context (branch prefix, area, voice).

## The "space" concept

A space is the preserved context across iterations. It's essentially an Agent minus the trigger machinery:

```python
@dataclass
class Space:
    repo: Path
    branch_prefix: str   # e.g., "jack.auth"
    area: list[str]      # optional, for context
    voice: list[str]     # optional, for prompts
    flow: str | None     # optional, if chaining steps
```

For manual use, space is **inferred from the current branch**:
- `jack.auth.20260123_1112` → prefix is `jack.auth`, suffix is `20260123_1112`
- Area/voice/flow come from `.lf/config.yaml` or CLI flags

## Two modes

### 1. Manual (user-invoked)

```bash
# Working on jack.auth.20260123_1112
lfops next                      # land PR, create ...-aurora-melody
lfops next --no-wait            # submit to queue, don't wait for merge
```

The command:
1. Runs `lfops land` (submit to merge queue)
2. Waits for merge (or fire-and-forget with `--no-wait`)
3. Creates new worktree with same base + magical-musical suffix
4. Opens terminal in new worktree

### 2. Loop primitive (for lfd)

`lfd loop` currently has `run_iteration` which:
- Creates worktree with numbered branch (`prefix/001`)
- Runs flow
- Creates PR to personal-main, auto-merges
- Cleans up worktree
- Returns

These are **different flows**:
- Manual next: PR → real main, wait for CI, then continue
- Loop iteration: instant merge to personal-main, batch-land later

**Don't unify the core.** But **do share naming utilities** — the word lists and branch generation move to `loopflow/lf/naming.py`.

## Branch naming

Current schema: `{user}.{name}.{date}_{ts}`

For cycles, append a magical-musical word pair (same scheme as agent main branches):

- First: `jack.auth.20260123_1112`
- Cycle 1: `jack.auth.20260123_1112-aurora-melody`
- Cycle 2: `jack.auth.20260123_1112-frost-cadence`

Word lists already exist in `src/loopflow/lfd/agent.py`:
- `MAGICAL`: aurora, cascade, crystal, drift, echo, ember, fern, frost, glade, ...
- `MUSICAL`: allegro, aria, ballad, cadence, canon, chord, coda, duet, forte, ...

Use `_generate_random_words()` which returns `f"{magical}-{musical}"`.

**Move word lists to shared location** — currently in `agent.py`, should be in a shared module like `loopflow.lf.naming` so both `cycle` and agent creation can use them.

## Key decisions

**Wait for merge?** Default: fire-and-forget (non-blocking). `--block` to wait for merge.

**What if CI fails?** Abort cycle, stay in current worktree. User fixes and retries.

**Run a step in the new worktree?** No. Just create and open. User decides what to do next. (They can `lf design: next chunk` or whatever.)

## Data structures

```python
# Move to loopflow/lf/naming.py (new module)

MAGICAL = ["aurora", "cascade", "crystal", ...]  # from agent.py
MUSICAL = ["allegro", "aria", "ballad", ...]     # from agent.py

def generate_word_pair() -> str:
    """Generate magical-musical pair like 'aurora-melody'."""
    return f"{random.choice(MAGICAL)}-{random.choice(MUSICAL)}"

def parse_branch_base(branch: str) -> str:
    """Extract base branch name for next iteration.

    If branch ends with magical-musical pair, strip it.
    Otherwise use as-is (first iteration).

    'jack.auth.20260123_1112' → 'jack.auth.20260123_1112'
    'jack.auth.20260123_1112-aurora-melody' → 'jack.auth.20260123_1112'
    """

def generate_next_branch(base: str, repo: Path) -> str:
    """Generate unique branch name for next iteration.

    Appends magical-musical pair, retries if exists.
    """
    for _ in range(100):
        candidate = f"{base}-{generate_word_pair()}"
        if not branch_exists(repo, candidate):
            return candidate
    raise ValueError(f"Could not generate unique branch from {base}")
```

Then update `agent.py` to import from `naming.py` instead of defining locally.

## Key function

```python
def next_worktree(
    repo: Path,
    branch: str,
    block: bool = False,        # wait for merge
    open_terminal: bool = True,
) -> Path:
    """Land current branch, create new worktree with magical-musical suffix.

    Returns path to new worktree.
    """
```

## CLI

```bash
lfops next                     # land + create next (aurora-melody, etc.)
lfops next --block             # wait for merge before creating worktree
lfops next --no-open           # don't open terminal
lfops next --create-pr         # create PR if none exists, then next
```

## UI changes

**CLI output (default, non-blocking):**
```
Enabling auto-merge for PR #42...
Creating worktree jack.auth.20260123_1112-aurora-melody...
Opening terminal...
```

**CLI output (with --block):**
```
Enabling auto-merge for PR #42...
Waiting for merge... (Ctrl+C to continue without waiting)
PR #42 merged.
Removing worktree jack.auth.20260123_1112...
Creating worktree jack.auth.20260123_1112-aurora-melody...
Opening terminal...
```

**No Concerto changes needed** — next is a terminal operation.

## Worktree cleanup

After merge completes:
1. Old worktree is removed (same as `lfops wt prune` would do)
2. New worktree is created
3. User ends up in new worktree

Without `--block`: old worktree stays until user runs `lfops wt prune` manually (or merge completes and you prune later).

## Constraints

- Must be on a feature branch (not main)
- PR must exist (use `lfops pr` first, or `--create-pr` flag)
- Current branch becomes the "base" for naming (no strict schema required)

## Done when

```bash
# Starting on jack.auth.20260123_1112
lfops next

# Output:
# Submitting PR #42 to merge queue...
# Creating worktree jack.auth.20260123_1112-aurora-melody
# Opening terminal...

pwd  # /Users/jack/src/loopflow.jack.auth.20260123_1112-aurora-melody

# With --block:
lfops next --block
# Submitting PR #42 to merge queue...
# Waiting for merge... done
# Removing worktree jack.auth.20260123_1112...
# Creating worktree jack.auth.20260123_1112-aurora-melody...
# Opening terminal...
```
