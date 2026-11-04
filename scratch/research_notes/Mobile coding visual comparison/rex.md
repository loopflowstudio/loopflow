# Rex mobile visual references

## What screenshots and demos did the Superlogical team share?

### Takeaway
Actual team media is available: a portrait iOS navigation/list clip shared by Mitchell Hashimoto and a portrait-to-wide terminal clip shared by Alasdair Monk. Frames were downloaded from the original Twitter video CDN and visually inspected; these are genuine source frames, not reconstructed mockups. The original X pages return 403 to the research tool, so post identity and dates were read through public indexed mirrors linking the originals.

### Cited Findings
- **Mitchell Hashimoto, October 22, 2026:** iOS external testing announcement, mentioning dedicated iPad and Duo support. The inspected short clip shows a native, dark grouped list: Machine headings (`Studio`, `MBP`), named groups (`Research`, `Rex Graphics`, `Rex Docs`), program icons and row chevrons; bottom controls include an eye symbol and search. At 2 seconds blue dots appear on several program icons. This proves visible attention markers, not their exact semantics. [Original post](https://x.com/mitchellh/status/2113346052090925406); [Alasdair's quoting post](https://x.com/almonk/status/2113347999560843634); [indexed mirror with quote and date](https://nitter.gq/almonk); [original video asset](https://video.twimg.com/amplify_video/2113346032172183552/vid/avc1/1206x2622/UGENJDGb3ySaq39O.mp4?tag=29).
- **Alasdair Monk, October 23, 2026:** a 3-second presentation transforms portrait phone into a wider device view. Portrait shows a Claude Code terminal with its own existing prompt, title and secondary session label, bottom program selectors and terminal keys. Wide shows the same kind of terminal alongside a left sidebar of named groups/program rows and search. The native chrome follows the terminal's dark colors. The visual is a rendered product demonstration, not a filmed hand using hardware. [Original post](https://x.com/almonk/status/2113677839853572233); [indexed mirror](https://nitter.gq/almonk); [original video asset](https://video.twimg.com/amplify_video/2113677773411598337/vid/avc1/2902x2066/9JQhaPyJPbfdS4dO.mp4?tag=29).
- **Independent tester, October 31, 2026:** Reona Shimada's first-hand screenshot shows a portrait shell filling almost all the display, compact back/title/overflow controls, a small pane-map overlay at top-right and a bottom program/pane selector. Software keyboard is hidden. The author reports testing Rex Dev 0.1.0 (1577) and horizontal swipe navigation; the screenshot itself does not prove swipe behavior or typing latency. [Tester report](https://dev.classmethod.jp/en/articles/reona-superlogical-rex-beta-try/); [image asset](https://devio2024-media.developers.io/image/upload/c_limit%2Cw_1200/f_auto/q_auto/v1793377067/2026/10/31/lc1qognvkferz2barrm9.jpg).

Downloaded visual assets, relative to this note:

| Asset | Caption for a comparison board | Provenance |
|---|---|---|
| `assets/rex-team-ios-list.jpg` | Rex iOS navigation: Machines, named groups and program rows. Team clip, 0 seconds. | Mitchell Oct22 video above; 1206×2622 source frame. |
| `assets/rex-team-ios-list-attention.jpg` | Same list with visible blue activity/attention dots. Exact meaning not established visually. | Mitchell Oct22 video above; 2 seconds. |
| `assets/rex-team-terminal-portrait.jpg` | Rex portrait: real agent terminal with its own prompt and a compact terminal-key toolbar. Keyboard hidden. | Alasdair Oct23 video above; 0 seconds, full unmodified 2902×2066 frame with device illustration. |
| `assets/rex-team-duo-terminal.jpg` | Rex wide view: the terminal plus its navigation sidebar; more width reveals navigation. | Alasdair Oct23 video above; 2 seconds, full unmodified frame. |
| `assets/rex-ios-tester.jpg` | Independent tester's Rex iPhone shell: compact chrome, pane map and pane selector. | Reona Shimada Oct31 article; image above. |

### Inferences
- The team list provides a strong visual precedent for familiar native rows leading directly into running programs. Loopflow can use existing Tasks as those rows without inheriting Rex's host/group/tab/pane hierarchy.
- The portrait/wide clip suggests adaptive navigation presentation is more valuable than simply shrinking desktop layout onto a phone. It does not justify adding iPad/foldable scope to the MVP.

### Gaps
- No team-post frame with the full software keyboard expanded was found in the two inspected clips. Do not present those frames as proof of keyboard-visible layout, dictation, IME, text selection or live latency.
- Original post dates are displayed by indexed mirrors; original X page fetches were blocked. Attribution follows the mirror's original-post link and attached CDN media, not an independently authenticated X view.
- Public press kit contains logos/team photographs, not additional mobile UI captures.

## What should Loopflow borrow for its chosen Task to Session flow?

### Takeaway
Loopflow's chosen navigation is already close to Rex's mobile pattern. The important remaining design gap is the terminal when the keyboard is open: a credible prompt, readable viewport, cursor movement and useful keys within thumb reach. The current HTML's textarea and canned reply cannot validate these.

### Cited Findings
- The team terminal frame visibly keeps the agent's terminal input inside the screen. At the bottom are `esc`, `ctrl`, `opt`, `tab`, directional controls and a keyboard toggle; the wide frame reveals all four arrows. It does not replace terminal input with a chat composer. [Alasdair's original demo](https://x.com/almonk/status/2113677839853572233).
- Rex's iOS settings document a reorderable key bar, shortcut groups and punctuation keys; modifier keys, clipboard, Home/End and page navigation are available choices. Settings also describe smooth scrollback, contrast correction, font sizing, autocorrect and restoring the keyboard on return. These are documented capabilities, not demonstrated behavior in the downloaded images. [Official iOS settings](https://www.superlogical.com/rex/docs/reference/ios-settings).
- The command palette documents showing/hiding the keyboard alongside session, tab and pane operations. This supports treating keyboard visibility as explicit UI state. Loopflow's MVP does not need the rest of the multiplexer actions. [Official iOS actions](https://www.superlogical.com/rex/docs/reference/ios-actions).
- The current Loopflow prototype has four accessory buttons (Esc, Tab, Ctrl C, Enter), a browser textarea, no expanded software-keyboard study, and hard-coded ready labels. Its own notes explicitly say these are simulations. [Local prototype](../../prototype-tasks.html); [scope and limits](../../mobile-prototypes.md).

### Inferences
- Keep Task identity as a compact title and one clear return action; let the terminal dominate the screen, as in Rex. The prototype's white, relatively tall header could become more compact and theme-coherent without copying Rex's whole styling system.
- Add a keyboard-visible design/test case before calling the prototype complete. Decide how much terminal remains visible, whether typing scrolls to the live prompt, and how hiding the keyboard restores context.
- Add up/down arrows for agent menus and history, plus left/right cursor motion. A literal Ctrl-C button alone cannot replace terminal modifier support; common Control combinations, Shift-Tab and paste deserve deliberate access. Exact initial key set should be tested on the real target harnesses.
- Show real Session status when available instead of every Task saying ready. Native icons or a restrained attention indicator may improve scanning, but do not infer a whole attention inbox requirement from Rex's blue dots.
- Keep Task switching one level deep. Rex's pane map, tabs, splitting and group management solve its broader multiplexer scope and are unnecessary for Jack Heart's accepted MVP.

### Gaps
- Visual references cannot settle input takeover, terminal geometry ownership, reconnect safety, or native renderer choice. Those still require the protocol and real-device experiments already in LOO-718.
- No claim is made that Rex's terminal text is edited using native selection handles. Its screenshot shows rendered terminal input, not implementation internals.

## How current and complete is this evidence?

### Takeaway
Separate dated team posts, current documentation and hands-on screenshots. The public installation page's availability description conflicts with later team/tester evidence, so it should not be used alone to claim mobile is unavailable.

### Cited Findings
- The October 5 announcement and current installation page describe macOS-first testing and other clients not yet in general testing. [Announcement](https://www.superlogical.com/updates/public-testing-beginning); [installation](https://www.superlogical.com/rex/docs/installation).
- Mitchell's October 28 rollout post says iOS testing has opened to everyone already invited. This is a later team statement, not a statement of general App Store availability. [Original post](https://x.com/mitchellh/status/2115569789574033436); [readable mirror with Oct28 timestamp](https://xitter.cf/mitchellh/status/2115569789574033436).
- The iOS privacy page describes App Store/TestFlight distribution and connection only to explicitly added servers. It supplies documented behavior rather than a screenshot of pairing or network repair. [Official privacy](https://www.superlogical.com/rex/docs/reference/privacy).
- Related design lineage worth comparing separately: Hector Simpson's March 30 post describes Echo, a native mobile SSH client made with Alasdair Monk, and its theme treatment. Echo is a distinct product, not evidence of Rex capabilities. [Original Echo post](https://x.com/dizzyup/status/2038628476798820604); [official Echo site](https://replay.software/echo).

### Inferences
- Rex should be treated as a serious visual/product reference with publicly shared mobile work, not dismissed because one installation page still says desktop-only.

### Gaps
- No Rex iOS binary was installed or operated in this research. Performance, reconnect quality, accessibility and keyboard ergonomics remain unmeasured.
