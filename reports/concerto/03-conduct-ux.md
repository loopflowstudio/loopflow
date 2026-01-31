# Conduct UX

## Notification-Driven

The app is quiet until something needs you:
- "feature-auth waiting for interactive step: design"
- "api-refactor completed, PR ready for review"
- "data-migration failed at step: implement"

Tap notification → jump to that wave.

## Dashboard View

```
┌─────────────────────────────────────────────────────┐
│ Waves                                    [+ New]    │
├─────────────────────────────────────────────────────┤
│ NEEDS YOU                                           │
│   feature-auth        design (interactive)  [Connect]│
│   api-cleanup         review (waiting)      [Connect]│
├─────────────────────────────────────────────────────┤
│ RUNNING                                             │
│   data-migration      implement (3/5)       ████░░  │
│   test-coverage       gate (auto)           ██████  │
├─────────────────────────────────────────────────────┤
│ READY TO LAND                                       │
│   bugfix-123          PR #456 ✓ CI          [Land]  │
│   perf-optimize       PR #789 ✓ CI          [Land]  │
└─────────────────────────────────────────────────────┘
```

## Connect Flow

Tap "Connect" → opens terminal session.

On macOS: native Ghostty terminal view.
On mobile: terminal view streaming from remote lfd server.

Same experience, different execution location.

## Finish Step

Interactive steps need an obvious way to signal "I'm done, proceed."

```
┌─────────────────────────────────────────────────────┐
│ Terminal output here...                             │
│                                                     │
│ > claude: I've completed the design. Ready to       │
│   proceed when you are.                             │
│                                                     │
├─────────────────────────────────────────────────────┤
│                              [Cancel]  [✓ Continue] │
└─────────────────────────────────────────────────────┘
```

"Continue" = Ctrl+D equivalent. Tells the agent you're satisfied, flow proceeds to next step.

This button lives outside the terminal view so it's always visible and tappable on mobile.

## Land Flow

Tap "Land" → confirms → calls `lfops land` → wave moves to completed.

## Non-Interactive Output

Auto steps also need good output display—not just raw logs. Terminal needs to be big when interactive, but non-interactive output should look nice too.
