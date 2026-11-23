# LOO-1069 · Modifier keys in Desktop Sessions

> “Scope is Loopflow Desktop: modifier keys are not working in desktop sessions.” — Jack Heart

Done when a focused Desktop Session delivers terminal-owned chords to the running program with their modifiers intact, including after reopening the Session. Desktop shortcuts and macOS app menus still work over a focused terminal, Ghostty terminal bindings run once, Option text / dead keys / IME composition type and cancel cleanly, and the same keyboard behavior applies in a Desktop Shell.

## Model and design

One keyboard ownership hierarchy governs every Desktop terminal pane (Session and Shell):

1. **Active composition (`NSTextInputClient.hasMarkedText()`):** While any focused text input client — including `GhosttyMetalView` — has marked text, Desktop shortcut monitors (`DesktopActionMonitor`, `WorkPaletteShortcut`) and `GhosttyMetalView.flagsChanged` leave composition alone.
2. **Desktop window/pane shortcuts & macOS app menus:** `DesktopActionMonitor` (`DesktopActions.swift`, including `Ctrl-Tab`, `Ctrl-Shift-Tab`, `Ctrl-`` when available) and `WorkPaletteShortcut` (`⌘K`, `⌘\`) act before the terminal. Inside `GhosttyMetalView.performKeyEquivalent`, macOS window cycling (`⌘\``, `⌘⇧~`) falls through to AppKit and exact `⌘C`/`⌘V`/`⌘F` stay with Desktop Copy/Paste/Find. Unbound equivalents — including app menus (`⌘Q`, `⌘,`, `⌘H`, `⌘M`, `⌘⇧W`, `⌘⇧O`, `⌘/`) — return `false` to AppKit.
3. **Ghostty terminal bindings:** `GhosttyManager` unbinds Ghostty's unimplemented standalone-app default keybinds so `ghostty_surface_key_is_binding` only claims terminal-surface bindings Ghostty or Loopflow can actually perform. Matching bindings and `Control-Return` (which macOS otherwise treats as a context-menu trigger) route from `performKeyEquivalent` into `keyDown` and execute once.
4. **The running program:** Every remaining keystroke flows through a single AppKit-to-Ghostty key path into `ghostty_surface_key`, then through the PTY and held-terminal relay (`transport.rs`, `transport/hosted.rs`, `transport/input.rs`, `rust/loopflow-terminal/src/repaint.rs`) with keyboard modes restored on attach.

### What changes across layers

- **Single AppKit-to-Ghostty key path (`GhosttyTerminalView.swift`).** Replace the four split paths (`performKeyEquivalent` calling `ghostty_surface_key` directly, `ghosttyShouldHandleKeyDownDirectly` bypassing AppKit, `insertText` pasting via `ghostty_surface_text`, and side-blind `flagsChanged`) with the pinned Ghostty AppKit responder contract:
  - Delete `ghosttyShouldHandleKeyDownDirectly`, `ghosttyShouldBypassInterpretKeyEvents`, `ghosttyKeyText`, `ghosttyInputSourceUsesTextComposition`, `ghosttyIsPrintableKeyText`, `_didInsertText`, `_currentKeyEventModifiers`, and `ghosttyShouldHandleTextAsKeyEvent`.
  - Translate modifiers with left/right side masks (`NX_DEVICEL*`/`NX_DEVICER*` -> `GHOSTTY_MODS_*_RIGHT` / base flags), carry press vs. `event.isARepeat` -> `GHOSTTY_ACTION_REPEAT` vs. release, and compute `unshifted_codepoint` from `event.characters(byApplyingModifiers: [])` (not `charactersIgnoringModifiers`, which retains Shift).
  - Query `ghostty_surface_key_translation_mods(surface, mods)` on `keyDown`. Construct a translated `NSEvent` only when translation modifiers differ from the event's modifiers; reuse the original `NSEvent` unchanged when equal (required by input methods such as Korean). Record `consumed_mods` from the translation modifiers (excluding Control and Super).
  - Run `interpretKeyEvents([translationEvent])` with a `keyTextAccumulator` open. Collect text AppKit passes to `insertText` during `keyDown` and deliver it on the single `ghostty_surface_key` call instead of calling `ghostty_surface_text`.
  - Keep marked text in `ghostty_surface_preedit`. When `setMarkedText` receives an empty string, clear `_markedRange` to `NSNotFound` and call `ghostty_surface_preedit(surface, nil, 0)` so cancellation never leaves `hasMarkedText()` stuck `true`. When finishing preedit with committed text (or when `insertText` arrives outside `keyDown`), commit text without the triggering chord's modifiers; when cancelling preedit with no committed text, emit no terminal keystroke.
  - Fix `flagsChanged` to skip while `hasMarkedText()` is true and determine press vs. release from the keycode's side-specific `NX_DEVICE*` bit rather than generic `.shift`/`.control`/`.option`/`.command`.
  - In `performKeyEquivalent`, keep first-responder, `surface`, `connectionAcceptsInput`, window-cycling (`⌘\``, `⌘⇧~`), and exact `GhosttyTerminalShortcut` (`⌘C`/`⌘V`/`⌘F`) handling. Route `Control-Return` (without Command) and bindings reported by `ghostty_surface_key_is_binding` into `keyDown` and return `true` (so each executes once, including `unconsumed:` bindings). Return `false` for all other equivalents so AppKit menus (`⌘Q`, `⌘,`, `⌘H`, `⌘⇧W`, `⌘⇧O`, `⌘/`) and non-Command `keyDown` dispatch run normally. Implement `@objc func selectAll(_ sender: Any?)` via `performBinding("select_all")` so Edit → Select All stays consistent.
  - Keep existing connection/exit gating: awaiting-relay or disconnected panes send no PTY input (while Copy, Find, window cycling, and Desktop menus still work), a missing surface falls through to `super`, and an exited child still accepts its local dismiss key.
- **Ghostty config & keyboard-layout sync (`GhosttyManager.swift`).**
  - Observe `NSTextInputContext.keyboardSelectionDidChangeNotification` and call `ghostty_app_keyboard_changed(app)` so Ghostty's layout-dependent `macos-option-as-alt` default and physical-key mapping stay accurate when input sources change.
  - Unbind Ghostty's unimplemented standalone-app default keybinds whose actions `GhosttyManager.action_cb` ignores (`super+q`, `super+comma`, `super+shift+w` in `embeddedConfig` so Desktop always owns Quit, Settings, and Close Window; unimplemented chrome defaults such as `new_split`, `toggle_fullscreen`, `toggle_command_palette`, `inspector`, `new_window`, `new_tab`, `close_surface`, `close_tab`, `goto_split`, `resize_split`, `toggle_split_zoom` in `loopflowConfig` before `~/.config/ghostty/config`). Keep surface-owned bindings (`super+arrow_up/down` prompt jump, `super+a` select all, font size, scrollback, and user `text:`/`csi:`/`esc:` bindings).
- **Composition guard on Desktop shortcut monitors (`DesktopActions.swift`, `WorkPalette.swift`).**
  - Change `(window.firstResponder as? NSTextView)?.hasMarkedText() != true` in both `DesktopActionMonitor` and `WorkPaletteShortcut` to `(window.firstResponder as? NSTextInputClient)?.hasMarkedText() != true` so active IME/dead-key composition in a terminal pane is protected identically to native text fields.
- **Held-terminal relay (`rust/loopflow-terminal/src/repaint.rs`, `rust/loopflow-execution/src/transport/`).**
  - Existing mode replay (`repaint.rs`) and non-sizing query filtering (`transport/input.rs`) already align with this model; no Rust change is planned unless Slice 3 exposes a defect in held-Session mode negotiation or forwarding.

### Choices made for review

- **Delete the direct printable-key bypass (`ghosttyShouldHandleKeyDownDirectly`).** Passed over keeping a second ASCII-only path because it skips `interpretKeyEvents` on non-IME dead-key layouts (US-International, ABC-Extended, European) and duplicates key translation. Reversible.
- **Unbind Ghostty's unimplemented standalone-app default bindings in `GhosttyManager`.** Passed over keeping Ghostty's full standalone defaults because `action_cb` returns `false` for them, which would make `ghostty_surface_key_is_binding` swallow `⌘Q`, `⌘,`, `⌘⇧W`, `⌘D`, and `⌘↩`. Reversible.
- **Keep Ghostty's effective `macos-option-as-alt` setting (layout-dependent default + user `~/.config/ghostty/config`) and sync layout changes via `ghostty_app_keyboard_changed`.** Passed over forcing a Desktop-wide Alt default, which would break accented Option typing on non-US keyboards and override explicit user settings. Reversible.

Clipboard paste, `lf desktop send`/`send-key`, selection, focus, connection recovery, iPhone, Rex, CLI flags, wire formats, and unifying `DesktopActionMonitor` with `WorkPaletteShortcut` are outside this change.

## Build and check

1. **Built the Option-B slice.** Desktop key equivalents now probe for a binding before sending a key. The shared Session and Shell responder lets AppKit interpret Option on the side Ghostty selected, then encodes committed text with the original key. Clipboard paste still uses the paste API.
2. **Built modifier, composition and shortcut ownership.** Each modifier release follows its physical key. Both Desktop shortcut monitors and the terminal's modifier handler defer during marked text. Input-source changes refresh Ghostty's keymap, and Edit → Select All selects terminal output. Ghostty's standalone shortcuts are removed before loading personal bindings. The Desktop menu keys for Quit, Settings and Close Window are unbound after loading them.
3. **Verified held Session reattachment.** `SessionModifierInputTests` opens a prepared Session on a private Machine. Its provider sets the PTY to raw mode and requests Kitty disambiguation once. The provider prints each batch as hex. Desktop reads the first result, releases the surface and prepares the same Session. The reopened pane must show that first result before Desktop sends the second batch. A replacement process would still be waiting for its first batch.

**Remaining.** Nothing left to build. Gate publishes and CI repeats the headless byte checks. A person still checks physical input-source switching and real IME typing on the Mac (see Demo).

**Slice 1 checks**
- `scripts/test_desktop.sh --skip-build --filter GhosttyModifierInputTests` after `swift build --build-tests --package-path swift` passed all four settings headless. With bracketed paste enabled, each side wrote exactly `1b 62` or `e2 88 ab`. CI repeats this check.
- `LOOPFLOW_NATIVE_TESTS=1 swift test --package-path swift --skip-build --no-parallel --filter typingAfterKeyboardRestore` passed both Kitty modes on the Mac. The mounted fixture now waits for the top history line before typing and reads the literal `x` at the live prompt. This stays in the optional display lane.

**Slice 2 checks**

- `scripts/test_desktop.sh --filter GhosttyModifierInputTests` passed all seven headless tests. In Kitty disambiguation mode the surface wrote exactly CR, CSI 13;2u and CSI 13;5u for Return, Shift-Return and Control-Return. CI repeats the byte checks.
- With `LOOPFLOW_NATIVE_TESTS=1`, `swift test --package-path swift --no-parallel --filter focusedKeyEquivalents` verified menu fallthrough, Control-Return dispatch, selection and a personal unconsumed binding. The mounted checks `composingInFocusedTerminal` and `pasteShortcutFollowsFirstResponder` passed after fixing their fixtures. The first dispatches through both local monitors in a test key window. The second waits for the Session relay's ready message before pasting.

**Slice 3 checks**

- `scripts/test_desktop.sh --filter SessionModifierInputTests` passed without WindowServer. The provider recorded exactly `0d 1b 5b 31 33 3b 32 75 1b 5b 31 33 3b 35 75` on each surface. The existing held-terminal replay restored Kitty mode.
- `LOOPFLOW_NATIVE_TESTS=1 swift test --package-path swift --no-parallel --filter nativeOpeningAndRecovery` passed. The former bare chat showed the prompt while the connection remained at generation zero. Opening the prepared Session exercised server loss, recovery and provider exit.
- `scripts/test_desktop.sh --skip-build --filter 'GhosttyModifierInputTests|SessionModifierInputTests'` passed all eight headless tests. The two mounted diagnostics run in the display lane. Physical keyboard layout switching and IME typing remain manual.

**Compress checks**

- `swift build --build-tests --package-path swift` and `scripts/test_desktop.sh --skip-build --filter 'GhosttyModifierInputTests|SessionModifierInputTests'` passed. Eight headless tests ran, including the provider’s on-screen byte report before and after reopening. CI repeats these checks.
- `LOOPFLOW_NATIVE_TESTS=1 uv run --no-sync python scripts/test_network.py swift test --skip-build --disable-sandbox --package-path swift --no-parallel --filter 'focusedKeyEquivalents|composingInFocusedTerminal|pasteShortcutFollowsFirstResponder|typingAfterKeyboardRestore|nativeOpeningAndRecovery'` passed all five mounted tests. An earlier direct Swift invocation carried this checkout’s Task ID into the private Machine and failed at Session launch. The wrapper clears inherited `LF_*` values.

**QA checks** ([report](scratch/qa.html))

- Branch Dev bundle on a private Machine, screen locked, keys posted into the app's event queue: legacy and Kitty modified Return/Tab/Option reached the held Session once each, also after quitting and reopening the app. ⌘, opened Settings without reaching the pane.
- Found and fixed: AppKit fires ⇧⌘ menu items on the unshifted chord, so ⌘↩ went to Zoom Pane and ⌘E switched the Session pane to the browser. `performKeyEquivalent` now keeps chords that only a Shift menu item would take; `shiftedMenuItems` covers the rule. `scripts/test_desktop.sh --filter 'GhosttyModifierInputTests|SessionModifierInputTests'` passed 11 headless tests; CI repeats.
- Open for a person at an unlocked Mac: key-window dispatch order (view before menu) for ⌘↩/⌘E, the ⌘K palette, physical keyboard, input-source switching, IME.

**Demo.** Open the isolated Session in Loopflow Desktop. Press Return, Shift-Return, and Control-Return; the fixture shows `0d`, `1b 5b 31 33 3b 32 75`, and `1b 5b 31 33 3b 35 75` once each. Close and reopen the pane and repeat. In legacy mode, Option-B shows `1b 62` with Option-as-Alt enabled and `∫` with it disabled on a US layout. Confirm `⌘Q`, `⌘,`, and `⌘⇧W` still reach Loopflow Desktop menus while the Kitty-mode Session is focused.

References: [Desktop keyboard docs](swift/README.md), [Concept review](scratch/concept-review.html), [Pinned Ghostty surface API](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/src/apprt/embedded.zig#L2015-L2076), [Ghostty AppKit responder](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/macos/Sources/Ghostty/Surface%20View/SurfaceView_AppKit.swift#L1102-L1426), [Ghostty event fields](https://github.com/ghostty-org/ghostty/blob/a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/macos/Sources/Ghostty/NSEvent%2BExtension.swift).
