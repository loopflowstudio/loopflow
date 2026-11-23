# Review choices

- The report names Desktop Sessions and no individual chord. Start with the reproducible Option-B translation defect and use Shift-Return and Control-Return to check that Session mode negotiation survives reattachment. The installed-app symptom has yet to be reproduced on a physical keyboard.
- Delete `ghosttyShouldHandleKeyDownDirectly` rather than preserving a second direct keyDown path for printable input, so non-IME dead-key layouts (US-International, ABC-Extended, European) go through `interpretKeyEvents` and key translation has one owner.
- Unbind Ghostty's unimplemented standalone-app default keybinds in `GhosttyManager` (`super+q`, `super+comma`, `super+shift+w` in `embeddedConfig`; unimplemented chrome/split/window defaults in `loopflowConfig` before user files) so `ghostty_surface_key_is_binding` does not swallow Loopflow Desktop menus or eat chords into `action_cb` no-ops.
- Keep Ghostty's effective `macos-option-as-alt` setting (layout-dependent default + `~/.config/ghostty/config`) and call `ghostty_app_keyboard_changed` on input-source changes, rather than forcing a Desktop-wide Alt default.
- Fix both `DesktopActionMonitor` and `WorkPaletteShortcut` to check `NSTextInputClient.hasMarkedText()` instead of `NSTextView`, leaving monitor consolidation as separate cleanup.
- For the Option-B fixture, enable bracketed paste and follow both keypresses with a terminal status query. Its reply marks the end of the byte stream for that pair.
