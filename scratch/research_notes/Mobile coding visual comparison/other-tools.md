# Other mobile coding experiences: Blink, Happy and Replit

## What should a terminal on a phone borrow from Blink?

### Takeaway
Blink supplies the most directly relevant input comparison: terminal output meets the real software keyboard, with a compact special-key strip between them. Loopflow can borrow those ergonomics while retaining its selected Task → Session navigation.

### Cited Findings
- **Observed:** The official quick-start Smart Keys GIF shows a real `blink>` prompt, the full iOS letter keyboard and a slim row with modifier symbols, Tab, punctuation, arrow navigation and Command. It is not a separate chat composer. The local GIF was downloaded and its first frame visually inspected; the board can embed the original animation. [Quick start](https://docs.blink.sh/) · [Original GIF](https://docs.blink.sh/gifs/navigating-blink-shell-image4.gif) · local `assets/blink-smart-keys.gif`.
- **Documented:** Blink’s quick start explains arrows for command history, modifier combinations, a horizontally scrollable special-key area, held modifiers for chained combinations, pinch to resize, and touch selection/drag-and-drop. Smart Keys are shown with the software keyboard and hidden with an external one. These are documented behaviors; the screenshot alone does not establish them. [Quick start](https://docs.blink.sh/).
- **Observed/documented:** Settings screenshots expose mappings for Control, Option, Command, function and cursor keys; docs explain external-keyboard customization. This demonstrates the depth of input handling behind a small terminal screen. [Keyboard customization](https://docs.blink.sh/basics/customize) · local `assets/blink-keyboard-settings.png`, `assets/blink-keyboard-shortcuts.png`.
- **Documented:** Blink pairs SSH/Mosh terminal access with optional VS Code web/server integration and describes Mosh continuity across network transitions. These are the vendor’s capabilities/claims, not proof of Loopflow’s transport behavior. [Blink](https://blink.sh/).
- **Source dating:** Documentation screenshots are undated and use older device/UI styling. Current App Store metadata retrieved November 3, 2026 reports Blink 18.7.1 released September 17, 2026, but individual screenshot publication dates are unknown. The App Store’s five retrieved landscape marketing cards were inspected and are much less useful than the official keyboard GIF. [App Store](https://apps.apple.com/us/app/blink-shell-build-code/id1594898306).

### Inferences
- Keep Loopflow’s compact Back-to-Tasks/title/connection header, but judge the terminal with the system keyboard actually occupying the bottom of the screen. The current HTML omits that viewport pressure and uses an 85px editable field plus a separate four-key bar. It proves the navigation, not comfortable terminal use. [Current prototype](../../prototype-tasks.html).
- Add arrow navigation to the next visual study. Esc, Tab and Ctrl-C alone are insufficient to represent editing earlier text, navigating TUI choices, history or permission dialogs. Exercise modifier sequences and external-keyboard behavior in the native spike; a fixed Ctrl-C button does not cover Control generally.
- Keep selection, scrollback, keyboard dismissal and resize behavior explicit. Do not inherit Blink’s host configuration, local Unix tools, new-shell gesture or multiple terminal windows: they are outside Jack Heart’s selected MVP. [Selected scope](../../mobile-prototypes.md).

### Gaps
No installed Blink session was operated, so latency, selection feel, dictation/IME correctness and current-version keyboard behavior are unmeasured. The `assets/blink-settings-phone.png` image is a settings sheet, despite its original source filename; do not caption it as terminal typing. Use `blink-smart-keys.gif` for input evidence.

## What do Happy’s small-screen agent views establish?

### Takeaway
Happy is a useful list-to-existing-conversation reference. Its message UI and system keyboard fit agent interaction well, but that is a different rendering model from Loopflow’s selected terminal.

### Cited Findings
- **Observed:** The current iPhone App Store screenshot set includes an Active Sessions list followed by Previous Sessions. Rows carry conversation title, working directory and online/last-seen state. This is a strong visual reference for making similar-looking work distinguishable. [App Store](https://apps.apple.com/us/app/happy-codex-claude-code-app/id6748571505) · local `assets/happy-appstore-4.jpg`.
- **Observed:** Another screenshot shows a normal mobile transcript, user message bubble, status just above a compact composer and the full software keyboard. Separate screenshots show collapsible file/tool cards and a native colored diff. This is visibly structured chat rather than a PTY grid. [App Store](https://apps.apple.com/us/app/happy-codex-claude-code-app/id6748571505) · `assets/happy-appstore-2.jpg` for keyboard, `happy-appstore-1.jpg` for tool cards, `happy-appstore-3.jpg` for diff.
- **Documented:** Happy describes QR pairing, sending messages, responding to permissions, switching models and reviewing changes while agents run on the computer. A remote host is an option when a laptop sleeps. Its mobile client can work with Happy Desktop or the original CLI. [Mobile access overview](https://happy.engineering/mobile-app/).
- **Documented:** The original CLI architecture uses an encrypted relay and stored encrypted history for catch-up after intermittent connectivity; the page now labels that CLI as maintenance-mode documentation. Do not transfer this architecture into Loopflow’s host-Tailscale choice merely because the mobile UX is attractive. [How it works](https://happy.engineering/docs/how-it-works/).
- **Dating:** App Store lookup returned version 1.8.0, October 7, 2026. Screenshots may predate that release and are marketing captures, not evidence that every documented current feature appears in this exact screen. Asset provenance, original URLs and retrieval date are in `other-assets.json`.

### Inferences
- Borrow the hierarchy: clear title, small checkout/project context and honest connection/activity status. Loopflow has Task names and IDs as its stronger first-level identity; it should keep those instead of adopting an agent-session browser. [Current prototype](../../prototype-tasks.html).
- A native message composer supplies familiar selection, dictation and multiline editing. Loopflow’s terminal must match the practical usability of those interactions, but copying the chat architecture would undo the selected terminal-first direction. The next comparison should show long editing and an approval/menu inside the actual terminal.
- Do not add Happy’s native diff viewer, model picker, voice mode or additional chats to the MVP solely because they are visible here.

### Gaps
No permission-dialog screenshot was obtained. Happy’s documented permission capability is therefore not visual evidence for an approval layout. No installed app was tested, and screenshots cannot prove continuity, encryption or draft synchronization.

## What changes when the goal is building and previewing apps, as in Replit?

### Takeaway
Replit demonstrates the build → inspect output → request changes loop. That is relevant to later mobile development usefulness, but broader than continuing an existing Loopflow Session from a phone.

### Cited Findings
- **Observed:** Replit’s App Store conversation image has a project selector/title, ordinary prose, a screenshot activity entry, checkpoint status and a compact message composer. A bottom chooser shows several project/output cards, plus an “Open” action. [App Store](https://apps.apple.com/us/app/replit-vibe-code-with-ai-fast/id1614022293) · local `assets/replit-appstore-4.jpg`.
- **Observed:** The next image shows the generated site nearly edge-to-edge, with lightweight back/menu controls and a bottom share/refresh/browser strip. Another screenshot shows voice input on a creation home with Recent Projects at the bottom. [App Store](https://apps.apple.com/us/app/replit-vibe-code-with-ai-fast/id1614022293) · `assets/replit-appstore-5.jpg` for preview, `replit-appstore-3.jpg` for voice/home.
- **Documented:** The current Replit mobile page explicitly scopes its iOS app to websites/web apps. Native mobile app creation/App Store publishing uses the web workflow with Agent, Expo Go previews and guided submission. Distinguish coding **from a phone** from building a **native mobile app**. [Mobile product page](https://replit.com/products/mobile), [current mobile landing page](https://replit.com/mobile).
- **Source quality:** The current landing-page images of a piano, fitness tracker, game and food tracker depict generated outputs, not the Replit authoring UI; do not use them as screenshots of its chat or navigation. The older `/products/mobile` “Chatting with Agent” asset is also a stylized promotional composition. The actual listing UI screenshots above are more useful. [Current landing page](https://replit.com/mobile), [older product page](https://replit.com/products/mobile).
- **Dating:** App Store lookup returned 2.203.2, October 8, 2026; individual screenshot dates unknown. All images were retrieved/inspected November 3. Manifest contains original asset URLs; no app was installed.

### Inferences
- Keep Loopflow’s terminal dominant and its Task context compact. Replit shows why output inspection matters once work changes a UI, but Jack explicitly excluded other window types for this MVP; a Preview pane should not silently enter scope. [Selected scope](../../mobile-prototypes.md).
- For the first native Loopflow proof, success is entering a prompt, editing it, responding to a terminal permission/menu and returning to the same Task. A future step can address opening an agent-created localhost preview over Tailscale and sending observations back. This would need its own design for host URLs and authentication.
- The comparison argues for testing the entire phone interaction, including the system keyboard, rather than investing more in a decorative phone frame. The current HTML is a valid navigation study but not yet a convincing demonstration of terminal ergonomics. [Current prototype](../../prototype-tasks.html).

### Gaps
No current Replit keyboard-visible authoring screenshot was obtained in the bounded research. Its screenshots do not prove how source editing, device rotation or disconnected mode behave. Do not treat its cloud build/publish pipeline as an interchangeable replacement for native Swift development in this repository.
