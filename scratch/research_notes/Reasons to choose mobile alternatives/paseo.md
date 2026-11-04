# Reasons to prefer Paseo mobile

Research for Jack Heart · November 3, 2026. Compared with the proposed Loopflow MVP: existing open Tasks → existing editable Session terminal, host Tailscale, no initial Task creation, diff/file panes, or push. Voice-primary composition is a proposal, not implemented. This review checked official documentation, releases and source; it did not operate the installed app. Earlier publisher screenshots remain evidence of presentation, not measured usability.

## Which advantages most directly affect work on a phone?

### Takeaway

Paseo's strongest advantage is helping someone complete the whole interaction: understand the agent, give rich feedback, inspect changes, and return when attention is needed. A terminal connection supplies only part of that loop. Five concrete reasons stand out.

### Cited Findings

1. **Readable conversation with less scanning.** Version 0.14 added completed-turn summaries, collapsible exchanges, grouped tool calls, selectable question descriptions and linked questions. It also opens subagents in place on phones with Back navigation. These are concrete ways to reduce the volume and navigation cost of a long agent run. [0.14 release](https://github.com/getpaseo/paseo/releases/tag/v0.14.0)
2. **Inspect the work rather than rely on the agent's description.** The current docs explicitly put agents, files, terminals and diffs in the iOS/Android app. They separately state that the built-in browser runs in Desktop, so the homepage's broad feature-parity claim must not be read as mobile browser parity. [Why Paseo](https://paseo.sh/docs/why)
3. **Speak a substantial request, and optionally hear replies.** Dictation is enabled by default; conversational voice mode is opt-in and can launch/control agents. Local speech uses host-managed ONNX models, while configured cloud speech is optional; the default local STT model is English-only with multilingual choices available. [Voice](https://paseo.sh/docs/voice) The mobile client streams recorded segments to its daemon and receives text back: “local” here must not be described as proven offline recognition on the iPhone. [Dictation transport source](https://github.com/getpaseo/paseo/blob/main/packages/app/src/dictation/dictation-stream-sender.ts)
4. **Bring visual evidence into the conversation.** The native mobile test flow exercises the composer attachment button and system photo picker. This is direct implementation evidence for selecting images, although this test only checks picker dismissal/reopening, not end-to-end image understanding. [Mobile picker test](https://github.com/getpaseo/paseo/blob/main/packages/app/maestro/image-picker-repro.yaml) The shared composer also represents review context and chat-history attachments. [Composer context binding](https://github.com/getpaseo/paseo/blob/main/packages/app/src/composer/attachments/workspace.tsx)
5. **Leave the app and return from a notification.** Native notification taps target the host and agent, resolve its current workspace, and wait for startup navigation to be ready. The documentation includes agent-finished push payloads and warm/cold-launch tests. [Notification contract](https://github.com/getpaseo/paseo/blob/main/docs/notifications.md) The server implements delivery through Expo's push service. [Push service](https://github.com/getpaseo/paseo/blob/main/packages/server/src/server/push/push-service.ts)

### Inferences

- **“I have thirty seconds to catch up.”** A collapsed completed turn and a direct question are plausibly easier to scan than locating the relevant part of a terminal's scrollback. This is a design inference, not a timed usability result.
- **“It says it fixed the bug; show me what changed.”** Mobile diffs/files make verification more independent of agent narration. The terminal MVP can ask for a diff, but must then make that output comfortable to inspect on a narrow display.
- **“This is the broken layout.”** Sending a screenshot plus a dictated correction can convey details that are cumbersome to type. Voice alone does not close the visual-feedback gap.
- **“Tell me when I need to come back.”** Push changes the product from a place to check into something that can recall attention. That can matter more than adding another navigation screen.
- Voice input is an advantage already present in Paseo; it would close a Loopflow gap, not uniquely differentiate Loopflow. Spoken replies and voice orchestration go further than the currently proposed dictation draft.

### Gaps

- No measured comparison of reading speed, accessibility, dictation accuracy, battery, reconnect latency or notification reliability. Do not label Paseo categorically more polished from screenshots or release claims.
- No verified mobile built-in browser preview: official docs explicitly locate that browser on Desktop. Native diff and file review are supported; a full visual app-testing loop from the phone needs a separate proof.
- Source establishes push machinery, not guaranteed delivery or exact notification eligibility across every provider. Do not promise approve/deny directly from a lock-screen notification; that was not verified.

## What makes Paseo easier to adopt or more useful beyond existing work?

### Takeaway

Someone without an existing Loopflow workflow can start from a directory and chosen agent, pair a phone without joining a VPN, and use a consistent interface across providers. For an existing Loopflow user, these advantages are smaller than preserving their current Task and Session identity.

### Cited Findings

- A workspace can use an existing directory or an isolated worktree, can be created in the app, and can exist before any agent starts. Multiple sessions belong to it. [Workspaces](https://paseo.sh/docs/workspaces)
- Paseo's current mobile product presentation includes New workspace, named work, and review/working state groups. This is a publisher-provided product scene, not an installed-app test. [Product](https://paseo.sh/)
- Relay pairing is opt-in and documented as a QR/link flow without Tailscale, port forwarding, or firewall configuration. Direct Tailscale is also supported; its documented setup includes binding the daemon and manually entering the host/port. [Connectivity](https://paseo.sh/docs/connectivity)
- Desktop bundles and starts its daemon; headless installation is available. At least one independently installed/authenticated agent CLI is still required. [Getting started](https://paseo.sh/docs)
- The current README documents one interface across Claude Code, Codex, Copilot, OpenCode, Pi and others, using the host environment. [README](https://github.com/getpaseo/paseo/blob/main/README.md)
- Core Paseo is advertised as free/open-source; agent subscriptions/credentials remain the person's responsibility. Git/GitHub is not required, and ordinary directories are valid. [Product FAQ](https://paseo.sh/)

### Inferences

- **“An idea occurred while I was out.”** Starting a new workspace/agent is a meaningful advantage over an MVP limited to already-open Tasks. No claim is made that Loopflow can never add this.
- **“I only want to use my agent from my phone.”** Requiring a Task system and tailnet can feel like extra adoption work. The relay removes one setup dependency, though it introduces reliance on a hosted transport. Jack's host-Tailscale choice remains valid; this is its product tradeoff.
- **“I switch between agents but want the same phone controls.”** A common native composer and review surface can reduce provider-specific TUI knowledge. Loopflow's provider flexibility would not by itself be unique against Paseo.

### Gaps

- No comparative onboarding trial or pricing comparison with Loopflow was performed. “Fewer steps” follows the documented choices, not observed success rates.
- Existing native Claude/Codex sessions may have import/resume constraints; do not promise attaching any arbitrary already-running terminal unchanged. For Loopflow users, preserving exactly their existing work is still an important potential advantage.

## What limitations should temper the comparison?

### Takeaway

The richer product carries provider adaptation and lifecycle complexity. The lesson is to study the benefits of those interactions without importing its entire workspace, relay or automation platform.

### Cited Findings

- The 0.15 release, dated November 3, reports native terminal selection handles, retained terminal programs across supported daemon restarts and up to 10,000 scrollback lines on reconnect. Its notes qualify the first upgrade and Windows upgrades; agent recovery is opt-in and sends a resume prompt. These are release assertions, not proof of uninterrupted execution in all cases. [0.15 changelog](https://paseo.sh/changelog)
- Version 0.13 reports fixes for iOS notification navigation, missing PR/check status after reconnect, an iPhone picker obscuring the composer, and large-history restoration. These demonstrate concrete failure classes under active development, not that the current release still has those bugs. [0.13 release](https://github.com/getpaseo/paseo/releases/tag/v0.13.0)
- The privacy policy separates optional relay metadata/ciphertext handling from agent-provider traffic. Providers continue receiving their usual API traffic; “local-first” is not a claim that model requests never leave the host. [Privacy policy](https://paseo.sh/privacy)
- Direct network access can be restricted with a daemon password; the docs say any reachable client can connect when no password is configured. Relay enrollment and its trust assumptions are distinct from direct tailnet reachability. [Security](https://paseo.sh/docs/security)

### Inferences

- Loopflow's strongest near-term comparison should be: can someone understand what happened, express the next instruction comfortably, inspect enough evidence to decide, and recover the same work? A terminal implementation can satisfy parts of that, but does not automatically supply all four.
- Prioritize evaluating readable catch-up, voice composition, visual attachments and attention return. Native review/start-work surfaces can remain explicit later scope choices. No accepted MVP requirement or prototype was changed by this research.

### Gaps

- No broad security audit, provider-compatibility audit, or native device trial was performed. The application is richer; that alone does not establish lower risk or higher reliability.
