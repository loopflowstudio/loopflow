# Shared library: what is left

Jack Heart, 2026-11-11, on the sharing plan: "agreed on your shared library pass".

`Loopflow` holds what both apps mean the same way: wire types and the
owner-protocol codec, one `ServerLink` over a `ServerCarrier`, the typed
`RegistryQuery` reads (`RegistryQuery.served` on the phone), the creation
journal, Task status and `WorkTone`, patch and file-state presentation, and
`PerfClock`. Each app keeps its windowing, input, platform storage and
connection copy.

## Left to build

- **`LoopflowTerminal`: one Ghostty app runtime.** Config load, theme
  candidate with diagnostics and colour readback, clipboard callbacks, one
  OSC 52 policy, `minimum-contrast = 3`. iOS (`GhosttySurface.swift`) and Mac
  (`GhosttyManager.swift`, `GhosttyThemeConfiguration.swift`) have drifted on all
  three. Both apps now link the same published lf8 archive, so nothing
  blocks it.
- **Split `RegistryQuery`.** Its `run` half (about 17 methods that shell out to
  `lf`, `onMachine`, `pinTaskLocation`) is Desktop-only; on the phone those
  calls throw. Moving them to `LoopflowMac` touches many Desktop call sites.
- **One Wave type.** `Wave`, `WaveViewModel` and `WaveSnapshot` describe one
  Wave; the phone now uses `Wave`.
- **Desktop rows on `WorkViewPlan.Content.status`.** Desktop's sidebar shows the
  richer `SessionProgramStatus`; it could fall back to the shared status where no
  report exists.
- **`CreationLookup.result` is `JSONValue`**, so reading a `TaskCreateReceipt`
  from it re-encodes once on the creation retry path.

## Open

- **Phone screen reads.** Should the phone's Task screen read `taskDetail` /
  `sessionCard` (`ScreenReads.swift`), or should the started list cover it and
  `ScreenReads.swift` go?
- **Paste encoding.** Can iOS `replyBytes` use `ghostty_surface_text` as Desktop
  does, on a surface with no PTY? Not checked.

## Checks

| Command | Result | Who runs it later |
|---|---|---|
| `swift test` (Desktop, full) | 808 tests; 13 deadline failures under full-suite load, all 93 in those suites pass alone | gate |
| `uv run python scripts/test_ios.py -only-testing:LoopflowiOSTests` | passed | gate / CI |
| `cargo test -p loopflow --test served_reads` | `screen_reads_are_compact_passive_and_match_cli` fails (plan board empty); not checked on the base | gate |
