# Open a Task, use its Session

Jack Heart confirmed the minimum iOS experience for LOO-718: start with existing open Tasks, like Desktop's working-task sidebar; tap a Task to open its existing Session; type, edit input, submit and respond to prompts in its terminal. No Wave view, shells or other pane types are needed initially.

[Open the prototype](prototype-tasks.html). [Prototype entry page](mobile-prototypes.html).

## Selected scope

Native Swift, terminal-first, over host-installed Tailscale. The first screen is the open-Task list. A Task opens directly into its existing Session; there is no intermediate intent, diff or checks screen. The Session must be usable for real work, beyond passive viewing. Wave navigation, shell tabs, changes and checks remain outside this surface. Jack Heart subsequently selected the describe-first new-Task sheet in [the creation studies](explore.html); its interaction and backend design are tracked in [new-Task flows](mobile-new-task-flows.md) and [backend draft](mobile-backend.md).

This selects the Tasks entry point but replaces the earlier context-before-terminal design. The older [Sessions](prototype-sessions.html) and [Focus](prototype-focus.html) studies are retained as historical alternatives, not equal implementation candidates.

## Desktop styling and input pass

Jack Heart requested applying the visual comparison findings and reusing Desktop's aesthetics. The current preview uses the actual bundled Cormorant Garamond Medium, Lato Regular/Bold and JetBrains Mono faces, exported colors from Loopflow Light/Dark, Desktop's compact metadata and slim status markers. Task titles and hit areas are enlarged for touch; the illustrated system keyboard uses neutral iOS-like keys rather than app colors.

Sources: [DesignSystem.swift](../swift/Loopflow/Design/DesignSystem.swift), [Loopflow Light](../swift/Loopflow/Themes/Catalogue/Loopflow%20Light), [Loopflow Dark](../swift/Loopflow/Themes/Catalogue/Loopflow%20Dark), [WorkNavigator.swift](../swift/LoopflowMac/Views/WorkNavigator.swift) and [TaskRowMetadata.swift](../swift/LoopflowMac/Views/TaskRowMetadata.swift). The local [prototype theme](mobile-desktop-theme.css) exports those current tokens; it is not wired to live Desktop settings.

## Session controls, not a shell key bar

After reviewing the input pass, Jack Heart asked to reduce modifiers because this MVP is for agent Sessions rather than a generic shell. The default accessory row now has Paste, New line and keyboard visibility. A permission prompt exposes Up, Down, Cancel and Confirm; a working agent exposes Stop. Native text selection handles touch cursor placement. These are conditional controls inside the existing Session, not new destinations.

Conditional actions require trustworthy program/permission state in the native implementation; HTML fixtures do not prove every provider reports it. Never infer permission semantics from silence. Stop represents agent interruption and preserves the preview input.

## Interactive study

The existing three Task fixtures open directly into their Session terminal. The list separates available work, an approval request and working activity; host connectivity is a separate indication, and disconnected rows are labeled cached. Returning to Tasks retains the last selection and each Session's own input/output.

The keyboard is visible on entry and takes actual layout space. Tap letters, Shift, numbers, Delete, space or Return; an external keyboard can edit the browser field too. Hiding the keyboard expands the terminal while keeping the input available to hardware keys. Paste and New line are the everyday touch actions. Native text selection handles editing; no Ctrl/Opt/Shift/Tab or directional cursor strip is displayed. External-keyboard editing remains available, including Shift-Return for a newline. Scrolling away exposes Back to prompt.

Paste inserts a fixed two-line example at the selection without submitting. It never reads the system clipboard. Ordinary browser paste into the field also works. The Shell lifetimes Task presents a command-permission fixture inside the terminal: use its contextual Up/Down controls, external keys or tap to select; Confirm/Return confirms; Cancel/Esc declines. Confirming produces illustrative output only. A selection never runs or approves anything outside this page.

Disconnection disables program input and approvals. Reconnect keeps input and the selected permission option without submitting either. Hide/show keyboard and scrolling remain presentation actions. Reset restores all fixtures. Dark appearance is an external preview control, not another app destination.

This is an interactive HTML sketch, not an iOS keyboard, terminal emulator or native binary. The browser textarea stands in for program-owned terminal input; it does not propose buffering real terminal keystrokes until Return. The rendered keyboard and selected shortcuts illustrate behavior, not general PTY emulation. Native IME, dictation, output selection/copy, geometry ownership, latency and actual agent menus remain unproved.

Task identifiers are familiar labels; output, availability and responses are fixtures, not live status. No agent, network or tracker receives input. The study begins after host connection; automatic Tailscale setup remains a separate requirement. The [comparison board](mobile-visual-comparison.html) retains screenshots of the earlier prototype as the research baseline.

## Remaining technical choices

The chosen navigation does not resolve the terminal library, viewport/geometry policy, input handoff when Desktop is also attached, stale/missing Session behavior or restoration after the host process ends. Preserve the same Session identity and never silently create a replacement conversation. The native proof must show real editing, submission, prompt response and continuity on a phone.

## Verification

Chrome/Playwright passed Desktop font/color loading and the original input pass. The simplified-controls pass checks that modifiers are absent, New line/Paste do not submit, selection replacement works, approval controls are contextual, Cancel does not approve, Stop preserves input and appears only while working, keyboard visibility changes the viewport, and reconnect never submits input or a selected approval. Checked 320px/390px light/dark layouts, 44px terminal accessory targets, index images and local links. Inspected light Tasks/terminal/approval and narrow dark terminal screenshots. These checks establish only the HTML behaviors; native device and terminal/network proofs remain outstanding.
