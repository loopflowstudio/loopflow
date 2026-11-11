# Shared library: what else Mac and iOS should share

Done on this branch: one `ServerLink` in `Loopflow` over a `ServerCarrier` (Unix socket on Desktop, WebSocket on iOS), with answers kept as `RawJSON` and decoded once (`2eafa81aa`); view plans carry `repo_path` (`a8792ca37`); Desktop's sidebar and the phone read one started list, `desktop.view.get` for `TaskList.started` (`ed226294d`). Line numbers below predate those commits. Left over from them: `CreationLookup.result` is still `JSONValue`, so the phone's retry path re-encodes it to read a `TaskCreateReceipt`.

## Shape

`Loopflow` should hold what both apps mean the same way:

- wire types (DTOs that mirror Rust)
- the typed operations a client performs against a Machine
- the local journals that make those operations safe to retry
- presentation that is pure data: status to label and tone, patch to tinted lines, file state to text
- the design tokens

Each app keeps its own windowing, input, platform storage (Keychain, `~/.lf` files) and connection copy.

Make one sub-module split: a second library target, `LoopflowTerminal`, that depends on GhosttyKit. `Loopflow` stays free of GhosttyKit, so its tests and DTO fixtures still build without the xcframework. Both apps link `LoopflowTerminal` for the Ghostty app runtime: config, theme readback and clipboard policy. Each platform keeps its own view: an NSView with a PTY on Mac, a UIView fed from outside on iOS.

There is one blocker. Mac pulls GhosttyKit as a `binaryTarget` from `bin.loopflow.studio` (`swift/Package.swift:32-35`). iOS links a locally built `.build/local/GhosttyKit.xcframework` (`swift/ios.yml:23`). A shared target needs one published xcframework that has both macOS and iOS slices.

`scripts/check_swift_multiplatform_boundaries.py` lets `Loopflow` contain an `#if` only when it is `canImport`, has an `#else`, or gates the whole file. It also bans AppKit, Cocoa, Carbon, ApplicationServices, IOKit and Metal imports there. `Loopflow` has no `#if os` today. The script does not catch a type that compiles everywhere but only Desktop uses. That is the kind of misplacement `Loopflow` has now:

| In `Loopflow` today | What it really is | Move to |
|---|---|---|
| `Models/MultiplexerStore.swift` (486), `Models/MultiplexerLayout.swift` (371) | Desktop's `@Observable` pane store and split tree. UI state, not DTOs | `LoopflowMac` |
| `Config/LoopflowState.swift` (75), `AppBootstrap.swift:17-25` `LaunchArguments.repoURL` | Desktop's selected-repo file and `--repo` argument | `LoopflowMac` |
| `Services/WaveOrigin.swift` (94) | Reads `.git`/`commondir` on local disk | `LoopflowMac` |
| `Services/RegistryQuery.swift`: `RegistryRunner`, `onMachine`, and about 17 methods that shell out to `lf` | The CLI transport. The phone has no `lf` | `LoopflowMac` (see #1) |
| `Models/ChatReference.swift` (259) | Referenced only by `LoopflowTests/ChatReferenceTests.swift` | Delete, or use it to linkify phone replies |
| `Models/ScreenReads.swift` (127): `PlanBoard`, `SessionCard`, `TaskDetail` | Only DTO fixture tests read them. Its doc says "Typed clients request them without `select`", which describes the phone | Decide alongside in-flight #3: either the phone reads `taskDetail`/`sessionCard`, or these go |
| `Wave`, `WaveViewModel`, `WaveSnapshot` (plus iOS `MobileWave`) | Four types for one Wave | `WaveSnapshot` plus `displayName` (see #1) |

`DesktopInspection.swift` is Desktop-only, but it is a wire type that Rust mirrors. It stays with the other DTOs.

## Candidates, ranked by leverage

### 1. One typed operation layer, used by the phone too

**Duplicated.** `LoopflowiOS/RemoteMobileClient.swift:33-53, 80-119` re-implements reads that `RegistryQuery` already types:

| iOS `RemoteMobileClient` | `RegistryQuery` |
|---|---|
| `waves` | `allWaves` (`:1046`) |
| `changes`, `diff`, `file` | `:1154`, `:1275`, `:1287` |
| `creationContext` | `portfolio` (`:907`) |
| `createTask` | `createTask` (`:1199`) |

`MobileWave` (`RemoteMobileClient.swift:171-183`) is a second `WaveSnapshot` (`RegistryQuery.swift:1505`). The two already disagree:

- **Status.** The phone filters on `status != "abandoned"` as a string, while `WaveSnapshot` has a `WorkStatus` enum.
- **Name.** The phone shows the raw `name` (`TaskListView.swift:224, 312`), while Desktop shows `displayName` (`Wave.swift:16`).

**Same concept?** Yes.

**Proposal.** Split `RegistryQuery` in two:

- **`MachineOperations` in `Loopflow`.** The `serve`/`write` methods, backed by the in-flight shared client, returning decoded values (in-flight #2) instead of `String`.
- **`LoopflowMac`.** The `lf` subprocess methods.

`RemoteMobileClient` then shrinks to session attach plus Machine identity. Collapse `Wave`/`WaveViewModel`/`MobileWave` into `WaveSnapshot`.

**Removes** about 80 lines on iOS and one Wave type or more. It also ends the name/status divergence.

### 2. `LoopflowTerminal`: one Ghostty app runtime

**Duplicated.**

| | iOS | Mac |
|---|---|---|
| `ghostty_init` | `LoopflowiOS/GhosttySurface.swift:20` | `LoopflowMac/Services/Ghostty/GhosttyManager.swift:181-185` |
| Config from temp file | `GhosttySurface.swift:36-47` | `LoopflowMac/Design/GhosttyThemeConfiguration.swift:16-21` |
| Theme apply, color scheme | `GhosttySurface.swift:49-65` | `GhosttyManager.swift:131-158` |
| Wakeup/tick | `GhosttySurface.swift:79-81, 67-69` | `GhosttyManager.swift:235-241, 396-399` |
| Clipboard read/confirm/write callbacks (same except for the pasteboard type) | `GhosttySurface.swift:100-139` | `GhosttyManager.swift:329-376` |

**Same concept?** Yes, for the app runtime. They already drift:

- **Theme failures.** iOS finalizes its config without the diagnostics count or colour readback (`GhosttyThemeConfiguration.swift:24-56`). A theme Ghostty rejects shows wrong colours silently on the phone.
- **OSC 52.** iOS denies OSC 52 clipboard writes (`GhosttySurface.swift:123`). Mac allows them (`GhosttyManager.swift:355-356`). Pick one policy.
- **Contrast.** iOS never sets `minimum-contrast = 3` (`GhosttyManager.swift:129`).

**Proposal.** `LoopflowTerminal` holds:

- `GhosttyConfig.load(text) throws`
- `GhosttyConfig.candidate(theme, baseline) throws`, with diagnostics and readback
- `GhosttyRuntime(clipboard: ClipboardAccess, actions: (surface userdata, action) -> Bool)`
- one clipboard policy

The Mac-only `ThemeRGB.nsColor` (`GhosttyThemeConfiguration.swift:4-9`) stays in `LoopflowMac`.

**Removes** about 120 lines. One theme-validation path covers the phone.

**Blocked on** the GhosttyKit packaging issue described in Shape.

### 3. One creation journal (save before send, reconcile by request ID)

**Duplicated.** Desktop `LoopflowMac/CreationSubmissions.swift:25-115` (`CreationSubmissions` actor) against iOS `LoopflowiOS/MobileTaskCreation.swift:53-59, 230-269` plus `RemoteMobileClient.swift:87-110`. Both save the intent under a client UUID before the first send. Both call `creation.get` and resend on `.continue`. Both retire the record only once the result is on screen: Desktop in `CreationPresentation` (`CreationSubmissions.swift:290-299`), iOS in `didPresentTask` (`MobileTaskCreation.swift:293-313`).

**Same concept?** Yes. The phone adds two things of its own: draft text and a saved-for-later list. Each side has a check the other lacks:

- **Desktop only.** It refuses a record from another protocol revision (`CreationSubmissions.swift:100-102`), and it resends through `submittedCreation` (the server replays the exact payload).
- **iOS only.** It checks the receipt's `kind`, `requestId` and Task ID (`RemoteMobileClient.swift:92-99`), and it resends with a new `.command(.taskCreate)`.

**Proposal.** Move `CreationSubmissions` into `Loopflow/Services/CreationJournal.swift`. It is already platform-neutral (`FileManager` plus `machine`/`lookup`/`send` closures). Give it the union of both sides' checks, and make `submittedCreation` the one resend path.

`MobileTaskCreation` keeps `MobileTaskDraft` and `savedRequests` as phone UI state. It hands submission and reconcile to the journal, with a directory under Application Support using `.completeFileProtection`.

**Removes** about 60 lines. It closes both missing checks.

### 4. Patch and file presentation

**Duplicated.**

- **Diff line tint.** iOS `LoopflowiOS/TaskWorkspaceView.swift:209-214` against Mac `TaskPatchLine.changes` (`LoopflowMac/TaskFileDocument.swift:251-271`). The Mac parser knows where hunks are. The iOS one tests prefixes on every line and skips `+++`/`---` to avoid tinting file headers. As a result, a hunk line whose text begins with `--` or `++` (a removed `-- comment`, for example) shows untinted. iOS also paints with `Color.green/red.opacity(0.16)` instead of `palette.successTone.fill`/`failureTone.fill` (`TaskFilesView.swift:519`). That ignores the theme.
- **File state labels.** Mac `LoopflowMac/Views/TaskFilesView.swift:333-341`. iOS shows "Can’t show this file" for every `TaskFileState` (`TaskWorkspaceView.swift:136-138`).
- **Changed-file state wording.** iOS `TaskWorkspaceView.swift:107-111` ("new / uncommitted / committed"), against the Mac row labels.

**Same concept?** Yes. Only the rendering differs: an `NSTextView` with attributes on Mac, a `LazyVStack` on iOS.

**Proposal.** In `Loopflow/Models/TaskWorkspace.swift`:

- `TaskDiffSnapshot.lines -> [PatchLine(text, kind: .added/.removed/.hunk/.context)]`
- `TaskFileState.title`
- `TaskChangedFile.stateLabel`

Each view maps `kind` to palette tones.

**Removes** about 40 lines. Fixes two visible iOS bugs.

### 5. Task status: one label and one tone

**Parallel today.**

- **iOS.** `mobileStatus` and `statusColor` (`LoopflowiOS/TaskListView.swift:166-176, 366-371`) read `WorkSelectionState` tags.
- **Desktop.** The sidebar reads `SessionProgramStatus` (`LoopflowMac/SessionProgramStatus.swift`) and `TaskExecutionState.presentation` (`LoopflowMac/Design/WorkStyle.swift:68-78`). Tones come from `WorkTone` (`WorkStyle.swift:41-65`).

**Same concept?** The same question ("what is this Task doing?"), but different inputs today, so the difference is real for now. The copy already disagrees: "Needs a response" against "Blocked", and "Working" against "Running".

**Proposal.**

- Now: move `WorkTone` (pure SwiftUI over `LoopflowPalette`) into `Loopflow/Design`.
- Add one `TaskStatus(label, tone)` projection from the started list's `WorkViewPlan.Content.states` in `Loopflow/Models`, used by both rows. Both now read the same states.

**Removes** about 30 lines. One vocabulary.

### 6. Owner-protocol codec into `Loopflow`

**Code.**

- `LoopflowiOS/OwnerStream.swift` (whole file).
- The request encoders `AttachRequest`/`ResizeRequest` (`RemoteMobileClient.swift:336-353`) and `HeartbeatRequest` (`OwnerStream.swift:163-166`).
- Bare tag bytes `0x11`/`0x12` (`RemoteMobileClient.swift:239, 245, 330`; `OwnerStream.swift:31, 42`).

**Same concept?** Nothing is duplicated. Desktop never decodes `rust/loopflow/src/transport/wire.rs` in Swift: its Ghostty surface runs an `lf` viewer in a PTY (`GhosttyTerminalView.swift:87-91`). The phone is the only Swift consumer.

**Proposal.** Add `Loopflow/Services/OwnerProtocol.swift`: a `Tag` enum mirroring `wire.rs:20-33`, the request structs, and `OwnerStream`. The win is not lines. It puts the codec next to the other wire types, and lets `OwnerStreamTests` run under macOS `swift test` against `tests/fixtures/dto/owner`, not only in the Simulator. It also gives a future Desktop remote viewer, one that attaches without a local `lf`, a decoder that already exists.

### 7. Perf core

**Duplicated.**

| | iOS `LoopflowiOS/Perf.swift` | Mac `LoopflowMac/Services/Perf.swift` |
|---|---|---|
| Process start | `:38-47` | `:103-111` |
| Elapsed ms | `:25-27` | `:114-117` |
| Signposter (`studio.loopflow`/`perf`) | `:17` | `:13` |

**Same concept?** Partly. The two APIs differ: iOS passes a `Wait` value token and writes a log line, while Mac keeps one named interval pending per name, ends a superseded one, and writes no log line. Both shapes are legitimate.

**Proposal.** `Loopflow/Services/Perf.swift` holds the signposter, `begin -> Wait`, `end(Wait)`, `sinceProcessStart()` and `elapsedMs`. Mac keeps its interval names, `pending` map and `agentInput` on top.

**Removes** about 25 lines.

### 8. Machine-scoped reading cache

**Parallel.**

- **iOS.** `TaskFeed` saves re-encoded `[RepositoryTasks]` per filter (`LoopflowiOS/TaskListView.swift`, `save()`).
- **Desktop.** `WorkCache` saves wire text and restores it through the live decoder, so an incompatible file fails to decode instead of drifting (`LoopflowMac/WorkCache.swift:1-9`).

**Same concept?** Yes: "show the last reading at once, never as authority, invalidated by Machine". The Desktop rule is the safer one.

**Proposal.** A small `ReadingCache(machineID, key) -> wire bytes` in `Loopflow`, used for the shared Task reading on both sides.

### 9. Terminal key names

**Parallel.** `DesktopKey` (`Loopflow/Models/DesktopInspection.swift:597`, a wire type) against iOS `TerminalKey` (`LoopflowiOS/SessionTerminal.swift:6-45`).

**Proposal.** Give `DesktopKey` its USB usage code in `Loopflow`. The iOS key row lists a subset of `DesktopKey` with phone titles and symbols.

**Removes** about 15 lines. Small, and it can be done in passing.

## Keep separate

| Item | Reason |
|---|---|
| `GhosttySurfaceView` (`LoopflowiOS/GhosttySurface.swift:154-361`) and `GhosttyMetalView` (`LoopflowMac/Services/Ghostty/GhosttyTerminalView.swift`) | Different IO models. On the phone, bytes are fed in and `io_write_cb` sends them out. On Mac, Ghostty owns a PTY that runs `lf`. Input translation differs too (`UIKey` against `NSEvent`). |
| `SessionTerminal`, `SessionLink` | Phone interaction: reply field, key row, "Continue here", finger scrolling over Ghostty rows. The take-control handoff has no Desktop equivalent, because Desktop's `lf` viewer handles it. |
| `machineConnectionMessage` (`LoopflowiOS/MachineConnectionView.swift:5-23`) | Tailscale and `URLError` copy for a phone. `ServerError` is already shared. |
| `MachineKeychain` (`LoopflowiOS/SavedMachine.swift`) | Only the phone pairs with a Machine. Desktop is the Machine. `MachineAddress` is already shared. |
| `ThemePaths`/`ThemeLibrary`/`AppearanceStore` (`LoopflowMac/Design/`) | They read `~/.lf/themes` and Ghostty's theme folder on local disk. If the phone should follow the person's theme, that is a server read (`configTheme`), not shared file code. |
| `MobileTaskDraft`, saved requests, `MobileRepositoryPicker` | Phone UX around the journal in #3. |
| `MobileSettings` (`LoopflowMac/Views/MobileSettings.swift`) | Produces the pairing code. The phone consumes it. |

## Open

- **GhosttyKit packaging.** Should one xcframework with both slices be published, or should `LoopflowTerminal` stay source-only, compiled into each app? This decides whether #2 is a package target.
- **Phone screen reads.** Should the phone read `taskDetail`/`sessionCard` (`ScreenReads.swift`) for its Task screen, or should the started list cover it and `ScreenReads.swift` be deleted?
- **Paste encoding.** Can iOS `replyBytes` (`SessionTerminal.swift:167-174`) use `ghostty_surface_text`, as Desktop does (`GhosttyTerminalView.swift:252`)? Ghostty would then do the bracketed-paste encoding, but this depends on how text input reaches `io_write_cb` on a surface with no PTY. Not checked.
