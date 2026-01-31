# Improvise UX

## Wave Creation

Hit + → immediately get a new wave. No wizard, no modal. Wave exists. Now configure it or start working.

## Area-First

Start by picking where to work:

```
┌─────────────────────────────────────────────────────┐
│ Explore                                             │
├─────────────────────────────────────────────────────┤
│ Recent areas:                                       │
│   src/api/          last: 2h ago                    │
│   swift/Concerto/   last: yesterday                 │
│                                                     │
│ [Browse...]  [From clipboard]  [Current branch]     │
└─────────────────────────────────────────────────────┘
```

## Step Runner

Once you have an area:

```
┌─────────────────────────────────────────────────────┐
│ src/api/auth/                          [Change Area]│
├─────────────────────────────────────────────────────┤
│ Direction: product-engineer, security    [Edit]     │
├─────────────────────────────────────────────────────┤
│ Quick steps:                                        │
│   [review]  [design]  [implement]  [debug]          │
├─────────────────────────────────────────────────────┤
│ Or run flow:                                        │
│   [ship]  [grind]  [research]  [custom...]          │
├─────────────────────────────────────────────────────┤
│ Prompt:                                             │
│ ┌─────────────────────────────────────────────────┐ │
│ │ add rate limiting to the auth endpoints        │ │
│ └─────────────────────────────────────────────────┘ │
│                                        [Run ▶]      │
└─────────────────────────────────────────────────────┘
```

"Run" → runs step on wave → shows output.

## Transitioning to Conduct

The wave exists from the start. When you're done improvising:

```
┌─────────────────────────────────────────────────────┐
│ auth-feature (4 steps run)                          │
│                                                     │
│ [Add Stimulus]  [Create PR]  [Archive]              │
└─────────────────────────────────────────────────────┘
```

"Add Stimulus" → set loop/watch/cron → wave runs autonomously, you conduct it.
