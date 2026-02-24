# Concerto UX Design

High-level user experience principles for the Concerto agent management UI.

## Core Philosophy

**Agents are autonomous workers that occasionally need human attention.**

The UI should make it easy to:
1. See which agents need attention right now
2. Review and approve their work (PRs)
3. Let them continue working autonomously

## Attention Hierarchy

Agents are organized by how urgently they need human attention:

### 1. Needs Attention (Blocked)
Agent is stopped and cannot proceed without human input.
- Interactive step waiting for response
- Error requiring acknowledgment
- **Visual**: Orange/red indicator, shown first in sidebar

### 2. Open PRs (Review Queue)
Agent has work ready for human review. Work can continue (agents commit to agent-main), but PRs are accumulating.
- PR open and awaiting review
- Agent may have hit PR limit
- **Visual**: Green PR badge, second section in sidebar

### 3. Active (Running)
Agent is working autonomously, no attention needed.
- Executing flow steps
- Making commits to agent-main
- **Visual**: Blue running indicator, third section

### 4. Idle (Dormant)
Agent is not running.
- Waiting to be started
- Scheduled but not currently active
- Includes newly created agents (unconfigured)
- **Visual**: Gray/dim indicator, bottom section

New agents start here. The detail panel shows configuration requirements (area not set, etc.) and disables run buttons until configured. No special treatment in sidebar - configuration is handled in the detail panel when selected.

## Agent Creation Flow

**Principle: Start from intent, not configuration.**

1. Click **Start designing**
2. Describe what you want to build
3. Concerto starts a `design` agent session inline with repo-aware context
4. The design session creates and configures the wave; Concerto picks it up on refresh

### Goals
- Inline goals (typed ad-hoc) can be auto-saved after successful runs
- LLM generates descriptive names behind the scenes
- Personal goal library grows through usage

### Flows
- Saved flows are replayable definitions
- Execution history captures what actually happened
- Interactive sessions are harder to capture as replayable flows

## Agent Configuration

Agents have four dimensions, configured in order:

1. **Area** - Which folders the agent works on (required to run)
2. **Goal** - What the agent should accomplish (inline text or preset)
3. **Flow** - Which steps to execute (ship, debug, etc.)
4. **Stimulus** - When to run (once, loop, watch, cron)

Area is required before running. Other dimensions have sensible defaults.

## Sidebar Sections

```
AGENTS
  Needs Attention
    swift-falcon       ⚠ blocked

  Open PRs
    crystal-melody     PR #12

  Active
    api-refactor       ● running

  Idle
    nightly-polish     ○ idle
```

Sections only appear when they have agents. Empty sections are hidden.

## Detail Panel

When an agent is selected:
- **Header**: Name, status, area/flow/stimulus summary
- **Content Section**: Vision, goals, risks, and roadmap progress parsed from `wave/<name>/README.md` and numbered roadmap files
- **Config Section** (idle): Area picker, goal selector, flow picker, run button
- **Progress Section** (active): Current step, live output
- **Files Section**: Changed files with diff stats

## Future Considerations

### Interactive Step Handling
When a flow hits an interactive step:
1. Agent status changes to "blocked"
2. Bubbles to top of sidebar
3. Detail panel shows what input is needed
4. After responding, option to save interaction as reusable step

### Execution History
- Separate concept from flow definitions
- Captures actual runtime behavior
- Useful for debugging, not replay
- Interactive session history is particularly hard to preserve meaningfully
