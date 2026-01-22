# web

Web client for Loopflow. Port of the Swift app.

## Philosophy

This is a **follower, not a leader**. Design decisions happen in the Swift app first; this port translates them to web.

Goals:
- **Portable patterns** — Standard Next.js/React/TypeScript. No exotic dependencies. Easy for any web dev to pick up.
- **Mirror Swift** — Components, models, and services map 1:1 to `swift/` where possible.
- **Don't innovate here** — If something needs design work, do it in Swift first, then port.

This may take on its own life at later maturity. For now, it follows.

## Development

```bash
npm install
npm run dev     # http://localhost:3000
```

## Modes

**Local mode**: Connect to lfd daemon via HTTP API at `localhost:8765`. Same data as Concerto but over HTTP instead of Unix sockets.

**Teams mode**: Connect to Symphonia API. Authenticated, multi-tenant.

## Structure

```
src/
├── app/                    # Next.js app router
│   ├── layout.tsx
│   ├── page.tsx            # Welcome/repo picker
│   └── repo/[path]/
│       └── page.tsx        # Main workspace view
├── components/
│   ├── WorktreeSidebar.tsx # Maps to Concerto/Views/WorktreeSidebar.swift
│   ├── PromptLauncher.tsx  # Maps to Concerto/Views/PromptLauncher.swift
│   ├── OutputPanel.tsx     # Maps to Concerto/Views/OutputPanel.swift
│   └── LoopStatus.tsx      # Maps to Concerto/Views/LoopRow.swift
├── models/
│   ├── worktree.ts         # Maps to LoopflowCore/Models/Worktree.swift
│   ├── step.ts             # Maps to LoopflowCore/Models/Step.swift
│   ├── loop.ts             # Maps to LoopflowCore/Models/Loop.swift
│   └── flow.ts             # Maps to LoopflowCore/Models/Flow.swift
└── services/
    ├── lfd-client.ts       # HTTP client for lfd daemon
    └── worktree-service.ts # Maps to LoopflowCore/Services/WorktreeService.swift
```
