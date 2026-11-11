# Mobile interface references · 2026-11-10

The strongest direction is a full-height live terminal with a small native shell: identity above, context-sensitive input below, and continuity when moving between Sessions. A phone should retain the desktop's visual identity and work context while showing one readable surface at a time. This is a design inference from the references below, not a tested Loopflow design.

## Product identification

“apradev” did not resolve to a coding product in exact-name search. **Paradev**, formerly Superset, was the strongest candidate; the conversation subsequently supplied: “Yes, Paradev.” It has desktop and native iPhone/iPad apps with live agent Sessions, terminals, and review. Its [official site](https://paradev.com/) states the former name; its [App Store listing](https://apps.apple.com/us/app/paradev-formerly-superset/id6788926383) records the October 26 rename.

## Rex: make the terminal itself comfortable

The official [iOS settings](https://www.superlogical.com/rex/docs/reference/ios-settings) document several unusually relevant details:

- Smooth history scrolling and immediate text reflow when terminal size changes.
- The outer screen can match a full-screen program's background; controls can use terminal theme colors. Light and dark palettes are separate.
- Device-local font size and automatic text contrast correction.
- A key bar whose keys can be hidden/reordered, additional shortcut groups, punctuation choice, and light haptic feedback.
- Keyboard restoration when returning to a terminal.

The [iOS command palette reference](https://www.superlogical.com/rex/docs/reference/ios-actions) makes less-frequent operations searchable: switch Session or tab, show/hide keyboard, font size, copy/paste, send a key, insert a path, and directory navigation. This suggests a small visible toolbar can coexist with complete terminal capability; it does not establish how discoverable Rex's palette is in practice.

**Screenshot inspected, community source:** [Reona Shimada's October 31 hands-on report](https://dev.classmethod.jp/en/articles/reona-superlogical-rex-beta-try/) includes [this iPhone screenshot](https://devio2024-media.developers.io/image/upload/c_limit,w_1200/f_auto/q_auto/v1793377067/2026/10/31/lc1qognvkferz2barrm9.jpg), for Rex Dev 0.1.0 (1577). It shows one terminal occupying nearly the entire screen, a program icon and abbreviated directory in the header, Session name beneath, back and overflow buttons, a miniature split-layout indicator at the upper right, and a small bottom pane picker with program icons. No modifier ribbon or software keyboard is visible in this reading state. The author reports horizontal swipe pane switching and little perceived typing latency; these are their observations, not measurements reproduced here.

**Implication for Loopflow:** use one coherent background and restrained chrome around the real terminal. Show the Task and selected Session without turning the top into a connection dashboard. Keep frequently needed actions reachable; move rare modifier combinations into an additional control surface. Do not copy Rex's pane minimap unless Loopflow has actual pane geometry to represent. A Session picker serves a different concept.

Rex's [Program Status Protocol](https://www.superlogical.com/rex/docs/build/program-status) gives the terminal explicit idle/working/blocked/done/error state. Its docs describe Session-picker indicators and unfocused-tab status symbols. The same page still describes push notifications and a unified inbox as future work, while the iOS action list includes Activity actions. Availability and exact inbox behavior therefore remain uncertain; don't present the full inbox/push experience as verified.

## Paradev: put attention and replies within reach

**Official screenshots inspected:** [Home](https://docs.paradev.com/images/mobile-home.png) and [Session](https://docs.paradev.com/images/mobile-session.png), linked by the [mobile guide](https://docs.paradev.com/mobile). Both screenshots still use Superset branding, so they are older visual references hosted in current documentation.

Home has a mostly black background, large white titles, quiet gray metadata, compact host/sort pills, and project-grouped workspaces. Each workspace row shows its branch and change count, with tiny agent/terminal icons at the right. A composer with attachment and microphone controls sits at the bottom. The Session screenshot uses ordinary readable prose, muted collapsed “N steps” rows, a single-line title, and the same bottom composer with a conspicuous stop control during work.

The mobile guide documents permission/failure/ready status dots, session switching from composer tabs, dictation and photo attachment, file-by-file diffs, and line comments sent back to a chosen Session. Its own screen images are useful for hierarchy, not proof that every interaction feels good.

The [App Store release history](https://apps.apple.com/us/app/paradev-formerly-superset/id6788926383) gives concrete continuity details: recently used Sessions stay live, each workspace reopens on its last tab, terminal PR links open inside the app, and connection failures received clearer messages. Version 1.2.1 says Chat is experimental and follows the account setting. Thus the current chat screenshot is not evidence that all terminal Sessions are converted to chat by default.

**Implication for Loopflow:** remember the selected Session, draft, and reading position. Put actions at the object they affect: Session switching with the selected Session, interruption beside input, Task review with the Task. Avoid adopting all of Paradev's branch/change metadata on every row; its screenshots visibly truncate many workspace titles. Task title clarity is more valuable than matching its density.

## Paseo and T3 Code: useful supporting patterns

[Paseo's official homepage](https://paseo.sh/) depicts mobile workspace lists, a thread, and a separate changed-file screen. The page identifies its hero as an interactive product mockup, so these are marketing compositions rather than captured-device screenshots. Its useful hierarchy is attention first: ready to review, working, and done; thread and diff each get the phone's full width. The [voice documentation](https://paseo.sh/docs/voice) separates dictation from conversational voice mode; dictation is on by default and voice mode is opt-in. This supports adding plain dictation without making an always-listening interaction the default. No on-device Paseo interaction was performed in this pass.

T3 Code supplies instructive counterexamples in public issues. [A November 4 report](https://github.com/pingdotgg/t3code/issues/30731) says the floating composer covers the final response lines; this is a reporter observation, not independently reproduced. [An October 28 report](https://github.com/pingdotgg/t3code/issues/26662) describes image taps appearing unresponsive while an entire screenshot gallery loads. Its [November 9 release notes](https://github.com/pingdotgg/t3code/releases/tag/v0.1.8) list the fix to open iOS image previews without waiting for the gallery. Do not report the older image issue as current behavior.

**Implication for Loopflow:** a polished bottom bar still needs correct content insets through keyboard changes. Opening a Session or attachment should reveal the selected thing promptly; loading unrelated content must not make the tap appear ignored. These are acceptance targets, not performance claims about Loopflow.

## Design priorities suggested by these references

1. **Reading must feel native.** Smooth touch scroll, stable position while output arrives, readable reflow, reliable selection, and an explicit return-to-latest affordance when away from live output. The return-to-latest affordance is a proposal, not a verified feature of all these products.
2. **Input should occupy space only when useful.** Reading gets the full screen; writing gets a compact native accessory surface with the few essential actions and an expandable collection for rare keys. Keep raw terminal input available.
3. **Continuity is the main delight.** The Session you chose, the draft you left, and your place in its history survive navigation and temporary disconnection. Restore to reading without taking keyboard control unless that was intended.
4. **Use desktop identity at phone scale.** Reuse colors, type hierarchy, icons, and material treatment; simplify simultaneous panes and metadata instead of miniaturizing them.
5. **Make attention meaningful.** Distinguish waiting for input from working and completed. Do not infer these states from screen colors or title text when the existing Session model can report them.

## Terminal and conversation reading

The physical-phone feedback supplied during this research was: “i typed that from the phone”; “i see my text on the phone as i tye on mobile”; “but scrolling didnt work there”. Live input is therefore useful already; scrolling is the reported failure. Fixing it does not require replacing the terminal.

Rex is the closest reference for the immediate surface: direct control of the same program, native scrolling, selective keys, and compact navigation. A terminal retains interactive prompts, shell/editor behavior, and exact provider output. Its cost is that wrapping, scrollback, selection, and screen redraws require careful phone handling.

Paradev's conversation view is the reference for an optional reading surface: prose wraps naturally, tools can collapse, and comments/photos fit ordinary phone controls. But reconstructing that view introduces a second representation, and a command or interactive choice can exist only in the terminal. If added, it should be a view of the existing Session with a clear return to its live terminal, backed by real structured history. It should not silently replace the active terminal or pretend a parsed transcript supports every interaction.

The next design can therefore keep the native terminal primary and borrow conversation-app hierarchy around it. Native history is a separate choice worth considering after reliable scrolling and continuity are established.

## Remaining uncertainties

- The current Rex iOS build's exact gestures, text selection, key-bar defaults, input latency, and reconnect behavior; the inspected screenshot is an October 30 alpha build, and no app was installed or operated.
- Whether the preferred Loopflow input experience is direct terminal typing with a tiny key bar, an editable send-on-submit draft, or a deliberate switch between them. The references support different choices; they don't settle this.
- On-device quality of Paseo/T3 Code: this pass used official docs and public reports, not hands-on runs. The separate source-code investigation should carry their implementation details.
