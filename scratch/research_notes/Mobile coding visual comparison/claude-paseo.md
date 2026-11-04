# Claude and Paseo mobile visual references

## What do the first-party mobile images actually show?

### Takeaway

Claude’s Code list and Paseo’s workspace sidebar support the selected Loopflow entry point: a short route from named existing work into interaction. Paseo provides the strongest directly inspected terminal reference; Claude demonstrates native conversation and approval surfaces, not a mobile terminal emulator.

### Cited Findings

All images below were downloaded from publisher-supplied App Store assets and inspected with `view_image` on November 3, 2026. These are marketing screenshots/composites, not hands-on captures. Exact CDN URLs, listing URLs, app version/date and local filenames are in `assets/claude-paseo-sources.json`. Listing release dates do not establish when each image was designed. [Claude listing](https://apps.apple.com/us/app/claude-by-anthropic/id6473753684), [Paseo listing](https://apps.apple.com/app/paseo-pocket-engineer/id6758887924).

| Asset (relative to this notes directory) | Observed content and classification |
|---|---|
| `assets/claude-appstore-7.png` | Code list with All/Blocked/In progress/Done filters. One row has a computer icon and green Connected label; other rows have cloud/repository labels. A magnified question card has one large Approve button. A mixed local/cloud Code marketing composition; not proof of a terminal detail view. |
| `assets/claude-appstore-1.png` | Ordinary Claude chat: large proportional text, restrained navigation, message bubble and map response. Not Claude Code. |
| `assets/claude-appstore-4.png` | Ordinary chat producing a flashcard app/Artifact, shown in an overlay. Not remote local code execution. |
| `assets/claude-appstore-6.png` | Dispatch-themed lock-screen notification marketing, rather than the Code conversation UI. |
| `assets/paseo-appstore-1.jpg` | Dark workspace sidebar, named work grouped by Needs input / Ready to review / Working / Done. Provider icons and small colored state markers; selected row is highlighted. New workspace, history, search and schedules also appear. |
| `assets/paseo-appstore-2.jpg` | Native-looking agent transcript: proportional prose, collapsed Write/Edit/Shell activity rows, compact title/repository header, bottom message composer with model and microphone controls. This is chat, not the terminal view. |
| `assets/paseo-appstore-5.jpg` | Separate monospace terminal with shell/test output. Two accessory rows: Esc, Tab, Ctrl, Up, Shift, keyboard toggle; Alt, Paste, Left, Down, Right, Enter. No software keyboard is visible in the image. |
| `assets/paseo-appstore-3.jpg` | Plugin/deployment sheet above chat; beyond Loopflow’s selected MVP. |
| `assets/paseo-appstore-4.jpg` | Native change-review view plus file-jump sheet; beyond the selected MVP. |

Source for each row: the corresponding publisher’s listing above and exact asset URL in the manifest. Apple lookup returned Claude version `1.261030.19`, released November 1, and Paseo `0.15.0`, released November 3; these describe the listing at retrieval, not verified installed builds. [Claude metadata](https://itunes.apple.com/lookup?id=6473753684&country=us), [Paseo metadata](https://itunes.apple.com/lookup?id=6758887924&country=us).

### Inferences

- The most useful three-panel comparison is Loopflow Task list, Claude Code list (`claude-appstore-7.png`), and Paseo workspace sidebar (`paseo-appstore-1.jpg`). It shows that named work plus attention/connectivity cues is a useful opening surface without requiring a Wave hierarchy.
- For terminal interaction, compare Loopflow directly with `paseo-appstore-5.jpg`, not Paseo’s chat image. Our four buttons omit directional keys, general modifiers, paste and keyboard show/hide. Those omissions matter for navigating agent questions and editing terminal input, even without shells or extra panes.
- Claude’s typography can inform native Task rows and navigation, but transplanting its chat composer wholesale would change the selected terminal interaction model.

### Gaps

No first-party Claude Remote Control conversation screenshot with the software keyboard open was visually verified. The Code list’s Connected row establishes a visible local-session cue; it does not demonstrate cursor behavior, latency, selection, IME, or low-level terminal keys. Neither product’s marketing assets establish actual device ergonomics or network recovery quality.

## What behavior is documented separately from those images?

### Takeaway

Claude Remote Control and Paseo both aim to continue machine-hosted work on the phone, but Claude’s remote surface is a synchronized conversation with product-specific controls. Paseo exposes both native agent chat and a real terminal.

### Cited Findings

- Claude’s mobile documentation explicitly places cloud sessions and Remote Control in the app’s Code tab, with Dispatch separate. Existing local work is reachable by QR or by selecting its named Session. Execution stays on the computer; cloud sessions execute elsewhere. [Claude mobile](https://code.claude.com/docs/en/mobile).
- Claude Remote Control keeps conversation/tool activity synchronized across terminal, browser and phone. It documents automatic reconnect after host sleep/network interruption, with the host and Claude process still required for local execution. This is documented behavior, not demonstrated by the images. [Remote Control](https://code.claude.com/docs/en/remote-control).
- Official deep links can open the Code list or an existing Code Session by ID. This is relevant to restoring the exact selected work rather than falling into an arbitrary new composer. [Claude mobile links, May 6, 2026](https://support.claude.com/en/articles/14898120-open-the-claude-mobile-app-with-a-link).
- A Claude maintainer reports mobile permission-mode selection (Manual, Accept edits, Plan) as available August 17, 2026, following an earlier limitation. Do not use February screenshots/reports to claim these controls are still absent. [Maintainer resolution](https://github.com/anthropics/claude-code/issues/29319).
- Paseo’s official download page links its iOS App Store app and standalone server. Its November 1 `0.15.0-beta.1` notes describe native mobile terminal text rendering, draggable selection handles, rotation support, retained terminal programs across daemon restart and restored scrollback up to 10,000 lines. These are release claims, not our measurements. [Downloads](https://paseo.sh/download), [Changelog](https://paseo.sh/changelog).
- Paseo’s mobile-testing guide explicitly exercises a real docked software keyboard, Ctrl/Esc/Enter, focus identity and keyboard hide/show. This is a better validation benchmark than screenshots with the keyboard absent. [Mobile testing guide](https://github.com/getpaseo/paseo/blob/main/docs/mobile-testing.md).

### Inferences

- Loopflow’s phone should never imply that a disconnected host is merely an idle agent. Use distinct connection state and attention state, borrowing the distinction visible in Claude’s Connected cue and Paseo’s status groups.
- “Editable” should be demonstrated through a real agent question, multiline input, correction, and return from background. A browser textarea test cannot verify those terminal semantics.
- Paseo’s broad feature parity is not the target scope. Its terminal keyboard and continuity contract can inform the narrow Task → Session experience independently of plugins, diffs, shells and schedules.

### Gaps

The current official documentation describes behavior beyond launch-era demos. Exact availability varies by client/version/account; this review did not install either app, enroll devices or authenticate a live remote session. No latency or battery claims are justified.

## What should change in the Loopflow study next?

### Takeaway

Keep the selected two-screen flow. Improve the realistic input state and the information needed to select the right Task; adding more destinations would not address the biggest demonstrated gaps.

### Cited Findings

- Loopflow’s current prototype uses identical Ready for input metadata for all three fixtures, a four-button accessory row and an editable textarea inside a simulated terminal; the page itself states it is not a terminal emulator. [Current local prototype](../../prototype-tasks.html), [Accepted interaction notes](../../mobile-prototypes.md).
- Paseo’s terminal screenshot visibly provides arrows, modifiers, Paste and keyboard toggle, while its workspace screenshot distinguishes attention states. [Paseo publisher assets](https://apps.apple.com/app/paseo-pocket-engineer/id6758887924).
- Claude’s Code screenshot separates local Connected state from cloud/repository labeling and makes a required decision prominent. [Claude publisher assets](https://apps.apple.com/us/app/claude-by-anthropic/id6473753684).

### Inferences

1. Prototype the software keyboard open, leaving an honest amount of terminal visible. Test large Dynamic Type, long Task names, selection, dictation/IME and landscape in the native spike.
2. Add only necessary terminal accessories: directional navigation, Ctrl/Shift (including Shift-Tab), paste and hide/show keyboard. Decide what is always visible versus secondary based on real Codex/Claude interactions.
3. Show a few distinct Task states: running, input needed, disconnected. Retain a clear Task title and Machine identity without a tall detail header.
4. Include a real prompt/menu in the terminal fixture, not only free-text conversation. “The Session works” includes choosing and approving through terminal input.
5. Preserve Task-specific typed input and scroll position, and return to the same Task after brief backgrounding. Keep reconnect/uncertain-send states visible; do not automatically submit preserved text.

### Gaps

These are recommendations for the next study, not newly accepted requirements. No prototype or Task state was changed during this research.
