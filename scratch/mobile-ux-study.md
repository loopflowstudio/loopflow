# A phone-native Loopflow terminal

LOO-718 · research · November 10, 2026. Source studies: [implementation](mobile-ux-sources.md), [visual references](mobile-ux-visuals.md). Current app inspected in `LoopflowiOSApp.swift`, `SessionTerminalView.swift` and `MobileNewTaskView.swift`; physical feedback and checks remain in `mobile-swipe.md`.

Jack Heart: “i want you to study the UX of t3, paseo, apradev, and rex, and see how to mke it more this ui more effective”; “and delightful”. Paradev identification: “Yes, Paradev.”

## Direction

Keep the live terminal primary. Make reading, replying and switching work feel native to a phone. The strongest improvement is continuity: the text stays where it was left, a tap does what its label promises, and returning does not make someone reconstruct their context. Use the Desktop palette and type hierarchy with less simultaneous chrome.

The comparisons are source inspection, documented behavior and screenshots; no competitor was operated on a physical device. Rex's community image is an October alpha; Paradev's official images retain older Superset branding. Proposed layouts are illustrations, not a working terminal demo.

## What to borrow

| Reference | Observed pattern | Loopflow application |
|---|---|---|
| [Rex iOS settings](https://www.superlogical.com/rex/docs/reference/ios-settings) and [actions](https://www.superlogical.com/rex/docs/reference/ios-actions) | Theme-matched screen, smooth scrolling/reflow, configurable keys and keyboard restoration; secondary actions in a palette. | One coherent terminal background, compact native identity, one keyboard affordance, rare keys behind Keys. Its pane minimap represents real splits; Loopflow's Session picker should not imitate that geometry. |
| [Paseo scroll controller](https://github.com/getpaseo/paseo/blob/33c818cc6b73f7f5ef866a5c0719e0c7d9e75780/packages/app/src/agent-stream/bottom-anchor-controller.ts) | Following and detached reading are separate states; gestures suspend following. | New output must respect reading. First opening starts live; a retained return restores its place. A visible Latest action returns to following. The source is chat behavior, not proof of terminal parity. |
| [T3 terminal UI](https://github.com/pingdotgg/t3code/blob/3a8c250cb0c8056f2f563091fd412fe5db809fa1/apps/mobile/src/features/terminal/ThreadTerminalRouteScreen.tsx) | Terminal actions in menus, accessory with the keyboard, one keyboard control while reading. | Reclaim the bottom bar when the keyboard is hidden. Preserve direct terminal input; do not add a second draft editor by default. Both T3 and Paseo have more terminal keys than this app needs. |
| [Paradev mobile](https://docs.paradev.com/mobile) | One readable surface, compact context, bottom reply controls, quiet collapsed details. | Move frequent actions within thumb reach. Keep Task and selected Session clear; put host and detailed connection information in the title's detail sheet. Keep failures visible. Its chat cards are a different interaction model. |

## Proposed next design

1. **Reading owns the screen.** One compact Task/Session header, a quiet activity indicator, terminal, and a bottom keyboard action. Show Latest only away from the live end. Preserve wide-grid panning; do not reuse its horizontal gesture for Session switching.
2. **Typing owns a small accessory.** Paste, New line, Keys, Hide keyboard. Keep controls and the final terminal line above the software keyboard. Hiding the keyboard returns to reading; control authority remains explicit. A proposed bottom keyboard action requests control when necessary and then opens typing.
3. **Details appear where they belong.** Tap the title for Session identity, Machine, activity and connection details. Session switching opens a small sheet. Keep transport errors actionable; remove repeated healthy-state text such as a permanent hostname plus “Keyboard connected”.
4. **Tasks stay familiar.** Preserve the existing order and search position. A reserved status slot can distinguish working from needs input without moving titles or animating every row. Do not replace the Task list with a Wave dashboard.
5. **Voice produces an editable intention.** Keep system dictation available now. A dedicated microphone can later record → review → insert, with submission remaining deliberate. Paseo's dictation and conversational voice are separate features; neither requires adopting its orchestration model. [Voice documentation](https://paseo.sh/docs/voice), [dictation controls](https://github.com/getpaseo/paseo/blob/33c818cc6b73f7f5ef866a5c0719e0c7d9e75780/packages/app/src/components/dictation-controls.tsx).
6. **Delight stays quiet.** Consistent cream/burgundy and Dark equivalents; generous targets; short, reduced-motion-aware transitions; light haptics for deliberate changes such as taking input control. No haptic for every streamed line. Stable position and instant acknowledgment matter more than extra animation.

## Sequence and checks

First finish native scrolling and reading retention, including output during touch/deceleration, oversized Desktop geometry, keyboard transitions and Back/reopen. Then try the compact header and bottom keyboard affordance on the physical phone. Keep explicit control loss and reconnect behavior. Measure usable text area and time/taps to reach input, not the number of features displayed.

After that, voice capture and focused review screens are worthwhile. A native conversation reader could improve long-form prose and tool disclosure, but it is a separate choice backed by real Session history. Do not reconstruct agent controls from terminal text or silently replace the live surface.

## Questions for the next design

1. Reading layout: **a)** compact title + bottom keyboard action (recommended), **b)** retain the current persistent status/host header.
2. Voice: **a)** dedicated record/review/insert action next, **b)** keep the system keyboard microphone and prioritize review screens first.
3. Later reading surface: **a)** continue terminal-only while improving it (recommended for this cut), **b)** explore an optional structured-history reader for the same Session.
