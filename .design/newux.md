# UX Improvement Loop

**What to build**: Three prompts that chain together to continuously improve Maestro's new user experience through simulated user research, competitive analysis, and targeted redesigns.

## Data Structures

No new code. Three prompt files:

```
.lf/ux-research.lf    # Simulate user profiles
.lf/ux-gaps.lf        # Compare against inspiration
.lf/ux-fix.lf         # Implement high-priority improvements
```

Pipeline config:

```yaml
pipelines:
  ux:
    tasks: [ux-research, ux-gaps, ux-fix]
```

## Key Functions (Prompts)

### 1. ux-research.lf

Simulates 3 user profiles interacting with Maestro:
- **New developer**: First time using AI coding tools
- **Claude Code power user**: Familiar with CLI, trying Maestro
- **Designer/PM**: Non-engineer exploring AI assistance

For each profile, walks through first-run experience using screenshots. Documents friction points, confusion, and unmet expectations.

Output: `.design/ux-research.md` with persona narratives and pain points.

### 2. ux-gaps.lf

Compares Maestro against:
- **Figma**: Onboarding, contextual help, progressive disclosure
- **Cursor**: AI integration UX, prompt input, context management
- **Notion**: New user flow, empty states, templates

References design principles:
- Bret Victor: Direct manipulation, immediate feedback
- Don Norman: Affordances, signifiers, mapping
- Jony Ive: Simplicity, clarity, deference to content

Output: `.design/ux-gaps.md` with specific gaps and patterns to adopt.

### 3. ux-fix.lf

Takes research and gaps, implements highest-impact improvements. Focuses on:
- First-run experience
- Prompt input flow
- Configuration defaults
- Error states and recovery

Makes actual code changes. One commit per improvement.

## Constraints

- **Screenshots required**: ux-research needs screenshots in `.design/screenshots/`. Capture with Cmd+Shift+S in Maestro.
- **Read-only research**: First two prompts research only, no code changes.
- **Incremental**: ux-fix makes small, testable changes—not wholesale redesigns.
- **Maestro-only**: Focus on the macOS app, not CLI or daemon.

## Done When

```bash
# Pipeline runs end-to-end
lf ux

# Artifacts exist
cat .design/ux-research.md  # Has 3 persona narratives
cat .design/ux-gaps.md      # Has competitive analysis
git log --oneline -5        # Shows UX fix commits
```

## Open Questions

1. Should ux-research use web search to study Figma/Cursor/Notion, or rely on LLM knowledge?
2. Should ux-fix auto-run after gaps, or require human review between steps?
3. How to capture "before/after" screenshots for verification?
