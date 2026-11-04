# Reasons to prefer Claude mobile

Research for Jack Heart · November 3, 2026 · Current official documentation, not hands-on testing. Comparison is against Loopflow's selected existing-Tasks → existing editable terminal MVP, including the newly discussed voice-draft proposal. No scope changes are implied.

## What advantages matter when continuing local coding work?

### Takeaway

The strongest direct competition is a complete mobile interaction around the same local agent: receive a notification, inspect changes, attach evidence, answer the agent, and leave. Remote access to an existing local conversation is already available in Claude; Loopflow must differentiate through its Work model or improve the mobile interaction.

### Cited Findings

- Remote Control synchronizes conversation and subagent/workflow progress across terminal and phone while using the local filesystem, tools and MCP servers. It includes local-path completion, a diff pane, and model/effort controls. These are documented capabilities, not measured ergonomic superiority. [Remote Control](https://code.claude.com/docs/en/remote-control)
- Its connection uses outbound HTTPS through Anthropic, requiring no inbound listener. Transcript storage on Anthropic's servers supports synchronization; execution remains local. It needs a supported subscription/account, a live computer/process, and does not support API-key/third-party endpoint setups. [Remote Control](https://code.claude.com/docs/en/remote-control)
- The phone can attach photos and other files to the local Session. Photos are passed to Claude and saved on the host; other files become local file references. With Remote Control active, push notifications can signal completion or a decision, including an explicit request such as notification after tests. [Mobile](https://code.claude.com/docs/en/mobile)
- Code mobile supports permission-mode selection but excludes bypass mode; some terminal-only commands remain unavailable. Code cloud and Remote Control are both in the Code tab. [Mobile](https://code.claude.com/docs/en/mobile)
- Server mode can create new local sessions from the phone, including isolated worktrees when configured. [Power user tips](https://support.claude.com/en/articles/14554000-claude-code-power-user-tips)

### Inferences

- **“I have 30 seconds to decide whether this is ready.”** A dedicated diff view can reduce the work of asking for a diff, finding its terminal output, and reading it within terminal geometry. This is a design inference, not an observed speed result.
- **“Tell me when I need to return.”** Push means someone can put the phone away. A foreground-only Loopflow app would require checking repeatedly, even if its terminal reconnects perfectly.
- **“This screen is wrong—fix what I’m seeing.”** Sending a screenshot directly to the existing local session is a material input advantage over text-only terminal control. A typed phone sandbox path is not an attachment.
- **“I already use Claude.”** Account-based access can avoid additional Tailscale installation, login and VPN setup. The tradeoff is vendor routing and transcript storage, which some Loopflow users explicitly prefer to avoid.
- Native conversation presentation may make paragraph reading, selecting output, and editing long prompts easier than a terminal grid. Earlier publisher screenshots support the distinction in presentation, but neither they nor these docs establish accessibility quality or measured usability.

### Gaps

- No installed Claude iOS session, real dictation, latency, recovery or accessibility trial was performed. Do not call it proven smoother or more reliable.
- Remote Control has history-switching edge cases and terminal-only commands; it is not an interchangeable general terminal. No need to copy these limitations into Loopflow.

## What can someone do from a phone that the current MVP deliberately excludes?

### Takeaway

Claude can serve as a place to originate work, not merely return to work already opened elsewhere. Cloud hosting also removes the laptop's availability from that workflow, at the cost of a different environment and service dependency.

### Cited Findings

- Cloud coding sessions can start from mobile against GitHub repositories, use isolated environments, and continue independently of the foreground client. They can work on repositories not cloned locally. Repository setup commands and network configuration remain relevant; this is not proof every project works without setup. [Cloud sessions](https://support.claude.com/en/articles/12618689-use-claude-code-in-the-cloud)
- New Code projects coordinate parallel threads, with a main conversation for steering and per-thread details. The September launch was a staged beta, so availability should be checked per account. [Projects announcement](https://claude.com/resources/articles/projects-redesigned)
- Current Cowork documentation lists starting, steering and reviewing tasks, connectors, skills/plugins, file previews, scheduled tasks and projects on mobile. Cloud tasks continue with the computer off; access to local resources still depends on Desktop and has session-origin restrictions. Its table does **not** promise mobile interactive Artifacts. [Cowork surfaces](https://support.claude.com/en/articles/15520349-use-claude-cowork-on-web-desktop-and-mobile)
- Dispatch provides a persistent delegating thread and outcome notifications, but current help restricts it to an existing-user limited beta. It requires an awake computer and open Desktop app. Do not recommend Dispatch as a universally available new setup. [Dispatch](https://support.claude.com/en/articles/13947068-assign-tasks-from-anywhere-in-claude-cowork)

### Inferences

- **“An idea occurred to me away from my desk.”** Starting a new request matters even when no matching Task exists. This exposes a deliberate boundary of Loopflow's existing-Task MVP, rather than a terminal implementation defect.
- **“My laptop is in my bag.”** Managed cloud execution can win immediately. Loopflow can address availability through an always-on private Machine, but that creates an operational requirement for its user.
- **“I want a result, not to supervise a running program.”** A coordinating conversation can summarize outcomes across tasks. Loopflow's Task sidebar offers a useful explicit structure, but selecting a particular Task is extra work when the person does not yet know where the request belongs.
- Starting with a narrower MVP is defensible; these are reasons someone might still keep Claude installed and switch to it.

### Gaps

- Cloud and local code work are different execution contexts. Cloud features do not imply access to arbitrary uncommitted local work or local tools.
- The cloud help page conflicts with itself about automatic PR creation versus a later create-PR action. Prefer the weaker claim that cloud changes can be reviewed and turned into PRs; do not promise a specific automatic completion sequence.
- No total-cost comparison was made. Subscription eligibility does not establish that cloud use is unmetered or that Loopflow is cheaper.

## How do voice and broader context change the choice?

### Takeaway

Voice input is only one part of a phone-native experience. Spoken answers, easy capture from other apps, and context already accumulated in the assistant can be stronger reasons to choose it than remote execution itself.

### Cited Findings

- Claude voice supports two-way spoken interaction, hands-free and push-to-talk modes, interruption, and switching between text and voice. Connected tools can participate. The current FAQ distinguishes surfaces: traditional Code/Cowork support dictation but not voice mode; the new unified Claude experience supports voice in conversations including tasks. This is a rollout-sensitive distinction, not evidence that a Remote Control Code session has two-way voice. [Voice mode](https://support.claude.com/en/articles/11101966-use-voice-mode)
- iOS integration includes Ask Claude through Siri, Spotlight and the Share menu; widgets can open dictation or photo capture. These are general-Claude entry points, not proof they target an existing Code Session. [iOS intents and widgets](https://support.claude.com/en/articles/10263469-use-claude-app-intents-shortcuts-and-widgets-on-ios)
- Claude Mobile supports past-chat search on paid plans and configurable memory. Chat/cloud-Cowork memory is shared; projects have additional boundaries. This documentation does not establish that ordinary chat memory becomes Claude Code's memory. [Chat search and memory](https://support.claude.com/en/articles/11817273-use-claude-s-chat-search-and-memory-to-build-on-previous-context)

### Inferences

- **“I’m walking and want to reason through a design.”** Dictation reduces typing, but a spoken response also removes the need to keep reading the screen. Our voice-draft proposal addresses composition, not that full exchange.
- **“The useful thing is in another app.”** Share-sheet capture can be more valuable than extra in-app controls. A screenshot, excerpt or URL should enter the right existing work with little ceremony.
- **“This assistant already knows the background.”** Established conversations, documents, connected tools and preferences can outweigh a better coding terminal. Loopflow has repository/Task context, but that is not automatically the context someone accumulated in a general assistant.
- Highest-value evaluation sequence for Loopflow: a long dictated prompt with corrections; screenshot-to-existing-Session; completion/decision notification; readable result/diff; starting an unplanned request. Evaluate these as real phone tasks, not a checklist of copied features.

### Gaps

- Do not claim ordinary Claude voice, memory, interactive Artifacts or widgets automatically operate on Code Sessions. The official product surfaces and rollout differ.
- No measured speech quality, battery usage, screen-reader experience or native text-editing comparison was found. Those need real-device trials.
