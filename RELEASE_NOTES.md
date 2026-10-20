# v0.14.9

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.9 is a repair release for installed Loopflow. Installed `pursue` Flows were stopping at `loop-or-next` with "captured no Session input". The Desktop planning view could also come up empty because of a query that shipped in v0.14.8. Both are fixed, so upgrade if you run Flows from an installed build or saw empty Working Tasks in Desktop after v0.14.8.

## Installed Flows route again

Flow steps on installed builds lost their transport handoff before they could use it. `transport::initialize()` consumes the handoff and removes its environment variables. The installed entry gate, and default-CLI dispatch, then `exec`ed the artifact with that stripped environment, so every step handed itself off to a new `transport serve`. The step's Session input was captured on a grandchild Process. Routing steps then found no input, and `lf task run`'s retry check missed the FlowProcess on its direct child. (#1604, LOO-522)

- Both exec sites now build their command through one `reexec()`. It restores the handoff with the original parent PID, which is safe because an exec keeps the PID and the parent.
- Routing steps such as `loop-or-next` in installed `pursue` Flows read their captured Session turn again, and `lf task run` retries see the FlowProcess where they expect it.
- If a capture really is missing, the error now names the step Process.

## Desktop planning reads survive agent Processes

The v0.14.8 lookup that follows transport delegation ran `json_extract` on `processes.command`. For agent Processes that column holds a bare name such as `codex`, so on real stores the query failed with "malformed JSON". Desktop's planning read failed with it, leaving Working Tasks empty and a warning on every Wave. The query now checks `json_valid` before extracting. (#1604)

## Operational notes

- No schema, command or wire-format change.
- **Verification:** `transport::tests` cover two cases: the handoff survives an exec with its original parent, and a process that is not a child adopts nothing. The delegation test now adds a bare-command Process and fails without the fix. Fmt and Clippy passed locally. The disposable-OS installation test runs the entry-proof Flow's step through the installed gate and asserts that no `transport serve` child starts. It needs Docker and runs in CI only. (#1604)