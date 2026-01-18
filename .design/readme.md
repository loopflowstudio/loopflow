# README Restructure

## What to build

A streamlined README that hooks readers fast, shows immediate value, and delegates depth to docs.

## Research Summary

Analyzed READMEs from Codex, TensorFlow, Letta, and Stripe. Common patterns:

| Pattern | Why it works |
|---------|--------------|
| Hero + one-liner | Instant understanding of what this is |
| Install first | Immediate actionability |
| Demo gif/code | Show don't tell |
| Progressive disclosure | Start simple, link for depth |
| Scannable sections | Users skim before reading |

**What the best READMEs avoid:**
- Reference documentation (belongs in docs site)
- Exhaustive option tables
- Deeply nested sections
- Trying to be everything to everyone

## Current README Problems

1. **Too comprehensive** - 300+ lines trying to be both intro and reference
2. **Buried value prop** - The "why" is after install and quick start
3. **Redundant with docs** - Options table, Commands table duplicate docs/lf.md
4. **No visual hook** - Demo gifs exist but aren't prominently featured

## Proposed Structure

```markdown
# Loopflow

One-line value prop.

[badges if we want them]

[hero gif - the debug demo]

## Install

pip install loopflow
lfops install

## Quick Start

Three examples showing immediate value:
1. Debug (paste error, fix) - our killer demo
2. Design + implement (interactive → autonomous)
3. Ship pipeline

## How It Works

Brief explanation: tasks are markdown, context is assembled, agents execute.
Link to docs for depth.

## Documentation

Links to key docs pages:
- Getting Started
- Built-in Tasks
- Configuration
- Patterns

## Requirements

macOS. Claude Code, Codex, or Gemini.

## License
```

## Key Decisions

### Lead with demo gif
"Show a 15-second gif of `lf debug -v` fixing a bug. This is our best hook."

Note: `docs/demo.gif` exists but docs/index.md references `debug-demo.gif` and `design-demo.gif` which don't exist. Use the existing `demo.gif` or create properly named variants.

### Remove reference material
"The Options table, Commands table, full Configuration section, Run Modes section, Voices section, Background Agents section, Session Tracking section - all of this belongs in docs, not README."

### Keep Why Worktrees brief
The current explanation is good but could be 2 sentences with a link.

### Three-example Quick Start
1. **Debug** - paste error, watch it fix (lowest friction)
2. **Build a feature** - design → implement → polish (the core workflow)
3. **Pipeline** - `lf ship` (shows chaining)

## Data Structures

N/A - this is documentation restructuring.

## Key Functions

N/A - no code changes.

## Constraints

- Must keep demo gifs working (they're in docs/)
- Must not break any existing links (we're restructuring, not removing docs)
- README should be under 150 lines
- Every section except Install should have a "learn more" link to docs

## Done When

```bash
# README is shorter
wc -l README.md  # < 150 lines

# Demo gif is visible
grep -q "demo" README.md

# Links to docs work
grep -E "\[.*\]\(docs/" README.md | head -5
```

Manual verification: README reads well as a landing page, not a manual.
