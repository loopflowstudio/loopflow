# Desktop

Concerto for macOS. The place where build work happens.

## Vision

Make Concerto the default build-driving surface for this repo. The conductor opens the app, launches work inside an embedded terminal workspace, keeps sessions alive across restarts, and only drops to a full Ghostty window when the task genuinely benefits from it.

Once that daily build loop feels first-class, polish native chat so exploratory work can stay in the app too.

### Not here

- Replacing the CLI — the CLI stays the source of truth; Concerto composes the work around it
- Replacing Ghostty for every long interactive session — external terminals remain an escape hatch
- Governance dashboards, calibration, portfolio, beat programming — those belong to `workflows` because they reflect engine and garden behavior, not just desktop chrome

## Tasks

1. **`embedded-terminal-build-driver`** (p1) — terminal launch, reattach, multi-agent dispatch, terminal tabs, workspace lifecycle, typed auth, and window polish compose into one daily build surface
2. **`native-chat-ux`** (p2) — rich rendering, history, and a real composer make chat worth using when the work is exploratory

## Risks

- Embedded terminal parity has a ceiling — for some sessions a real terminal window will still win
- Build-driver polish can sprawl; keep the finish line anchored to daily use, not feature matching
- Chat UX should not steal focus from the build-driver milestone
