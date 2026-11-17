# Loopflow mobile exploration

Jack: "lets build loopflow mobile"
Jack: "Explore the mobile experience first"
Jack: "Native iPhone app"

## What to explore
A native iPhone client for existing Loopflow Work. Three proposed experiences appear in explore.html with illustrative content, not live state.

1. Decisions: open on work needing judgment, inspect context, send a specific answer. Cost: needs reliable identification of actionable requests; Task steer is not a substitute for provider permission responses.
2. Conversations: open on existing repository, Wave and Task conversations. Read recent history and send a steer. Cost: structured history and provider-specific interactions need investigation; terminal attachment alone is not a native conversation UI.
3. Direction: open on Wave outcomes, drill into Tasks, redirect effort. Cost: useful summaries must retain source and age; prioritization must map to existing planning operations.

## Current system
swift/Package.swift declares iOS 18 support for the shared library; swift/project.yml has no iPhone app target. docs/dev/server.md assigns mobile and remote admission to LOO-718. The current server is a private Unix socket, not a phone endpoint. Shared DTOs and existing reads/writes remain canonical. Remote admission must authenticate the phone separately from local peer credentials.

## Shape
Additive series. First choose the experience, then specify a single real host-to-phone workflow. No new Task or Wave created. No production architecture selected by this exploration.

## Constraints
Execution stays on the host. Disconnection does not imply execution stopped. Reconnect restores current readings; never replay mutations or input after an uncertain result. Revocation leaves local access and execution intact. Phone credentials belong in Keychain; server admission and secure transport need their own implementation design.

## Demo
Compare three interactive phone sketches. Each action changes only local example content. No backend connection or native build is claimed.

## Remaining
Choose the opening experience and the first useful action. Then detail pairing, transport, native navigation and headless acceptance for that slice. Later possibilities: notifications, review, terminal access.

## Checks
Exploration only: render self-contained HTML and exercise local interaction logic; no application tests required.

