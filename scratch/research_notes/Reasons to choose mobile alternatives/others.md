# Reasons to prefer Happy or Replit

Research for Jack Heart · November 3, 2026. Compared with LOO-718's proposed existing Tasks → editable Session terminal MVP. Voice-primary composition is under discussion; no prototype or accepted scope changed. These are current publisher-documented capabilities, not hands-on measurements.

## Why choose Happy for existing machine-hosted agents?

### Takeaway

Happy offers more of the interaction around an agent: send an image, read changed files, adjust its settings, begin another Session, or converse by voice. Several of these remain useful even if Loopflow's terminal and reconnection work perfectly.

### Cited Findings

- The phone can send encrypted images, answer questions and permissions, stop a turn, change model/effort/permission mode, start a Session in a directory, and read files/Git changes. Pairing uses a QR, and existing links are reused. — [Mobile Access](https://happy.engineering/mobile/)
- Happy documents Claude, Codex, Grok, Gemini and ACP agents; some integrations are beta. The app is free, while voice is charged separately. Local execution needs its host online; a remote server is the alternative. — [Mobile app](https://happy.engineering/mobile-app/)
- Its current voice experience goes beyond dictation: a separate conversational voice agent relays requests to the coding agent, reports completion aloud, follows the focused Session, and accepts spoken allow/deny answers. It uses ElevenLabs, with 20 free minutes per 30 days, then a subscription or an externally configured ElevenLabs agent. Audio and selected decrypted Session context, including permission arguments, go to ElevenLabs. — [Voice](https://happy.engineering/voice/)
- Current Happy Agent runs its own harness; the original CLI instead forwards the original agent CLIs and is in maintenance mode. The mobile app supports both. Phone synchronization uses Happy's encrypted relay; it can expose synchronized chats while the server is offline, but initially published history is limited. — [Remote server guide](https://happy.engineering/guides/remote-server/)
- Push notifications for input and completion are explicitly documented in the original CLI documentation. This is evidence for that supported path, not proof that every new Desktop/Agent path has identical notification behavior. — [Original CLI docs](https://happy.engineering/docs/)

### Inferences

- **“Show the agent what I mean.”** Attaching a screenshot avoids explaining a visual bug in words. Terminal text entry alone does not supply this loop.
- **“Let me judge the work.”** A file/change view lets a person inspect the result, rather than rely entirely on the agent's summary or terminal-formatted diff.
- **“I want to keep talking without looking down.”** Spoken replies and permission answers support a different interaction from dictation followed by reading. Loopflow's proposed microphone would close the input gap, not this entire gap.
- Multi-provider access by itself is weak differentiation for Loopflow: Happy already offers it. Its competing advantage is the integrated mobile workflow.

### Gaps

No device trial established selection comfort, accessibility, latency, notification reliability, or voice accuracy. Happy's relay and voice provider are architectural/privacy tradeoffs, not reasons to abandon Jack's Tailscale-only choice. Current Happy's own harness should not be described as merely a terminal attachment to the same provider process.

## Why choose Replit for building from a phone?

### Takeaway

Replit's strongest advantage is completing a visible build loop: begin an idea, inspect the generated web app, request changes and publish it. Its voice implementation is also a direct reference for the composition interaction just proposed for Loopflow.

### Cited Findings

- Replit mobile supports new projects and continuing recent work, importing GitHub/templates, and an artifact switcher for previews. It documents notifications when Agent needs help or finishes, plus Live Activities for progress without opening the app. Usage beyond included Agent credits is billed separately. — [Mobile app documentation](https://docs.replit.com/features/platforms/mobile-app)
- Voice Mode records a prompt into editable text and requires explicit sending. It is available in existing-project and new-project input on mobile app/web and desktop. It does not speak responses. Sessions allow up to six minutes of recording; backgrounding or silence stops recording. — [Voice Mode](https://docs.replit.com/features/agent/voice-mode)
- The mobile product supports building, previewing and deploying websites/web apps to a live URL. Native iOS app creation/submission is directed to the web workflow; do not confuse building from an iPhone with a native-app toolchain inside its iOS app. — [Mobile product](https://replit.com/products/mobile)

### Inferences

- **“I noticed a problem and want to start now.”** Existing-Tasks-only Loopflow intentionally excludes initiating new work. Replit makes that a home-screen action.
- **“I need to see whether it looks right.”** Preview access matters more than terminal fidelity for a person changing a website. Returning to a laptop to inspect the result breaks the mobile loop.
- **“Tell me when I can help.”** Notifications and Live Activities support brief, intermittent attention. A foreground-only client depends on remembered polling.
- Replit's documented dictate/edit/send interaction validates it as a pattern worth testing, not proof that Loopflow's terminal submission integration is solved.

### Gaps

No installed-app trial measured usability or confirmed every feature's reliability. Replit is an integrated development/hosting product, not an interchangeable remote client for an arbitrary Loopflow checkout. Its publishing and billing model should not be imported into Loopflow merely to copy its useful preview loop.

## Which comparisons should influence the next Loopflow proof?

### Takeaway

Prioritize complete mobile actions over additional destinations. The strongest gaps are supplying context, judging results, receiving timely attention cues, and starting work while away.

### Cited Findings

- The current Loopflow study covers selecting an existing Task, terminal interaction and recovery; attachments, result views, creation and push are outside its selected scope. — [Local MVP](../../mobile-prototypes.md)

### Inferences

The most revealing comparison tasks are: dictate and correct a long instruction; attach a screenshot; inspect a changed UI; leave the app and respond when needed; and initiate a newly noticed issue. The first fits the current direction directly. The others expose deliberate scope limits worth discussing, rather than silently adding requirements. Happy and Replit demonstrate why a usable terminal can still be insufficient for some people's preferred mobile workflow.

### Gaps

These are proposed evaluation scenarios, not user-approved feature additions. Ordinary general-purpose chat clients were not researched in this bounded contribution; Claude is covered separately.
