# Desktop

Concerto for macOS. The place where build work happens.

## Vision

Make Concerto the default build-driving surface for this repo. Open the app, launch work inside an embedded terminal workspace, keep sessions alive across restarts, and only drop to a full Ghostty window when the task genuinely benefits from it.

Once that daily build loop feels first-class, polish native chat so exploratory work can stay in the app too.

## Priorities

1. **`embedded-terminal-build-driver`** (p1) — terminal launch, reattach, multi-agent dispatch, tabs, wave lifecycle, typed auth, and window polish compose into one daily build surface
2. **`native-chat-ux`** (p2) — markdown rendering, history, and a real composer make chat worth using when the work is exploratory

## Desktop owns

- The embedded terminal workspace and its lifecycle
- Build-driving interactions in the macOS app
- Chat polish that makes Concerto useful for exploratory work

## Not here

- Replacing the CLI — the CLI stays the source of truth; Concerto composes the work around it
- Replacing Ghostty for every long interactive session — external terminals remain the escape hatch
- Governance dashboards, calibration, portfolio, and release surfaces — those belong to `workflows`

## Risks

- Embedded terminal parity has a ceiling — some sessions will still want a real terminal window
- Build-driver polish can sprawl; keep the finish line anchored to daily use, not feature matching
- Chat UX should not steal focus from the build-driver milestone
