# Concerto Backlog

Pickable work items for Concerto, the loopflow app.

## Phases

| Phase | Focus | Status |
|-------|-------|--------|
| 1 | Polish (macOS local) | Complete |
| 2 | Non-interactive mobile | In progress |
| 3 | Chat experience | Future |
| 4 | Agent harness | Future |

## Phase 2: Non-interactive mobile

Mobile as remote control. No chat, no terminal. Just status and actions.

**What mobile can do:**
- See wave status (running, waiting, idle)
- Trigger non-interactive steps/flows
- Land PRs
- Create waves
- See results

**Infrastructure:**
- iOS app (shared SwiftUI views where possible)
- Auth via loopflow.studio (JWT)
- Remote lfd connection via HTTP API
- Push notifications for "wave needs attention"

**What's explicitly NOT in Phase 2:**
- Terminal streaming
- Chat/conversation
- Interactive steps on mobile

Conductors check in, trigger actions, move on. Deep work happens at the laptop.

## Phase 3: Chat experience

Add LLM-powered conversation. No tools, no execution—just discussion.

- "What's wrong with this PR?"
- "How should I approach this bug?"
- "Review this diff"

Uses Claude/OpenAI/Gemini API directly. Context assembled from codebase. Suggestions can trigger Phase 2 actions.

## Phase 4: Agent harness

Chat gains tools. Full agent on phone.

- File read/write, bash, git
- Structured permission prompts
- Unified across LLM providers

Significant work, but gives us control over the agent experience instead of wrapping three different CLI tools.

## Phase 1 summary

Shipped polish for local macOS workflows:
- Attention summary and grouping in the sidebar
- History and recency for recent activity
- Waiting state actions (connect + PR badges)
- Running state progress and elapsed time
- Empty state that teaches and invites action
- Quick experiment flow without waves

## Screenshot pipeline

```bash
uv run python scripts/generate_screenshots.py --snapshot-only --repo-path ~/src/loopflow-demos --no-clone
uv run python scripts/generate_screenshots.py --ui-test-only --repo-path ~/src/loopflow-demos --no-clone
uv run lf ux-review --direction conductor --area docs/screenshots/
```

## Item format

```yaml
---
status: todo | in-progress | done
phase: 2 | 3 | 4
---
```

## Reference

Design docs: `scratch/concerto-mobile-direction.md`
Personas: `.lf/directions/{conductor,improviser,listener}.md`
