# Embedded macOS terminals

AppKit offers `performKeyEquivalent` to sibling views, including terminals that
do not own keyboard focus. Consume terminal shortcuts only when the NSView is
the window's first responder. Verify split input by dispatching through the
parent view and inspecting both live PTY buffers; a focus border or a
`makeFirstResponder` assertion does not prove paste routing.

Each Ghostty view owns its surface handle; the window's pool retains views.
Release through that pool, never a global Session-ID registry: two windows can
briefly hold surfaces for the same Session during handoff. Resolve bell/title
identity from Ghostty's surface userdata while the callback handle is valid.

Carry `TerminalIdentity` through views, pools, and notifications. Session,
shell pane, and Task terminal identities remain distinct even when their raw
IDs match. Derive input policy from the case, never an ID prefix.

Keep command-block geometry independent of selection. One optional block ID
and copied-text value owns selection; derive its visual highlight. For a
marked-output PTY fixture, wait for parsed command blocks before testing clicks
and the actual pasteboard; a geometry helper alone does not prove block copy.

GhosttyKit changes live in `swift/GhosttyKitPatches/`. Rebuild the pinned upstream
source with `uv run python scripts/loopflow-dev.py ghostty-build`, publish the
versioned artifact when authorized, then pin its URL and checksum in
`swift/Package.swift`. Do not leave a local `.build` binary path in the manifest.
