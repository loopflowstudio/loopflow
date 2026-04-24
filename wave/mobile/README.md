# Mobile

Remote view of loopflow from iOS. Check on waves, browse roadmap, see attention — without opening a laptop.

## Vision

The phone is a read surface first. Conductors check what's happening across waves while walking, in meetings, between things. Later, the phone can participate — tap to resolve, ask the agent a question, approve a mutation — but that earns its place after the read surface is in daily use.

Chat on mobile is a high-priority follow-on, not a near-term target. Build the read surface first.

### Not here

- Running build work from the phone.
- Editing code, rebasing, landing.
- Anything that requires a full keyboard.

## Priorities

1. **Remote lfd connection.** iPhone app authenticates to a remote lfd host and reads wave state, roadmap, attention queue.
2. **Wave + roadmap browse.** Read-only but rich — see what each wave is working on, what's blocked, what shipped recently.
3. **Self-contained dependencies.** Anything `lfd` or `model` needs to expose for remote read-only use lives in this wave's scope, so mobile can ship without waiting on other waves' roadmaps to catch up.

## Risks

- TestFlight / distribution overhead — the app has to actually ship to be useful.
- Auth for remote lfd — tokens, provisioning, discovery all need a clean story.
- Scope creep — "just one more thing" pulls toward build work. The line: if it requires a keyboard, it's not mobile.
