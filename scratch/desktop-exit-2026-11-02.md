# Desktop exit evidence — November 2

Jack Heart reported production Desktop gone around 13:20 PST. The installed
0.15.1 bundle was intact. Relaunch restored it; no store, Sessions or Graphs
were changed. Recovery did not repair the cause.

Four exits occurred October 29 (07:27 and 08:00) and November 2 (11:19 and
13:05), all PST. Three surviving fatal envelopes/minidumps identify WebKit
`RemoteLayerTreeDrawingAreaProxyMac::displayLink()` reached through
`commitLayerTree` after page destruction. The fourth matches the exit signature
only; its dump is missing. The observed environment was macOS 26.0.1 (25A362),
reported WebKit 623.1.14. The exact Desktop action that released the pages is
unobserved. Close/Undo displacement and workspace loss remain candidates.

The November 2 launches were PIDs 99798 (10:37:03–11:19:22.44) and 91660
(11:40:25–13:05:39.72). launchd recorded `(0, 0, 256)` (exit status 1), with
no journal exit line or matching macOS report. WebKit load/stop/close logs
preceded both exits by hundreds of milliseconds. This timing alone was only
correlation; the preserved minidumps established the crash mechanism.

Ghostty initializes sentry-native's Breakpad backend by default. Its macOS Mach
exception port intercepts EXC_BAD_ACCESS before BSD signal delivery. Breakpad
writes a dump then calls `_exit(exception_type)`; EXC_BAD_ACCESS is 1. That
bypasses atexit, signal hooks and ReportCrash. Missing signal/macOS evidence
therefore does not exclude a native crash. LOO-683 / #1771's hooks cannot
observe that path. Exit status 1 alone cannot diagnose it either.

The preserved envelopes, minidumps, frame walks and parsing scripts live at
`~/Library/Logs/Loopflow/desktop-exit-minidumps-2026-11-02/`. Originals were in
`~/.cache/ghostty/sentry/*.run/`; sentry-native can rotate them. Do not copy
process memory or derived private evidence into Git or upload it.

The complete original report was preserved before curation at
`~/Library/Logs/Loopflow/desktop-exit-minidumps-2026-11-02/source-notes-20261114/desktop-exit-2026-11-02.md`.
SHA-256: `d267930fecc22441e817a0d7d6abd36e117c23afcb9f80e109094e55b7079f72`.
It is also embedded in this checkout's
`.lf/prompts/20261114-083755-2da6c7de-cc05-48f9-84ba-a99e4f5c66d3-2Q1qU9-summary.md`.

The original report's Sparkle control-test/old-Dev-build analysis and Claude
weekly-limit failover analysis are separate incidents. They establish no
browser lifetime repair and select no work here. Current scope and remaining
experiments are in `loo-715-plan.md`; reporter disabling and envelope correlation
remain alternatives awaiting measured comparison and Jack's policy decision.
